//! The [Basic device information] API.
//!
//! [Basic device information]: https://developer.axis.com/vapix/network-video/basic-device-information/

use std::{
    fmt::{Display, Formatter},
    str::FromStr,
};

use anyhow::{anyhow, bail, Context};
use semver::Version;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    http::HttpClient,
    protocol_helpers::{http::Error, json_rpc, json_rpc_http},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    InvalidParameter = 1000,
    AccessForbidden = 2001,
    UnsupportedHttpMethod = 2002,
    UnsupportedApiVersion = 2003,
    UnsupportedMethod = 2004,
    InvalidJson = 4000,
    MissingOrInvalidParameter = 4002,
    InternalError = 8000,
}

impl TryFrom<u16> for ErrorKind {
    type Error = u16;

    fn try_from(code: u16) -> Result<Self, u16> {
        match code {
            1000 => Ok(Self::InvalidParameter),
            2001 => Ok(Self::AccessForbidden),
            2002 => Ok(Self::UnsupportedHttpMethod),
            2003 => Ok(Self::UnsupportedApiVersion),
            2004 => Ok(Self::UnsupportedMethod),
            4000 => Ok(Self::InvalidJson),
            4002 => Ok(Self::MissingOrInvalidParameter),
            8000 => Ok(Self::InternalError),
            _ => Err(code),
        }
    }
}

fn serialize_none_as_empty_string<T, S>(value: &Option<T>, serializer: S) -> Result<S::Ok, S::Error>
where
    T: Display,
    S: Serializer,
{
    match value {
        Some(v) => serializer.serialize_str(&v.to_string()),
        None => serializer.serialize_str(""),
    }
}

fn deserialize_empty_string_as_none<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: FromStr,
    T::Err: Display,
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;

    if s.is_empty() {
        Ok(None)
    } else {
        T::from_str(&s).map(Some).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ProductType {
    AirQualitySensor,
    BispectralZoomPTZCamera,
    BoxCamera,
    DomeCamera,
    NetworkCamera,
    NetworkStrobeSpeaker,
    PanoramicCamera,
    PeopleCounter3D,
    Radar,
    ThermalCamera,
}

impl Display for ProductType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AirQualitySensor => write!(f, "Air Quality Sensor"),
            Self::BispectralZoomPTZCamera => write!(f, "Bispectral Zoom PTZ Camera"),
            Self::BoxCamera => write!(f, "Box Camera"),
            Self::DomeCamera => write!(f, "Dome Camera"),
            Self::NetworkCamera => write!(f, "Network Camera"),
            Self::NetworkStrobeSpeaker => write!(f, "Network Strobe Speaker"),
            Self::PanoramicCamera => write!(f, "Panoramic Camera"),
            Self::PeopleCounter3D => write!(f, "3D People Counter"),
            Self::Radar => write!(f, "Radar"),
            Self::ThermalCamera => write!(f, "Thermal Camera"),
        }
    }
}

impl FromStr for ProductType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Air Quality Sensor" => Ok(Self::AirQualitySensor),
            "Bispectral Zoom PTZ Camera" => Ok(Self::BispectralZoomPTZCamera),
            "Box Camera" => Ok(Self::BoxCamera),
            "Dome Camera" => Ok(Self::DomeCamera),
            "Network Camera" => Ok(Self::NetworkCamera),
            "Network Strobe Speaker" => Ok(Self::NetworkStrobeSpeaker),
            "Panoramic Camera" => Ok(Self::PanoramicCamera),
            "3D People Counter" => Ok(Self::PeopleCounter3D),
            "Radar" => Ok(Self::Radar),
            "Thermal Camera" => Ok(Self::ThermalCamera),
            _ => Err(anyhow!("unrecognized product type '{s}'")),
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllUnrestrictedPropertiesData {
    pub property_list: UnrestrictedProperties,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct UnrestrictedProperties {
    pub brand: String,
    pub build_date: String,
    #[serde(rename = "HardwareID")]
    pub hardware_id: String,
    pub prod_full_name: String,
    pub prod_nbr: String,
    pub prod_short_name: String,
    pub prod_type: String,
    /// Appears to not be set on 8.45.4.5, as well as more recent versions of AXIS OS.
    #[serde(
        serialize_with = "serialize_none_as_empty_string",
        deserialize_with = "deserialize_empty_string_as_none"
    )]
    pub prod_variant: Option<String>,
    pub serial_number: String,
    pub version: String,
    #[serde(rename = "WebURL")]
    pub web_url: String,
}

impl UnrestrictedProperties {
    pub fn parse_product_type(&self) -> anyhow::Result<ProductType> {
        self.prod_type.parse().context("invalid product type")
    }

    // AXIS OS versions less than 10 do not always follow semver.
    // TODO: Parse firmware versions <10
    pub fn parse_version(&self) -> anyhow::Result<Version> {
        Version::parse(self.version.as_str()).context("invalid version")
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAllUnrestrictedPropertiesRequest {
    api_version: &'static str,
    method: &'static str,
}

const PATH: &str = "axis-cgi/basicdeviceinfo.cgi";

impl GetAllUnrestrictedPropertiesRequest {
    pub fn new() -> Self {
        Self {
            api_version: "1.0",
            method: "getAllUnrestrictedProperties",
        }
    }

    pub async fn send(
        self,
        client: &(impl HttpClient + Sync),
    ) -> Result<AllUnrestrictedPropertiesData, Error<json_rpc::Error>> {
        json_rpc_http::send_request(client, PATH, &self).await
    }
}

impl Default for GetAllUnrestrictedPropertiesRequest {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Architecture {
    Aarch64,
    Armv7hf,
    Armv7l,
    Mips,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SocSerialNumber {
    Plain(u64),
    Dashed64(u64),
    Dashed128(u128),
}

impl Display for SocSerialNumber {
    #[expect(
        clippy::as_conversions,
        reason = "each cast deliberately truncates to a 32-bit hex window of a wider integer; \
        `TryFrom` would be misleading since truncation, not overflow, is the point"
    )]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Plain(v) => write!(f, "{v:016X}"),
            Self::Dashed64(v) => write!(f, "{:08X}-{:08X}", (*v >> 32) as u32, *v as u32),
            Self::Dashed128(v) => write!(
                f,
                "{:08X}-{:08X}-{:08X}-{:08X}",
                (*v >> 96) as u32,
                (*v >> 64) as u32,
                (*v >> 32) as u32,
                *v as u32
            ),
        }
    }
}

