#[macro_use]
mod macros;
mod io;
mod data_structures;
mod decoder;
mod exporter;

pub mod common {
    pub use crate::data_structures::file_format_consts as f_fmt;
    pub type PdfColor = [f64; 3];
}

pub mod error {
    pub use crate::decoder::DecoderError;
    pub use crate::data_structures::DataStructureError;
    pub use crate::exporter::PotraceError;
    pub use crate::data_structures::StrokeError;
}

mod ui;
pub use io::load;
pub use data_structures::Notebook;
pub use decoder::ColorMap;

/// Starts the EGUI App (default behaviour)
pub fn start_app() {
    let app = ui::MyApp::new();
    let _ = eframe::run_native(
        "Supernote Tool",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder { icon: Some(ui::icon::get_icon().into()), ..Default::default()  },
            ..Default::default()
        },
        Box::new(|_ctx| {
            Ok(Box::new(app))
        })
    );
}
