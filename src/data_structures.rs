use std::collections::HashMap;
use std::error::Error;
use std::fs::File;

use super::io::extract_key_and_read;

pub mod metadata;
pub mod stroke;

use stroke::Stroke;

pub mod file_format_consts {
    pub const PAGE_HEIGHT: usize = 1872;
    pub const PAGE_WIDTH: usize = 1404;
}

use metadata::Metadata;
use serde::Serialize;

#[derive(Debug)]
pub enum DataStructureError {
    MissingField{t: StructType, k: String},
    RectFailure,
}

#[derive(Debug, Clone, Copy)]
pub enum StructType {
    Title,
    Link,
}

/// Will contain all the necessary information from the Notebook
/// 
/// # ToDo!
/// * Keyword
#[derive(Debug, Serialize)]
pub struct Notebook {
    /// The file name (not including the extension)
    pub file_name: String,
    /// The ID used to identify the file, see [Metadata::file_id]
    pub file_id: String,
    /// A list containing all the [Titles](Title)
    /// 
    /// Titles will be sorted by Page and then Position
    /// to facilitate Bookmark Generation
    pub titles: HashMap<u64, Title>,
    /// A list containing all the [Links](Link)
    pub links: Vec<Link>,
    /// A list containing all the [Pages](Page)
    /// 
    /// Pages are sorted
    pub pages: Vec<Page>,
    /// Map between PAGE_ID and page indexes.
    pub page_id_map: HashMap<String, usize>,
    /// The notebook's starting page.
    /// 
    /// Used when chaining multiple [Notebook]s
    /// into a single PDF.
    pub starting_page: usize,
}

#[derive(Debug, Serialize, Default)]
pub struct Title {
    /// The encoded content of the Title.
    /// 
    /// To be decoded into a Bitmap
    pub content: Option<Vec<u8>>,
    /// The hash of [Self::content], if any.
    /// Otherwise it will be a hash of the:
    /// 1. `page_id`, and
    /// 2. [Self::title_level]
    pub hash: u64,
    /// Essentially the type of title
    /// 
    /// [TitleLevel] will later be used to determine
    /// how to order the ToC in the PDF.
    /// Smaller titles closer to root.
    pub title_level: TitleLevel,
    /// The page_index in the `.note` file.
    /// Needs to be shifted when exporting
    pub page_index: usize,
    /// The vertical position on the page.
    /// Same as [`coords[1]`](Self::coords)
    pub position: u32,
    /// The rectangle defined by
    /// `[x_min, y_min, x_max, y_max]`
    pub coords: [u32; 4],
    /// The actual pen [strokes](Stroke) that make up the
    /// [Title].
    strokes: Vec<Stroke>,
    pub width: usize,
    pub height: usize,
    pub name: Option<String>,
}
#[derive(Debug, Serialize)]
pub struct Link {
    pub start_page: usize,
    pub link_type: LinkType,
    pub coords: [u32; 4],
}

#[derive(Debug, Serialize)]
pub struct Page {
    pub totalpath: Vec<Stroke>,
    // pub recogn_file: Option<Vec<u8>>,
    // pub recogn_text: Option<Vec<u8>>,
    pub layers: Vec<Layer>,
    pub page_num: usize,
    pub page_id: String,
}

#[derive(Debug, Serialize)]
pub struct Layer {
    pub is_background: bool,
    pub content: Option<Vec<u8>>,
}

#[derive(Debug, Serialize)]
pub enum LinkType {
    /// A link to the same file, containing the page index
    SameFile{page_id: String},
    /// A link to the same file, containing:
    /// * Page Index
    /// * The other's file_id
    OtherFile{page_id: String, file_id: String},
    /// A link to a website, contains the link.
    WebLink{link: String},
}

#[derive(Debug, Clone, Copy, Serialize, Default, Hash, std::cmp::PartialEq, std::cmp::Eq, std::cmp::PartialOrd, std::cmp::Ord)]
#[repr(u8)]
pub enum TitleLevel {
    FileLevel,
    #[default]
    BlackBack,
    LightGray,
    DarkGray,
    Stripped,
}

