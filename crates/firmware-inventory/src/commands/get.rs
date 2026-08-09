use std::{collections::BTreeSet, fmt::Write, fs};

use anyhow::{bail, Context};
use log::info;

use crate::{
    authenticated_client,
    db::{Database, FileUrl},
    track::{self, Selector},
};

#[derive(Clone, Debug, clap::Args)]
#[command(group(clap::ArgGroup::new("get_selector").required(true).args(["version", "track"])))]
pub struct GetCommand {
    /// Glob patterns to match product names (each must match exactly one)
    #[clap(required = true)]
    pub products: Vec<glob::Pattern>,
    #[command(flatten)]
    pub selector: Selector,
}

async fn download(
    client: &reqwest::Client,
    db: &Database,
    fileurl: &FileUrl,
) -> anyhow::Result<()> {
    let url = fileurl.url()?;
    info!("Downloading {url}");

    let response = client
        .get(url)
        .send()
        .await?
        .error_for_status()
        .context("Failed to download firmware")?;
    let bytes = response.bytes().await?;

    let path = db.firmware_path(fileurl);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Failed to create firmware directory")?;
    }
    fs::write(&path, &bytes).context("Failed to write firmware file")
}

impl GetCommand {
    pub(crate) async fn exec(self, db: &Database, offline: bool) -> anyhow::Result<String> {
        let Self { products, selector } = self;

        let index = db.read_index()?;

        // Resolve every pattern before fetching anything, so that a pattern that cannot be
        // satisfied is reported before any bytes are spent on the ones before it.
        let mut resolved: Vec<FileUrl> = Vec::new();
        for pattern in &products {
            let matching: Vec<_> = index
                .iter()
                .filter(|(p, _)| pattern.matches(p.as_str()))
                .collect();
            let (product, versions) = match matching.as_slice() {
                [] => bail!("No indexed products matched {pattern:?}. Run update first."),
                [pair] => *pair,
                pairs => {
                    let names: Vec<_> = pairs.iter().map(|(p, _)| p.as_str()).collect();
                    bail!(
                        "Product glob {pattern} matched {} products: {names:?}. Use a more specific pattern.",
                        names.len()
                    )
                }
            };

            let candidates: Vec<_> = versions.keys().collect();

            let Some(req) = selector.resolve(&candidates) else {
                let available = track::available_tracks(&candidates);
                let available = if available.is_empty() {
                    "none".to_string()
                } else {
                    available.join(", ")
                };
                bail!(
                    "{product} has no firmware on {}. Tracks with firmware for {product}: {available}",
                    selector.describe()
                );
            };

            // The versions are sorted, so the last one matching the requirement is the best.
            let (version, fileurl) = versions
                .iter()
                .rfind(|(v, _)| v.matches(&req))
                .with_context(|| {
                    format!("No version of {product} matched {}", selector.describe())
                })?;

            info!("Best match: {product} {version}");
            resolved.push(fileurl.clone());
        }

        // A pattern may be given twice, or two patterns may resolve to the same image -- as the
        // products sharing one do -- so fetch it once and report it once per pattern.
        let missing: BTreeSet<&FileUrl> = resolved
            .iter()
            .filter(|f| !db.firmware_path(f).exists())
            .collect();

        if !missing.is_empty() {
            if offline {
                let paths: Vec<_> = missing
                    .iter()
                    .map(|f| db.firmware_path(f).display().to_string())
                    .collect();
                bail!(
                    "Firmware not cached and offline mode is enabled: {}",
                    paths.join(", ")
                );
            }

            let cookie = db
                .read_cookie()?
                .context("No login session, please run the login command")?;
            let client = authenticated_client(cookie)?;

            for fileurl in missing {
                download(&client, db, fileurl).await?;
            }
        }

        let mut out = String::new();
        for fileurl in &resolved {
            writeln!(out, "{}", db.firmware_path(fileurl).display())?;
        }
        Ok(out)
    }
}
