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

use futures::{future, FutureExt as _, TryFutureExt as _};
use futures::stream::{FuturesUnordered, StreamExt};
use tokio::sync::{mpsc, RwLock};

use crate::data_structures::cache::NotebookCache;
use crate::data_structures::TitleCollection;
use crate::io::LoadResult;
use crate::{load, AppCache, ColorMap, Notebook, ServerConfig};
use crate::exporter::{to_pdf, export_multiple};

pub mod messages {
    //! These are the messages coming from the [`Scheduler`](super::Scheduler)
    use super::{PathBuf, TitleCollection};
    pub enum SchedulerResponse {
        NoteMessage(NoteMsg),
        CahceMessage(CacheMsg),
        /// The file has ben exported and saved to path
        FileSaved(PathBuf),
        /// The export to file failed with `error`.
        ExportFailed(String),
    }
    
    pub enum NoteMsg {
        /// Notebook Loaded (still waiting on titles)
        /// 
        /// Contains the `file_name`
        LoadedToMemory(String),
        /// The notebook has been loaded and titles
        /// have been transcribed
        /// (contained in the message).
        TitlesLoaded(TitleCollection),
        /// Notebook failed to load with error message.
        FailedToLoad(String),
        FullyLoaded(u64),
    }
    
    pub enum CacheMsg {
        Loaded,
        FailedToLoad(String),
        FailedToSave(String),
        Saved,
    }
}

macro_rules! misc_task {
    {$self:ident($($cloned:ident),+) => $func:block} => {
        $(let $cloned = $self.$cloned.clone();)+
        $self.add_task(async move {
            $func
        }.boxed_local());
    };
}

use messages::*;

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

pub enum ExportSettings {
    Merged(PathBuf),
    Seprate(Vec<(u64, PathBuf)>),
}

enum SchedulerCommands {
    LoadNotebook(Vec<PathBuf>),
    LoadCache(PathBuf),
    ExportTo(Vec<TitleCollection>, ExportSettings),
    SaveCache(PathBuf),
    UpdateCache(u64, NotebookCache),
    UpdateSettings(ServerConfig),
}

struct SchedulerIn {
    /// The current [`AppCache`].
    app_cache: Arc<RwLock<AppCache>>,
    /// The given [server configuration](ServerConfig)
    config: Arc<RwLock<ServerConfig>>,
    /// The fully_loaded notebooks.
    loaded_notebooks: Arc<RwLock<HashMap<u64, Notebook>>>,
    loaded_titles: Arc<RwLock<HashMap<u64, TitleCollection>>>,
    response_sender: mpsc::Sender<SchedulerResponse>,
    
    loader_template: SingeNoteLoader,
    
    note_tasks: StreamGuard<SingeNoteLoader>,
    misc_tasks: StreamGuard<FutureBox<()>>,
}

struct StreamGuard<T: Future> {
    tsk: FuturesUnordered<T>,
}

#[derive(Clone)]
struct SingeNoteLoader {
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

struct NotebookExporter {
    tsk: ExportStage,
    tx: mpsc::Sender<SchedulerResponse>,
}

#[derive(Default)]
enum ExportStage {
    PreLoading(Vec<u64>, ExportSettings,
        Arc<RwLock<HashMap<u64, Notebook>>>,
        Arc<RwLock<HashMap<u64, TitleCollection>>>,
        Vec<(Notebook, TitleCollection)>,
    ),
    ExecutingSingle(FutureBox<()>),
    ExecutingMult(FuturesUnordered<FutureBox<()>>),
    #[default]
    Empty
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
                    use SchedulerResponse::*;
                    tokio::select! {
                        Some(res) = &mut scheduler.note_tasks => match res {
                            Ok(note) => scheduler.add_notebook(vec![note]),
                            Err(err) => scheduler.response_sender.send(NoteMessage(NoteMsg::FailedToLoad(err.to_string()))).await.unwrap(),
                        },

                        // Some(res) = scheduler.export_tasks.next() => {}

                        None = &mut scheduler.misc_tasks => {println!("Guard Failed")}

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
        self.command_sender.blocking_send(SchedulerCommands::UpdateSettings(config)).unwrap();
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
            loader_template: SingeNoteLoader::new(response_sender.clone(), app_cache.clone(), config.clone()),
            note_tasks: StreamGuard::new(),
            response_sender,
            app_cache,
            config,
            loaded_notebooks: Default::default(),
            misc_tasks: StreamGuard::new(),
            loaded_titles: Default::default(),
        }
    }

