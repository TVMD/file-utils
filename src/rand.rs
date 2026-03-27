use std::fs;
use std::io;
use std::path::Path;

use rand::seq::SliceRandom;
use rand::rng;
use regex::Regex;

/// Strip existing random prefix (digits followed by underscore at start of filename)
fn strip_prefix(name: &str) -> &str {
    let re = Regex::new(r"^\d+_").unwrap();
    match re.find(name) {
        Some(m) => &name[m.end()..],
        None => name,
    }
}

pub fn run(dir: &Path, dry_run: bool, clear: bool) -> io::Result<()> {
    let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());

    let entries: Vec<_> = fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            e.file_type().map(|t| t.is_file()).unwrap_or(false)
                && !name.starts_with('.')
                && !name.starts_with("._")
        })
        .collect();

    if entries.is_empty() {
        println!("No files found in {}", dir.display());
        return Ok(());
    }

    if clear {
        let mut count = 0;
        for entry in &entries {
            let name = entry.file_name().to_string_lossy().to_string();
            let clean = strip_prefix(&name);
            if clean != name {
                let new_path = dir.join(clean);
                if dry_run {
                    println!("  {} -> {}", name, clean);
                } else {
                    fs::rename(entry.path(), &new_path)?;
                    println!("  \x1b[32m{}\x1b[0m -> \x1b[36m{}\x1b[0m", name, clean);
                }
                count += 1;
            }
        }
        if count == 0 {
            println!("No prefixed files found.");
        } else if dry_run {
            println!("\n{} file(s) would be renamed. Use without --dry-run to apply.", count);
        } else {
            println!("\nCleared prefix from {} file(s).", count);
        }
        return Ok(());
    }

    // Phase 1: Strip all existing prefixes first (rename to clean names via temp)
    let mut clean_files: Vec<(std::path::PathBuf, String)> = Vec::new();

    for entry in &entries {
        let name = entry.file_name().to_string_lossy().to_string();
        let clean = strip_prefix(&name).to_string();

        if clean != name && !dry_run {
            // Rename to temp first to avoid collisions
            let tmp_name = format!(".fus_strip_{}", clean);
            let tmp_path = dir.join(&tmp_name);
            fs::rename(entry.path(), &tmp_path)?;
            clean_files.push((tmp_path, clean));
        } else if clean != name {
            clean_files.push((entry.path(), clean));
        } else {
            clean_files.push((entry.path(), clean));
        }
    }

    // Phase 1b: Move temp files to clean names
    if !dry_run {
        for (path, clean) in &clean_files {
            let fname = path.file_name().unwrap().to_string_lossy();
            if fname.starts_with(".fus_strip_") {
                let clean_path = dir.join(clean);
                fs::rename(path, &clean_path)?;
            }
        }
        // Update paths to clean names
        for (path, clean) in &mut clean_files {
            *path = dir.join(&*clean);
        }
    }

    let count = clean_files.len();
    let width = format!("{}", count).len();

    // Generate shuffled indices
    let mut indices: Vec<usize> = (1..=count).collect();
    indices.shuffle(&mut rng());

    // Sort by clean name for consistent display
    clean_files.sort_by(|a, b| a.1.cmp(&b.1));

    println!("Randomizing {} files in {}\n", count, dir.display());

    // Phase 2: Rename all to temp names with prefix
    let mut renames: Vec<(std::path::PathBuf, String)> = Vec::new();
    for (i, (src_path, clean_name)) in clean_files.iter().enumerate() {
        let new_name = format!("{:0width$}_{}", indices[i], clean_name, width = width);
        renames.push((src_path.clone(), new_name));
    }

    if dry_run {
        for (src_path, new_name) in &renames {
            let old_name = src_path.file_name().unwrap().to_string_lossy();
            println!("  {} -> {}", old_name, new_name);
        }
        println!("\nDry run — no files were renamed. Remove --dry-run to apply.");
    } else {
        // Two-phase rename to avoid collisions:
        // First rename all to temp names
        let mut temp_paths: Vec<(std::path::PathBuf, String)> = Vec::new();
        for (src_path, new_name) in &renames {
            let tmp_name = format!(".fus_tmp_{}", new_name);
            let tmp_path = dir.join(&tmp_name);
            fs::rename(src_path, &tmp_path)?;
            temp_paths.push((tmp_path, new_name.clone()));
        }
        // Then rename from temp to final
        for (i, (tmp_path, new_name)) in temp_paths.iter().enumerate() {
            let final_path = dir.join(new_name);
            fs::rename(tmp_path, &final_path)?;
            let old_name = clean_files[i].1.as_str();
            println!("  \x1b[33m{}\x1b[0m -> \x1b[32m{}\x1b[0m", old_name, new_name);
        }
        println!("\nDone! Randomized {} file(s).", count);
    }

    Ok(())
}
