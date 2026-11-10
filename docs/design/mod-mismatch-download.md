# Plan: mod-mismatch screen, download from the server, build identity

Drawn up 2026-11-10 by reading the source (nothing edited). All paths are under the repository root.
Decisions taken with the user: the missing content mods are downloaded **from the server being joined**;
the screen also shows **both builds** so two players can see whether they run the same game build.
Native mods are compiled into the game and cannot be downloaded: a differing build is only shown.

## 0. Findings that shape the design

- **One server path.** The dedicated server is `horizon-client --dedicated` (`crates/horizon-server/src/engine.rs:269`), so it and the LAN host both run `login_and_configure` in `crates/horizon-client/src/lan/conn.rs:1153`.
- **The client keeps nothing today.** `configure` in `crates/horizon-client/src/net.rs:1129` answers `horizon:mod_list` at `:1165-1175` and drops the server's list.
- **A restart is required.** `mod_list::LOCAL` is a `OnceLock` (`crates/horizon-protocol/src/mod_list.rs:69`). Only asset/data-only mods switch live (`mod_content::live_switch`, `mod_content.rs:2923`; `switch_mods`, `game/mods.rs:504`).
- **No extraction is needed.** The client loads `.hmod` zips directly (`Package::read`, `crates/horizon-mods/src/manifest.rs:326`; `Content::read_declaring`, `content.rs:596`). Send each mod as one `.hmod` stream.
- **No streaming packer exists.** `Package::write_files` (`manifest.rs:382`) packs in memory only.
- **Folder beats file.** `horizon_mods::find` (`manifest.rs:409`) sorts `<id>/` before `<id>.hmod`, and `content_root` keeps the first id (`game/mods.rs:118`). An old folder must be moved aside.
- **Native mods change the second check.** Native blocks/items join the content identity (`native_declared`, `game/mods.rs:60`). So "Content registry mismatch" (`conn.rs:1362-1400`) is the usual symptom of differing builds and must open the same screen.
- **No revision is compiled in.** Rust has `release::VERSION`/`NIGHTLY` (`crates/horizon-core/src/release.rs:13,20`) and `HORIZON_MIXIN_MODS` ids (`crates/horizon-client/build.rs:18-21`). `.horizon-revision` is written (`.local/native/build-native.ps1:53`) and exists in `.local/native/stage/`, but nothing reads it.
- **Git revision alone is not enough.** `build-native.ps1:44-48` copies uncommitted changes into the build. `.horizon-mixins-stage.json` has `inputs` (path to sha256) plus `injections`/`modules` with fingerprints, which identify the real source.
- **Size limits.** Serverbound custom payload is 32767 bytes (`conn.rs:1810`, enforced in `decode_config:1836`). Clientbound follows vanilla's 1 MiB (`attachment_sync.rs:25`); frames cap at 2 MiB - 1 (`frame.rs:11`).
- **`--server HOST[:PORT]` exists** (`game.rs:926`) and feeds `StartupMenu::new` (`game/startup_menu.rs:61`).

## 1. Protocol (`crates/horizon-protocol/src/`)

1. `mod_list.rs`: add `compare_detailed(server, client) -> Detailed { missing: Vec<Entry>, extra: Vec<Entry>, changed: Vec<(Entry, Entry)> }`. Rebuild `compare` (`:195`) on it so `reason()` text and the tests at `:233-345` stay unchanged.
2. New `build_id.rs`, modelled on `mod_list` (OnceLock `set_local`/`local`):
   - Channel `horizon:build`.
   - `Build { version, nightly: Option<String>, revision (<=40), dirty: bool, fingerprint: [u8; 32], native: Vec<(id, version)> (<=128, 64 chars each) }`.
   - `label()` gives e.g. `0.1.0 (rev e42afee, 53 native mods, build 1a2b3c4d)`.
