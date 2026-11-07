use anyhow::bail;

use crate::{db::Database, env::envs};

#[derive(Clone, Debug, clap::Parser)]
pub struct ForEachCommand {
    /// The alias of the devices to target
    #[arg(short, long)]
    alias: Option<String>,
    /// Program to run
    program: String,
    /// Arguments to pass to the program
    arguments: Vec<String>,
}

impl ForEachCommand {
    pub async fn exec(self, db: Database) -> anyhow::Result<()> {
        let Self {
            alias,
            program,
            arguments,
        } = self;
        let mut devices = db.read_devices()?;
        if let Some(pattern) = &alias {
            let pattern = glob::Pattern::new(pattern)?;
            devices.retain(|alias, _| pattern.matches(alias));
        }

        let mut sorted_devices: Vec<_> = devices.into_iter().collect();
        sorted_devices.sort_by(|(left, _), (right, _)| left.cmp(right));
        for (alias, device) in sorted_devices {
            let mut cmd = std::process::Command::new(&program);
            cmd.args(&arguments).env("AXIS_DEVICE_ALIAS", &alias).envs(
                envs(&device)
                    .into_iter()
                    .flat_map(|(k, v)| v.map(|v| (k, v))),
            );
            let status = cmd.status()?;
            if !status.success() {
                bail!("Command exited with status {status} for device {alias}");
            }
        }
        Ok(())
    }
}
