use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
#[command(name = "Supernote Tool Rust")]
#[command(version)]
#[command(
    about = "Loads input .note files and exports to PDF",
    long_about = None
)]
pub struct Args {
    /// The input files
    #[arg(short, long)]
    pub input: Vec<PathBuf>,
    /// Wether to merge the files or not.
    #[arg(short, long, default_value_t = false)]
    pub merge: bool,
    /// The path to the existing
    /// transcription settings
    #[arg(short = 't', long = "transcript")]
    pub app_cache: Option<PathBuf>,
    /// Path to the ServerConfig JSON file
    #[arg(short, long)]
    pub config: Option<PathBuf>,
    /// The path (to folder) to save the PDF
    #[arg(short, long)]
    pub export: PathBuf,
}