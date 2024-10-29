use std::error::Error;
use std::path::PathBuf;

use rfd::FileDialog;

use crate::data_structures::{Notebook, Title, TitleLevel, Transciption, ServerConfig};
use crate::decoder::ColorMap;
use crate::error::*;
use crate::exporter::export_multiple;
use crate::io::to_file;
use crate::data_structures::cache::*;

pub mod icon;

pub struct MyApp {
    app_cache: AppCache,
    server_config: ServerConfig,
    notebooks: Vec<(Notebook, TitleHolder)>,
    colormap: ColorMap,
    out_folder: Option<PathBuf>,
    out_err: Option<Vec<Box<dyn Error>>>,
    out_name: String,
    settings_path: Option<PathBuf>,
    show_only_empty: bool,
    focused_id: Option<egui::Id>,
}

#[derive(Default)]
struct TitleHolder {
    file_id: String,
    file_name: String,
    /// List of titles in the file.
    titles: Vec<TitleEditor>,
}

pub struct TitleEditor {
    title: String,
    persis_id: egui::Id,
    img_texture: Option<egui::TextureHandle>,
    level: TitleLevel,
    children: Option<Vec<TitleEditor>>,
    /// The hash value of the content (encoded).
    hash: u64,
    /// The page_id on the notebook.
    page_id: String,
    /// Whether it was edited by the user, ever (it was in Cache).
    was_edited: bool,
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
            server_config: ServerConfig::from_path_or_default("./my_script_keys.json"),
            colormap: ColorMap::default(),
            out_folder: None,
            out_err: None,
            app_cache: AppCache::default(),
            out_name: String::new(),
            settings_path: None,
            show_only_empty: false,
            focused_id: None,
        }
    }

    /// Adds error to [out_err](Self::out_err).
    fn add_err(&mut self, e: Box<dyn Error>) {
        self.out_err.get_or_insert(vec![]).push(e);
    }

    /// Adds a notebook to the app.
    /// It will:
    /// 1. Update the cache & notebook (see [AppCache::load_or_add]).
    /// 2. Create the [title editors](TitleHolder).
    /// 3. Shift the pages of the notebooks, in case of merge when exporting.
    pub fn add_notebook(&mut self, mut notebook: Notebook, ui: &egui::Ui, ctx: &egui::Context) -> Result<(), Box<dyn Error>> {
        notebook.process_titles();
        self.app_cache.update_from_notebook(&notebook);
        let new_titles = TitleHolder::from_notebook(&notebook, ui, ctx);
        
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
        self.update_cache_from_editor();
        if let Some(p) = self.settings_path.as_ref() {
            self.process_result(self.app_cache.save_to(p));
        }

        self.update_note_from_holder();

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

    /// Updates the [Self::notebooks] (the [Notebook] and [TitleHolder])
    /// from [Self::app_cache].
    /// 
    /// Should be called only after loading new [cache](AppCache).
    fn update_note_from_cache(&mut self) {
        for (notebook, holder) in self.notebooks.iter_mut() {
            let title_cache = self.app_cache.notebooks.get(&notebook.file_id).unwrap();
            for (&k, cache) in title_cache {
                notebook.update_title(k, &cache.title);
            }
            holder.update_editor(notebook);
        }
    }

    /// Will update the [notebooks](Notebook) based on the content in the [TitleHolder].
    fn update_note_from_holder(&mut self) {
        for (notebook, holder) in self.notebooks.iter_mut() {
            for title in holder.titles.iter() {
                title.update_notebook(notebook);
            }
        }
    }

    /// Updates [Self::app_cache] from the [TitleEditor]s
    /// in [Self::notebooks].
    fn update_cache_from_editor(&mut self) {
        for (_, holder) in &self.notebooks {
            let (k, v) = holder.as_list();
            self.app_cache.update(k, v);
        }
    }

    /// Essentially works as a [Result::ok] but saves the [Err] to
    /// [out_err](Self::out_err) by calling [self.add_err(e)](Self::add_err).
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
            // Load/Save Export buttons
            ui.horizontal(|ui| {
                ui.vertical(|ui| {
                    if ui.button("Add File(s)").clicked() {
                        if let Some(path_list) = FileDialog::new().add_filter("Supernote File", &["note"]).pick_files() {
                            for file_p in path_list {
                                self.process_result(
                                    crate::io::load(file_p, &self.app_cache, &self.server_config)
                                ).and_then(|n| {
                                    let r = self.add_notebook(n, ui, ctx);
                                    self.process_result(r)
                                });
                            }
                        }
                    }

                    if !self.notebooks.is_empty() && ui.button(format!(
                        "Close Notebook{}",
                        if self.notebooks.len() < 2 {""} else {"s"}
                    )).clicked() {
                        self.update_cache_from_editor();
                        self.notebooks.clear();
                    }
                });
                
                ui.vertical(|ui| {
                    if ui.button(format!(
                        "{} Output Folder",
                        if self.out_folder.is_none() {"Add"} else {"Update"}
                    )).clicked() {
                        self.out_folder = FileDialog::new().pick_folder();
                    }

                    if self.out_folder.is_some() && !self.notebooks.is_empty() && ui.button("Export to PDF").clicked() {
                        if let Err(e) = self.package_and_export() {
                            self.add_err(e);
                        }
                    }
                });

                ui.vertical(|ui| {
                    if ui.button("Load Cache").clicked() {
                        if let Some(paths) = FileDialog::new().add_filter("Settings", &["json"]).pick_files() {
                            self.update_cache_from_editor();
                            for file_p in paths {
                                let file_p = self.settings_path.insert(file_p);
                                if let Err(e) = self.app_cache.merge_from_path(file_p) {
                                    self.add_err(e);
                                }
                            }
                            self.update_note_from_cache();
                        }
                    }
    
                    if ui.button("Save Cache").clicked() {
                        let file_dialog = match &self.settings_path {
                            Some(path) => {
                                let base_dialog = FileDialog::new().add_filter("JSON", &["json"]);
                                match (path.parent(), path.file_name()) {
                                    (None, None) => base_dialog,
                                    (None, Some(file_name)) => base_dialog.set_file_name(file_name.to_str().unwrap()),
                                    (Some(path), None) => base_dialog.set_directory(path),
                                    (Some(path), Some(file_name)) => base_dialog.set_directory(path).set_file_name(file_name.to_str().unwrap()),
                                }
                            },
                            None => FileDialog::new().add_filter("JSON", &["json"]),
                        };
                        if let Some(out_path) = file_dialog.save_file() {
                            self.update_cache_from_editor();
                            if let Err(e) = self.app_cache.save_to(&out_path) {
                                self.add_err(e);
                            }
                            self.settings_path = Some(out_path);
                        }
                    }
                });
            });

            ui.horizontal(|ui| {
                if ui.checkbox(&mut self.show_only_empty, "Only Show Empty Titles").changed() && !self.show_only_empty {
                    self.focused_id.take();
                }
                // Combine checkmark
                if self.notebooks.len() > 1 {
                    ui.checkbox(&mut self.app_cache.combine_pdfs, "Combine Notebooks?");
                    if self.app_cache.combine_pdfs {
                        ui.text_edit_singleline(&mut self.out_name);
                    }
                }
            });

            // Error showcasing
            if self.out_err.is_some() && ui.button("Clear Errors").clicked() {
                self.out_err = None;
            }
            if let Some(e) = &self.out_err {
                if e.len() < 2 {
                    ui.label(e[0].to_string());
                } else {
                    ui.collapsing("Errors: ", |ui| {
                        for err in e.iter() {
                            ui.label(err.to_string());
                        }
                    });
                }
            }

            egui::ScrollArea::vertical().max_width(f32::INFINITY).show(ui, |ui| {
                // TitleHolder render
                let mut title_bx = vec![];
                for (_, holder) in self.notebooks.iter_mut() {
                    ui.collapsing(holder.file_name.clone(), |ui| {
                        let mut used = false;
                        for title in holder.titles.iter_mut() {
                            let text_boxes = title.show(ui, self.show_only_empty, &mut self.focused_id);
                            if !text_boxes.is_empty() {
                                used = true;
                                title_bx.extend(text_boxes);
                            }
                        }
                        if !used {ui.label("All Titles are transcribed");}
                    });
                }
    
                // Showing the image.
                if let Some((txt_box, Some(texture))) = title_bx.iter().find(|(it, _)| it.has_focus()).or(title_bx.iter().find(|(i, _)| i.hovered())) {
                    let width = ctx.input(|i: &egui::InputState| i.screen_rect()).width() - txt_box.interact_rect.right();
                    let height = width / texture.aspect_ratio();
    
                    let mid_y = txt_box.interact_rect.top() + txt_box.interact_rect.height() * 0.5;
                    let min = egui::pos2(txt_box.interact_rect.right(), mid_y - height * 0.5);
    
                    let rect = egui::Rect::from_min_size(min, egui::Vec2 { x: width, y: height });
                    
                    if txt_box.gained_focus() {
                        ui.scroll_to_rect(rect, None);
                    }
                    
                    egui::Image::from_texture(texture)
                        .maintain_aspect_ratio(true)
                        .max_width(width)
                        .paint_at(ui, rect);
                }
            });
        
        });
    }
}

