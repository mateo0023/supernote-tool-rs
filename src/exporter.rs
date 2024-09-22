use crate::data_structures::*;
use crate::decoder::{decode_separate, ColorMap, DecodedImage};
use crate::error::DecoderError;

const A4_WIDTH: f32 = 210.;
const A4_HEIGHT: f32 = 297.;

mod potrace;

use lopdf::content::Content;
use lopdf::{dictionary, Document, Stream};

pub fn to_pdf(notebook: &Notebook, colormap: &ColorMap) -> Result<Document, String> {
    let mut pdf = Document::with_version("1.7");
    let base_page_id = pdf.new_object_id();

    let (page_commands, errors) = notebook.pages.iter().map(|page| 
        page_to_svg(page, colormap)
    ).fold((vec![], vec![]), |(mut pages, mut errors), page_res| {
        match page_res {
            Ok(c) => pages.push(c),
            Err(e) => errors.push(e),
        }
        (pages, errors)
    });

    if !errors.is_empty() {
        return Err(errors.join("\n"))
    }

    let mut pages = Vec::with_capacity(page_commands.len());
    for content in page_commands {
        let encode = match content.encode() {
            Ok(e) => e,
            Err(err) => return Err(err.to_string()),
        };

        let content_id = pdf.add_object(Stream::new(dictionary! {}, encode));

        let page_id = pdf.add_object(dictionary!{
            "Type" => "Page",
            "Parent" => base_page_id,
            "Contents" => content_id,
        });
        pages.push(page_id);
    }

    pdf.compress();

    Ok(pdf)
}

pub fn get_bitmap(page: &Page, colormap: &ColorMap) -> Result<Vec<u8>, Vec<DecoderError>> {
    let (image, errors) = page.layers.iter()
        .filter(|l| !l.is_background())
        .filter_map(|l| l.content.as_ref())
        // Decode layers
        .map(|data| decode_separate(data))
        // Ignore errors
        .fold((DecodedImage::default(), vec![]), |(mut acc_img, mut acc_err), dec_res| {
            match dec_res {
                Ok(img) => acc_img += img,
                Err(e) => acc_err.push(e),
            };
            (acc_img, acc_err)
        });

    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(image.into_color(colormap))
}

/// Exports a given page to a SVG String
pub fn page_to_svg(page: &Page, colormap: &ColorMap) -> Result<Content, String> {
    let (image, errors) = page.layers.iter()
        .filter(|l| !l.is_background())
        .filter_map(|l| l.content.as_ref())
        // .for_each(|d| println!("{}", d))
        .map(|data| decode_separate(data))
        .fold((DecodedImage::default(), vec![]), |(mut img, mut errors), item| {
            match item {
                Ok(layer) => img += layer,
                Err(error) => errors.push(error),
            };
            (img, errors)
    });
    
    if ! errors.is_empty() {
        return Err(format!(
            "Encountered {} when exporting page to SVG: {:?}",
            if errors.len() == 1 {"an Error"} else {"Errors"}, errors
        ));
    }

    potrace::trace_and_generate(image, colormap).map(|operations| {
        Content {
            operations,
        }
    })
}
