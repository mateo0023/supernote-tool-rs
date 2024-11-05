
use std::collections::HashMap;
use std::future::Future;
use std::error::Error;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;

use futures::{future, FutureExt as _, TryFutureExt as _};
use tokio::sync::{mpsc, RwLock};

use crate::data_structures::TitleCollection;
use crate::io::LoadResult;
use crate::scheduler::NoteMsg;
use crate::{load, AppCache, ColorMap, Notebook, ServerConfig};
use crate::exporter::{to_pdf, export_multiple};
use super::{ExportSettings, FutureBox, SchedulerResponse};

/// A [Future] that loads a single [Notebook].
#[derive(Clone)]
pub struct SingleNoteLoader {
    task: LoadingStage,
    cache: Arc<RwLock<AppCache>>,
    config: Arc<RwLock<ServerConfig>>,
    message_sender: mpsc::Sender<SchedulerResponse>,
}

#[derive(Default)]
enum LoadingStage {
    /// When loading the Title from file.
    Initial(FutureBox<Result<LoadResult, Box<dyn Error>>>),
    /// Holds both transcription and to_pdf_commands
    Title(Option<FutureBox<Result<(), String>>>, FutureBox<Notebook>),
    #[default]
    Empty
}

impl SingleNoteLoader {
    pub fn new(channel: mpsc::Sender<SchedulerResponse>, cache: Arc<RwLock<AppCache>>, config: Arc<RwLock<ServerConfig>>) -> Self {
        Self {
            task: LoadingStage::Empty,
            message_sender: channel,
            cache,
            config,
        }
    }

    /// Create a new [SingleNoteLoader] as a [Future] loading
    /// `path`.
    pub fn clone_w_task(&self, path: PathBuf) -> Self {
        let mut new = self.clone();
        new.task = LoadingStage::Initial(async move {load(path)}.boxed_local());
        new
    }
}

impl Future for SingleNoteLoader {
    type Output = Result<Notebook, Box<dyn Error>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        use SchedulerResponse::NoteMessage as Msg;

        let next = match self.task.take() {
            LoadingStage::Initial(mut task) => {
                match task.poll_unpin(cx) {
                    Poll::Ready(res) => match res {
                        Ok((note, metadata, data, page_data, file_name)) => {
                            let tx1 = self.message_sender.clone();
                            let file_id = note.file_id;
                            let arc_cache = self.cache.clone();
                            let config = self.config.clone();
                            
                            LoadingStage::Title(Some(async move {
                                    let _ = tx1.send(Msg(NoteMsg::LoadedToMemory(file_name.clone()))).await;
                                    let cache = arc_cache.read().await
                                        .notebooks.get(&file_id).cloned();
                                    TitleCollection::transcribe_titles(metadata, data, cache, config, page_data, file_name)
                                    .map_err(|e| e.to_string())
                                    .and_then(|title| tx1.send(Msg(NoteMsg::TitleLoaded(title)))
                                    .map_err(|e| e.to_string()))
                                    .await
                                }.boxed_local()),
                                async move {note.into_commands(ColorMap::default())}.boxed_local()
                            )
                        },
                        Err(e) => {
                            cx.waker().wake_by_ref();
                            return Poll::Ready(Err(e))
                        },
                    },
                    Poll::Pending => LoadingStage::Initial(task),
                }
            },
            LoadingStage::Title(mut title_task, mut notebook) => {
                let title_task = if let Some(mut title_task) = title_task.take() {
                    match title_task.poll_unpin(cx) {
                        Poll::Ready(_) => None,
                        Poll::Pending => Some(title_task),
                }} else { None };
                match notebook.poll_unpin(cx) {
                    Poll::Ready(note) => match title_task.is_some() {
                        // Transcrption still working
                        true => LoadingStage::Title(title_task, future::ready(note).boxed_local()),
                        false => {
                            cx.waker().wake_by_ref();
                            return Poll::Ready(Ok(note))
                        },
                    },
                    Poll::Pending => LoadingStage::Title(title_task, notebook),
                }
            },
            LoadingStage::Empty => {
                cx.waker().wake_by_ref();
                return Poll::Pending
            },
        };
        self.task = next;
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

impl LoadingStage {
    fn take(&mut self) -> Self {
        std::mem::take(self)
    }
}

impl Clone for LoadingStage {
    /// Creats new [`Empty`](LoadingStage::Empty)
    /// `Self`
    fn clone(&self) -> Self {
        Self::Empty
    }
}

/// Exports the notebooks given by their id in a separate thread.
pub fn export_notes(
    mut ids: Vec<u64>, export_settings: ExportSettings,
    loaded_notebooks: Arc<RwLock<HashMap<u64, Notebook>>>,
    loaded_titles: Arc<RwLock<HashMap<u64, TitleCollection>>>,
    response_sender: mpsc::Sender<SchedulerResponse>,
) -> std::thread::JoinHandle<()> {
    use super::SchedulerResponse::ExportMessage as Msg;
    use super::messages::ExpMsg as Ex;
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all().build().unwrap();

        rt.block_on(async {
            let mut loaded = vec![];
            let total_docs = ids.len() as f32;
            // Loop till all notebooks have been loaded.
            while !ids.is_empty() {
                // See if more notebooks have been loaded.
                let loaded_notebooks = loaded_notebooks.read().await;
                let loaded_titles = loaded_titles.read().await;
                let mut non_loaded = vec![];
                for id in ids {
                    match (loaded_notebooks.get(&id), loaded_titles.get(&id)) {
                        (Some(n), Some(t)) => loaded.push((n.clone(), t.clone())),
                        _ => {non_loaded.push(id);},
                    }
                }
                if non_loaded.is_empty() {
                    tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
                }
                ids = non_loaded
            }

            let mut docs_res = match export_settings {
                ExportSettings::Merged(path_buf) => {
                    loaded.sort_by(|a, b| a.1.note_name.cmp(&b.1.note_name));
                    let (notebooks, title_cols) = loaded.into_iter().unzip();
                    let _ = response_sender.send(Msg(Ex::CreatingDocs(0.))).await;
                    vec![(export_multiple(notebooks, title_cols), path_buf)]
                },
                ExportSettings::Seprate(mut paths) => {
                    loaded.sort_by_key(|n| n.0.file_id);
                    paths.sort_by_key(|n| n.0);
                    loaded.into_iter().zip(paths).enumerate()
                    .map(|(i, ((notebook, titles), (_, path)))| {
                        let _ = response_sender.try_send(
                            Msg(Ex::CreatingDocs(i as f32 / total_docs))
                        );
                        (to_pdf(notebook, titles), path)
                    }).collect()
                },
            };
            for (idx, (doc, _)) in docs_res.iter_mut().enumerate() {
                let _ = response_sender.send(Msg(Ex::CompressingDocs(idx as f32 / total_docs))).await;
                if let Ok(doc) = doc {
                    doc.compress();
                }
            }
            for (i, (doc, path)) in docs_res.into_iter().enumerate() {
                let i = i as f32;
                let _ = match doc {
                    Ok(mut d) => match d.save(path.clone()) {
                        Ok(_) => response_sender.send(Msg(Ex::SavingDocs(i / total_docs))).await,
                        Err(e) => response_sender.send(Msg(Ex::Error(e.to_string()))).await,
                    },
                    Err(e) => response_sender.send(Msg(Ex::Error(e.to_string()))).await,
                };
            }
            let _ = response_sender.send(Msg(Ex::Complete)).await;
        })
    })
}
