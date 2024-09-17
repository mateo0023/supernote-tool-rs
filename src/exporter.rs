use crate::data_structures::*;
use crate::decoder::{decode_data, ColorMap};

mod potrace;

pub fn export(notebook: &Notebook, map: &ColorMap) -> () {
    
}

// pub fn page_to_svg(page: &Page, colormap: &ColorMap) -> Vec<Result<vtracer::SvgFile, String>> {
pub fn page_to_svg(page: &Page, colormap: &ColorMap) -> Vec<Result<Vec<u8>, crate::decoder::DecoderError>> {
    use file_format_consts::{PAGE_HEIGHT, PAGE_WIDTH};

    page.layers.iter()
        .filter(|l| !l.is_background())
        .filter_map(|l| l.content.as_ref())
        // .for_each(|d| println!("{}", d))
        .map(|data| decode_data(data, colormap))
        // .map(|decode_res| {
        //     match decode_res {
        //         Ok(bitmap) => {
        //             let img = ColorImage {
        //                 pixels: bitmap,
        //                 width: PAGE_WIDTH,
        //                 height: PAGE_HEIGHT,
        //             };
        //             let config = Config::default();
        //             convert(img, config)
        //         },
        //         Err(err) => Err(err.to_string()),
        //     }
        // })
        .collect()
}