fn parse_dashed_soc_serial_128(s: &str) -> anyhow::Result<u128> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 4 {
        bail!("Expected 4 segments, got {}", parts.len());
    }
    let mut bits: u128 = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.len() != 8 {
            bail!(
                "Expected each segment to be 8 characters long, but segment {} is {}",
                i + 1,
                part.len()
            );
        }
        let segment = u32::from_str_radix(part, 16)?;
        bits |= u128::from(segment) << (96 - i * 32);
    }
    Ok(bits)
}

fn parse_dashed_soc_serial_64(s: &str) -> anyhow::Result<u64> {
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 2 {
        bail!("Expected 2 segments, got {}", parts.len());
    }
    let mut bits: u64 = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.len() != 8 {
            bail!(
                "Expected each segment to be 8 characters long, but segment {} is {}",
                i + 1,
                part.len()
            );
        }
        let segment = u32::from_str_radix(part, 16)?;
        bits |= u64::from(segment) << (32 - i * 32);
    }
    Ok(bits)
}

fn parse_plain_soc_serial(s: &str) -> anyhow::Result<u64> {
    if s.len() != 16 {
        bail!("Expected 16 characters long, got {}", s.len());
    }
    Ok(u64::from_str_radix(s, 16)?)
}

impl FromStr for SocSerialNumber {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.chars().filter(|c| *c == '-').count() {
            0 => Ok(Self::Plain(parse_plain_soc_serial(s)?)),
            1 => Ok(Self::Dashed64(parse_dashed_soc_serial_64(s)?)),
            3 => Ok(Self::Dashed128(parse_dashed_soc_serial_128(s)?)),
            n => Err(anyhow!("Expected 1, 2 or 4 segments, got {}", n + 1)),
        }
    }
}

impl Serialize for SocSerialNumber {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for SocSerialNumber {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        SocSerialNumber::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct RestrictedProperties {
    pub architecture: Architecture,
    /// Appears to not be set on 8.45.4.5
    #[serde(
        serialize_with = "serialize_none_as_empty_string",
        deserialize_with = "deserialize_empty_string_as_none"
    )]
    pub soc_serial_number: Option<String>,
    // TODO: Consider enumerating all known variants
    pub soc: String,
}

impl RestrictedProperties {
    pub fn parse_soc_serial_number(&self) -> anyhow::Result<Option<SocSerialNumber>> {
        self.soc_serial_number
            .as_deref()
            .map(SocSerialNumber::from_str)
            .transpose()
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct AllProperties {
    #[serde(flatten)]
    pub unrestricted: UnrestrictedProperties,
    #[serde(flatten)]
    pub restricted: RestrictedProperties,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AllPropertiesData {
    pub property_list: AllProperties,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GetAllPropertiesRequest {
    api_version: &'static str,
    method: &'static str,
}

impl GetAllPropertiesRequest {
    pub fn new() -> Self {
        Self {
            api_version: "1.0",
            method: "getAllProperties",
        }
    }

    pub async fn send(
        self,
        client: &(impl HttpClient + Sync),
    ) -> Result<AllPropertiesData, Error<json_rpc::Error>> {
        json_rpc_http::send_request(client, PATH, &self).await
    }
}

impl Default for GetAllPropertiesRequest {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use expect_test::expect;

    use super::*;

    #[test]
    fn soc_serial_number_dashed_from_to_string_roundtrip() {
        let expected = "00000000-00000000-032CDEEE-01349999";
        let actual = SocSerialNumber::from_str(expected).unwrap().to_string();
        assert_eq!(expected, actual);
    }

    #[test]
    fn soc_serial_number_plain_from_to_string_roundtrip() {
        let expected = "032CDEEE01349999";
        let actual = SocSerialNumber::from_str(expected).unwrap().to_string();
        assert_eq!(expected, actual);
    }

    #[test]
    fn invalid_soc_serial_number_strings_fail_to_parse() {
        expect!("Expected each segment to be 8 characters long, but segment 4 is 7").assert_eq(
            &SocSerialNumber::from_str("00000000-00000000-00000000-0000000")
                .unwrap_err()
                .to_string(),
        );
        expect!("Expected 1, 2 or 4 segments, got 3").assert_eq(
            &SocSerialNumber::from_str("00000000-00000000-0000000000000000")
                .unwrap_err()
                .to_string(),
        );
        expect!("invalid digit found in string").assert_eq(
            &SocSerialNumber::from_str("00000000-00000000-00000000-0000000G")
                .unwrap_err()
                .to_string(),
        );

        expect!("Expected 16 characters long, got 15").assert_eq(
            &SocSerialNumber::from_str("000000000000000")
                .unwrap_err()
                .to_string(),
        );
        expect!("invalid digit found in string").assert_eq(
            &SocSerialNumber::from_str("000000000000000G")
                .unwrap_err()
                .to_string(),
        );
        // Note that lower-case hex letters are accepted even though they never appear in responses from the API
    }
}
