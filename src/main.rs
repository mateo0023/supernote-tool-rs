use std::collections::HashMap;

mod io;
mod data_structures;


fn main() {
    let notebook = io::load("./test/v15.note").unwrap();

    println!("{:?}", notebook);
    // if let Ok(json) = serde_json::to_string_pretty(&meta) {
    //     println!("{}", &json);
    //     use std::fs::File;
    //     use std::io::Write;
    //     if let Ok(mut f) = File::create("./test/out.json") {
    //         let _ = f.write(json.as_bytes());
    //     }
    // }
}
