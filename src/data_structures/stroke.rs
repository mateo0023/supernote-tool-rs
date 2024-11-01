//! This module contains all the data structures relevant to
//! the storage and (possibly) transcription of stroke paths.
//! 
//! See the file `/examples/TotalPath Notes.pdf` for my notes

use std::error::Error;

use serde::Serialize;

mod my_script;

pub use my_script::{ServerConfig, transcribe, TransciptionError};

use crate::common::f_fmt;

/// The pressure force of a point.
type Force = u16;
/// The maximum force applied
const MAX_FORCE: f64 = 0xFFF as f64;

/// How much to scale pixels to points.
/// 
/// `point = pixel * SCALE_FACTOR`
const SCALE_FACTOR: f64 = 11.2;
/// The maximum width possible in the path
/// representation. This is the regular
/// [width](f_fmt::PAGE_WIDTH)
const MAX_WIDTH: f64 = f_fmt::PAGE_WIDTH as f64 * SCALE_FACTOR;
/// The size of the length values in bytes.
const LEN_SIZE: usize = std::mem::size_of::<u32>();

#[derive(Debug)]
pub enum StrokeError {
    /// The `&[u8]` is too short to extract data.
    /// **OR** too short for the given `path_len`
    TooShort,
    /// The [Stroke] is missing a length property
    /// for the `&str`
    MissingLength(&'static str),
    /// When the length properties of the [Stroke]
    /// don't match up.
    UnmatchedLen,
    /// It's when the point cannot process the 
    /// value stored in `&str`.
    IncorrectPoint(&'static str),
}

num_enum!{ Color <u32> {
    Black     = 0,
    DarkGray  = 0x9D,
    LightGray = 0xCA,
    White     = 0xFE,
}}

num_enum!{PenType <u32> {
    InkPen      = 0x1,
    NeedlePoint = 0xA,
    Marker      = 0xB,
}}

/// Is a single stroke. Made for transforming into Text with the
/// [MyScript](https://developer.myscript.com) library.
/// 
/// Each point is spread across 4 vectors:
/// * X value (0.01 mm = 1 unit)
/// * Y value (0.01 mm = 1 unit)
/// * Force (max = `0xFFF`)
/// * Time Delta (nano seconds)
#[derive(Debug, Clone, Serialize, std::cmp::PartialEq)]
pub struct Stroke {
    /// The x coordinate of the point.
    /// 0 is right, and the units are
    /// 100 points per `mm`
    /// (~11.2 points/pixel)
    x: Vec<u32>,
    /// The y coordinate of the point.
    /// 0 is above, and the units are
    /// 100 points per `mm`
    /// (~11.2 points/pixel)
    y: Vec<u32>,
    /// The force value applied as a f64
    /// (max being 1.0).
    force: Vec<f64>,
    /// The delta-time of the stroke in milliseconds
    time: Vec<u32>,
    /// The coordinates of the stroke:
    /// `[min_x, min_y, max_x, max_y]`
    #[serde(skip_serializing)]
    coord: [u32; 4],
    /// The stroke color
    #[serde(skip_serializing)]
    color: Color,
    /// The type of stool
    #[serde(skip_serializing)]
    tool: PenType,
    /// The thikness of the line.
    #[serde(skip_serializing)]
    line_thikness: u32,
}

/// Extracts the first 4 bytes and turns them into a [u32].
///
/// # Returns
/// * The [u32] and the array starting right after the [u32].
///   So, `&data[4..]`
/// * Will return [Err] if there are not enough bytes to cast.
#[inline]
fn get_u32(data: &[u8]) -> Result<(u32, &[u8]), ()> {
    if data.len() < LEN_SIZE {
        return Err(());
    }

    let num = u32::from_le_bytes([
        data[0],
        data[1],
        data[2],
        data[3],
    ]);

    Ok((num, &data[LEN_SIZE..]))
}

/// Is the same as [get_u32()] but casts the [u32] into a [usize].
#[inline]
fn get_len(data: &[u8]) -> Result<(usize, &[u8]), ()> {
    let (n, d) = get_u32(data)?;
    Ok((n as usize, d))
}

impl Error for StrokeError {}

impl std::fmt::Display for StrokeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StrokeError::TooShort => write!(f, "Data Stream was TooShort"),
            StrokeError::MissingLength(msg) => write!(f, "Data stream was incorrect format: {}", msg),
            StrokeError::UnmatchedLen => write!(f, "Unmatched Lengths between segments"),
            StrokeError::IncorrectPoint(var) => write!(f, "Unexpected error when parsing a stroke point, missing {}", var),
        }
    }
}

