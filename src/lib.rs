#[macro_use]
mod macros;
mod io;
mod data_structures;
mod decoder;
pub mod exporter;
mod scheduler;
#[cfg(feature = "gui")]
mod ui;
#[cfg(not(feature = "gui"))]
pub mod command_line;

pub mod common {
    pub use crate::data_structures::file_format_consts as f_fmt;
    pub type PdfColor = [f64; 3];
}

pub mod error {
    pub use crate::decoder::DecoderError;
    pub use crate::data_structures::DataStructureError;
    pub use crate::exporter::PotraceError;
    pub use crate::data_structures::StrokeError;
    pub use crate::data_structures::TransciptionError;
}

use std::path::PathBuf;

pub use io::{load, LoadResult};
pub use data_structures::{Notebook, ServerConfig};
pub use data_structures::cache::AppCache;
pub use decoder::ColorMap;

pub use scheduler::{Scheduler, ExportSettings, MergeOrSep, messages};

/// Starts the EGUI App (default behaviour)
#[cfg(feature = "gui")]
pub fn start_app() {
    let _ = eframe::run_native(
        "Supernote Tool",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder { icon: Some(ui::icon::get_icon().into()), ..Default::default()  },
            follow_system_theme: false,
            default_theme: eframe::Theme::Light,
            ..Default::default()
        },
        Box::new(|ctx| {
            use raw_window_handle::HasWindowHandle;
            Ok(Box::new(ui::MyApp::new(ctx.window_handle().unwrap())))
        })
    );
}

pub fn sync_work(
    paths: Vec<PathBuf>, cache: Option<AppCache>, config: ServerConfig,
    merge: bool, export_path: PathBuf
) -> Vec<Result<(), Box<dyn std::error::Error>>>{
    use std::sync::Arc;
    use tokio::sync::RwLock;
    let cache = cache.unwrap_or_default();
    let config = Arc::new(RwLock::new(config));
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let results = paths.into_iter()
        .map(load)
        .map(|n_res| match n_res {
            Ok((
                note, metadata,
                data, page_data, file_name
            )) => {
                let note = note.into_commands(ColorMap::default());
                let c = cache.notebooks.get(&note.file_id);
                match rt.block_on(data_structures::TitleCollection::transcribe_titles(
                    metadata, data, c.cloned(), config.clone(), page_data, file_name.clone()
                )) {
                    Ok(titles) => Ok((note, titles, file_name)),
                    Err(err) => Err(err),
                }
            },
            Err(e) => Err(e),
        }).collect::<Vec<_>>();
        match merge {
            true => {
                // Cannot have any errors till now.
                let mut notes = Vec::with_capacity(results.len());
                let mut titles = Vec::with_capacity(results.len());

                let mut err_cont = false;
                let errors = results.into_iter().map(|r| match r {
                    Ok((n, t, _)) => {
                        notes.push(n);
                        titles.push(t.into());
                        Ok(())
                    },
                    Err(e) => {
                        err_cont = true;
                        Err(e)
                    },
                }).collect();
                // Create PDF & export.
                if !err_cont {
                    let mut map = exporter::MultiNotePageMap::new();
                    notes.iter().for_each(|n| map.push(exporter::PageMap::new_full(n.pages.len())));
                    match exporter::export_multiple(notes.iter().collect(), titles, map) {
                        Ok(mut doc) => {
                            doc.compress();
                            if let Err(e) = doc.save(export_path) {
                                return vec![Err(Box::new(e))];
                            }
                        },
                        Err(e) => return vec![Err(e)],
                    }
                }
                errors
            },
            false => {
                results.into_iter().map(|r| match r {
                    Ok((notebook, titles, name)) => {
                        let page_map = exporter::PageMap::new_full(notebook.pages.len());
                        match exporter::to_pdf(&notebook, titles.into(), &page_map) {
                            Err(e) => Err(e),
                            Ok(mut doc) => {
                                doc.compress();
                                match doc.save(
                                    export_path.with_file_name(format!("{}.pdf", name))
                                ) {
                                    Ok(_) => Ok(()),
                                    Err(e) => Err(Box::new(e).into()),
                                }
                            },
                        }
                    },
                    Err(e) => Err(e),
                }).collect()
            },
        }
}
