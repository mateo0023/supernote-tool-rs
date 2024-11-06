use std::path::PathBuf;
use std::time::{Duration, Instant};

use rfd::FileDialog;

use crate::data_structures::{ServerConfig, Title, TitleCollection, TitleLevel, Transciption};
use crate::error::*;
use crate::data_structures::cache::*;
use crate::scheduler::*;

pub mod icon;

pub struct MyApp {
    server_config: ServerConfig,
    scheduler: Scheduler,
    notebooks: Vec<(TitleCollection, TitleHolder)>,
    /// The folder to export the PDF(s)
    out_folder: Option<PathBuf>,
    /// Any error messages to display.
    out_err: Option<Vec<String>>,
    combine_pdfs: bool,
    /// The name to save the Merged PDF
    out_name: String,
    /// The path to the [AppCache]
    settings_path: Option<PathBuf>,
    show_only_empty: bool,
    /// The [egui::Id] of the [TitleEditor]
    /// currently in focus.
    focused_id: Option<egui::Id>,
    cache_loading: ProcessState,
    cache_saving: ProcessState,
    /// 0. How many notebooks have been sent to load
    /// 1. How many notebooks are waiting for titles.
    /// 2. How many notebooks have been loaded.
    /// 3. Message to display
    note_loading_status: Option<(usize, usize, usize, String)>,
    /// 0. How far along we are [0, 1]
    /// 1. Message to display.
    note_exp_status: Option<(f32, String)>,
}

#[derive(Default, Clone, Copy)]
enum ProcessState {
    /// It's processing (unkown progress)
    Processing,
    /// Process is done, but we need some timeout
    /// to display result.
    TimeOut(Instant),
    /// The process is fully done.
    #[default]
    Done,
}

