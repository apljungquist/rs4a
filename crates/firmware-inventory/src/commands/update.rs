use anyhow::{bail, Context};
use log::{debug, info};

use crate::{
    authenticated_client,
    catalog::{self, CatalogEntry},
    db::{Database, FileUrl, Index, ShortName},
    software_root,
    version::Version,
    CATALOG_PATH,
};

fn index_from_catalog(catalog: Vec<CatalogEntry>) -> Index {
    let mut index = Index::new();
    for entry in catalog {
        let CatalogEntry {
            product,
            revision,
            fileurl,
        } = entry;
        let version = match Version::try_coerced(&revision) {
            Ok(version) => version,
            Err(e) => {
                debug!("Skipping {product} {revision}, which is not a version: {e}");
                continue;
            }
        };
        let fileurl = match FileUrl::new(&fileurl) {
            Ok(fileurl) => fileurl,
            Err(e) => {
                debug!("Skipping {product} {revision}: {e}");
                continue;
            }
        };
        let product = ShortName::new_normalized(&product);
        // Revisions that differ only in a component this version type drops collide here; the
        // catalog lists them oldest-first, so the newest of them wins.
        if let Some(replaced) = index
            .entry(product.clone())
            .or_default()
            .insert(version, fileurl)
        {
            debug!("{product} {revision} takes the place of {replaced}");
        }
    }
    index
}

#[derive(Clone, Debug, clap::Args)]
pub struct UpdateCommand {}

impl UpdateCommand {
    pub(crate) async fn exec(self, db: &Database, offline: bool) -> anyhow::Result<String> {
        let Self {} = self;

        if offline {
            bail!("Cannot update index when offline");
        }

        let cookie = db
            .read_cookie()?
            .context("No login session, please run the login command")?;
        let client = authenticated_client(cookie)?;

        let url = software_root()
            .join(CATALOG_PATH)
            .context("Failed to build the catalog url")?;
        info!("Fetching firmware catalog from {url}");
        let xml = client
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        let entries = catalog::parse_catalog(&xml)?;
        let index = index_from_catalog(entries);

        db.write_index(&index)?;
        info!("Index updated");

        Ok(String::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(product: &str, directory: &str, revision: &str) -> CatalogEntry {
        let underscored = revision.replace('.', "_");
        CatalogEntry {
            product: product.to_string(),
            revision: revision.to_string(),
            fileurl: format!("/MPQT/{directory}/{underscored}/{directory}_{underscored}.bin"),
        }
    }

    fn products(index: &Index) -> Vec<String> {
        index.keys().map(|p| p.to_string()).collect()
    }

    fn versions(index: &Index, product: &str) -> Vec<String> {
        index[&ShortName::new_normalized(product)]
            .keys()
            .map(|v| v.to_string())
            .collect()
    }

    #[test]
    fn index_from_catalog_merges_the_models_of_one_product() {
        let index = index_from_catalog(vec![
            entry("AXIS P8815-2", "P8815-2", "10.6.0"),
            entry(
                "AXIS P8815-2 3D People Counter",
                "P8815-2_3D_People_Counter",
                "11.11.192",
            ),
            entry("AXIS P8815-2", "P8815-2", "11.11.220"),
        ]);
        assert_eq!(products(&index), ["P8815-2"]);
        assert_eq!(
            versions(&index, "P8815-2"),
            ["10.6.0", "11.11.192", "11.11.220"]
        );
    }

    #[test]
    fn index_from_catalog_keeps_the_location_the_catalog_gave() {
        let index = index_from_catalog(vec![entry(
            "AXIS P8815-2 3D People Counter",
            "P8815-2_3D_People_Counter",
            "11.11.192",
        )]);
        let product = ShortName::new_normalized("P8815-2");
        let version = Version::try_coerced("11.11.192").unwrap();
        assert_eq!(
            index[&product][&version].as_str(),
            "MPQT/P8815-2_3D_People_Counter/11_11_192/P8815-2_3D_People_Counter_11_11_192.bin"
        );
    }

    #[test]
    fn index_from_catalog_gives_models_that_share_a_directory_an_entry_each() {
        let index = index_from_catalog(vec![
            entry("AXIS P1265", "P12_MkII", "12.9.57"),
            entry("AXIS P1275", "P12_MkII", "12.9.57"),
        ]);
        assert_eq!(products(&index), ["P1265", "P1275"]);
        // ...pointing at the one image they share, so it is fetched and kept once.
        let version = Version::try_coerced("12.9.57").unwrap();
        assert_eq!(
            index[&ShortName::new_normalized("P1265")][&version],
            index[&ShortName::new_normalized("P1275")][&version]
        );
    }

    #[test]
    fn index_from_catalog_drops_a_revision_that_is_not_a_version() {
        let index = index_from_catalog(vec![
            entry("AXIS M1075-L", "M1075-L", "12.9.57"),
            entry("AXIS M1075-L", "M1075-L", "latest"),
        ]);
        assert_eq!(versions(&index, "M1075-L"), ["12.9.57"]);
    }

    #[test]
    fn index_from_catalog_lets_the_last_of_two_colliding_revisions_win() {
        // The version type keeps three components, so these two revisions are one key.
        let index = index_from_catalog(vec![
            entry("AXIS C1004-E", "C1004-E", "10.0.2.1"),
            entry("AXIS C1004-E", "C1004-E", "10.0.2.2"),
        ]);
        assert_eq!(versions(&index, "C1004-E"), ["10.0.2"]);
        let product = ShortName::new_normalized("C1004-E");
        let version = Version::try_coerced("10.0.2").unwrap();
        assert!(index[&product][&version].as_str().ends_with("10_0_2_2.bin"));
    }
}
