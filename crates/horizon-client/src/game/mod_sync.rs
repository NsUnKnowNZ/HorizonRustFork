//! A join a Horizon server refused over mods (docs/mods.md, "Playing on a
//! server with mods"): the screen that lists the differences
//! (`multiplayer::ModMismatch`), the download of the server's content mods
//! (`mod_transfer`), and the restart into that server with them turned on.
use super::Game;
use super::event_loop::EventLoopCtl;
use horizon_client::{mod_list, mod_transfer, multiplayer::ModMismatch, net};
use std::{path::PathBuf, sync::OnceLock};

/// What a Horizon server said about its mods during configuration, kept on
/// the connection for the screen shown when it turns this game away.
#[derive(Default)]
pub(super) struct ServerMods {
    /// Its build (`horizon:build`).
    pub(super) build: Option<horizon_protocol::build_id::Build>,
    /// The mods it has on (`horizon:mod_list`): a reconfiguration's list
    /// replaces the first.
    pub(super) mods: Option<Vec<horizon_protocol::mod_list::Entry>>,
    /// It sends its mods' files, up to this many bytes (`horizon:mod_files`).
    pub(super) offer: Option<u64>,
}

/// Where the game was started, before `--game-dir` moved it: a restart
/// starts there, so the same arguments name the same paths.
static START_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Notes the folder the game was started in; call before `--game-dir` changes it.
pub(super) fn note_start_dir() {
    if let Ok(dir) = std::env::current_dir() {
        let _ = START_DIR.set(dir);
    }
}

/// The arguments a restart into the server at `address` gets: `original`
/// (the game's own, without the program's name) minus what would open
/// something else or cannot be given twice (the world, another server, the
/// frame limits of a test run, the launcher's account file, which is read
/// once and deleted), then `--server ADDRESS`. A signed-in player keeps the
/// account's `name`, though not the account.
pub(super) fn restart_args(
    original: impl IntoIterator<Item = String>,
    address: &str,
    name: Option<&str>,
) -> Vec<String> {
    const WITH_VALUE: [&str; 7] = [
        "--world",
        "--quickPlaySingleplayer",
        "--server",
        "--quickPlayMultiplayer",
        "--session-file",
        "--frames",
        "--world-frames",
    ];
    let mut out = Vec::new();
    let mut args = original.into_iter();
    while let Some(arg) = args.next() {
        if WITH_VALUE.contains(&arg.as_str()) || (name.is_some() && arg == "--name") {
            args.next();
        } else if arg != "--session-stdin" {
            out.push(arg);
        }
    }
    out.extend(["--server".to_owned(), address.to_owned()]);
    if let Some(name) = name {
        out.extend(["--name".to_owned(), name.to_owned()]);
    }
    out
}

/// Starts the game again, joining `address`: the same program and arguments
/// (`restart_args`), in the folder this one was started in.
fn restart_into(address: &str, name: Option<&str>) -> std::io::Result<()> {
    let mut command = std::process::Command::new(std::env::current_exe()?);
    command.args(restart_args(std::env::args().skip(1), address, name));
    if let Some(dir) = START_DIR.get() {
        command.current_dir(dir);
    }
    command.spawn().map(drop)
}

