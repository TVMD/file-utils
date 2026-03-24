mod dedup;
mod rand;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "fus", about = "A collection of file utility tools", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Find and delete duplicate files based on fuzzy name matching
    Dedup {
        /// Directory to scan
        #[arg(default_value = ".")]
        dir: std::path::PathBuf,

        /// Actually delete files (without this flag, only prints what would be deleted)
        #[arg(long)]
        delete: bool,

        /// Interactive mode — open editor to choose which files to keep/delete
        #[arg(short, long)]
        interactive: bool,

        /// Similarity threshold (0.0 - 1.0, default 0.8)
        #[arg(long, default_value = "0.8")]
        threshold: f64,
    },

    /// Randomize file order by adding number prefixes to filenames
    Rand {
        /// Directory to randomize
        #[arg(default_value = ".")]
        dir: std::path::PathBuf,

        /// Preview changes without renaming
        #[arg(long)]
        dry_run: bool,

        /// Remove number prefixes instead of randomizing
        #[arg(long)]
        clear: bool,
    },
}

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Dedup { dir, delete, interactive, threshold } => {
            dedup::run(&dir, delete, interactive, threshold)
        }
        Commands::Rand { dir, dry_run, clear } => {
            rand::run(&dir, dry_run, clear)
        }
    }
}
