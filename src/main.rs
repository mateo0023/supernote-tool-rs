mod io;
mod data_structures;
mod decoder;
mod exporter {
    use crate::data_structures::*;
    use crate::decoder::{decode_data, ColorMap};

    
    pub fn export(notebook: &Notebook, map: &ColorMap) -> () {
        
    }
    
    pub fn page_to_svg(page: &Page, colormap: &ColorMap) -> Vec<Vec<u8>> {
        use file_format::{PAGE_HEIGHT, PAGE_WIDTH};
        use vtracer::{ColorImage, convert, Config};

        let layers: Vec<_> = page.layers.iter()
            .filter(|l| l.is_background())
            .filter_map(|l| l.content.as_ref())
            .map(|data| crate::decoder::decode_data(data, colormap))
            // .map(|bitmap| {
            //     let img = ColorImage {
            //         pixels: bitmap,
            //         width: PAGE_WIDTH,
            //         height: PAGE_HEIGHT,
            //     };
            //     let config = Config::default();
            //     convert(img, config)
            // })
            .collect();

        layers
    }
}

mod ui;

fn main() {
    let notebook = io::load("./test/v15.note").unwrap();

    let app = ui::MyApp::new(notebook);
    let _ = eframe::run_native("SuperNote Exporter", eframe::NativeOptions::default(), Box::new(|_ctx| Ok(Box::new(app))));
}
