use crate::data_structures::*;
use crate::decoder::{decode_separate, ColorMap, DecodedImage};
use crate::error::DecoderError;

const A4_WIDTH: i32 = crate::common::f_fmt::PAGE_WIDTH as i32;
const A4_HEIGHT: i32 = crate::common::f_fmt::PAGE_HEIGHT as i32;

mod potrace;

use lopdf::content::Content;
use lopdf::{dictionary, Document, Object, ObjectId, Stream};

pub fn to_pdf(notebook: &Notebook, colormap: &ColorMap) -> Result<Document, String> {
    let mut doc = Document::with_version("1.7");
    let base_page_id = doc.new_object_id();

    let pages = add_pages(base_page_id, &mut doc, notebook, colormap)?;

    let links = &notebook.links;

    let page_count = pages.len();

    // Add the pages object to the document
    doc.objects.insert(base_page_id, Object::Dictionary(dictionary!{
        // Type of dictionary
        "Type" => "Pages",
        // Vector of page IDs in document. Normally would contain more than one ID
        // and be produced using a loop of some kind.
        "Kids" => pages.into_iter().map(|p| p.into()).collect::<Vec<_>>(),
        // Page count
        "Count" => page_count as i64,
        // A rectangle that defines the boundaries of the physical or digital media.
        // This is the "page size".
        "MediaBox" => vec![0.into(), 0.into(), A4_WIDTH.into(), A4_HEIGHT.into()]
    }));

    // Creating document catalog.
    // There are many more entries allowed in the catalog dictionary.
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => base_page_id,
    });

    // The "Root" key in trailer is set to the ID of the document catalog,
    // the remainder of the trailer is set during `doc.save()`.
    doc.trailer.set("Root", catalog_id);

    // pdf.compress();

    Ok(doc)
}

fn add_pages(pages_id: ObjectId, doc: &mut Document, notebook: &Notebook, colormap: &ColorMap) -> Result<Vec<ObjectId>, String> {
    let (page_commands, errors) = notebook.pages.iter().map(|page| 
        page_to_commands(page, colormap)
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

    let mut pages: Vec<ObjectId> = Vec::with_capacity(page_commands.len());
    for content in page_commands {
        let encoded = match content.encode() {
            Ok(e) => e,
            Err(err) => return Err(err.to_string()),
        };

        let content_id = doc.add_object(Stream::new(dictionary! {}, encoded));

        let page_id = doc.add_object(dictionary!{
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), A4_WIDTH.into(), A4_HEIGHT.into()],
            "Contents" => content_id,
        });
        pages.push(page_id);
    }

    Ok(pages)
}


/// Function to add an internal link annotation to a page
fn add_internal_link(
    doc: &mut Document,
    page_id: ObjectId,
    rect: [f64; 4],
    destination_page_id: ObjectId,
) -> Result<(), String> {
    // Define the GoTo action
    let action = dictionary! {
        "Type" => "Action",
        "S" => "GoTo",
        "D" => vec![Object::Reference(destination_page_id), Object::Name("Fit".into())],
    };

    let action_id = doc.add_object(action);

    // Define the link annotation
    let annotation = dictionary! {
        "Type" => "Annot",
        "Subtype" => "Link",
        "Rect" => rect.iter().map(|v| (*v).into()).collect::<Vec<_>>(),
        "Border" => vec![0.into(), 0.into(), 0.into()], // No border
        "A" => Object::Reference(action_id),
    };

    let annotation_id = doc.add_object(annotation);

    // Add the annotation to the page's /Annots array
    if let Some(Object::Dictionary(ref mut page_dict)) = doc.objects.get_mut(&page_id) {
        // Retrieve or create the /Annots array
        let annots = page_dict.as_hashmap_mut().entry("Annots".into()).or_insert_with(|| Object::Array(vec![]));

        if let Object::Array(ref mut annots_array) = annots {
            annots_array.push(Object::Reference(annotation_id));
        } else {
            // If /Annots exists but is not an array, return an error
            return Err("Page /Annots is not an array".into());
        }
    } else {
        return Err("Page object is not a dictionary".into());
    }

    Ok(())
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
fn page_to_commands(page: &Page, colormap: &ColorMap) -> Result<Content, String> {
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
