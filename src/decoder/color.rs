//! Holds the necessary Color items to keep
//! the namespace clean.

use crate::common::PdfColor;
pub type ColorType = [u8; 4];

#[derive(Debug, Clone, Copy)]
pub enum ColorList {
    White, LightGray, DarkGray, Black,
    Transparent,
}

const      TRANSPARENT: ColorType = [0xff, 0xff, 0xff, 0];

/// The color Code that corresponds to BLACK
const COLORCODE_BLACK: u8 = 0x61;
/// The color Code that corresponds to BACKGROUND
const COLORCODE_BACKGROUND: u8 = 0x62;
/// The color Code that corresponds to DARK_GRAY
const COLORCODE_DARK_GRAY: u8 = 0x9D;
/// The color Code that corresponds to GRAY
const COLORCODE_GRAY: u8 = 0xC9;
/// The color Code that corresponds to WHITE
const COLORCODE_WHITE: u8 = 0x65;
/// The color Code that corresponds to MARKER_BLACK
const COLORCODE_MARKER_BLACK: u8 = 0x66;
/// The color Code that corresponds to MARKER_DARK_GRAY
const COLORCODE_MARKER_DARK_GRAY: u8 = 0x9E;
/// The color Code that corresponds to MARKER_GRAY
const COLORCODE_MARKER_GRAY: u8 = 0xCA;

#[derive(Debug)]
pub struct ColorMap {
    black: ColorType,
    darkgray: ColorType,
    gray: ColorType,
    white: ColorType,
    transparent: ColorType,
}

impl ColorMap {
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

    pub fn get_f_rgb(&self, color: ColorList) -> PdfColor {
        let c = match color {
            ColorList::White => self.white,
            ColorList::LightGray => self.gray,
            ColorList::DarkGray => self.darkgray,
            ColorList::Black => self.black,
            ColorList::Transparent => self.transparent,
        };
        [
            c[0] as f64 / 255.,
            c[1] as f64 / 255.,
            c[2] as f64 / 255.,
        ]
    }
}

impl Default for ColorMap {
    fn default() -> Self {
        ColorMap {
            black: [0x00, 0x00, 0x00, 0xff],
            darkgray: [0x46, 0x69, 0xd6, 0xff],
            gray: [0xfd, 0xfa, 0x75, 0xff],
            white: [0xfe, 0xfe, 0xfe, 0xff],
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