impl Stroke {
    /// Creates a [Stroke] from the given memory slice.
    /// # Returns
    /// ([Stroke], `remaining_bits`).
    fn from_slice(data: &[u8]) -> Result<(Option<Self>, &[u8]), StrokeError> {
        let (total_path_len, data) = get_len(data).map_err(|_| StrokeError::TooShort)?;
        let final_ref = &data[total_path_len..];
        if data.len() < total_path_len {
            return Err(StrokeError::TooShort);
        }
        
        // * Tool Code
        let (tool_code, data) = get_u32(data).map_err(|_| StrokeError::TooShort)?;
        let tool = match PenType::try_from(tool_code) {
            Ok(c) => c,
            Err(_) => return Ok((None, final_ref)),
        };
        // * Color Code
        let (color, data) = get_u32(data).map_err(|_| StrokeError::TooShort)?;
        let color = match color.try_into() {
            Ok(c) => c,
            Err(_) => return Ok((None, final_ref)),
        };
        // * Line Thinkness
        let (line_thikness, data) = get_u32(data).map_err(|_| StrokeError::TooShort)?;

        // Remove the 196 unkown bytes:
        let data = &data[196..];

        // The count of the 24-byte structures.
        const STRUCTURE_SIZE: usize = 24;
        let (structure_count, data) = get_len(data).map_err(|_| StrokeError::MissingLength("Missing 24-byte Structure Length"))?;
        let data = &data[structure_count * STRUCTURE_SIZE..];
        
        // It's 4 (u32) * 2 = 8.
        const PTS_SIZE: usize = 8;
        let (y_x_ct, y_x_pts) = get_len(data).map_err(|_| StrokeError::MissingLength("(Y, X)"))?;
        let data = &y_x_pts[PTS_SIZE * y_x_ct..];

        /// It's the number of u16 (Force)
        const FRC_SIZE: usize = std::mem::size_of::<Force>();
        let (force_ct, force_ms) = get_len(data).map_err(|_| StrokeError::MissingLength("Force"))?;
        if force_ct != y_x_ct { return Err(StrokeError::UnmatchedLen) }
        let data = &force_ms[force_ct * FRC_SIZE..];

        const TIME_SIZE: usize = std::mem::size_of::<u32>();
        let (time_ct, deltas) = get_len(data).map_err(|_| StrokeError::MissingLength("Time Deltas"))?;
        if time_ct != y_x_ct { return Err(StrokeError::UnmatchedLen) }

        let (mut min_x, mut min_y, mut max_x, mut max_y) = (u32::MAX, u32::MAX, u32::MIN, u32::MIN);
        let mut x_vals = Vec::with_capacity(y_x_ct);
        let mut y_vals = Vec::with_capacity(y_x_ct);
        let mut forces = Vec::with_capacity(y_x_ct);
        let mut time_deltas = Vec::with_capacity(y_x_ct);
        // We've made sure we had enough
        for idx in 0..y_x_ct {
            let (y, x_st) = get_u32(&y_x_pts[idx * PTS_SIZE..]).map_err(|_| StrokeError::IncorrectPoint("y"))?;
            let (x, _) = get_u32(x_st).map_err(|_| StrokeError::IncorrectPoint("x"))?;
            // The force as floating point.
            let force = u16::from_le_bytes([
                force_ms[idx * FRC_SIZE],
                force_ms[idx * FRC_SIZE + 1],
            ]) as f64 / MAX_FORCE;
            // Time in nanoseconds
            let (time, _) = get_u32(&deltas[idx * TIME_SIZE..]).map_err(|_| StrokeError::IncorrectPoint("time_delta"))?;
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            x_vals.push((MAX_WIDTH.ceil() as u32).saturating_sub(x));
            y_vals.push(y);
            forces.push(force);
            // Change time from 10^-9 to 10^-3 (10^6)
            time_deltas.push(time / 1_000_000);
        }

        Ok((Some(Self {
            x: x_vals,
            y: y_vals,
            force: forces,
            time: time_deltas,
            coord: [
                ((MAX_WIDTH - max_x as f64) / SCALE_FACTOR) as u32,
                (min_y as f64 / SCALE_FACTOR) as u32,
                ((MAX_WIDTH - min_x as f64) / SCALE_FACTOR) as u32,
                (max_y as f64 / SCALE_FACTOR) as u32,
            ],
            color,
            tool,
            line_thikness,
        }), final_ref))
    }

    pub fn process_page(data: &[u8]) -> Result<Vec<Self>, StrokeError> {
        let (path_count, mut data) = get_len(&data).map_err(|_| StrokeError::TooShort)?;
        let mut paths = Vec::with_capacity(path_count);

        while !data.is_empty() {
            let (stroke, next) = Stroke::from_slice(data)?;
            if let Some(stroke) = stroke {
                paths.push(stroke);
            }
            data = next;
        }

        Ok(paths)
    }

    /// Returns `true` if the given stroke is fully contained within the
    /// given points `[x_min, y_min, x_max, y_max]`.
    pub fn contained(&self, rect: [u32; 4]) -> bool {
        // x_min
        self.coord[0] >= rect[0]
        // y_min
        && self.coord[1] >= rect[1]
        // x_max
        && self.coord[2] <= rect[2]
        // y_max
        && self.coord[3] <= rect[3]
    }
}

/// Will clone the storkes that are not markers and are fully contained 
/// within `rect`, defined by corners.
pub fn clone_strokes_contained(strokes: &[Stroke], rect: [u32; 4]) -> Vec<Stroke> {
    strokes.iter()
    // Have only non-markers fully inside rect.
    .filter(|stroke| stroke.tool != PenType::Marker && stroke.contained(rect))
            .map(Stroke::clone).collect()
}
