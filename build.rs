use anyhow::{Context, Result};
use self_update::backends::github::Update;
use std::{env, fs::File, path::PathBuf};

fn main() -> Result<()> {
	let out_path = PathBuf::from(env::var("OUT_DIR")?).join("Vasc.rbxm");

	if !cfg!(feature = "plugin") {
		File::create(out_path)?;
		return Ok(());
	}

	let mut builder = Update::configure();

	if let Ok(token) = env::var("GITHUB_TOKEN") {
		builder.auth_token(&token);
	} else {
		println!("cargo:warning=GITHUB_TOKEN not set, rate limits may apply!")
	}

	builder
		.repo_owner("vadymcap")
		.repo_name("Vasc-roblox")
		.bin_name("Vasc.rbxm")
		.bin_install_path(out_path)
		.target("");

	builder
		.build()?
		.download()
		.context("Failed to download Vasc plugin from GitHub!")?;

	Ok(())
}
