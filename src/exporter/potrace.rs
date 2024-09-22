pub mod bindings;
mod wrapper;

use crate::decoder::{DecodedImage, ColorList, ColorMap};

use crate::common::*;

use lopdf::content::Operation;
use wrapper::{Bitmap, PotraceParams, PotraceState, trace, generate_combined_svg};

struct MultiColorBitmap {
    white_btmp: Bitmap,
    l_gray_btmp: Bitmap,
    d_gray_btmp: Bitmap,
    black_btmp: Bitmap,
    white_color: PdfColor,
    l_gray_color: PdfColor,
    d_gray_color: PdfColor,
    black_color: PdfColor,
}

pub fn trace_and_generate(image: DecodedImage, color_map: &ColorMap) -> Result<Vec<Operation>, String> {
    let params = PotraceParams::new()?;

    let mut bitmamps: MultiColorBitmap = image.try_into()?;
    bitmamps.add_color_map(color_map);
    let paths = bitmamps.trace(&params)?;

    Ok(generate_combined_svg(paths))
}

impl MultiColorBitmap {
    pub fn add_color_map(&mut self, color_map: &ColorMap) {
        use ColorList::*;

        self.white_color = color_map.get_f_rgb(White);
        self.l_gray_color = color_map.get_f_rgb(LightGray);
        self.d_gray_color = color_map.get_f_rgb(DarkGray);
        self.black_color = color_map.get_f_rgb(Black);
    }

    pub fn trace(self, params: &PotraceParams) -> Result<Vec<(PotraceState, PdfColor)>, String> {
        Ok(vec![
            (trace(&self.white_btmp, params)?, self.white_color),
            (trace(&self.l_gray_btmp, params)?, self.l_gray_color),
            (trace(&self.d_gray_btmp, params)?, self.d_gray_color),
            (trace(&self.black_btmp, params)?, self.black_color),
        ])
    }
}

impl TryFrom<DecodedImage> for MultiColorBitmap {
    type Error = String;
    
    /// Will map from [DecodedImage] to [MultiColorBitmap] 
    /// using the default [ColorMap]
    fn try_from(value: DecodedImage) -> Result<Self, Self::Error> {
        use ColorList::*;

        let map = ColorMap::default();
        Ok(Self {
            white_btmp: Bitmap::from_vec(&value.white)?,
            l_gray_btmp: Bitmap::from_vec(&value.l_gray)?,
            d_gray_btmp: Bitmap::from_vec(&value.d_gray)?,
            black_btmp: Bitmap::from_vec(&value.black)?,
            white_color: map.get_f_rgb(White),
            l_gray_color: map.get_f_rgb(LightGray),
            d_gray_color: map.get_f_rgb(DarkGray),
            black_color: map.get_f_rgb(Black),
        })
    }
}
