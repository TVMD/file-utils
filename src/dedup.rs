use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

fn hash_file(path: &Path) -> io::Result<String> {
    let data = fs::read(path)?;
    let hash = Sha256::digest(&data);
    Ok(format!("{:x}", hash))
}

/// Score a filename — lower score = more likely the "original".
/// Files with copy patterns like "(2)", " copy" get a higher score.
fn copy_score(name: &str) -> u32 {
    let lower = name.to_lowercase();
    if is_copy_pattern(&lower) {
        return 1;
    }
    0
}

fn is_copy_pattern(name: &str) -> bool {
    // "file (2).ext", "file (3).ext", etc.
    if name.contains(" (") && name.contains(')') {
        let start = name.rfind(" (").unwrap();
        let end = name[start..].find(')').unwrap() + start;
        let inner = &name[start + 2..end];
        if inner.parse::<u32>().is_ok() || inner == "copy" {
            return true;
        }
    }
    // " copy" or " - copy" patterns
    if name.contains(" copy") || name.contains(" - copy") {
        return true;
    }
    false
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

pub fn run(dir: &Path, delete: bool) -> io::Result<()> {
    if !dir.is_dir() {
        eprintln!("Error: {} is not a directory", dir.display());
        std::process::exit(1);
    }

    // Group files by size first to avoid hashing everything
    let mut size_groups: HashMap<u64, Vec<PathBuf>> = HashMap::new();

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let meta = fs::metadata(&path)?;
        size_groups.entry(meta.len()).or_default().push(path);
    }

    // Only hash files that share a size with at least one other file
    let mut hash_groups: HashMap<String, Vec<PathBuf>> = HashMap::new();

    for paths in size_groups.values() {
        if paths.len() < 2 {
            continue;
        }
        for path in paths {
            match hash_file(path) {
                Ok(hash) => hash_groups.entry(hash).or_default().push(path.clone()),
                Err(e) => eprintln!("Warning: could not read {}: {}", path.display(), e),
            }
        }
    }

    let mut groups: Vec<Vec<PathBuf>> = Vec::new();

    for (_hash, mut paths) in hash_groups {
        if paths.len() < 2 {
            continue;
        }
        // Sort: lowest copy_score first (original), then alphabetically
        paths.sort_by(|a, b| {
            let name_a = a.file_name().unwrap_or_default().to_string_lossy();
            let name_b = b.file_name().unwrap_or_default().to_string_lossy();
            let sa = copy_score(&name_a);
            let sb = copy_score(&name_b);
            sa.cmp(&sb).then_with(|| a.cmp(b))
        });
        groups.push(paths);
    }

    if groups.is_empty() {
        println!("No duplicates found.");
        return Ok(());
    }

    let mut total_duplicates = 0;

    for (i, paths) in groups.iter().enumerate() {
        if i > 0 {
            println!();
        }
        let keep = &paths[0];
        let file_size = fs::metadata(keep).map(|m| m.len()).unwrap_or(0);
        println!("Group {} ({}):", i + 1, format_size(file_size));
        println!("  \x1b[32m✓ Keep:      {}\x1b[0m", keep.display());
        for dup in &paths[1..] {
            println!("  \x1b[31m✗ Duplicate: {}\x1b[0m", dup.display());
            total_duplicates += 1;
        }
    }

    let total_wasted: u64 = groups.iter().map(|paths| {
        let size = fs::metadata(&paths[0]).map(|m| m.len()).unwrap_or(0);
        size * (paths.len() as u64 - 1)
    }).sum();

    println!("\n{} duplicate(s) in {} group(s), {} wasted.",
        total_duplicates, groups.len(), format_size(total_wasted));

    if delete {
        for paths in &groups {
            for dup in &paths[1..] {
                match fs::remove_file(dup) {
                    Ok(()) => println!("Deleted: {}", dup.display()),
                    Err(e) => eprintln!("Error deleting {}: {}", dup.display(), e),
                }
            }
        }
        println!("Done. Deleted {} file(s), freed {}.", total_duplicates, format_size(total_wasted));
    } else {
        println!("Dry run — use --delete to actually remove files.");
    }

    Ok(())
}
