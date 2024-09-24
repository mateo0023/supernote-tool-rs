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
}

mod ui;

fn main() {
    let notebook = io::load("./test/Test Doc.note").unwrap();

    let _ = eframe::run_native("SuperNote Exporter", eframe::NativeOptions::default(), Box::new(|ctx| {
        let app = ui::MyApp::new(notebook, &ctx.egui_ctx);
        Ok(Box::new(app))
    }));
}
