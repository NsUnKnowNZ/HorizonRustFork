//! The native mods horizon-mixins compiled into a workspace, read from the record it leaves there.
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

/// What horizon-mixins writes at the root of a workspace it changed: each injection and module it
/// applied, with its mod's `[mod] id`. The launcher's native builds have one, and so has a bundle
/// applied by hand or by a mod's own script, which has no `native-mods.json` beside its game.
pub const MIXINS_STAGE: &str = ".horizon-mixins-stage.json";

/// The ids of the mods whose injections or modules horizon-mixins applied to the workspace at
/// `root`, sorted: none for a workspace it never changed, a record that names no mod, or one that
/// does not parse.
pub fn staged_mods(root: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(root.join(MIXINS_STAGE)) else {
        return Vec::new();
    };
    let Ok(record) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let ids: BTreeSet<&str> = ["injections", "modules"]
        .iter()
        .filter_map(|list| record[*list].as_array())
        .flatten()
        .filter_map(|entry| entry["mod_id"].as_str())
        .filter(|id| !id.is_empty())
        .collect();
    ids.into_iter().map(str::to_owned).collect()
}

/// The mods of [`staged_mods`] with the version each was applied at, by id: what a game built
/// from the workspace at `root` tells a server or a player about its native mods.
pub fn staged_native(root: &Path) -> Vec<(String, String)> {
    let Some(record) = read_record(root) else {
        return Vec::new();
    };
    let mods: BTreeMap<&str, &str> = ["injections", "modules"]
        .iter()
        .filter_map(|list| record[*list].as_array())
        .flatten()
        .filter_map(|entry| {
            let id = entry["mod_id"].as_str().filter(|id| !id.is_empty())?;
            Some((id, entry["mod_version"].as_str().unwrap_or("?")))
        })
        .collect();
    mods.into_iter()
        .map(|(id, version)| (id.to_owned(), version.to_owned()))
        .collect()
}

/// What the game built from the workspace at `root` is made of, as the SHA-256 (in hex) of the
/// record's source files under `crates/` and the two Cargo files, each with the hash
/// horizon-mixins took of it before changing anything, and of every injection, module and data
/// file it applied, code included. Two workspaces with the same fingerprint build the same game,
/// uncommitted changes and all, which a git revision cannot say. None for a workspace
/// horizon-mixins never changed.
pub fn staged_fingerprint(root: &Path) -> Option<String> {
    let record = read_record(root)?;
    let mut lines: Vec<String> = Vec::new();
    if let Some(inputs) = record["inputs"].as_object() {
        for (path, hash) in inputs {
            if path.starts_with("crates/") || path == "Cargo.lock" || path == "Cargo.toml" {
                lines.push(format!("input {path} {}", hash.as_str().unwrap_or_default()));
            }
        }
    }
    for list in ["injections", "modules", "data"] {
        for entry in record[list].as_array().into_iter().flatten() {
            lines.push(format!("{list} {entry}"));
        }
    }
    // By content, not by the order the record lists them in.
    lines.sort();
    let digest = sha256(lines.join("\n").as_bytes());
    Some(digest.iter().map(|b| format!("{b:02x}")).collect())
}

fn read_record(root: &Path) -> Option<serde_json::Value> {
    let text = std::fs::read_to_string(root.join(MIXINS_STAGE)).ok()?;
    serde_json::from_str(&text).ok()
}

