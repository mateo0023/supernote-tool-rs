use crate::data_structures::{Notebook, Title};
use crate::decoder::ColorMap;
use crate::error::*;

pub struct MyApp {
    notebooks: Notebook,
    titles: Vec<TitleHolder>,
    colormap: ColorMap,
    out_path: String,
    out_err: Option<String>,
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
            out_err: None,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if ui.button("Export to PDF").clicked() {
                if let Err(e) = self.notebooks.to_pdf_file(&self.colormap, &self.out_path) {
                    self.out_err = Some(e);
                }
            }

            if let Some(e) = &self.out_err {
                ui.label(format!("Failed to save. Error: {}", e));
            }

            let mut title_bx = vec![];
            for title in &mut self.titles {
                let txt_box = ui.text_edit_singleline(&mut title.t);
                if txt_box.changed() {
                    self.notebooks.update_title(title.title_id, &title.t);
                }
                title_bx.push((txt_box, title.img_texture.clone()));
            }

            if let Some((txt_box, texture)) = title_bx.iter().find(|(it, _)| it.has_focus()).or(title_bx.iter().find(|(i, _)| i.hovered())) {
                let max_width = ctx.input(|i: &egui::InputState| i.screen_rect()).width() - txt_box.rect.right();
                
                egui::Window::new("Image")
                .vscroll(false)
                .current_pos(txt_box.rect.right_top())
                .show(ctx, |ui| {
                    ui.add(
                        egui::Image::from_texture(texture)
                        .maintain_aspect_ratio(true)
                        .max_width(max_width)
                    );
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
