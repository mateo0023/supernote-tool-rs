use std::error::Error;
use std::cmp::*;

use std::collections::HashMap;
use serde::{Deserialize, Serialize};

use crate::data_structures::{Notebook, Title, TitleLevel};
use crate::decoder::ColorMap;
use crate::error::*;

pub struct MyApp {
    app_cache: AppCache,
    notebooks: Vec<Notebook>,
    titles: Vec<TitleHolder>,
    colormap: ColorMap,
    out_path: String,
    out_err: Option<String>,
}

#[derive(Default, Serialize, Deserialize)]
struct TitleHolder {
    file_name: String,
    /// List of titles in the file.
    titles: Vec<TitleEditor>,
}

#[derive(Default, Serialize, Deserialize)]
struct TitleEditor {
    title: String,
    #[serde(skip)]
    persis_id: Option<egui::Id>,
    /// The index that the title contains
    title_index: Option<usize>,
    #[serde(skip)]
    img_texture: Option<egui::TextureHandle>,
    level: i32,
    children: Option<Vec<TitleEditor>>,
    /// The coordinates in the page for the title.
    coords: [i32; 4],
    /// The page_id on the notebook.
    page_id: String,
}

/// Will hold the settings for all the notebooks.
/// 
/// Maps the [notebook_id](Notebook::file_id) to the [NotebookCache]
#[derive(Default, Serialize, Deserialize)]
pub struct AppCache {
    /// Maps between file_id and Title Cache
    notebooks: HashMap<String, HashMap<TitleCache, TitleCache>>
}

/// Will be used to store the relevant information
/// on the title. Will check for page_id and location
/// of the title only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TitleCache {
    /// The corrected title.
    title: String,
    /// The Page Id from the Notebook
    page_id: String,
    /// The coordinates of the title.
    coords: [i32; 4],
}

impl MyApp {
    pub fn new(notebook: Notebook, ctx: &egui::Context) -> Result<Self, Box<dyn Error>> {
        let titles = TitleHolder::from_notebook(&notebook, ctx);
        let app_cache = AppCache::from_note(&notebook)?;

        Ok(MyApp {
            notebooks: vec![notebook],
            titles: vec![titles],
            colormap: ColorMap::default(),
            out_path: "./test/out.pdf".to_string(),
            out_err: None,
            app_cache,
        })
    }

    pub fn add_notebook(&mut self, mut notebook: Notebook, ctx: &egui::Context) -> Result<(), Box<dyn Error>> {
        self.app_cache.load_or_add(&mut notebook)?;
        let new_titles = TitleHolder::from_notebook(&notebook, ctx);
        
        self.notebooks.push(notebook);
        self.titles.push(new_titles);

        Ok(())
    }

