//! Java Edition's resource packs and shader packs, without copying them by hand: at start the
//! packs in Java's game folders (`.minecraft` and any listed in `horizon-java-folders.txt`, one
//! game or instance folder per line) that Horizon's own `resourcepacks/` and `shaderpacks/`
//! lack are copied in, and a pack Java has a newer copy of is copied again. Packs are never
//! removed or changed in the Java folders. A line `off` in the file turns it off.
use horizon_core::rfs::RPath as _;
use std::path::{Path, PathBuf};

const LIST: &str = "horizon-java-folders.txt";
const KINDS: [&str; 2] = ["resourcepacks", "shaderpacks"];

/// Java's default game folder on this system.
fn default_java_dir() -> Option<PathBuf> {
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|a| PathBuf::from(a).join(".minecraft"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support/minecraft"))
    } else {
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".minecraft"))
    }
}

/// The Java folders to import from, and whether importing is on.
fn sources(game: &Path) -> Vec<PathBuf> {
    let list = game.join(LIST);
    let text = horizon_core::rfs::read_to_string(&list).unwrap_or_else(|_| {
        let text = "# Java Edition game or instance folders whose resourcepacks/ and shaderpacks/\n\
                    # Horizon copies in at start (Java's own .minecraft is always looked at).\n\
                    # A line `off` turns importing off.\n";
        let _ = std::fs::write(&list, text);
        text.to_owned()
    });
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    if lines.iter().any(|l| l.eq_ignore_ascii_case("off")) {
        return Vec::new();
    }
    let mut out: Vec<PathBuf> = default_java_dir().into_iter().collect();
    out.extend(lines.iter().map(PathBuf::from));
    let here = game.canonicalize().ok();
    out.retain(|p| p.r_is_dir() && p.canonicalize().ok() != here);
    out.dedup();
    out
}

fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in horizon_core::rfs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.r_is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

fn modified(p: &Path) -> Option<std::time::SystemTime> {
    horizon_core::rfs::metadata(p).and_then(|m| m.modified()).ok()
}

/// Copies in the Java packs the game folder `game` lacks (or has older copies of).
pub fn import(game: &Path) {
    for source in sources(game) {
        for kind in KINDS {
            let Ok(entries) = horizon_core::rfs::read_dir(source.join(kind)) else {
                continue;
            };
            let dest = game.join(kind);
            for entry in entries.flatten() {
                let from = entry.path();
                let name = entry.file_name();
                let lower = name.to_string_lossy().to_lowercase();
                let is_dir = from.r_is_dir();
                // Packs (folders and zips), and a shader pack's chosen options (`<pack>.txt`).
                let wanted = is_dir
                    || lower.ends_with(".zip")
                    || (kind == "shaderpacks" && lower.ends_with(".txt"));
                if !wanted {
                    continue;
                }
                let to = dest.join(&name);
                let newer = match (modified(&from), modified(&to)) {
                    (_, None) => true,
                    (Some(a), Some(b)) => !is_dir && a > b,
                    _ => false,
                };
                if !newer {
                    continue;
                }
                let _ = std::fs::create_dir_all(&dest);
                let done = if is_dir {
                    copy_dir(&from, &to)
                } else {
                    std::fs::copy(&from, &to).map(|_| ())
                };
                match done {
                    Ok(()) => log::info!(
                        "Horizon: imported {} from {}",
                        to.display(),
                        from.display()
                    ),
                    Err(e) => log::warn!("Horizon: could not import {}: {e}", from.display()),
                }
            }
        }
    }
}
