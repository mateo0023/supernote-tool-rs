use std::collections::HashMap;

mod io;
mod data_structures;

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
