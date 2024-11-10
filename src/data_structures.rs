use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

use super::io::extract_key_and_read;

pub mod metadata;
pub mod stroke;
pub mod cache;


use futures::FutureExt;
use lopdf::content::Content;
pub use stroke::StrokeError;
pub use stroke::TransciptionError;
use cache::NotebookCache;
use stroke::Stroke;
pub use stroke::ServerConfig;
use tokio::sync::RwLock;

use crate::exporter::page_to_commands;
use crate::ColorMap;

/// It contains:
/// 
/// 0. [Notebook]
/// 1. [Metadata]
/// 2. A vector with the ([`page_id_`](Page::page_id), [`Stroke`]s)
pub type NotebookReturn = (Notebook, Metadata, Vec<(u64, Option<Vec<Stroke>>)>);

/// A tuple type that contains:
/// 
/// 0. [Page]
/// 1. `(page_id, strokes)`
///    0. [`page_id`](Page::page_id)
///    1. `Option<Vec<Stroke>>`, see [Stroke].
pub type PageAndStroke = (Page, (u64, Option<Vec<Stroke>>));

pub mod file_format_consts {
    pub const PAGE_HEIGHT: usize = 1872;
    pub const PAGE_WIDTH: usize = 1404;
}

use metadata::Metadata;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub enum Transciption {
    Manual(String),
    MyScript(String),
    #[default]
    None
}

#[derive(Clone)]
pub struct Notebook {
    // /// The file name (not including the extension)
    // pub file_name: String,
    /// The ID used to identify the file, see [Metadata::file_id]
    pub file_id: u64,
    /// A list containing all the [Links](Link)
    pub links: Vec<Link>,
    /// A list containing all the [Pages](Page)
    /// 
    /// Pages are sorted
    pub pages: Vec<PageOrCommand>,
    /// Map between [`PAGE_ID`](Page::page_id) and page indexes.
    pub page_id_map: HashMap<u64, usize>,
    /// The notebook's starting page.
    /// 
    /// Used when chaining multiple [Notebook]s
    /// into a single PDF.
    pub starting_page: usize,
}

#[derive(Clone, Default)]
pub struct TitleCollection {
    /// A list containing all the [Titles](Title)
    /// 
    /// Titles will be sorted by Page and then Position
    /// to facilitate Bookmark Generation
    pub titles: HashMap<u64, Title>,
    pub note_id: u64,
    pub note_name: String,
}

#[derive(Serialize, Clone, Default)]
pub struct Title {
    /// The encoded content of the Title.
    /// 
    /// To be decoded into a Bitmap
    pub content: Option<Vec<u8>>,
    /// The hash of [`Self::content`], if any.
    /// Otherwise it will be a hash of the:
    /// 1. `page_id`, and
    /// 2. [`Self::title_level`]
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
    pub page_id: u64,
    // /// The vertical position on the page.
    // /// Same as [`coords[1]`](Self::coords)
    // pub position: u32,
    /// The rectangle defined by
    /// `[x_min, y_min, x_max, y_max]`
    pub coords: [u32; 4],
    // pub width: usize,
    // pub height: usize,
    pub name: Transciption,
}
#[derive(Debug, Clone, Serialize)]
pub struct Link {
    pub start_page: usize,
    pub link_type: LinkType,
    pub coords: [u32; 4],
}

#[derive(Debug, Clone)]
pub enum PageOrCommand {
    Page(Page),
    Command(lopdf::content::Content)
}

#[derive(Debug, Clone)]
pub struct Page {
    pub layers: Vec<Layer>,
    pub page_num: usize,
    pub page_id: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Layer {
    pub is_background: bool,
    pub content: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Serialize)]
