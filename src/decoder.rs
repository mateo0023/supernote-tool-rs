//! This module contains all the necessary functions to render a Page into
//! a SVG file and other vizualisation formats.

const SPECIAL_LENGTH_MARKER: usize = 0xff;
const SPECIAL_LENGTH: usize = 0x4000;
const SPECIAL_LENGTH_FOR_BLANK: usize = 0x400;

mod color {
    //! Holds the necessary Color items to keep
    //! the namespace clean.
    pub type Color = [u8; 4];

    const            BLACK: Color = [0; 4];
    const        DARK_GRAY: Color = [0x9d, 0x9d, 0x9d, 0];
    const             GRAY: Color = [0xc9, 0xc9, 0xc9, 0];
    const            WHITE: Color = [0xfe, 0xfe, 0xfe, 0];
    const      TRANSPARENT: Color = [0xff, 0xff, 0xff, 0];
    const DARK_GRAY_COMPAT: Color = [0x30, 0x30, 0x30, 0];
    const      GRAY_COMPAT: Color = [0x50, 0x50, 0x50, 0];

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
        pub fn get(&self, colorcode: u8) -> Color {
            match colorcode {
                COLORCODE_BLACK => self.black,
                COLORCODE_BACKGROUND => self.transparent,
                COLORCODE_DARK_GRAY => self.darkgray,
                COLORCODE_GRAY => self.gray,
                COLORCODE_WHITE => self.white,
                COLORCODE_MARKER_BLACK => self.black,
                COLORCODE_MARKER_DARK_GRAY => self.darkgray,
                COLORCODE_MARKER_GRAY => self.gray,
                _ => self.black,
            }
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

pub fn decode_data(data: &[u8], colormap: &ColorMap) -> Vec<u8> {
    use std::collections::VecDeque;
    use crate::data_structures::file_format::{PAGE_HEIGHT, PAGE_WIDTH};

    const EXPECTED_LEN: usize = PAGE_HEIGHT * PAGE_WIDTH * 4;

    let mut data = data.iter();

    let mut uncompressed = Vec::<u8>::new();

    let mut holder: Option<(u8, usize)> = None;
    let mut queue: VecDeque<(u8, usize)> = VecDeque::with_capacity(4);

    while let (Some(&colorcode), Some(&length)) = (data.next(), data.next()) {
        let mut data_pushed = false;
        let mut length = length as usize;

        if let Some((prev_colorcode, mut prev_length)) = holder.take() {
            if colorcode == prev_colorcode {
                length = length + 1 + (((prev_length & 0x7f) + 1) << 7);
                queue.push_back((colorcode, length));
                data_pushed = true;
            } else {
                prev_length = ((prev_length & 0x7f) + 1) << 7;
                queue.push_back((prev_colorcode, prev_length));
            }
        }

        if !data_pushed {
            if length == SPECIAL_LENGTH_MARKER {
                // We're working with blank (the one called without layers)
                length = SPECIAL_LENGTH_FOR_BLANK;
                queue.push_back((colorcode, length));
            } else if length & 0x80 != 0 {
                // To be processed next loop
                holder = Some((colorcode, length));
            } else {
                length += 1;
                queue.push_back((colorcode, length));
            }
        }

        while let Some((colorcode, length)) = queue.pop_front() {
            uncompressed.append(&mut colormap.get(colorcode).repeat(length));
        }
    }
    
    if let Some((colorcode, length)) = holder.take() {
        let length = adjust_tail_length(length, uncompressed.len(), EXPECTED_LEN);
        if length > 0 {
            uncompressed.append(&mut colormap.get(colorcode).repeat(length));
        }
    }

    uncompressed
}

fn adjust_tail_length(tail_length: usize, current_length: usize, total_length: usize) -> usize {
    let gap = total_length - current_length;
    for i in (0..8).rev() {
        let l = ((tail_length & 0x7f) + 1) << i;
        if l <= gap {
            return l;
        }
    }
    0
}
