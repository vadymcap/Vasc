use actix_web::{
	get, post,
	web::{Data, Json, Query},
	App, HttpResponse, HttpServer, Responder,
};
use anyhow::Result;
use log::info;
use serde::Deserialize;
use std::{
	path::{Path, PathBuf},
	sync::{Arc, Mutex},
	time::Duration,
};

use crate::collab::{
	protocol::{AuthRequest, AuthResponse, ChangeProposal, ChangeResult, ChangesBatch, ChangesSinceQuery, FileContent, Manifest, PROTOCOL_VERSION},
	snapshot::build_manifest_from_dir,
	state::{hash_content, CollabState, SharedState},
};

/// Configuration passed to the host actix-web workers
struct HostConfig {
	state: SharedState,
	token: Option<String>,
	project_dir: PathBuf,
}

// ------------------------------------------------------------------
// Helpers
// ------------------------------------------------------------------

fn check_auth(token: &Option<String>, expected: &Option<String>) -> bool {
	match (token, expected) {
		(_, None) => true,
		(Some(t), Some(e)) => t == e,
		(None, Some(_)) => false,
	}
}

// ------------------------------------------------------------------
// Endpoints
// ------------------------------------------------------------------

#[derive(Deserialize)]
struct FileQuery {
	path: String,
}

/// POST /auth — exchange token for a session ID
#[post("/auth")]
async fn auth(config: Data<Arc<HostConfig>>, body: Json<AuthRequest>) -> impl Responder {
	if body.protocol_version != PROTOCOL_VERSION {
		return HttpResponse::BadRequest().json(AuthResponse {
			session_id: 0,
			ok: false,
			error: Some(format!(
				"protocol version mismatch: server={PROTOCOL_VERSION}, client={}",
				body.protocol_version
			)),
		});
	}

	if !check_auth(&body.token, &config.token) {
		return HttpResponse::Unauthorized().json(AuthResponse {
			session_id: 0,
			ok: false,
			error: Some("invalid token".into()),
		});
	}

	let session_id = config.state.lock().unwrap().add_session();
	info!("New session {session_id} authenticated");

	HttpResponse::Ok().json(AuthResponse {
		session_id,
		ok: true,
		error: None,
	})
}

/// GET /manifest — return current file manifest
#[get("/manifest")]
async fn manifest(config: Data<Arc<HostConfig>>) -> impl Responder {
	let state = config.state.lock().unwrap();
	let files: Vec<_> = {
		let mut v: Vec<_> = state
			.files
			.iter()
			.map(|(path, fs)| crate::collab::protocol::FileEntry {
				path: path.clone(),
				rev: fs.rev,
				hash: fs.hash.clone(),
			})
			.collect();
		v.sort_by(|a, b| a.path.cmp(&b.path));
		v
	};
	let manifest = Manifest {
		files,
		head_rev: state.global_seq,
	};
	HttpResponse::Ok().json(manifest)
}

/// GET /file?path=<relative_path> — download a single file
#[get("/file")]
async fn file(config: Data<Arc<HostConfig>>, query: Query<FileQuery>) -> impl Responder {
	let state = config.state.lock().unwrap();
	match state.files.get(&query.path) {
		Some(fs) => HttpResponse::Ok().json(FileContent {
			path: query.path.clone(),
			content: fs.content.clone(),
			rev: fs.rev,
			hash: fs.hash.clone(),
		}),
		None => HttpResponse::NotFound().body(format!("file not found: {}", query.path)),
	}
}

/// POST /propose — client proposes a change to the host
#[post("/propose")]
async fn propose(config: Data<Arc<HostConfig>>, body: Json<ChangeProposal>) -> impl Responder {
	let mut state = config.state.lock().unwrap();

	// Validate session
	if !state.sessions.contains(&body.session_id) {
		return HttpResponse::Unauthorized().json(ChangeResult {
			path: body.path.clone(),
			accepted: false,
			new_rev: None,
			conflict_rev: None,
			conflict_hash: None,
			error: Some("unknown session; re-authenticate".into()),
		});
	}

	let current_rev = state.files.get(&body.path).map(|f| f.rev).unwrap_or(0);
	if body.base_rev != current_rev {
		let current_hash = state.files.get(&body.path).map(|f| f.hash.clone());
		return HttpResponse::Conflict().json(ChangeResult {
			path: body.path.clone(),
			accepted: false,
			new_rev: None,
			conflict_rev: Some(current_rev),
			conflict_hash: current_hash,
			error: Some(format!(
				"conflict: base_rev={} but current_rev={}",
				body.base_rev, current_rev
			)),
		});
	}

	// Apply the change
	let entry = state.apply_change(body.path.clone(), body.content.clone(), Some(body.session_id));

	// Also persist to disk
	let dest = config.project_dir.join(body.path.replace('/', std::path::MAIN_SEPARATOR_STR));
	drop(state); // release lock before IO

	if let Some(parent) = dest.parent() {
		if let Err(e) = std::fs::create_dir_all(parent) {
			return HttpResponse::InternalServerError().json(ChangeResult {
				path: body.path.clone(),
				accepted: false,
				new_rev: None,
				conflict_rev: None,
				conflict_hash: None,
				error: Some(format!("failed to create parent dir: {e}")),
			});
		}
	}

	if let Err(e) = std::fs::write(&dest, &body.content) {
		return HttpResponse::InternalServerError().json(ChangeResult {
			path: body.path.clone(),
			accepted: false,
			new_rev: None,
			conflict_rev: None,
			conflict_hash: None,
			error: Some(format!("failed to write file: {e}")),
		});
	}

	info!("Accepted change for '{}' (rev {})", entry.path, entry.rev);

	HttpResponse::Ok().json(ChangeResult {
		path: body.path.clone(),
		accepted: true,
		new_rev: Some(entry.rev),
		conflict_rev: None,
		conflict_hash: None,
		error: None,
	})
}

