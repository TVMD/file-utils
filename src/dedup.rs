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

pub fn run(dir: &Path, delete: bool) -> io::Result<()> {
    if !dir.is_dir() {
        eprintln!("Error: {:?} is not a directory", dir);
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

    for (_size, paths) in &size_groups {
        if paths.len() < 2 {
            continue;
        }
        for path in paths {
            match hash_file(path) {
                Ok(hash) => hash_groups.entry(hash).or_default().push(path.clone()),
                Err(e) => eprintln!("Warning: could not read {:?}: {}", path, e),
            }
        }
    }

    let mut to_delete: Vec<PathBuf> = Vec::new();

    for (_hash, mut paths) in hash_groups {
        if paths.len() < 2 {
            continue;
        }
        // Sort: lowest copy_score first (original), then alphabetically
        paths.sort_by(|a, b| {
            let sa = copy_score(a.file_name().unwrap().to_str().unwrap_or(""));
            let sb = copy_score(b.file_name().unwrap().to_str().unwrap_or(""));
            sa.cmp(&sb).then_with(|| a.cmp(b))
        });

        let keep = &paths[0];
        println!("Keep: {:?}", keep);
        for dup in &paths[1..] {
            println!("  Duplicate: {:?}", dup);
            to_delete.push(dup.clone());
        }
    }

    if to_delete.is_empty() {
        println!("No duplicates found.");
        return Ok(());
    }

    println!("\nFound {} duplicate(s).", to_delete.len());

    if delete {
        for path in &to_delete {
            match fs::remove_file(path) {
                Ok(()) => println!("Deleted: {:?}", path),
                Err(e) => eprintln!("Error deleting {:?}: {}", path, e),
            }
        }
        println!("Done. Deleted {} file(s).", to_delete.len());
    } else {
        println!("Dry run — use --delete to actually remove files.");
    }

    Ok(())
}
