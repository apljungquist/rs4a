use std::{
    collections::BTreeMap,
    fmt::{self, Display, Formatter},
    fs,
    path::PathBuf,
};

use anyhow::{bail, Context};
use log::debug;
use rs4a_authentication::{CookieStore, SessionCookie};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{software_root, version::Version};

const INDEX_FILE_NAME: &str = "index.json";

/// Directory, under the application data directory, mirroring the software root.
const FIRMWARE_DIR_NAME: &str = "firmware";

/// Model names that belong to a product the device reports under a different name.
///
/// The catalog names most models after the product number the device reports (`Brand.ProdNbr`),
/// but some after its full name (`Brand.ProdFullName` less the brand), and only the catalog knows
/// which. Where the two disagree, the pair is listed here so that the index is keyed the way a
/// device asks for firmware.
const PRODUCT_ALIASES: &[(&str, &str)] = &[
    // Firmware for the 3D people counter is attributed to two models: `P8815-2` up to 10.6.0 and
    // again from 11.11.220, `P8815-2 3D People Counter` in between. Its images report a `ProdNbr`
    // of "P8815-2" and a `ProdFullName` of "AXIS P8815-2 3D People Counter", so folding the latter
    // model into the former both matches the device and gives one continuous version history.
    ("P8815-2 3D People Counter", "P8815-2"),
];

/// The name a device reports itself as, i.e. its `Brand.ProdNbr`.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct ShortName(String);

