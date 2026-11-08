//! Facilities for parsing responses.
use std::{
    fmt::{Display, Formatter},
    net::Ipv4Addr,
    path::PathBuf,
    str::FromStr,
};

use anyhow::{anyhow, Context};
use chrono::{DateTime, SecondsFormat, Utc};
use serde::{ser::SerializeSeq, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use url::Host;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FirmwareVersion(String);

impl Display for FirmwareVersion {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct DataEnvelope<T> {
    success: bool,
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    meta: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Serialize)]
struct NoDataEnvelope {
    success: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error(transparent)]
    InvalidJson(anyhow::Error),
    #[error(transparent)]
    SchemaMismatch(anyhow::Error),
    #[error("Not successful")]
    Remote,
}

fn debug_assert_lossless<T>(s: &str, data: &T, meta: Option<Vec<String>>)
where
    T: Clone + for<'de> Deserialize<'de> + Serialize,
{
    let envelope = DataEnvelope {
        success: true,
        data: Some(data.clone()),
        meta,
    };
    let actual = serde_json::to_string(&envelope).unwrap();
    let actual = serde_json::from_str::<Value>(&actual).unwrap();
    let expected = serde_json::from_str::<Value>(s).unwrap();
    debug_assert_eq!(actual, expected);
}

pub fn parse_data<T>(s: &str) -> Result<T, ParseError>
where
    T: Clone + for<'de> Deserialize<'de> + Serialize,
{
    match serde_json::from_str(s) {
        Ok(envelope) => {
            let DataEnvelope::<T> {
                success,
                data,
                meta,
            } = envelope;
            match (success, data) {
                (false, None) => Err(ParseError::Remote),
                (false, Some(_)) => {
                    debug_assert!(false);
                    Err(ParseError::Remote)
                }
                (true, None) => match serde_json::from_str::<T>("null") {
                    Ok(data) => Ok(data),
                    Err(_) => Err(ParseError::SchemaMismatch(anyhow::anyhow!(
                        "Response was a success, but data was missing"
                    ))),
                },
                (true, Some(data)) => {
                    debug_assert_lossless(s, &data, meta);
                    Ok(data)
                }
            }
        }
        Err(e) => match serde_json::from_str::<DataEnvelope<Value>>(s) {
            Ok(_) => Err(ParseError::SchemaMismatch(e.into())),
            Err(e) => Err(ParseError::InvalidJson(e.into())),
        },
    }
}

#[non_exhaustive]
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "lowercase")]
pub enum DeviceArchitecture {
    Aarch64,
    Armv7hf,
    Armv7l,
    Mips,
}

impl DeviceArchitecture {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeviceArchitecture::Aarch64 => "aarch64",
            DeviceArchitecture::Armv7hf => "armv7hf",
            DeviceArchitecture::Armv7l => "armv7l",
            DeviceArchitecture::Mips => "mips",
        }
    }
}

#[non_exhaustive]
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum, serde::Deserialize, serde::Serialize,
)]
#[serde(try_from = "u8", into = "u8")]
pub enum DeviceStatus {
    Connected = 1,
    OnLoan = 3,
}

impl DeviceStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            DeviceStatus::Connected => "connected",
            DeviceStatus::OnLoan => "on-loan",
        }
    }
}

impl TryFrom<u8> for DeviceStatus {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(DeviceStatus::Connected),
            3 => Ok(DeviceStatus::OnLoan),
            _ => Err(format!("Unknown device status: {value}")),
        }
    }
}

impl From<DeviceStatus> for u8 {
    #[expect(
        clippy::as_conversions,
        reason = "better than the alternative: enumerating variants and risking it goes out of sync"
    )]
    fn from(status: DeviceStatus) -> Self {
        status as u8
    }
}

#[non_exhaustive]
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, clap::ValueEnum, serde::Deserialize, serde::Serialize,
)]
#[serde(try_from = "u8", into = "u8")]
pub enum LoanStatus {
    Active = 1,
    Expired = 2,
    Cancelled = 3,
    Scheduled = 4,
    /// The device is installing the requested version and is not yet usable.
    ///
    /// This is transient, and lasts for a minute or two at the start of a loan.
    UpgradingOs = 5,
}

