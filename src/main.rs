mod io;
mod data_structures;
mod decoder;
mod exporter;

mod common {
    pub use crate::data_structures::file_format_consts as f_fmt;
    pub type PdfColor = [f64; 3];
}

mod error {
    pub use crate::decoder::DecoderError;
    pub use crate::exporter::PotraceError;
}

mod ui;

fn main() {
    let app = ui::MyApp::new();
    let _ = eframe::run_native("SuperNote Exporter", eframe::NativeOptions::default(), Box::new(|_ctx| {
        Ok(Box::new(app))
    }));
}
