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

    let colormap = decoder::ColorMap::new(
        [0x00, 0x00, 0x00, 0xff],
        [0x46, 0x69, 0xd6, 0xff],
        [0xfd, 0xfa, 0x75, 0xff],
        [0xfe, 0xfe, 0xfe, 0xff]
    );

    let handle = exporter::to_pdf(&notebook, &colormap);
    if let Ok(mut pdf) = handle {
        let _ = pdf.save("./test/out.pdf");
    }

    // let app = ui::MyApp::new(notebook);
    // let _ = eframe::run_native("SuperNote Exporter", eframe::NativeOptions::default(), Box::new(|_ctx| Ok(Box::new(app))));
}
