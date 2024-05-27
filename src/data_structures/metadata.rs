//! Where all the metadata-relevant structs go.

use std::collections::HashMap;

use serde::Serialize;

/// The type used by the metadata, a map between a `String` and a `Vec<String>`
pub type MetaMap = HashMap<String, Vec<String>>;

/// The data type used to hold the metadata of a `.note` file for the Supernote A5X
#[derive(Serialize, Debug)]
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

/// It's the metadata of a single page.
#[derive(Debug, Serialize, Clone)]
pub struct PageMeta {
    /// The [metadata](MetaMap) for each page including the address for the [layers](PageMeta::layers).
    pub page_info: MetaMap,
    /// A [vector](Vec) containing a list with the [metadata](MetaMap) for each layer.
    pub layers: Vec<MetaMap>,
}

/// The footer is the main metadata container. It's address in the file is located on the last 4 bytes of data.
#[derive(Debug, Serialize, Default)]
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


// ###########################################################################################################
// ###########################################################################################################
// ###########################################################################################################
// ############################################# IMPLEMENTATIONS #############################################
// ###########################################################################################################
// ###########################################################################################################
// ###########################################################################################################


impl Metadata {
    /// Simply calls `get` on the [Metadata::footer], see [MetaMap]
    pub fn get(&self, k: &str) -> Option<&Vec<String>> {
        self.footer.get(k)
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

    /// Retuns the page numbers of all the titles as ordered by page
    pub fn get_page_numbers_with_key(&self, key: &str) -> Vec<usize> {
        self.main.keys().filter_map(|k| match k.starts_with(key) {
            true => Some(k[6..10].parse::<usize>().unwrap()),
            false => None,
        }).collect()
    }
}
