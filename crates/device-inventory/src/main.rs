mod commands;
mod db;
mod db_vlt;
mod env;
mod fusion;
mod psst;
mod vlt;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use rs4a_bin_utils::completions_command::CompletionsCommand;

use crate::{
    commands::{
        activate::ActivateCommand, add::AddCommand, deactivate::DeactivateCommand,
        dump::DumpCommand, for_each::ForEachCommand, list::ListCommand, load::LoadCommand,
        login::LoginCommand, remove::RemoveCommand, sync::SyncCommand,
    },
    db::Database,
};

#[derive(Parser)]
#[command(version)]
struct Cli {
    /// Location of the application data.
    #[clap(long, env = "DEVICE_INVENTORY_LOCATION")]
    inventory: Option<PathBuf>,
    #[clap(long, env = "DEVICE_INVENTORY_OFFLINE")]
    offline: bool,
    #[command(subcommand)]
    command: Commands,
}

impl Cli {
    pub async fn exec(self) -> anyhow::Result<()> {
        let Self {
            inventory: db,
            offline,
            command,
        } = self;
        let db = Database::open_or_create(db)?;
        match command {
            Commands::Login(cmd) => cmd.exec(db, offline).await?,
            Commands::Add(cmd) => cmd.exec(db).await?,
            Commands::Deactivate(cmd) => cmd.exec().await?,
            Commands::Sync(cmd) => cmd.exec(&db, offline).await?,
            Commands::ForEach(cmd) => cmd.exec(db).await?,
            Commands::List(cmd) => cmd.exec(&db, offline).await?,
            Commands::Activate(cmd) => cmd.exec(db).await?,
            Commands::Remove(cmd) => cmd.exec(db).await?,
            Commands::Dump(cmd) => cmd.exec(&db).await?,
            Commands::Load(cmd) => cmd.exec(&db).await?,
            Commands::Completions(cmd) => cmd.exec::<Self>()?,
        }
        Ok(())
    }
}

#[derive(Subcommand)]
enum Commands {
    /// Login to a pool of shared devices
    Login(LoginCommand),
    /// Add a device
    Add(AddCommand),
    /// Deactivate any active device.
    ///
    /// For devices activated using environment variables, the printed commands must be run.
    Deactivate(DeactivateCommand),
    /// Sync devices from the VLT to the local inventory
    Sync(SyncCommand),
    /// Run a command with environment variables set for each device
    ForEach(ForEachCommand),
    /// List available devices
    List(ListCommand),
    /// Activate an existing device.
    Activate(ActivateCommand),
    /// Remove a device
    Remove(RemoveCommand),
    /// Print the device-inventory database to stdout.
    Dump(DumpCommand),
    /// Load the device-inventory database from stdin.
    Load(LoadCommand),
    /// Print a completion file for the given shell.
    ///
    /// Example: `device-inventory completions zsh | source /dev/stdin`.
    Completions(CompletionsCommand),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut guard = rs4a_bin_utils::logger::init();
    Cli::parse().exec().await?;
    guard.disarm();
    Ok(())
}
