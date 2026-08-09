use std::{
    fmt::{self, Display, Formatter},
    str::FromStr,
};

use anyhow::{anyhow, bail};
use semver::{Comparator, Op, Prerelease, VersionReq};

use crate::version::Version;

/// An AXIS OS long-term support track.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LtsTrack {
    pub year: u16,
    pub major: u64,
    pub minor: u64,
}

// TODO: Consider inferring these from external source to avoid toil on author and user side

/// The major version of the active track.
const ACTIVE_MAJOR: u64 = 12;

/// Known LTS tracks, newest first.
const LTS_TRACKS: &[LtsTrack] = &[
    LtsTrack {
        year: 2026,
        major: 12,
        minor: 11,
    },
    LtsTrack {
        year: 2024,
        major: 11,
        minor: 11,
    },
    LtsTrack {
        year: 2022,
        major: 10,
        minor: 12,
    },
    LtsTrack {
        year: 2020,
        major: 9,
        minor: 80,
    },
];

fn req_line(major: u64, minor: Option<u64>) -> VersionReq {
    VersionReq {
        comparators: vec![Comparator {
            op: Op::Exact,
            major,
            minor,
            patch: None,
            pre: Prerelease::EMPTY,
        }],
    }
}

/// A release track, as an alternative to naming a version requirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Track {
    /// The track Axis is currently releasing new features on.
    Active,
    /// The newest LTS track that has firmware for the product.
    LatestLts,
    /// One specific LTS track.
    Lts(&'static LtsTrack),
}

impl Track {
    /// The version requirement matching this track.
    fn resolve(self, versions: &[&Version]) -> Option<VersionReq> {
        match self {
            Self::Active => {
                let req = req_line(ACTIVE_MAJOR, None);
                versions.iter().any(|v| v.matches(&req)).then_some(req)
            }
            Self::Lts(t) => {
                let req = req_line(t.major, Some(t.minor));
                versions.iter().any(|v| v.matches(&req)).then_some(req)
            }
            Self::LatestLts => {
                debug_assert!(
                    LTS_TRACKS.windows(2).all(|w| match w {
                        [a, b] => a.year > b.year,
                        _ => true,
                    }),
                    "LTS_TRACKS must be sorted newest-first"
                );
                LTS_TRACKS
                    .iter()
                    .map(|t| req_line(t.major, Some(t.minor)))
                    .find(|req| versions.iter().any(|v| v.matches(req)))
            }
        }
    }
}

impl Display for Track {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::LatestLts => write!(f, "lts"),
            Self::Lts(t) => write!(f, "lts{}", t.year),
        }
    }
}

impl FromStr for Track {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim().to_ascii_lowercase();
        if s == "active" {
            return Ok(Self::Active);
        }
        let Some(rest) = s.strip_prefix("lts") else {
            bail!("expected \"active\", \"lts\", or an LTS year such as \"lts2024\", got {s:?}");
        };
        let rest = rest.trim_matches(|c: char| c == '-' || c == '_' || c.is_whitespace());
        if rest.is_empty() {
            return Ok(Self::LatestLts);
        }
        LTS_TRACKS
            .iter()
            .find(|t| t.year.to_string() == rest)
            .map(Self::Lts)
            .ok_or_else(|| {
                let known: Vec<_> = LTS_TRACKS.iter().map(|t| t.year.to_string()).collect();
                anyhow!(
                    "unknown LTS year {rest:?}, known years are {}",
                    known.join(", ")
                )
            })
    }
}

/// Which tracks have firmware for a product, for use in error messages.
pub(crate) fn available_tracks(versions: &[&Version]) -> Vec<String> {
    let mut out = Vec::new();
    if Track::Active.resolve(versions).is_some() {
        out.push(format!("active ({ACTIVE_MAJOR}.x)"));
    }
    for t in LTS_TRACKS {
        if Track::Lts(t).resolve(versions).is_some() {
            out.push(format!("lts{} ({}.{}.x)", t.year, t.major, t.minor));
        }
    }
    out
}

/// How to pick a version, by requirement or by track.
#[derive(Clone, Debug, clap::Args)]
pub struct Selector {
    /// Semver version requirement (e.g. "12", "^12.8", "<13")
    #[clap(long)]
    pub version: Option<VersionReq>,
    /// Release track: "active", "lts", or an LTS year (e.g. "lts2024")
    #[clap(long)]
    pub track: Option<Track>,
}