/// Process a rectangle in the form `[x, y, width, height]`
/// to the rectangle: `[x_min, y_min, x_max, y_max]`
fn process_rect_to_corners(rect: Vec<u32>) -> Result<[u32; 4], DataStructureError> {
    if let [x1, y1, w, h, ..] = rect[..] {
        Ok([
            x1, y1, x1 + w, y1 + h
        ])
    } else {
        Err(DataStructureError::RectFailure)
    }
}

/// Will hash the string using [DefaultHasher](std::hash::DefaultHasher).
fn hash(content: &[u8]) -> u64 {
    use std::hash::{DefaultHasher, Hasher as _};

    let mut hasher = DefaultHasher::new();
    hasher.write(content);
    hasher.finish()
}

// ###########################################################################################################
// ###########################################################################################################
// ###########################################################################################################
// ############################################# IMPLEMENTATIONS #############################################
// ###########################################################################################################
// ###########################################################################################################
// ###########################################################################################################

impl Notebook {
    /// Update the title's name field given its hash value.
    /// 
    /// Will set it to none if empty.
    pub fn update_title(&mut self, title_hash: u64, new_title: Option<&str>) {
        if let Some(title) = self.titles.get_mut(&title_hash) {
            if let Some(new) = new_title {
                title.name = Some(new.to_string());
            }
        }
    }

    pub fn get_sorted_titles(&self) -> Vec<&Title> {
        let mut titles: Vec<&Title> = self.titles.values().collect();
        titles.sort();
        titles
    }

    /// Gets the page_id corresponding to the page at internal `index`
    /// 
    /// *NOT SHIFTED* by [starting_page](Self::starting_page)
    /// 
    /// # Return
    /// * `Some(String)` with the [id](Page::page_id)
    /// * `None` if the index is out of bounds.
    pub fn get_page_id_from_internal(&self, index: usize) -> Option<String> {
        self.pages.get(index).map(|page| page.page_id.clone())
    }

    /// Will get the PDF page number given the `page_id` and the internal
    /// [starting_page](Self::starting_page).
    pub fn get_page_index_from_id(&self, page_id: &str) -> Option<usize> {
        self.page_id_map.get(page_id).copied().map(|idx| idx + self.starting_page)
    }
}

impl Title {
    /// Create a new [Title] that will be used to indicate a file.
    pub fn new_for_file(name: &str, index: usize) -> Self {
        Title {
            title_level: TitleLevel::FileLevel,
            page_index: index,
            name: Some(name.to_string()),
            ..Default::default()
        }
    }

    /// Creates a new *ghost* title.
    /// 
    /// These are the titles are the are missing in the tree structure.
    pub fn new_ghost(title_level: TitleLevel, reference_t: &Title, page_id: &str) -> Self {
        let hash = {
            use std::hash::{DefaultHasher, Hasher as _};
    
            let mut hasher = DefaultHasher::new();
            hasher.write(page_id.as_bytes());
            hasher.write(&[title_level as u8]);
            hasher.finish()
        };

        Self {
            hash,
            title_level,
            page_index: reference_t.page_index,
            position: reference_t.position,
            coords: reference_t.coords,
            ..Default::default()
        }
    }

    /// Used to exporting into a ToC. Will create a
    /// [Title] with default values for all except:
    /// * [name](Self::name), will be the same (clone)
    /// * [page_index](Self::page_index), which will be shifted by `shift`
    /// * [title_level](Self::title_level), will be the same (copy)
    pub fn basic_for_toc(&self, shift: usize) -> Self {
        Title {
            name: self.name.clone(),
            page_index: self.page_index + shift,
            title_level: self.title_level,
            ..Default::default()
        }
    }