    fn add_notebook(&mut self, note_res: Vec<Notebook>) {
        misc_task!(self(loaded_notebooks) => {
            loaded_notebooks.write().await.extend(note_res.into_iter().map(|n| (n.file_id, n)));
        });
    }

    fn process_msg(&mut self, msg: SchedulerCommands) {
        match msg {
            SchedulerCommands::LoadNotebook(vec) => {
                self.note_tasks.extend(
                vec.into_iter().map(|path|
                        self.loader_template.clone_w_task(path)
                    )
                );
            },
            SchedulerCommands::LoadCache(path_buf) => {
                misc_task!(self(app_cache, response_sender) => {
                    use SchedulerResponse::CahceMessage as Msg;
                    match AppCache::from_path(path_buf).await {
                        Ok(cache) => {
                            response_sender.send(Msg(CacheMsg::Loaded))
                            .then(|_|
                                app_cache.write().then(|mut c| {
                                    c.merge(cache);
                                    future::ready(())
                                })
                            ).await;
                        },
                        Err(e) => {response_sender.send(Msg(CacheMsg::FailedToLoad(e.to_string()))).await.unwrap();},
                    }
                });
            },
            SchedulerCommands::ExportTo(titles, export_settings) => {
                let ids = titles.iter().map(|t| t.note_id).collect();
                misc_task!(self(app_cache, loaded_titles, response_sender, loaded_notebooks) => {
                    let mut c = app_cache.write().await;
                    titles.iter().for_each(|t| c.update_from_notebook(t));
                    loaded_titles.write().await.extend(
                        titles.into_iter().map(|t| (t.note_id, t))
                    );
                    NotebookExporter {
                        tsk: ExportStage::PreLoading(
                            ids, export_settings,
                            loaded_notebooks, loaded_titles,
                            vec![]),
                        tx: response_sender
                    }.await
                });
            },
            SchedulerCommands::SaveCache(path) => {
                misc_task!(self(app_cache, response_sender) => {
                    use SchedulerResponse::CahceMessage as MSG;
                    match app_cache.read().await.save_to(&path) {
                        Ok(_) => response_sender.send(MSG(CacheMsg::Saved)).await.unwrap(),
                        Err(e) => response_sender
                            .send(MSG(CacheMsg::FailedToSave(e.to_string()))).await.unwrap(),
                    };
                });
            },
            SchedulerCommands::UpdateCache(k, title_cache) => {
                misc_task!(self(app_cache) => {
                    app_cache.write()
                        .then(|mut cache| future::ready(cache.notebooks.insert(k, title_cache)))
                        .await;
                });
            },
            SchedulerCommands::UpdateSettings(server_config) => {
                misc_task!(self(config) => {
                    *config.write().await = server_config;
                });
            },
        }
    }

    fn add_task(&mut self, tsk: FutureBox<()>) {
        self.misc_tasks.push(tsk);
    }

    fn update_cache(&mut self) {
        misc_task!{self(app_cache, loaded_titles) => {
            let mut c = app_cache.write().await;
            loaded_titles.read().await.values()
                .for_each(|t| c.update_from_notebook(t));
        }}
    }
}

impl<T: Future> StreamGuard<T> {
    fn new() -> Self {
        Self { tsk: FuturesUnordered::new() }
    }

    fn push(&mut self, value: T) {
        if self.tsk.is_empty() {
            self.tsk = FuturesUnordered::new();
        }
        self.tsk.push(value);
    }

    fn extend<I: IntoIterator<Item = T>>(&mut self, paths: I) {
        if self.tsk.is_empty() {
            self.tsk = FuturesUnordered::new();
        }
        self.tsk.extend(
            paths
        );
    }
}

