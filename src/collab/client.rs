use anyhow::{bail, Context, Result};
use colored::Colorize;
use log::info;
use std::{
	collections::{HashMap, HashSet},
	path::{Path, PathBuf},
	time::{Duration, SystemTime},
};

use crate::collab::{
	protocol::{AuthRequest, AuthResponse, ChangeProposal, ChangeResult, ChangesBatch, ChangesSinceQuery, FileContent, Manifest, PROTOCOL_VERSION},
	snapshot::apply_snapshot,
	state::hash_content,
};

// ------------------------------------------------------------------
// Join
// ------------------------------------------------------------------

/// Join a collab session hosted at `addr`.
///
/// Steps:
///  1. Warn about overwrite; optionally back up `target_dir`.
///  2. Authenticate with the host.
///  3. Download the manifest + all files, materialize into `target_dir`.
///  4. Enter event loop:
///     - Poll host for changes and apply them locally.
///     - Watch local files for changes and propose to host.
pub fn join_session(
	addr: String,
	token: Option<String>,
	target_dir: PathBuf,
	backup: bool,
) -> Result<()> {
	let base_url = format!("http://{addr}");

	// 1. Warn about overwrite
	crate::argon_warn!(
		"WARNING: the directory '{}' will be overwritten with the host's project.\n\
		 Any existing content not present on the host WILL BE LOST.",
		target_dir.display()
	);

	// 2. Optional backup
	if backup && target_dir.exists() {
		let timestamp = chrono::Local::now().format("%Y%m%dT%H%M%S");
		let backup_dir = target_dir
			.parent()
			.unwrap_or(Path::new("."))
			.join(".vasc-collab-backup")
			.join(timestamp.to_string());

		info!("Backing up '{}' to '{}'", target_dir.display(), backup_dir.display());
		copy_dir_all(&target_dir, &backup_dir)?;
		crate::argon_info!("Backed up existing directory to '{}'", backup_dir.display().to_string().bold());
	}

	// 3. Authenticate
	let client = reqwest::blocking::Client::new();

	let auth_resp: AuthResponse = client
		.post(format!("{base_url}/auth"))
		.json(&AuthRequest {
			token,
			protocol_version: PROTOCOL_VERSION,
		})
		.send()
		.context("connecting to host")?
		.json()
		.context("parsing auth response")?;

	if !auth_resp.ok {
		bail!(
			"Authentication failed: {}",
			auth_resp.error.unwrap_or_else(|| "unknown error".into())
		);
	}

	let session_id = auth_resp.session_id;
	crate::argon_info!("Joined collab session {} on {}", session_id, addr);

	// 4. Download manifest
	let manifest: Manifest = client
		.get(format!("{base_url}/manifest"))
		.send()
		.context("fetching manifest")?
		.json()
		.context("parsing manifest")?;

	crate::argon_info!("Manifest received: {} files", manifest.files.len());

	// 5. Download all files and materialize
	let mut snapshot_files: Vec<(String, Vec<u8>)> = Vec::new();
	for file_entry in &manifest.files {
		let fc: FileContent = client
			.get(format!("{base_url}/file"))
			.query(&[("path", &file_entry.path)])
			.send()
			.with_context(|| format!("downloading {}", file_entry.path))?
			.json()
			.with_context(|| format!("parsing file content for {}", file_entry.path))?;
		snapshot_files.push((fc.path, fc.content));
	}

	// Remove and recreate target dir
	if target_dir.exists() {
		std::fs::remove_dir_all(&target_dir).context("removing existing target dir")?;
	}
	std::fs::create_dir_all(&target_dir).context("creating target dir")?;

	apply_snapshot(&target_dir, &snapshot_files)?;
	crate::argon_info!("Snapshot applied to '{}'", target_dir.display().to_string().bold());

	// 6. Build local file revision map (path -> (rev, hash))
	let mut local_revs: HashMap<String, (u64, String)> = manifest
		.files
		.iter()
		.map(|f| (f.path.clone(), (f.rev, f.hash.clone())))
		.collect();

	// Track paths recently updated from host to avoid re-proposing them
	let mut host_applied: HashSet<String> = HashSet::new();

	let mut last_seen_rev = manifest.head_rev;

	// mtime cache for local change detection
	let mut mtimes: HashMap<String, SystemTime> = build_mtime_cache(&target_dir);

	// 7. Main event loop
	loop {
		// --- Poll host for new changes ---
		let resp = client
			.get(format!("{base_url}/changes"))
			.query(&ChangesSinceQuery { since_rev: last_seen_rev })
			.send();

		match resp {
			Err(e) => {
				crate::argon_warn!("Lost connection to host: {e}. Retrying…");
				std::thread::sleep(Duration::from_secs(3));
				continue;
			}
			Ok(r) => {
				let batch: ChangesBatch = match r.json() {
					Ok(b) => b,
					Err(e) => {
						crate::argon_warn!("Failed to parse changes response: {e}");
						std::thread::sleep(Duration::from_secs(1));
						continue;
					}
				};

				for entry in &batch.entries {
					let dest = target_dir.join(entry.path.replace('/', std::path::MAIN_SEPARATOR_STR));
					if let Some(parent) = dest.parent() {
						let _ = std::fs::create_dir_all(parent);
					}
					if let Err(e) = std::fs::write(&dest, &entry.content) {
						crate::argon_warn!("Failed to apply change to '{}': {e}", entry.path);
					} else {
						info!("Applied host change: {} (rev {})", entry.path, entry.rev);
						local_revs.insert(entry.path.clone(), (entry.rev, entry.hash.clone()));
						host_applied.insert(entry.path.clone());
						// Update mtime cache to avoid re-proposing this write
						if let Ok(meta) = dest.metadata() {
							if let Ok(mtime) = meta.modified() {
								mtimes.insert(entry.path.clone(), mtime);
							}
						}
					}
				}

				last_seen_rev = batch.head_rev;
			}
		}

		// --- Check local filesystem for changes ---
		let changed_files = scan_local_changes(&target_dir, &mut mtimes);

		for (rel_path, content) in changed_files {
			// Skip files that were just written by a host broadcast
			if host_applied.remove(&rel_path) {
				continue;
			}

			let base_rev = local_revs.get(&rel_path).map_or(0, |(rev, _)| *rev);
			let new_hash = hash_content(&content);

			// Skip if content hasn't actually changed
			let same = local_revs.get(&rel_path).map_or(false, |(_, h)| *h == new_hash);
			if same {
				continue;
			}

			info!("Proposing change for '{rel_path}' (base_rev={base_rev})");

			let result: Result<ChangeResult, _> = client
				.post(format!("{base_url}/propose"))
				.json(&ChangeProposal {
					session_id,
					path: rel_path.clone(),
					base_rev,
					content: content.clone(),
				})
				.send()
				.and_then(|r| r.json());

			match result {
				Ok(cr) if cr.accepted => {
					if let Some(new_rev) = cr.new_rev {
						local_revs.insert(rel_path, (new_rev, new_hash));
					}
				}
				Ok(cr) => {
					crate::argon_warn!(
						"Change for '{}' rejected: {}",
						rel_path,
						cr.error.unwrap_or_else(|| "conflict".into())
					);
					// Re-fetch the file to resolve conflict
					if let Ok(fc) = client
						.get(format!("{base_url}/file"))
						.query(&[("path", &rel_path)])
						.send()
						.and_then(|r| r.json::<FileContent>())
					{
						let dest = target_dir.join(rel_path.replace('/', std::path::MAIN_SEPARATOR_STR));
						let _ = std::fs::write(&dest, &fc.content);
						local_revs.insert(rel_path.clone(), (fc.rev, fc.hash.clone()));
						host_applied.insert(rel_path);
					}
				}
				Err(e) => {
					crate::argon_warn!("Failed to send change proposal for '{rel_path}': {e}");
				}
			}
		}

		std::thread::sleep(Duration::from_millis(500));
	}
}

