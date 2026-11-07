use std::{borrow::Cow, cmp::Ordering, str::FromStr};

use anyhow::bail;
use rs4a_vapix::apis::basic_device_info_1::{
    Architecture, RestrictedProperties, UnrestrictedProperties,
};
use rs4a_vlt::responses::{DeviceArchitecture, Loan};
use semver::{BuildMetadata, Version, VersionReq};
use url::Host;

use crate::mdns_source;

pub struct Device {
    basic_device_info: Option<UnrestrictedProperties>,
    dut_device: Option<rs4a_dut::Device>,
    inventory_device: Option<(String, crate::db::Device)>,
    mdns_device: Option<mdns_source::Device>,
    vlt_loan: Option<Loan>,
    firmware: Option<Version>,
    architecture: Option<DeviceArchitecture>,
}

// TODO: Consider removing the non-exhaustive attribute on `Architecture`
fn convert_architecture(a: Architecture) -> anyhow::Result<DeviceArchitecture> {
    match a {
        Architecture::Aarch64 => Ok(DeviceArchitecture::Aarch64),
        Architecture::Armv7hf => Ok(DeviceArchitecture::Armv7hf),
        Architecture::Armv7l => Ok(DeviceArchitecture::Armv7l),
        Architecture::Mips => Ok(DeviceArchitecture::Mips),
    }
}

fn coerce_firmware_version(s: &str) -> anyhow::Result<Version> {
    let mut parts = s.splitn(4, '.');
    let major = parts.next().unwrap_or_default().parse()?;
    let minor = parts.next().unwrap_or_default().parse()?;
    let patch = parts.next().unwrap_or_default().parse()?;
    let build = match parts.next() {
        None => BuildMetadata::EMPTY,
        Some(s) => BuildMetadata::from_str(s)?,
    };
    Ok(Version {
        major,
        minor,
        patch,
        pre: Default::default(),
        build,
    })
}

impl Device {
    pub fn cmp(lhs: &Self, other: &Self) -> Ordering {
        lhs.priority()
            .partial_cmp(&other.priority())
            .or_else(|| lhs.host().partial_cmp(&other.host()))
            .or_else(|| lhs.http_port().partial_cmp(&other.http_port()))
            .unwrap_or(Ordering::Equal)
    }

    pub fn from_dut_device(device: rs4a_dut::Device) -> Self {
        Self {
            basic_device_info: None,
            dut_device: Some(device),
            inventory_device: None,
            mdns_device: None,
            vlt_loan: None,
            firmware: None,
            architecture: None,
        }
    }
    pub fn from_inventory_device(alias: String, device: crate::db::Device) -> Self {
        Self {
            basic_device_info: None,
            dut_device: None,
            inventory_device: Some((alias, device)),
            mdns_device: None,
            vlt_loan: None,
            firmware: None,
            architecture: None,
        }
    }
    pub fn from_mdns_device(device: crate::mdns_source::Device) -> Self {
        Self {
            basic_device_info: None,
            dut_device: None,
            inventory_device: None,
            mdns_device: Some(device),
            vlt_loan: None,
            firmware: None,
            architecture: None,
        }
    }

    pub fn from_vlt_loan(loan: Loan) -> Self {
        Self {
            basic_device_info: None,
            dut_device: None,
            inventory_device: None,
            vlt_loan: Some(loan),
            mdns_device: None,
            firmware: None,
            architecture: None,
        }
    }

    pub fn fingerprint(&self) -> String {
        let Self {
            basic_device_info: _,
            dut_device: from_active,
            inventory_device: from_inventory,
            vlt_loan: from_loan,
            mdns_device,
            firmware: _,
            architecture: _,
        } = self;
        let from_active = from_active.as_ref().map(active_fingerprint);
        let from_inventory = from_inventory
            .as_ref()
            .map(|(_, d)| inventory_fingerprint(d));
        let from_loan = from_loan.as_ref().map(loan_fingerprint);
        let mdns_device = mdns_device.as_ref().map(mdns_fingerprint);

        from_active
            .or(from_inventory)
            .or(from_loan)
            .or(mdns_device)
            .expect("At least one field is some")
    }

    pub fn add_unrestricted_properties(
        &mut self,
        properties: UnrestrictedProperties,
    ) -> anyhow::Result<()> {
        let UnrestrictedProperties { version, .. } = &properties;

        let new = coerce_firmware_version(version)?;
        if let Some(old) = self.firmware.as_ref() {
            if old != &new {
                bail!("Attempted to add conflicting firmware")
            }
        } else {
            self.firmware = Some(new);
        }

        self.basic_device_info = Some(properties);

        Ok(())
    }