impl LoanStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            LoanStatus::Active => "active",
            LoanStatus::Expired => "expired",
            LoanStatus::Cancelled => "cancelled",
            LoanStatus::Scheduled => "scheduled",
            LoanStatus::UpgradingOs => "upgrading-os",
        }
    }
}

impl TryFrom<u8> for LoanStatus {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(LoanStatus::Active),
            2 => Ok(LoanStatus::Expired),
            3 => Ok(LoanStatus::Cancelled),
            4 => Ok(LoanStatus::Scheduled),
            5 => Ok(LoanStatus::UpgradingOs),
            _ => Err(format!("Unknown loan status: {value}")),
        }
    }
}

impl From<LoanStatus> for u8 {
    #[expect(
        clippy::as_conversions,
        reason = "better than the alternative: enumerating variants and risking it goes out of sync"
    )]
    fn from(status: LoanStatus) -> Self {
        status as u8
    }
}

fn serialize_datetime<S>(v: &DateTime<Utc>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let s = v.to_rfc3339_opts(SecondsFormat::Millis, true);
    serializer.serialize_str(&s)
}

fn serialize_datetime_array<S>(vs: &Vec<DateTime<Utc>>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let mut seq = serializer.serialize_seq(Some(vs.len()))?;
    for v in vs {
        let s = v.to_rfc3339_opts(SecondsFormat::Millis, true);
        seq.serialize_element(&s)?;
    }
    seq.end()
}

fn serialize_semicolon_separated_list<S>(
    list: &[FirmwareVersion],
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let list = list.iter().map(|fw| fw.0.to_string()).collect::<Vec<_>>();
    let s = list.join(";");
    serializer.serialize_str(&s)
}

fn deserialize_semicolon_separated_list<'de, D>(
    deserializer: D,
) -> Result<Vec<FirmwareVersion>, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Ok(s.split(';')
        .map(|s| FirmwareVersion(s.to_string()))
        .collect())
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct ExternalIp(Ipv4Addr);

impl ExternalIp {
    pub fn to_bits(&self) -> u32 {
        self.0.to_bits()
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "String")]
pub struct InternalIp {
    host: Ipv4Addr,
    base_port: u16,
}

impl InternalIp {
    fn forwarded_port(&self, protocol_index: u8) -> u16 {
        // Replace the first digit of `self.base_port` with `protocol_index`
        let scale = 10u16.pow(self.base_port.checked_ilog10().unwrap_or(0));
        let suffix = self.base_port % scale;
        u16::from(protocol_index) * scale + suffix
    }

    /// Returns the port forwarded to port 80.
    pub fn http_port(&self) -> u16 {
        self.forwarded_port(1)
    }

    /// Returns the port forwarded to port 443.
    pub fn https_port(&self) -> u16 {
        self.forwarded_port(4)
    }

    /// Returns the port forwarded to port 22.
    pub fn ssh_port(&self) -> u16 {
        self.forwarded_port(2)
    }

    /// Returns the port forwarded to port 554.
    pub fn rtsp_port(&self) -> u16 {
        self.forwarded_port(3)
    }
}

impl Display for InternalIp {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.host, self.base_port)
    }
}

impl Serialize for InternalIp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl TryFrom<String> for InternalIp {
    type Error = anyhow::Error;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        let (host, port) = s
            .rsplit_once(':')
            .ok_or_else(|| anyhow!("Internal IP `{s}` has no port"))?;
        let base_port = port
            .parse()
            .context("Internal IP `{s}` has an invalide port")?;
        let host =
            Ipv4Addr::from_str(host).context("Internal IP `{s}` has an invalid IP address")?;
        Ok(Self { host, base_port })
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PortcastDevice {
    pub id: u16,
    pub device_id: u16,
    pub raw_name: String,
    pub model: String,
    pub r#type: String,
    #[serde(serialize_with = "serialize_datetime")]
    pub created_at: DateTime<Utc>,
}

#[non_exhaustive]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Device {
    pub architecture: DeviceArchitecture,
    #[serde(deserialize_with = "deserialize_semicolon_separated_list")]
    #[serde(serialize_with = "serialize_semicolon_separated_list")]
    pub available_fw_versions: Vec<FirmwareVersion>,
    #[serde(serialize_with = "serialize_datetime_array")]
    pub booked: Vec<DateTime<Utc>>,
    /// Despite it's name, the device is not accessible from the internet at this IP.
    pub external_ip: ExternalIp,
    pub firmware_version: FirmwareVersion,
    pub id: LoanableId,
    pub image_url: PathBuf,
    pub model: String,
    pub platform: String,
    pub portcast: bool,
    portcast_device: Option<PortcastDevice>,
    pub release_year: u16,
    pub resolution: Option<String>,
    pub sdcard: bool,
    pub status: DeviceStatus,
    pub r#type: String,
}

