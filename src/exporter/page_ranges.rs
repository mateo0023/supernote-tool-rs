//! This module should contain the needed structs and functions to build a struct used by the
//! export functions to determine which pages to ignore

/// Builds a Mapping between old page indexes and new (when exported)
/// indexes.
/// 
/// May have unexpected behaviour when building ranges out of order.
pub struct RangeBuilder {
    /// The total length of pages.
    len: usize,
    /// Included page ranges.
    /// 
    /// Ranges are non-inclusive, so `(0, 4)` is the
    /// indices `0, 1, 2, 3`.
    ranges: Vec<(usize, usize)>,
    /// The first page of the index to include (if any).
    active_idx: Option<usize>,
}

/// Contains a vector of [PageMap]s and the total count
pub struct MultiNotePageMap {
    notes: Vec<PageMap>,
    total_len: usize,
}

/// Is a vector that maps and index to the new page index.
/// Also contains the new length of the document.
#[derive(Clone)]
pub struct PageMap {
    pages: Vec<Option<usize>>,
    new_len: usize,
}

impl RangeBuilder {
    pub fn new(len: usize) -> RangeBuilder {
        RangeBuilder {
            len,
            ranges: vec![],
            active_idx: None,
        }
    }

    /// Open an included range at `idx`
    pub fn start_included(&mut self, idx: usize) {
        if let Some(st_idx) = self.active_idx.as_mut() {
            *st_idx = (*st_idx).min(idx);
        } else {
            self.active_idx = Some(idx);
        }
    }

    /// Open an exluded range at `idx`, closing the open range if
    /// needed.
    pub fn start_excluded(&mut self, idx: usize) {
        if let Some(st_id) = self.active_idx.take() {
            self.ranges.push((st_id, idx));
        }
    }

    /// Close an open range, including the index.
    pub fn close_and_include(&mut self, idx: usize) {
        self.start_excluded(idx+1);
    }

    /// Builds [self] into the page number mapping.
    pub fn build(mut self) -> PageMap {
        let mut page_map = vec![None; self.len];
        if let Some(idx) = self.active_idx {
            self.ranges.push((idx, self.len));
        }

        let mut indexes = self.ranges.into_iter()
            .flat_map(|(st, en)| st..en)
            .collect::<Vec<_>>();
        indexes.sort();

        let mut counter = 0;
        for idx in indexes {
            // Ensure we ignore repeated indexes.
            if page_map[idx].is_none() {
                page_map[idx] = Some(counter);
                counter += 1;
            }
        }
        PageMap {
            pages: page_map,
            new_len: counter,
        }
    }
}

impl MultiNotePageMap {
    pub const fn new() -> Self {
        Self { notes: vec![], total_len: 0 }
    }

    /// Build a [MultiNotePageMap] from a list of individual 
    /// [PageMap]s (will shift internal indexes for export).
    pub fn from_vec(mut notes: Vec<PageMap>) -> Self {
        let mut total_len = 0;

        for map in notes.iter_mut() {
            for idx in map.pages.iter_mut().flatten() {
                *idx += total_len;
            }
            total_len += map.get_len();
        }

        Self {
            notes,
            total_len,
        }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, PageMap> {
        self.notes.iter()
    }

    pub fn push(&mut self, mut value: PageMap) {
        for idx in value.pages.iter_mut().flatten() {
            *idx += self.total_len;
        }
        self.total_len += value.new_len;
        self.notes.push(value);
    }
}

impl Default for MultiNotePageMap {
    fn default() -> Self {
        Self::new()
    }
}

impl PageMap {
    /// Creates a new empty [PageMap].
    pub const fn new_empty() -> Self {
        Self { pages: vec![], new_len: 0 }
    }

    pub fn new_full(len: usize) -> Self {
        Self {
            pages: (0..len).map(Some).collect(),
            new_len: len,
        }
    }

    pub const fn is_empty(&self) -> bool {
        self.new_len == 0
    }

    /// Gets the new length of the
    /// [Notebook](crate::data_structures::Notebook)
    /// after removing ignored pages.
    pub const fn get_len(&self) -> usize {
        self.new_len
    }

    /// Returns the new index (if the page is included).
    pub fn get_new_idx(&self, idx: usize) -> Option<usize> {
        self.pages.get(idx)
            .and_then(|op| *op)
    }
}
