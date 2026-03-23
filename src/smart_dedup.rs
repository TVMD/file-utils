use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use strsim::jaro_winkler;
use unicode_normalization::UnicodeNormalization;

/// Normalize a filename for comparison:
/// - Strip extension
/// - Lowercase
/// - Remove diacritics (Vietnamese, accented chars)
/// - Remove copy patterns like (1), (2), (copy), " - Copy"
/// - Collapse whitespace and special chars
fn normalize_name(name: &str) -> String {
    let stem = match name.rfind('.') {
        Some(i) if i > 0 => &name[..i],
        _ => name,
    };
    let lower = stem.to_lowercase();
    let no_diacritics: String = lower
        .nfd()
        .filter(|c| !('\u{0300}'..='\u{036F}').contains(c))
        .collect();
    let cleaned = remove_copy_patterns(&no_diacritics);
    let normalized: String = cleaned
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn remove_copy_patterns(s: &str) -> String {
    let mut result = s.to_string();
    while let Some(start) = result.find(" (") {
        if let Some(end_offset) = result[start..].find(')') {
            let end = start + end_offset;
            let inner = &result[start + 2..end];
            if inner.parse::<u32>().is_ok() || inner == "copy" {
                result = format!("{}{}", &result[..start], &result[end + 1..]);
                continue;
            }
        }
        break;
    }
    for pattern in &[" - copy", " copy", "_copy", "-copy"] {
        if let Some(pos) = result.find(pattern) {
            result = result[..pos].to_string();
        }
    }
    result
}

/// Score how "original" a filename looks. Lower = more original.
fn originality_score(name: &str) -> u32 {
    let lower = name.to_lowercase();
    let mut score = 0;
    if lower.contains(" (") && lower.contains(')') {
        let start = lower.rfind(" (").unwrap();
        let end = lower[start..].find(')').unwrap() + start;
        let inner = &lower[start + 2..end];
        if inner.parse::<u32>().is_ok() || inner == "copy" {
            score += 10;
        }
    }
    if lower.contains(" copy") || lower.contains(" - copy") || lower.contains("_copy") {
        score += 10;
    }
    score += (name.len() as u32) / 10;
    score
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

struct UnionFind {
    parent: Vec<usize>,
    rank: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            rank: vec![0; n],
        }
    }

    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }

    fn union(&mut self, x: usize, y: usize) {
        let rx = self.find(x);
        let ry = self.find(y);
        if rx == ry {
            return;
        }
        match self.rank[rx].cmp(&self.rank[ry]) {
            std::cmp::Ordering::Less => self.parent[rx] = ry,
            std::cmp::Ordering::Greater => self.parent[ry] = rx,
            std::cmp::Ordering::Equal => {
                self.parent[ry] = rx;
                self.rank[rx] += 1;
            }
        }
    }
}

type GroupResult = (Vec<PathBuf>, Vec<String>, Vec<Vec<usize>>);

/// A file entry with its action (keep or delete)
struct FileEntry {
    path: PathBuf,
    action: char, // 'k' = keep, 'd' = delete
}

/// Find similar file groups. Returns (files, normalized_names, groups).
fn find_groups(
    dir: &Path,
    threshold: f64,
) -> io::Result<Option<GroupResult>> {
    if !dir.is_dir() {
        eprintln!("Error: {} is not a directory", dir.display());
        std::process::exit(1);
    }

    let mut files: Vec<PathBuf> = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            files.push(path);
        }
    }

    if files.len() < 2 {
        println!("Not enough files to compare.");
        return Ok(None);
    }

    let normalized: Vec<String> = files
        .iter()
        .map(|f| {
            let name = f.file_name().unwrap_or_default().to_string_lossy();
            normalize_name(&name)
        })
        .collect();

    let extensions: Vec<String> = files
        .iter()
        .map(|f| {
            f.extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
        })
        .collect();

    let mut uf = UnionFind::new(files.len());
    for i in 0..files.len() {
        for j in (i + 1)..files.len() {
            if extensions[i] != extensions[j] {
                continue;
            }
            let sim = jaro_winkler(&normalized[i], &normalized[j]);
            if sim >= threshold {
                uf.union(i, j);
            }
        }
    }

    let mut group_map: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..files.len() {
        let root = uf.find(i);
        group_map.entry(root).or_default().push(i);
    }

    let mut groups: Vec<Vec<usize>> = group_map
        .into_values()
        .filter(|g| g.len() >= 2)
        .collect();
    groups.sort_by_key(|g| g[0]);

    if groups.is_empty() {
        println!("No similar files found (threshold: {:.0}%).", threshold * 100.0);
        return Ok(None);
    }

    // Sort within each group: most "original" first
    for group in &mut groups {
        group.sort_by(|&a, &b| {
            let name_a = files[a].file_name().unwrap_or_default().to_string_lossy().to_string();
            let name_b = files[b].file_name().unwrap_or_default().to_string_lossy().to_string();
            let sa = originality_score(&name_a);
            let sb = originality_score(&name_b);
            sa.cmp(&sb).then_with(|| name_a.cmp(&name_b))
        });
    }

    Ok(Some((files, normalized, groups)))
}

