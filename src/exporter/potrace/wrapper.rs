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

pub type Word = potrace_word;

impl Bitmap {
    /// Create a [Bitmap] from the vector.
    /// 
    /// # Returns
    /// * `Error`: if the given vector is not the size for an
    /// Supernote A5X document.
    pub fn from_vec(data: Vec<Word>) -> Result<Self, PotraceError> {
        // Calculate dy: words per scanline
        let bits_per_word = mem::size_of::<c_ulong>() * 8;
        let dy = ((f_fmt::PAGE_WIDTH + bits_per_word - 1) / bits_per_word) as i32;
        
        // Allocate the map: dy * h words
        let size = (dy * PAGE_HEIGHT).unsigned_abs() as usize;
        if data.len() != size {
            return Err(PotraceError::WrongSize);
        }

        let mut vec = std::mem::ManuallyDrop::new(data);

        // Initialize the bitmap struct
        let bitmap = potrace_bitmap_t {
            w: PAGE_WIDTH,
            h: PAGE_HEIGHT,
            dy,
            map: vec.as_mut_ptr(),
        };

        Ok(Self { bitmap })
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

    // There seems to be around 2_500 - 2_600 operations per PotraceState
    let mut operations: Vec<Operation> = Vec::with_capacity(estimate_capacity(&paths)); 

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
                    process_curve(&curve, &mut operations);
    
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

/// Will compute the estimated number of Operations needed.
/// Will loop over the [PotraceState] and their `paths`.
/// 
/// Assumes paths are generally curved, with only 5% of
/// the paths being starights. This should reduce the
/// amount of memory allocations.
fn estimate_capacity(paths: &[(PotraceState, PdfColor)]) -> usize {
    let mut accum = 1;
    for (state, _) in paths.iter() {
        unsafe {
            let mut path = (*state.state).plist;

            // For setting the path color
            // and the fill command
            if !path.is_null() {
                accum += 2;
            }

            while !path.is_null() {
                let curve = (*path).curve;
                // Assumes mostly curved paths, 5% are starights
                // We need 2 for Operation for starting and closing the path.
                accum += (curve.n as f32 * 1.05) as usize + 2;

                path = (*path).next;
            }
        }
    }

    accum
}

/// Generates the [Operation]s for the given curve and pushes them to `operations`.
unsafe fn process_curve(curve: &potrace_curve_s, operations: &mut Vec<Operation>) {
    const Y: f64 = f_fmt::PAGE_HEIGHT as f64;

    if curve.n == 0 {
        return;
    }

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
                operations.push(Operation::new("c", vec![
                    c1.x.into(), (Y - c1.y).into(),
                    c2.x.into(), (Y - c2.y).into(),
                    c3.x.into(), (Y - c3.y).into()
                ]));
            }
            _ => {}
        }
    }

    // Close the curve ("subpath" in PDF terms)
    operations.push(Operation::new("h", vec![]));
}
