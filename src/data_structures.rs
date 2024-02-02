use serde::Serialize;
use crate::MetaMap;

/// Will contain all the necessary information from the Notebook
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

pub struct Keyword {
    // pub page_number: 
}
pub struct Title {
    pub metadata: MetaMap,
    pub content: Option<Vec<u8>>,
    pub page_number: Option<u32>,
    pub position: u32,
}
pub struct Link;
pub struct Page {
    pub metadata: PageMeta,
    pub content: Option<Vec<u8>>,
    pub totalpath: Option<u32>,
    pub recogn_file: Option<u32>,
    pub recogn_text: Option<u32>,
    pub layers: Vec<Layer>,
}

pub struct Layer {
    pub metadata: MetaMap,
    pub content: Option<Vec<u8>>,
}

/// The data type used to hold the metadata of a `.note` file for the Supernote A5X
#[derive(Serialize)]
pub struct Metadata {
    /// The version number, an 8-digit integer
    pub version: u32,
    /// The [Footer] of the file, containing all the metadata of where the [header](Metadata::header) and [pages](Metadata::pages) are in the file
    pub footer: Footer,
    /// Contains a lot of metadata on device information and file status.
    pub header: MetaMap,
    /// A list of the page's metadata, represented by [PageMeta]
    pub pages: Vec<PageMeta>,
}

/// The footer is the main metadata container. It's address in the file is located on the last 4 bytes of data.
#[derive(Serialize, Default)]
pub struct Footer {
    /// Contains a series of keywords and addresses of where to get metadata on those keywords.
    /// It includes addresses for the metadata for:
    /// * The [Header](Metadata::header)
    /// * The [pages](PageMeta)' metadata
    /// * [Keywords](Footer::keywords)
    /// * [Titles](Footer::titles)
    /// * [Links](Footer::links)
    pub main: MetaMap,
    /// If there are any addresses for keywords it will contain a vector with their [MetaMap]
    pub keywords: Option<Vec<MetaMap>>,
    /// If there are any addresses for Titles it will contain a vector with their [MetaMap]
    pub titles: Option<Vec<MetaMap>>,
    /// If there are any addresses for Links it will contain a vector with their [MetaMap]
    pub links: Option<Vec<MetaMap>>,
}

/// It's the metadata of a single page.
#[derive(Serialize, Clone)]
pub struct PageMeta {
    /// The [metadata](MetaMap) for each page including the address for the [layers](PageMeta::layers).
    pub page_info: MetaMap,
    /// A [vector](Vec) containing a list with the [metadata](MetaMap) for each layer.
    pub layers: Vec<MetaMap>,
}

impl Metadata {
    /// Simply calls `get` on the [Metadata::footer], see [MetaMap]
    pub fn get(&self, k: &str) -> Option<&Vec<String>> {
        self.footer.get(k)
    }
}

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
    pub fn get_vec_from_meta(metadata: &Metadata) -> Vec<Keyword> {
        vec![]
    }
}

impl Title {
    /// It loops over the titles in [Metadata::footer::titles](Footer::titles) and maps it to a [Title] by calling [Title::from_meta].
    /// 
    /// # Returns
    /// Will return an empty vector if [Metadata::footer::titles](Footer::titles) is [None], otherwise, it will return the mapped values 
    /// as specified above.
    /// 
    /// # Panics
    /// It may panic when calling [Title::from_meta]
    pub fn get_vec_from_meta(metadata: &Metadata) -> Vec<Title> {
        match &metadata.footer.titles {
            Some(v) => v.iter().map(Title::from_meta).collect(),
            None => vec![],
        }
    }

    /// Will create a [Title] from its [MetaMap]. Will clone `metadata` and leave `content` and `page_number` as [None]
    /// 
    /// # Panics
    /// It will panic if the [MetaMap] doesn't contain the entry `"TITLERECTORI"` consisting of a list with one string.
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
    ///     content: None,
    ///     page_number: None,
    ///     position: u32,
    /// }
    /// ```
    pub fn from_meta(metadata: &MetaMap) -> Title {
        // Very long chain with possible errors. But it should be fine as long as the file is properly formatted
        let page_pos = metadata.get("TITLERECTORI").unwrap().first().unwrap().split(",").nth(1).unwrap().parse().unwrap();
        Title { 
            metadata: metadata.clone(),
            content: None,
            page_number: None,
            position: page_pos,
        }
    }
}

impl Link {
    pub fn get_vec_from_meta(metadata: &Metadata) -> Vec<Link> {
        vec![]
    }
}

impl Page {
    /// Given al vector of [page metadata](PageMeta) it will return a vector of [pages](Page).
    /// 
    /// Due to not having access to the [file](std::fs::File), all the Page's field that are Options will be [None]:
    /// * [Page::content]
    /// * [Page::totalpath]
    /// * [Page::recogn_file]
    /// * [Page::recogn_text]
    pub fn get_vec_from_meta(metadata: &[PageMeta]) -> Vec<Page> {
        metadata.iter().map(Page::from_meta).collect()
    }

    /// Given a [PageMeta] it returns a [Page].
    /// 
    /// Due to not having access to the [file](std::fs::File), all the Options will be [None]:
    /// * [Page::content]
    /// * [Page::totalpath]
    /// * [Page::recogn_file]
    /// * [Page::recogn_text]
    pub fn from_meta(metadata: &PageMeta) -> Self {
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
    /// Given a vector of layer [metadata](MetaMap), it retrns a vector of [Layer].
    pub fn get_vec_fom_vec(layers: &[MetaMap]) -> Vec<Self> {
        layers.iter().map(Layer::from_meta).collect()
    }

    /// Creates a layer purely by cloning [meta](MetaMap) and keeping [content](Layer::content) as [None].
    pub fn from_meta(meta: &MetaMap) -> Self {
        Layer {
            metadata: meta.clone(),
            content: None,
        }
    }
}

impl Footer {
    pub fn new(f: MetaMap, keywords: Option<Vec<MetaMap>>, titles: Option<Vec<MetaMap>>, links: Option<Vec<MetaMap>>) -> Self {
        Footer { main: f, keywords, titles, links }
    }

    /// Simply calls `get` on the [Footer::main], see [MetaMap]
    pub fn get(&self, k: &str) -> Option<&Vec<String>> {
        self.main.get(k)
    }
}