/// Build the interactive editor content
fn build_editor_content(
    files: &[PathBuf],
    normalized: &[String],
    groups: &[Vec<usize>],
) -> String {
    let mut content = String::new();

    content.push_str("# Smart Dedup — Interactive Mode\n");
    content.push_str("#\n");
    content.push_str("# Commands:\n");
    content.push_str("#   k = keep this file\n");
    content.push_str("#   d = delete this file\n");
    content.push_str("#\n");
    content.push_str("# Lines starting with # are comments and will be ignored.\n");
    content.push_str("# Save and close the editor to proceed.\n");
    content.push_str("# Delete ALL lines or leave only comments to abort.\n");
    content.push_str("#\n\n");

    for (i, group) in groups.iter().enumerate() {
        let keep_idx = group[0];
        let file_size = fs::metadata(&files[keep_idx]).map(|m| m.len()).unwrap_or(0);
        content.push_str(&format!(
            "# ── Group {} ({} files, {}) ──\n",
            i + 1,
            group.len(),
            format_size(file_size)
        ));

        for (j, &idx) in group.iter().enumerate() {
            let name = files[idx]
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let sim = if j == 0 {
                100.0
            } else {
                jaro_winkler(&normalized[keep_idx], &normalized[idx]) * 100.0
            };

            // First file in group = keep, rest = delete (preselected)
            let action = if j == 0 { 'k' } else { 'd' };
            content.push_str(&format!("{} {} # {:.0}% match\n", action, name, sim));
        }
        content.push('\n');
    }

    content
}

/// Parse the editor output and return list of (path, action) pairs
fn parse_editor_output(
    content: &str,
    files: &[PathBuf],
    dir: &Path,
) -> Vec<FileEntry> {
    let mut entries = Vec::new();

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Parse: "k filename # comment" or "d filename # comment"
        let action = match trimmed.chars().next() {
            Some('k') | Some('K') => 'k',
            Some('d') | Some('D') => 'd',
            _ => continue,
        };

        // Get filename: everything after the action char and space, before " #"
        let rest = trimmed[1..].trim_start();
        let filename = if let Some(comment_pos) = rest.find(" # ") {
            &rest[..comment_pos]
        } else {
            rest
        };

        let filename = filename.trim();
        if filename.is_empty() {
            continue;
        }

        // Find the matching file
        let path = dir.join(filename);
        if path.exists() {
            entries.push(FileEntry {
                path,
                action,
            });
        } else {
            // Try to match by filename against known files
            if let Some(matched) = files.iter().find(|f| {
                f.file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    == filename
            }) {
                entries.push(FileEntry {
                    path: matched.clone(),
                    action,
                });
            } else {
                eprintln!("Warning: file not found, skipping: {}", filename);
            }
        }
    }

    entries
}

/// Open $EDITOR with the content and return the edited result
fn open_editor(content: &str) -> io::Result<String> {
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vim".to_string());

    let tmp_dir = std::env::temp_dir();
    let tmp_path = tmp_dir.join("fus-smart-dedup.txt");

    // Write content to temp file
    {
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(content.as_bytes())?;
    }

    // Open editor
    let status = Command::new(&editor)
        .arg(&tmp_path)
        .status()
        .map_err(|e| io::Error::other(format!("Failed to open editor '{}': {}", editor, e)))?;

    if !status.success() {
        return Err(io::Error::other("Editor exited with non-zero status"));
    }

    // Read back
    let result = fs::read_to_string(&tmp_path)?;

    // Clean up
    let _ = fs::remove_file(&tmp_path);

    Ok(result)
}