3. New `mod_files.rs`, channel `horizon:mod_files`, first byte is the message kind:
   - Clientbound `Offer { max_total: u64 }`: the server serves files.
   - Serverbound `Request { ids }`: at most 128 ids, fits in 32767 bytes.
   - Clientbound `Begin { id, version, size: u64, chunks: u32 }`.
   - Clientbound `Chunk { seq: u32, bytes }`, `CHUNK = 256 KiB`.
   - Clientbound `End { sha256 }` per mod, `Refused { id, why }`, and `Done`.
   - Serverbound `Ack { seq }` every 8 chunks and `Cancel`.
   - Caps: `MAX_MOD = 512 MiB` (matches `content::MAX_TOTAL`, `content.rs:23`), `MAX_REQUEST = 1 GiB`.

## 2. Build identity

1. `crates/horizon-mod-api-gen/src/staged.rs` (beside `staged_mods:12`): add `staged_native(root) -> Vec<(id, version)>` and `staged_fingerprint(root)`. The fingerprint is a SHA-256 over sorted `inputs` under `crates/` plus the injection and module fingerprints.
2. `crates/horizon-client/build.rs` after `:21`:
   - Read `<root>/.horizon-revision`, else `git rev-parse HEAD` plus a dirty flag, else `unknown`.
   - Emit `HORIZON_BUILD_REV`, `HORIZON_BUILD_FINGERPRINT`, `HORIZON_BUILD_NATIVE`.
3. Register it once at start, beside `register_standalone_models` (`game/mods.rs:76`). Merge in `native-mods.json` when it sits beside the exe (`native_mods_detail`, `game/mods.rs:325`).

## 3. Client handshake (`crates/horizon-client/src/net.rs`)

1. Add `Event::ServerBuild(Build)`, `Event::ServerMods(Vec<Entry>)` and `Event::ModFilesOffer(Offer)` to the enum at `:42`.
2. Emit them from `receive_brand` (`:1305`) and from the mod-list arm (`:1165`), after replying as today.
3. Store them on `Remote` (`game/remote.rs:511`), next to the `Event::Brand` handling at `:2259`.
4. In `configure`, when a `Disconnect` arrives (`:1241`):
   - if the local `compare` is non-empty, return a `Reason` with key `horizon.disconnect.mods`;
   - if the text is `content_identity::MISMATCH`, use key `horizon.disconnect.build`.
5. Add both keys to `reconnect::classified_key` (`reconnect.rs:20`) as never-retry.
6. In the `Event::Disconnected` handler (`game/remote.rs:2361-2385`), take the stored lists, builds, offer, address, name and lan before `leave_server`. Extend `leave_server` (`:1575`) to build the new screen for those two keys.

## 4. Server (`crates/horizon-client/src/lan/conn.rs`)

1. Beside the quiet/mod_net announcements (`:1294-1310`), send `horizon:build` to Horizon clients. Send the `Offer` when serving is on.
2. In the mod-list wait loop (`:1338-1349`), add an arm for a `mod_files::Request` arriving before `Pong(1)`:
   - call `mod_transfer::serve(c, shared, ids)`;
   - then send `Disconnect` and `return Ok(None)`. A download connection never reaches play.
   - Login, whitelist and ban checks have already run in `serve_login` (`:947`).
3. `serve` looks ids up only in a table recorded at load. In `content_root` (`game/mods.rs:101-180`), collect `(Entry, PathBuf)` and call `mod_transfer::set_sources` beside `set_local` (`:179`). Unknown or non-`Both` ids get `Refused`. No client string ever becomes a path.
4. What gets sent:
   - a `.hmod` goes as is, so its hash equals the announced `sha256`;
   - a folder is packed once by a new streaming `Package::pack_dir(dir, out)` in `manifest.rs` (manifest plus `Content.files`; `walk` at `content.rs:1302` already skips symlinks; `check_path` validates names);
   - the packed file is cached at `.horizon-cache/served-mods/<id>-<version>-<stamp>.hmod`.
