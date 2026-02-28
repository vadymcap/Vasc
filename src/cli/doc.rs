use anyhow::Result;
use clap::Parser;
use colored::Colorize;

use crate::vasc_info;

const LINK: &str = "https://vasc.wiki";

/// Open vasc's documentation in the browser
#[derive(Parser)]
pub struct Doc {}

impl Doc {
	pub fn main(self) -> Result<()> {
		vasc_info!("Launched browser. Manually go to: {}", LINK.bold());

		open::that(LINK)?;

		Ok(())
	}
}
