//! Stores the items necessary for saving the settings.

use serde::{Serialize, Deserialize};
use std::{collections::HashMap, error::Error, path::PathBuf};

use super::{Notebook, Title, Transciption};

/// Is what's mapped within each
/// [notebook's cache](AppCache::notebooks).
/// 
/// Maps from [`title.hash`](Title::hash) to
/// [`TitleCache`].
pub type NotebookCache = HashMap<u64, TitleCache>;

/// Will hold the settings for all the notebooks.
/// 
/// Maps the [`notebook_id`](Notebook::file_id) to the 
/// map between [`Title::hash`](super::Title::hash) and [`TitleCache`].
#[derive(Default, Serialize, Deserialize)]
pub struct AppCache {
    /// Maps from [file_id](Notebook::file_id) to [NotebookCache].
    pub notebooks: HashMap<String, NotebookCache>,
    /// Wether to combina all the [Notebook]s into 
    /// a single pdf or export them separately.
    pub combine_pdfs: bool,
}

 /// The old version of the [AppCache]
#[derive(Deserialize)]
struct AppCacheV1 {
    /// Maps between file_id and Title Cache
    pub notebooks: HashMap<String, HashMap<u64, TitleCacheV1>>,
    /// Wether to combina all the [Notebook]s into 
    /// a single pdf or export them separately.
    pub combine_pdfs: bool,
}

/// Will be used to store the relevant information
/// on the title. Will check for page_id and location
/// of the title only.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct TitleCache {
    /// The corrected title.
    pub title: Transciption,
    /// The Page Id from the Notebook
    pub page_id: String,
    /// The hash value of the [content](Title::content).
    pub hash: u64,
}

/// Old version of [TitleCache]
#[derive(Deserialize)]
struct TitleCacheV1 {
    /// The corrected title.
    pub title: Option<String>,
    /// The Page Id from the Notebook
    pub page_id: String,
    /// The hash value of the [content](Title::content).
    pub hash: u64,
}

impl AppCache {
    /// Load an AppCache from a path and merge it into itself.
    pub fn merge_from_path(&mut self, path: &PathBuf) -> Result<(), Box<dyn Error>> {
        let cache: Self = match serde_json::from_reader(std::fs::File::open(path)?) {
            Ok(c) => c,
            Err(_) => serde_json::from_reader::<_, AppCacheV1>(std::fs::File::open(path)?)?.into(),
        };
        
        self.combine_pdfs = cache.combine_pdfs;

        for (note_id, titles) in cache.notebooks {
            // Either add new title settings or update
            // the existing one.
            match self.notebooks.contains_key(&note_id) {
                true => if let Some(old_titles) = self.notebooks.insert(note_id.clone(), titles) {
                    let new_titles = self.notebooks.get_mut(&note_id).unwrap();
                    TitleCache::merge_list_into(new_titles, old_titles);
                },
                false => {self.notebooks.insert(note_id, titles);},
            }
        }

        Ok(())
    }

    /// Replaces the Cache data at the key ([file_id](Notebook::file_id) by the new
    /// [TitleCache]
    pub fn update(&mut self, k: String, v: NotebookCache) {
        self.notebooks.insert(k, v);
    }

    /// It updates the cached titles in the [notebook](Notebook) and removes
    /// the ones no longer existing from [AppCache].
    pub fn sync_w_notebook(&mut self, notebook: &mut Notebook) {
        if let Some(old_cache) = self.notebooks.get_mut(&notebook.file_id) {
            old_cache.retain(|k, c| match notebook.titles.contains_key(k) {
                true => {
                    notebook.update_title(*k, &c.title);
                    true
                },
                false => false,
            });
        } else {
            self.notebooks.insert(notebook.file_id.clone(), HashMap::new());
        }
    }

    /// Replaces the existing cache with [Notebook::get_cache()]
    pub fn update_from_notebook(&mut self, notebook: &Notebook) {
        if let Some(old_cache) = self.notebooks.get_mut(&notebook.file_id) {
            *old_cache = notebook.get_cache();
        } else {
            self.notebooks.insert(notebook.file_id.clone(), notebook.get_cache());
        }
    }

    /// Save to the given path, if any
    pub fn save_to(&self, path: &PathBuf) -> Result<(), Box<dyn Error>> {
        let f = std::fs::File::create(path)?;
        serde_json::to_writer(f, self)?;
        Ok(())
    }

    pub fn update_title(&mut self, file_id: &str, title: TitleCache) {
        if let Some(map) = self.notebooks.get_mut(file_id){ 
            map.insert(title.hash, title);
        }
    }

}

impl TitleCache {
    pub fn form_title(title: &Title, page_id: String) -> Option<Self> {
        title.name.get_clone_for_cache()
            .map(|transcription| TitleCache {
                title: transcription,
                page_id,
                hash: title.hash,
            })
    }

    /// Will merge the titles that are both in the receiver and donor lists.
    /// 
    /// If the title is:
    /// * Only in the `receiver`, it is left alone.
    /// * Only in the `donor`, it is ignored.
    /// * In both, the `donnor` is merged into the `receiver`. See [Self::merge_into]
    pub fn merge_list_into(receiver: &mut NotebookCache, donor: NotebookCache) {
        for (hash, old) in donor {
            if let Some(r) = receiver.get_mut(&hash) {
                r.merge_into(old);
            }
        }
    }

    /// Will update the [title](Self::title) if it is [None] and
    /// the other contains a [title](Self::title) (is [Some]).
    fn merge_into(&mut self, other: TitleCache) {
        self.title.merge_into(other.title);
    }
}

impl From<AppCacheV1> for AppCache {
    fn from(value: AppCacheV1) -> Self {
        let mut notebooks = HashMap::with_capacity(value.notebooks.capacity());
        for (k, notebook) in value.notebooks.into_iter() {
            let mut v = HashMap::with_capacity(notebook.capacity());
            for (hash, title) in notebook.into_iter() {
                v.insert(hash, title.into());
            }
            notebooks.insert(k, v);
        }
        AppCache {
            notebooks,
            combine_pdfs: value.combine_pdfs,
        }
    }
}

impl From<TitleCacheV1> for TitleCache {
    fn from(value: TitleCacheV1) -> Self {
        TitleCache {
            title: match value.title {
                Some(txt) => Transciption::Manual(txt),
                None => Transciption::None,
            },
            page_id: value.page_id,
            hash: value.hash,
        }
    }
}