impl TitleHolder {
    pub fn from_notebook(notebook: &Notebook, ui: &egui::Ui, ctx: &egui::Context) -> Self {
        let mut titles = TitleHolder {
            file_id: notebook.file_id.clone(),
            file_name: notebook.file_name.clone(),
            titles: vec![],
        };
        titles.create_editors(notebook, ui, ctx);
        titles
    }

    /// Creates the [TitleEditor]s from the given [Notebook].
    fn create_editors(&mut self, notebook: &Notebook, ui: &egui::Ui, ctx: &egui::Context) {
        notebook.get_sorted_titles().into_iter()
            .filter_map(|title| {
                let page_id = notebook.get_page_id_from_internal(title.page_index)?;
                TitleEditor::new(title, page_id, ui, ctx)
            }.map(|te| (te, title.title_level)).ok()
            )
            .for_each(|(title, lvl)| self.add_title(title, lvl));
    }

    /// Updates the editor ([egui] elements) from the given [Notebook].
    pub fn update_editor(&mut self, notebook: &Notebook) {
        let mut new_titles = notebook.get_sorted_titles().into_iter();
        for title in &mut self.titles {
            title.visit_mut(&mut |editor| if let Some(Title {name, ..}) = new_titles.next() {
                match name {
                    Transciption::Manual(title) => {
                        editor.title.clone_from(title);
                        editor.was_edited = true;
                    },
                    Transciption::MyScript(title) => {
                        editor.title.clone_from(title);
                        editor.was_edited = false;
                    },
                    Transciption::None => {
                        editor.title = String::new();
                        editor.was_edited = false;
                    },
                    Transciption::Processing(_) =>
                        todo!("Unexpected Title still Processing"),
                }
                }
            );
        }
    }