    pub fn add_restricted_properties(
        &mut self,
        properties: RestrictedProperties,
    ) -> anyhow::Result<()> {
        let RestrictedProperties { architecture, .. } = properties;

        let new = convert_architecture(architecture)?;
        if let Some(old) = self.architecture.as_ref() {
            if old != &new {
                bail!("Attempted to add conflicting architecture")
            }
        } else {
            self.architecture = Some(new);
        }

        Ok(())
    }

    pub fn replace_dut_device(&mut self, device: rs4a_dut::Device) -> Option<rs4a_dut::Device> {
        self.dut_device.replace(device)
    }

    pub fn replace_inventory_device(
        &mut self,
        alias: String,
        device: crate::db::Device,
    ) -> Option<(String, crate::db::Device)> {
        self.inventory_device.replace((alias, device))
    }

    pub fn replace_vlt_loan(&mut self, loan: Loan) -> Option<Loan> {
        self.vlt_loan.replace(loan)
    }

    pub fn replace_mdns_device(
        &mut self,
        device: mdns_source::Device,
    ) -> Option<mdns_source::Device> {
        self.mdns_device.replace(device)
    }

    pub(crate) fn alias(&self) -> Option<Cow<str>> {
        if let Some((a, _)) = self.inventory_device.as_ref() {
            return Some(a.into());
        }
        None
    }

    pub(crate) fn architecture(&self) -> Option<DeviceArchitecture> {
        self.architecture
    }

    pub(crate) fn firmware(&self) -> Option<&Version> {
        self.firmware.as_ref()
    }

    pub(crate) fn host(&self) -> Host {
        if let Some(d) = self.dut_device.as_ref() {
            return d.host.clone();
        }
        if let Some((_, d)) = self.inventory_device.as_ref() {
            return d.host.clone();
        }
        if let Some(d) = self.vlt_loan.as_ref() {
            return d.host();
        }
        if let Some(d) = self.mdns_device.as_ref() {
            return d.host.clone();
        }
        unreachable!()
    }

    /// Returns any port mapping for the HTTP port.
    ///
    /// This is how the values should be interpreted:
    ///
    /// - `None`: Not known, because the device is not accessible or should not be accessed.
    /// - `Some(None)`: No port remapping.
    /// - `Some(Some(p))`: The port has been remapped to `p`
    pub fn http_port(&self) -> Option<Option<u16>> {
        let mut values = Vec::new();
        values.extend(self.dut_device.as_ref().map(|d| d.http_port));
        values.extend(self.inventory_device.as_ref().map(|(_, d)| d.http_port));
        values.extend(self.vlt_loan.as_ref().map(|d| match d.http_port() {
            80 => None,
            p => Some(p),
        }));
        values.extend(self.mdns_device.as_ref().map(|_| None));
        values.dedup();
        debug_assert!(values.len() < 2);
        values.pop()
    }

    /// Returns any port mapping for the HTTPS port.
    ///
    /// This is how the values should be interpreted:
    ///
    /// - `None`: Not known, because the device is not accessible or should not be accessed.
    /// - `Some(None)`: No port remapping.
    /// - `Some(Some(p))`: The port has been remapped to `p`
    pub fn https_port(&self) -> Option<Option<u16>> {
        let mut values = Vec::new();
        values.extend(self.dut_device.as_ref().map(|d| d.https_port));
        values.extend(self.inventory_device.as_ref().map(|(_, d)| d.https_port));
        values.extend(self.vlt_loan.as_ref().map(|d| match d.https_port() {
            443 => None,
            p => Some(p),
        }));
        values.extend(self.mdns_device.as_ref().map(|_| None));
        values.dedup();
        debug_assert!(values.len() < 2);
        values.pop()
    }

    pub fn username(&self) -> Option<String> {
        let mut values = Vec::new();
        values.extend(self.dut_device.as_ref().map(|d| d.username.to_string()));
        values.extend(
            self.inventory_device
                .as_ref()
                .map(|(_, d)| d.username.clone()),
        );
        values.extend(self.vlt_loan.as_ref().map(|d| d.username.clone()));
        values.dedup();
        debug_assert!(values.len() < 2);
        values.pop()
    }

    pub fn password(&self) -> Option<String> {
        let mut values = Vec::new();
        values.extend(self.dut_device.as_ref().map(|d| d.password.clone()));
        // TODO: Consider propagating the password type or removing it altogether.
        values.extend(
            self.inventory_device
                .as_ref()
                .map(|(_, d)| d.password.dangerous_reveal().to_string()),
        );
        values.extend(self.vlt_loan.as_ref().map(|d| d.password.clone()));
        values.dedup();
        debug_assert!(values.len() < 2);
        values.pop()
    }

    pub(crate) fn model(&self) -> Option<Cow<str>> {
        if let Some(d) = self.basic_device_info.as_ref() {
            return Some(d.prod_short_name.as_str().into());
        }
        if let Some(d) = self.vlt_loan.as_ref() {
            return Some(d.loanable.model.as_str().into());
        }
        None
    }

