//! A Horizon client whose mods differ from a hosted world's: the join is
//! refused with the server's build and its offer to send its mods, and a
//! download over a connection of its own brings and installs them. Over real
//! sockets, through the server's own login and configuration. In a process
//! of its own: the mods a game has on are set once. Needs the prepared
//! resources; skipped without them.
extern crate horizon_client_lib as horizon_client;
use horizon_client::{
    lan::{Lan, NetConfig, data::SyncData},
    mod_transfer::{self, Fetch, Source, State},
};
use horizon_protocol::{
    PROTOCOL_VERSION,
    build_id::{self, Build},
    connection::Connection,
    crypto::offline_uuid,
    mod_files,
    mod_list::{self, Entry, Side},
    packets::{
        Empty,
        configuration::{self as cfg, ClientInformation},
        handshake, login,
    },
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard, OnceLock},
    time::{Duration, Instant},
};

fn resources() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../.local/horizon-client")
        .join(horizon_core::VANILLA_VERSION);
    root.join("items.json").exists().then_some(root)
}

fn manifest(id: &str, version: &str) -> String {
    format!(
        "id = \"{id}\"\nname = \"{id}\"\nversion = \"{version}\"\napi = \"0.2\"\nside = \"both\"\ncapabilities = [\"registries\"]\n"
    )
}

/// What the whole process shares: the server's mods on disk, set as this
/// game's own once, and its build.
struct World {
    dir: PathBuf,
    sync: Arc<SyncData>,
    build: Build,
    ruby: Entry,
    pipes: Entry,
    pipes_bytes: Vec<u8>,
}

