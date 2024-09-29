use std::error::Error;
use std::cmp::*;
use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use rfd::FileDialog;

use crate::data_structures::{Notebook, Title, TitleLevel};
use crate::decoder::ColorMap;
use crate::error::*;
use crate::exporter::export_multiple;
use crate::io::to_file;

const SETTINGS_PATH: &str = "./test/config.json";

pub struct MyApp {
    app_cache: AppCache,
    notebooks: Vec<(Notebook, TitleHolder)>,
    colormap: ColorMap,
    out_folder: Option<PathBuf>,
    out_err: Option<Vec<Box<dyn Error>>>,
    out_name: String,
}

#[derive(Default)]
struct TitleHolder {
    file_id: String,
    file_name: String,
    /// List of titles in the file.
    titles: Vec<TitleEditor>,
}

#[derive(Default)]
pub struct TitleEditor {
    title: String,
    persis_id: Option<egui::Id>,
    /// The index that the title contains
    title_index: Option<usize>,
    img_texture: Option<egui::TextureHandle>,
    level: i32,
    children: Option<Vec<TitleEditor>>,
    /// The hash value of the content (encoded).
    hash: u64,
    /// The page_id on the notebook.
    page_id: String,
}

/// Will hold the settings for all the notebooks.
/// 
/// Maps the [notebook_id](Notebook::file_id) to the [NotebookCache]
#[derive(Default, Serialize, Deserialize)]
pub struct AppCache {
    /// Maps between file_id and Title Cache
    notebooks: HashMap<String, Vec<TitleCache>>,
    /// Wether to combina all the [Notebook]s into 
    /// a single pdf or export them separately.
    combine_pdfs: bool,
}

/// Will be used to store the relevant information
/// on the title. Will check for page_id and location
/// of the title only.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TitleCache {
    /// The corrected title.
    title: Option<String>,
    /// The Page Id from the Notebook
    page_id: String,
    /// The hash value of the [content](Title::content).
    hash: u64,
}

/// Loads the as a texture with the given context and returns the [TextureHandle](egui::TextureHandle)
/// or [DecoderError].
pub fn add_image(bitmap: &[u8], width: usize, height: usize, hash: u64, ctx: &egui::Context)
    -> Result<egui::TextureHandle, DecoderError>
{
    let image = egui::ColorImage::from_rgba_unmultiplied([width, height], bitmap);
    Ok(ctx.load_texture(format!("title#{}", hash), image, egui::TextureOptions::default()))
}

impl MyApp {
    pub fn new() -> Self {
        MyApp {
            notebooks: vec![],
            colormap: ColorMap::default(),
            out_folder: None,
            out_err: None,
            app_cache: AppCache::load_or_default(),
            out_name: String::new(),
        }
    }

    fn add_err(&mut self, e: Box<dyn Error>) {
        self.out_err.get_or_insert(vec![]).push(e);
    }

    pub fn add_notebook(&mut self, mut notebook: Notebook, ctx: &egui::Context) -> Result<(), Box<dyn Error>> {
        self.app_cache.load_or_add(&mut notebook)?;
        let new_titles = TitleHolder::from_notebook(&notebook, ctx);
        
        self.notebooks.push((notebook, new_titles));
        self.notebooks.sort_by_cached_key(|n| n.0.file_name.clone());

        let mut page = 0;
        for (n, _) in self.notebooks.iter_mut() {
            n.starting_page = page;
            page += n.pages.len();
        }

        Ok(())
    }

    /// Will update the titles and render the [notebook(s)](Self::notebooks)
    /// into a PDF (or PDFs).
    fn package_and_export(&mut self) -> Result<(), Box<dyn Error>> {
        self.update_cache();
        self.process_result(self.app_cache.save());

        for (notebook, holder) in self.notebooks.iter_mut() {
            for title in holder.titles.iter() {
                let (id, name) = title.get_data();
                if let Some(id) = id {
                    notebook.update_title_at_idx(id, name);
                }
            }
        }

        if self.notebooks.len() < 2 || !self.app_cache.combine_pdfs {
            for (note, _) in &self.notebooks {
                if let Some(path) = &self.out_folder {
                    if let Err(e) = to_file(note.to_pdf(&self.colormap)?, path, &note.file_name) {
                        self.out_err.get_or_insert(vec![])
                            .push(e);
                    }
                }
            }
        } else if let Some(path) = &self.out_folder {
            self.process_result(
                to_file(
                    export_multiple(
                        &self.notebooks.iter().map(|(n, _)| n).collect::<Vec<_>>(),
                        &self.colormap
                    )?, path, &self.out_name
                )
            );
        }
        
        Ok(())
    }

