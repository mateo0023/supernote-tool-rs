//! Loads the data and metadata

use std::fs::File;
use std::io::{self, prelude::*, SeekFrom};

use byteorder::{LittleEndian, ReadBytesExt};
use regex::Regex;

use crate::data_structures::{*, metadata::{Metadata, MetaMap}};

pub mod f_fmt {
    //! It's the file format information.
    //!
    //! Contains the variables and data needed to read the *.note file.

    /// The latest version of the file supported by the library.
    pub const SUPPORTED_VERSION: u32 = 20230015;

    /// The number of bytes that will be taken by irrelevant characters
    /// before the version number. It is the text `noteSN_FILE_VER_`
    pub const BYTES_BEFORE_VERSION_NUM: u64 = 16;
    /// The length in characters used to represent
    /// the version number. Because it is encoded as a ASCII string.
    pub const VERSION_NUM_BYTE_LEN: usize = 8;

    /// The number of characters that determine an address
    pub const ADDR_SIZE: u64 = 4;
    /// The type of the address as stored on the file
    pub type AddrType = u32;
    
    /// The possible Keywords in the `.note` file that are used for metadata.
    pub enum Keyword {
        Keyword,
        Title,
        Link,
        Page,
    }


    impl std::fmt::Display for Keyword {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Keyword::Keyword => write!(f, "KEYWORD_"),
                Keyword::Title => write!(f, "TITLE_"),
                Keyword::Link =>  write!(f, "LINK0_"),
                Keyword::Page =>  write!(f, "PAGE"),
            }
        }
    }

    impl Keyword {
        pub fn as_str(&self) -> &'static str {
            match self {
                Keyword::Keyword => "KEYWORD_",
                Keyword::Title => "TITLE_",
                Keyword::Link =>  "LINK0_",
                Keyword::Page =>  "PAGE",
            }
        }

        /// Extracts the page number from the full key (ie: "LINKO_00050360015301061245") based on [self]:
        /// * [Title](Keyword::Title) `6..10`
        /// * [Link](Keyword::Link) `6..10`
        /// * [Page](Keyword::Page) `4..`
        /// * **Others** [todo!]
        /// 
        /// # Returns
        /// [String]
        pub fn page_number_str(&self, key: &str) -> String {
            match self {
                Keyword::Keyword => todo!(),
                Keyword::Title
                | Keyword::Link => key[6..10].to_string(),
                Keyword::Page => key[4..].to_string(),
            }
        }

        /// Calls [page_number_str](Self::page_number_str) removes one and returns as [usize] (failing if not properly parsed).
        pub fn page_index(&self, key: &str) -> usize {
            self.page_number_str(key).parse::<usize>().unwrap() - 1
        }
    }

}

const LAYER_KEYS: [&str; 5] = ["MAINLAYER", "LAYER1", "LAYER2", "LAYER3", "BGLAYER"];


/// Loads
pub fn load(path: &str) -> io::Result<Notebook> {
    let mut file = File::open(path)?;

    Notebook::from_file(&mut file)
}

/// Looks at the beggining of the file where the file version should be.
///
/// # Errors
/// If it cannot read the file or if it's shorter than 24 bytes.
///
/// # Return
/// It returns the version number as [`u32`] or [`None`] if it cannot be parsed from
/// a string.
///
/// # Context
/// Note X generation devices begin with `noteSN_FILE_VER_` followed by an 8-digit
/// number represented by UTF-8 characters
fn read_file_version(file: &mut File) -> io::Result<Option<u32>> {
    file.seek(SeekFrom::Start(f_fmt::BYTES_BEFORE_VERSION_NUM))?;
    let mut buf = [0; f_fmt::VERSION_NUM_BYTE_LEN];
    if file.read(&mut buf)? < buf.len() {
        todo!("File has less than {} bytes", buf.len())
    }

    let version = match std::str::from_utf8(&buf) {
        Ok(s) => s.parse(),
        Err(err) => todo!(
            "Found error when parsing version number at start of file {:?}",
            err
        ),
    };

    match version {
        Ok(v) => Ok(Some(v)),
        Err(_) => Ok(None),
    }
}

