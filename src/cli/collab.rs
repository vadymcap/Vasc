use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// LAN/VPN collaboration commands
#[derive(Parser)]
pub struct Collab {
	#[command(subcommand)]
	command: CollabCommands,
}

impl Collab {
	pub fn main(self) -> Result<()> {
		match self.command {
			CollabCommands::Host(cmd) => cmd.main(),
			CollabCommands::Join(cmd) => cmd.main(),
		}
	}
}

#[derive(Subcommand)]
enum CollabCommands {
	Host(Host),
	Join(Join),
}

// ------------------------------------------------------------------
// Host subcommand
// ------------------------------------------------------------------

/// Start a collaboration host session for a project directory
#[derive(Parser)]
pub struct Host {
	/// Path to the project directory to share
	#[arg(short, long, value_name = "PATH")]
	project: PathBuf,

	/// IP address to bind (e.g. 0.0.0.0 or a specific interface)
	#[arg(short, long, value_name = "IP", default_value = "0.0.0.0")]
	bind: String,

	/// Port to listen on
	#[arg(short, long, value_name = "PORT", default_value_t = 8080)]
	port: u16,

	/// Optional shared secret token that clients must provide
	#[arg(short, long, value_name = "TOKEN")]
	token: Option<String>,
}

impl Host {
	pub fn main(self) -> Result<()> {
		use colored::Colorize;

		let project = self.project.canonicalize().unwrap_or(self.project);

		crate::argon_info!(
			"Starting collab host for '{}' on {}:{}",
			project.display().to_string().bold(),
			self.bind.bold(),
			self.port.to_string().bold()
		);

		if self.token.is_some() {
			crate::argon_info!("Token authentication enabled");
		} else {
			crate::argon_warn!("No token set — session is open to anyone on the network");
		}

		crate::collab::host::start_host(project, self.bind, self.port, self.token)?;

		Ok(())
	}
}

// ------------------------------------------------------------------
// Join subcommand
// ------------------------------------------------------------------

/// Join a collaboration session hosted by another machine
#[derive(Parser)]
pub struct Join {
	/// Host address in `host:port` format (e.g. 192.168.1.10:8080)
	#[arg(value_name = "HOST:PORT")]
	addr: String,

	/// Shared secret token (required when the host uses token auth)
	#[arg(short, long, value_name = "TOKEN")]
	token: Option<String>,

	/// Local directory where the project will be placed
	#[arg(short, long, value_name = "PATH")]
	dir: PathBuf,

	/// Back up the existing target directory before overwriting (default: true)
	#[arg(long, overrides_with = "no_backup")]
	backup: bool,

	/// Skip backing up the existing target directory
	#[arg(long)]
	no_backup: bool,
}

impl Join {
	pub fn main(self) -> Result<()> {
		// --backup is the default; --no-backup disables it
		let do_backup = !self.no_backup;

		crate::collab::client::join_session(self.addr, self.token, self.dir, do_backup)?;

		Ok(())
	}
}
