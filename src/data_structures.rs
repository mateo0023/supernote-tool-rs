use std::collections::HashMap;
use std::fs::File;
use std::io;

use crate::decoder::decode_separate;

use super::io::extract_key_and_read;

pub mod metadata;

pub mod file_format_consts {
    pub const PAGE_HEIGHT: usize = 1872;
    pub const PAGE_WIDTH: usize = 1404;
}

use metadata::Metadata;
use serde::Serialize;

/// Will contain all the necessary information from the Notebook
/// 
/// # ToDo!
/// * Keyword
#[derive(Debug, Serialize)]
pub struct Notebook {
    /// Is the [Metadata] of the `.note` file
    pub metadata: Metadata,
    /// Is the version number, see [Metadata::file_id]
    pub file_id: String,
    /// A list containing all the [Titles](Title)
    /// 
    /// Titles will be sorted by Page and then Position
    /// to facilitate Bookmark Generation
    pub titles: Vec<Title>,
    /// A list containing all the [Links](Link)
    pub links: Vec<Link>,
    /// A list containing all the [Pages](Page)
    /// 
    /// Pages are sortedß
    pub pages: Vec<Page>,
    /// Map between PAGE_ID and page indexes.
    pub page_id_map: HashMap<String, usize>,
}

#[derive(Debug, Serialize)]
pub struct Title {
    pub metadata: metadata::MetaMap,
    pub content: Vec<u8>,
    pub title_level: TitleLevel,
    pub page_index: usize,
    pub position: u32,
    pub coords: [i32; 4],
    pub width: usize,
    pub height: usize,
    pub name: String,
}
#[derive(Debug, Serialize)]
pub struct Link {
    pub metadata: metadata::MetaMap,
    pub start_page: usize,
    pub link_type: LinkType,
    pub coords: [i32; 4],
}
#[derive(Debug, Serialize)]
pub struct Page {
    pub metadata: metadata::PageMeta,
    pub totalpath: Option<Vec<u8>>,
    pub recogn_file: Option<Vec<u8>>,
    pub recogn_text: Option<Vec<u8>>,
    pub layers: Vec<Layer>,
    pub page_num: usize,
    pub page_id: String,
}

#[derive(Debug, Serialize)]
pub struct Layer {
    pub metadata: metadata::MetaMap,
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
pub enum TitleLevel {
    #[default]
    BlackBack,
    LightGray,
    DarkGray,
    Stripped,
}

fn process_rect_to_corners(rect: Vec<i32>) -> [i32; 4] {
    if let [x1, y1, w, h, ..] = rect[..] {
        [
            x1, y1, x1 + w, y1 + h
        ]
    } else {
        todo!("rect did not contain 4 elements {:?}", rect)
    }
}

// ###########################################################################################################
// ###########################################################################################################
// ###########################################################################################################
// ############################################# IMPLEMENTATIONS #############################################
// ###########################################################################################################
// ###########################################################################################################
// ###########################################################################################################

impl Notebook {
    pub fn update_title(&mut self, title_idx: usize, new_title: &str) {
        self.titles[title_idx].name = new_title.to_string();
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

        let coords: Vec<i32> = metadata.get("TITLERECT").unwrap()[0].split(',').map(|v| v.parse().unwrap()).collect();
        let width = coords[2].unsigned_abs() as usize;
        let height = coords[3].unsigned_abs() as usize;
        let coords = process_rect_to_corners(coords);

        let title_level = TitleLevel::from_meta(metadata);
        
        let content = extract_key_and_read(file, metadata, "TITLEBITMAP").unwrap();
        let title = {
            let img = get_blurred_image(&content, width, height);
            match tesseract::ocr_from_frame(&img, width as i32, height as i32, 1, width as i32, "eng") {
                Ok(t) => t.chars().filter(char::is_ascii).collect(),
                Err(err) => todo!("{}", err),
            }
        };
        
        Ok(Title { 
            metadata: metadata.clone(),
            content,
            page_index,
            position: page_pos,
            title_level,
            coords,
            width,
            height,
            name: title,
        })
    }
}

fn get_blurred_image(content: &[u8], width: usize, height: usize) -> Vec<u8> {
    blur_image(
        &decode_separate(content, width * height).unwrap().as_black_white(),
        width, height
    )
}

fn blur_image(img: &[u8], w: usize, h: usize) -> Vec<u8> {
    const R: usize = 3;
    let mut blurred = Vec::from(img);
    let get_i = move |x: usize, y: usize| x + y * w;

    for idx in 0..(w*h) {
        let x_c = idx % w;
        let y_c = idx / w;
        //Only blur pixel if not black
        if img[get_i(x_c, y_c)] > 100 {
            let min_x = x_c.saturating_sub(R);
            let max_x = (x_c + R + 1).min(w);
            let min_y = y_c.saturating_sub(R);
            let max_y = (y_c + R + 1).min(h);

            let points: Vec<_> = (min_x..max_x)
                .flat_map(|x| (min_y..max_y)
                    .map(move |y| get_i(x, y))
                ).collect();
            let weight = points.len() as f32;
            let av = points.into_iter().map(|i| img[i] as f32).sum::<f32>() / weight;
            blurred[get_i(x_c, y_c)] = av as u8;
        };
    }

    blurred
}

impl Link {
    pub fn get_vec_from_meta(metadata: &Metadata) -> Vec<Link> {
        match &metadata.footer.links {
            Some(links) => links.iter().zip(Link::extract_page_numbers_from_meta(metadata).iter())
                .filter_map(|(link_meta, &page_num)| Link::new(link_meta, page_num, &metadata.file_id)).collect(),
            None => vec![],
        }
    }

    fn new(link_meta: &metadata::MetaMap, page_num: usize, file_id: &str) -> Option<Self> {
        if Link::is_incoming(link_meta) {
            return None;
        }
        Some(Link {
            metadata: link_meta.clone(),
            start_page: page_num,
            link_type: LinkType::from_meta(link_meta, file_id),
            coords: Self::get_link_rect(link_meta),
        })
    }

    fn is_incoming(link_meta: &metadata::MetaMap) -> bool {
        link_meta.get("LINKINOUT").unwrap()[0] == "1"
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

    fn get_link_rect(link_meta: &metadata::MetaMap) -> [i32; 4] {
        process_rect_to_corners(link_meta.get("LINKRECT").unwrap()[0].split(',')
            .map(|p| p.parse::<i32>().unwrap())
            .collect())
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
            page_num: metadata.page_info.get("PAGE_NUMBER").unwrap()[0].parse().unwrap(),
            page_id: metadata.page_info.get("PAGEID").unwrap()[0].clone(),
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
}

impl std::fmt::Display for TitleLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                TitleLevel::BlackBack => "BlackBack",
                TitleLevel::LightGray => "LightGray",
                TitleLevel::DarkGray => "DarkGray",
                TitleLevel::Stripped => "Stripped",
            }
        )
    }
}
