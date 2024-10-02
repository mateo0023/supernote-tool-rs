//! This module contains all the necessary functions to render a Page into
//! a SVG file and other vizualisation formats.

const ALL_BLANK: bool = false;

const SPECIAL_LENGTH_MARKER: u8 = 0xff;
const SPECIAL_LENGTH: usize = 0x4000;
const SPECIAL_LENGTH_FOR_BLANK: usize = 0x400;

mod color;

pub use color::{ColorMap, ColorList};

use crate::exporter::PotraceWord;

/// Stores the decoded information from the page or content
#[derive(Debug)]
pub struct DecodedImage {
    /// The amount of pixels pushed
    idx: usize,
    /// The amount of pixels expected
    pixel_count: usize,
    /// The number of pixels across
    width: usize,
    /// Array of wether pixel at bit is that color
    pub white: Vec<PotraceWord>,
    /// A boolean whether we've stored in white
    pub used_white: bool,
    /// Array of wether pixel at bit is that color
    pub l_gray: Vec<PotraceWord>,
    /// A boolean whether we've stored in l_gray
    pub used_l_gray: bool,
    /// Array of wether pixel at bit is that color
    pub d_gray: Vec<PotraceWord>,
    /// A boolean whether we've stored in d_gray
    pub used_d_gray: bool,
    /// Array of wether pixel at bit is that color
    pub black: Vec<PotraceWord>,
    /// A boolean whether we've stored in black
    pub used_black: bool,
}

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

/// Decode a single Image/Layer into a [DecodedImage]
pub fn decode_separate(data: &[u8], width: usize, height: usize) -> Result<DecodedImage, DecoderError> {
    use std::collections::VecDeque;

    let mut data_iter = data.iter();
    let mut image = DecodedImage::new(width, height);

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
            image.push(colorcode, length)?;
        }
    }

    // Handle any remaining holder
    if let Some((colorcode, length_byte)) = holder {
        let length = adjust_tail_length(length_byte, image.len(), image.pixel_count());
        if length > 0 {
            image.push(colorcode, length)?;
        }
    }

    // Check if uncompressed length matches expected length
    if !image.is_full() {
        return Err(DecoderError::UncompressedLengthMismatch {
            actual: image.len(),
            expected: image.pixel_count(),
        });
    }

    // Return the uncompressed data, size, and bits per pixel
    Ok(image)
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

impl DecodedImage {
    pub fn new(width: usize, height: usize) -> Self {
        let bits_per_word = PotraceWord::BITS as usize;
        let words_per_scanline = (width + bits_per_word - 1) / bits_per_word;
        let true_capacity = words_per_scanline * height;
        DecodedImage {
            idx: 0,
            pixel_count: width * height,
            width,
            white: vec![0; true_capacity],
            used_white: false,
            l_gray: vec![0; true_capacity],
            used_l_gray: false,
            d_gray: vec![0; true_capacity],
            used_d_gray: false,
            black: vec![0; true_capacity],
            used_black: false,
        }
    }

    /// Add the given `colorcode` for the specified `length`.
    pub fn push(&mut self, colorcode: u8, length: usize) -> Result<(), DecoderError>{
        use color::ColorList::*;
        match color::ColorList::decode(colorcode)? {
            White => {
                self.used_white = true;
                Self::process(&mut self.white, &mut self.idx, length, self.width)
            },
            LightGray => {
                self.used_l_gray = true;
                Self::process(&mut self.l_gray, &mut self.idx, length, self.width)
            },
            DarkGray => {
                self.used_d_gray = true;
                Self::process(&mut self.d_gray, &mut self.idx, length, self.width)
            },
            Black => {
                self.used_black = true;
                Self::process(&mut self.black, &mut self.idx, length, self.width)
            },
            Transparent => {self.idx = self.pixel_count().min(self.idx + length);},
        };
        Ok(())
    }