#[derive(Default)]
struct TitleHolder {
    file_id: u64,
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
    page_id: u64,
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
            scheduler: Scheduler::new(),
            notebooks: vec![],
            server_config: ServerConfig::from_path_or_default("./my_script_keys.json"),
            out_folder: None,
            out_err: None,
            out_name: String::new(),
            settings_path: None,
            show_only_empty: false,
            focused_id: None,
            combine_pdfs: true,
            cache_loading: ProcessState::Done,
            cache_saving: ProcessState::Done,
            note_loading_status: None,
            note_exp_status: None,
        }
    }

    pub fn load_cache(&mut self, path: PathBuf) {
        self.scheduler.load_cache(path);
        self.cache_loading = ProcessState::Processing;
    }

    /// Adds a notebook to the app.
    /// It will:
    /// 1. Update the cache & notebook (see [AppCache::load_or_add]).
    /// 2. Create the [title editors](TitleHolder).
    /// 3. Shift the pages of the notebooks, in case of merge when exporting.
    pub fn add_notebook(&mut self, notebook: TitleCollection, ui: &egui::Ui, ctx: &egui::Context) {
        let new_titles = TitleHolder::from_notebook(&notebook, ui, ctx);
        
        self.notebooks.push((notebook, new_titles));
        self.notebooks.sort_by_cached_key(|n| n.0.note_name.clone());
    }

    /// Will update the titles and render the [notebook(s)](Self::notebooks)
    /// into a PDF (or PDFs).
    fn package_and_export(&mut self) {
        self.update_cache_from_editor();
        if let Some(p) = self.settings_path.as_ref() {
            self.scheduler.save_cache(p.clone());
            self.cache_saving = ProcessState::Processing;
        }

        self.update_note_from_holder();

        if self.notebooks.len() < 2 || !self.combine_pdfs || self.out_name.is_empty() {
            if let Some(path) = &self.out_folder {
                let mut notes = vec![];
                let mut paths = vec![];
                for (note, _) in &self.notebooks {
                    let new_path = path.join(format!("{}.pdf", note.note_name));
                    notes.push(note.clone());
                    paths.push((note.note_id, new_path));
                }
                self.note_exp_status = Some((0., "Loading Notebooks".to_string()));
                self.scheduler.save_notebooks(
                    notes,
                    ExportSettings::Seprate(paths)
                );
            }
        } else if let Some(path) = &self.out_folder {
            self.note_exp_status = Some((0., "Loading Notebooks".to_string()));
            self.scheduler.save_notebooks(
                self.notebooks.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>(),
                ExportSettings::Merged(path.join(format!("{}.pdf", self.out_name)))
            );
        }
    }

    /// Will update the [notebooks](TitleCollection)
    /// based on the content in the [TitleHolder].
    fn update_note_from_holder(&mut self) {
        for (notebook, holder) in self.notebooks.iter_mut() {
            for title in holder.titles.iter() {
                title.update_notebook(notebook);
            }
        }
    }

    /// Updates app_cache from the [TitleEditor]s
    /// in [Self::notebooks].
    fn update_cache_from_editor(&mut self) {
        for (_, holder) in &self.notebooks {
            let (k, v) = holder.get_cache();
            self.scheduler.update_cache(k, v);
        }
    }

    /// Checks the messages from the [Scheduler] and updates necessary
    /// internal values:
    /// * [`note_loading_status`](MyApp::note_loading_status)
    /// * [`note_exp_status`](MyApp::note_exp_status)
    /// * [`out_err`](MyApp::out_err)
    /// * [`cache_loading`](MyApp::cache_loading)
    fn check_messages(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        if let Some(msg) = self.scheduler.check_update() {
            use messages::SchedulerResponse::*;
            match msg {
                NoteMessage(note_msg) => match note_msg {
                    messages::NoteMsg::LoadedToMemory(name) => if let Some((_, p_l, _, msg)) = self.note_loading_status.as_mut() {
                        *p_l += 1;
                        *msg = format!("{} Processing Titles", name);
                    },
                    messages::NoteMsg::TitleLoaded(notebook) => {
                        if let Some((t, _, done, msg)) = self.note_loading_status.as_mut() {
                            *done += 1;
                            *msg = format!("{} LOADED", notebook.note_name.clone());
                            if t <= done {
                                self.note_loading_status = None;
                            }
                        }
                        self.add_notebook(notebook, ui, ctx);
                    },
                    messages::NoteMsg::FailedToLoad(msg) => {
                        if let Some((_, _, done, _)) = self.note_loading_status.as_mut() {
                            *done += 1;
                        }
                        self.out_err.get_or_insert(vec![]).push(
                            format!("A notebook failed to load due to {}", msg)
                        );
                    },
                    messages::NoteMsg::FullyLoaded(_) => (),
                },
                CahceMessage(cache_msg) => match cache_msg {
                    messages::CacheMsg::Loaded => self.cache_loading.one_sec(),
                    messages::CacheMsg::FailedToLoad(msg) => {
                        self.cache_loading = ProcessState::Done;
                        self.out_err.get_or_insert(vec![]).push(
                            format!("Cache Failed to load due to {}", msg)
                        )
                    },
                    messages::CacheMsg::FailedToSave(msg) => {
                        self.cache_saving = ProcessState::Done;
                        self.out_err.get_or_insert(vec![]).push(
                            format!("Cache failed to save due to {}", msg)
                        )
                    },
                    messages::CacheMsg::Saved => self.cache_saving.one_sec(),
                },
                ExportMessage(exp_msg) => match exp_msg {
                    messages::ExpMsg::Error(err) => {self.out_err.get_or_insert(vec![]).push(err);},
                    messages::ExpMsg::CreatingDocs(p) => self.note_exp_status = Some((p * 0.3, "Creating PDF(s)".to_string())),
                    messages::ExpMsg::CompressingDocs(p) => self.note_exp_status = Some((0.3 + p * 0.5, "Compressing PDF(s)".to_string())),
                    messages::ExpMsg::SavingDocs(p) => self.note_exp_status = Some((0.8 + p * 0.2, "Saving PDF(s)".to_string())),
                    messages::ExpMsg::Complete => self.note_exp_status = None,
                    
                },
            }
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            // Load/Save Export buttons
            ui.horizontal(|ui| {
                // Add/Remove Notebooks
                ui.vertical(|ui| {
                    if ui.button("Add File(s)").clicked() {
                        if let Some(path_list) = FileDialog::new().add_filter("Supernote File", &["note"]).pick_files() {
                            self.note_loading_status = Some((path_list.len(), 0, 0, format!("Loading {} files", path_list.len())));
                            self.scheduler.load_notebooks(path_list, self.server_config.clone());
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
                
                // Output Folder & Export Buttons
                ui.vertical(|ui| {
                    if ui.button(format!(
                        "{} Output Folder",
                        if self.out_folder.is_none() {"Add"} else {"Update"}
                    )).clicked() {
                        self.out_folder = FileDialog::new().pick_folder();
                    }

                    if self.out_folder.is_some() && !self.notebooks.is_empty() && ui.button("Export to PDF").clicked() {
                        self.package_and_export();
                    }
                });

                // Cache Buttons
                ui.vertical(|ui| {
                    ui.horizontal( |ui| {
                        if ui.button("Load Transcriptions").clicked() {
                            if let Some(path) = FileDialog::new().add_filter("Transcriptions", &["json"]).pick_file() {
                                self.load_cache(path);
                            }
                        }
                        match self.cache_loading {
                            ProcessState::Processing => {
                                ui.add(egui::Spinner::new());
                            },
                            ProcessState::TimeOut(end) => if Instant::now() < end {
                                ui.label(egui::RichText::new("✅")
                                    .size(16.)
                                );
                            },
                            ProcessState::Done => (),
                        }
                    });
    
                    if ui.button("Save Transcriptions").clicked() {
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
                            self.scheduler.save_cache(out_path.clone());
                            self.cache_saving = ProcessState::Processing;
                            self.settings_path = Some(out_path);
                        }
                    }
                });
            });

            self.check_messages(ui, ctx);

            // Note Loading progress
            if let Some((total, part, comp, msg)) = self.note_loading_status.as_ref() {
                let total = *total as f32;
                let progress = *part as f32 / total * 0.4
                    + *comp as f32 / total * 0.6;
                ui.horizontal(|ui| {
                    ui.label(msg);
                    ui.add(
                        egui::ProgressBar::new(progress)
                        .animate(true)
                    );
                });
            }

            // Note EXPORT progress
            if let Some((p, msg)) = self.note_exp_status.as_ref() {
                ui.horizontal(|ui| {
                    ui.label(msg);
                    ui.add(egui::ProgressBar::new(*p)
                        .animate(true)
                    );
                });
            }

            ui.horizontal(|ui| {
                if ui.checkbox(&mut self.show_only_empty, "Only Show Empty Titles").changed() && !self.show_only_empty {
                    self.focused_id.take();
                }
                // Combine checkmark
                if self.notebooks.len() > 1 {
                    ui.checkbox(&mut self.combine_pdfs, "Combine Notebooks?");
                    if self.combine_pdfs {
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
                    if holder.is_empty() {
                        ui.label(format!("File {} contains no titles", holder.file_name));
                    } else {
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

impl ProcessState {
    pub fn one_sec(&mut self) {
        *self = ProcessState::TimeOut(
            Instant::now().checked_add(Duration::from_secs(1)).unwrap()
        );
    }
}

impl TitleHolder {
    pub fn from_notebook(notebook: &TitleCollection, ui: &egui::Ui, ctx: &egui::Context) -> Self {
        let mut titles = TitleHolder {
            file_id: notebook.note_id,
            file_name: notebook.note_name.clone(),
            titles: vec![],
        };
        titles.create_editors(notebook, ui, ctx);
        titles
    }

    /// Creates the [TitleEditor]s from the given [TitleCollection].
    fn create_editors(&mut self, notebook: &TitleCollection, ui: &egui::Ui, ctx: &egui::Context) {
        notebook.get_sorted_titles().into_iter()
            .filter_map(|title| {
                TitleEditor::new(title, title.page_id, ui, ctx)
            }.map(|te| (te, title.title_level)).ok()
            )
            .for_each(|(title, lvl)| self.add_title(title, lvl));
    }

    pub fn get_cache(&self) -> (u64, NotebookCache) {
        let list = self.titles.iter().flat_map(|t| t.as_cache_list()).map(|t| (t.hash, t)).collect();
        (self.file_id, list)
    }

    fn add_title(&mut self, title: TitleEditor, lvl: TitleLevel) {
        if let TitleLevel::BlackBack = lvl {
            self.titles.push(title);
        } else {
            self.titles.last_mut().expect("Should already contain a home-title")
                .add_child(title);
        }
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.titles.is_empty()
    }
}

impl TitleEditor {
    pub fn new(title: &Title, page_id: u64, ui: &egui::Ui, ctx: &egui::Context) -> Result<Self, DecoderError> {
        let bitmap = title.render_bitmap()?;
        let width = (title.coords[2] - title.coords[0]) as usize;
        let height = (title.coords[3] - title.coords[1]) as usize;
        let img_texture = match bitmap {
            Some(bitmap) => Some(add_image(&bitmap, width, height, title.hash, ctx)?),
            None => None,
        };
        let persis_id = ui.make_persistent_id(format!("collapsing#{}", title.hash));
        let (title_transcript, was_edited) = match &title.name {
            Transciption::Manual(title) => (title.clone(), true),
            Transciption::MyScript(title) => (title.clone(), false),
            Transciption::None => (String::new(), false),
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
    /// be updated in the [TitleCollection].
    /// 
    /// That's the [title's hash](Title::hash) and
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

    /// Update the contents of [self] to the given [TitleCollection].
    pub fn update_notebook(&self, notebook: &mut TitleCollection) {
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
            page_id: self.page_id,
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
}

