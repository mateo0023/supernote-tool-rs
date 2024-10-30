fn main() {
    use supernote_tool_rs::*;
    let mut notebook = load(
        "./test/01. Asset Allocation.note".into(),
        &AppCache::default(),
        &ServerConfig::default()
    ).unwrap();
    notebook.to_pdf(&ColorMap::default()).unwrap();
}