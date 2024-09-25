use serde::{Deserialize, Serialize};

use crate::data_structures::{Notebook, Title, TitleLevel};
use crate::decoder::ColorMap;
use crate::error::*;

pub struct MyApp {
    notebooks: Notebook,
    titles: TitleHolder,
    colormap: ColorMap,
    out_path: String,
    out_err: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct TitleHolder {
    titles: Option<Vec<TitleEditor>>,
}

#[derive(Default, Serialize, Deserialize)]
struct TitleEditor {
    t: String,
    #[serde(skip)]
    persis_id: Option<egui::Id>,
    title_id: Option<usize>,
    #[serde(skip)]
    img_texture: Option<egui::TextureHandle>,
    level: i32,
    children: Option<Vec<TitleEditor>>,
}

impl MyApp {
    pub fn new(notebooks: Notebook, ctx: &egui::Context) -> Self {
        let mut titles = TitleHolder::default();
        notebooks.titles.iter().enumerate()
            .filter_map(|(i, title)| TitleEditor::new(title, i, ctx)
                .map(|te| (te, title.title_level)).ok()
            )
            .for_each(|(title, lvl)| titles.add_title(title, lvl));

        MyApp {
            notebooks,
            titles,
            colormap: ColorMap::default(),
            out_path: "./test/out.pdf".to_string(),
            out_err: None,
        }
    }

    /// Will update the titles and render the [notebook(s)](Self::notebooks)
    /// into a PDF (or PDFs).
    fn package_and_export(&mut self) {
        if let Some(titles) = &self.titles.titles {
            for title in titles {
                let (id, title) = title.get_data();
                if let Some(id) = id {
                    self.notebooks.update_title(id, title);
                }
            }
        }
        if let Err(e) = self.notebooks.to_pdf_file(&self.colormap, &self.out_path) {
            self.out_err = Some(e);
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if ui.button("Export to PDF").clicked() {
                self.package_and_export();
            }

            if let Some(e) = &self.out_err {
                ui.label(format!("Failed to save. Error: {}", e));
            }

            let mut title_bx = vec![];
            if let Some(titles) = self.titles.titles.as_mut() {
                for title in titles {
                    title_bx.extend(title.show(ui));
                }
            }

            if let Some((txt_box, Some(texture))) = title_bx.iter().find(|(it, _)| it.has_focus()).or(title_bx.iter().find(|(i, _)| i.hovered())) {
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
    pub fn add_title(&mut self, title: TitleEditor, lvl: TitleLevel) {
        self.titles = if let TitleLevel::BlackBack = lvl {
            let mut t = self.titles.take().unwrap_or_default();
            t.push(title);
            Some(t)
        } else {
            let mut t = self.titles.take().unwrap_or(vec![TitleEditor::default()]);
            t.last_mut().unwrap().add_child(title, lvl);
            Some(t)
        };
    }
}

impl TitleEditor {
    pub fn new(title: &Title, idx: usize, ctx: &egui::Context) -> Result<Self, DecoderError> {
        let texture = title.render_and_add(ctx)?;
        Ok(TitleEditor {
            t: title.name.clone(),
            persis_id: None,
            img_texture: Some(texture),
            title_id: Some(idx),
            level: title.title_level.into(),
            children: None,
        })
    }

    pub fn get_data(&self) -> (Option<usize>, &str) {
        (self.title_id, &self.t)
    }

    pub fn add_child(&mut self, title: TitleEditor, lvl: TitleLevel) {
        if self.level + 1 == lvl.into() {
            // Reached the correct level
            let ch = self.children.get_or_insert(vec![]);
            ch.push(title);
        } else {
            // Need to go one level down
            // Create a default (empty title)
            let ch = self.children.get_or_insert(vec![TitleEditor::default()]);
            ch.last_mut().unwrap().add_child(title, lvl);
        }
    }

    /// Renders all the titles as [CollapsingHeader](egui::CollapsingHeader)
    /// 
    /// If no [children](Self::children), simply render a [](egui::TextBox)
    pub fn show(&mut self, ui: &mut egui::Ui) -> Vec<(egui::Response, Option<egui::TextureHandle>)> {
        match &mut self.children {
            Some(children) => {
                let &mut id = self.persis_id.get_or_insert(
                    ui.make_persistent_id(format!("{}_{}", self.level, self.t))
                );
                let mut text_boxes = vec![];

                egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
                    .show_header(ui, |ui| {
                        text_boxes.push((
                            ui.text_edit_singleline(&mut self.t),
                            self.img_texture.clone()
                        ));
                    })
                    .body(|ui| {
                        text_boxes.extend(children.iter_mut().flat_map(|t| t.show(ui)));
                    });

                text_boxes
            },
            None => {
                // Simply add 
                vec![(ui.text_edit_multiline(&mut self.t), self.img_texture.clone())]
            },
        }
    }
}