    /// Processes consumes itself into an RGBA image
    /// of [ColorType](color::ColorType)
    pub fn into_color(self, colormap: &ColorMap) -> Vec<u8> {
        let mut bitmap = Vec::with_capacity(std::mem::size_of::<color::ColorType>() * self.pixel_count());

        for idx in 0..self.pixel_count() {
            bitmap.extend_from_slice(&colormap.map(self.get_color_at(idx)));
        }
        
        bitmap
    }

    fn get_color_at(&self, idx: usize) -> ColorList {
        use ColorList::*;

        let (idx, mask) = self.get_idx_and_mask(idx);

        if self.black.get(idx).unwrap_or(&0) & mask != 0 {
            return Black;
        }
        if self.d_gray.get(idx).unwrap_or(&0) & mask != 0 {
            return DarkGray;
        }
        if self.l_gray.get(idx).unwrap_or(&0) & mask != 0 {
            return LightGray;
        }
        if self.white.get(idx).unwrap_or(&0) & mask != 0 {
            return White;
        }
        Transparent
    }

    /// Will set `length` bits (corresponding with picels) to 1, from index `start`.
    /// 
    /// Also updates `start` to `+= length`
    fn process(arr: &mut [PotraceWord], start: &mut usize, mut length: usize, width: usize) {
        let bits_per_word = PotraceWord::BITS as usize;
        let words_per_scanline = (width + bits_per_word - 1) / bits_per_word;
        let (mut x, y) = (*start % width, *start / width);

        *start += length;

        // Calculate the index into `map_slice` for the current pixel.
        let mut word_idx = y * words_per_scanline + (x / bits_per_word);

        // Calculate the bit index within the word for the current pixel.
        let mut bit_idx = x % bits_per_word;
        
        let iter_len = length.min(width-x).min(bits_per_word - bit_idx);
        for _ in 0..iter_len {
            arr[word_idx] |= Self::get_mask(bit_idx);

            bit_idx += 1;
        }
        x += iter_len;
        if bit_idx >= bits_per_word || x >= width {
            word_idx += 1;
            bit_idx = 0;
            if x >= width {
                x = 0;
            }
        }
        length -= iter_len;
        
        while length >= bits_per_word {
            arr[word_idx] = PotraceWord::MAX;
            word_idx += 1;
            if x + bits_per_word >= width {
                length -= width - x;
                x = 0;
            } else {
                x += bits_per_word;
                length -= bits_per_word;
            }
        }

        for idx in bit_idx..(bit_idx + length) {
            arr[word_idx] |= Self::get_mask(idx);
        }
    }

    fn get_idx_and_mask(&self, idx: usize) -> (usize, PotraceWord) {
        let bits_per_word = PotraceWord::BITS as usize;
        let words_per_scanline = (self.width + bits_per_word - 1) / bits_per_word;
        let (x, y) = (idx % self.width, idx / self.width);

        // Calculate the index into `map_slice` for the current pixel.
        let word_idx = y * words_per_scanline + x / bits_per_word;

        // Calculate the bit index within the word for the current pixel.
        let bit_idx = x % bits_per_word;

        (word_idx, Self::get_mask(bit_idx))
    }

    fn get_mask(rem: usize) -> PotraceWord {
        // 1 << rem
        1 << (PotraceWord::BITS as usize - 1 - rem)
    }

    pub fn len(&self) -> usize {
        self.idx
    }

    pub fn is_full(&self) -> bool {
        self.idx == self.pixel_count()
    }

    pub const fn pixel_count(&self) -> usize {
        self.pixel_count
    }
}

impl Default for DecodedImage {
    fn default() -> Self {
        use crate::common::f_fmt;
        Self::new(f_fmt::PAGE_WIDTH, f_fmt::PAGE_HEIGHT)
    }
}

impl std::ops::AddAssign for DecodedImage {
    fn add_assign(&mut self, rhs: Self) {
        self.idx = self.idx.max(rhs.idx).min(self.pixel_count);
        for idx in 0..self.white.len() {
            self.white[idx] |= rhs.white[idx];
            self.l_gray[idx] |= rhs.l_gray[idx];
            self.d_gray[idx] |= rhs.d_gray[idx];
            self.black[idx] |= rhs.black[idx];
        }
    }
}
