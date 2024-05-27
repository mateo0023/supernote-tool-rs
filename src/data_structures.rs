use std::fs::File;
use std::io;

use super::io::get_content_at_address;

pub mod metadata;

use metadata::Metadata;

/// Will contain all the necessary information from the Notebook
/// 
/// # ToDo!
/// * Keyword
/// * Title
/// * Link
/// * Page
#[derive(Debug)]
pub struct Notebook {
    /// Is the [Metadata] of the `.note` file
    pub metadata: Metadata,
    /// Is the version number, see [Metadata::version]
    pub version: u32,
    /// A list containing all the [Keywords](Keyword)
    pub keywords: Vec<Keyword>,
    /// A list containing all the [Titles](Title)
    pub titles: Vec<Title>,
    /// A list containing all the [Links](Link)
    pub links: Vec<Link>,
    /// A list containing all the [Pages](Page)
    pub pages: Vec<Page>,
}

#[derive(Debug)]
pub struct Keyword {
    pub page_number: usize,
    pub metadata: metadata::MetaMap,
}
#[derive(Debug)]
pub struct Title {
    pub metadata: metadata::MetaMap,
    pub content: Vec<u8>,
    pub page_index: usize,
    pub position: u32,
}
#[derive(Debug)]
pub struct Link;
#[derive(Debug)]
pub struct Page {
    pub metadata: metadata::PageMeta,
    pub content: Option<Vec<u8>>,
    pub totalpath: Option<u32>,
    pub recogn_file: Option<u32>,
    pub recogn_text: Option<u32>,
    pub layers: Vec<Layer>,
}

#[derive(Debug)]
pub struct Layer {
    pub metadata: metadata::MetaMap,
    pub content: Option<Vec<u8>>,
}


// ###########################################################################################################
// ###########################################################################################################
// ###########################################################################################################
// ############################################# IMPLEMENTATIONS #############################################
// ###########################################################################################################
// ###########################################################################################################
// ###########################################################################################################


impl Notebook {
    pub fn new(metadata: Metadata) -> Self {
        let version = metadata.version;

        Notebook { 
            metadata,
            version,
            keywords: todo!(),
            titles: todo!(),
            links: todo!(),
            pages: todo!(),
        }
    }
}

impl Keyword {
    pub fn get_vec_from_meta(metadata: &Metadata, file: &mut File) -> Vec<Keyword> {
        vec![]
    }
}

impl Title {
    /// It loops over the titles in [Metadata::footer::titles](metadata::Footer::titles) and maps it to a [Title] by calling [Title::from_meta].
    /// 
    /// # Returns
    /// Will return an empty vector if [Metadata::footer::titles](metadata::Footer::titles) is [None], otherwise, it will return the mapped values 
    /// as specified above.
    /// 
    /// # Panics
    /// It may panic when calling [Title::from_meta]
    pub fn get_vec_from_meta(metadata: &Metadata, file: &mut File) -> io::Result<Vec<Title>> {
        match &metadata.footer.titles {
            Some(v) => v.iter().map(|metadata| Title::from_meta(metadata, file)).collect(),
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
    /// }1
    /// ```
    pub fn from_meta(metadata: &metadata::MetaMap, file: &mut File) -> io::Result<Title> {
        // Very long chain with possible errors. But it should be fine as long as the file is properly formatted
        let page_pos = metadata.get("TITLERECTORI").unwrap().first().unwrap().split(',').nth(1).unwrap().parse().unwrap();
        let bitmap_loc: u64 = metadata.get("TITLEBITMAP").unwrap().first().unwrap().parse().unwrap();
        let page_index = metadata.get("PAGE_NUMBER").unwrap().first().unwrap().parse::<usize>().unwrap() - 1;

        Ok(Title { 
            metadata: metadata.clone(),
            content: get_content_at_address(file, bitmap_loc)?,
            page_index,
            position: page_pos,
        })
    }
}

impl Link {
    pub fn get_vec_from_meta(metadata: &Metadata, file: &mut File) -> Vec<Link> {
        vec![]
    }
}

impl Page {
    /// Given al vector of [page metadata](metadata::PageMeta) it will return a vector of [pages](Page).
    /// 
    /// Due to not having access to the [file](std::fs::File), all the Page's field that are Options will be [None]:
    /// * [Page::content]
    /// * [Page::totalpath]
    /// * [Page::recogn_file]
    /// * [Page::recogn_text]
    pub fn get_vec_from_meta(metadata: &[metadata::PageMeta], file: &mut File) -> Vec<Page> {
        metadata.iter().map(Page::from_meta).collect()
    }

    /// Given a [PageMeta](metadata::PageMeta) it returns a [Page].
    /// 
    /// Due to not having access to the [file](std::fs::File), all the Options will be [None]:
    /// * [Page::content]
    /// * [Page::totalpath]
    /// * [Page::recogn_file]
    /// * [Page::recogn_text]
    pub fn from_meta(metadata: &metadata::PageMeta) -> Self {
        Page {
            metadata: metadata.clone(),
            content: None,
            totalpath: None,
            recogn_file: None,
            recogn_text: None,
            layers: Layer::get_vec_fom_vec(&metadata.layers),
        }
    }
}

impl Layer {
    /// Given a vector of layer [metadata](metadata::MetaMap), it retrns a vector of [Layer].
    pub fn get_vec_fom_vec(layers: &[metadata::MetaMap]) -> Vec<Self> {
        layers.iter().map(Layer::from_meta).collect()
    }

    /// Creates a layer purely by cloning [meta](metadata::MetaMap) and keeping [content](Layer::content) as [None].
    pub fn from_meta(meta: &metadata::MetaMap) -> Self {
        Layer {
            metadata: meta.clone(),
            content: None,
        }
    }
}
