mod dedup;

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
}

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Dedup { dir, delete, interactive, threshold } => {
            dedup::run(&dir, delete, interactive, threshold)
        }
    }
}
