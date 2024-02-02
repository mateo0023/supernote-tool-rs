use std::collections::HashMap;
use serde::Serialize;

mod io;

/// The type used by the metadata, a map between a `String` and a `Vec<String>`
pub type MetaMap = HashMap<String, Vec<String>>;

fn main() {
    let meta = io::load("./test/v15.note").unwrap();
    if let Ok(json) = serde_json::to_string_pretty(&meta) {
        println!("{}", &json);
        use std::fs::File;
        use std::io::Write;
        if let Ok(mut f) = File::create("./test/out.json") {
            let _ = f.write(json.as_bytes());
        }
    }
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
#[derive(Serialize)]
pub struct PageMeta {
    /// The [metadata](MetaMap) for each page including the address for the [layers](PageMeta::layers).
    pub page_info: MetaMap,
    /// A [vector](Vec) containing a list with the [metadata](MetaMap) for each layer.
    pub layers: Vec<MetaMap>,
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