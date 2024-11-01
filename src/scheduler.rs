//! Module contains all the necessary interfaces. 
//! 
//! The library would only need to interface with [Scheduler].
//! Library should be able to:
//! * Send load commands for:
//!   * [`Notebook`](crate::data_structures::Notebook)
//!   * [`AppCache`](crate::data_structures::cache::AppCache)
//! * Send abort commands for running tasks.
//! * Receive 

use std::collections::HashMap;
use std::future::Future;
use std::error::Error;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;

use futures::{future, FutureExt as _,};
use futures::stream::{FuturesUnordered, StreamExt};
use tokio::sync::{mpsc, RwLock};

use crate::data_structures::cache::NotebookCache;
use crate::data_structures::TitleCollection;
use crate::error::TransciptionError;
use crate::io::LoadResult;
use crate::{load, AppCache, ColorMap, Notebook, ServerConfig};
use crate::exporter::{to_pdf, export_multiple};

/// The ammount of messages buffered.
const MSG_BUFFER: usize = 10;

/// This is the main scheduler.
/// 
/// You send commands to it and it runs them in parallel.
/// It is an async interface with messages.
pub struct Scheduler {
    command_sender: mpsc::Sender<SchedulerCommands>,
    response_receiver: mpsc::Receiver<SchedulerResponse>,
}

pub type FutureBox<T> = Pin<Box<dyn Future<Output = T>>>;
type ProcessStream<T> = FuturesUnordered<FutureBox<Result<T, Box<dyn Error>>>>;

pub enum ExportSettings {
    Merged(PathBuf),
    Seprate(Vec<PathBuf>),
}

enum SchedulerCommands {
    LoadNotebook(Vec<PathBuf>),
    LoadCache(PathBuf),
    ExportTo(Vec<TitleCollection>, ExportSettings),
    SaveCache(PathBuf),
    UpdateCache(u64, NotebookCache),
    UpdateSettings(ServerConfig),
}
pub enum SchedulerResponse {
    /// Notebook Loaded (still waiting on titles)
    /// 
    /// Contains the `file_name`
    NotebookPartLoaded(String),
    /// The notebook has been loaded and titles
    /// have been transcribed
    /// (contained in the message).
    NotebookFullLoaded(TitleCollection),
    /// The file has ben exported and saved to path
    FileSaved(PathBuf),
    CacheLoaded,
    /// Notebook failed to load with error message.
    NotebookFailedToLoad(String),
    /// Errors in transcription (if any).
    TranscriptError(Vec<TransciptionError>),
    /// The Cache Failed to load with `error`.
    CacheLoadFailed(String),
    /// The export to file failed with `error`.
    ExportFailed(String),
}

struct SchedulerIn {
    /// The current [`AppCache`].
    pub app_cache: Arc<RwLock<AppCache>>,
    /// The given [server configuration](ServerConfig)
    pub config: Arc<RwLock<ServerConfig>>,
    /// The fully_loaded notebooks.
    pub loaded_notebooks: HashMap<u64, Notebook>,

    loader_template: NotebookLoader,

    response_sender: mpsc::Sender<SchedulerResponse>,
    pub export_requests: Option<Vec<(Vec<TitleCollection>, ExportSettings)>>,

    note_task: FuturesUnordered<NotebookLoader>,
    pub export_process: ProcessStream<PathBuf>,
    pub cahe_load: ProcessStream<AppCache>,
}

#[derive(Clone)]
struct NotebookLoader {
    tasks: LoadingStage,
    cache: Arc<RwLock<AppCache>>,
    config: Arc<RwLock<ServerConfig>>,
    message_sender: mpsc::Sender<SchedulerResponse>,
}

#[derive(Default)]
enum LoadingStage {
    Initial(FutureBox<Result<LoadResult, Box<dyn Error>>>),
    Title(FutureBox<Result<TitleCollection, Box<dyn Error>>>, Notebook),
    Final(FutureBox<Notebook>),
    #[default]
    Empty
}

