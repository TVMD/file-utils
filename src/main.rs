mod dedup;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "fus", about = "A collection of file utility tools")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Find and delete duplicate files based on content
    Dedup {
        /// Directory to scan for duplicates
        #[arg(default_value = ".")]
        dir: std::path::PathBuf,

        /// Actually delete files (without this flag, only prints what would be deleted)
        #[arg(long)]
        delete: bool,
    },
}

fn main() -> std::io::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Dedup { dir, delete } => dedup::run(&dir, delete),
    }
}
