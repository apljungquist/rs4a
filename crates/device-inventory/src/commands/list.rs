use std::collections::{hash_map::Entry, HashMap};

use anyhow::Context;
use log::warn;
use rs4a_vapix::apis::basic_device_info_1::{
    AllProperties, GetAllPropertiesRequest, GetAllUnrestrictedPropertiesRequest,
    RestrictedProperties, UnrestrictedProperties,
};
use rs4a_vlt::requests;
use tokio::task::JoinSet;
use url::Host;

use crate::{
    db::Database,
    db_vlt,
    fusion::{
        active_fingerprint, inventory_fingerprint, loan_fingerprint, mdns_fingerprint, Device,
        DeviceFilterParser,
    },
    mdns_source,
};

struct Table {
    aliases: Vec<String>,
    architectures: Vec<String>,
    firmware: Vec<String>,
    hosts: Vec<String>,
    models: Vec<String>,
    serials: Vec<String>,
    priorities: Vec<String>,
}

impl Table {
    fn new() -> Self {
        Self {
            aliases: vec!["ALIAS".to_string()],
            architectures: vec!["ARCHITECTURE".to_string()],
            firmware: vec!["FIRMWARE".to_string()],
            hosts: vec!["HOST".to_string()],
            models: vec!["MODEL".to_string()],
            serials: vec!["SERIAL".to_string()],
            priorities: vec!["PRIORITIES".to_string()],
        }
    }

    fn push(&mut self, row: Device) {
        self.aliases
            .push(row.alias().unwrap_or_default().to_string());
        self.architectures.push(
            row.architecture()
                .map(|a| a.as_str().to_string())
                .unwrap_or_default(),
        );
        self.firmware
            .push(row.firmware().map(|v| v.to_string()).unwrap_or_default());
        self.hosts.push(row.host().to_string());
        self.models
            .push(row.model().unwrap_or_default().to_string());
        self.serials
            .push(row.serial().map(|s| s.to_string()).unwrap_or_default());
        self.priorities.push(
            row.priorities()
                .into_iter()
                .map(|p| p.to_string())
                .collect::<Vec<_>>()
                .join(","),
        );
    }

    pub fn pretty_print(self) {
        let Self {
            aliases,
            architectures,
            firmware,
            hosts,
            models,
            serials,
            priorities,
        } = self;

        let alias_width = 1 + aliases.iter().map(|s| s.len()).max().unwrap();
        let firmware_width = 1 + firmware.iter().map(|s| s.len()).max().unwrap();
        let architecture_width = 1 + architectures.iter().map(|s| s.len()).max().unwrap();
        let model_width = 1 + models.iter().map(|s| s.len()).max().unwrap();
        let serial_width = 1 + serials.iter().map(|s| s.len()).max().unwrap();
        let priority_width = 1 + priorities.iter().map(|s| s.len()).max().unwrap();

        for ((((((priority, alias), model), host), architecture), serial), firmware) in priorities
            .into_iter()
            .zip(aliases.into_iter())
            .zip(models.into_iter())
            .zip(hosts.into_iter())
            .zip(architectures.into_iter())
            .zip(serials.into_iter())
            .zip(firmware.into_iter())
        {
            println!(
                "{priority:priority_width$} {alias:alias_width$} {serial:serial_width$} {model:model_width$} {architecture:architecture_width$} {firmware:firmware_width$} {host}"
            );
        }
    }
}

#[expect(clippy::too_many_arguments, reason = "TODO: Improve this")]
async fn probe_device(
    offline: bool,
    fingerprint: String,
    host: Host,
    http_port: Option<u16>,
    https_port: Option<u16>,
    username: Option<String>,
    password: Option<String>,
    https_self_signed: bool,
) -> anyhow::Result<(String, UnrestrictedProperties, Option<RestrictedProperties>)> {
    assert!(!offline);
    match (username, password) {
        (Some(username), Some(password)) => {
            let client = rs4a_vapix::Client::builder(host)
                .plain_port(http_port)
                .secure_port(https_port)
                .with_inner(|b| b.danger_accept_invalid_certs(https_self_signed))
                .username_password(&username, &password)
                .build()
                .await
                .context("Could not create client")?;

            let AllProperties {
                unrestricted,
                restricted,
            } = GetAllPropertiesRequest::new()
                .send(&client)
                .await?
                .property_list;
            Ok((fingerprint, unrestricted, Some(restricted)))
        }
        _ => {
            let client = rs4a_vapix::Client::builder(host)
                .plain_port(http_port)
                .secure_port(https_port)
                .with_inner(|b| b.danger_accept_invalid_certs(https_self_signed))
                .build()
                .await
                .context("Could not create client")?;

            let unrestricted = GetAllUnrestrictedPropertiesRequest::new()
                .send(&client)
                .await?
                .property_list;
            Ok((fingerprint, unrestricted, None))
        }
    }
}