impl SchedulerResponse {
    /// Consumes Self and returns [`TitleCollection`] or Panics
    /// if not [`NoteFullLoaded`](SchedulerResponse::NotebookFullLoaded)
    /// variant.
    fn into_title(self) -> TitleCollection {
        match self {
            SchedulerResponse::NotebookFullLoaded(t) => t,
            _ => panic!("Response tried to extract title")
        }
    }
}

impl Scheduler {
    pub fn new() -> Self {
        let (command_sender, mut command_receiver) = mpsc::channel::<SchedulerCommands>(MSG_BUFFER);
        let (response_sender, response_receiver) = mpsc::channel::<SchedulerResponse>(MSG_BUFFER);

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all().build().unwrap();

            rt.block_on(async {
                let mut scheduler = SchedulerIn::new(response_sender.clone());
                
                loop {
                    tokio::select! {
                        Some(res) = scheduler.note_task.next() => match res {
                            Ok(note) => scheduler.add_notebook(vec![note]),
                            Err(err) => scheduler.response_sender.send(SchedulerResponse::NotebookFailedToLoad(err.to_string())).await.unwrap(),
                        },

                        Some(res) = scheduler.export_process.next() => match res {
                            Ok(p) => response_sender.send(SchedulerResponse::FileSaved(p)).await.unwrap(),
                            Err(e) => response_sender.send(SchedulerResponse::ExportFailed(e.to_string())).await.unwrap(),
                        },

                        Some(cache) = scheduler.cahe_load.next() => match cache {
                            Ok(cache) => {
                                response_sender.send(SchedulerResponse::CacheLoaded).await.unwrap();
                                scheduler.app_cache.write().await.merge(cache);
                            },
                            Err(e) => response_sender.send(SchedulerResponse::CacheLoadFailed(e.to_string())).await.unwrap(),
                        },

                        // _ = scheduler.message_sender.next() => {}

                        msg = command_receiver.recv() => match msg {
                            // Process the incomming message.
                            Some(msg) => scheduler.process_msg(msg),
                            // Messenger was dropped.
                            None => break,
                        },
                    }
                }
            });
        });

        Self {
            command_sender,
            response_receiver,
        }
    }

    pub fn save_cache(&mut self, path: PathBuf) {
        self.command_sender.blocking_send(SchedulerCommands::SaveCache(path)).unwrap();
    }

    pub fn load_cache(&self, path: PathBuf) {
        self.command_sender.blocking_send(SchedulerCommands::LoadCache(path)).unwrap();
    }

    pub fn update_cache(&self, k: u64, v: NotebookCache) {
        self.command_sender.blocking_send(SchedulerCommands::UpdateCache(k, v)).unwrap();
    }

    pub fn load_notebooks(&self, paths: Vec<PathBuf>, config: ServerConfig) {
        if let Err(e) = self.command_sender.blocking_send(SchedulerCommands::LoadNotebook(paths)) {
            panic!("Failed with {:?}", e);
        };
        self.command_sender.blocking_send(SchedulerCommands::UpdateSettings(config));
    }

    pub fn check_update(&mut self) -> Option<SchedulerResponse> {
        self.response_receiver.try_recv().ok()
    }

    pub fn save_notebooks(&self, notes: Vec<TitleCollection>, config: ExportSettings) {
        self.command_sender.blocking_send(SchedulerCommands::ExportTo(notes, config)).unwrap();
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl SchedulerIn {
    fn new(response_sender: mpsc::Sender<SchedulerResponse>) -> Self {
        let config: Arc<RwLock<ServerConfig>> = Default::default();
        let app_cache = Arc::new(RwLock::new(AppCache::default()));
        Self {
            loader_template: NotebookLoader::new(response_sender.clone(), app_cache.clone(), config.clone()),
            note_task: FuturesUnordered::new(),
            response_sender,
            app_cache,
            config,
            export_requests: Default::default(),
            loaded_notebooks: Default::default(),
            export_process: Default::default(),
            cahe_load: Default::default(),
        }
    }

    fn add_notebook(&mut self, note_res: Vec<Notebook>) {
        self.loaded_notebooks.extend(note_res.into_iter().map(|n| (n.file_id, n)));
        // self.notebook_loading_future = futures::future::pending().boxed_local();
    }

    fn process_msg(&mut self, msg: SchedulerCommands) {
        match msg {
            SchedulerCommands::LoadNotebook(vec) => {
                let futures = vec.into_iter()
                    .map(|p| self.loader_template.clone_w_task(p));
                if self.note_task.is_empty() {
                    self.note_task = FuturesUnordered::from_iter(futures);
                } else {
                    self.note_task.extend(futures);
                }
            },
            SchedulerCommands::LoadCache(path_buf) => self.cahe_load.push(Box::pin(AppCache::from_path(path_buf))),
            SchedulerCommands::ExportTo(titles, export_settings) => {
                {
                    let mut cache = self.app_cache.blocking_write();
                    titles.iter().for_each(|t| cache.update_from_notebook(t));
                }
                self.export_requests.get_or_insert(vec![])
                    .push((titles, export_settings));
            },
            SchedulerCommands::SaveCache(path) => {self.app_cache.blocking_read().save_to(&path);},
            SchedulerCommands::UpdateCache(k, title_cache) => {
                self.app_cache.write().then(|mut cache| future::ready(cache.notebooks.insert(k, title_cache)));
            },
            SchedulerCommands::UpdateSettings(server_config) => (),
        }
    }
}

impl NotebookLoader {
    fn new(channel: mpsc::Sender<SchedulerResponse>, cache: Arc<RwLock<AppCache>>, config: Arc<RwLock<ServerConfig>>) -> Self {
        Self {
            tasks: LoadingStage::Empty,
            message_sender: channel,
            cache,
            config,
        }
    }

    fn clone_w_task(&self, path: PathBuf) -> Self {
        let mut new = self.clone();
        new.tasks = LoadingStage::Initial(load(path).boxed_local());
        new
    }
}

impl Future for NotebookLoader {
    type Output = Result<Notebook, Box<dyn Error>>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        use SchedulerResponse::*;

        let next = match self.tasks.take() {
            LoadingStage::Initial(mut task) => {
                match task.poll_unpin(cx) {
                    Poll::Ready(res) => match res {
                        Ok((note, metadata, data, page_data, file_name)) => {
                            let tx = self.message_sender.clone();
                            let file_id = note.file_id;
                            let arc_cache = self.cache.clone();
                            let config = self.config.clone();
                            
                            LoadingStage::Title(async move {
                                    let _ = tx.send(NotebookPartLoaded(file_name.clone())).await;
                                    let cache = arc_cache.read().await
                                        .notebooks.get(&file_id).cloned();
                                    TitleCollection::transcribe_titles(metadata, data, cache, config, page_data, file_name)
                                        .await
                                }.boxed_local(),
                                note
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
            LoadingStage::Title(mut title_task, notebook) => {
                match title_task.poll_unpin(cx) {
                    Poll::Ready(res) => match res {
                        Ok(title) => {
                            match self.message_sender.try_send(NotebookFullLoaded(title)) {
                                Ok(_) => LoadingStage::Final(
                                    notebook.into_commands(ColorMap::default())
                                        .boxed_local()
                                ),
                                Err(err) => {
                                    let title = err.into_inner().into_title();
                                    LoadingStage::Title(future::ok(title).boxed_local(), notebook)
                                },
                            }
                        },
                        Err(e) => {cx.waker().wake_by_ref();return Poll::Ready(Err(e))},
                    },
                    Poll::Pending => LoadingStage::Title(title_task, notebook),
                }
            },
            LoadingStage::Final(mut note) => match note.poll_unpin(cx) {
                Poll::Ready(n) => {
                    cx.waker().wake_by_ref();
                    return Poll::Ready(Ok(n));
                },
                Poll::Pending => LoadingStage::Final(note),
            },
            LoadingStage::Empty => {
                cx.waker().wake_by_ref();
                return Poll::Pending
            },
        };
        self.tasks = next;
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