impl<T: Future> Future for StreamGuard<T> {
    type Output = Option<T::Output>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        // Ensure we never poll when it's empty.
        if !self.tsk.is_empty() {
            self.tsk.poll_next_unpin(cx)
        } else {
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

impl SingeNoteLoader {
    fn new(channel: mpsc::Sender<SchedulerResponse>, cache: Arc<RwLock<AppCache>>, config: Arc<RwLock<ServerConfig>>) -> Self {
        Self {
            task: LoadingStage::Empty,
            message_sender: channel,
            cache,
            config,
        }
    }

    fn clone_w_task(&self, path: PathBuf) -> Self {
        let mut new = self.clone();
        new.task = LoadingStage::Initial(load(path).boxed_local());
        new
    }
}

impl Future for SingeNoteLoader {
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
                                    .and_then(|title| tx1.send(Msg(NoteMsg::TitlesLoaded(title)))
                                    .map_err(|e| e.to_string()))
                                    .await
                                }.boxed_local()),
                                note.into_commands(ColorMap::default()).boxed_local()
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

impl Future for NotebookExporter {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        use SchedulerResponse::{FileSaved, ExportFailed};
        let tsk = match self.tsk.take() {
            ExportStage::PreLoading(vec, export_settings, note, title, mut loaded) => {
                let mut unused_id = None;
                let mut id_it = vec.into_iter();
                {
                    let mut n_r = note.read().boxed_local();
                    let mut t_r = title.read().boxed_local();
                    if let (Poll::Ready(note_r), Poll::Ready(title_r)) = (n_r.poll_unpin(cx), t_r.poll_unpin(cx)) {
                        for id in id_it.by_ref() {
                            match (note_r.get(&id), title_r.get(&id)) {
                                (Some(n), Some(t)) => loaded.push((n.clone(), t.clone())),
                                _ => {unused_id = Some(id); break},
                            }
                        }
                    }
                    
                }
                match unused_id {
                    // Not ready to export
                    Some(id) => {
                        let mut v: Vec<_> = id_it.collect();
                        v.push(id);
                        ExportStage::PreLoading(v, export_settings, note, title, loaded)
                    },
                    // Ready to export
                    None => {
                        match export_settings {
                            ExportSettings::Merged(path_buf) => {
                                loaded.sort_by(|a, b| a.1.note_name.cmp(&b.1.note_name));
                                let (notebooks, title_cols) = loaded.into_iter().unzip();
                                let tx = self.tx.clone();
                                ExportStage::ExecutingSingle(async move {
                                    let path = path_buf.clone();
                                    export_multiple(notebooks, title_cols)
                                        .map_err(|e| e.to_string())
                                        .and_then(|mut doc| future::ready(
                                            doc.save(path).map_err(|e| e.to_string())
                                        )).then(|res| match res {
                                            Ok(_) => tx.send(FileSaved(path_buf)),
                                            Err(e) => tx.send(ExportFailed(e)),
                                        }).await.unwrap();
                                    }.boxed_local())
                            },
                            ExportSettings::Seprate(mut paths) => {
                                loaded.sort_by_key(|n| n.0.file_id);
                                paths.sort_by_key(|n| n.0);
                                let futs = paths.into_iter().zip(loaded).map(|((_, path), (note, title))| {
                                    let path_buf = path.clone();
                                    let tx = self.tx.clone();
                                    async move {
                                        to_pdf(note, title)
                                        .map_err(|e| e.to_string())
                                        .and_then(|mut doc| future::ready(
                                            doc.save(path).map_err(|e| e.to_string())
                                        )).then(|res| match res {
                                            Ok(_) => tx.send(FileSaved(path_buf)),
                                            Err(e) => tx.send(ExportFailed(e)),
                                        }).await.unwrap();
                                    }.boxed_local()
                                });
                                ExportStage::ExecutingMult(FuturesUnordered::from_iter(futs))
                            },
                        }
                    },
                }
            },
            ExportStage::ExecutingSingle(mut pin) => match pin.poll_unpin(cx) {
                Poll::Ready(_) => {cx.waker().wake_by_ref();return Poll::Ready(())},
                Poll::Pending => ExportStage::ExecutingSingle(pin),
            },
            ExportStage::ExecutingMult(mut futures_unordered) => match futures_unordered.poll_next_unpin(cx) {
                Poll::Ready(None) => {cx.waker().wake_by_ref();return Poll::Ready(())},
                Poll::Ready(Some(_)) |
                Poll::Pending => ExportStage::ExecutingMult(futures_unordered),
            },
            ExportStage::Empty => {cx.waker().wake_by_ref();return Poll::Ready(())},
        };
        self.tsk = tsk;
        cx.waker().wake_by_ref();
        Poll::Pending
    }
}

impl ExportStage {
    fn take(&mut self) -> Self {
        std::mem::take(self)
    }
}