    /// It loops over the titles in [Metadata::footer::titles](metadata::Footer::titles) and maps it to a [Title] by calling [Title::from_meta].
    /// 
    /// # Returns
    /// Will return an empty vector if [Metadata::footer::titles](metadata::Footer::titles) is [None], otherwise, it will return the mapped values 
    /// as specified above.
    /// 
    /// # Panics
    /// It may panic when calling [Title::from_meta]
    pub fn get_vec_from_meta(metadata: &Metadata, file: &mut File, pages: &[Page]) -> Result<Vec<Title>, Box<dyn Error>> {
        match &metadata.footer.titles {
            Some(v) => v.iter().map(|metadata| Title::from_meta(metadata, file, pages)).collect(),
            None => Ok(vec![]),
        }
    }

    /// Will create a [Title] from its [MetaMap]. Will clone `metadata` and read content from the [file](File)
    /// 
    /// # Panics
    /// It will panic if the [MetaMap](metadata::MetaMap) doesn't contain the entry `"TITLERECTORI"` consisting of a list with one string.
    /// That string being a comma-separated list of at least 2 integers.
    /// ```json
    /// // ...
    /// "TITLERECTORI": [
    ///     "41,149,752,78"
    /// ],
    /// // ...
    /// ```
    /// 
    /// # Returns
    /// ```rust
    /// Title {
    ///     metadata: MetaMap,
    ///     content: Vec<u8>,
    ///     page_number: None,
    ///     position: u32,
    /// }
    /// ```
    pub fn from_meta(metadata: &metadata::MetaMap, file: &mut File, pages: &[Page]) -> Result<Title, Box<dyn Error>> {
        // Very long chain with possible errors. But it should be fine as long as the file is properly formatted
        let page_pos = metadata.get("TITLERECTORI")
            .ok_or(Box::new(DataStructureError::MissingField { t: StructType::Title, k: "TITLERECTORI".to_string() }))?[0]
            .split(',').nth(1).unwrap().parse()?;
        let page_index = metadata.get("PAGE_NUMBER")
            .ok_or(DataStructureError::MissingField { t: StructType::Title, k: "PAGE_NUMBER".to_string() })?[0]
            .parse::<usize>()? - 1;

        let coords: Vec<u32> = {
            let mut c = vec![];
            let it = metadata.get("TITLERECT")
                .ok_or(DataStructureError::MissingField { t: StructType::Title, k: "TITLERECT".to_string() })?[0]
                .split(',');
            for p in it {
                c.push(p.parse()?);
            }
            c
        };
        let width = coords[2] as usize;
        let height = coords[3] as usize;
        let coords = process_rect_to_corners(coords)?;

        let title_level = TitleLevel::from_meta(metadata);
        
        let content = extract_key_and_read(file, metadata, "TITLEBITMAP")
            .ok_or(DataStructureError::MissingField { t: StructType::Title, k: "TITLEBITMAP".to_string() })?;
        let hash = hash(&content);

        let strokes = pages[page_index].clone_strokes_contained(coords);

        Ok(Title {
            content: Some(content),
            hash,
            page_index,
            position: page_pos,
            title_level,
            coords,
            width,
            height,
            name: None,
            strokes,
        })
    }

    /// Returns the title's name (text contained in there).
    /// 
    /// Will default to an empty string.
    pub fn get_name(&self) -> String {
        self.name.clone().unwrap_or_default()
    }
}

impl std::cmp::PartialEq for Title {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash
    }
}

impl std::cmp::Eq for Title {}

impl std::cmp::Ord for Title {
    /// Compare 2 [Title]s in the following order (going down if equal)
    /// 1. [page_index](Self::page_index)
    /// 2. [position](Self::position)
    /// 3. [title_level](Self::title_level)
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering::Equal;
        match self.page_index.cmp(&other.page_index) {
            Equal => match self.position.cmp(&other.position) {
                Equal => self.title_level.cmp(&other.title_level),
                order => order,
            },
            order => order,
        }
    }
}

