pub mod bindings;
mod wrapper;

pub use wrapper::Word;

use std::error::Error;

use crate::decoder::{DecodedImage, ColorList, ColorMap};

use crate::common::*;

#[derive(Debug)]
pub enum PotraceError {
    /// There was an error tracing the image
    /// with a potrace error code
    TraceError(i32),
    /// There was an error building the potrace parameters
    /// Sould not occur as code calls the default.
    PotraceParams,
    /// The passed vector was of an unexpected size.
    WrongSize,
}

impl Error for PotraceError{}

impl std::fmt::Display for PotraceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PotraceError::TraceError(e) => write!(
                f,
                "Found error {} while tracing",
                e
            ),
            PotraceError::PotraceParams => write!(f, "Unable to create potrace parameters"),
            PotraceError::WrongSize => write!(f, "The passed Vec<_> was of incorrect length"),
        }
    }
}

use lopdf::content::Operation;
use wrapper::{Bitmap, PotraceParams, PotraceState, trace, generate_combined_paths};

struct MultiColorBitmap {
    white_btmp: Option<Bitmap>,
    l_gray_btmp: Option<Bitmap>,
    d_gray_btmp: Option<Bitmap>,
    black_btmp: Option<Bitmap>,
    white_color: PdfColor,
    l_gray_color: PdfColor,
    d_gray_color: PdfColor,
    black_color: PdfColor,
}

pub fn trace_and_generate(image: DecodedImage, color_map: &ColorMap) -> Result<Vec<Operation>, Box<dyn Error>> {
    let params = PotraceParams::new()?;

    let mut bitmamps: MultiColorBitmap = image.try_into()?;
    bitmamps.add_color_map(color_map);
    let paths = bitmamps.trace(&params)?;

    Ok(generate_combined_paths(paths))
}

impl MultiColorBitmap {
    pub fn add_color_map(&mut self, color_map: &ColorMap) {
        use ColorList::*;

        self.white_color = color_map.get_f_rgb(White);
        self.l_gray_color = color_map.get_f_rgb(LightGray);
        self.d_gray_color = color_map.get_f_rgb(DarkGray);
        self.black_color = color_map.get_f_rgb(Black);
    }

    pub fn trace(self, params: &PotraceParams) -> Result<Vec<(PotraceState, PdfColor)>, Box<dyn Error>> {
        let mut traces = Vec::with_capacity(4);
        if let Some(white_btmp) = self.white_btmp {
            traces.push((trace(&white_btmp, params)?, self.white_color));
        }
        if let Some(l_gray_btmp) = self.l_gray_btmp {
            traces.push((trace(&l_gray_btmp, params)?, self.l_gray_color));
        }
        if let Some(d_gray_btmp) = self.d_gray_btmp {
            traces.push((trace(&d_gray_btmp, params)?, self.d_gray_color));
        }
        if let Some(black_btmp) = self.black_btmp {
            traces.push((trace(&black_btmp, params)?, self.black_color));
        }
        Ok(traces)
    }
}

impl TryFrom<DecodedImage> for MultiColorBitmap {
    type Error = Box<dyn Error>;
    
    /// Will map from [DecodedImage] to [MultiColorBitmap] 
    /// using the default [ColorMap]
    fn try_from(value: DecodedImage) -> Result<Self, Self::Error> {
        use ColorList::*;

        let white_btmp =  if value.used_white  { Some(Bitmap::from_vec(value.white)?)  } else {None};
        let l_gray_btmp = if value.used_l_gray { Some(Bitmap::from_vec(value.l_gray)?) } else {None};
        let d_gray_btmp = if value.used_d_gray { Some(Bitmap::from_vec(value.d_gray)?) } else {None};
        let black_btmp =  if value.used_black  { Some(Bitmap::from_vec(value.black)?)  } else {None};

        let map = ColorMap::default();
        Ok(Self {
            white_btmp,
            l_gray_btmp,
            d_gray_btmp,
            black_btmp,
            white_color: map.get_f_rgb(White),
            l_gray_color: map.get_f_rgb(LightGray),
            d_gray_color: map.get_f_rgb(DarkGray),
            black_color: map.get_f_rgb(Black),
        })
    }
}