/// Loads a block the size specified by the first [`f_fmt::ADDR_SIZE`] bytes after the address
/// and parses them into a [`MetaMap`].
///
/// # Returns
/// Saving any [`io::error`] it returns the [`MetaMap`] and if there are no values, it returns [`None`]
///
/// # Panics
/// Can occur if the regex used to search kewyords cannot be created.
fn parse_meta_block(file: &mut File, addr: u64) -> io::Result<Option<MetaMap>> {
    let meta = get_content_at_address(file, addr)?;
    let meta = String::from_utf8_lossy(&meta);

    let regex = match Regex::new(r"<([^:<>]+):([^:<>]*)>") {
        Ok(r) => r,
        Err(e) => panic!("Encountered error creating a regex: {}", e),
    };

    let mut map = MetaMap::new();
    for m in regex.captures_iter(&meta) {
        if let (Some(key), Some(value)) = (m.get(1), m.get(2)) {
            let key = key.as_str().to_string();
            let value = value.as_str();
            map.entry(key)
                .and_modify(|list| list.push(value.to_string()))
                .or_insert(vec![value.to_string()]);
        }
    }

    match map.is_empty() {
        true => Ok(None),
        false => Ok(Some(map)),
    }
}

/// Loops through the entries that begin with `keyword` and converts the string
/// value into addresses (where the actual metadata is located) and extracts the *page number* (held in the characters 6 through 10).
/// Collecting all of them into a single vector of ([`AddrType`](f_fmt::AddrType), [String])
fn get_keyword_addresses(
    metadata: &MetaMap,
    keyword: f_fmt::Keyword,
) -> Option<Vec<(f_fmt::AddrType, String)>> {
    let addresses: Vec<(f_fmt::AddrType, String)> = metadata
        .iter()
        .filter_map(|(k, v)| match k.starts_with(keyword.as_str()) {
            true => {
                Some(v.iter().map(|n| match n.parse::<f_fmt::AddrType>() {
                    Ok(num) => (num, keyword.page_number_str(k)),
                    Err(_) => todo!(),
                }))
            }
            false => None,
        })
        .flatten()
        .collect();

    match addresses.is_empty() {
        true => None,
        false => Some(addresses),
    }
}

/// Gets the keyword metadata from the file given a list of addresses.
///
/// Essentially calls [`parse_meta_block`] on every address and collects
///
/// # Errors
/// This function will ignore any I/O errors encountered
fn parse_addresses_to_meta(file: &mut File, k_addrs: Vec<(f_fmt::AddrType, String)>) -> Vec<MetaMap> {
    k_addrs
        .iter()
        .filter_map(|(addr, page_num)|
            parse_meta_block(file, *addr as u64).unwrap_or(None)
                .map(|mut map| {
                    map.insert("PAGE_NUMBER".to_string(), vec![page_num.clone()]);
                    map
                })
        )
        .collect()
}

/// Does what it says
fn get_all_meta_on_keyword(file: &mut File, meta: &MetaMap, keyword: f_fmt::Keyword) -> Option<Vec<MetaMap>> {
    get_keyword_addresses(meta, keyword).map(|k_addrs| parse_addresses_to_meta(file, k_addrs))
}

/// Goes through the page addresses getting their metadata and layer information
fn parse_pages(file: &mut File, addrs: Vec<(f_fmt::AddrType, String)>) -> io::Result<Vec<metadata::PageMeta>> {
    let mut pages = Vec::with_capacity(addrs.len());
    for (addr, page_num) in addrs {
        let page_info = parse_meta_block(file, addr as u64)?.map(|mut m| {
            m.insert("PAGE_NUMBER".to_string(), vec![page_num]);
            m
        }).unwrap();

        let layer_addrs: Vec<_> = page_info
            .iter()
            .filter_map(|(k, v)| match LAYER_KEYS.contains(&k.as_str()) {
                true => Some(v.iter().filter_map(|s| match s.parse::<u64>().unwrap() {
                    0 => None,
                    a => Some(a),
                })),
                false => None,
            })
            .flatten()
            .collect();

        let layers: Vec<_> = layer_addrs
            .iter()
            .filter_map(|&addr| match parse_meta_block(file, addr) {
                Ok(v) => v,
                Err(err) => todo!("Err ecountered parsing at {}\t{}", addr, err),
            })
            .collect();

        pages.push(metadata::PageMeta { page_info, layers });
    }

    Ok(pages)
}