// ------------------------------------------------------------------
// Helpers
// ------------------------------------------------------------------

fn build_mtime_cache(dir: &Path) -> HashMap<String, SystemTime> {
	let mut map = HashMap::new();
	fill_mtime_cache(dir, dir, &mut map);
	map
}

static CLIENT_IGNORE: &[&str] = &[".git", ".vasc-collab-backup", "node_modules"];

fn fill_mtime_cache(root: &Path, dir: &Path, map: &mut HashMap<String, SystemTime>) {
	let Ok(entries) = std::fs::read_dir(dir) else { return };
	for entry in entries.flatten() {
		let path = entry.path();
		let name = entry.file_name();
		let name_str = name.to_string_lossy();
		if CLIENT_IGNORE.iter().any(|p| *p == name_str.as_ref()) {
			continue;
		}
		if path.is_dir() {
			fill_mtime_cache(root, &path, map);
		} else {
			if let Ok(rel) = path.strip_prefix(root) {
				let rel_str = rel.to_string_lossy().replace('\\', "/");
				if let Ok(meta) = path.metadata() {
					if let Ok(mtime) = meta.modified() {
						map.insert(rel_str, mtime);
					}
				}
			}
		}
	}
}

/// Returns a list of `(relative_path, content)` for files whose mtime changed.
fn scan_local_changes(dir: &Path, mtimes: &mut HashMap<String, SystemTime>) -> Vec<(String, Vec<u8>)> {
	let mut changed = Vec::new();
	collect_changes(dir, dir, mtimes, &mut changed);
	changed
}

fn collect_changes(
	root: &Path,
	dir: &Path,
	mtimes: &mut HashMap<String, SystemTime>,
	changed: &mut Vec<(String, Vec<u8>)>,
) {
	let Ok(entries) = std::fs::read_dir(dir) else { return };
	for entry in entries.flatten() {
		let path = entry.path();
		let name = entry.file_name();
		let name_str = name.to_string_lossy();
		if CLIENT_IGNORE.iter().any(|p| *p == name_str.as_ref()) {
			continue;
		}
		if path.is_dir() {
			collect_changes(root, &path, mtimes, changed);
		} else {
			let Ok(rel) = path.strip_prefix(root) else { continue };
			let rel_str = rel.to_string_lossy().replace('\\', "/");
			let Ok(meta) = path.metadata() else { continue };
			let Ok(mtime) = meta.modified() else { continue };
			let stale = mtimes.get(&rel_str).map_or(true, |prev| *prev != mtime);
			if stale {
				mtimes.insert(rel_str.clone(), mtime);
				if let Ok(content) = std::fs::read(&path) {
					changed.push((rel_str, content));
				}
			}
		}
	}
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
	std::fs::create_dir_all(dst).context("creating backup dir")?;
	for entry in std::fs::read_dir(src).context("reading source dir for backup")? {
		let entry = entry?;
		let src_path = entry.path();
		let dst_path = dst.join(entry.file_name());
		if src_path.is_dir() {
			copy_dir_all(&src_path, &dst_path)?;
		} else {
			std::fs::copy(&src_path, &dst_path)
				.with_context(|| format!("copying {} to {}", src_path.display(), dst_path.display()))?;
		}
	}
	Ok(())
}
