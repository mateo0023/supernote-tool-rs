// src/potrace/wrapper.rs

use super::bindings::*;
use std::mem;
use std::os::raw::c_ulong;

use crate::data_structures::file_format_consts as f_fmt;

const PAGE_WIDTH: i32 = f_fmt::PAGE_WIDTH as i32;
const PAGE_HEIGHT: i32 = f_fmt::PAGE_HEIGHT as i32;

pub struct Bitmap {
    pub bitmap: potrace_bitmap_t,
}

impl Bitmap {
    /// Create a new bitmap with the given width and height.
    pub fn new() -> Result<Self, String> {
        unsafe {
            // Calculate dy: words per scanline
            let bits_per_word = mem::size_of::<c_ulong>() * 8;
            let dy = ((f_fmt::PAGE_WIDTH + bits_per_word - 1) / bits_per_word) as i32;

            // Allocate the map: dy * h words
            let size = (dy * PAGE_HEIGHT).unsigned_abs() as usize;
            let map = libc::calloc(size, mem::size_of::<c_ulong>()) as *mut c_ulong;
            if map.is_null() {
                return Err("Was not able to allocate memory for Bitmap (potrace binding)".to_string());
            }

            // Initialize the bitmap struct
            let bitmap = potrace_bitmap_t {
                w: PAGE_WIDTH,
                h: PAGE_HEIGHT,
                dy,
                map: map as *mut _,
            };

            Ok(Self { bitmap })
        }
    }

    pub fn from_vec(data: &[bool]) -> Result<Self, String> {
        // Create a new bitmap
        let mut bitmap = Self::new()?;

        // Set the pixels from the data vector
        bitmap.set_pixels_from_vec(data)?;

        Ok(bitmap)
    }

    /// Sets the pixels of the bitmap from a `Vec<bool>`.
    ///
    /// Each element in `data` represents a pixel in the bitmap:
    /// - `true` for a black pixel (set bit)
    /// - `false` for a white pixel (cleared bit)
    ///
    /// # Parameters
    ///
    /// - `data`: A slice of booleans representing the pixel data.
    /// Its length must be equal to `width * height`.
    ///
    /// # Returns
    ///
    /// - `Ok(())` on success.
    /// - `Err(String)` with an error message if an error occurs.
    ///
    /// # Safety
    ///
    /// This method uses unsafe code to manipulate raw pointers and must be used with care.
    fn set_pixels_from_vec(&mut self, data: &[bool]) -> Result<(), String> {
        use std::slice;

        unsafe {
            // Obtain a mutable reference to the underlying Potrace bitmap.
            let bitmap = &mut self.bitmap;

            // Number of bits in a `potrace_word`.
            // Typically 32 or 64 bits, depending on the platform.
            let word_bits = 8 * std::mem::size_of::<potrace_word>();

            // Get the number of words per scanline (row).
            // `bitmap.dy` is the number of words per scanline.
            let dy_words = bitmap.dy.unsigned_abs() as usize;

            // Create a mutable slice over the bitmap's raw data.
            // The total length is `dy_words * bitmap.h`,
            // which is the total number of words in the bitmap.
            let map_slice = slice::from_raw_parts_mut(
                bitmap.map,
                dy_words * bitmap.h as usize,
            );

            // Get the width and height of the bitmap as usize for indexing.
            let width = bitmap.w as usize;
            let height = bitmap.h as usize;

            // Iterate over each pixel coordinate (x, y).
            for y in 0..height {
                for x in 0..width {
                    // Calculate the index into the `data` array for the current pixel.
                    let idx = y * width + x;

                    // Get the value of the pixel from `data`.
                    let value = data[idx];

                    // Calculate the index into `map_slice` for the current pixel.
                    let word_idx = y * dy_words + x / word_bits;

                    // Calculate the bit index within the word for the current pixel.
                    let bit_idx = word_bits - 1 - x%word_bits;

                    // Boundary check: Ensure `word_idx` is within the bounds of `map_slice`.
                    if word_idx >= map_slice.len() {
                        return Err(format!(
                            "word_idx {} out of bounds (len {}), x={}, y={}, dy_words={}, width={}, height={}",
                            word_idx, map_slice.len(), x, y, dy_words, width, height
                        ));
                    }

                    // Create a bitmask for the current pixel.
                    let mask = 1 << bit_idx;

                    if value {
                        // Set the bit to 1 (black pixel).
                        map_slice[word_idx] |= mask;
                    } else {
                        // Clear the bit to 0 (white pixel).
                        map_slice[word_idx] &= !mask;
                    }
                }
            }
        }

        Ok(())
    }
}

