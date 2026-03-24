use std::fs;
use std::io;
use std::path::Path;

use rand::seq::SliceRandom;
use rand::rng;
use regex::Regex;

/// Strip existing random prefix (digits followed by underscore at start of filename)
fn strip_prefix(name: &str) -> &str {
    let re = Regex::new(r"^\d+_").unwrap();
    if re.is_match(name) {
        &name[re.find(name).unwrap().end()..]
    } else {
        name
    }
}

pub fn run(dir: &Path, dry_run: bool, clear: bool) -> io::Result<()> {
    let entries: Vec<_> = fs::read_dir(dir)?
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
        // Just remove prefixes, no randomization
        let mut count = 0;
        for entry in &entries {
            let name = entry.file_name().to_string_lossy().to_string();
            let clean = strip_prefix(&name);
            if clean != name {
                let new_path = entry.path().with_file_name(clean);
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

    // Collect clean names (strip existing prefix if any)
    let mut files: Vec<(std::path::PathBuf, String)> = entries
        .iter()
        .map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let clean = strip_prefix(&name).to_string();
            (e.path(), clean)
        })
        .collect();

    let count = files.len();
    let width = format!("{}", count).len();

    // Generate shuffled indices
    let mut indices: Vec<usize> = (1..=count).collect();
    indices.shuffle(&mut rng());

    // Sort files by clean name for consistent display
    files.sort_by(|a, b| a.1.cmp(&b.1));

    println!("Randomizing {} files in {}\n", count, dir.display());

    for (i, (old_path, clean_name)) in files.iter().enumerate() {
        let new_name = format!("{:0width$}_{}", indices[i], clean_name, width = width);
        let new_path = old_path.with_file_name(&new_name);

        if dry_run {
            let old_name = old_path.file_name().unwrap().to_string_lossy();
            println!("  {} -> {}", old_name, new_name);
        } else {
            // Use a temp name to avoid collisions during rename
            let tmp_name = format!(".fus_tmp_{}", new_name);
            let tmp_path = old_path.with_file_name(&tmp_name);
            fs::rename(old_path, &tmp_path)?;
            fs::rename(&tmp_path, &new_path)?;
            let old_name = old_path.file_name().unwrap().to_string_lossy();
            println!("  \x1b[33m{}\x1b[0m -> \x1b[32m{}\x1b[0m", old_name, new_name);
        }
    }

    if dry_run {
        println!("\nDry run — no files were renamed. Remove --dry-run to apply.");
    } else {
        println!("\nDone! Randomized {} file(s).", count);
    }

    Ok(())
}
