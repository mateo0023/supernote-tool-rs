use std::collections::HashMap;
use serde::Serialize;

mod io;

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

#[derive(Serialize)]
pub struct Metadata {
    pub version: u32,
    pub footer: Footer,
    pub header: MetaMap,
    pub pages: Vec<PageMeta>,
}

#[derive(Serialize, Default)]
pub struct Footer {
    pub main: MetaMap,
    pub keywords: Option<Vec<MetaMap>>,
    pub titles: Option<Vec<MetaMap>>,
    pub links: Option<Vec<MetaMap>>,
}

/// It's the data containg a single page.
#[derive(Serialize)]
pub struct PageMeta {
    pub page_info: MetaMap,
    pub layers: Vec<MetaMap>,
}

impl Footer {
    pub fn new(f: MetaMap, keywords: Option<Vec<MetaMap>>, titles: Option<Vec<MetaMap>>, links: Option<Vec<MetaMap>>) -> Self {
        Footer { main: f, keywords, titles, links }
    }

    pub fn get(&self, k: &str) -> Option<&Vec<String>> {
        self.main.get(k)
    }
}