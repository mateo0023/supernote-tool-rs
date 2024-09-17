mod io;
mod data_structures;
mod decoder;
mod exporter;

mod error {
    pub use crate::decoder::DecoderError;
}

mod ui;

fn main() {
    let notebook = io::load("./test/v15.note").unwrap();

    let app = ui::MyApp::new(notebook);
    let _ = eframe::run_native("SuperNote Exporter", eframe::NativeOptions::default(), Box::new(|_ctx| Ok(Box::new(app))));
}
