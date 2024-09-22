use crate::data_structures::Notebook;

// #[derive(Debug)]
pub struct MyApp {
    notebooks: Notebook,
    cache_image: Option<TempImageHolder>,//Result<vtracer::SvgFile, String>>>,
    page_to_load: usize,
}

use crate::error::*;

enum TempImageHolder {
    Image(egui::TextureHandle),
    Error(Vec<DecoderError>)
}

impl MyApp {
    pub fn new(notebooks: Notebook) -> Self {
        MyApp {
            notebooks,
            cache_image: None,
            page_to_load: 0,
        }
    }

    fn generate_cache(&mut self, ctx: &egui::Context) {
        let img_handle = match crate::exporter::get_bitmap(&self.notebooks.pages[self.page_to_load], &crate::decoder::ColorMap::default()) {
            Ok(data) => {
                use crate::data_structures::file_format_consts::*;
                let image = egui::ColorImage::from_rgba_unmultiplied([PAGE_WIDTH, PAGE_HEIGHT], &data);
                TempImageHolder::Image(ctx.load_texture(format!("page#{}", self.page_to_load+1), image, egui::TextureOptions::default()))
            },
            Err(err) => TempImageHolder::Error(err),
        };
        self.cache_image = Some(img_handle);
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(format!("Notebook Loaded with {} pages", self.notebooks.pages.len()));

            ui.horizontal(|ui| {
                if ui.add(egui::Slider::new(&mut self.page_to_load, 0..=(self.notebooks.pages.len()-1))).changed() {
                    self.generate_cache(ctx);
                }

                if ui.button("Export SVG").clicked() {
                    let image = crate::exporter::page_to_svg(&self.notebooks.pages[self.page_to_load], &crate::decoder::ColorMap::default());
                    match image {
                        Ok(svg) => {
                            todo!("We have operations {:?}", svg);
                        },
                        Err(err) => todo!("{err}"),
                    }
                }
            });

            match &self.cache_image {
                Some(result) => match result {
                    TempImageHolder::Image(image) => {
                        ui.image(image);
                    },
                    TempImageHolder::Error(err) => {ui.label(format!("Page {} had error: {:?}", self.page_to_load, err));},
                },
                None => self.generate_cache(ctx),
            }
        });
    }
}