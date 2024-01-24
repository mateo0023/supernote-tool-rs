use std::collections::HashMap;

fn main() {
    let meta = io::load("C:\\Users\\matab\\Downloads\\2023-10 Practice.note").unwrap();
    println!("{:?}", meta.items);
}

pub struct Metadata {
    items: HashMap<String, Vec<String>>,
}

mod io {
    //! Loads the data and metadata
    
    mod file_format {
       //! Contains the variables and data needed to read the *.note file.
       
       pub const SUPPORTED_VERSION: u32 = 20230015;
    }
    
    use std::collections::HashMap;
    use std::io::{self, prelude::*, SeekFrom};
    use std::fs::File;
    use crate::Metadata;

    /// Loads
    pub fn load(path: &str) -> io::Result<Metadata> {
        let mut file = File::open(path)?;

        let version = read_file_version(&mut file)?;

        if let Some(v) = version {
            if v > file_format::SUPPORTED_VERSION {
                return Err(io::ErrorKind::InvalidInput.into());
            }
        }

        Ok(Metadata { items: HashMap::new() })
    }

    fn read_file_version(file: &mut File) -> io::Result<Option<u32>> {
        file.seek(SeekFrom::Start(16))?;
        let mut buf = [0; 8];
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
}