use crate::data_structures::*;
use crate::decoder::{decode_separate, ColorMap, DecodedImage};
use crate::error::DecoderError;

mod potrace;

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
pub fn page_to_svg(page: &Page, colormap: &ColorMap) -> Result<String, String> {
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

    potrace::trace_and_generate(image, colormap)
}
