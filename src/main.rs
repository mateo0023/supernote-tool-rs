#[cfg(feature = "test_mode")]
fn main() {
    supernote_tool_rs::big_test();
}

#[cfg(not(feature = "test_mode"))]
fn main() {
    supernote_tool_rs::start_app()
}
