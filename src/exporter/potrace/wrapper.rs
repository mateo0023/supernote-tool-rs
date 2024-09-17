// src/potrace/wrapper.rs

use super::bindings::*;
use std::mem;
use std::os::raw::c_ulong;

pub struct Bitmap {
    pub bitmap: potrace_bitmap_t,
}

impl Bitmap {
    /// Create a new bitmap with the given width and height.
    pub fn new(width: i32, height: i32) -> Option<Self> {
        unsafe {
            // Calculate dy: words per scanline
            let bits_per_word = mem::size_of::<c_ulong>() * 8;
            let dy = ((width as usize + bits_per_word - 1) / bits_per_word) as i32;

            // Allocate the map: dy * h words
            let size = (dy as usize) * (height as usize);
            let map = libc::calloc(size, mem::size_of::<c_ulong>()) as *mut c_ulong;
            if map.is_null() {
                return None;
            }

            // Initialize the bitmap struct
            let bitmap = potrace_bitmap_t {
                w: width,
                h: height,
                dy,
                map: map as *mut _,
            };

            Some(Self { bitmap })
        }
    }

    /// Set the pixel at (x, y) to the given value (true for black, false for white).
    pub fn set_pixel(&mut self, x: i32, y: i32, value: bool) {
        unsafe {
            let bits_per_word = mem::size_of::<c_ulong>() * 8;
            let dy = self.bitmap.dy as usize;
            let map = self.bitmap.map as *mut c_ulong;

            let index = (y as usize) * dy + (x as usize) / bits_per_word;
            let bit = (x as usize) % bits_per_word;

            let word_ptr = map.add(index);

            if value {
                *word_ptr |= 1 << (bits_per_word - 1 - bit);
            } else {
                *word_ptr &= !(1 << (bits_per_word - 1 - bit));
            }
        }
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
    pub fn new() -> Option<Self> {
        unsafe {
            let ptr = potrace_param_default();
            if ptr.is_null() {
                None
            } else {
                Some(Self { params: ptr })
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

pub fn trace(bitmap: &Bitmap, params: &PotraceParams) -> Option<PotraceState> {
    unsafe {
        let state = potrace_trace(params.params, &bitmap.bitmap);
        if state.is_null() || (*state).status != POTRACE_STATUS_OK as i32 {
            None
        } else {
            Some(PotraceState { state })
        }
    }
}

pub fn write_svg(state: &PotraceState, filename: &str, width: i32, height: i32) -> Result<(), String> {
    use std::fs::File;
    use std::io::Write;

    unsafe {
        let mut file = File::create(filename).map_err(|e| e.to_string())?;

        // Write SVG header
        write!(file, r#"<?xml version="1.0" standalone="no"?>
<svg width="{0}" height="{1}" version="1.1" xmlns="http://www.w3.org/2000/svg">
"#, width, height).map_err(|e| e.to_string())?;

        // Begin path data
        write!(file, r#"<path d=""#).map_err(|e| e.to_string())?;

        // Generate SVG path data
        let mut path = (*state.state).plist;
        while !path.is_null() {
            let curve = &(*path).curve;

            if curve.n > 0 {
                // Start the path
                let c_ptr = curve.c.add(0);
                let c_array = &*c_ptr; // c_array is &[potrace_dpoint_s; 3]
                let start_point = c_array[2]; // Starting point
                write!(file, " M {} {}", start_point.x, start_point.y).map_err(|e| e.to_string())?;

                for i in 0..curve.n {
                    let i = i as usize;
                    let tag: u32 = *curve.tag.add(i) as u32;
                    match tag {
                        POTRACE_CURVETO => {
                            let c_ptr = curve.c.add(i);
                            let c_array = &*c_ptr;

                            let c1 = c_array[0];
                            let c2 = c_array[1];
                            let c3 = c_array[2];

                            write!(file, " C {} {}, {} {}, {} {}", c1.x, c1.y, c2.x, c2.y, c3.x, c3.y).map_err(|e| e.to_string())?;
                        },
                        POTRACE_CORNER => {
                            let c_ptr = curve.c.add(i);
                            let c_array = &*c_ptr;

                            let c1 = c_array[1];
                            let c2 = c_array[2];

                            write!(file, " L {} {}", c1.x, c1.y).map_err(|e| e.to_string())?;
                            write!(file, " L {} {}", c2.x, c2.y).map_err(|e| e.to_string())?;
                        },
                        _ => {},
                    }
                }

                write!(file, " z ").map_err(|e| e.to_string())?; // Close the path
            }

            path = (*path).next;
        }

        // Finish the path element
        write!(file, r#"" fill="black" stroke="none"/>"#).map_err(|e| e.to_string())?;

        // Write SVG footer
        write!(file, "\n</svg>\n").map_err(|e| e.to_string())?;
    }

    Ok(())
}