/// The fixture, and the lock that runs one test at a time: a server sends to
/// two clients at once at most.
fn world() -> Option<(&'static World, MutexGuard<'static, ()>)> {
    static WORLD: OnceLock<Option<World>> = OnceLock::new();
    static ONE: Mutex<()> = Mutex::new(());
    let guard = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let world = WORLD.get_or_init(|| {
        let resources = resources()?;
        let dir = std::env::temp_dir().join(format!("horizon-mod-download-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        // A mod run from a folder and one from a file, as a server has them.
        let ruby_dir = dir.join("server/ruby");
        let write = |name: &str, bytes: &[u8]| {
            let path = ruby_dir.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, bytes).unwrap();
        };
        write("horizon-mod.toml", manifest("ruby", "1.2.0").as_bytes());
        write("assets/ruby/lang/en_us.json", br#"{"item.ruby.gem": "Ruby"}"#);
        write(
            "data/ruby/horizon/item/gem.json",
            br#"{"template": "minecraft:diamond"}"#,
        );
        write("LICENSE", b"MIT");
        let pipes_bytes = horizon_mods::Package::write_files(
            &manifest("pipes", "2.0"),
            None,
            &[("assets/pipes/lang/en_us.json", br#"{"block.pipes.pipe": "Pipe"}"#)],
        )
        .unwrap();
        let pipes_file = dir.join("server/pipes.hmod");
        std::fs::write(&pipes_file, &pipes_bytes).unwrap();
        let entry = |id: &str, version: &str, sha256| Entry {
            id: id.into(),
            version: version.into(),
            sha256,
            side: Side::Both,
        };
        let ruby = entry("ruby", "1.2.0", None);
        let pipes = entry(
            "pipes",
            "2.0",
            Some(<sha2::Sha256 as sha2::Digest>::digest(&pipes_bytes).into()),
        );
        assert!(mod_list::set_local(vec![ruby.clone(), pipes.clone()]));
        assert!(mod_transfer::set_sources(vec![
            Source {
                entry: ruby.clone(),
                path: ruby_dir.clone(),
                declared: Vec::new(),
            },
            Source {
                entry: pipes.clone(),
                path: pipes_file,
                declared: Vec::new(),
            },
        ]));
        let build = Build {
            version: "0.1.0".into(),
            nightly: None,
            revision: "e42afee0c02067".into(),
            dirty: false,
            fingerprint: [3; 32],
            native: vec![("jei".into(), "0.3".into())],
        };
        assert!(build_id::set_local(build.clone()));
        // Packed folders go to `.horizon-cache` in the working directory.
        std::env::set_current_dir(&dir).unwrap();
        let sync = Arc::new(
            SyncData::load(&resources, &horizon_world::loot::PackOverlay::NONE).expect("sync data"),
        );
        Some(World {
            dir,
            sync,
            build,
            ruby,
            pipes,
            pipes_bytes,
        })
    });
    world.as_ref().map(|world| (world, guard))
}

fn host(world: &World, serve_mods: bool) -> Lan {
    let net = NetConfig {
        announce: false,
        serve_mods,
        ..NetConfig::loopback(0, "Mods - World".into(), 8)
    };
    Lan::start_with(world.sync.clone(), net).unwrap()
}

/// Downloads `wanted` into a new `horizon-mods` and waits for the end.
fn fetch(world: &World, port: u16, name: &str, wanted: Vec<Entry>) -> (PathBuf, State) {
    let mods_dir = world.dir.join(name).join("horizon-mods");
    let job = mod_transfer::fetch(Fetch {
        address: format!("127.0.0.1:{port}"),
        name: name.into(),
        auth: None,
        lang: Arc::default(),
        info: ClientInformation::new(4),
        wanted,
        mods_dir: mods_dir.clone(),
        declared: Vec::new(),
        lan: false,
    });
    let start = Instant::now();
    while job.running() {
        assert!(start.elapsed() < Duration::from_secs(60), "{:?}", job.state());
        std::thread::sleep(Duration::from_millis(10));
    }
    (mods_dir, job.state())
}

/// What a Horizon client with no mods hears when it joins: the server's
/// build, its offer, and why it was turned away.
struct Refusal {
    build: Option<Build>,
    offer: Option<u64>,
    server_mods: Vec<Entry>,
    reason: String,
}

fn join_without_mods(port: u16, name: &str) -> Result<Refusal, String> {
    let e = |e: horizon_protocol::Error| e.to_string();
    let mut c = Connection::connect("127.0.0.1", port, Duration::from_secs(5)).map_err(e)?;
    c.reader
        .set_read_timeout(Some(Duration::from_secs(20)))
        .map_err(e)?;
    c.send(
        &handshake::Serverbound::Intention(handshake::Intention {
            protocol: PROTOCOL_VERSION,
            host: "127.0.0.1".into(),
            port,
            intent: handshake::LOGIN,
        })
        .encode(),
    )
    .map_err(e)?;
    c.send(
        &login::Serverbound::Hello(login::ServerHello {
            name: name.into(),
            uuid: offline_uuid(name),
        })
        .encode(),
    )
    .map_err(e)?;
    loop {
        let (id, payload) = c.recv().map_err(e)?;
        match login::Clientbound::decode(id, &payload).map_err(e)? {
            login::Clientbound::Compression(p) => c.set_compression(p.threshold),
            login::Clientbound::LoginFinished(_) => {
                c.send(&login::Serverbound::LoginAcknowledged(Empty).encode())
                    .map_err(e)?;
                break;
            }
            login::Clientbound::Disconnect(d) => return Err(d.reason_json),
            other => return Err(format!("unexpected login packet {other:?}")),
        }
    }
    let horizon = || {
        let mut w = horizon_protocol::buf::Writer::new();
        w.string("horizon");
        cfg::Serverbound::CustomPayload(cfg::CustomPayload {
            channel: "minecraft:brand".into(),
            data: w.data,
        })
        .encode()
    };
    c.send(&horizon()).map_err(e)?;
    c.send(&cfg::Serverbound::ClientInformation(ClientInformation::new(4)).encode())
        .map_err(e)?;
    let mut heard = Refusal {
        build: None,
        offer: None,
        server_mods: Vec::new(),
        reason: String::new(),
    };
    loop {
        let (id, payload) = c.recv().map_err(e)?;
        match cfg::Clientbound::decode(id, &payload).map_err(e)? {
            cfg::Clientbound::SelectKnownPacks(p) => c
                .send(
                    &cfg::Serverbound::SelectKnownPacks(cfg::SelectKnownPacks { packs: p.packs })
                        .encode(),
                )
                .map_err(e)?,
            cfg::Clientbound::CustomPayload(p) if p.channel == build_id::CHANNEL => {
                heard.build = Some(Build::decode(&p.data).map_err(e)?);
            }
            cfg::Clientbound::CustomPayload(p) if p.channel == mod_files::CHANNEL => {
                match mod_files::Clientbound::decode(&p.data).map_err(e)? {
                    mod_files::Clientbound::Offer { max_total } => heard.offer = Some(max_total),
                    other => return Err(format!("unasked for: {other:?}")),
                }
            }
            // A client with no mods says so, and answers the ping after it.
            cfg::Clientbound::CustomPayload(p) if p.channel == mod_list::CHANNEL => {
                heard.server_mods = mod_list::decode(&p.data).map_err(e)?;
                c.send(
                    &cfg::Serverbound::CustomPayload(cfg::CustomPayload {
                        channel: mod_list::CHANNEL.into(),
                        data: mod_list::encode(&[]),
                    })
                    .encode(),
                )
                .map_err(e)?;
            }
            cfg::Clientbound::Ping(p) => c
                .send(&cfg::Serverbound::Pong(p).encode())
                .map_err(e)?,
            cfg::Clientbound::Disconnect(d) => {
                heard.reason = horizon_protocol::text::plain(&d.reason, &Default::default());
                return Ok(heard);
            }
            cfg::Clientbound::FinishConfiguration(_) => {
                return Err("the server let a client without its mods in".into());
            }
            _ => {}
        }
    }
}

#[test]
fn a_client_without_the_mods_is_told_the_build_and_offered_the_files() {
    let Some((world, _one)) = world() else {
        return;
    };
    let lan = host(world, true);
    let heard = join_without_mods(lan.port, "NoMods").unwrap();
    assert_eq!(heard.build.as_ref(), Some(&world.build));
    assert_eq!(heard.offer, Some(mod_files::MAX_REQUEST));
    assert_eq!(heard.server_mods, [world.pipes.clone(), world.ruby.clone()]);
    assert!(heard.reason.starts_with(mod_list::REFUSAL), "{}", heard.reason);
    assert!(heard.reason.contains("Missing: pipes 2.0, ruby 1.2.0"), "{}", heard.reason);
    // What the screen then lists, and what its download asks for.
    let detail = mod_list::compare_detailed(&heard.server_mods, &[]);
    assert_eq!(detail.wanted(), heard.server_mods);
}

#[test]
fn the_servers_mods_are_downloaded_and_installed() {
    let Some((world, _one)) = world() else {
        return;
    };
    let lan = host(world, true);
    let wanted = vec![world.ruby.clone(), world.pipes.clone()];
    let (mods_dir, state) = fetch(world, lan.port, "Fetcher", wanted);
    let State::Done(installed) = state else {
        panic!("{state:?}");
    };
    assert_eq!(installed.ids, ["ruby", "pipes"]);
    assert!(installed.pending.is_empty());
    // The file as the server has it, and the folder as a `.hmod` of the same content.
    assert_eq!(
        std::fs::read(mods_dir.join("pipes.hmod")).unwrap(),
        world.pipes_bytes
    );
    let ruby = mods_dir.join("ruby.hmod");
    let package = horizon_mods::Package::read(&ruby).unwrap();
    assert_eq!(package.manifest.version, "1.2.0");
    let content = horizon_mods::Content::read(&ruby, &package.manifest).unwrap();
    assert_eq!(
        content.files,
        [
            "assets/ruby/lang/en_us.json",
            "data/ruby/horizon/item/gem.json"
        ]
    );
    assert_eq!(
        horizon_mods::find(&mods_dir),
        [mods_dir.join("pipes.hmod"), ruby]
    );
    // The folder was packed into the server's cache, not beside its mods.
    let cache = world.dir.join(".horizon-cache/served-mods");
    let packed: Vec<_> = std::fs::read_dir(&cache).unwrap().flatten().collect();
    assert_eq!(packed.len(), 1);
    assert!(
        packed[0]
            .file_name()
            .to_string_lossy()
            .starts_with("ruby-1.2.0-")
    );
    // The connection that came for mods never became a player, and is gone.
    let start = Instant::now();
    while lan.shared.online() != 0 {
        assert!(start.elapsed() < Duration::from_secs(15), "still listed");
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn a_mod_the_server_does_not_have_is_refused_and_nothing_is_installed() {
    let Some((world, _one)) = world() else {
        return;
    };
    let lan = host(world, true);
    let nope = Entry {
        id: "nope".into(),
        ..world.ruby.clone()
    };
    let (mods_dir, state) = fetch(world, lan.port, "Asker", vec![world.pipes.clone(), nope]);
    let State::Failed(why) = state else {
        panic!("{state:?}");
    };
    assert!(why.contains("nope") && why.contains("not a mod this server shares"), "{why}");
    assert!(horizon_mods::find(&mods_dir).is_empty());
    assert!(!mods_dir.join(".downloads/pipes.hmod.part").exists());
}

#[test]
fn a_server_with_the_switch_off_sends_nothing() {
    let Some((world, _one)) = world() else {
        return;
    };
    let lan = host(world, false);
    // No offer is heard, and the refusal still names the mods.
    let heard = join_without_mods(lan.port, "NoMods").unwrap();
    assert_eq!(heard.offer, None);
    assert!(heard.reason.starts_with(mod_list::REFUSAL), "{}", heard.reason);
    let (mods_dir, state) = fetch(world, lan.port, "Fetcher2", vec![world.ruby.clone()]);
    let State::Failed(why) = state else {
        panic!("{state:?}");
    };
    assert!(why.contains("does not share"), "{why}");
    assert!(horizon_mods::find(&mods_dir).is_empty());
}
