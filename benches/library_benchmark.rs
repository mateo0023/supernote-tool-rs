fn main() {
    use supernote_tool_rs::*;
    let notebook = load("./test/01. Asset Allocation.note".into()).unwrap();
    notebook.to_pdf(&ColorMap::default()).unwrap();
}