5. Flow control: at most 16 unacknowledged chunks. Model the read side on `wait_alive` (`:1569`) so keep-alives continue.
6. Limits: 2 concurrent transfers (static counter), a per-address cool-down, and `MAX_REQUEST`.
7. Switch: `horizon-serve-mods` in server.properties (default on).
   - Constant beside `HORIZON_CLIENTS_ONLY` (`lan/mod.rs:205`), field in `NetConfig` (`lan/mod.rs:274`), read in `game/dedicated.rs:1142`.
   - Document it in `docs/hosting.md` near `:343`.

## 5. Client download and install (new `crates/horizon-client/src/mod_transfer.rs`, lib crate)

1. Extract connect-and-login from `run` (`net.rs:764-944`) into a helper.
2. `fetch(address, name, auth, lang, wanted: Vec<Entry>, home) -> Job` runs on a thread with shared progress:
   - logs in and answers brand, known packs and keep-alives;
   - on `horizon:mod_list` sends `Request` and holds back the `Pong`.
3. Each mod streams to `horizon-mods/.downloads/<id>.hmod.part`. Check `seq`, `size` and the SHA-256.
4. Validate with existing readers before installing:
   - `Package::read`: id and version equal the server's entry, side is both, `wasm.is_none()`;
   - `Content::read` passes.
5. Install:
   - move an existing `<id>/` or `<id>.hmod` to `horizon-mods/.replaced/<id>-<version>-<time>/` (never delete; `find` ignores it);
   - rename the `.part` to `horizon-mods/<id>.hmod`, the same part-then-rename pattern as `mod_registry::install` (`mod_registry.rs:207-219`);
   - drop a stale `mod-browser.json` entry (`Installed::record`, `:302`).
6. Settings go through the game's in-memory copy, not the file. `horizon-mods.txt` is saved from `settings.mods` (`crates/horizon-client-render/src/settings.rs:370,691`).
   - Use `ModList::turn_on` (`crates/horizon-client-render/src/mod_list.rs:135`) and add a `turn_off`.
   - Then save. Do not call `switch_mods`.
7. Restart:
   - add a pure `restart_args(original, address)` in `game.rs` that drops `--world`, `--quickPlaySingleplayer`, `--server`, `--quickPlayMultiplayer`, `--session-file`, `--frames` and appends `--server ADDRESS`;
   - spawn `current_exe()` with them, then `event_loop.exit()` (`startup_menu.rs:746`);
   - if the spawn fails, show "Restart the game, then join again".

## 6. The screen (`crates/horizon-client/src/multiplayer.rs`)

1. Add `Screen::ModMismatch { address, name, lan, detail, server_build, client_build, offer, job, turn_off_extra, scroll, note }` to the enum at `:97`.
2. Follow the existing patterns:
   - `buttons()` returns `ButtonSpec` (`:187`, Disconnected at `:894`);
   - `draw` uses `centered` and `wrap` (`:1484`, `:2169`);
   - `click` matches `(self, id)` (`:1554`, `:1785`);
   - scrolling follows `List` (`crate::ui::Scroll`, `wheel:1881`).
3. Also update `id()` (`:171`, new id `mod_mismatch`), `narration` (`:957`), `escape` (`:2089`) and `button_action` (`:157`). Pick unused button ids; 61 and up look free, verify.
4. Layout, top to bottom:
   - title "Your mods don't match this server's";
   - `Server: <build label>` and `You: <build label>`, green when equal;
   - when they differ: "Native mods are built into the game and can't be downloaded. Both players need the same build.", plus the differing native ids;
   - a scrolling list in three sections: Missing, Different version or file (`server x, yours y`), Not on this server;
   - a progress line: `Downloading chipped 3.1 / 12.4 MB (2 of 5)`;
   - buttons: "Download and restart" (only with an offer and something to fetch), "Turn off extra mods: ON/OFF", "Back to Server List".
