# fus

A fast file utility toolkit written in Rust.

## Install

### From release (recommended)

```bash
curl -sSL https://raw.githubusercontent.com/TVMD/file-utils/main/install.sh | bash
```

### From source

```bash
git clone https://github.com/TVMD/file-utils.git
cd fus
cargo install --path .
```

### With cargo

```bash
cargo install --git https://github.com/TVMD/file-utils.git
```

## Commands

### `dedup` — Remove duplicate files

Finds duplicate files by comparing SHA-256 content hashes. Keeps the original and removes copies (files with patterns like `(2)`, `copy` in their names).

```bash
# Dry run — shows what would be deleted
fus dedup /path/to/folder

# Actually delete duplicates
fus dedup /path/to/folder --delete
```

### `smart-dedup` — Remove duplicates by fuzzy name matching

Uses AI-like fuzzy matching (Jaro-Winkler similarity) to find files with similar names — even with different casing, diacritics, or copy suffixes. Great for cleaning up music libraries, downloads folders, etc. Supports Vietnamese and other Unicode filenames.

```bash
# Dry run — shows what would be deleted
fus smart-dedup /path/to/folder

# Actually delete duplicates
fus smart-dedup /path/to/folder --delete

# Custom similarity threshold (default 0.8 = 80%)
fus smart-dedup /path/to/folder --threshold 0.9
```

## Release

Push a version tag to trigger a GitHub Actions build for Linux and macOS (x86_64 + aarch64):

```bash
git tag v0.1.0
git push origin v0.1.0
```
