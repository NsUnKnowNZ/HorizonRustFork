//! A server's mods sent through memory to a client: a folder packed and a
//! `.hmod` as it is, what each side refuses, and installing without ever
//! deleting the player's own copy.
use super::*;
use horizon_protocol::connection::Pipe;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "horizon-mod-transfer-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn manifest(id: &str, version: &str, side: &str) -> String {
    let api = if side == "server" { "0.3" } else { "0.2" };
    let capabilities = if side == "server" { "" } else { "\"registries\"" };
    format!(
        "id = \"{id}\"\nname = \"{id}\"\nversion = \"{version}\"\napi = \"{api}\"\nside = \"{side}\"\ncapabilities = [{capabilities}]\n"
    )
}

/// The content of the mod `id`: a texture big enough for several chunks'
/// worth of acknowledgements when asked, a lang file and an item.
fn files(id: &str, big: bool) -> Vec<(String, Vec<u8>)> {
    // Not compressible: the packed file stays as big as the texture.
    let mut texture = Vec::new();
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let len = if big { mod_files::CHUNK * 20 + 1234 } else { 3000 };
    while texture.len() < len {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        texture.extend_from_slice(&state.to_le_bytes());
    }
    vec![
        (format!("assets/{id}/textures/item/gem.png"), texture),
        (format!("assets/{id}/lang/en_us.json"), b"{}".to_vec()),
        (
            format!("data/{id}/horizon/item/gem.json"),
            br#"{"template": "minecraft:diamond"}"#.to_vec(),
        ),
    ]
}

/// The mod as a `.hmod`'s bytes.
fn hmod(id: &str, version: &str, big: bool) -> Vec<u8> {
    let files = files(id, big);
    let files: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(name, bytes)| (name.as_str(), bytes.as_slice()))
        .collect();
    Package::write_files(&manifest(id, version, "both"), None, &files).unwrap()
}

/// The mod as a folder `dir/<id>`.
fn folder(dir: &Path, id: &str, version: &str, big: bool) -> PathBuf {
    let root = dir.join(id);
    let write = |name: &str, bytes: &[u8]| {
        let path = root.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    };
    write(horizon_mods::manifest::MANIFEST, manifest(id, version, "both").as_bytes());
    for (name, bytes) in files(id, big) {
        write(&name, &bytes);
    }
    write("LICENSE", b"MIT");
    root
}

fn entry(id: &str, version: &str, sha256: Option<[u8; 32]>) -> Entry {
    Entry {
        id: id.into(),
        version: version.into(),
        sha256,
        side: mod_list::Side::Both,
    }
}

fn source(id: &str, version: &str, path: &Path) -> Source {
    let sha256 = path
        .is_file()
        .then(|| Sha256::digest(fs::read(path).unwrap()).into());
    Source {
        entry: entry(id, version, sha256),
        path: path.to_owned(),
        declared: Vec::new(),
    }
}

fn payload(channel: &str, data: Vec<u8>) -> Vec<u8> {
    cfg::Clientbound::CustomPayload(cfg::CustomPayload {
        channel: channel.into(),
        data,
    })
    .encode()
}

/// The server's side of a download's connection as a join starts it: the
/// offer, its mods and the ping after them, then `after_request` with the
/// ids the client asked for.
fn serve_on(
    pipe: Pipe,
    offer: Option<u64>,
    after_request: impl FnOnce(&mut Connection, Vec<String>) -> Result<Served, String> + Send + 'static,
) -> std::thread::JoinHandle<Result<Served, String>> {
    std::thread::spawn(move || {
        let e = |e: horizon_protocol::Error| e.to_string();
        let mut c = Connection::local(pipe);
        if let Some(max_total) = offer {
            send(&mut c, Down::Offer { max_total })?;
        }
        c.send(&payload(mod_list::CHANNEL, mod_list::encode(&[])))
            .map_err(e)?;
        c.send(&cfg::Clientbound::Ping(cfg::Ping { id: 1 }).encode())
            .map_err(e)?;
        loop {
            let (id, body) = c.recv().map_err(e)?;
            match cfg::Serverbound::decode(id, &body).map_err(e)? {
                cfg::Serverbound::CustomPayload(p) if p.channel == mod_files::CHANNEL => {
                    let Up::Request { ids } = Up::decode(&p.data).map_err(e)? else {
                        return Err("not a request".to_owned());
                    };
                    return after_request(&mut c, ids);
                }
                // The download never answers the list's ping.
                cfg::Serverbound::Pong(_) => return Err("the ping was answered".to_owned()),
                _ => {}
            }
        }
    })
}