/// GET /changes?since_rev=<n> — poll for changes since a global sequence number
#[get("/changes")]
async fn changes(config: Data<Arc<HostConfig>>, query: Query<ChangesSinceQuery>) -> impl Responder {
	let state = config.state.lock().unwrap();
	let (entries, head_rev) = state.changes_since(query.since_rev);
	HttpResponse::Ok().json(ChangesBatch { entries, head_rev })
}

// ------------------------------------------------------------------
// Server startup
// ------------------------------------------------------------------

/// Start the collab host server.
///
/// This function loads the project directory into the shared state, then
/// starts an actix-web HTTP server.  It blocks until the server stops.
#[actix_web::main]
pub async fn start_host(project_dir: PathBuf, bind: String, port: u16, token: Option<String>) -> Result<()> {
	let state: SharedState = Arc::new(Mutex::new(CollabState::new()));

	// Load project files into state
	{
		let mut locked = state.lock().unwrap();
		build_manifest_from_dir(&project_dir, &mut locked)?;
		info!("Loaded {} files from project directory", locked.files.len());
	}

	// Spawn a filesystem watcher in a background thread so host-side changes
	// are picked up and broadcast to polling clients.
	{
		let state_clone = state.clone();
		let dir_clone = project_dir.clone();

		std::thread::spawn(move || {
			watch_project_dir(dir_clone, state_clone);
		});
	}

	let config = Arc::new(HostConfig {
		state,
		token,
		project_dir,
	});

	let address = format!("{bind}:{port}");
	info!("Starting collab host on {address}");

	HttpServer::new(move || {
		App::new()
			.app_data(Data::new(config.clone()))
			.service(auth)
			.service(manifest)
			.service(file)
			.service(propose)
			.service(changes)
	})
	.bind(&address)?
	.run()
	.await?;

	Ok(())
}

/// Very lightweight filesystem watcher — polls the project directory every
/// second for content changes using mtime/size, then updates the shared state
/// so clients see the change on their next poll.
///
/// Changes that already match the cached content hash are silently skipped
/// (idempotent), which also prevents re-broadcasting client-applied changes.
fn watch_project_dir(project_dir: PathBuf, state: SharedState) {
	use std::{collections::HashMap, time::SystemTime};

	// mtime cache: path -> last_modified
	let mut mtimes: HashMap<String, SystemTime> = HashMap::new();

	loop {
		std::thread::sleep(Duration::from_secs(1));
		scan_for_changes(&project_dir, &project_dir, &mut mtimes, &state);
	}
}

static WATCH_IGNORE: &[&str] = &[".git", ".vasc-collab-backup", "node_modules"];

fn scan_for_changes(
	root: &Path,
	dir: &Path,
	mtimes: &mut std::collections::HashMap<String, std::time::SystemTime>,
	state: &SharedState,
) {
	let Ok(entries) = std::fs::read_dir(dir) else { return };

	for entry in entries.flatten() {
		let path = entry.path();
		let name = entry.file_name();
		let name_str = name.to_string_lossy();

		if WATCH_IGNORE.iter().any(|p| *p == name_str.as_ref()) {
			continue;
		}

		if path.is_dir() {
			scan_for_changes(root, &path, mtimes, state);
		} else {
			let Ok(rel) = path.strip_prefix(root) else { continue };
			let rel_str = rel.to_string_lossy().replace('\\', "/");

			let Ok(meta) = path.metadata() else { continue };
			let Ok(mtime) = meta.modified() else { continue };

			let changed = mtimes.get(&rel_str).map_or(true, |prev| *prev != mtime);
			if !changed {
				continue;
			}
			mtimes.insert(rel_str.clone(), mtime);

			let Ok(content) = std::fs::read(&path) else { continue };
			let new_hash = hash_content(&content);

			let mut locked = state.lock().unwrap();
			let already_current = locked.files.get(&rel_str).map_or(false, |fs| fs.hash == new_hash);
			if already_current {
				continue;
			}

			info!("Host file changed: {rel_str}");
			locked.apply_change(rel_str, content, None);
		}
	}
}
