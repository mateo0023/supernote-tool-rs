use crate::data_structures::Notebook;

// #[derive(Debug)]
pub struct MyApp {
    notebooks: Notebook,
    cache_images: Option<Vec<TempImageHolder>>,//Result<vtracer::SvgFile, String>>>,
    page_to_load: usize,
}

use crate::error::*;

enum TempImageHolder {
    Image(egui::TextureHandle),
    Error(DecoderError)
}

impl MyApp {
    pub fn new(notebooks: Notebook) -> Self {
        MyApp {
            notebooks,
            cache_images: None,
            page_to_load: 0,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.label(format!("Notebook Loaded with {} pages", self.notebooks.pages.len()));

            ui.horizontal(|ui| {
                if ui.add(egui::Slider::new(&mut self.page_to_load, 0..=(self.notebooks.pages.len()-1))).changed() {
                    self.cache_images = None;
                }
                if ui.button("Render").clicked() {

                    let img_handles = crate::exporter::page_to_svg(&self.notebooks.pages[self.page_to_load], &crate::decoder::ColorMap::default())
                        .into_iter().enumerate().map(|(idx, image_data)| {
                            match image_data {
                                Ok(data) => {
                                    use crate::data_structures::file_format_consts::*;
                                    let image = egui::ColorImage::from_rgba_unmultiplied([PAGE_WIDTH, PAGE_HEIGHT], &data);
                                    // {
                                    //     let mut it = data.iter();
                                    //     while let (Some(&r), Some(&g), Some(&b), Some(&a)) = (it.next(), it.next(), it.next(), it.next()){
                                    //         if a != 0 || r != 255 || g != 255 || b != 255 {
                                    //             println!("Color is not white ({:#04x},{:#04x},{:#04x} {:#04x})", r, g, b, a);
                                    //             continue;
                                    //         }
                                    //     }
                                    // }
                                    // let image = egui::ColorImage::example();
                                    TempImageHolder::Image(ctx.load_texture(format!("test{idx}"), image, egui::TextureOptions::default()))
                                },
                                Err(err) => TempImageHolder::Error(err),
                            }
                        }).collect();


                    self.cache_images = Some(img_handles);
                }
                if ui.button("Export SVG").clicked() {
                    let imgages = crate::exporter::page_to_svg(&self.notebooks.pages[self.page_to_load], &crate::decoder::ColorMap::default());
                    
                }
            });

            if let Some(images) = &self.cache_images {
                for (i, bytes) in images.iter().enumerate() {
                    match bytes {
                        TempImageHolder::Image(image) => {
                            ui.horizontal(|ui| {
                                ui.label("Adding Image");
                                ui.image(image);
                            });
                        },
                        TempImageHolder::Error(err) => {ui.label(format!("Layer {i} had error: {}", err));},
                    };
                }
            }
        });
    }
}