use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use strsim::jaro_winkler;
use unicode_normalization::UnicodeNormalization;

/// Normalize a filename for comparison:
/// - Strip extension
/// - Lowercase
/// - Remove diacritics (Vietnamese, accented chars)
/// - Remove copy patterns like (1), (2), (copy), " - Copy"
/// - Collapse whitespace and special chars
fn normalize_name(name: &str) -> String {
    // Strip extension
    let stem = match name.rfind('.') {
        Some(i) if i > 0 => &name[..i],
        _ => name,
    };

    // Lowercase
    let lower = stem.to_lowercase();

    // Remove diacritics: NFD decompose, then strip combining marks (U+0300..U+036F)
    let no_diacritics: String = lower
        .nfd()
        .filter(|c| !('\u{0300}'..='\u{036F}').contains(c))
        .collect();

    // Remove copy patterns: (1), (2), (copy), " - copy", " copy", "_copy"
    let cleaned = remove_copy_patterns(&no_diacritics);

    // Replace non-alphanumeric with space, collapse multiple spaces
    let normalized: String = cleaned
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();

    // Collapse whitespace and trim
    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn remove_copy_patterns(s: &str) -> String {
    let mut result = s.to_string();

    // Remove " (N)" patterns
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

    // Remove common copy suffixes
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

    // Penalize copy patterns
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

    // Prefer shorter names (less clutter)
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

/// Union-Find structure for grouping similar files
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

pub fn run(dir: &Path, delete: bool, threshold: f64) -> io::Result<()> {
    if !dir.is_dir() {
        eprintln!("Error: {} is not a directory", dir.display());
        std::process::exit(1);
    }

    // Collect all files
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
        return Ok(());
    }

    // Normalize all names
    let normalized: Vec<String> = files
        .iter()
        .map(|f| {
            let name = f.file_name().unwrap_or_default().to_string_lossy();
            normalize_name(&name)
        })
        .collect();

    // Also group by extension — only compare files with same extension
    let extensions: Vec<String> = files
        .iter()
        .map(|f| {
            f.extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
        })
        .collect();

    // Pairwise similarity + union-find grouping
    let mut uf = UnionFind::new(files.len());

    for i in 0..files.len() {
        for j in (i + 1)..files.len() {
            // Only compare files with same extension
            if extensions[i] != extensions[j] {
                continue;
            }
            let sim = jaro_winkler(&normalized[i], &normalized[j]);
            if sim >= threshold {
                uf.union(i, j);
            }
        }
    }

    // Collect groups
    let mut group_map: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..files.len() {
        let root = uf.find(i);
        group_map.entry(root).or_default().push(i);
    }

    // Filter to groups with 2+ files
    let mut groups: Vec<Vec<usize>> = group_map
        .into_values()
        .filter(|g| g.len() >= 2)
        .collect();

    // Sort groups for consistent output
    groups.sort_by_key(|g| g[0]);

    if groups.is_empty() {
        println!("No similar files found (threshold: {:.0}%).", threshold * 100.0);
        return Ok(());
    }

    let mut total_duplicates = 0;

    for (i, group) in groups.iter_mut().enumerate() {
        // Sort within group: most "original" first
        group.sort_by(|&a, &b| {
            let name_a = files[a].file_name().unwrap_or_default().to_string_lossy().to_string();
            let name_b = files[b].file_name().unwrap_or_default().to_string_lossy().to_string();
            let sa = originality_score(&name_a);
            let sb = originality_score(&name_b);
            sa.cmp(&sb).then_with(|| name_a.cmp(&name_b))
        });

        if i > 0 {
            println!();
        }

        let keep_idx = group[0];
        let file_size = fs::metadata(&files[keep_idx]).map(|m| m.len()).unwrap_or(0);
        println!("Group {} ({} files, {}):", i + 1, group.len(), format_size(file_size));

        let keep_name = files[keep_idx].file_name().unwrap_or_default().to_string_lossy().to_string();
        println!("  \x1b[32m✓ Keep:   {}\x1b[0m", keep_name);

        for &idx in &group[1..] {
            let dup_name = files[idx].file_name().unwrap_or_default().to_string_lossy().to_string();
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
        for group in &groups {
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
        println!("Dry run — use --delete to actually remove files.");
    }

    Ok(())
}