/// SHA-256 (FIPS 180-4) of `data`. Written out here because this crate is a build script's
/// dependency and has no hashing crate among its own; the tests hold it to the standard's
/// vectors.
fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut message = data.to_vec();
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&(data.len() as u64).wrapping_mul(8).to_be_bytes());
    for block in message.chunks_exact(64) {
        let mut w = [0u32; 64];
        for (word, bytes) in w.iter_mut().zip(block.chunks_exact(4)) {
            *word = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choice = (e & f) ^ (!e & g);
            let t1 = h
                .wrapping_add(s1)
                .wrapping_add(choice)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (sum, word) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *sum = sum.wrapping_add(word);
        }
    }
    let mut out = [0u8; 32];
    for (bytes, word) in out.chunks_exact_mut(4).zip(state) {
        bytes.copy_from_slice(&word.to_be_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(name: &str, record: Option<&str>) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "horizon-mixins-stage-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        if let Some(record) = record {
            std::fs::write(dir.join(MIXINS_STAGE), record).unwrap();
        }
        dir
    }

    #[test]
    fn the_mods_of_every_injection_and_module_once() {
        // The shape horizon-mixins 0.6.0 writes (schema 6), trimmed to the fields read here.
        let dir = workspace(
            "mods",
            Some(
                r#"{"schema": 6, "transformer": "0.6.0", "inputs": {"Cargo.toml": "ab"},
                "injections": [
                    {"mod_id": "ironfall", "injection_id": "hud", "kind": "HEAD"},
                    {"mod_id": "veinminer", "injection_id": "break", "kind": "HEAD"},
                    {"mod_id": "ironfall", "injection_id": "tick", "kind": "HEAD"}
                ],
                "modules": [
                    {"mod_id": "ironfall", "module_id": "glue"},
                    {"mod_id": "atlas", "module_id": "maps"}
                ]}"#,
            ),
        );
        assert_eq!(staged_mods(&dir), ["atlas", "ironfall", "veinminer"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sha256_matches_the_standard() {
        let hex = |data: &[u8]| -> String {
            sha256(data).iter().map(|b| format!("{b:02x}")).collect()
        };
        assert_eq!(
            hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // More than one block, and a length that pads into another.
        assert_eq!(
            hex(&[b'a'; 200]),
            "c2a908d98f5df987ade41b5fce213067efbcc21ef2240212a41e54b5e7c28ae5"
        );
    }

    #[test]
    fn native_mods_with_versions_and_a_fingerprint_of_what_was_built() {
        let record = |lock: &str, helper: &str, readme: &str| {
            format!(
                r#"{{"schema": 6, "inputs": {{"Cargo.lock": "{lock}", "README.md": "{readme}",
                    "crates/a/src/lib.rs": "11"}},
                "injections": [
                    {{"mod_id": "veinminer", "mod_version": "1.2.0", "helper_source": "{helper}"}},
                    {{"mod_id": "ironfall", "mod_version": "0.3.1", "helper_source": "fn a() {{}}"}}
                ],
                "modules": [{{"mod_id": "ironfall", "mod_version": "0.3.1", "source": "x"}},
                    {{"mod_id": "old", "source": "y"}}]}}"#
            )
        };
        let dir = workspace("native", Some(&record("ab", "fn b() {}", "r1")));
        assert_eq!(
            staged_native(&dir),
            [
                ("ironfall".to_owned(), "0.3.1".to_owned()),
                ("old".to_owned(), "?".to_owned()),
                ("veinminer".to_owned(), "1.2.0".to_owned())
            ]
        );
        let first = staged_fingerprint(&dir).unwrap();
        assert_eq!(first.len(), 64);
        assert_eq!(staged_fingerprint(&dir).unwrap(), first);
        // A file that is no source of the game changes nothing; a mod's code or a locked
        // dependency does.
        std::fs::write(dir.join(MIXINS_STAGE), record("ab", "fn b() {}", "r2")).unwrap();
        assert_eq!(staged_fingerprint(&dir).unwrap(), first);
        std::fs::write(dir.join(MIXINS_STAGE), record("ab", "fn c() {}", "r1")).unwrap();
        assert_ne!(staged_fingerprint(&dir).unwrap(), first);
        std::fs::write(dir.join(MIXINS_STAGE), record("cd", "fn b() {}", "r1")).unwrap();
        assert_ne!(staged_fingerprint(&dir).unwrap(), first);
        std::fs::remove_dir_all(&dir).unwrap();
        let none = workspace("native-none", None);
        assert!(staged_native(&none).is_empty());
        assert_eq!(staged_fingerprint(&none), None);
        std::fs::remove_dir_all(&none).unwrap();
    }

    #[test]
    fn no_mods_without_a_record_or_with_an_empty_or_broken_one() {
        for (name, record) in [
            ("none", None),
            (
                "empty",
                Some(r#"{"schema": 6, "injections": [], "modules": []}"#),
            ),
            ("broken", Some("{\"injections\": [")),
        ] {
            let dir = workspace(name, record);
            assert!(staged_mods(&dir).is_empty(), "{name}");
            std::fs::remove_dir_all(&dir).unwrap();
        }
    }
}
