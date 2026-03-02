use anyhow::{Context, Result};
use std::{fs, path::Path};

use crate::collab::{
	protocol::{FileEntry, Manifest},
	state::CollabState,
};

/// Patterns for files/dirs to skip when building the manifest
static IGNORE_PATTERNS: &[&str] = &[
	".git",
	".vasc-collab-backup",
	"node_modules",
	".DS_Store",
	"Thumbs.db",
];

/// Recursively walk `root` and load all files into `state`, returning a Manifest.
pub fn build_manifest_from_dir(root: &Path, state: &mut CollabState) -> Result<Manifest> {
	let mut files: Vec<FileEntry> = Vec::new();
	walk_dir(root, root, state, &mut files)?;
	files.sort_by(|a, b| a.path.cmp(&b.path));
	let head_rev = state.global_seq;
	Ok(Manifest { files, head_rev })
}

fn walk_dir(root: &Path, dir: &Path, state: &mut CollabState, files: &mut Vec<FileEntry>) -> Result<()> {
	for entry in fs::read_dir(dir).with_context(|| format!("reading dir {}", dir.display()))? {
		let entry = entry?;
		let path = entry.path();
		let name = entry.file_name();
		let name_str = name.to_string_lossy();

		if IGNORE_PATTERNS.iter().any(|p| *p == name_str.as_ref()) {
			continue;
		}

		if path.is_dir() {
			walk_dir(root, &path, state, files)?;
		} else {
			let rel = path
				.strip_prefix(root)
				.with_context(|| "stripping path prefix")?;
			// Normalize to forward slashes for cross-platform consistency
			let rel_str = rel.to_string_lossy().replace('\\', "/");

			let content = fs::read(&path).with_context(|| format!("reading file {}", path.display()))?;
			let file_entry = state.apply_change(rel_str, content, None);
			files.push(file_entry);
		}
	}
	Ok(())
}

/// Materialize a set of `(relative_path, content)` pairs into `target_dir`,
/// creating any missing parent directories.
pub fn apply_snapshot(target_dir: &Path, files: &[(String, Vec<u8>)]) -> Result<()> {
	for (rel, content) in files {
		let dest = target_dir.join(rel.replace('/', std::path::MAIN_SEPARATOR_STR));
		if let Some(parent) = dest.parent() {
			fs::create_dir_all(parent).with_context(|| format!("creating dirs for {}", dest.display()))?;
		}
		fs::write(&dest, content).with_context(|| format!("writing file {}", dest.display()))?;
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::collab::state::CollabState;
	use std::fs;
	use tempfile::TempDir;

	fn make_temp_project() -> TempDir {
		let dir = TempDir::new().unwrap();
		let src = dir.path().join("src");
		fs::create_dir_all(&src).unwrap();
		fs::write(src.join("main.lua"), b"print('hello')").unwrap();
		fs::write(dir.path().join("default.project.json"), b"{}").unwrap();
		// This should be ignored
		fs::create_dir_all(dir.path().join(".git")).unwrap();
		dir
	}

	#[test]
	fn manifest_includes_expected_files() {
		let dir = make_temp_project();
		let mut state = CollabState::new();
		let manifest = build_manifest_from_dir(dir.path(), &mut state).unwrap();

		let paths: Vec<&str> = manifest.files.iter().map(|f| f.path.as_str()).collect();
		assert!(paths.contains(&"default.project.json"), "expected project file in manifest");
		assert!(paths.contains(&"src/main.lua"), "expected source file in manifest");
		// .git should be excluded
		assert!(!paths.iter().any(|p| p.starts_with(".git")), ".git should be ignored");
	}

	#[test]
	fn manifest_files_have_valid_hashes() {
		let dir = make_temp_project();
		let mut state = CollabState::new();
		let manifest = build_manifest_from_dir(dir.path(), &mut state).unwrap();

		for file in &manifest.files {
			assert_eq!(file.hash.len(), 16, "hash should be 16 hex chars");
			assert!(file.rev > 0, "revision should be positive");
		}
	}

	#[test]
	fn apply_snapshot_creates_files() {
		let target = TempDir::new().unwrap();
		let files = vec![
			("src/foo.lua".to_string(), b"return {}".to_vec()),
			("src/bar/baz.lua".to_string(), b"return nil".to_vec()),
		];
		apply_snapshot(target.path(), &files).unwrap();

		assert!(target.path().join("src/foo.lua").exists());
		assert!(target.path().join("src/bar/baz.lua").exists());
		assert_eq!(fs::read(target.path().join("src/foo.lua")).unwrap(), b"return {}");
	}
}