    fn update_cache(&mut self) {
        for (_, holder) in &self.notebooks {
            let (k, v) = holder.as_list();
            self.app_cache.update(k, v);
        }
    }

    fn process_result<T>(&mut self, r: Result<T, Box<dyn Error>>) -> Option<T> {
        match r {
            Ok(o) => Some(o),
            Err(e) => {
                self.add_err(e);
                None
            },
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Select File").clicked() {
                    if let Some(path_list) = FileDialog::new().add_filter("Supernote File", &["note"]).pick_files() {
                        for file_p in path_list {
                            self.process_result(
                                crate::io::load(file_p)
                            ).and_then(|n| {
                                let r = self.add_notebook(n, ctx);
                                self.process_result(r)
                            });
                        }
                    }
                }
    
                match self.out_folder.is_some() {
                    true => {
                        if !self.notebooks.is_empty() && ui.button("Export to PDF").clicked() {
                            if let Err(e) = self.package_and_export() {
                                self.add_err(e);
                            }
                        }
                    },
                    false => if ui.button("Select OutPut Folder").clicked() {
                        self.out_folder = FileDialog::new().pick_folder();
                    },
                }


                if !self.notebooks.is_empty() && ui.button("Close Notebooks").clicked() {
                    self.notebooks.clear();
                }
            });

            if self.notebooks.len() > 1 {
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.app_cache.combine_pdfs, "Combine Notebooks?");
                    if self.app_cache.combine_pdfs {
                        ui.text_edit_singleline(&mut self.out_name);
                    }
                });
            }

            if self.out_err.is_some() && ui.button("Clear Errors").clicked() {
                self.out_err = None;
            }
            if let Some(e) = &self.out_err {
                ui.collapsing("Errors: ", |ui| {
                    for err in e.iter() {
                        ui.label(err.to_string());
                    }
                });
            }

            let mut title_bx = vec![];
            for (_, holder) in self.notebooks.iter_mut() {
                ui.collapsing(holder.file_name.clone(), |ui| {
                    for title in holder.titles.iter_mut() {
                        title_bx.extend(title.show(ui));
                    }
                });
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

impl TitleHolder {
    pub fn from_notebook(notebook: &Notebook, ctx: &egui::Context) -> Self {
        let mut titles = TitleHolder {
            file_id: notebook.file_id.clone(),
            file_name: notebook.file_name.clone(),
            titles: vec![],
        };
        notebook.titles.iter().enumerate()
            .filter_map(|(idx, title)| {
                let page_id = notebook.get_page_id_from_internal(title.page_index)?;
                TitleEditor::new(title, idx, &page_id, ctx)
            }.map(|te| (te, title.title_level)).ok()
            )
            .for_each(|(title, lvl)| titles.add_title(title, lvl));
        titles
    }

    pub fn as_list(&self) -> (String, Vec<TitleCache>) {
        let list = self.titles.iter().flat_map(|t| t.as_cache_list()).collect();
        (self.file_id.clone(), list)
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
        let bitmap = title.render_bitmap()?;
        let texture = add_image(&bitmap, title.width, title.height, title.content_hash, ctx)?;
        Ok(TitleEditor {
            title: title.get_name(),
            persis_id: None,
            img_texture: Some(texture),
            title_index: Some(idx),
            level: title.title_level.into(),
            children: None,
            hash: title.content_hash,
            page_id: page_id.to_string(),
        })
    }

    pub fn get_data(&self) -> (Option<usize>, &str) {
        (self.title_index, &self.title)
    }

    pub fn add_child(&mut self, title: TitleEditor, lvl: TitleLevel) {
        if self.level + 1 == Into::<i32>::into(lvl) {
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

    pub fn as_cache_list(&self) -> Vec<TitleCache> {
        match &self.children {
            Some(ch) => {
                let mut c: Vec<_> = ch.iter().flat_map(|t| t.as_cache_list()).collect();
                c.push(self.as_single_cache());
                c
            },
            None => vec![self.as_single_cache()],
        }
    }

    fn as_single_cache(&self) -> TitleCache {
        TitleCache {
            title: match self.title.is_empty() {
                true => None,
                false => Some(self.title.clone()),
            },
            page_id: self.page_id.clone(),
            hash: self.hash,
        }
    }

    /// Renders all the titles as [CollapsingHeader](egui::CollapsingHeader)
    /// 
    /// If no [children](Self::children), simply render a [TextEdit](egui::TextEdit)
    pub fn show(&mut self, ui: &mut egui::Ui) -> Vec<(egui::Response, Option<egui::TextureHandle>)> {
        match &mut self.children {
            Some(children) => {
                let &mut id = self.persis_id.get_or_insert(
                    ui.make_persistent_id(format!("collapsing#{}", self.hash))
                );
                let mut text_boxes = vec![];

                egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
                    .show_header(ui, |ui| {
                        text_boxes.push((Self::text_edit(&mut self.title, ui), self.img_texture.clone()));
                    })
                    .body(|ui| {
                        text_boxes.extend(children.iter_mut().flat_map(|t| t.show(ui)));
                    });

                text_boxes
            },
            None => {
                // Simply add text box
                vec![(Self::text_edit(&mut self.title, ui), self.img_texture.clone())]
            },
        }
    }

    fn text_edit(title: &mut String, ui: &mut egui::Ui) -> egui::Response {
        ui.horizontal(|ui| {
            ui.text_edit_singleline(title)
        }).inner
    }
}

impl AppCache {
    pub fn load_or_default() -> Self {
        match std::fs::File::open(SETTINGS_PATH) {
            Ok(f) => match serde_json::from_reader(f) {
                Ok(cache) => cache,
                Err(_) => Default::default(),
            },
            Err(_) => Default::default(),
        }
    }

    /// Replaces the Cache data at the key ([file_id](Notebook::file_id) by the new
    /// [TitleCache]
    pub fn update(&mut self, k: String, v: Vec<TitleCache>) {
        self.notebooks.insert(k, v);
    }

    /// Either updates the [notebook](Notebook)'s titles or it 
    /// creates a cache for the notebook.
    pub fn load_or_add(&mut self, notebook: &mut Notebook) -> Result<(), Box<dyn Error>> {
        match self.notebooks.get_mut(&notebook.file_id) {
            Some(cache) => {
                // Already had cache, update title.
                let old_cache = {
                    let mut on_file_titles = TitleCache::from_notebook(notebook)?;
                    std::mem::swap(cache, &mut on_file_titles);
                    on_file_titles
                    
                };
                for c in old_cache {
                    for (i, other) in cache.iter_mut().enumerate() {
                        if other.equals(&c) {
                            if let Some(name) = c.title.as_ref() {
                                notebook.update_title_at_idx(i, name);
                                other.title = c.title;
                            }
                            break;
                        }
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

    pub fn save(&self) -> Result<(), Box<dyn Error>> {
        let f = std::fs::File::create(SETTINGS_PATH)?;
        serde_json::to_writer_pretty(f, self)?;
        Ok(())
    }
}

impl TitleCache {
    pub fn from_notebook(notebook: &Notebook) -> Result<Vec<Self>, Box<dyn Error>> {
        // let mut cached_titles = HashMap::new();
        let mut ls = vec![];
        for title in &notebook.titles {
            let title = TitleCache::new(title, notebook)?;
            // cached_titles.insert(title.clone(), title);
            ls.push(title);
        }
        // Ok(cached_titles)
        Ok(ls)
    }

    pub fn new(title: &Title, notebook: &Notebook) -> Result<Self, Box<dyn Error>> {
        let page_id = match notebook.get_page_id_from_internal(title.page_index) {
            Some(id) => id,
            None => todo!("Create processing error"),
        };

        Ok(TitleCache {
            title: title.name.clone(),
            page_id,
            hash: title.content_hash,
        })
    }

    /// Checks wether they point to the same [Title]
    /// (page_id and hash)
    pub fn equals(&self, rhs: &Self) -> bool {
        self.page_id.eq(&rhs.page_id)
        && self.hash.eq(&rhs.hash)
    }
}

impl std::hash::Hash for TitleCache {
    /// Hash only the [page_id](Self::page_id) and [hash](Self::coords)
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.page_id.hash(state);
        self.hash.hash(state);
    }
}