    pub fn as_list(&self) -> (String, NotebookCache) {
        let list = self.titles.iter().flat_map(|t| t.as_cache_list()).map(|t| (t.hash, t)).collect();
        (self.file_id.clone(), list)
    }

    fn add_title(&mut self, title: TitleEditor, lvl: TitleLevel) {
        if let TitleLevel::BlackBack = lvl {
            self.titles.push(title);
        } else {
            self.titles.last_mut().expect("Should already contain a home-title")
                .add_child(title);
            // match self.titles.last_mut() {
            //     Some(t) => t.add_child(title),
            //     None => {
            //         let mut t = TitleEditor::create_blank(&title.page_id, TitleLevel::default());
            //         t.add_child(title);
            //         self.titles.push(t);
            //     },
            // }
        }
    }
}

impl TitleEditor {
    pub fn new(title: &Title, page_id: String, ui: &egui::Ui, ctx: &egui::Context) -> Result<Self, DecoderError> {
        let bitmap = title.render_bitmap()?;
        let img_texture = match bitmap {
            Some(bitmap) => Some(add_image(&bitmap, title.width, title.height, title.hash, ctx)?),
            None => None,
        };
        let persis_id = ui.make_persistent_id(format!("collapsing#{}", title.hash));
        let (title_transcript, was_edited) = match &title.name {
            Transciption::Manual(title) => (title.clone(), true),
            Transciption::MyScript(title) => (title.clone(), false),
            Transciption::None => (String::new(), false),
            Transciption::Processing(_) => todo!(),
        };
        Ok(TitleEditor {
            title: title_transcript,
            persis_id,
            img_texture,
            level: title.title_level,
            children: None,
            hash: title.hash,
            page_id,
            was_edited,
        })
    }

    /// Get's the data needed for the [Title] to
    /// be updated in the [Notebook].
    /// 
    /// That's the [title's hash](Title::content_hash) and
    /// new [name](Title::name).
    pub fn get_data(&self) -> (u64, Transciption) {
        let title = match self.title.is_empty() {
            true => Transciption::None,
            false => match self.was_edited {
                true => Transciption::Manual(self.title.clone()),
                false => Transciption::MyScript(self.title.clone()),
            },
        };
        (self.hash, title)
    }