#[derive(Clone, Debug, clap::Parser)]
pub struct ListCommand {
    /// Probe devices for properties not indexed by the source.
    #[arg(long)]
    probe: bool,
    /// Consider devices from mDNS.
    #[arg(long, env = "DI_MDNS")]
    mdns: bool,
    /// Consider loans from the VLT.
    #[arg(long, env = "DI_VLT")]
    vlt: bool,
    /// Accept self-signed HTTPS certificates when probing devices.
    #[arg(long, env = "AXIS_DEVICE_HTTPS_SELF_SIGNED", value_parser = clap::builder::BoolishValueParser::new())]
    https_self_signed: bool,
    #[command(flatten)]
    device_filter: DeviceFilterParser,
}

impl ListCommand {
    pub async fn exec(self, db: &Database, offline: bool) -> anyhow::Result<()> {
        let Self {
            probe,
            mdns,
            vlt,
            https_self_signed,
            device_filter,
        } = self;
        let device_filter = device_filter.into_filter()?;

        let mut devices: HashMap<String, Device> = HashMap::new();

        if let Some(d) = rs4a_dut::Device::from_env()? {
            let f = active_fingerprint(&d);
            match devices.entry(f) {
                Entry::Occupied(mut e) => {
                    e.get_mut().replace_dut_device(d);
                }
                Entry::Vacant(e) => {
                    e.insert(Device::from_dut_device(d));
                }
            };
        }

        for (a, d) in db.read_devices()? {
            let f = inventory_fingerprint(&d);
            match devices.entry(f) {
                Entry::Occupied(mut e) => {
                    e.get_mut().replace_inventory_device(a, d);
                }
                Entry::Vacant(e) => {
                    e.insert(Device::from_inventory_device(a, d));
                }
            };
        }

        if mdns {
            for d in mdns_source::discover_devices().await? {
                let f = mdns_fingerprint(&d);
                match devices.entry(f) {
                    Entry::Occupied(mut e) => {
                        e.get_mut().replace_mdns_device(d);
                    }
                    Entry::Vacant(e) => {
                        e.insert(Device::from_mdns_device(d));
                    }
                };
            }
        }

        let vlt_client = match vlt {
            true => Some(
                db_vlt::client(db, offline)
                    .await?
                    .context("VLT is not configured, skipping VLT loans")?,
            ),
            false => None,
        };

        if let Some(client) = vlt_client.as_ref() {
            for d in requests::loans().send(client).await? {
                let f = loan_fingerprint(&d);
                match devices.entry(f) {
                    Entry::Occupied(mut e) => e.get_mut().replace_vlt_loan(d),
                    Entry::Vacant(e) => {
                        e.insert(Device::from_vlt_loan(d));
                        None
                    }
                };
            }
        }

        if probe {
            let mut join_set = JoinSet::new();
            for device in devices.values() {
                join_set.spawn(probe_device(
                    offline,
                    device.fingerprint(),
                    device.host(),
                    device
                        .http_port()
                        .expect("all devices added have a http port"),
                    device
                        .https_port()
                        .expect("all devices added have a https port"),
                    device.username(),
                    device.password(),
                    https_self_signed,
                ));
            }
            for r in join_set.join_all().await {
                match r {
                    Ok((fingerprint, unrestricted, restricted)) => {
                        let device = devices
                            .get_mut(&fingerprint)
                            .expect("Fingerprint comes from a device already in devices");
                        device.add_unrestricted_properties(unrestricted)?;
                        if let Some(restricted) = restricted {
                            device.add_restricted_properties(restricted)?;
                        }
                    }
                    Err(e) => {
                        warn!("Could not get properties for a device: {e:?}");
                    }
                }
            }
        }

        let mut devices = devices.into_values().collect::<Vec<_>>();
        devices.sort_by(Device::cmp);

        let mut table = Table::new();
        for device in devices {
            if device.is_matched_by(&device_filter) {
                table.push(device);
            }
        }
        table.pretty_print();

        Ok(())
    }
}
