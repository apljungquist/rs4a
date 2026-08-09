use std::{
    cmp::Ordering,
    fs::File,
    io::BufReader,
    path::{Path, PathBuf},
};

use anyhow::Context;
use log::info;
use rs4a_fimage::{archive::read_info_json, info::ImageInfo};
use rs4a_vapix::apis::{
    basic_device_info_1::GetAllUnrestrictedPropertiesRequest,
    firmware_management_1::{AutoCommit, FactoryDefaultMode},
};
use semver::{Version, VersionReq};

#[derive(Clone, Debug, clap::Args)]
pub struct InstallCommand {
    #[command(flatten)]
    netloc: rs4a_device_manager::Netloc,
    /// Semver version requirement (e.g. "12", "^12.8", "<13")
    version: VersionReq,
    /// Location of the firmware-inventory data.
    #[clap(long, env = "FIRMWARE_INVENTORY_LOCATION")]
    inventory: Option<PathBuf>,
    /// Do not fetch firmware that is not already cached locally.
    #[clap(long, env = "FIRMWARE_INVENTORY_OFFLINE")]
    offline: bool,
    /// Auto-commit behavior after upgrade
    #[arg(long, short = 'c')]
    auto_commit: Option<AutoCommit>,
    /// Auto-rollback behavior: "never", or minutes
    #[arg(long, short = 'r')]
    auto_rollback: Option<String>,
    /// Device initialization profile
    #[arg(long, default_value_t)]
    profile: rs4a_device_manager::Profile,
}

/// The version an image installs, according to the image itself.
fn version_from_firmware(path: &Path) -> anyhow::Result<Version> {
    let file = File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let info: ImageInfo = read_info_json(BufReader::new(file))
        .with_context(|| format!("Failed to read the metadata in {}", path.display()))?
        .parse()
        .with_context(|| format!("Failed to parse the metadata in {}", path.display()))?;
    Version::parse(&info.release)
        .with_context(|| format!("Could not parse a version from release {:?}", info.release))
}

impl InstallCommand {
    pub async fn exec(self) -> anyhow::Result<String> {
        let Self {
            netloc,
            version,
            inventory,
            offline,
            auto_commit,
            auto_rollback,
            profile,
        } = self;

        info!("Querying device for model and version");
        let client = netloc.connect().await?;
        let props = GetAllUnrestrictedPropertiesRequest::new()
            .send(&client)
            .await?
            .property_list;
        let current = props.parse_version()?;
        let model = match props.prod_nbr {
            s if s == "P8815-2" => "P8815-2_3D_People_Counter".to_string(),
            s => s,
        };
        info!("Device model: {model}, version: {current}");

        let product = glob::Pattern::new(&model)
            .with_context(|| format!("Invalid product glob derived from model '{model}'"))?;

        let update_cli = rs4a_firmware_inventory::Cli {
            inventory: inventory.clone(),
            offline,
            command: rs4a_firmware_inventory::Commands::Update(
                rs4a_firmware_inventory::UpdateCommand {
                    product: product.clone(),
                },
            ),
        };
        update_cli.exec().await?;

        let get_cli = rs4a_firmware_inventory::Cli {
            inventory,
            offline,
            command: rs4a_firmware_inventory::Commands::Get(rs4a_firmware_inventory::GetCommand {
                products: vec![product],
                selector: rs4a_firmware_inventory::Selector {
                    version: Some(version),
                    track: None,
                },
            }),
        };
        let firmware_output = get_cli.exec().await?;
        let firmware_path = PathBuf::from(firmware_output.trim_end_matches('\n'));
        info!("Firmware resolved to {}", firmware_path.display());

        let target = version_from_firmware(&firmware_path)?;
        let factory_default_mode = match target.cmp(&current) {
            Ordering::Less => {
                info!("Target {target} < current {current}: downgrade with factory default");
                Some(FactoryDefaultMode::Hard)
            }
            Ordering::Equal => {
                info!("Target {target} == current {current}: exiting early");
                return Ok(String::new());
            }
            Ordering::Greater => {
                info!("Target {target} > current {current}: upgrade without factory default");
                None
            }
        };

        let upgrade_cli = rs4a_device_manager::Cli {
            command: rs4a_device_manager::Commands::Upgrade(rs4a_device_manager::UpgradeCommand {
                netloc: netloc.clone(),
                firmware: firmware_path,
                factory_default_mode,
                auto_commit,
                auto_rollback,
            }),
        };
        upgrade_cli.exec().await?;

        if factory_default_mode.is_some() {
            info!("Running init after downgrade");
            let init_cli = rs4a_device_manager::Cli {
                command: rs4a_device_manager::Commands::Init(rs4a_device_manager::InitCommand {
                    netloc,
                    profile,
                }),
            };
            init_cli.exec().await?;
        }

        Ok(String::new())
    }
}