    pub fn priorities(&self) -> Vec<u8> {
        let mut priorities = Vec::new();
        if self.dut_device.is_some() {
            priorities.push(0)
        }
        if self.inventory_device.is_some() {
            priorities.push(1)
        }
        if self.vlt_loan.is_some() {
            priorities.push(2)
        }
        if self.mdns_device.is_some() {
            priorities.push(3)
        }
        priorities
    }

    fn priority(&self) -> u8 {
        self.priorities()
            .into_iter()
            .min()
            .expect("Constructors ensure at least one source is set")
    }

    pub(crate) fn serial(&self) -> Option<Cow<str>> {
        if let Some(d) = self.basic_device_info.as_ref() {
            return Some(d.serial_number.as_str().into());
        }
        if let Some(d) = self.mdns_device.as_ref() {
            return Some(d.to_serial().into());
        }
        None
    }

    pub fn is_matched_by(&self, device_filter: &DeviceFilter) -> bool {
        device_filter.matches(BorrowedDevice {
            alias: self.alias(),
            architecture: self.architecture(),
            firmware: self.firmware(),
            model: self.model(),
        })
    }
}

impl Device {}

// TODO: Consider making these mandatory.
pub struct BorrowedDevice<'a> {
    pub(crate) alias: Option<Cow<'a, str>>,
    pub(crate) architecture: Option<DeviceArchitecture>,
    pub(crate) firmware: Option<&'a Version>,
    pub(crate) model: Option<Cow<'a, str>>,
}

#[derive(Clone, Debug, clap::Parser)]
pub struct DeviceFilterParser {
    /// Consider only devices with a matching alias.
    ///
    /// Accepts a glob pattern.
    #[arg(long)]
    alias: Option<String>,
    /// Consider only devices with a matching architecture.
    #[arg(long, short)]
    architecture: Option<DeviceArchitecture>,
    /// Consider only devices with a matching firmware version.
    ///
    /// Accepts the same version requirement syntax as Cargo, see
    /// <https://doc.rust-lang.org/cargo/reference/specifying-dependencies.html#version-requirement-syntax>
    ///
    /// Older firmware versions, which don't follow SemVer, are coerced into SemVer.
    #[arg(long)]
    firmware: Option<VersionReq>,
    /// Consider only devices with a matching model.
    ///
    /// Accepts a glob pattern.
    #[arg(long, short)]
    model: Option<String>,
}

impl DeviceFilterParser {
    pub fn into_filter(self) -> anyhow::Result<DeviceFilter> {
        let Self {
            alias,
            architecture,
            firmware,
            model,
        } = self;
        let alias = alias
            .map(|s| glob::Pattern::new(s.to_lowercase().as_str()))
            .transpose()?;
        let model = model
            .map(|s| glob::Pattern::new(s.to_lowercase().as_str()))
            .transpose()?;
        Ok(DeviceFilter {
            alias,
            model,
            architecture,
            firmware,
        })
    }
}

pub struct DeviceFilter {
    alias: Option<glob::Pattern>,
    model: Option<glob::Pattern>,
    architecture: Option<DeviceArchitecture>,
    firmware: Option<VersionReq>,
}

impl DeviceFilter {
    pub fn matches(&self, d: BorrowedDevice) -> bool {
        let BorrowedDevice {
            alias,
            architecture,
            firmware,
            model,
        } = d;

        if let Some(p) = self.alias.as_ref() {
            if let Some(alias) = alias {
                if !p.matches(&alias.to_lowercase()) {
                    return false;
                }
            } else {
                return false;
            }
        }

        if let Some(p) = self.model.as_ref() {
            if let Some(model) = model {
                if !p.matches(&model.to_lowercase()) {
                    return false;
                }
            } else {
                return false;
            }
        }

        if let Some(a) = self.architecture {
            if let Some(architecture) = architecture {
                if a != architecture {
                    return false;
                }
            } else {
                return false;
            }
        }
        if let Some(req) = self.firmware.as_ref() {
            if let Some(v) = firmware {
                if !req.matches(v) {
                    return false;
                }
            } else {
                return false;
            }
        }

        true
    }
}

pub fn active_fingerprint(d: &rs4a_dut::Device) -> String {
    format!("{}:{}", d.host, d.http_port.unwrap_or(80))
}

pub fn inventory_fingerprint(d: &crate::db::Device) -> String {
    format!("{}:{}", d.host, d.http_port.unwrap_or(80))
}

pub fn loan_fingerprint(d: &Loan) -> String {
    format!("{}:{}", d.host(), d.http_port())
}

pub fn mdns_fingerprint(d: &mdns_source::Device) -> String {
    // If the device is on the local network, then there probably isn't any port mapping going on.
    format!("{}:80", d.host)
}