/// Interactive mode: open editor, let user choose k/d, then execute
fn run_interactive(
    dir: &Path,
    files: &[PathBuf],
    normalized: &[String],
    groups: &[Vec<usize>],
) -> io::Result<()> {
    let content = build_editor_content(files, normalized, groups);
    let edited = open_editor(&content)?;

    // Check if user aborted (empty or only comments)
    let has_actions = edited
        .lines()
        .any(|l| {
            let t = l.trim();
            !t.is_empty() && !t.starts_with('#')
        });

    if !has_actions {
        println!("Aborted — no changes made.");
        return Ok(());
    }

    let entries = parse_editor_output(&edited, files, dir);

    let to_delete: Vec<&FileEntry> = entries.iter().filter(|e| e.action == 'd').collect();
    let to_keep: Vec<&FileEntry> = entries.iter().filter(|e| e.action == 'k').collect();

    if to_delete.is_empty() {
        println!("Nothing to delete.");
        return Ok(());
    }

    // Show summary
    println!("\n\x1b[32m✓ Keeping {} file(s)\x1b[0m", to_keep.len());
    for entry in &to_keep {
        println!(
            "  \x1b[32m  {}\x1b[0m",
            entry.path.file_name().unwrap_or_default().to_string_lossy()
        );
    }

    println!("\x1b[31m✗ Deleting {} file(s)\x1b[0m", to_delete.len());
    for entry in &to_delete {
        println!(
            "  \x1b[31m  {}\x1b[0m",
            entry.path.file_name().unwrap_or_default().to_string_lossy()
        );
    }

    // Delete
    let mut deleted = 0;
    let mut freed: u64 = 0;
    for entry in &to_delete {
        let size = fs::metadata(&entry.path).map(|m| m.len()).unwrap_or(0);
        match fs::remove_file(&entry.path) {
            Ok(()) => {
                deleted += 1;
                freed += size;
            }
            Err(e) => eprintln!("Error deleting {}: {}", entry.path.display(), e),
        }
    }

    println!(
        "\nDone. Deleted {} file(s), freed {}.",
        deleted,
        format_size(freed)
    );

    Ok(())
}

/// Non-interactive mode: just print or delete
fn run_non_interactive(
    files: &[PathBuf],
    normalized: &[String],
    groups: &[Vec<usize>],
    delete: bool,
) -> io::Result<()> {
    let mut total_duplicates = 0;

    for (i, group) in groups.iter().enumerate() {
        if i > 0 {
            println!();
        }

        let keep_idx = group[0];
        let file_size = fs::metadata(&files[keep_idx]).map(|m| m.len()).unwrap_or(0);
        println!("Group {} ({} files, {}):", i + 1, group.len(), format_size(file_size));

        let keep_name = files[keep_idx]
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        println!("  \x1b[32m✓ Keep:   {}\x1b[0m", keep_name);

        for &idx in &group[1..] {
            let dup_name = files[idx]
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let sim = jaro_winkler(&normalized[keep_idx], &normalized[idx]);
            println!(
                "  \x1b[31m✗ Remove: {} ({:.0}% similar)\x1b[0m",
                dup_name,
                sim * 100.0
            );
            total_duplicates += 1;
        }
    }

    let total_wasted: u64 = groups
        .iter()
        .map(|group| {
            let size = fs::metadata(&files[group[0]]).map(|m| m.len()).unwrap_or(0);
            size * (group.len() as u64 - 1)
        })
        .sum();

    println!(
        "\n{} similar duplicate(s) in {} group(s), ~{} wasted.",
        total_duplicates,
        groups.len(),
        format_size(total_wasted)
    );

    if delete {
        let mut deleted = 0;
        let mut freed: u64 = 0;
        for group in groups {
            for &idx in &group[1..] {
                let size = fs::metadata(&files[idx]).map(|m| m.len()).unwrap_or(0);
                match fs::remove_file(&files[idx]) {
                    Ok(()) => {
                        println!("Deleted: {}", files[idx].display());
                        deleted += 1;
                        freed += size;
                    }
                    Err(e) => eprintln!("Error deleting {}: {}", files[idx].display(), e),
                }
            }
        }
        println!(
            "Done. Deleted {} file(s), freed {}.",
            deleted,
            format_size(freed)
        );
    } else {
        println!("Dry run — use --delete to remove, or -i for interactive editor.");
    }

    Ok(())
}

pub fn run(dir: &Path, delete: bool, interactive: bool, threshold: f64) -> io::Result<()> {
    let result = find_groups(dir, threshold)?;
    let (files, normalized, groups) = match result {
        Some(data) => data,
        None => return Ok(()),
    };

    if interactive {
        run_interactive(dir, &files, &normalized, &groups)
    } else {
        run_non_interactive(&files, &normalized, &groups, delete)
    }
}
