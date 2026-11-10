//! Offline player profiles: the names a player without an account switches between on the
//! title screen, each with an arm model and, optionally, a skin.
//!
//! - `horizon-profiles.txt` beside `options.txt` lists them: `profile:<name>:<classic|slim>`.
//! - The one in use is the name in `horizon-player.txt` (`offline_name`), with its model on a
//!   `model:` line there.
//! - `profiles/<name>.png` is a profile's skin, shown to its own player
//!   (`skins::offline_skin`), read when the game starts.
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, Instant},
};

pub const FILE: &str = "horizon-profiles.txt";
pub const DIR: &str = "profiles";
/// As many as the profiles screen has rows for.
pub const MAX: usize = 8;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub name: String,
    pub slim: bool,
}

/// A name a profile can have: a valid player name of letters, digits and underscores (it is
/// also its skin's file name).
pub fn valid(name: &str) -> bool {
    crate::offline_name::is_valid(name)
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn model(slim: bool) -> &'static str {
    if slim { "slim" } else { "classic" }
}

/// The profiles kept in `dir`, the one in use among them (added when the list lacks it).
pub fn load(dir: &Path) -> Vec<Profile> {
    let mut list: Vec<Profile> = Vec::new();
    for line in horizon_core::rfs::read_to_string(dir.join(FILE)).unwrap_or_default().lines() {
        let Some(rest) = line.trim().strip_prefix("profile:") else {
            continue;
        };
        let (name, kind) = rest.split_once(':').unwrap_or((rest, "classic"));
        if valid(name) && !list.iter().any(|p| p.name == name) {
            list.push(Profile {
                name: name.to_owned(),
                slim: kind == "slim",
            });
        }
    }
    let active = crate::offline_name::load_or_create(dir);
    if !list.iter().any(|p| p.name == active) {
        list.insert(
            0,
            Profile {
                name: active,
                slim: false,
            },
        );
    }
    list.truncate(MAX);
    list
}

pub fn save(dir: &Path, list: &[Profile]) -> io::Result<()> {
    let text: String = list
        .iter()
        .map(|p| format!("profile:{}:{}\n", p.name, model(p.slim)))
        .collect();
    fs::write(dir.join(FILE), text)
}

/// Makes `profile` the one in use: the next world or server is joined under its name.
pub fn select(dir: &Path, profile: &Profile) -> io::Result<()> {
    fs::write(
        dir.join(crate::offline_name::FILE),
        format!("name:{}\nmodel:{}\n", profile.name, model(profile.slim)),
    )?;
    if let Ok(mut cached) = CURRENT.lock() {
        *cached = None;
    }
    Ok(())
}

/// Where a profile's skin is looked for.
pub fn skin_file(dir: &Path, name: &str) -> PathBuf {
    dir.join(DIR).join(format!("{name}.png"))
}

/// The folder skins go in.
pub fn skins_dir(dir: &Path) -> PathBuf {
    dir.join(DIR)
}

static CURRENT: Mutex<Option<(Instant, String)>> = Mutex::new(None);

/// The name in use, for a label drawn every frame: read again once a second.
pub fn current_name() -> String {
    let mut cached = CURRENT.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((at, name)) = cached.as_ref()
        && at.elapsed() < Duration::from_secs(1)
    {
        return name.clone();
    }
    let name = crate::offline_name::load_or_create(&crate::logging::player_dir());
    *cached = Some((Instant::now(), name.clone()));
    name
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("horizon-profiles-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_profile_in_use_is_listed_and_a_selected_one_is_the_name() {
        let dir = dir("select");
        let first = load(&dir);
        assert_eq!(first.len(), 1);
        let mut list = first.clone();
        list.push(Profile {
            name: "Andre".into(),
            slim: true,
        });
        save(&dir, &list).unwrap();
        select(&dir, &list[1]).unwrap();
        assert_eq!(crate::offline_name::load_or_create(&dir), "Andre");
        assert_eq!(load(&dir), list);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn bad_names_and_repeats_are_left_out() {
        let dir = dir("bad");
        fs::write(dir.join(crate::offline_name::FILE), "name:Host\n").unwrap();
        fs::write(
            dir.join(FILE),
            "profile:Host:classic\nprofile:has space:slim\nprofile:Host:slim\nprofile:Guest\n",
        )
        .unwrap();
        let names: Vec<_> = load(&dir).into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["Host", "Guest"]);
        let _ = fs::remove_dir_all(&dir);
    }
}
