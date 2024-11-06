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
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::task::Poll;

use futures::{future, FutureExt as _,};
use futures::stream::{FuturesUnordered, StreamExt};
use tasks::SingleNoteLoader;
use tokio::sync::{mpsc, RwLock};

use crate::data_structures::cache::NotebookCache;
use crate::data_structures::TitleCollection;
use crate::{AppCache, Notebook, ServerConfig};

pub mod messages {
    //! These are the messages coming from the [`Scheduler`](super::Scheduler)
    use super::TitleCollection;
    pub enum SchedulerResponse {
        NoteMessage(NoteMsg),
        CahceMessage(CacheMsg),
        ExportMessage(ExpMsg),
    }

    pub enum ExpMsg {
        CreatingDocs(f32),
        CompressingDocs(f32),
        SavingDocs(f32),
        Complete,
        Error(String),
    }
    
    pub enum NoteMsg {
        /// Notebook Loaded (still waiting on titles)
        /// 
        /// Contains the `file_name`
        LoadedToMemory(String),
        /// The notebook has been loaded and titles
        /// have been transcribed
        /// (contained in the message).
        TitleLoaded(TitleCollection),
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

mod tasks;

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
    /// Export the given [TitleCollection]s and settings.
    /// 
    /// Needs to have already loaded the [Notebook]s to RAM.
    ExportTo(Vec<TitleCollection>, ExportSettings),
    SaveCache(PathBuf),
    UpdateCache(u64, NotebookCache),
    UpdateSettings(ServerConfig),
}

struct SchedulerIn {
    /// The current [`AppCache`].
    app_cache: Arc<RwLock<AppCache>>,
    app_cache_path: Arc<RwLock<Option<PathBuf>>>,
    /// The given [server configuration](ServerConfig)
    config: Arc<RwLock<ServerConfig>>,
    /// The fully_loaded notebooks.
    loaded_notebooks: Arc<RwLock<HashMap<u64, Notebook>>>,
    loaded_titles: Arc<RwLock<HashMap<u64, TitleCollection>>>,
    response_sender: mpsc::Sender<SchedulerResponse>,
    
    loader_template: SingleNoteLoader,
    
    /// Stores the [Notebook] import tasks in a [`StreamGuard`]
    note_tasks: StreamGuard<SingleNoteLoader>,
    /// Stores all other tasks with return type `()` in
    /// a [`StreamGuard`]
    misc_tasks: StreamGuard<FutureBox<()>>,
}

/// A wrapper around [`FuturesUnordered<T>`] to ensure it
/// can always be
/// [push](StreamGuard::push)ed/[extend](StreamGuard::extend)ed
/// and [poll](StreamGuard::poll)ed with known behaviour.
/// 
/// Works diffrently than [FuturesUnordered],
/// it implements [`Future<Output = T::Output>`](Future).
/// When polled, will return [`Poll::Pending`] if there are no futures left.
/// As opposed to returning [`Poll::Ready(None)`](Poll::Ready).
struct StreamGuard<T: Future> {
    tsk: FuturesUnordered<T>,
    wk: Option<std::task::Waker>,
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
                        res = &mut scheduler.note_tasks => match res {
                            Ok(note) => scheduler.add_notebook(vec![note]),
                            Err(err) => scheduler.response_sender.send(NoteMessage(NoteMsg::FailedToLoad(err.to_string()))).await.unwrap(),
                        },

                        _ = &mut scheduler.misc_tasks => {}

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
        self.command_sender.blocking_send(SchedulerCommands::UpdateSettings(config)).unwrap();
        if let Err(e) = self.command_sender.blocking_send(SchedulerCommands::LoadNotebook(paths)) {
            panic!("Failed with {:?}", e);
        };
    }

    /// Checks for an update, panicing if the channel disconnected.
    pub fn check_update(&mut self) -> Option<SchedulerResponse> {
        match self.response_receiver.try_recv() {
            Ok(r) => Some(r),
            Err(mpsc::error::TryRecvError::Empty) => None,
            Err(mpsc::error::TryRecvError::Disconnected) => panic!("Thread Disconnected"),
        }
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
        let app_cache = Arc::new(RwLock::const_new(AppCache::default()));
        let loader_template = SingleNoteLoader::new(response_sender.clone(), app_cache.clone(), config.clone());
        Self {
            app_cache,
            app_cache_path: Arc::new(RwLock::const_new(None)),
            config,
            loaded_notebooks: Default::default(),
            loaded_titles: Default::default(),
            response_sender,
            loader_template,
            note_tasks: StreamGuard::new(),
            misc_tasks: StreamGuard::new(),
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
                misc_task!(self(app_cache, response_sender, app_cache_path) => {
                    use SchedulerResponse::CahceMessage as Msg;
                    let _ = app_cache_path.write().await.insert(path_buf.clone());
                    match AppCache::from_path(path_buf) {
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
                misc_task!(self(app_cache, loaded_titles, response_sender, loaded_notebooks, app_cache_path) => {
                    {
                        let mut c = app_cache.write().await;
                        titles.iter().for_each(|t| c.update_from_notebook(t));
                        loaded_titles.write().await.extend(
                            titles.into_iter().map(|t| (t.note_id, t))
                        );
                    }
                    let handle = tasks::export_notes(ids, export_settings, loaded_notebooks, loaded_titles, response_sender.clone());
                    
                    if let Some(p) = app_cache_path.read().await.as_ref() {
                        use SchedulerResponse::CahceMessage as Msg;

                        if let Err(e) = app_cache.read().await.save_to(p) {
                            use CacheMsg::FailedToSave as Fail;
                            let _ = response_sender.send(Msg(Fail(e.to_string()))).await;
                        } else {
                            let _ = response_sender.send(Msg(CacheMsg::Saved)).await;
                        }
                    } else {
                        use SchedulerResponse::CahceMessage as Msg;
                        let _ = response_sender.send(Msg(CacheMsg::FailedToSave(
                            "No settings were sent".to_string()
                        ))).await;
                    }
                    handle.join().unwrap()
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
}

impl<T: Future> StreamGuard<T> {
    fn new() -> Self {
        Self { tsk: FuturesUnordered::new(), wk: None }
    }

    /// Pushes a [Future] to the internal
    /// [FuturesUnordered] and wakes the [Waker](std::task::Waker)
    #[inline]
    fn push(&mut self, value: T) {
        self.tsk.push(value);
        if let Some(wk) = self.wk.as_ref() {
            wk.wake_by_ref();
        }
    }

    /// Will extend the internal [FuturesUnordered]
    /// and wake the [Waker](std::task::Waker).
    #[inline]
    fn extend<I: IntoIterator<Item = T>>(&mut self, paths: I) {
        self.tsk.extend(
            paths
        );
        if let Some(wk) = self.wk.as_ref() {
            wk.wake_by_ref();
        }
    }
}

impl<T: Future> Future for StreamGuard<T> {
    type Output = T::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        // Ensure we never poll when it's empty.
        if !self.tsk.is_empty() {
            self.tsk.poll_next_unpin(cx).map(Option::unwrap)
        } else {
            // We want to wake only when we have items.
            self.wk = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}