/// The offer a server makes.
const ALL: Option<u64> = Some(mod_files::MAX_REQUEST);

/// A real server's half: [`serve_from`] over `sources`.
fn serving(pipe: Pipe, sources: Vec<Source>, cache: PathBuf) -> std::thread::JoinHandle<Result<Served, String>> {
    serve_on(pipe, ALL, move |c, ids| serve_from(c, &sources, &ids, &cache))
}

/// The client's half over `pipe`: what [`download`] keeps, or why not.
fn get(pipe: Pipe, wanted: &[Entry], mods_dir: &Path) -> Result<Vec<Staged>, String> {
    let mut c = Connection::local(pipe);
    download(
        &mut c,
        wanted,
        &mods_dir.join(DOWNLOADS),
        &[],
        &Language::new(),
        &Job::new(),
    )
}

/// Downloads `wanted` from a server with `sources` and installs it into `mods_dir`.
fn fetch_into(
    name: &str,
    sources: Vec<Source>,
    wanted: &[Entry],
    mods_dir: &Path,
) -> (Result<Vec<String>, String>, Served) {
    let (client, server) = Pipe::pair();
    let cache = scratch(&format!("{name}-cache"));
    let server = serving(server, sources, cache.clone());
    let result = get(client, wanted, mods_dir).and_then(|staged| {
        let ids: Vec<String> = staged.iter().map(|s| s.entry.id.clone()).collect();
        record_pending(mods_dir, staged)?;
        assert_eq!(apply_pending(mods_dir), Vec::<String>::new());
        Ok(ids)
    });
    let served = server.join().unwrap().unwrap();
    let _ = fs::remove_dir_all(&cache);
    (result, served)
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .map(|read| {
            read.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn a_folder_mod_is_packed_sent_and_installed() {
    let dir = scratch("folder");
    let server_mods = dir.join("server");
    let mods_dir = dir.join("client").join("horizon-mods");
    let ruby = folder(&server_mods, "ruby", "1.2.0", true);
    let sources = vec![source("ruby", "1.2.0", &ruby)];
    let wanted = [entry("ruby", "1.2.0", None)];
    let (result, served) = fetch_into("folder", sources, &wanted, &mods_dir);
    assert_eq!(result.unwrap(), ["ruby"]);
    assert_eq!(served.sent, ["ruby"]);
    assert!(served.refused.is_empty() && !served.cancelled);
    // More chunks than a window: the acknowledgements kept it moving.
    assert!(served.bytes > u64::from(mod_files::WINDOW) * mod_files::CHUNK as u64);
    // Installed as a `.hmod` the game's own readers take, file for file.
    let installed = mods_dir.join("ruby.hmod");
    assert_eq!(horizon_mods::find(&mods_dir), [installed.clone()]);
    let package = Package::read(&installed).unwrap();
    assert_eq!(
        (package.manifest.id.as_str(), package.manifest.version.as_str()),
        ("ruby", "1.2.0")
    );
    assert!(package.wasm.is_none());
    let from_folder = Content::read(&ruby, &package.manifest).unwrap();
    let from_file = Content::read(&installed, &package.manifest).unwrap();
    assert_eq!(from_file.files, from_folder.files);
    let texture = "assets/ruby/textures/item/gem.png";
    assert_eq!(
        from_file.reader().unwrap().read(texture).unwrap(),
        fs::read(ruby.join(texture)).unwrap()
    );
    assert_eq!(installed.metadata().unwrap().len(), served.bytes);
    // Nothing waits and no unfinished download is left.
    assert_eq!(names(&mods_dir.join(DOWNLOADS)), Vec::<String>::new());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_hmod_is_sent_as_it_is_and_several_mods_in_one_request() {
    let dir = scratch("hmod");
    let server_mods = dir.join("server");
    fs::create_dir_all(&server_mods).unwrap();
    let mods_dir = dir.join("client").join("horizon-mods");
    let pipes_bytes = hmod("pipes", "2.0", false);
    let pipes = server_mods.join("pipes-2.0.hmod");
    fs::write(&pipes, &pipes_bytes).unwrap();
    let ruby = folder(&server_mods, "ruby", "1.2.0", false);
    let sources = vec![
        source("pipes", "2.0", &pipes),
        source("ruby", "1.2.0", &ruby),
    ];
    let wanted = [sources[0].entry.clone(), sources[1].entry.clone()];
    assert!(wanted[0].sha256.is_some() && wanted[1].sha256.is_none());
    let (result, served) = fetch_into("hmod", sources, &wanted, &mods_dir);
    assert_eq!(result.unwrap(), ["pipes", "ruby"]);
    assert_eq!(served.sent, ["pipes", "ruby"]);
    // Byte for byte the server's file, under the mod's own name.
    assert_eq!(fs::read(mods_dir.join("pipes.hmod")).unwrap(), pipes_bytes);
    assert!(Package::read(&mods_dir.join("ruby.hmod")).is_ok());
    assert_eq!(names(&mods_dir), [DOWNLOADS, "pipes.hmod", "ruby.hmod"]);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_server_only_sends_its_own_shared_mods() {
    let dir = scratch("refused");
    let server_mods = dir.join("server");
    let mods_dir = dir.join("client").join("horizon-mods");
    let ruby = folder(&server_mods, "ruby", "1.2.0", false);
    // A file beside the mods, a server-side mod and a mod with code: none is sent.
    fs::write(dir.join("secret.hmod"), hmod("secret", "1", false)).unwrap();
    let admin = server_mods.join("admin.hmod");
    fs::write(&admin, hmod("admin", "1", false)).unwrap();
    let coded = server_mods.join("coded.hmod");
    fs::write(
        &coded,
        Package::write_files(&manifest("coded", "1", "both"), Some(b"\0asm"), &[]).unwrap(),
    )
    .unwrap();
    let mut sources = vec![
        source("ruby", "1.2.0", &ruby),
        source("admin", "1", &admin),
        source("coded", "1", &coded),
    ];
    sources[1].entry.side = mod_list::Side::Server;
    let cache = scratch("refused-cache");
    for (asked, why) in [
        ("secret", NOT_SHARED),
        ("admin", NOT_SHARED),
        ("coded", "code"),
        ("ruby_2", NOT_SHARED),
    ] {
        let (client, server) = Pipe::pair();
        let server = serving(server, sources.clone(), cache.clone());
        let wanted = [entry("ruby", "1.2.0", None), entry(asked, "1", None)];
        let refused = get(client, &wanted, &mods_dir).unwrap_err();
        assert!(refused.contains(asked) && refused.contains(why), "{refused}");
        let served = server.join().unwrap().unwrap();
        assert_eq!(served.sent, ["ruby"]);
        assert_eq!(served.refused, [asked]);
        // One refusal and nothing is kept, the mod that did arrive included.
        assert_eq!(names(&mods_dir.join(DOWNLOADS)), Vec::<String>::new());
        assert_eq!(names(&mods_dir), [DOWNLOADS]);
    }
    // A mod whose file was swapped for another since the server loaded it.
    fs::write(&admin, hmod("other", "1", false)).unwrap();
    sources[1].entry.side = mod_list::Side::Both;
    let (client, server) = Pipe::pair();
    let server = serving(server, sources, cache.clone());
    let refused = get(client, &[entry("admin", "1", None)], &mods_dir).unwrap_err();
    assert!(refused.contains("changed since"), "{refused}");
    assert_eq!(server.join().unwrap().unwrap().refused, ["admin"]);
    fs::remove_dir_all(&dir).unwrap();
    let _ = fs::remove_dir_all(&cache);
}

/// A server that sends `messages` whatever was asked.
fn rogue(pipe: Pipe, offer: Option<u64>, messages: Vec<Down>) -> std::thread::JoinHandle<Result<Served, String>> {
    serve_on(pipe, offer, move |c, _| {
        for message in messages {
            // The client may have hung up already: that is its refusal.
            if send(c, message).is_err() {
                break;
            }
        }
        Ok(Served::default())
    })
}

/// A whole file as its messages.
fn whole(id: &str, version: &str, bytes: &[u8]) -> Vec<Down> {
    let mut messages = vec![Down::Begin {
        id: id.into(),
        version: version.into(),
        size: bytes.len() as u64,
        chunks: mod_files::chunks_for(bytes.len() as u64),
    }];
    for (seq, chunk) in bytes.chunks(mod_files::CHUNK).enumerate() {
        messages.push(Down::Chunk {
            seq: seq as u32,
            bytes: chunk.to_vec(),
        });
    }
    messages.push(Down::End {
        sha256: Sha256::digest(bytes).into(),
    });
    messages
}

#[test]
fn the_client_keeps_only_what_it_asked_for_and_can_check() {
    let dir = scratch("rogue");
    let mods_dir = dir.join("horizon-mods");
    let ruby = hmod("ruby", "1.2.0", false);
    let wanted = [entry("ruby", "1.2.0", None)];
    let done = |mut messages: Vec<Down>| {
        messages.push(Down::Done);
        messages
    };
    let with_code =
        Package::write_files(&manifest("ruby", "1.2.0", "both"), Some(b"\0asm"), &[]).unwrap();
    let client_only = Package::write_files(
        &manifest("ruby", "1.2.0", "client"),
        None,
        &[("assets/ruby/lang/en_us.json", b"{}")],
    )
    .unwrap();
    let bad_content = Package::write_files(
        &manifest("ruby", "1.2.0", "both"),
        None,
        &[("data/ruby/horizon/item/gem.json", b"{")],
    )
    .unwrap();
    let mut wrong_hash = whole("ruby", "1.2.0", &ruby);
    *wrong_hash.last_mut().unwrap() = Down::End { sha256: [9; 32] };
    let mut skipped = whole("ruby", "1.2.0", &hmod("ruby", "1.2.0", true));
    skipped.remove(2);
    let mut short = whole("ruby", "1.2.0", &ruby);
    short.remove(1);
    let cases: Vec<(&str, Option<u64>, Vec<Down>, &str)> = vec![
        ("no offer", None, done(whole("ruby", "1.2.0", &ruby)), "does not share"),
        ("another mod", ALL, done(whole("sapphire", "1", &hmod("sapphire", "1", false))), "nobody asked for"),
        ("another version", ALL, done(whole("ruby", "9.9", &hmod("ruby", "9.9", false))), "version 9.9"),
        ("another mod inside", ALL, done(whole("ruby", "1.2.0", &hmod("sapphire", "1.2.0", false))), "the mod sapphire"),
        ("another version inside", ALL, done(whole("ruby", "1.2.0", &hmod("ruby", "1.0", false))), "version 1.0"),
        ("code inside", ALL, done(whole("ruby", "1.2.0", &with_code)), "code"),
        ("not for both sides", ALL, done(whole("ruby", "1.2.0", &client_only)), "both sides"),
        ("content the game refuses", ALL, done(whole("ruby", "1.2.0", &bad_content)), "ruby:"),
        ("not a mod at all", ALL, done(whole("ruby", "1.2.0", b"MZ not a zip")), "ruby:"),
        ("a wrong checksum", ALL, done(wrong_hash), "checksum"),
        ("a chunk skipped", ALL, done(skipped), "out of place"),
        ("an end too early", ALL, done(short), "ended early"),
        ("twice", ALL, done([whole("ruby", "1.2.0", &ruby), whole("ruby", "1.2.0", &ruby)].concat()), "twice"),
        ("never ended", ALL, done(whole("ruby", "1.2.0", &ruby)[..2].to_vec()), "never ended"),
        ("nothing at all", ALL, done(Vec::new()), "did not send ruby"),
        ("refused", ALL, done(vec![Down::Refused { id: "ruby".into(), why: "no".into() }]), "ruby (no)"),
        ("more than offered", Some(10), done(whole("ruby", "1.2.0", &ruby)), "more than the server offered"),
    ];
    for (case, offer, messages, why) in cases {
        let (client, server) = Pipe::pair();
        let server = rogue(server, offer, messages);
        let refused = get(client, &wanted, &mods_dir).unwrap_err();
        assert!(refused.contains(why), "{case}: {refused}");
        let _ = server.join();
        // Nothing is kept and nothing reached the mods.
        assert_eq!(names(&mods_dir.join(DOWNLOADS)), Vec::<String>::new(), "{case}");
        assert!(horizon_mods::find(&mods_dir).is_empty(), "{case}");
    }
    // The checksum the server's list named binds the file too.
    let (client, server) = Pipe::pair();
    let server = rogue(server, ALL, done(whole("ruby", "1.2.0", &ruby)));
    let named = [entry("ruby", "1.2.0", Some([1; 32]))];
    assert!(get(client, &named, &mods_dir).unwrap_err().contains("checksum"));
    let _ = server.join();
    // An id that is no mod's is never asked for, let alone written.
    let (client, _server) = Pipe::pair();
    let odd = [entry("../ruby", "1", None)];
    assert!(get(client, &odd, &mods_dir).unwrap_err().contains("not a mod's id"));
    assert!(!dir.join("ruby.hmod.part").exists());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_cancelled_download_keeps_nothing() {
    let dir = scratch("cancel");
    let mods_dir = dir.join("horizon-mods");
    let server_mods = dir.join("server");
    let ruby = folder(&server_mods, "ruby", "1.2.0", true);
    let (client, server) = Pipe::pair();
    let cache = dir.join("cache");
    let server = serving(server, vec![source("ruby", "1.2.0", &ruby)], cache);
    let job = Job::new();
    job.cancel();
    let mut c = Connection::local(client);
    let cancelled = download(
        &mut c,
        &[entry("ruby", "1.2.0", None)],
        &mods_dir.join(DOWNLOADS),
        &[],
        &Language::new(),
        &job,
    );
    assert_eq!(cancelled.unwrap_err(), CANCELLED);
    drop(c);
    // The server never got a request: its side ends with the connection.
    assert!(server.join().unwrap().is_err());
    assert_eq!(names(&mods_dir.join(DOWNLOADS)), Vec::<String>::new());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn the_players_own_copy_is_moved_aside_never_deleted() {
    let dir = scratch("aside");
    let server_mods = dir.join("server");
    let mods_dir = dir.join("client").join("horizon-mods");
    fs::create_dir_all(&mods_dir).unwrap();
    // The player runs an older ruby from a folder, has a copy of it under
    // another file name too, and another mod that is nobody's business.
    let old_folder = folder(&mods_dir, "ruby", "1.0.0", false);
    fs::write(mods_dir.join("Ruby (old).hmod"), hmod("ruby", "0.9", false)).unwrap();
    fs::write(mods_dir.join("pipes.hmod"), hmod("pipes", "2.0", false)).unwrap();
    assert_eq!(horizon_mods::find(&mods_dir).len(), 3);
    assert!(old_folder.is_dir());
    let ruby = folder(&server_mods, "ruby", "1.2.0", false);
    let sources = vec![source("ruby", "1.2.0", &ruby)];
    let wanted = [entry("ruby", "1.2.0", None)];
    let (result, _) = fetch_into("aside", sources, &wanted, &mods_dir);
    assert_eq!(result.unwrap(), ["ruby"]);
    // The game now finds the server's ruby and the untouched pipes.
    assert_eq!(
        horizon_mods::find(&mods_dir),
        [mods_dir.join("pipes.hmod"), mods_dir.join("ruby.hmod")]
    );
    let now = Package::read(&mods_dir.join("ruby.hmod")).unwrap();
    assert_eq!(now.manifest.version, "1.2.0");
    // Both old copies are whole, in one folder of `.replaced`.
    let replaced = names(&mods_dir.join(REPLACED));
    assert_eq!(replaced.len(), 1, "{replaced:?}");
    assert!(replaced[0].starts_with("ruby-"), "{replaced:?}");
    let aside = mods_dir.join(REPLACED).join(&replaced[0]);
    assert_eq!(names(&aside), ["Ruby (old).hmod", "ruby"]);
    assert_eq!(
        Package::read(&aside.join("ruby")).unwrap().manifest.version,
        "1.0.0"
    );
    assert!(aside.join("ruby/LICENSE").is_file());
    assert_eq!(
        fs::read(aside.join("Ruby (old).hmod")).unwrap(),
        hmod("ruby", "0.9", false)
    );
    // The same mod again replaces the file the last download installed.
    let (result, _) = fetch_into(
        "aside-again",
        vec![source("ruby", "1.2.0", &ruby)],
        &wanted,
        &mods_dir,
    );
    assert_eq!(result.unwrap(), ["ruby"]);
    assert_eq!(names(&mods_dir.join(REPLACED)).len(), 2);
    assert_eq!(horizon_mods::find(&mods_dir).len(), 2);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_download_waits_for_the_next_start_and_a_bad_note_installs_nothing() {
    let dir = scratch("pending");
    let mods_dir = dir.join("horizon-mods");
    let downloads = mods_dir.join(DOWNLOADS);
    fs::create_dir_all(&downloads).unwrap();
    // A checked download kept for later, as when the game holds the old copy.
    let part = downloads.join("ruby.hmod.part");
    fs::write(&part, hmod("ruby", "1.2.0", false)).unwrap();
    let staged = vec![Staged {
        entry: entry("ruby", "1.2.0", None),
        part,
    }];
    record_pending(&mods_dir, staged).unwrap();
    assert_eq!(names(&downloads), [PENDING, "ruby.hmod"]);
    assert!(horizon_mods::find(&mods_dir).is_empty());
    // The next start installs it and the note is gone.
    assert_eq!(apply_pending(&mods_dir), Vec::<String>::new());
    assert_eq!(horizon_mods::find(&mods_dir), [mods_dir.join("ruby.hmod")]);
    assert_eq!(names(&downloads), Vec::<String>::new());
    // With no note there is nothing to do.
    assert_eq!(apply_pending(&mods_dir), Vec::<String>::new());
    // A note someone edited: ids that are no mod's, a file that is another
    // mod, another version, code, or not there. None is installed, none
    // leaves the downloads, and the note is dropped.
    fs::write(downloads.join("sapphire.hmod"), hmod("ruby", "1", false)).unwrap();
    fs::write(downloads.join("pipes.hmod"), hmod("pipes", "2.0", false)).unwrap();
    fs::write(
        downloads.join("coded.hmod"),
        Package::write_files(&manifest("coded", "1", "both"), Some(b"\0asm"), &[]).unwrap(),
    )
    .unwrap();
    fs::write(dir.join("outside.hmod"), hmod("outside", "1", false)).unwrap();
    let note = serde_json::json!({ "installs": [
        { "id": "../outside", "version": "1" },
        { "id": "..", "version": "1" },
        { "id": "Sapphire", "version": "1" },
        { "id": "sapphire", "version": "1" },
        { "id": "pipes", "version": "3.0" },
        { "id": "coded", "version": "1" },
        { "id": "missing", "version": "1" },
    ]});
    fs::write(downloads.join(PENDING), note.to_string()).unwrap();
    assert_eq!(apply_pending(&mods_dir), Vec::<String>::new());
    assert_eq!(horizon_mods::find(&mods_dir), [mods_dir.join("ruby.hmod")]);
    assert_eq!(
        names(&downloads),
        ["coded.hmod", "pipes.hmod", "sapphire.hmod"]
    );
    assert!(dir.join("outside.hmod").is_file());
    assert_eq!(names(&dir), ["horizon-mods", "outside.hmod"]);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_copy_the_game_holds_open_stays_until_it_is_free() {
    let dir = scratch("held");
    let mods_dir = dir.join("horizon-mods");
    let downloads = mods_dir.join(DOWNLOADS);
    fs::create_dir_all(&downloads).unwrap();
    let old = folder(&mods_dir, "ruby", "1.0.0", false);
    let part = downloads.join("ruby.hmod.part");
    fs::write(&part, hmod("ruby", "1.2.0", false)).unwrap();
    record_pending(
        &mods_dir,
        vec![Staged {
            entry: entry("ruby", "1.2.0", None),
            part,
        }],
    )
    .unwrap();
    // The running game reads the old folder's files (Windows then refuses to
    // move the folder; elsewhere it moves at once).
    let held = fs::File::open(old.join("assets/ruby/lang/en_us.json")).unwrap();
    let left = apply_pending(&mods_dir);
    if left.is_empty() {
        assert_eq!(horizon_mods::find(&mods_dir), [mods_dir.join("ruby.hmod")]);
    } else {
        // Nothing moved or changed: the old mod still loads, the new one waits.
        assert_eq!(left, ["ruby"]);
        assert_eq!(horizon_mods::find(&mods_dir), [old.clone()]);
        assert_eq!(
            Package::read(&old).unwrap().manifest.version,
            "1.0.0"
        );
        assert_eq!(names(&downloads), [PENDING, "ruby.hmod"]);
        assert_eq!(names(&mods_dir.join(REPLACED)), Vec::<String>::new());
    }
    drop(held);
    // Free now (the next start): installed, the old folder whole in `.replaced`.
    assert_eq!(apply_pending(&mods_dir), Vec::<String>::new());
    assert_eq!(horizon_mods::find(&mods_dir), [mods_dir.join("ruby.hmod")]);
    let replaced = names(&mods_dir.join(REPLACED));
    assert_eq!(replaced.len(), 1);
    let aside = mods_dir.join(REPLACED).join(&replaced[0]).join("ruby");
    assert_eq!(Package::read(&aside).unwrap().manifest.version, "1.0.0");
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_folder_is_packed_once_until_it_changes() {
    let dir = scratch("cache");
    let cache = dir.join("cache");
    let ruby = folder(&dir.join("server"), "ruby", "1.2.0", false);
    let s = source("ruby", "1.2.0", &ruby);
    let first = packed(&s, &cache).unwrap();
    assert_eq!(first.parent(), Some(cache.as_path()));
    assert!(names(&cache)[0].starts_with("ruby-1.2.0-"));
    let written = first.metadata().unwrap().modified().unwrap();
    assert_eq!(packed(&s, &cache).unwrap(), first);
    assert_eq!(first.metadata().unwrap().modified().unwrap(), written);
    // An edit that changes no version: a new pack, and the old one goes.
    fs::write(ruby.join("assets/ruby/lang/en_us.json"), b"{\"a\": \"b\"}").unwrap();
    let second = packed(&s, &cache).unwrap();
    assert_ne!(second, first);
    assert_eq!(names(&cache).len(), 1);
    assert!(second.is_file() && !first.exists());
    // A version is part of a file name only as plain characters.
    assert_eq!(file_safe("1.2.0-beta+7"), "1.2.0-beta_7");
    assert_eq!(file_safe("../../x"), ".._.._x");
    assert_eq!(file_safe(".."), "_");
    assert_eq!(file_safe("a/b\\c:d"), "a_b_c_d");
    assert_eq!(file_safe(&"v".repeat(100)).len(), 32);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn transfers_are_limited_at_once_and_by_address() {
    let now = Instant::now();
    let a: IpAddr = "203.0.113.7".parse().unwrap();
    let b: IpAddr = "203.0.113.8".parse().unwrap();
    let c: IpAddr = "203.0.113.9".parse().unwrap();
    let first = Slot::take(Some(a), now).expect("a place");
    // The same address again, at once or after the transfer: not yet.
    assert!(Slot::take(Some(a), now).is_none());
    drop(first);
    assert!(Slot::take(Some(a), now + Duration::from_secs(1)).is_none());
    let later = now + COOL_DOWN + Duration::from_secs(1);
    let again = Slot::take(Some(a), later).expect("after the wait");
    // Two run at once; a third waits for one to end.
    let second = Slot::take(Some(b), later).expect("a second place");
    assert!(Slot::take(Some(c), later).is_none());
    drop(second);
    let third = Slot::take(Some(c), later).expect("a place again");
    drop((again, third));
    // This machine's own clients are not made to wait.
    let local: IpAddr = "127.0.0.1".parse().unwrap();
    let one = Slot::take(Some(local), later).expect("local");
    drop(one);
    assert!(Slot::take(Some(local), later).is_some());
    assert!(Slot::take(None, later).is_some());
}