impl ShortName {
    /// The product number a device reports for firmware attributed to a catalog model.
    ///
    /// The alias cannot be derived from the model name: `P8815-2 3D People Counter` carries a
    /// product type that is not part of the product number, whereas `M1075-L Mk II` names hardware
    /// distinct from `M1075-L` and must not be folded into it. Nothing in the catalog tells the two
    /// cases apart, so they are enumerated in [`PRODUCT_ALIASES`] instead.
    pub fn new_normalized(model: &str) -> Self {
        // Third-party models — `ExCam XF P1377`, `XP40-Q1785`, `IP Verso 2.0` — carry no brand.
        let model = model.strip_prefix("AXIS ").unwrap_or(model);
        let prod_nbr = PRODUCT_ALIASES
            .iter()
            .find(|(name, _)| *name == model)
            .map_or(model, |(_, prod_nbr)| *prod_nbr);
        Self(prod_nbr.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ShortName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// The location of a fimage, relative to the software root.
///
/// One location serves both purposes it is needed for: joined onto the software root it is where
/// the image is fetched from, and joined onto the firmware directory it is where the image is kept.
/// Mirroring the layout rather than inventing one means no naming convention of ours can disagree
/// with the catalog, and the 36 images that more than one model shares are kept once, not once per
/// product.
// Deserialized through `new`, so that reading an index cannot introduce a location that building
// one would have refused.
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(try_from = "String")]
pub struct FileUrl(String);

impl FileUrl {
    /// Take a `fileurl` as the catalog spells it, rejecting what cannot name a file underneath a
    /// directory of ours.
    ///
    /// The catalog is fetched over the network, and its `fileurl` decides where bytes land on this
    /// machine, so a component that escapes the firmware directory has to be refused rather than
    /// trusted not to appear.
    pub fn new(fileurl: &str) -> anyhow::Result<Self> {
        let relative = fileurl.trim_start_matches('/');
        for component in relative.split('/') {
            match component {
                "" => bail!("{fileurl:?} has an empty component"),
                "." | ".." => bail!("{fileurl:?} has a {component:?} component"),
                // A drive-relative or UNC-looking component would escape the directory on Windows.
                c if c.contains(':') || c.contains('\\') => {
                    bail!("{fileurl:?} has a component that is not a plain file name: {c:?}")
                }
                _ => {}
            }
        }
        Ok(Self(relative.to_string()))
    }

    /// The location relative to whichever root it is resolved against.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Where the image is fetched from.
    pub fn url(&self) -> anyhow::Result<Url> {
        software_root()
            .join(&self.0)
            .with_context(|| format!("Failed to build a url for {:?}", self.0))
    }
}

impl TryFrom<String> for FileUrl {
    type Error = anyhow::Error;

    fn try_from(fileurl: String) -> Result<Self, Self::Error> {
        Self::new(&fileurl)
    }
}

impl Display for FileUrl {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A map from product name to software version to fimage url
pub type Index = BTreeMap<ShortName, BTreeMap<Version, FileUrl>>;

pub struct Database {
    dir: PathBuf,
    cookie_store: CookieStore,
}

impl Database {
    pub fn open_or_create(data_dir: Option<PathBuf>) -> anyhow::Result<Self> {
        let (db_dir, cookie_store) = match data_dir {
            None => {
                let dir = dirs::data_dir()
                    .context("Could not infer a data directory")?
                    .join("rs4a-firmware-inventory");
                (dir, CookieStore::open_default()?)
            }
            Some(custom) => {
                let cookie_store = CookieStore::new(custom.clone());
                (custom, cookie_store)
            }
        };
        fs::create_dir_all(&db_dir).context("Failed to create the data directory")?;
        Ok(Self {
            dir: db_dir,
            cookie_store,
        })
    }

    pub fn read_cookie(&self) -> anyhow::Result<Option<SessionCookie>> {
        self.cookie_store.read()
    }

    pub fn write_cookie(&self, cookie: &SessionCookie) -> anyhow::Result<()> {
        self.cookie_store.write(cookie)
    }

    pub fn read_index(&self) -> anyhow::Result<Index> {
        let file = self.dir.join(INDEX_FILE_NAME);
        match fs::read_to_string(&file) {
            Ok(t) => serde_json::from_str(&t)
                .context("Failed to deserialize index")
                .with_context(|| format!("Consider removing {file:?}")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                debug!("{INDEX_FILE_NAME} not found, returning an empty index");
                Ok(BTreeMap::new())
            }
            Err(e) => Err(e).context("Failed to read index")?,
        }
    }

    pub fn write_index(&self, index: &Index) -> anyhow::Result<()> {
        let index = serde_json::to_string_pretty(index).context("Failed to serialize index")?;
        fs::write(self.dir.join(INDEX_FILE_NAME), index)
            .context("Failed to write index")
            .map(|_| ())
    }

    /// Where an image is kept, mirroring its location under the software root.
    pub fn firmware_path(&self, fileurl: &FileUrl) -> PathBuf {
        let mut path = self.dir.join(FIRMWARE_DIR_NAME);
        path.extend(fileurl.as_str().split('/'));
        path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_normalized_drops_the_brand() {
        assert_eq!(
            ShortName::new_normalized("AXIS M1075-L").as_str(),
            "M1075-L"
        );
    }

    #[test]
    fn new_normalized_leaves_a_model_that_carries_no_brand_alone() {
        assert_eq!(
            ShortName::new_normalized("ExCam XF P1377").as_str(),
            "ExCam XF P1377"
        );
    }

    #[test]
    fn new_normalized_folds_the_3d_people_counter_onto_its_product_number() {
        assert_eq!(
            ShortName::new_normalized("AXIS P8815-2 3D People Counter").as_str(),
            "P8815-2"
        );
        assert_eq!(
            ShortName::new_normalized("AXIS P8815-2").as_str(),
            "P8815-2"
        );
    }

    #[test]
    fn new_normalized_keeps_a_hardware_revision_apart_from_its_predecessor() {
        assert_eq!(
            ShortName::new_normalized("AXIS M1075-L Mk II").as_str(),
            "M1075-L Mk II"
        );
    }

    #[test]
    fn a_fileurl_drops_the_leading_slash_the_catalog_writes() {
        let fileurl = FileUrl::new("/MPQT/M1075-L/12_9_57/M1075-L_12_9_57.bin").unwrap();
        assert_eq!(fileurl.as_str(), "MPQT/M1075-L/12_9_57/M1075-L_12_9_57.bin");
    }

    #[test]
    fn a_fileurl_resolves_against_the_software_root() {
        let fileurl = FileUrl::new("/MPQT/M1075-L/12_9_57/M1075-L_12_9_57.bin").unwrap();
        assert_eq!(
            fileurl.url().unwrap().as_str(),
            "https://www.axis.com/ftp/pub/axis/software/MPQT/M1075-L/12_9_57/M1075-L_12_9_57.bin"
        );
    }

    #[test]
    fn a_fileurl_refuses_to_name_a_file_outside_the_directory_it_is_joined_to() {
        for escape in [
            "/MPQT/../../../etc/passwd",
            "/MPQT/./M1075-L/12_9_57/x.bin",
            "/MPQT//12_9_57/x.bin",
            "/",
            "",
            "C:/windows/system32/x.bin",
            "MPQT/a\\..\\..\\b.bin",
        ] {
            assert!(
                FileUrl::new(escape).is_err(),
                "{escape:?} was accepted as a fileurl"
            );
        }
    }

    #[test]
    fn a_fileurl_read_back_from_an_index_is_checked_the_same_way() {
        let json = r#""MPQT/M1075-L/12_9_57/M1075-L_12_9_57.bin""#;
        assert_eq!(
            serde_json::from_str::<FileUrl>(json).unwrap(),
            FileUrl::new("MPQT/M1075-L/12_9_57/M1075-L_12_9_57.bin").unwrap()
        );
        assert!(serde_json::from_str::<FileUrl>(r#""MPQT/../../etc/x.bin""#).is_err());
    }

    #[test]
    fn firmware_path_mirrors_the_software_root_under_the_data_directory() {
        let db = Database {
            dir: PathBuf::from("/data"),
            cookie_store: CookieStore::new(PathBuf::from("/data")),
        };
        let fileurl = FileUrl::new("/MPQT/M1075-L/12_9_57/M1075-L_12_9_57.bin").unwrap();
        assert_eq!(
            db.firmware_path(&fileurl),
            PathBuf::from("/data/firmware/MPQT/M1075-L/12_9_57/M1075-L_12_9_57.bin")
        );
    }

    #[test]
    fn a_short_name_round_trips_through_a_json_map_key() {
        let index = BTreeMap::from([(ShortName::new_normalized("AXIS P8815-2"), 1)]);
        let json = serde_json::to_string(&index).unwrap();
        assert_eq!(json, r#"{"P8815-2":1}"#);
        assert_eq!(
            serde_json::from_str::<BTreeMap<ShortName, u8>>(&json).unwrap(),
            index
        );
    }
}
