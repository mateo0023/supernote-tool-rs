// #![windows_subsystem = "windows"]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] 
#[cfg(feature = "gui")]
fn main() {
    supernote_tool_rs::start_app()
}

#[cfg(not(feature = "gui"))]
fn main() {
    use clap::Parser;
    use supernote_tool_rs::command_line::Args;
    use supernote_tool_rs::{sync_work, ServerConfig, AppCache};
    let Args { input: paths, merge, app_cache, config, export } = Args::parse();
    let config = match config {
        Some(p) => ServerConfig::from_path_or_default(p),
        None => ServerConfig::default(),
    };
    let cache = app_cache.and_then(|p| AppCache::from_path(p).ok());
    let errs = sync_work(paths, cache, config, merge, export)
        .into_iter().enumerate().filter_map(|(idx, r)| {
            match r {
                Ok(_) => None,
                Err(e) => Some(format!("{}.\t{}\n", idx, e)),
            }
        }).collect::<String>();
    if errs.is_empty() {
        println!("Succesfully exported all files");
    } else {
        print!("There were some errors exporing the notebooks:\n{}", errs);
    }
}