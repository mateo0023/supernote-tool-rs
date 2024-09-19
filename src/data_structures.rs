use std::fs::File;
use std::io;

use super::io::extract_key_and_read;

pub mod metadata;

pub mod file_format_consts {
    pub const PAGE_HEIGHT: usize = 1872;
    pub const PAGE_WIDTH: usize = 1404;
}

use metadata::Metadata;

/// Will contain all the necessary information from the Notebook
/// 
/// # ToDo!
/// * Keyword
#[derive(Debug)]
pub struct Notebook {
    /// Is the [Metadata] of the `.note` file
    pub metadata: Metadata,
    /// Is the version number, see [Metadata::version]
    pub version: u32,
    // /// A list containing all the [Keywords](Keyword)
    // pub keywords: Vec<Keyword>,
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
pub struct Link {
    pub metadata: metadata::MetaMap,
    pub content: Option<Vec<u8>>,
    pub page: Option<usize>,
}
#[derive(Debug)]
pub struct Page {
    pub metadata: metadata::PageMeta,
    pub totalpath: Option<Vec<u8>>,
    pub recogn_file: Option<Vec<u8>>,
    pub recogn_text: Option<Vec<u8>>,
    pub layers: Vec<Layer>,
}

#[derive(Debug)]
pub struct Layer {
    pub metadata: metadata::MetaMap,
    pub content: Option<Vec<u8>>,
}

#[derive(Debug, Default)]
pub enum LinkType {
    #[default]
    SameFile,
    OtherFile,
    WebLink,
}


// ###########################################################################################################
// ###########################################################################################################
// ###########################################################################################################
// ############################################# IMPLEMENTATIONS #############################################
// ###########################################################################################################
// ###########################################################################################################
// ###########################################################################################################


impl Notebook {
    pub fn new(metadata: Metadata, file: &mut File) -> io::Result<Self> {
        let version = metadata.version;

        let titles = Title::get_vec_from_meta(&metadata, file).unwrap();
        let links = Link::get_vec_from_meta(&metadata, file);
        let pages = Page::get_vec_from_meta(&metadata.pages, file);

        Ok(Notebook { 
            metadata,
            version,
            titles,
            links,
            pages,
        })
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
    /// }
    /// ```
    pub fn from_meta(metadata: &metadata::MetaMap, file: &mut File) -> io::Result<Title> {
        // Very long chain with possible errors. But it should be fine as long as the file is properly formatted
        let page_pos = metadata.get("TITLERECTORI").unwrap().first().unwrap().split(',').nth(1).unwrap().parse().unwrap();
        // let bitmap_loc: u64 = metadata.get("TITLEBITMAP").unwrap().first().unwrap().parse().unwrap();
        let page_index = metadata.get("PAGE_NUMBER").unwrap().first().unwrap().parse::<usize>().unwrap() - 1;

        Ok(Title { 
            metadata: metadata.clone(),
            content: extract_key_and_read(file, metadata, "TITLEBITMAP").unwrap(),
            page_index,
            position: page_pos,
        })
    }
}

impl Link {
    pub fn get_vec_from_meta(metadata: &Metadata, file: &mut File) -> Vec<Link> {
        match &metadata.footer.links {
            Some(links) => links.iter().zip(Link::extract_page_numbers_from_meta(metadata).iter())
                .map(|(link, &page_num)| Link {
                    metadata: link.clone(),
                    content: extract_key_and_read(file, link, "LINKBITMAP"),
                    page: Some(page_num),
                }).collect(),
            None => vec![],
        }
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
}

impl Page {
    /// Given al vector of [page metadata](metadata::PageMeta) it will return a vector of [pages](Page).
    pub fn get_vec_from_meta(metadata: &[metadata::PageMeta], file: &mut File) -> Vec<Page> {
        metadata.iter().map(|meta| Page::from_meta(meta, file)).collect()
    }

    /// Given a [PageMeta](metadata::PageMeta) it returns a [Page].
    pub fn from_meta(metadata: &metadata::PageMeta, file: &mut File) -> Self {
        Page {
            metadata: metadata.clone(),
            totalpath: extract_key_and_read(file, &metadata.page_info, "TOTALPATH"),
            recogn_file: extract_key_and_read(file, &metadata.page_info, "RECOGNFILE"),
            recogn_text: extract_key_and_read(file, &metadata.page_info, "RECOGNTEXT"),
            layers: Layer::get_vec_fom_vec(&metadata.layers, file),
        }
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
            metadata: meta.clone(),
            content: extract_key_and_read(file, meta, "LAYERBITMAP"),
        }
    }

    pub fn get_name(&self) -> Option<&str> {
        self.metadata.get("LAYERNAME").map(|n| n[0].as_str())
    }

    pub fn is_background(&self) -> bool {
        if let Some(name) = self.get_name() {
            "BGLAYER".eq(name)
        } else {
            false
        }
    }
}
