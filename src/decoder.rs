//! This module contains all the necessary functions to render a Page into
//! a SVG file and other vizualisation formats.

use crate::data_structures::file_format_consts::*;

const ALL_BLANK: bool = false;

const SPECIAL_LENGTH_MARKER: u8 = 0xff;
const SPECIAL_LENGTH: usize = 0x4000;
const SPECIAL_LENGTH_FOR_BLANK: usize = 0x400;
const EXPECTED_LEN: usize = PAGE_HEIGHT * PAGE_WIDTH * 4; // 4 bytes per pixel (RGBA)

mod color;

pub use color::{ColorMap, ColorList};

#[derive(Debug)]
pub struct DecodedImage {
    idx: usize,
    pub white: Vec<bool>,
    pub l_gray: Vec<bool>,
    pub d_gray: Vec<bool>,
    pub black: Vec<bool>,
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

pub fn decode_data(data: &[u8], colormap: &ColorMap) -> Result<Vec<u8>, DecoderError> {
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

pub fn decode_separate(data: &[u8]) -> Result<DecodedImage, DecoderError> {
    use std::collections::VecDeque;

    let mut data_iter = data.iter();
    let mut image = DecodedImage::default();

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
        let length = adjust_tail_length(length_byte, image.len(), image.capacity());
        if length > 0 {
            image.push(colorcode, length)?;
        }
    }

    // Check if uncompressed length matches expected length
    if !image.is_full() {
        return Err(DecoderError::UncompressedLengthMismatch {
            actual: image.len(),
            expected: image.capacity(),
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
    pub fn push(&mut self, colorcode: u8, length: usize) -> Result<(), DecoderError>{
        use color::ColorList::*;
        match color::ColorList::decode(colorcode)? {
            White => Self::process(&mut self.white, &mut self.idx, length),
            LightGray => Self::process(&mut self.l_gray, &mut self.idx, length),
            DarkGray => Self::process(&mut self.d_gray, &mut self.idx, length),
            Black => Self::process(&mut self.black, &mut self.idx, length),
            Transparent => {self.idx = self.capacity().min(self.idx + length);},
        };
        Ok(())
    }

    pub fn into_color(self, colormap: &ColorMap) -> Vec<u8> {
        let mut bitmap = Vec::with_capacity(std::mem::size_of::<color::ColorType>() * self.capacity());

        for idx in 0..self.capacity() {
            bitmap.extend_from_slice(&colormap.map(self.get_color_at(idx)));
        }
        
        bitmap
    }

    fn get_color_at(&self, idx: usize) -> ColorList {
        use ColorList::*;

        if let Some(true) = self.black.get(idx) {
            return Black;
        }
        if let Some(true) = self.d_gray.get(idx) {
            return DarkGray;
        }
        if let Some(true) = self.l_gray.get(idx) {
            return LightGray;
        }
        if let Some(true) = self.white.get(idx) {
            return White;
        }
        Transparent
    }

    fn process(arr: &mut [bool], start: &mut usize, length: usize) {
        arr.iter_mut().skip(*start).take(length)
            .for_each(|pixel| *pixel = true);
        *start = arr.len().min(*start + length);
    }

    pub fn len(&self) -> usize {
        self.idx
    }

    pub fn is_full(&self) -> bool {
        self.idx == self.capacity()
    }

    pub const fn capacity(&self) -> usize {
        PAGE_HEIGHT * PAGE_WIDTH
    }
}

impl Default for DecodedImage {
    fn default() -> Self {
        Self {
            idx: 0,
            white: vec![false; PAGE_HEIGHT * PAGE_WIDTH],
            l_gray: vec![false; PAGE_HEIGHT * PAGE_WIDTH],
            d_gray: vec![false; PAGE_HEIGHT * PAGE_WIDTH],
            black: vec![false; PAGE_HEIGHT * PAGE_WIDTH],
        }
    }
}

impl std::ops::Add for DecodedImage {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        let mut res = Self {
            idx: self.idx.max(rhs.idx),
            white: self.white,
            l_gray: self.l_gray,
            d_gray: self.d_gray,
            black: self.black,
        };
        for idx in 0..res.idx {
            res.white[idx] |= rhs.white[idx];
            res.l_gray[idx] |= rhs.l_gray[idx];
            res.d_gray[idx] |= rhs.d_gray[idx];
            res.black[idx] |= rhs.black[idx];
        }
        res
    }
}

impl std::ops::AddAssign for DecodedImage {
    fn add_assign(&mut self, rhs: Self) {
        self.idx = self.idx.max(rhs.idx);
        for idx in 0..self.idx {
            self.white[idx] |= rhs.white[idx];
            self.l_gray[idx] |= rhs.l_gray[idx];
            self.d_gray[idx] |= rhs.d_gray[idx];
            self.black[idx] |= rhs.black[idx];
        }
    }
}

impl std::iter::Sum for DecodedImage {
    fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
        iter.fold(DecodedImage::default(), |acc, i| acc + i)
    }
}
