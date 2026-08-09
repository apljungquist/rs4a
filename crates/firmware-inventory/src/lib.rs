mod catalog;
pub mod commands;
mod db;
mod track;
mod version;

use std::path::PathBuf;

use anyhow::Context;
use clap::{Parser, Subcommand};
use rs4a_authentication::SessionCookie;
use rs4a_bin_utils::completions_command::CompletionsCommand;
use url::Url;

use crate::db::Database;
pub use crate::{
    commands::{get::GetCommand, list::ListCommand, login::LoginCommand, update::UpdateCommand},
    track::{Selector, Track},
    version::Version,
};

pub(crate) fn authenticated_client(cookie: SessionCookie) -> anyhow::Result<reqwest::Client> {
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(reqwest::header::COOKIE, cookie.into_header_value());
    reqwest::Client::builder()
        .default_headers(headers)
        .build()
        .context("Failed to build HTTP client")
}

/// Base URL under which firmware, and the catalog describing it, are hosted.
const SOFTWARE_BASE_URL: &str = "https://www.axis.com/ftp/pub/axis/software/";

/// Path, relative to [`SOFTWARE_BASE_URL`], of the catalog.
pub(crate) const CATALOG_PATH: &str = "pubtool/boxdesc/boxdesc.xml";

/// Root that every [`crate::db::FileUrl`] is relative to.
pub(crate) fn software_root() -> Url {
    Url::parse(SOFTWARE_BASE_URL).expect("Literal is valid URL")
}

#[derive(Parser)]
#[command(name = "firmware-inventory", version)]
pub struct Cli {
    /// Location of the application data.
    #[clap(long, env = "FIRMWARE_INVENTORY_LOCATION")]
    pub inventory: Option<PathBuf>,
    #[clap(long, env = "FIRMWARE_INVENTORY_OFFLINE")]
    pub offline: bool,
    #[command(subcommand)]
    pub command: Commands,
}

impl Cli {
    pub async fn exec(self) -> anyhow::Result<String> {
        let Self {
            inventory,
            offline,
            command,
        } = self;
        let db = Database::open_or_create(inventory)?;
        match command {
            Commands::Login(cmd) => cmd.exec(db).await,
            Commands::Update(cmd) => cmd.exec(&db, offline).await,
            Commands::List(cmd) => cmd.exec(&db),
            Commands::Get(cmd) => cmd.exec(&db, offline).await,
            Commands::Completions(cmd) => {
                cmd.exec::<Self>()?;
                Ok(String::new())
            }
        }
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// Login to access firmware downloads
    Login(LoginCommand),
    /// Update the local firmware index from the upstream catalog
    Update(UpdateCommand),
    /// List indexed firmware versions, showing which are cached locally
    List(ListCommand),
    /// Get firmware for each product, matching a version requirement or a track
    Get(GetCommand),
    /// Print a completion file for the given shell.
    ///
    /// Example: `firmware-inventory completions zsh | source /dev/stdin`.
    Completions(CompletionsCommand),
}
