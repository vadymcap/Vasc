use std::{
	collections::HashMap,
	hash::{Hash, Hasher},
	sync::{Arc, Mutex},
};

use crate::collab::protocol::{BroadcastEntry, FileEntry};

/// Per-file state tracked by the host
#[derive(Debug, Clone)]
pub struct FileState {
	pub rev: u64,
	pub hash: String,
	pub content: Vec<u8>,
}

/// A logged change for client polling
#[derive(Debug, Clone)]
pub struct ChangeRecord {
	pub global_seq: u64,
	pub entry: BroadcastEntry,
}

/// The authoritative collaboration state held by the host
pub struct CollabState {
	/// Maps canonical (forward-slash) relative paths to file state
	pub files: HashMap<String, FileState>,
	/// Monotonically increasing global sequence number; bumped on every accepted change
	pub global_seq: u64,
	/// Ordered log of all accepted changes (used by clients for polling)
	pub change_log: Vec<ChangeRecord>,
	/// Registered client session IDs
	pub sessions: Vec<u32>,
	/// Next session ID to assign
	next_session_id: u32,
}

impl CollabState {
	pub fn new() -> Self {
		Self {
			files: HashMap::new(),
			global_seq: 0,
			change_log: Vec::new(),
			sessions: Vec::new(),
			next_session_id: 1,
		}
	}

	/// Register a new client session and return the assigned ID
	pub fn add_session(&mut self) -> u32 {
		let id = self.next_session_id;
		self.next_session_id += 1;
		self.sessions.push(id);
		id
	}

	/// Remove a client session
	pub fn remove_session(&mut self, id: u32) {
		self.sessions.retain(|&s| s != id);
	}

	/// Apply a change (from host filesystem or accepted proposal), bump global_seq,
	/// record in change_log, and return the new FileEntry.
	pub fn apply_change(
		&mut self,
		path: String,
		content: Vec<u8>,
		from_session: Option<u32>,
	) -> FileEntry {
		self.global_seq += 1;
		let seq = self.global_seq;

		let hash = hash_content(&content);

		let file_rev = {
			let state = self.files.entry(path.clone()).or_insert(FileState {
				rev: 0,
				hash: hash.clone(),
				content: content.clone(),
			});
			state.rev += 1;
			state.hash = hash.clone();
			state.content = content.clone();
			state.rev
		};

		let record = ChangeRecord {
			global_seq: seq,
			entry: BroadcastEntry {
				path: path.clone(),
				content,
				rev: file_rev,
				hash: hash.clone(),
				from_session,
			},
		};

		self.change_log.push(record);

		FileEntry {
			path,
			rev: file_rev,
			hash,
		}
	}

	/// Returns all change records with global_seq > since_rev
	pub fn changes_since(&self, since_rev: u64) -> (Vec<BroadcastEntry>, u64) {
		let entries: Vec<BroadcastEntry> = self
			.change_log
			.iter()
			.filter(|r| r.global_seq > since_rev)
			.map(|r| r.entry.clone())
			.collect();

		(entries, self.global_seq)
	}
}

/// Produce a deterministic hex hash of file content for change detection.
///
/// Uses a 64-bit FNV-1a hash encoded as 16 hex digits.  This is not
/// cryptographically secure but is sufficient for detecting file changes on a
/// trusted LAN.
pub fn hash_content(content: &[u8]) -> String {
	struct FnvHasher(u64);

	impl Hasher for FnvHasher {
		fn finish(&self) -> u64 {
			self.0
		}

		fn write(&mut self, bytes: &[u8]) {
			const FNV_PRIME: u64 = 1099511628211;
			for &byte in bytes {
				self.0 ^= u64::from(byte);
				self.0 = self.0.wrapping_mul(FNV_PRIME);
			}
		}
	}

	impl FnvHasher {
		fn new() -> Self {
			const FNV_OFFSET: u64 = 14695981039346656037;
			FnvHasher(FNV_OFFSET)
		}
	}

	let mut h = FnvHasher::new();
	content.hash(&mut h);
	format!("{:016x}", h.finish())
}

pub type SharedState = Arc<Mutex<CollabState>>;

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn hash_is_deterministic() {
		let a = hash_content(b"hello world");
		let b = hash_content(b"hello world");
		assert_eq!(a, b);
	}

	#[test]
	fn hash_differs_for_different_content() {
		let a = hash_content(b"foo");
		let b = hash_content(b"bar");
		assert_ne!(a, b);
	}

	#[test]
	fn apply_change_increments_revs() {
		let mut state = CollabState::new();
		let entry = state.apply_change("src/foo.lua".into(), b"print('hi')".to_vec(), None);
		assert_eq!(entry.rev, 1);
		assert_eq!(state.global_seq, 1);

		let entry2 = state.apply_change("src/foo.lua".into(), b"print('hello')".to_vec(), None);
		assert_eq!(entry2.rev, 2);
		assert_eq!(state.global_seq, 2);
	}

	#[test]
	fn changes_since_filters_correctly() {
		let mut state = CollabState::new();
		state.apply_change("a.lua".into(), b"1".to_vec(), None);
		state.apply_change("b.lua".into(), b"2".to_vec(), None);
		state.apply_change("c.lua".into(), b"3".to_vec(), None);

		let (entries, head) = state.changes_since(1);
		assert_eq!(entries.len(), 2);
		assert_eq!(entries[0].path, "b.lua");
		assert_eq!(entries[1].path, "c.lua");
		assert_eq!(head, 3);
	}
}