impl Selector {
    /// The matching version requirement for one product.
    ///
    /// Selecting nothing at all matches every version, which is what `list` without filters wants.
    pub(crate) fn resolve(&self, versions: &[&Version]) -> Option<VersionReq> {
        match (&self.version, self.track) {
            (Some(req), _) => Some(req.clone()),
            (None, Some(track)) => track.resolve(versions),
            (None, None) => Some(VersionReq::STAR),
        }
    }

    /// How the selection was expressed, for use in error messages.
    pub(crate) fn describe(&self) -> String {
        match (&self.version, self.track) {
            (Some(req), _) => format!("version requirement {req}"),
            (None, Some(track)) => format!("track {track}"),
            (None, None) => "no filter".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn versions(raw: &[&str]) -> Vec<Version> {
        raw.iter()
            .map(|v| Version::try_coerced(v).unwrap())
            .collect()
    }

    fn best(track: Track, raw: &[&str]) -> Option<Version> {
        let versions = versions(raw);
        let req = track.resolve(&versions.iter().collect::<Vec<_>>())?;
        versions.into_iter().filter(|v| v.matches(&req)).max()
    }

    #[test]
    fn track_parses_every_accepted_spelling() {
        assert_eq!("active".parse::<Track>().unwrap(), Track::Active);
        assert_eq!("ACTIVE".parse::<Track>().unwrap(), Track::Active);
        assert_eq!("lts".parse::<Track>().unwrap(), Track::LatestLts);
        for s in ["lts2024", "LTS 2024", "lts-2024", "lts_2024"] {
            let Ok(Track::Lts(t)) = s.parse::<Track>() else {
                panic!("{s} did not parse as a specific LTS track");
            };
            assert_eq!((t.major, t.minor), (11, 11));
        }
    }

    #[test]
    fn track_rejects_years_it_has_no_mapping_for() {
        assert!("lts2019".parse::<Track>().is_err());
        assert!("stable".parse::<Track>().is_err());
    }

    #[test]
    fn active_picks_the_newest_release_on_the_active_major() {
        assert_eq!(
            best(Track::Active, &["11.11.152", "12.5.35", "12.11.68"]),
            Some(Version::new(12, 11, 68))
        );
    }

    #[test]
    fn active_ignores_a_major_the_table_does_not_yet_call_active() {
        // Until ACTIVE_MAJOR is bumped, firmware from a newer major is not the active track.
        assert_eq!(
            best(Track::Active, &["12.11.68", "13.0.1"]),
            Some(Version::new(12, 11, 68))
        );
    }

    #[test]
    fn active_fails_rather_than_falling_back_for_a_product_that_never_got_it() {
        assert_eq!(best(Track::Active, &["10.12.239", "11.11.152"]), None);
    }

    #[test]
    fn lts_is_a_minor_line_not_a_major() {
        // 11.9 is on the same major as the 2024 LTS track but is not on the track itself.
        assert_eq!(
            best(Track::LatestLts, &["11.9.1", "11.11.152", "12.5.35"]),
            Some(Version::new(11, 11, 152))
        );
    }

    #[test]
    fn latest_lts_prefers_the_newest_track_the_product_has() {
        assert_eq!(
            best(Track::LatestLts, &["11.11.152", "12.11.68"]),
            Some(Version::new(12, 11, 68))
        );
        assert_eq!(
            best(Track::LatestLts, &["10.12.239", "11.11.152"]),
            Some(Version::new(11, 11, 152))
        );
    }

    #[test]
    fn latest_lts_falls_back_for_a_product_too_old_for_the_newest_track() {
        assert_eq!(
            best(Track::LatestLts, &["9.80.3", "10.12.239"]),
            Some(Version::new(10, 12, 239))
        );
    }

    #[test]
    fn available_tracks_names_only_tracks_with_firmware() {
        let versions = versions(&["10.12.239", "11.9.1", "11.11.152"]);
        assert_eq!(
            available_tracks(&versions.iter().collect::<Vec<_>>()),
            ["lts2024 (11.11.x)", "lts2022 (10.12.x)"]
        );
    }
}
