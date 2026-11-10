//! Which build of the game each side of a connection runs: the release, the
//! source revision, a fingerprint of the source with its native mods applied,
//! and the native mods themselves (docs/mods.md, "Playing on a server with
//! mods").
//!
//! Native mods are compiled into the game, so two players only have the same
//! ones when they run the same build. A Horizon server sends its build on
//! [`CHANNEL`] in the configuration phase and the client shows it beside its
//! own when a join is refused over mods. It is told, not proven: the content
//! registry check (`horizon:block_registry`) is what decides a join.
use crate::{
    Error, Result,
    buf::{Reader, Writer},
};
use std::sync::OnceLock;

/// The custom payload channel, clientbound.
pub const CHANNEL: &str = "horizon:build";
/// Native mods in a build at most; a build with more is refused whole.
pub const MAX_NATIVE: usize = 128;
/// Characters in a version, a nightly's name or a native mod's id or version at most.
pub const MAX_TEXT: usize = 64;
/// Characters in a revision at most (a full git hash).
pub const MAX_REVISION: usize = 40;

/// A build of the game.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Build {
    /// The release (`horizon_core::release::VERSION`).
    pub version: String,
    /// A nightly's date and commit; none for a release or a build from source.
    pub nightly: Option<String>,
    /// The source's git revision, or `unknown`.
    pub revision: String,
    /// The source had changes the revision does not name when it was built.
    pub dirty: bool,
    /// SHA-256 of the source files and the native mods' code, as horizon-mixins
    /// recorded them; zero for a build without native mods.
    pub fingerprint: [u8; 32],
    /// The native mods built in: id and version, by id.
    pub native: Vec<(String, String)>,
}

/// This game's build, set once at start (`game/mods.rs`).
static LOCAL: OnceLock<Build> = OnceLock::new();

/// Sets this game's build; false when it was set already.
pub fn set_local(build: Build) -> bool {
    LOCAL.set(build.capped()).is_ok()
}

/// This game's build; none before `set_local` (a test, a tool).
pub fn local() -> Option<Build> {
    LOCAL.get().cloned()
}

impl Build {
    /// The build within the caps a payload has, its native mods by id.
    fn capped(mut self) -> Self {
        let cut = |s: &str, max: usize| -> String { s.chars().take(max).collect() };
        self.version = cut(&self.version, MAX_TEXT);
        self.nightly = self.nightly.map(|n| cut(&n, MAX_TEXT));
        self.revision = cut(&self.revision, MAX_REVISION);
        self.native = self
            .native
            .iter()
            .map(|(id, version)| (cut(id, MAX_TEXT), cut(version, MAX_TEXT)))
            .collect();
        self.native.sort();
        self.native.dedup_by(|a, b| a.0 == b.0);
        self.native.truncate(MAX_NATIVE);
        self
    }

    /// The build as a player reads it: `0.1.0 (rev e42afee, 53 native mods,
    /// build 1a2b3c4d)`. A `+` after the revision marks source changes it does
    /// not name; `build` is the start of the fingerprint.
    pub fn label(&self) -> String {
        let mut parts = Vec::new();
        let short: String = self.revision.chars().take(7).collect();
        let dirty = if self.dirty { "+" } else { "" };
        parts.push(format!("rev {short}{dirty}"));
        parts.push(match self.native.len() {
            0 => "no native mods".to_owned(),
            1 => "1 native mod".to_owned(),
            n => format!("{n} native mods"),
        });
        if self.fingerprint != [0; 32] {
            let hex: String = self.fingerprint[..4]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            parts.push(format!("build {hex}"));
        }
        let nightly = match &self.nightly {
            Some(nightly) => format!(" nightly {nightly}"),
            None => String::new(),
        };
        format!("{}{nightly} ({})", self.version, parts.join(", "))
    }

    /// Whether both are known to be the same game: every part equal, and no
    /// unnamed source changes without a fingerprint to tell them apart.
    pub fn same_as(&self, other: &Build) -> bool {
        self == other && !(self.dirty && self.fingerprint == [0; 32])
    }

    /// How `yours` differs from this (the server's) in native mods, one line
    /// each: `jei 1.2 (server only)`, `xray 1.0 (yours only)`,
    /// `jade (server 1.1, yours 1.0)`.
    pub fn native_difference(&self, yours: &Build) -> Vec<String> {
        let mut lines = Vec::new();
        for (id, version) in &self.native {
            match yours.native.iter().find(|(other, _)| other == id) {
                None => lines.push(format!("{id} {version} (server only)")),
                Some((_, mine)) if mine != version => {
                    lines.push(format!("{id} (server {version}, yours {mine})"));
                }
                Some(_) => {}
            }
        }
        for (id, version) in &yours.native {
            if !self.native.iter().any(|(other, _)| other == id) {
                lines.push(format!("{id} {version} (yours only)"));
            }
        }
        lines
    }

    /// The build as a payload.
    pub fn encode(&self) -> Vec<u8> {
        let build = self.clone().capped();
        let mut w = Writer::new();
        w.string(&build.version);
        match &build.nightly {
            Some(nightly) => {
                w.bool(true);
                w.string(nightly);
            }
            None => w.bool(false),
        }
        w.string(&build.revision);
        w.bool(build.dirty);
        w.bytes(&build.fingerprint);
        w.varint(build.native.len() as i32);
        for (id, version) in &build.native {
            w.string(id);
            w.string(version);
        }
        w.data
    }