    /// Will update the titles and render the [notebook(s)](Self::notebooks)
    /// into a PDF (or PDFs).
    fn package_and_export(&mut self) {
        for holder in &self.titles {
            for (idx, title) in holder.titles.iter().enumerate() {
                let (id, title) = title.get_data();
                if let Some(id) = id {
                    self.notebooks[idx].update_title_at_idx(id, title);
                }
            }
        }
        for note in &self.notebooks {
            if let Err(e) = note.to_pdf_file(&self.colormap, &self.out_path) {
                self.out_err.get_or_insert("".to_string())
                    .push_str(format!("\n{}", e).as_str());
            }
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
            for holder in self.titles.iter_mut() {
                ui.label(&holder.file_name);
                for title in holder.titles.iter_mut() {
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
    pub fn from_notebook(notebook: &Notebook, ctx: &egui::Context) -> Self {
        let mut titles = TitleHolder::default();
        notebook.titles.iter().enumerate()
            .filter_map(|(idx, title)| {
                let page_id = notebook.get_page_id(title.page_index)?;
                TitleEditor::new(title, idx, &page_id, ctx)
            }.map(|te| (te, title.title_level)).ok()
            )
            .for_each(|(title, lvl)| titles.add_title(title, lvl));
        titles
    }

    pub fn add_title(&mut self, title: TitleEditor, lvl: TitleLevel) {
        if let TitleLevel::BlackBack = lvl {
            self.titles.push(title);
        } else {
            match self.titles.last_mut() {
                Some(t) => t.add_child(title, lvl),
                None => {
                    let mut t = TitleEditor::default();
                    t.add_child(title, lvl);
                    self.titles.push(t);
                },
            }
        }
    }
}

impl TitleEditor {
    pub fn new(title: &Title, idx: usize, page_id: &str, ctx: &egui::Context) -> Result<Self, DecoderError> {
        let texture = title.render_and_add(ctx)?;
        Ok(TitleEditor {
            title: title.name.clone(),
            persis_id: None,
            img_texture: Some(texture),
            title_index: Some(idx),
            level: title.title_level.into(),
            children: None,
            coords: title.coords,
            page_id: page_id.to_string(),
        })
    }

    pub fn get_data(&self) -> (Option<usize>, &str) {
        (self.title_index, &self.title)
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
                    ui.make_persistent_id(format!("{}_{}", self.level, self.title))
                );
                let mut text_boxes = vec![];

                egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
                    .show_header(ui, |ui| {
                        text_boxes.push((
                            ui.text_edit_singleline(&mut self.title),
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
                vec![(ui.text_edit_multiline(&mut self.title), self.img_texture.clone())]
            },
        }
    }
}

impl AppCache {
    pub fn from_note(notebook: &Notebook) -> Result<Self, Box<dyn Error>> {
        let title_cache = TitleCache::from_notebook(notebook)?;
        let mut map = HashMap::new();
        map.insert(notebook.file_id.clone(), title_cache);

        Ok(AppCache {
            notebooks: map,
        })
    }

    /// Either updates the [notebook](Notebook)'s titles or it 
    /// creates a cache for it.
    pub fn load_or_add(&mut self, notebook: &mut Notebook) -> Result<(), Box<dyn Error>> {
        match self.notebooks.get_mut(&notebook.file_id) {
            Some(cache) => {
                // Already had cache, update title.
                let updated_titles = TitleCache::from_notebook(notebook)?;
                cache.retain(|t, _| updated_titles.contains_key(t));
                for (k, v) in updated_titles {
                    // Update if necessary and add create the TitleEditor
                    match cache.get(&k) {
                        Some(v) => {
                            notebook.update_title_by_page(&v.page_id, v.coords, &v.title);
                        },
                        None => {
                            cache.insert(k, v);
                        },
                    }
                }
            },
            None => {
                // Generate cache data.
                let cached_titles = TitleCache::from_notebook(notebook)?;
                self.notebooks.insert(notebook.file_id.clone(), cached_titles);
            },
        }

        Ok(())
    }
}

impl TitleCache {
    pub fn from_notebook(notebook: &Notebook) -> Result<HashMap<Self, Self>, Box<dyn Error>> {
        let mut cached_titles = HashMap::new();
        for title in &notebook.titles {
            let title = TitleCache::new(title, notebook)?;
            cached_titles.insert(title.clone(), title);
        }
        Ok(cached_titles)
    }

    pub fn new(title: &Title, notebook: &Notebook) -> Result<Self, Box<dyn Error>> {
        let page_id = match notebook.get_page_id(title.page_index) {
            Some(id) => id,
            None => todo!("Create processing error"),
        };

        Ok(TitleCache {
            title: title.name.clone(),
            page_id,
            coords: title.coords,
        })
    }

    /// Checks wether they point to the same Title
    /// (page_id and coords)
    pub fn equals(&self, rhs: &Self) -> bool {
        self.page_id.eq(&rhs.page_id)
        && self.coords.eq(&rhs.coords)
    }
}

impl std::hash::Hash for TitleCache {
    /// Hash only the [page_id](Self::page_id) and [coordinates](Self::coords)
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.page_id.hash(state);
        self.coords.hash(state);
    }
}

impl From<TitleEditor> for TitleCache {
    fn from(value: TitleEditor) -> Self {
        TitleCache {
            title: value.title,
            page_id: value.page_id,
            coords: value.coords,
        }
    }
}
