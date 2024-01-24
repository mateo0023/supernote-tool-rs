//! Loads the data and metadata

use std::io::{self, prelude::*, SeekFrom};
use std::fs::File;

use byteorder::{LittleEndian, ReadBytesExt};
use regex::Regex;

use crate::{Metadata, Footer, MetaMap, PageMeta};

mod f_fmt {
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
}

const LAYER_KEYS: [&str; 5] = ["MAINLAYER", "LAYER1", "LAYER2", "LAYER3", "BGLAYER"];

/// Loads
pub fn load(path: &str) -> io::Result<Metadata> {
    let mut file = File::open(path)?;

    let version = match read_file_version(&mut file)? {
        Some(v) => if v > f_fmt::SUPPORTED_VERSION {
            return Err(io::ErrorKind::InvalidInput.into());
        } else {
            v
        },
        None => return Err(io::ErrorKind::InvalidInput.into()),
    };

    // Parse the footer, it's address is on the last address of memory.
    file.seek(SeekFrom::End(-(f_fmt::ADDR_SIZE as i64)))?;
    let footer_addr = file.read_u32::<LittleEndian>()? as u64;
    
    // Might need to have more robust checks if there are no metadata found
    // at the address
    let footer = match parse_meta_block(&mut file, footer_addr)? {
        Some(f) => f,
        None => return Err(io::ErrorKind::InvalidData.into()),
    };

    let keywords_meta = get_all_meta_on_keyword(&mut file, &footer, "KEYWORD_");

    let titles_meta = get_all_meta_on_keyword(&mut file, &footer, "TITLE_");

    let links_meta = get_all_meta_on_keyword(&mut file, &footer, "LINK");

    let footer = Footer::new(footer, keywords_meta, titles_meta, links_meta);

    // Series of unwraps, if reading the right file should be fine
    let header_addr: u64 = footer.get("FILE_FEATURE").unwrap().first().unwrap().parse().unwrap();
    let header = match parse_meta_block(&mut file, header_addr)? {
        Some(h) => h,
        None => return Err(io::ErrorKind::InvalidData.into()),
    };

    let page_addrs = match get_keyword_addresses(&footer.main, "PAGE"){
        Some(p) => p,
        None => return Err(io::ErrorKind::InvalidData.into()),
    };
    let pages = parse_pages(&mut file, page_addrs)?;

    Ok(Metadata { version, footer, header, pages })
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
        Err(err) => todo!("Found error when parsing version number at start of file {:?}", err),
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
    file.seek(SeekFrom::Start(addr))?;
    let block_size = file.read_u32::<LittleEndian>()?;
    // Could use Vec::with_capactiy and the unsafe set_len for possibly quicker
    // performance. But it's unsafe
    let mut meta = vec![0; block_size as usize];
    file.read_exact(&mut meta)?;
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
            map.entry(key).and_modify(|list| list.push(value.to_string())).or_insert(vec![value.to_string()]);
        }
    }

    match map.is_empty() {
        true => Ok(None),
        false => Ok(Some(map)),
    }
}

/// Loops through the entries that begin with `keyword` and converts the string 
/// value into addresses. Collecting all of them into a single vector of [`AddrType`](f_fmt::AddrType)
fn get_keyword_addresses(metadata: &MetaMap, keyword: &str) -> Option<Vec<f_fmt::AddrType>> {
    let addresses: Vec<f_fmt::AddrType> = metadata.iter().filter_map(|(k, v)| {
        if k.starts_with(keyword) {
            Some(v.iter().map(|n| match n.parse::<f_fmt::AddrType>() {
                Ok(num) => num,
                Err(_) => todo!(),
            }))
        } else {
            None
        }
    }).flatten().collect();

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
fn parse_addresses_to_meta(file: &mut File, k_addrs: Vec<f_fmt::AddrType>) -> Vec<MetaMap> {
    k_addrs.iter().filter_map(|&addr| {
        parse_meta_block(file, addr as u64).unwrap_or(None)
    }).collect()
}

/// Does what it says
fn get_all_meta_on_keyword(file: &mut File, meta: &MetaMap, keyword: &str) -> Option<Vec<MetaMap>> {
    get_keyword_addresses(meta, keyword)
        .map(|k_addrs| parse_addresses_to_meta(file, k_addrs))
}

/// Goes through the page addresses getting their metadata and layer information
fn parse_pages(file: &mut File, addrs: Vec<f_fmt::AddrType>) -> io::Result<Vec<PageMeta>> {
    let mut pages = Vec::with_capacity(addrs.len());
    for addr in addrs {
        let page_info = parse_meta_block(file, addr as u64)?.unwrap();

        let layer_addrs: Vec<_> = page_info.iter().filter_map(|(k, v)| {
            match LAYER_KEYS.contains(&k.as_str()) {
                true => {
                    Some(v.iter().filter_map(|s| match s.parse::<u64>().unwrap() {
                        0 => None,
                        a => Some(a),
                    }))},
                false => None,
            }
        }).flatten().collect();

        let layers: Vec<_> = layer_addrs.iter().filter_map(|&addr| match parse_meta_block(file, addr) {
            Ok(v) => v,
            Err(err) => todo!("Err ecountered parsing at {}\t{}", addr, err),
        }).collect();

        pages.push(PageMeta {
            page_info,
            layers,
        });
    }

    Ok(pages)
}

// #######################################################################
// #######################################################################
// ########################### IMPLEMENTATIONS ###########################
// #######################################################################
// #######################################################################

impl Footer {
    pub fn from_file(file: &mut File) -> io::Result<Option<Self>> {
        let mut addr_buf = [0; f_fmt::ADDR_SIZE as usize];
        // Parse the footer, it's address is on the last address of memory.
        file.seek(SeekFrom::End(-(f_fmt::ADDR_SIZE as i64)))?;
        file.read_exact(&mut addr_buf)?;
        let footer_addr = match std::str::from_utf8(&addr_buf).unwrap().parse() {
            Ok(n) => n,
            Err(e) => todo!("Couldn't parse the footer address due to: {}", e),
        };
        let footer = parse_meta_block(file, footer_addr)?;
        
        Ok(footer.map(|footer| {
            let keywords_meta = get_all_meta_on_keyword(file, &footer, "KEYWORD_");
            let titles_meta = get_all_meta_on_keyword(file, &footer, "TITLE_");
            let links_meta = get_all_meta_on_keyword(file, &footer, "LINK");

            Footer { main: footer, keywords: keywords_meta, titles: titles_meta, links: links_meta }
        }))
    }
}