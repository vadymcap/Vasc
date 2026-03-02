use serde::{Deserialize, Serialize};

pub const PROTOCOL_VERSION: u32 = 1;

/// A single file entry in the manifest
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileEntry {
	pub path: String,
	pub rev: u64,
	pub hash: String,
}

/// Sent by client to authenticate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthRequest {
	pub token: Option<String>,
	pub protocol_version: u32,
}

/// Sent by host in response to auth
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthResponse {
	pub session_id: u32,
	pub ok: bool,
	pub error: Option<String>,
}

/// The full manifest of project files
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
	pub files: Vec<FileEntry>,
	pub head_rev: u64,
}

/// Response for a single file download
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileContent {
	pub path: String,
	pub content: Vec<u8>,
	pub rev: u64,
	pub hash: String,
}

/// A change proposed by a client
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeProposal {
	pub session_id: u32,
	pub path: String,
	/// The revision the client based this change on
	pub base_rev: u64,
	pub content: Vec<u8>,
}

/// Host's response to a change proposal
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangeResult {
	pub path: String,
	pub accepted: bool,
	pub new_rev: Option<u64>,
	/// Set when rejected due to conflict
	pub conflict_rev: Option<u64>,
	pub conflict_hash: Option<String>,
	pub error: Option<String>,
}

/// Query params for the /changes endpoint
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangesSinceQuery {
	pub since_rev: u64,
}

/// A single broadcasted change event
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastEntry {
	pub path: String,
	pub content: Vec<u8>,
	pub rev: u64,
	pub hash: String,
	/// Session that originated the change (None = host filesystem)
	pub from_session: Option<u32>,
}

/// Collection of broadcast entries returned by /changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangesBatch {
	pub entries: Vec<BroadcastEntry>,
	pub head_rev: u64,
}
