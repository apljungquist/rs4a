use std::fmt::{self, Display, Formatter};

use serde::{Deserialize, Serialize};

/// Parse a version the way it appears in archive directory names, e.g. `12_11_68`.
pub(crate) fn version_from_underscore(s: &str) -> Option<Version> {
    Version::try_coerced(&s.replace('_', ".")).ok()
}

/// The version of a firmware release.
///
/// Firmware revisions aren't strict semver:
/// - some have more than three components (a build number tail, dropped here), and
/// - some have fewer (rejected, since major/minor/patch can't all be inferred).
///
/// Because the tail is dropped, two revisions differing only there — `10.0.2.1` and `10.0.2.2` —
/// are one and the same version here.
// TODO: Improve the support for non-semver versions with our own Version type and req matching
#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Version(semver::Version);

impl Version {
    /// Create a [`Version`]
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self(semver::Version::new(major, minor, patch))
    }

    /// Coerce a dotted revision into a version by keeping only its first three components.
    pub fn try_coerced(revision: &str) -> anyhow::Result<Self> {
        let mut parts = revision.splitn(4, '.');
        let major = parts.next().unwrap_or_default().parse()?;
        let minor = parts.next().unwrap_or_default().parse()?;
        let patch = parts.next().unwrap_or_default().parse()?;
        Ok(Self(semver::Version::new(major, minor, patch)))
    }

    /// Whether this version satisfies a requirement.
    pub fn matches(&self, req: &semver::VersionReq) -> bool {
        req.matches(&self.0)
    }

    /// The version in the form the `semver` crate compares, for callers outside this crate.
    pub fn semver(&self) -> &semver::Version {
        &self.0
    }
}

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Parse each version string, pairing it with its parsed form.
///
/// NB: drops unparseable version strings.
pub(crate) fn parse_versions(versions: &[String]) -> Vec<(&str, Version)> {
    versions
        .iter()
        .filter_map(|v| Some((v.as_str(), version_from_underscore(v)?)))
        .collect()
}
