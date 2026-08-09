use std::fmt::Write;

use anyhow::bail;

use crate::{db::Database, track::Selector};

#[derive(Clone, Debug, clap::Args)]
#[command(group(clap::ArgGroup::new("list_selector").args(["version", "track"])))]
pub struct ListCommand {
    /// Glob patterns to match product names (default: all)
    pub products: Vec<glob::Pattern>,
    #[command(flatten)]
    pub selector: Selector,
}

impl ListCommand {
    pub(crate) fn exec(self, db: &Database) -> anyhow::Result<String> {
        let Self { products, selector } = self;

        let index = db.read_index()?;
        let matching: Vec<_> = index
            .iter()
            .filter(|(p, _)| {
                products.is_empty() || products.iter().any(|pat| pat.matches(p.as_str()))
            })
            .collect();

        if matching.is_empty() {
            bail!("No indexed products found. Run update first.");
        }

        let mut out = String::new();
        for (product, versions) in matching {
            let candidates: Vec<_> = versions.keys().collect();

            // A product with nothing on the selected track is simply not listed; unlike `get`,
            // this command is a filter and has no single product it could fail on behalf of.
            let Some(req) = selector.resolve(&candidates) else {
                continue;
            };

            // The versions are sorted ascending; list the newest first.
            for (version, fileurl) in versions.iter().rev().filter(|(v, _)| v.matches(&req)) {
                let cached = if db.firmware_path(fileurl).exists() {
                    " [cached]"
                } else {
                    ""
                };
                writeln!(out, "{product} {version}{cached}")?;
            }
        }

        Ok(out)
    }
}