/// Reads the a block of data at addr.
///
/// # Error
/// It will error when there's an [io::Error] reading the file or seeking the position.
///
/// # Returns
/// It returns a block
pub fn get_content_at_address(file: &mut File, addr: u64) -> io::Result<Vec<u8>> {
    if addr == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Read address was 0",
        ));
    }
    file.seek(SeekFrom::Start(addr))?;
    let block_size = file.read_u32::<LittleEndian>()?;
    // Could use Vec::with_capactiy and the unsafe set_len for possibly quicker
    // performance. But it's unsafe
    let mut data = vec![0; block_size as usize];
    file.read_exact(&mut data)?;
    Ok(data)
}

/// Will get the keyword (`key`) at the [MetaMap] and then read the content at that address from the `file` ([File]).
/// 
/// Turns all errors into [None].
pub fn extract_key_and_read(file: &mut File, meta: &MetaMap, key: &str) -> Option<Vec<u8>> {
    meta.get(key).and_then(|str_v| str_v[0].parse::<u64>().ok()).and_then(|addr| get_content_at_address(file, addr).ok())
}

// #######################################################################
// #######################################################################
// ########################### IMPLEMENTATIONS ###########################
// #######################################################################
// #######################################################################
    
impl metadata::Footer {
    pub fn from_file(file: &mut File) -> io::Result<Self> {
        // Parse the footer, it's address is on the last address of memory.
        file.seek(SeekFrom::End(-(f_fmt::ADDR_SIZE as i64)))?;
        let footer_addr = file.read_u32::<LittleEndian>()? as u64;

        // Might need to have more robust checks if there are no metadata found
        // at the address
        let footer = match parse_meta_block(file, footer_addr)? {
            Some(f) => f,
            None => return Err(io::ErrorKind::InvalidData.into()),
        };

        let keywords_meta = get_all_meta_on_keyword(file, &footer, f_fmt::Keyword::Keyword);

        let titles_meta = get_all_meta_on_keyword(file, &footer, f_fmt::Keyword::Title);

        let links_meta = get_all_meta_on_keyword(file, &footer, f_fmt::Keyword::Link);

        Ok(metadata::Footer::new(footer, keywords_meta, titles_meta, links_meta))
    }
}

impl metadata::Metadata {
    pub fn from_file(file: &mut File) -> io::Result<Self> {
        let version = match read_file_version(file)? {
            Some(v) => {
                if v > f_fmt::SUPPORTED_VERSION {
                    return Err(io::ErrorKind::InvalidInput.into());
                } else {
                    v
                }
            }
            None => return Err(io::ErrorKind::InvalidInput.into()),
        };

        let footer = metadata::Footer::from_file(file)?;

        // Series of unwraps, if reading the right file should be fine
        let header_addr: u64 = footer
            .get("FILE_FEATURE")
            .unwrap()
            .first()
            .unwrap()
            .parse()
            .unwrap();
        let header = match parse_meta_block(file, header_addr)? {
            Some(h) => h,
            None => return Err(io::ErrorKind::InvalidData.into()),
        };

        let page_addrs = match get_keyword_addresses(&footer.main, f_fmt::Keyword::Page) {
            Some(p) => p,
            None => return Err(io::ErrorKind::InvalidData.into()),
        };
        let pages = parse_pages(file, page_addrs)?;

        Ok(metadata::Metadata {
            version,
            footer,
            header,
            pages,
        })
    }
}

impl Notebook {
    pub fn from_file(file: &mut File) -> io::Result<Self> {
        let metadata = Metadata::from_file(file)?;
        let version = metadata.version;
        // let mut keywords = Keyword::get_vec_from_meta(&metadata, file);
        // todo!("Still need to work on the keywords");
        let titles = Title::get_vec_from_meta(&metadata, file)?;
        let links = Link::get_vec_from_meta(&metadata, file);
        let pages = Page::get_vec_from_meta(&metadata.pages, file);

        Ok(Notebook {
            metadata,
            version,
            // keywords,
            titles,
            links,
            pages,
        })
    }
}