impl Game<'_> {
    /// The screen for a disconnect whose `key` says the server refused this
    /// game over its mods (`net::MODS_REFUSED`) or its content registry
    /// (`net::BUILD_REFUSED`); none for any other. Asked while the connection
    /// still holds what the server said.
    pub(super) fn mod_mismatch(&self, reason: &str, key: Option<&str>) -> Option<Box<ModMismatch>> {
        let registry = match key? {
            net::MODS_REFUSED => false,
            net::BUILD_REFUSED => true,
            _ => return None,
        };
        // A splitscreen guest plays in the first player's process: it has no
        // mods of its own to change and nothing to restart.
        if self.options.split_guest {
            return None;
        }
        let remote = self.remote.as_ref()?;
        let said = &remote.server_mods;
        Some(Box::new(ModMismatch::new(
            reason.to_owned(),
            (
                remote.session.address.clone(),
                remote.session.name.clone(),
                self.server_is_lan(),
            ),
            registry,
            said.mods.clone().unwrap_or_default(),
            &horizon_protocol::mod_list::local(),
            [said.build.clone(), horizon_protocol::build_id::local()],
            said.offer,
        )))
    }

    /// Shows the mismatch screen where `leave_server` put the Disconnected one.
    pub(super) fn show_mod_mismatch(&mut self, screen: Box<ModMismatch>) {
        if let Some(title) = self.title.as_mut() {
            title.mode = horizon_client::title::Mode::Multiplayer(
                horizon_client::multiplayer::Screen::ModMismatch(screen),
            );
        }
    }

    /// The mismatch screen's download: the server's mods it names, over a
    /// connection of their own to the server just left, as the same player.
    pub(super) fn fetch_mods(&mut self) {
        let lang = std::sync::Arc::new(net::load_language(&self.options.resources));
        let info = self
            .settings
            .client_information(super::remote::client_view_distance(
                self.settings.render_distance,
            ));
        let auth = self.auth();
        let declared = super::mods::native_item_ids();
        let mods_dir = Self::game_dir().join(mod_list::DIR);
        let Some(m) = self.title.as_mut().and_then(|t| t.mod_mismatch_mut()) else {
            return;
        };
        let wanted = m.wanted();
        if m.job.as_ref().is_some_and(|job| job.running()) || wanted.is_empty() || m.offer.is_none()
        {
            return;
        }
        // A signed-in account always plays under its own name (`join_server`).
        let name = auth
            .as_ref()
            .map_or_else(|| m.name.clone(), |a| a.name.clone());
        log::info!(
            "Horizon: downloading {} mod(s) from {}",
            wanted.len(),
            net::shown_address(&m.address)
        );
        m.note = None;
        m.job = Some(mod_transfer::fetch(mod_transfer::Fetch {
            address: m.address.clone(),
            name,
            auth,
            lang,
            info,
            wanted,
            mods_dir,
            declared,
            lan: m.lan,
        }));
    }

    /// Each frame: a download that ended. One that worked turns its mods on
    /// and restarts the game; one that failed leaves its reason on the screen.
    pub(super) fn mods_fetch_poll(&mut self, event_loop: &dyn EventLoopCtl) {
        use mod_transfer::State;
        let Some(m) = self.title.as_mut().and_then(|t| t.mod_mismatch_mut()) else {
            return;
        };
        match m.job.as_ref().map(|job| job.state()) {
            Some(State::Done(installed)) => {
                if !installed.pending.is_empty() {
                    log::info!(
                        "Horizon: {} is in use: installed when the game starts again",
                        installed.pending.join(", ")
                    );
                }
                self.apply_mods(event_loop, installed.ids);
            }
            Some(State::Failed(why)) => {
                log::warn!("Horizon: mods not downloaded: {why}");
                m.job = None;
                m.note = Some(why);
            }
            _ => {}
        }
    }

    /// Turns the downloaded mods on and, when the screen says so, the mods
    /// the server lacks off; saves that (`horizon-mods.txt`, from the game's
    /// own copy of it) and starts the game again into the server. The mods
    /// load at a start: nothing is switched in the running game.
    pub(super) fn apply_mods(&mut self, event_loop: &dyn EventLoopCtl, fetched: Vec<String>) {
        let Some(m) = self.title.as_mut().and_then(|t| t.mod_mismatch_mut()) else {
            return;
        };
        let (address, extra) = (m.address.clone(), m.extra_to_turn_off());
        for id in &fetched {
            // Not `turn_on`: the Mods page has not listed a mod that just arrived.
            self.settings.mods.disabled.remove(id);
            self.settings.mods.enabled.insert(id.clone());
        }
        for id in &extra {
            self.settings.mods.turn_off(id);
        }
        self.settings.save(&Self::options_path());
        log::info!(
            "Horizon: mods turned on: [{}], off: [{}]; restarting into {}",
            fetched.join(", "),
            extra.join(", "),
            net::shown_address(&address)
        );
        let name = horizon_client::auth::current().map(|a| a.name.clone());
        let restarted = restart_into(&address, name.as_deref());
        if let Err(e) = &restarted {
            log::warn!("Horizon: could not start the game again: {e}");
        }
        if let Some(m) = self.title.as_mut().and_then(|t| t.mod_mismatch_mut()) {
            // Whatever comes next, this download is over.
            m.job = None;
            if restarted.is_err() {
                m.note = Some(horizon_client::lang::get_or(
                    "horizon.mods.mismatch.restart_yourself",
                    "The mods are ready. Restart the game, then join again.",
                ));
            }
        }
        if restarted.is_ok() {
            event_loop.exit();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::restart_args;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|a| (*a).to_owned()).collect()
    }

    #[test]
    fn a_restart_joins_the_server_and_nothing_else() {
        let original = args(&[
            "--game-dir",
            "D:\\Games\\Horizon",
            "--world",
            "OneBlockFresh",
            "--control-socket",
            "ctl.json",
            "--fullscreen",
            "--session-file",
            "account.json",
            "--quickPlayMultiplayer",
            "old.example:25565",
            "--frames",
            "600",
        ]);
        assert_eq!(
            restart_args(original, "192.168.1.20:25570", None),
            args(&[
                "--game-dir",
                "D:\\Games\\Horizon",
                "--control-socket",
                "ctl.json",
                "--fullscreen",
                "--server",
                "192.168.1.20:25570",
            ])
        );
    }

    #[test]
    fn a_restart_with_no_arguments_only_names_the_server() {
        assert_eq!(
            restart_args(Vec::new(), "friend:ABCD-1234", None),
            args(&["--server", "friend:ABCD-1234"])
        );
        // A value that is missing is not read past the end.
        assert_eq!(
            restart_args(args(&["--creative", "--world"]), "host", None),
            args(&["--creative", "--server", "host"])
        );
    }

    #[test]
    fn a_signed_in_player_keeps_the_name_and_not_the_account() {
        let original = args(&[
            "--session-stdin",
            "--name",
            "Other",
            "--quickPlaySingleplayer",
            "World",
            "--server",
            "a.example",
            "--world-frames",
            "9",
        ]);
        assert_eq!(
            restart_args(original.clone(), "b.example", Some("Andre")),
            args(&["--server", "b.example", "--name", "Andre"])
        );
        // Without an account the name given at the start stays.
        assert_eq!(
            restart_args(original, "b.example", None),
            args(&["--name", "Other", "--server", "b.example"])
        );
    }
}