impl std::cmp::PartialOrd for Title {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Link {
    pub fn get_vec_from_meta(metadata: &Metadata) -> Vec<Link> {
        match &metadata.footer.links {
            Some(links) => links.iter().zip(Link::extract_page_numbers_from_meta(metadata).iter())
                .filter_map(|(link_meta, &page_num)| Link::new(link_meta, page_num, &metadata.file_id).unwrap_or_default()).collect(),
            None => vec![],
        }
    }

    fn new(link_meta: &metadata::MetaMap, page_num: usize, file_id: &str) -> Result<Option<Self>, Box<dyn Error>> {
        if Link::is_incoming(link_meta)? {
            return Ok(None);
        }
        Ok(Some(Link {
            start_page: page_num,
            link_type: LinkType::from_meta(link_meta, file_id),
            coords: Self::get_link_rect(link_meta)?,
        }))
    }

    /// Wether the link is incoming (receiving) or linking **to** something
    fn is_incoming(link_meta: &metadata::MetaMap) -> Result<bool, Box<dyn Error>> {
        Ok(link_meta.get("LINKINOUT")
            .ok_or(DataStructureError::MissingField { t: StructType::Link, k: "LINKINOUT".to_string() })?[0] == "1")
    }

    fn extract_page_numbers_from_meta(metadata: &Metadata) -> Vec<usize> {
        metadata.footer.main.keys()
            // Look only at those that start with "LINK" ie "LINKO_00020803014801651111"
            .filter(|key| key.starts_with("LINK"))
            // Get only the indices 6 through 9
            // LINKO_00020803014801651111  =>  0002
            .filter_map(|k| k.get(6..10))
            // Parse that number into a `usize`
            // Also parse the address (value) of where the metadata is located.
            .filter_map(|k| 
                k.parse::<usize>().ok().map(|page| page-1)
            )
            .collect()
    }

    /// Extracts the link's rectangle (where it's located, not where it points).
    fn get_link_rect(link_meta: &metadata::MetaMap) -> Result<[u32; 4], Box<dyn Error>> {
        let mut poitns = vec![];
        let it = link_meta.get("LINKRECT")
            .ok_or(DataStructureError::MissingField { t: StructType::Link, k: "LINKRECT".to_string() })?[0].split(',');
        for p in it {
            poitns.push(p.parse()?);
        }
        Ok(process_rect_to_corners(poitns)?)
    }
}

impl Page {
    /// Given al vector of [page metadata](metadata::PageMeta) it will return a vector of [pages](Page).
    pub fn get_vec_from_meta(metadata: &[metadata::PageMeta], file: &mut File) -> Vec<Page> {
        metadata.iter().map(|meta| Page::from_meta(meta, file)).collect()
    }

    /// Given a [PageMeta](metadata::PageMeta) it returns a [Page].
    pub fn from_meta(metadata: &metadata::PageMeta, file: &mut File) -> Self {
        let paths = extract_key_and_read(file, &metadata.page_info, "TOTALPATH").unwrap();
        Page {
            totalpath: stroke::Stroke::process_page(paths).expect("Failed to process the strokes in page"),
            // recogn_file: extract_key_and_read(file, &metadata.page_info, "RECOGNFILE"),
            // recogn_text: extract_key_and_read(file, &metadata.page_info, "RECOGNTEXT"),
            layers: Layer::get_vec_fom_vec(&metadata.layers, file),
            page_num: metadata.page_info.get("PAGE_NUMBER").unwrap()[0].parse().unwrap(),
            page_id: metadata.page_info.get("PAGEID").unwrap()[0].clone(),
        }
    }

    /// Clone the [strokes](Stroke) fully contained in the rectangle defined by
    /// `[x, y, width, height]`. Should be the same as [Title::coords].
    fn clone_strokes_contained(&self, coords: [u32; 4]) -> Vec<Stroke>{
        stroke::clone_strokes_contained(&self.totalpath, coords)
    }
}

impl Layer {
    /// Given a vector of layer [metadata](metadata::MetaMap), it retrns a vector of [Layer].
    pub fn get_vec_fom_vec(layers: &[metadata::MetaMap], file: &mut File) -> Vec<Self> {
        layers.iter().map(|meta| Layer::from_meta(meta, file)).collect()
    }