    pub fn add_child(&mut self, title: TitleEditor) {
        if self.level.add() == title.level {
            // Reached the correct level
            let ch = self.children.get_or_insert(vec![]);
            ch.push(title);
        } else {
            // Need to go one level down
            let ch = self.children.as_mut().unwrap();
            ch.last_mut().unwrap().add_child(title);
        }
    }

    /// Get a flat list of [TitleCache]
    pub fn as_cache_list(&self) -> Vec<TitleCache> {
        match &self.children {
            Some(ch) => {
                let mut c: Vec<_> = ch.iter().flat_map(|t| t.as_cache_list()).collect();
                if let Some(cache) = self.as_single_cache() {
                    c.push(cache);
                }
                c
            },
            None => match self.as_single_cache() {
                Some(cache) => vec![cache],
                None => vec![],
            },
        }
    }

    /// Update the contents of [self] to the given [Notebook].
    pub fn update_notebook(&self, notebook: &mut Notebook) {
        let (hash, name) = self.get_data();
        notebook.update_title(hash, &name);
        if let Some(ch) = &self.children {
            ch.iter().for_each(|title| {
                title.update_notebook(notebook)
            });
        }
    }

    /// Converts itself to a [TitleCache] to be cached.
    /// **IGNORING CHILDREN**
    fn as_single_cache(&self) -> Option<TitleCache> {
        if !self.was_edited {
            return None
        }
        Some(TitleCache {
            title: match self.title.is_empty() {
                true => Transciption::None,
                false => match self.was_edited {
                    true => Transciption::Manual(self.title.clone()),
                    false => Transciption::MyScript(self.title.clone()),
                },
            },
            page_id: self.page_id.clone(),
            hash: self.hash,
        })
    }

    /// Renders all the titles as [CollapsingHeader](egui::CollapsingHeader)
    /// 
    /// If no [children](Self::children), simply render a [TextEdit](egui::TextEdit)
    pub fn show(&mut self, ui: &mut egui::Ui, show_empty: bool, focus: &mut Option<egui::Id>) -> Vec<(egui::Response, Option<egui::TextureHandle>)> {
        match &mut self.children {
            Some(children) => {
                let mut text_boxes = vec![];

                if show_empty {
                    if *focus == Some(self.persis_id) || self.title.is_empty() {
                        let txt_edit = Self::text_edit(&mut self.title, ui);
                        self.was_edited |= txt_edit.changed();
                        if txt_edit.has_focus() {
                            *focus = Some(self.persis_id);
                        }
                        text_boxes.push((txt_edit, self.img_texture.clone()));
                    }
                    text_boxes.extend(children.iter_mut().flat_map(|t| t.show(ui, show_empty, focus)));
                } else {
                    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), self.persis_id, false)
                        .show_header(ui, |ui| {
                            let txt_edit = Self::text_edit(&mut self.title, ui);
                            self.was_edited |= txt_edit.changed();
                            if txt_edit.has_focus() {
                                *focus = Some(self.persis_id);
                            }
                            text_boxes.push((txt_edit, self.img_texture.clone()));
                        })
                        .body(|ui| {
                            text_boxes.extend(children.iter_mut().flat_map(|t| t.show(ui, show_empty, focus)));
                        });
                }

                text_boxes
            },
            None => {
                // Simply add text box
                if !show_empty || (*focus == Some(self.persis_id) || self.title.is_empty()) {
                    let txt_edit = Self::text_edit(&mut self.title, ui);
                    self.was_edited |= txt_edit.changed();
                    if txt_edit.has_focus() {
                        *focus = Some(self.persis_id);
                    }
                    vec![(txt_edit, self.img_texture.clone())]
                } else {
                    vec![]
                }
            },
        }
    }

    /// Add the a single-line text editor to the [ui](egui::Ui) & returns that response.
    fn text_edit(title: &mut String, ui: &mut egui::Ui) -> egui::Response {
        ui.text_edit_singleline(title)
    }

    /// Perform the `f` function on itself and children mutably.
    pub fn visit_mut<F>(&mut self, f: &mut F)
    where
        F: FnMut(&mut TitleEditor)
    {
        f(self);

        if let Some(children) = &mut self.children {
            for child in children {
                child.visit_mut(f);
            }
        }
    }
}