5. New `Outcome::FetchMods` and `Outcome::ApplyAndRestart` (`:150`), mapped to `title::Action` (`title.rs:371`, `:3655`), handled in `menu_action` (`game/startup_menu.rs:744`).
6. Other matchers of `Disconnected` to update: `game/agent.rs:674`, `game/net_harness.rs:48`, `game/split/players.rs:62`, `game/extras.rs:121`.

## 7. Tests

- Extend the existing `mod_list.rs` tests (`:233-345`) with `compare_detailed`.
- Round-trip and cap tests for `mod_files` and `build_id`.
- `staged.rs` fingerprint tests, beside its existing workspace tests (`:33`).
- `mod_transfer` temp-folder tests, in the style of `mod_registry/tests`:
  - pack a folder, then read it back with `Package::read` and `Content::read`;
  - refusals: wrong id, wrong version, code inside, bad hash, oversize;
  - a folder is moved aside.
- New integration file `crates/horizon-client/tests/mod_download.rs`. It needs its own process because `set_local` is global. Reuse the `lan_bot.rs` helpers (`connect:86`, `server_with:218`). Cover a bot fetch, an unknown id, and the switch off.
- A client `configure` test like the one at `net.rs:1982`.
- A layout test like `multiplayer.rs:2397`.
- A `restart_args` test.
- Manual two-client run with `docs/horizon-ctl-commands.md`:
  - two `--game-dir` folders with different `horizon-mods`;
  - A: `open`, then `lan open 25570`;
  - B: `connect 127.0.0.1:25570`, `wait-for screen mod_mismatch`, `ui`, `click-button`.

## 8. Order of work (each step compiles)

1. `compare_detailed`.
2. `build_id`, the `build.rs` variables and the `horizon:build` send and receive.
3. Client events, the disconnect keys and the screen, read-only (lists and builds; no download yet).
4. `mod_files` wire format.
5. `pack_dir` and `set_sources`.
6. Server `serve` and the config switch.
7. Client `fetch` and install.
8. Settings change and restart.
9. Docs: `docs/mods.md:107` ("Playing on a server with mods") and `docs/hosting.md`.

## 9. Risks and open questions

1. **Signed-in account on restart.** `--session-file` is read once then deleted (`game.rs:717-729`). A relaunch loses the account for online-mode servers unless the child gets it through `--session-stdin`. That needs a serializer in `crates/horizon-client-render/src/auth.rs`, which was not read.
2. **Launcher-started games.** The relaunched child is not the process the launcher tracks. The `Play ... .bat` shortcuts are unaffected.
3. **Windows rename.** Moving an in-use mod folder may fail, because folder files can be shared into `.horizon-cache` (`FileReader::source_path`, `content.rs:549`). Fallback: write a pending-install record and apply it at start, before `content_root`.
4. **LAN and friend joins.** `--server` gives `lan: false`. LAN-discovered and `friend:CODE` joins may need a `lan` flag or invite support. Relay joins (`net.rs:557`) would push files through the relay; consider not offering there.
5. **Folder mods have no hash** (`announced_entry`, `game/mods.rs:206`). Same-version edited folders pass the list check and fail the identity check. With equal builds the screen can only offer "Download all shared mods again". A packaging-independent content hash would fix this but changes the meaning of `sha256` for older peers.
6. **Reconfiguration** calls `configure` again (`net.rs:1029`). Make sure the stored lists are simply replaced.
7. **Licences.** `pack_dir` should carry top-level `LICENSE*` files; readers ignore extra entries.
8. **Control socket.** The restarted child may race the dying parent for `--control`.

### Critical files
- `crates/horizon-client/src/lan/conn.rs`
- `crates/horizon-client/src/net.rs`
- `crates/horizon-client/src/multiplayer.rs`
- `crates/horizon-protocol/src/mod_list.rs`
- `crates/horizon-client/src/game/mods.rs`