    /// Creates a layer purely by cloning [meta](metadata::MetaMap) and reading the [contents](Layer::content) with [extract_key_and_read].
    pub fn from_meta(meta: &metadata::MetaMap, file: &mut File) -> Self {
        Layer {
            is_background: meta.get("LAYERNAME").map(|n| n[0].eq("BGLAYER")).unwrap_or(false),
            content: extract_key_and_read(file, meta, "LAYERBITMAP"),
        }
    }

    pub fn is_background(&self) -> bool {
        self.is_background
    }
}

impl LinkType {
    const KEY_STYLE: &'static str = "LINKTYPE";
    const KEY_FILE_ID: &'static str = "LINKFILEID";
    const TO_PAGE: &'static str = "0";
    const TO_WEB: &'static str = "4";
    
    pub fn from_meta(link_meta: &metadata::MetaMap, file_id: &str) -> Self {
        let link_style = link_meta.get(Self::KEY_STYLE).unwrap()[0].as_str();
        // Link to website
        if link_style.eq(Self::TO_WEB) {
            return LinkType::WebLink { link: link_meta.get("LINKFILE").unwrap()[0].clone() };
        }
        // Is internal/external
        if link_style.eq(Self::TO_PAGE) {
            let page_id = link_meta.get("PAGEID").unwrap()[0].clone();
            let to_file_id = link_meta.get(Self::KEY_FILE_ID).unwrap()[0].as_str();

            match to_file_id.eq(file_id) {
                true => LinkType::SameFile { page_id },
                false => LinkType::OtherFile { page_id, file_id: to_file_id.to_string() },
            }
        } else {
            todo!("Not implemented linking to files (without page info)")
        }
    }
}

impl TitleLevel {
    /// Looks at the `"TITLESTYLE"` and returns the appropiate
    /// Type.
    /// 
    /// Returns the default value if no style is identified.
    pub fn from_meta(title_meta: &metadata::MetaMap) -> Self {
        let style = title_meta.get("TITLESTYLE").unwrap()[0].clone();
        if style.eq("1000254") {
            Self::BlackBack
        } else if style.eq("1201000") {
            Self::LightGray
        } else if style.eq("1157254") {
            Self::DarkGray
        } else if style.eq("1000000") {
            Self::Stripped
        } else {
            Self::default()
        }
    }

    pub fn add(&self) -> Self {
        use TitleLevel::*;
        match self {
            FileLevel => BlackBack,
            BlackBack => LightGray,
            LightGray => DarkGray,
            DarkGray => Stripped,
            Stripped => Stripped,
        }
    }
}

impl std::fmt::Display for TitleLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                TitleLevel::FileLevel => "File",
                TitleLevel::BlackBack => "BlackBack",
                TitleLevel::LightGray => "LightGray",
                TitleLevel::DarkGray => "DarkGray",
                TitleLevel::Stripped => "Stripped",
            }
        )
    }
}

impl From<TitleLevel> for i32 {
    fn from(value: TitleLevel) -> Self {
        match value {
            TitleLevel::FileLevel => 0,
            TitleLevel::BlackBack => 1,
            TitleLevel::LightGray => 2,
            TitleLevel::DarkGray => 3,
            TitleLevel::Stripped => 4,
        }
    }
}

impl Error for DataStructureError {}

impl std::fmt::Display for DataStructureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DataStructureError::MissingField { t, k } => write!(f, "{} Missing Field {}", t, k),
            DataStructureError::RectFailure => write!(f, "The rectangle did not contain 4 values"),
            
        }
    }
}

impl std::fmt::Display for StructType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use StructType::*;
        match self {
            // Notebook => write!(f, "Notebook"),
            Title => write!(f, "Title"),
            Link => write!(f, "Link"),
            // Page => write!(f, "Page"),
            // Layer => write!(f, "Layer"),
        }
    }
}
