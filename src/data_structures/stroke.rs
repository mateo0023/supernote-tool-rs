//! This module contains all the data structures relevant to
//! the storage and (possibly) transcription of stroke paths.

use serde::Serialize;

type Force = u16;

const MAX_FORCE: Force = 0xFFF;
const LEN_SIZE: usize = std::mem::size_of::<u32>();

#[derive(Debug)]
pub enum Error {
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

// pub enum Color {
//     Black = 0,
//     DarkGray = 0x9D,
//     LightGray = 0xCA,
//     White = 0xFE,
// }

// pub enum PenType {
//     InkPen      = 0x1,
//     NeedlePoint = 0xA,
//     Marker      = 0xB,
// }

#[derive(Debug, Clone, Serialize, std::cmp::Eq, std::cmp::PartialEq)]
pub struct Stroke {
    stroke: Vec<Point>,
    // color: Color,
    // pen: PenType,
    // width: u32,
}

/// This represents a single point in the
/// storke.
#[derive(Debug, Clone, Serialize, std::cmp::Eq, std::cmp::PartialEq)]
pub struct Point {
    /// The x coordinate of the point.
    /// 0 is right, and
    /// there are 12 points per pixel.
    x: u32,
    /// The y coordinate of the point.
    /// 0 is above, and
    /// there are 12 points per pixel.
    y: u32,
    /// The force value applied as a u16.
    /// The maximum value is `0xFFF`.
    force: Force,
    /// The delta-time of the stroke in milliseconds
    time: u32,
}

/// Extracts the first 4 bytes and turns them into a [u32].
///
/// # Returns
/// * The [u32] and the array starting right after the [u32].
/// So, `&data[4..]`
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

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::TooShort => write!(f, "Data Stream was TooShort"),
            Error::MissingLength(msg) => write!(f, "Data stream was incorrect format: {}", msg),
            Error::UnmatchedLen => write!(f, "Unmatched Lengths between segments"),
            Error::IncorrectPoint(var) => write!(f, "Unexpected error when parsing a stroke point, missing {}", var),
        }
    }
}

impl Stroke {
    /// Creates a [Stroke] from the given memory slice.
    /// # Returns
    /// ([Stroke], `remaining_bits`).
    fn from_slice(data: &[u8]) -> Result<(Self, &[u8]), Error> {
        let (total_path_len, data) = get_len(data).map_err(|_| Error::TooShort)?;
        if data.len() != total_path_len {
            return Err(Error::TooShort);
        }
        let mut stroke = Vec::with_capacity(total_path_len);
        // skip the first 3 numbers:
        // * Tool Code
        // * Color Code
        // * Line Thinkness
        let data = &data[3 * LEN_SIZE..];

        // Remove the 196 unkown bytes:
        let data = &data[196..];

        // The count of the 24-byte structures.
        const STRUCTURE_SIZE: usize = 24;
        let (structure_count, data) = get_len(data).map_err(|_| Error::MissingLength("Missing 24-byte Structure Length"))?;
        let data = &data[structure_count * STRUCTURE_SIZE..];
        
        // It's 4 (u32) * 2 = 8.
        const PTS_SIZE: usize = 8;
        let (y_x_ct, y_x_pts) = get_len(data).map_err(|_| Error::MissingLength("(Y, X)"))?;
        let data = &data[PTS_SIZE * y_x_ct..];

        /// It's the number of u16 (Force)
        const FRC_SIZE: usize = std::mem::size_of::<Force>();
        let (force_ct, force_ms) = get_len(data).map_err(|_| Error::MissingLength("Force"))?;
        if force_ct != y_x_ct { return Err(Error::UnmatchedLen) }
        let data = &data[force_ct * FRC_SIZE..];

        const TIME_SIZE: usize = std::mem::size_of::<u32>();
        let (time_ct, deltas) = get_len(data).map_err(|_| Error::MissingLength("Time Deltas"))?;
        if time_ct != y_x_ct { return Err(Error::UnmatchedLen) }

        // We've made sure we had enough
        for idx in 0..y_x_ct {
            stroke.push(Point::new(
                &y_x_pts[idx * PTS_SIZE..],
                &force_ms[idx * FRC_SIZE..],
                &deltas[idx * TIME_SIZE..]
            ).map_err(|_| Error::TooShort)?);
        }

        Ok((Self {
            stroke,
        }, &data[total_path_len..]))
    }

    pub fn process_page(data: Vec<u8>) -> Result<Vec<Self>, Error> {
        let (path_count, mut data) = get_len(&data).map_err(|_| Error::TooShort)?;
        let mut paths = Vec::with_capacity(path_count);

        while !data.is_empty() {
            let (stroke, next) = Stroke::from_slice(data)?;
            paths.push(stroke);
            data = next;
        }

        Ok(paths)
    }

    /// Returns `true` if the given stroke is fully contained within the
    /// given points `[x_1, y_1, x_2, y_2]`.
    pub fn contained(&self, rect: [u32; 4]) -> bool {
        let rect = [
            rect[0].min(rect[2]) * 12,
            rect[1].min(rect[3]) * 12,
            rect[0].max(rect[2]) * 12,
            rect[1].max(rect[3]) * 12,
        ];

        for pt in self.stroke.iter() {
            if !pt.contained(rect) {
                return false;
            }
        }
        true
    }
}

impl Point {
    /// Will parse into [Point] given the corresponding slice starting points.
    fn new(y_x: &[u8], force: &[u8], time: &[u8]) -> Result<Self, Error> {
        let (y, x_st) = get_u32(y_x).map_err(|_| Error::IncorrectPoint("y"))?;
        let (x, _) = get_u32(x_st).map_err(|_| Error::IncorrectPoint("x"))?;
        let force = u16::from_le_bytes([
            force[0],
            force[1],
        ]);
        // Time in nanoseconds
        let (time, _) = get_u32(time).map_err(|_| Error::IncorrectPoint("time_delta"))?;
        Ok(Self {
            y,
            x,
            force,
            // From nano to milliseconds
            // From 10^-9 to 10^-3
            // (delta = 10^-6)
            time: time / 1_000_000,
        })
    }

    /// Returns `true` if the point is within `[x_min, y_min, x_max, y_max]`
    #[inline]
    fn contained(&self, rect: [u32; 4]) -> bool {
        self.x >= rect[0] && self.x <= rect[2]
        && self.y >= rect[1] && self.y <= rect[3]
    }
}