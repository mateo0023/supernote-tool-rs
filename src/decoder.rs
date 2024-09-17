//! This module contains all the necessary functions to render a Page into
//! a SVG file and other vizualisation formats.

use crate::data_structures::file_format_consts::*;

const SPECIAL_LENGTH_MARKER: u8 = 0xff;
const SPECIAL_LENGTH: usize = 0x4000;
const SPECIAL_LENGTH_FOR_BLANK: usize = 0x400;
const EXPECTED_LEN: usize = PAGE_HEIGHT * PAGE_WIDTH * 4; // 4 bytes per pixel (RGBA)

mod color {
    //! Holds the necessary Color items to keep
    //! the namespace clean.
    pub type Color = [u8; 4];

    const            BLACK: Color = [0, 0, 0, 0xff];
    const        DARK_GRAY: Color = [0x9d, 0x9d, 0x9d, 0xff];
    const             GRAY: Color = [0xc9, 0xc9, 0xc9, 0xff];
    const            WHITE: Color = [0xfe, 0xfe, 0xfe, 0xff];
    const      TRANSPARENT: Color = [0xff, 0xff, 0xff, 0];
    const DARK_GRAY_COMPAT: Color = [0x30, 0x30, 0x30, 0xff];
    const      GRAY_COMPAT: Color = [0x50, 0x50, 0x50, 0xff];

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

    pub struct ColorMap {
        pub black: Color,
        pub darkgray: Color,
        pub gray: Color,
        pub white: Color,
        pub transparent: Color,
        pub darkgray_compat: Color,
        pub gray_compat: Color,
    }

    impl ColorMap {
        /// Creates a new ColorMap Object with the given colors and compatibility colors.
        /// 
        /// The colors should be ordered:
        /// 1. [Black](Self::black)
        /// 2. [Dark Grey](Self::darkgray)
        /// 3. [Gray](Self::gray)
        /// 4. [White](Self::white)
        /// 
        /// The Compatibility Colors should be
        /// 1. [Dark Grey Compatibility](Self::darkgray_compat)
        /// 2. [Grey Compatibility](Self::gray_compat)
        pub fn new(colors: &[Color; 4], compat_colors: &[Color; 2]) -> Self {
            ColorMap {
                black: colors[0],
                darkgray: colors[1],
                gray: colors[2],
                white: colors[3],
                transparent: TRANSPARENT,
                darkgray_compat: compat_colors[0],
                gray_compat: compat_colors[1],
            }
        }

        /// Maps the Supernote ColorCode to its corresponding Color
        /// 
        /// Defaults to [Black](Self::black).
        pub fn get(&self, colorcode: u8) -> Result<Color, super::DecoderError> {
            match colorcode {
                COLORCODE_BLACK => Ok(self.black),
                COLORCODE_BACKGROUND => Ok(self.transparent),
                COLORCODE_DARK_GRAY => Ok(self.darkgray),
                COLORCODE_GRAY => Ok(self.gray),
                COLORCODE_WHITE => Ok(self.white),
                COLORCODE_MARKER_BLACK => Ok(self.black),
                COLORCODE_MARKER_DARK_GRAY => Ok(self.darkgray),
                COLORCODE_MARKER_GRAY => Ok(self.gray),
                _ => Err(super::DecoderError::UnknownColorCode(colorcode)),
            }
        }

        pub fn get_bytes(&self, colorcode: u8, length: usize) -> Result<Vec<u8>, super::DecoderError> {
            Ok(self.get(colorcode)?.repeat(length))
        }
    }

    impl Default for ColorMap {
        fn default() -> Self {
            ColorMap {
                black: BLACK,
                darkgray: DARK_GRAY,
                gray: GRAY,
                white: WHITE,
                transparent: TRANSPARENT,
                darkgray_compat: DARK_GRAY_COMPAT,
                gray_compat: GRAY_COMPAT,
            }
        }
    }
}

pub use color::ColorMap;

#[derive(Debug)]
pub enum DecoderError {
    UncompressedLengthMismatch { actual: usize, expected: usize },
    UnknownColorCode(u8),
    DataEndedUnexpectedly,
    // LengthOverflow,
}

impl std::fmt::Display for DecoderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecoderError::UncompressedLengthMismatch { actual, expected } => {
                write!(
                    f,
                    "Uncompressed bitmap length = {}, expected = {}",
                    actual, expected
                )
            }
            DecoderError::UnknownColorCode(code) => write!(f, "Unknown color code: {:#04x}", code),
            DecoderError::DataEndedUnexpectedly => write!(f, "Data ended unexpectedly"),
            // DecoderError::LengthOverflow => write!(f, "Length overflow detected"),
        }
    }
}

impl std::error::Error for DecoderError {}

pub fn decode_data(data: &[u8], colormap: &ColorMap) -> Result<Vec<u8>, DecoderError> {
    const ALL_BLANK: bool = false;

    use std::collections::VecDeque;

    let mut data_iter = data.iter();
    let mut uncompressed = Vec::<u8>::with_capacity(EXPECTED_LEN);

    let mut holder: Option<(u8, u8)> = None;
    let mut queue: VecDeque<(u8, usize)> = VecDeque::with_capacity(4);

    while let Some(&colorcode) = data_iter.next() {
        let length_byte = match data_iter.next() {
            Some(&l) => l,
            None => return Err(DecoderError::DataEndedUnexpectedly),
        };
        let mut data_pushed = false;

        if let Some((prev_colorcode, prev_length)) = holder.take() {
            if colorcode == prev_colorcode {
                let length = 1 + (length_byte as usize)
                    + (((prev_length & 0x7f) as usize + 1) << 7);
                queue.push_back((colorcode, length));
                data_pushed = true;
            } else {
                let prev_length = ((prev_length & 0x7f) as usize + 1) << 7;
                queue.push_back((prev_colorcode, prev_length));
            }
        }

        if !data_pushed {
            if length_byte == SPECIAL_LENGTH_MARKER {
                let length = if ALL_BLANK {
                    SPECIAL_LENGTH_FOR_BLANK
                } else {
                    SPECIAL_LENGTH
                };
                queue.push_back((colorcode, length));
            } else if length_byte & 0x80 != 0 {
                holder = Some((colorcode, length_byte));
                // Held data will be processed in the next loop iteration
            } else {
                let length = (length_byte as usize) + 1;
                queue.push_back((colorcode, length));
            }
        }

        while let Some((colorcode, length)) = queue.pop_front() {
            let color_bytes = colormap.get_bytes(colorcode, length)?;
            uncompressed.extend(color_bytes);
        }
    }

    // Handle any remaining holder
    if let Some((colorcode, length_byte)) = holder {
        let length = adjust_tail_length(length_byte, uncompressed.len(), EXPECTED_LEN);
        if length > 0 {
            let color_bytes = colormap.get_bytes(colorcode, length)?;
            uncompressed.extend(color_bytes);
        }
    }

    // Check if uncompressed length matches expected length
    if uncompressed.len() != EXPECTED_LEN {
        return Err(DecoderError::UncompressedLengthMismatch {
            actual: uncompressed.len(),
            expected: EXPECTED_LEN,
        });
    }

    // Return the uncompressed data, size, and bits per pixel
    Ok(uncompressed)
}

fn adjust_tail_length(tail_length: u8, current_length: usize, total_length: usize) -> usize {
    let gap = total_length - current_length;
    for i in (0..8).rev() {
        let l = ((tail_length & 0x7f) as usize + 1) << i;
        if l <= gap {
            return l;
        }
    }
    0
}