impl Device {
    pub fn host(&self) -> Host {
        Host::Ipv4(Ipv4Addr::from([195, 60, 68, 14]))
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NewLoanable {
    pub id: LoanableId,
    pub internal_ip: InternalIp,
    pub model: String,
}

#[non_exhaustive]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NewLoan {
    #[serde(serialize_with = "serialize_datetime")]
    pub loan_start: DateTime<Utc>,
    #[serde(serialize_with = "serialize_datetime")]
    pub loan_end: DateTime<Utc>,
    pub id: LoanId,
    pub selected_firmware: FirmwareVersion,
    pub password: String,
    #[serde(serialize_with = "serialize_datetime")]
    pub started_at: DateTime<Utc>,
    pub status: LoanStatus,
    pub username: String,
    pub loanable: NewLoanable,
    pub meta: Vec<String>,
}

#[non_exhaustive]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Loan {
    #[serde(serialize_with = "serialize_datetime")]
    pub created_at: DateTime<Utc>,
    pub id: LoanId,
    #[serde(serialize_with = "serialize_datetime")]
    pub loan_end: DateTime<Utc>,
    #[serde(serialize_with = "serialize_datetime")]
    pub loan_start: DateTime<Utc>,
    pub loanable: Loanable,
    pub meta: Vec<String>,
    pub password: String,
    pub selected_firmware: FirmwareVersion,
    #[serde(serialize_with = "serialize_datetime")]
    pub started_at: DateTime<Utc>,
    pub status: LoanStatus,
    pub username: String,
}

impl Loan {
    /// Returns a host accessible from the internet with ports forwarded to the device.
    pub fn host(&self) -> Host {
        Host::Ipv4(Ipv4Addr::from([195, 60, 68, 14]))
    }

    /// Returns the port forwarded to port 80.
    pub fn http_port(&self) -> u16 {
        self.loanable.internal_ip.http_port()
    }

    /// Returns the port forwarded to port 443.
    pub fn https_port(&self) -> u16 {
        self.loanable.internal_ip.https_port()
    }

    /// Returns the port forwarded to port 22.
    pub fn ssh_port(&self) -> u16 {
        self.loanable.internal_ip.ssh_port()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct LoanId(u32);

impl Display for LoanId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[non_exhaustive]
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Loanable {
    pub device_map_id: u32,
    pub display_name: String,
    /// Despite it's name, the device is not accessible from the internet at this IP.
    /// Consider using [`Loan::host`] instead.
    pub external_ip: ExternalIp,
    pub id: LoanableId,
    pub image_url: PathBuf,
    pub internal_ip: InternalIp,
    pub model: String,
    portcast_device: Option<PortcastDevice>,
    pub r#type: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct LoanableId(u16);

impl LoanableId {
    pub fn as_u16(&self) -> u16 {
        self.0
    }
}

impl Display for LoanableId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn internal_ip(s: &str) -> InternalIp {
        InternalIp::try_from(s.to_string()).unwrap()
    }

    #[test]
    fn ports_are_computed_correctly_on_example() {
        let ip = internal_ip("10.20.41.10:11060");
        assert_eq!(ip.http_port(), 11060);
        assert_eq!(ip.ssh_port(), 21060);
        assert_eq!(ip.rtsp_port(), 31060);
        assert_eq!(ip.https_port(), 41060);
    }

    #[test]
    fn parsing_rejects_strings_without_a_valid_port() {
        let () = assert!(InternalIp::try_from("10.20.41.10".to_string()).is_err());
        let () = assert!(InternalIp::try_from("10.20.41.10:bogus".to_string()).is_err());
    }
}
