# fus

A fast file utility toolkit written in Rust.

## Install

### From release (recommended)

```bash
curl -sSL https://raw.githubusercontent.com/ziaminhta/fus/main/install.sh | bash
```

### From source

```bash
git clone https://github.com/ziaminhta/fus.git
cd fus
cargo install --path .
```

### With cargo

```bash
cargo install --git https://github.com/ziaminhta/fus.git
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

## Release

Push a version tag to trigger a GitHub Actions build for Linux and macOS (x86_64 + aarch64):

```bash
git tag v0.1.0
git push origin v0.1.0
```
