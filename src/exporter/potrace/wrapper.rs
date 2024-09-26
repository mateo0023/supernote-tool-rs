use lopdf::content::Operation;

use std::error::Error;
use std::mem;
use std::os::raw::c_ulong;

use super::{bindings::*, PdfColor, PotraceError};
use crate::data_structures::file_format_consts as f_fmt;

const PAGE_WIDTH: i32 = f_fmt::PAGE_WIDTH as i32;
const PAGE_HEIGHT: i32 = f_fmt::PAGE_HEIGHT as i32;

pub struct Bitmap {
    pub bitmap: potrace_bitmap_t,
}

impl Bitmap {
    /// Create a new bitmap with the given width and height.
    pub fn new() -> Result<Self, Box<dyn Error>> {
        unsafe {
            // Calculate dy: words per scanline
            let bits_per_word = mem::size_of::<c_ulong>() * 8;
            let dy = ((f_fmt::PAGE_WIDTH + bits_per_word - 1) / bits_per_word) as i32;

            // Allocate the map: dy * h words
            let size = (dy * PAGE_HEIGHT).unsigned_abs() as usize;
            let map = libc::calloc(size, mem::size_of::<c_ulong>()) as *mut c_ulong;
            if map.is_null() {
                return Err(Box::new(PotraceError::MemAlloc));
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

    pub fn from_vec(data: &[bool]) -> Result<Self, Box<dyn Error>> {
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
    fn set_pixels_from_vec(&mut self, data: &[bool]) -> Result<(), Box<dyn Error>> {
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
                        return Err(Box::new(PotraceError::Bounds { word_idx, map_len: map_slice.len() }));
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
    pub fn new() -> Result<Self, Box<dyn Error>> {
        unsafe {
            let ptr = potrace_param_default();
            if ptr.is_null() {
                Err(Box::new(PotraceError::PotraceParams))
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

/// Generate a trace of the given bitmap.
pub fn trace(bitmap: &Bitmap, params: &PotraceParams) -> Result<PotraceState, Box<dyn Error>> {
    unsafe {
        let state = potrace_trace(params.params, &bitmap.bitmap);
        if state.is_null() || (*state).status != POTRACE_STATUS_OK as i32 {
            Err(Box::new(PotraceError::TraceError((*state).status)))
        } else {
            Ok(PotraceState { state })
        }
    }
}

/// Will generate the combined [Operation]s for all the paths in a given image
pub fn generate_combined_paths(
    paths: Vec<(PotraceState, PdfColor)>,
) -> Vec<Operation> {
    use lopdf::content::*;

    let mut operations: Vec<Operation> = Vec::new();

    for (state, fill_color) in paths {
        unsafe {
            let mut path = (*state.state).plist;
            
            if !path.is_null() {
                // Set the color to be used to the path
                operations.push(Operation::new(
                    "rg",
                    fill_color.into_iter().map(|c| c.into()).collect()
                ));
                
                // Loop over all the subpaths with the given color
                while !path.is_null() {
                    let curve = (*path).curve;
    
                    // Should already contain + and - loops in their corresponding
                    // order. This could be a possible issue if assumed wrong.
                    operations.extend(process_curve(&curve));
    
                    path = (*path).next;
                }

                // Fill the paths with the pre-set color
                // uses the nonzero winding number rule.
                operations.push(Operation::new("f", vec![]));
            }
        }
    }

    operations
}

/// Generates the [Operation]s for the given curve
unsafe fn process_curve(curve: &potrace_curve_s) -> Vec<Operation> {
    const Y: f64 = f_fmt::PAGE_HEIGHT as f64;

    if curve.n == 0 {
        return vec![];
    }

    let mut operations = Vec::new();

    // Get the number of segments
    let n = curve.n as usize;

    // Create slices for tags and control points
    let tags = std::slice::from_raw_parts(curve.tag, n);
    let c = std::slice::from_raw_parts(curve.c, n);

    // The starting position is the same as the ending one.
    let c0 = c[n-1][2];
    // Move to the starting position
    operations.push(Operation::new("m", vec![c0.x.into(), (Y - c0.y).into()]));

    for i in 0..n {
        let tag = tags[i].unsigned_abs();

        // Get the control points for this segment
        let c_array = c[i]; // c_array is [potrace_dpoint_t; 3]

        match tag {
            POTRACE_CORNER => {
                let c1 = c_array[1];
                let c2 = c_array[2];

                operations.push(Operation::new("l", vec![c1.x.into(), (Y - c1.y).into()]));
                operations.push(Operation::new("l", vec![c2.x.into(), (Y - c2.y).into()]));
            }
            POTRACE_CURVETO => {
                let c1 = c_array[0];
                let c2 = c_array[1];
                let c3 = c_array[2];

                // Push the Bezier Curve
                operations.push(Operation::new("c", [
                    c1.x, (Y - c1.y), c2.x, (Y - c2.y), c3.x, (Y - c3.y)
                ].into_iter().map(|it| it.into()).collect()));
            }
            _ => {}
        }
    }

    // Close the curve ("subpath" in PDF terms)
    operations.push(Operation::new("h", vec![]));

    operations
}
