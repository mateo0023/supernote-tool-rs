use crate::data_structures::Notebook;

// #[derive(Debug)]
pub struct MyApp {
    notebooks: Notebook,
    cache_images: Option<Vec<egui::TextureHandle>>,//Result<vtracer::SvgFile, String>>>,
    page_to_load: usize,
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
                            use crate::data_structures::file_format::*;
                            let image = egui::ColorImage::from_rgba_unmultiplied([PAGE_WIDTH, PAGE_HEIGHT], &image_data);
                            // let image = egui::ColorImage::example();
                            ctx.load_texture(format!("test{idx}"), image, egui::TextureOptions::default())
                        }).collect();


                    self.cache_images = Some(img_handles);
                }
            });

            if let Some(images) = &self.cache_images {
                for (i, bytes) in images.iter().enumerate() {
                    ui.image(bytes);
                    // match render_result {
                    //     Ok(bytes) => {ui.image(egui::ImageSource::from((format!("bytes::/img{i}.svg"), bytes.clone())));},
                    //     Err(err) => {ui.label(format!("Encountered the following errors when loading Layer {i}:\t{}", err));},
                    // }
                    
                }
            }
        });
    }
}