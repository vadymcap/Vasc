use anyhow::Result;
use clap::Parser;
use colored::Colorize;

use crate::vasc_info;

const LINK: &str = "https://github.com/vadymcap/Vasc";

/// Open Vasc's documentation in the browser
#[derive(Parser)]
pub struct Doc {}

impl Doc {
	pub fn main(self) -> Result<()> {
		vasc_info!("Launched browser. Manually go to: {}", LINK.bold());

		open::that(LINK)?;

		Ok(())
	}
}