impl Drop for Bitmap {
    fn drop(&mut self) {
        unsafe {
            if !self.bitmap.map.is_null() {
                libc::free(self.bitmap.map as *mut _);
            }
        }
    }
}

pub struct PotraceState {
    pub state: *mut potrace_state_t,
}

impl Drop for PotraceState {
    fn drop(&mut self) {
        unsafe {
            if !self.state.is_null() {
                potrace_state_free(self.state);
            }
        }
    }
}

pub struct PotraceParams {
    pub params: *mut potrace_param_t,
}

impl PotraceParams {
    pub fn new() -> Result<Self, String> {
        unsafe {
            let ptr = potrace_param_default();
            if ptr.is_null() {
                Err("Unable to create potrace parameters".to_string())
            } else {
                Ok(Self { params: ptr })
            }
        }
    }
}

impl Drop for PotraceParams {
    fn drop(&mut self) {
        unsafe {
            if !self.params.is_null() {
                potrace_param_free(self.params);
            }
        }
    }
}

pub fn trace(bitmap: &Bitmap, params: &PotraceParams) -> Result<PotraceState, String> {
    unsafe {
        let state = potrace_trace(params.params, &bitmap.bitmap);
        if state.is_null() || (*state).status != POTRACE_STATUS_OK as i32 {
            Err(format!("Unable to trace the bitmap. State: {}", (*state).status))
        } else {
            Ok(PotraceState { state })
        }
    }
}

pub fn generate_combined_svg(
    paths: Vec<(PotraceState, String)>,
) -> Result<String, String> {
    use std::fmt::Write;

    let mut svg_data = String::new();

    // Write SVG header
    write!(
        svg_data,
        r#"<?xml version="1.0" standalone="no"?>
<svg width="{0}" height="{1}" version="1.1" xmlns="http://www.w3.org/2000/svg">
"#,
        PAGE_WIDTH, PAGE_HEIGHT
    ).map_err(|e| e.to_string())?;

    for (state, fill_color) in paths {
        unsafe {
            let mut path = (*state.state).plist;

            while !path.is_null() {
                let curve = &(*path).curve;

                if curve.n == 0 {
                    path = (*path).next;
                    continue;
                }

                // Start the path element
                write!(svg_data, r#"<path d=""#).map_err(|e| e.to_string())?;

                let n = curve.n as usize;

                for i in 0..n {
                    let tag = *curve.tag.add(i) as u32;

                    // Get the control points for this segment
                    let c_array = *curve.c.add(i); // c_array is [potrace_dpoint_t; 3]

                    match tag {
                        POTRACE_CORNER => {
                            if i == 0 {
                                // Move to the first point
                                let c0 = c_array[2];
                                write!(svg_data, " M {} {}", c0.x, c0.y).map_err(|e| e.to_string())?;
                            }
                            let c1 = c_array[1];
                            let c2 = c_array[2];

                            write!(svg_data, " L {} {}", c1.x, c1.y).map_err(|e| e.to_string())?;
                            write!(svg_data, " L {} {}", c2.x, c2.y).map_err(|e| e.to_string())?;
                        }
                        POTRACE_CURVETO => {
                            if i == 0 {
                                // Move to the first point
                                let c0 = c_array[2];
                                write!(svg_data, " M {} {}", c0.x, c0.y).map_err(|e| e.to_string())?;
                            }
                            let c1 = c_array[0];
                            let c2 = c_array[1];
                            let c3 = c_array[2];

                            write!(
                                svg_data,
                                " C {} {}, {} {}, {} {}",
                                c1.x, c1.y, c2.x, c2.y, c3.x, c3.y
                            ).map_err(|e| e.to_string())?;
                        }
                        _ => {}
                    }
                }

                write!(svg_data, " z ").map_err(|e| e.to_string())?; // Close the path

                // Finish the path element with the fill color
                write!(
                    svg_data,
                    r#"" fill="{}" stroke="none"/>"#,
                    fill_color
                ).map_err(|e| e.to_string())?;

                path = (*path).next;
            }
        }
    }

    // Write SVG footer
    write!(svg_data, "\n</svg>\n").map_err(|e| e.to_string())?;

    Ok(svg_data)
}
