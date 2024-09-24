use crate::data_structures::{Notebook, Title};
use crate::decoder::ColorMap;
use crate::error::*;

// #[derive(Debug)]
pub struct MyApp {
    notebooks: Notebook,
    titles: Vec<TitleHolder>,
    colormap: ColorMap,
    out_path: String
}

struct TitleHolder {
    t: String,
    title_id: usize,
    img_texture: egui::TextureHandle,
}

impl MyApp {
    pub fn new(notebooks: Notebook, ctx: &egui::Context) -> Self {
        let titles = notebooks.titles.iter().enumerate().filter_map(|(i, title)| TitleHolder::new(title, i, ctx).ok()).collect();
        MyApp {
            notebooks,
            titles,
            colormap: ColorMap::default(),
            out_path: "./test/out.pdf".to_string(),
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if ui.button("Export to PDF").clicked() {
                self.notebooks.to_pdf_file(&self.colormap, &self.out_path);
            }

            ui.label(format!("Notebook Loaded with {} pages", self.notebooks.pages.len()));
            for title in &mut self.titles {
                ui.horizontal(|ui| {
                    ui.image(&title.img_texture);
                    if ui.text_edit_singleline(&mut title.t).changed() {
                        self.notebooks.update_title(title.title_id, &title.t);
                    }
                });
            }
        });
    }
}

impl Title {
    pub fn render_and_add(&self, ctx: &egui::Context) -> Result<egui::TextureHandle, DecoderError> {
        let bitmap = self.render_bitmap()?;
        let image = egui::ColorImage::from_rgba_unmultiplied([self.width, self.height], &bitmap);
        Ok(ctx.load_texture(format!("title#{}", self.metadata.get("TITLEBITMAP").unwrap()[0]), image, egui::TextureOptions::default()))
    }
}

impl TitleHolder {
    pub fn new(title: &Title, idx: usize, ctx: &egui::Context) -> Result<Self, DecoderError> {
        Ok(TitleHolder {
            t: title.name.clone(),
            img_texture: title.render_and_add(ctx)?,
            title_id: idx,
        })
    }
}
