//! "Horizon OneBlock.exe": starts `bin\horizon-client.exe` with the folder this program is in
//! as the game folder, passing its arguments on. The game is a console program; started this
//! way it gets no console window. Its log is in `%APPDATA%\horizon\logs\latest.log`.
//!
//! A copy of the folder passed to another player comes without `bin\resources`: those are
//! made from each player's own Minecraft and are not ours to pass on. Then the game is given
//! the resources that player prepared themselves: the folder named in `horizon-resources.txt`,
//! a Horizon source folder near this one, or the Horizon launcher's. With none, a note is
//! opened that says what to do, and the game is not started.
#![windows_subsystem = "windows"]
use std::path::{Path, PathBuf};
use std::process::Command;

/// The Minecraft version the resources are prepared from (`horizon_core::VANILLA_VERSION`).
const VERSION: &str = "26.3";
const NOTE: &str = "Horizon OneBlock - resources needed.txt";

fn prepared(dir: &Path) -> bool {
    dir.join("horizon-resources.json").is_file()
}

/// A Horizon source folder's prepared resources, if it has them.
fn of_source(source: &Path) -> Option<PathBuf> {
    let dir = source.join(".local").join("horizon-client").join(VERSION);
    prepared(&dir).then_some(dir)
}

/// The folders in `dir`, itself first.
fn with_children(dir: &Path) -> Vec<PathBuf> {
    let mut all = vec![dir.to_owned()];
    if let Ok(entries) = std::fs::read_dir(dir) {
        all.extend(entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()));
    }
    all
}

/// Prepared resources under `dir`, at most `depth` folders down (the Horizon launcher keeps
/// one set a version).
fn under(dir: &Path, depth: u32) -> Option<PathBuf> {
    if let Some(found) = of_source(dir) {
        return Some(found);
    }
    if depth == 0 {
        return None;
    }
    let entries = std::fs::read_dir(dir).ok()?;
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .find_map(|p| under(&p, depth - 1))
}

/// The resources another player prepared on this computer, for a game folder without its own.
fn elsewhere(game: &Path) -> Option<PathBuf> {
    // The folder they named: the first line that is not a comment.
    if let Ok(text) = std::fs::read_to_string(game.join("horizon-resources.txt")) {
        let named = text.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with('#'));
        if let Some(line) = named {
            let dir = PathBuf::from(line.trim_matches('"'));
            if prepared(&dir) {
                return Some(dir);
            }
            if let Some(found) = of_source(&dir) {
                return Some(found);
            }
        }
    }
    // A source folder beside this one, in the home folder, on the desktop, in the documents,
    // or at a drive's root.
    let mut places: Vec<PathBuf> = game.parent().map(Path::to_owned).into_iter().collect();
    if let Some(home) = std::env::var_os("USERPROFILE").map(PathBuf::from) {
        places.extend([home.clone(), home.join("Desktop"), home.join("Documents"), home.join("source").join("repos")]);
    }
    places.extend(('C'..='Z').map(|drive| PathBuf::from(format!("{drive}:\\"))));
    for place in places {
        if let Some(found) = with_children(&place).iter().find_map(|p| of_source(p)) {
            return Some(found);
        }
    }
    // The Horizon launcher's own.
    let roaming = std::env::var_os("APPDATA").map(PathBuf::from)?;
    under(&roaming.join("horizon-launcher"), 3)
}

fn main() {
    let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|d| d.to_owned()))
    else {
        return;
    };
    let game = dir.join("bin").join("horizon-client.exe");
    // `--find-resources`: only say which resources the game would be started with.
    if std::env::args().nth(1).as_deref() == Some("--find-resources") {
        let own = dir.join("bin").join("resources");
        let found = if prepared(&own) { Some(own) } else { elsewhere(&dir) };
        let text = found.map_or_else(|| "none".to_owned(), |p| p.display().to_string());
        let _ = std::fs::write(dir.join("Horizon OneBlock - resources found.txt"), text);
        return;
    }
    let mut command = Command::new(&game);
    if !prepared(&dir.join("bin").join("resources")) {
        match elsewhere(&dir) {
            Some(resources) => {
                command.arg("--resources").arg(resources);
                let _ = std::fs::remove_file(dir.join(NOTE));
            }
            None => {
                let note = format!(
                    "Horizon OneBlock needs the game resources made from your own Minecraft: Java Edition {VERSION}.\r\n\
                     They are not part of this folder: each player prepares their own.\r\n\r\n\
                     Do one of these, then start \"Horizon OneBlock.exe\" again:\r\n\r\n\
                     1. If you built Horizon from source and have played it: open horizon-resources.txt in this\r\n\
                     \x20  folder and write the path of your Horizon folder on its own line, for example\r\n\
                     \x20  D:\\HorizonRust\r\n\
                     \x20  (the resources are in its .local\\horizon-client\\{VERSION} folder).\r\n\r\n\
                     2. Or install the Horizon launcher from playhorizon.dev and start the game from it once,\r\n\
                     \x20  so it prepares the resources from your Minecraft. They are found by themselves after that.\r\n"
                );
                let path = dir.join(NOTE);
                if std::fs::write(&path, note).is_ok() {
                    let _ = Command::new("notepad.exe").arg(&path).spawn();
                }
                return;
            }
        }
    }
    command.args(std::env::args_os().skip(1)).current_dir(&dir);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW
        command.creation_flags(0x0800_0000);
    }
    if let Err(e) = command.spawn() {
        let note = format!("Could not start {}: {e}\r\n", game.display());
        let _ = std::fs::write(dir.join("Horizon OneBlock - could not start.txt"), note);
    }
}
