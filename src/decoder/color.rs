//! Holds the necessary Color items to keep
//! the namespace clean.
pub type ColorType = [u8; 4];

#[derive(Debug, Clone, Copy)]
pub enum ColorList {
    White, LightGray, DarkGray, Black,
    Transparent,
}

const            BLACK: ColorType = [0, 0, 0, 0xff];
const        DARK_GRAY: ColorType = [0x9d, 0x9d, 0x9d, 0xff];
const             GRAY: ColorType = [0xc9, 0xc9, 0xc9, 0xff];
const            WHITE: ColorType = [0xfe, 0xfe, 0xfe, 0xff];
const      TRANSPARENT: ColorType = [0xff, 0xff, 0xff, 0];

/// The color Code that corresponds to BLACK
const COLORCODE_BLACK: u8 = 0x61;
/// The color Code that corresponds to BACKGROUND
const COLORCODE_BACKGROUND: u8 = 0x62;
/// The color Code that corresponds to DARK_GRAY
const COLORCODE_DARK_GRAY: u8 = 0x63;
/// The color Code that corresponds to GRAY
const COLORCODE_GRAY: u8 = 0x64;
/// The color Code that corresponds to WHITE
const COLORCODE_WHITE: u8 = 0x65;
/// The color Code that corresponds to MARKER_BLACK
const COLORCODE_MARKER_BLACK: u8 = 0x66;
/// The color Code that corresponds to MARKER_DARK_GRAY
const COLORCODE_MARKER_DARK_GRAY: u8 = 0x67;
/// The color Code that corresponds to MARKER_GRAY
const COLORCODE_MARKER_GRAY: u8 = 0x68;

#[derive(Debug)]
pub struct ColorMap {
    black: ColorType,
    darkgray: ColorType,
    gray: ColorType,
    white: ColorType,
    transparent: ColorType,
}

impl ColorMap {
    /// Creates a new ColorMap Object with the given colors and compatibility colors.
    /// 
    /// The colors should be ordered:
    /// 1. [Black](Self::black)
    /// 2. [Dark Grey](Self::darkgray)
    /// 3. [Gray](Self::gray)
    /// 4. [White](Self::white)
    pub fn new(colors: &[ColorType; 4]) -> Self {
        ColorMap {
            black: colors[0],
            darkgray: colors[1],
            gray: colors[2],
            white: colors[3],
            transparent: TRANSPARENT,
        }
    }

    /// Maps the Supernote ColorCode to its corresponding Color
    /// 
    /// Defaults to [Black](Self::black).
    pub fn get(&self, colorcode: u8) -> Result<ColorType, super::DecoderError> {
        match ColorList::decode(colorcode)? {
            ColorList::White => Ok(self.black),
            ColorList::LightGray => Ok(self.gray),
            ColorList::DarkGray => Ok(self.darkgray),
            ColorList::Black => Ok(self.black),
            ColorList::Transparent => Ok(self.transparent),
        }
    }

    /// Similar to [Self::get], but it will repeat the resulting [color](ColorType)
    /// by `length` as to complete the [vector](Vec<u8>) of RBGA values.
    pub fn get_bytes(&self, colorcode: u8, length: usize) -> Result<Vec<u8>, super::DecoderError> {
        Ok(self.get(colorcode)?.repeat(length))
    }

    /// Will return the appropiate [RGBA color](ColorType)
    /// given a [color enum](ColorList).
    pub fn map(&self, c: ColorList) -> ColorType {
        match c {
            ColorList::White => self.white,
            ColorList::LightGray => self.gray,
            ColorList::DarkGray => self.darkgray,
            ColorList::Black => self.black,
            ColorList::Transparent => self.transparent,
        }
    }

    pub fn get_color_hex(&self, color: ColorList) -> String {
        match color {
            ColorList::White => format_color(self.white),
            ColorList::LightGray => format_color(self.gray),
            ColorList::DarkGray => format_color(self.darkgray),
            ColorList::Black => format_color(self.black),
            ColorList::Transparent => format_color(self.transparent),
        }
    }
}

pub fn format_color(color: ColorType) -> String {
    format!("#{:02X}{:02X}{:02X}", color[0], color[1], color[2])
}

impl Default for ColorMap {
    fn default() -> Self {
        ColorMap {
            black: BLACK,
            darkgray: DARK_GRAY,
            gray: GRAY,
            white: WHITE,
            transparent: TRANSPARENT,
        }
    }
}

impl ColorList {
    pub fn decode(colorcode: u8) -> Result<Self, super::DecoderError> {
        use ColorList::*;
        match colorcode {
            COLORCODE_BLACK => Ok(Black),
            COLORCODE_BACKGROUND => Ok(Transparent),
            COLORCODE_DARK_GRAY => Ok(DarkGray),
            COLORCODE_GRAY => Ok(LightGray),
            COLORCODE_WHITE => Ok(White),
            COLORCODE_MARKER_BLACK => Ok(Black),
            COLORCODE_MARKER_DARK_GRAY => Ok(DarkGray),
            COLORCODE_MARKER_GRAY => Ok(LightGray),
            _ => Err(super::DecoderError::UnknownColorCode(colorcode)),
        }
    }
}