pub enum LinkType {
    /// A link to the same file, containing the page index
    SameFile{page_id: u64},
    /// A link to the same file, containing:
    /// * Page Index
    /// * The other's [`file_id`](Notebook::file_id)
    OtherFile{page_id: u64, file_id: u64},
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
pub fn hash(content: &[u8]) -> u64 {
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

impl Transciption {
    pub async fn transcribe(strokes: Vec<Stroke>, config: Arc<RwLock<stroke::ServerConfig>>) -> Self {
        match stroke::transcribe(strokes, config).await {
            Ok(s) => Transciption::MyScript(s),
            Err(_) => Transciption::None,
        }
    }
    
    pub async fn from_stroke_and_cache(strokes: Vec<Stroke>, config: Arc<RwLock<stroke::ServerConfig>>, other: &Transciption) -> Self {
        match other {
            Transciption::Manual(s) => Transciption::Manual(s.clone()),
            Transciption::MyScript(s) => Transciption::MyScript(s.clone()),
            Transciption::None => Self::transcribe(strokes, config).await,
        }
    }

    /// Will get the transcription.
    /// 
    /// [`None`](Transciption::None) will return an empty `&str`
    pub fn get_or_default(&self) -> &str {
        match self {
            Transciption::Manual(txt) |
            Transciption::MyScript(txt) => txt.as_str(),
            Transciption::None => "",
        }
    }

    /// Merges the `other` [Transciption] into self.
    pub fn merge_into(&mut self, other: Transciption) {
        if self.should_merge(&other) {
            *self = other;
        }
    }

    /// Clone the [`Transciption`] only if it's been transcribed already
    /// (it's [`Manual`](Transciption::Manual) or
    /// [`MyScript`](Transciption::MyScript))
    pub fn get_clone_for_cache(&self) -> Option<Self> {
        match self {
            Transciption::Manual(s) => Some(Transciption::Manual(s.clone())),
            Transciption::MyScript(s) => Some(Transciption::MyScript(s.clone())),
            Transciption::None => None,
        }
    }

    /// Merges the `other` [Transciption] into `self`.
    pub fn merge_into_ref(&mut self, other: &Transciption) {
        *self = match (other, std::mem::take(self)) {
            (Transciption::Manual(s), _) => Transciption::Manual(s.clone()),
            (Transciption::MyScript(s), Transciption::None) => Transciption::MyScript(s.clone()),
            (Transciption::MyScript(_), old_self) => old_self,
            (Transciption::None, old_self) => old_self,
        }
    }

    /// Wether we should merge `other` into [self].
    fn should_merge(&self, other: &Transciption) -> bool {
        match (other, &self) {
            (Transciption::Manual(_), _) => true,
            (Transciption::MyScript(_), Transciption::None) => true,
            (Transciption::MyScript(_), _) => false,
            (Transciption::None, _) => false,
        }
    }
}

impl Notebook {
    /// Create a [Notebook] given an open `.note` file and 
    /// a [file name](String)
    pub fn from_file(file: &[u8]) -> Result<NotebookReturn, Box<dyn Error>> {
        let metadata = Metadata::from_file(file)?;
        let file_id = metadata.file_id;
        let links = Link::get_vec_from_meta(&metadata);
        let mut pages = Page::get_vec_from_meta(&metadata.pages, file);
        pages.sort_by_key(|p| p.0.page_num);

        let page_id_map = HashMap::from_iter(pages.iter().map(|page| (page.1.0, page.0.page_num - 1)));

        let (pages, page_data) = {
            let mut pages_sep = Vec::with_capacity(pages.len());
            let mut other = Vec::with_capacity(pages.len());
            for (page, oth) in pages.into_iter() {
                pages_sep.push(PageOrCommand::Page(page));
                other.push(oth);
            }
            (pages_sep, other)
        };

        Ok((Notebook {
            file_id,
            links,
            pages,
            page_id_map,
            // file_name: name,
            starting_page: 0,
        }, metadata, page_data))
    }

    /// Will get the PDF page number given the `page_id` and the internal
    /// [starting_page](Self::starting_page).
    pub fn get_page_index_from_id(&self, page_id: u64) -> Option<usize> {
        self.page_id_map.get(&page_id).copied().map(|idx| idx + self.starting_page)
    }

    pub fn into_commands(mut self, colormap: ColorMap) -> Self {
        use PageOrCommand::*;
        self.pages = 
            self.pages.into_iter().map(|page| -> Result<Content, Box<dyn Error>> {
                match page {
                    Page(page) => page_to_commands(page, colormap),
                    Command(content) => Ok(content),
                }
            })
            .map(|c| Command(c.unwrap())).collect();
        self
    }
}

impl TitleCollection {
    /// Update the title's [name](Title::name)
    /// field given the hash value and [new_title](Transciption) (from [AppCache])
    /// 
    /// ### Name
    /// Will set it to [None](Transciption::None) if empty.
    /// 
    /// ### Strokes
    /// Will set to [None](StrokeContainer::None) if there's already a transcription
    pub fn update_title(&mut self, title_hash: u64, new_title: &Transciption) {
        if let Some(title) = self.titles.get_mut(&title_hash) {
            title.name.merge_into_ref(new_title);
        }
    }

    pub async fn transcribe_titles(
        metadata: Metadata, data: Vec<u8>,
        cache: Option<NotebookCache>, config: Arc<RwLock<ServerConfig>>,
        page_data: Vec<(u64, Option<Vec<Stroke>>)>,
        file_name: String,
    ) -> Result<Self, Box<dyn Error>> {
        let note_id = metadata.file_id;
        let titles = {
            let mut titles = Title::get_vec_from_meta(metadata, data, page_data, cache.as_ref(), config)
                .await?;
            titles.sort();

            let mut ghost_titles = vec![];
            let mut prev_level = TitleLevel::FileLevel;
            for t in titles.iter() {
                while (prev_level as u8) + 1 < t.title_level as u8 {
                    prev_level = prev_level.add();
                    let mut title = Title::new_ghost(prev_level, t);
                    // Update transcription if already done so.
                    if let Some(note_cache) = cache.as_ref() {
                        if let Some(tr) = note_cache.get(&title.hash) {
                            title.name = tr.title.clone();
                        }
                    }
                    ghost_titles.push(title);
                }
                prev_level = t.title_level;
            }
            titles.extend(ghost_titles);

            HashMap::from_iter(
                titles.into_iter()
                .map(|t| (t.hash, t))
            )
        };
        Ok(Self {
            titles,
            note_id,
            note_name: file_name,
        })
    }

    /// See [Title::cmp]
    pub fn get_sorted_titles(&self) -> Vec<&Title> {
        let mut titles: Vec<&Title> = self.titles.values().collect();
        titles.sort();
        titles
    }
    /// Computes the [`NotebookCache`] given the already-processed
    /// Title's [`Transcription`](Transciption).
    fn get_cache(&self) -> NotebookCache {
        self.titles.iter()
            .filter_map(|(&k, title)|
                cache::TitleCache::form_title(
                    title,
                )
                .map(|c| (k, c))
            ).collect()
    }
}

impl Title {
    /// Create a new [Title] that will be used to indicate a file.
    pub fn new_for_file(name: &str, index: usize) -> Self {
        Title {
            title_level: TitleLevel::FileLevel,
            page_index: index,
            name: Transciption::Manual(name.to_string()),
            ..Default::default()
        }
    }

    async fn transcribe(mut self, strokes: Vec<Stroke>, config: Arc<RwLock<ServerConfig>>) -> Self {
        let new_name = Transciption::transcribe(strokes, config).await;
        self.name = new_name;
        self
    }

    /// Creates a new *ghost* title.
    /// 
    /// These are the titles are the are missing in the tree structure.
    pub fn new_ghost(title_level: TitleLevel, reference_t: &Title) -> Self {
        let hash = {
            use std::hash::{DefaultHasher, Hasher as _};
    
            let mut hasher = DefaultHasher::new();
            hasher.write_u64(reference_t.page_id);
            hasher.write(&[title_level as u8]);
            hasher.finish()
        };

        Self {
            hash,
            title_level,
            page_index: reference_t.page_index,
            coords: reference_t.coords,
            page_id: reference_t.page_id,
            content: None,
            name: Transciption::None,
        }
    }

    /// Used to exporting into a ToC. Will create a
    /// [Title] with default values for all except:
    /// * [name](Self::name), will be the same (clone)
    /// * [page_index](Self::page_index), which will be shifted by `shift`
    /// * [title_level](Self::title_level), will be the same (copy)
    pub fn basic_for_toc(&self, shift: usize) -> Self {
        Title {
            name: self.name.get_clone_for_cache().unwrap_or_default(),
            page_index: self.page_index + shift,
            title_level: self.title_level,
            ..Default::default()
        }
    }

    /// It loops over the titles in [Metadata::footer::titles](metadata::Footer::titles) and maps it to a [Title] by calling [Title::from_meta_no_transcript].
    /// 
    /// # Returns
    /// Will return an empty vector if [Metadata::footer::titles](metadata::Footer::titles) is [None], otherwise, it will return the mapped values 
    /// as specified above.
    /// 
    /// # Panics
    /// It may panic when calling [Title::from_meta_no_transcript]
    pub async fn get_vec_from_meta(metadata: Metadata, file: Vec<u8>, page_data: Vec<(u64, Option<Vec<Stroke>>)>, cache: Option<&NotebookCache>, config: Arc<RwLock<ServerConfig>>) -> Result<Vec<Title>, Box<dyn Error>> {
        match &metadata.footer.titles {
            Some(v) => {
                let mut f: Vec<_> = vec![];
                for metadata in v.iter() {
                    let title = Title::from_meta_no_transcript(metadata.clone(), &file, cache)?;
                    f.push(
                        if let Transciption::None = &title.name {
                            match &page_data[title.page_index].1 {
                                Some(strokes) => {
                                    let strokes = stroke::clone_strokes_contained(
                                        strokes,
                                        title.coords
                                    );
                                    title.transcribe(strokes, config.clone()).boxed()
                                },
                                None => async {title}.boxed(),
                            }
                        } else {
                            async {title}.boxed()
                        }
                    );
                }
                Ok(futures::future::join_all(f).await)
            },
            None => Ok(vec![]),
        }
    }

    /// Will create a [Title] from its [`MetaMap`](metadata::MetaMap). Will clone `metadata` and read content from the file.
    /// 
    /// It will **not** perform transcription, [`self.name`](Title::name) will be [`Transciption::None`]
    /// if it's not in the [`NotebookCache`]
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
    fn from_meta_no_transcript(metadata: metadata::MetaMap, file: &[u8], cache: Option<&NotebookCache>) -> Result<Title, Box<dyn Error>> {
        // Very long chain with possible errors. But it should be fine as long as the file is properly formatted
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
        let coords = process_rect_to_corners(coords)?;

        let title_level = TitleLevel::from_meta(&metadata);

        let content = Vec::from(extract_key_and_read(file, &metadata, "TITLEBITMAP")
            .ok_or(DataStructureError::MissingField { t: StructType::Title, k: "TITLEBITMAP".to_string() })?);
        let hash = hash(&content);

        let name = match cache {
            Some(note_cache) => match note_cache.get(&hash) {
                Some(cache) => match &cache.title {
                    Transciption::Manual(s) => Transciption::Manual(s.clone()),
                    Transciption::MyScript(s) => Transciption::MyScript(s.clone()),
                    Transciption::None => Transciption::None,
                },
                None => Transciption::None,
            },
            None => Transciption::None,
        };

        Ok(Title {
            content: Some(content),
            hash,
            page_index,
            title_level,
            coords,
            name,
            page_id: 0,
        })
    }

    /// Returns the title's name (text contained in there).
    /// 
    /// Will default to an empty string.
    pub fn get_name(&self) -> String {
        self.name.get_or_default().to_string()
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
    /// 2. [position](Self::coords) (2nd element)
    /// 3. [title_level](Self::title_level)
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering::Equal;
        match self.page_index.cmp(&other.page_index) {
            Equal => match self.coords[1].cmp(&other.coords[1]) {
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

    fn new(link_meta: &metadata::MetaMap, page_num: usize, file_id: &u64) -> Result<Option<Self>, Box<dyn Error>> {
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

impl PageOrCommand {
    pub fn command(&self) -> &lopdf::content::Content {
        match self {
            PageOrCommand::Page(_) => panic!("Still not processed into commands"),
            PageOrCommand::Command(content) => content,
        }
    }
}

impl Page {
    /// Given al vector of [page metadata](metadata::PageMeta) it will return a vector of [pages](Page).
    pub fn get_vec_from_meta(metadata: &[metadata::PageMeta], file: &[u8]) -> Vec<PageAndStroke> {
        metadata.iter().map(|meta| Page::from_meta(meta, file)).collect()
    }

    /// Given a [PageMeta](metadata::PageMeta) it returns a [Page].
    pub fn from_meta(metadata: &metadata::PageMeta, file: &[u8]) -> (Self, (u64, Option<Vec<Stroke>>)) {
        // Page might be empty.
        let totalpath = extract_key_and_read(file, &metadata.page_info, "TOTALPATH")
            .map(|paths|
                stroke::Stroke::process_page(paths)
                    .expect("Failed to process the strokes in page")
            );
        let page_id = hash(metadata.page_info.get("PAGEID").unwrap()[0].as_bytes());
        (Page {
            // recogn_file: extract_key_and_read(file, &metadata.page_info, "RECOGNFILE"),
            // recogn_text: extract_key_and_read(file, &metadata.page_info, "RECOGNTEXT"),
            layers: Layer::get_vec_fom_vec(&metadata.layers, file),
            page_num: metadata.page_info.get("PAGE_NUMBER").unwrap()[0].parse().unwrap(),
            page_id,
        }, (page_id, totalpath))
    }
}

impl Layer {
    /// Given a vector of layer [metadata](metadata::MetaMap), it retrns a vector of [Layer].
    pub fn get_vec_fom_vec(layers: &[metadata::MetaMap], file: &[u8]) -> Vec<Self> {
        layers.iter().map(|meta| Layer::from_meta(meta, file)).collect()
    }

    /// Creates a layer purely by cloning [meta](metadata::MetaMap) and reading the [contents](Layer::content) with [extract_key_and_read].
    pub fn from_meta(meta: &metadata::MetaMap, file: &[u8]) -> Self {
        Layer {
            is_background: meta.get("LAYERNAME").map(|n| n[0].eq("BGLAYER")).unwrap_or(false),
            content: extract_key_and_read(file, meta, "LAYERBITMAP").map(Vec::from),
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
    
    pub fn from_meta(link_meta: &metadata::MetaMap, file_id: &u64) -> Self {
        let link_style = link_meta.get(Self::KEY_STYLE).unwrap()[0].as_str();
        // Link to website
        if link_style.eq(Self::TO_WEB) {
            return LinkType::WebLink { link: link_meta.get("LINKFILE").unwrap()[0].clone() };
        }
        // Is internal/external
        if link_style.eq(Self::TO_PAGE) {
            let page_id = hash(link_meta.get("PAGEID").unwrap()[0].as_bytes());
            let to_file_id = hash(link_meta.get(Self::KEY_FILE_ID).unwrap()[0].as_bytes());

            match to_file_id.eq(file_id) {
                true => LinkType::SameFile { page_id },
                false => LinkType::OtherFile { page_id, file_id: to_file_id },
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