    /// A payload's build; an error for one that breaks a cap.
    pub fn decode(data: &[u8]) -> Result<Build> {
        let mut r = Reader::new(data);
        let version = r.string(MAX_TEXT)?;
        let nightly = match r.bool()? {
            true => Some(r.string(MAX_TEXT)?),
            false => None,
        };
        let revision = r.string(MAX_REVISION)?;
        let dirty = r.bool()?;
        let fingerprint = r.bytes(32)?.try_into().expect("32 bytes");
        let count = r.varint()?;
        if !(0..=MAX_NATIVE as i32).contains(&count) {
            return Err(Error::new(format!("build: {count} native mods")));
        }
        let native = (0..count)
            .map(|_| Ok((r.string(MAX_TEXT)?, r.string(MAX_TEXT)?)))
            .collect::<Result<Vec<_>>>()?;
        if r.remaining() > 0 {
            return Err(Error::new("build: trailing bytes"));
        }
        Ok(Build {
            version,
            nightly,
            revision,
            dirty,
            fingerprint,
            native,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build() -> Build {
        Build {
            version: "0.1.0".into(),
            nightly: None,
            revision: "e42afee0c02067aa11bb22cc33dd44ee55ff6677".into(),
            dirty: false,
            fingerprint: {
                let mut f = [0u8; 32];
                f[..4].copy_from_slice(&[0x1a, 0x2b, 0x3c, 0x4d]);
                f
            },
            native: vec![("jade".into(), "1.1".into()), ("jei".into(), "0.3".into())],
        }
    }

    #[test]
    fn a_build_round_trips() {
        let b = build();
        assert_eq!(Build::decode(&b.encode()).unwrap(), b);
        let nightly = Build {
            nightly: Some("2026-10-28 0c02067".into()),
            dirty: true,
            native: Vec::new(),
            ..build()
        };
        assert_eq!(Build::decode(&nightly.encode()).unwrap(), nightly);
        assert_eq!(
            Build::decode(&Build::default().encode()).unwrap(),
            Build::default()
        );
    }

    #[test]
    fn a_build_reads_as_a_label() {
        assert_eq!(
            build().label(),
            "0.1.0 (rev e42afee, 2 native mods, build 1a2b3c4d)"
        );
        let plain = Build {
            version: "0.2.3".into(),
            revision: "unknown".into(),
            dirty: true,
            ..Build::default()
        };
        assert_eq!(plain.label(), "0.2.3 (rev unknown+, no native mods)");
        let nightly = Build {
            nightly: Some("2026-10-28 0c02067".into()),
            native: vec![("jei".into(), "0.3".into())],
            fingerprint: [0; 32],
            ..build()
        };
        assert_eq!(
            nightly.label(),
            "0.1.0 nightly 2026-10-28 0c02067 (rev e42afee, 1 native mod)"
        );
    }

    #[test]
    fn builds_over_the_caps_are_refused_and_cut_when_sent() {
        let mut w = Writer::new();
        w.string(&"x".repeat(MAX_TEXT + 1));
        assert!(Build::decode(&w.data).is_err());
        let mut long = build().encode();
        long.push(0);
        assert!(Build::decode(&long).is_err());
        assert!(Build::decode(&[]).is_err());
        // A count over the cap, after an otherwise valid head.
        let mut w = Writer::new();
        w.string("1");
        w.bool(false);
        w.string("r");
        w.bool(false);
        w.bytes(&[0; 32]);
        w.varint(MAX_NATIVE as i32 + 1);
        assert!(Build::decode(&w.data).is_err());
        // What is sent always decodes: too many mods and long texts are cut.
        let big = Build {
            version: "v".repeat(200),
            revision: "r".repeat(200),
            native: (0..300)
                .map(|i| (format!("mod{i:03}"), "1".repeat(100)))
                .collect(),
            ..Build::default()
        };
        let read = Build::decode(&big.encode()).unwrap();
        assert_eq!(read.version.len(), MAX_TEXT);
        assert_eq!(read.revision.len(), MAX_REVISION);
        assert_eq!(read.native.len(), MAX_NATIVE);
        assert_eq!(read.native[0].1.len(), MAX_TEXT);
    }

    #[test]
    fn the_same_build_and_its_differences() {
        let server = build();
        assert!(server.same_as(&build()));
        assert!(server.native_difference(&build()).is_empty());
        let yours = Build {
            native: vec![("jade".into(), "1.0".into()), ("xray".into(), "2".into())],
            ..build()
        };
        assert!(!server.same_as(&yours));
        assert_eq!(
            server.native_difference(&yours),
            [
                "jade (server 1.1, yours 1.0)",
                "jei 0.3 (server only)",
                "xray 2 (yours only)"
            ]
        );
        // Changed source with no fingerprint cannot be told apart: not known to be the same.
        let dirty = Build {
            dirty: true,
            fingerprint: [0; 32],
            ..build()
        };
        assert!(!dirty.same_as(&dirty.clone()));
        let other_source = Build {
            fingerprint: [9; 32],
            ..build()
        };
        assert!(!server.same_as(&other_source));
    }
}
