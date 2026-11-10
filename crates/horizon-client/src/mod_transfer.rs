//! Content mods' files between a Horizon server and a client whose mods
//! differ from its own (`horizon_protocol::mod_files`; docs/mods.md, "Playing
//! on a server with mods").
//!
//! The server half ([`serve`]) sends mods only from the table recorded when
//! the game loaded them ([`set_sources`]): a requested id is compared with
//! that table's ids and never becomes a path. A `.hmod` goes as it is; a
//! folder is packed once into `.horizon-cache/served-mods/`.
//!
//! The client half ([`fetch`]) logs in on a connection of its own, asks for
//! the mods instead of naming its own, and writes each to
//! `horizon-mods/.downloads/`. Nothing is installed before it passed the
//! checks a mod on disk passes (`Package::read`, `Content::read`): the id and
//! version the server named, for both sides, and no code. Installing never
//! deletes: the player's copy of a mod is moved to `horizon-mods/.replaced/`.
//! A copy the running game holds open stays where it is and the new file
//! waits in `.downloads/` for the next start ([`apply_pending`]).
use crate::auth::AuthSession;
use horizon_mods::{Content, Package, Side};
use horizon_protocol::{
    connection::Connection,
    mod_files::{self, Clientbound as Down, Serverbound as Up},
    mod_list::{self, Entry},
    packets::configuration as cfg,
    text::Language,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    net::IpAddr,
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

/// A mod this game loaded, as a server may send it.
#[derive(Clone, Debug)]
pub struct Source {
    /// What the server tells joining clients about it.
    pub entry: Entry,
    /// The `.hmod` or the folder it was loaded from.
    pub path: PathBuf,
    /// The item ids its native code declares (`Content::read_declaring`).
    pub declared: Vec<String>,
}

/// The mods this game loaded, set once beside `mod_list::set_local`
/// (`game/mods.rs`): all a server ever sends.
static SOURCES: OnceLock<Vec<Source>> = OnceLock::new();

/// Sets the mods a server may send; false when they were set already.
pub fn set_sources(sources: Vec<Source>) -> bool {
    SOURCES.set(sources).is_ok()
}

fn shared(source: &Source) -> bool {
    source.entry.side == mod_list::Side::Both
}

/// Whether this game has a mod a client could ask for.
pub fn serves_any() -> bool {
    SOURCES.get().is_some_and(|s| s.iter().any(shared))
}

/// Transfers a server runs at once at most.
const MAX_ACTIVE: usize = 2;
/// How soon an address may ask again.
const COOL_DOWN: Duration = Duration::from_secs(5);
/// A keep-alive this often while a folder is packed.
const KEEP_ALIVE: Duration = Duration::from_secs(5);
/// How long a finished transfer waits for the client to hang up.
const LINGER: Duration = Duration::from_secs(10);
/// Packed folders, in `.horizon-cache`.
const SERVED: &str = "served-mods";
/// What a client is told when the server has no room for it right now.
const BUSY: &str = "the server is sending mods to other players: try again in a moment";
/// What a client is told about a mod that is not the server's to send.
const NOT_SHARED: &str = "not a mod this server shares";

static ACTIVE: AtomicUsize = AtomicUsize::new(0);
static RECENT: Mutex<Vec<(IpAddr, Instant)>> = Mutex::new(Vec::new());
/// One folder is packed at a time.
static PACKING: Mutex<()> = Mutex::new(());

/// A running transfer's place among [`MAX_ACTIVE`].
struct Slot;
impl Slot {
    /// A place for a transfer to `address` (none for a connection through
    /// memory): none while [`MAX_ACTIVE`] run, or when that address asked
    /// within [`COOL_DOWN`]. This machine's own addresses may ask at once.
    fn take(address: Option<IpAddr>, now: Instant) -> Option<Slot> {
        let address = address.filter(|ip| !ip.is_loopback());
        let mut recent = RECENT.lock().unwrap_or_else(|e| e.into_inner());
        recent.retain(|(_, at)| now.saturating_duration_since(*at) < COOL_DOWN);
        if address.is_some_and(|ip| recent.iter().any(|(other, _)| *other == ip)) {
            return None;
        }
        // Counted with `recent` locked: no two transfers take the last place.
        if ACTIVE.load(Ordering::SeqCst) >= MAX_ACTIVE {
            return None;
        }
        ACTIVE.fetch_add(1, Ordering::SeqCst);
        if let Some(ip) = address {
            recent.push((ip, now));
        }
        Some(Slot)
    }
}
impl Drop for Slot {
    fn drop(&mut self) {
        ACTIVE.fetch_sub(1, Ordering::SeqCst);
    }
}

/// What a transfer did, for the server's log.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Served {
    pub sent: Vec<String>,
    pub refused: Vec<String>,
    pub bytes: u64,
    /// The client stopped it.
    pub cancelled: bool,
}

/// Sends the mods a client asked for by `ids`, or says why not, then
/// [`Down::Done`]. Only this game's loaded mods for both sides are sent.
/// `address` is the client's, for the limits.
pub fn serve(
    c: &mut Connection,
    address: Option<IpAddr>,
    ids: &[String],
) -> Result<Served, String> {
    let Some(_slot) = Slot::take(address, Instant::now()) else {
        return refuse_all(c, ids, BUSY);
    };
    let sources = SOURCES.get().map(Vec::as_slice).unwrap_or_default();
    let cache = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join(".horizon-cache")
        .join(SERVED);
    serve_from(c, sources, ids, &cache)
}

/// Refuses every mod of a request for the same reason.
pub fn refuse_all(c: &mut Connection, ids: &[String], why: &str) -> Result<Served, String> {
    let mut served = Served::default();
    for id in ids.iter().take(mod_files::MAX_IDS) {
        send(
            c,
            Down::Refused {
                id: id.clone(),
                why: why.to_owned(),
            },
        )?;
        served.refused.push(id.clone());
    }
    send(c, Down::Done)?;
    linger(c);
    Ok(served)
}

/// Waits for the client to hang up, for [`LINGER`] at most, reading what it
/// still sends. A connection closed over unread acknowledgements is reset,
/// and a reset can cost the client the last of what was sent to it.
fn linger(c: &mut Connection) {
    let _ = c.reader.set_read_timeout(Some(LINGER));
    let until = Instant::now() + LINGER;
    while Instant::now() < until && c.recv().is_ok() {}
}

/// [`serve`] over a given table, packing folders into `cache`.
fn serve_from(
    c: &mut Connection,
    sources: &[Source],
    ids: &[String],
    cache: &Path,
) -> Result<Served, String> {
    let mut served = Served::default();
    let mut budget = mod_files::MAX_REQUEST;
    let mut flow = Flow::default();
    let mut seen: Vec<&str> = Vec::new();
    for id in ids.iter().take(mod_files::MAX_IDS) {
        if seen.contains(&id.as_str()) {
            continue;
        }
        seen.push(id);
        // The id is only compared with the ids of the mods this game loaded:
        // what is opened is the path recorded for the one that matches.
        let source = sources.iter().find(|s| s.entry.id == *id && shared(s));
        let ready = match source {
            Some(source) => ready(c, source, cache, budget).map(|file| (source, file)),
            None => Err(NOT_SHARED.to_owned()),
        };
        match ready {
            Ok((source, (file, size))) => {
                if !send_file(c, &source.entry, file, size, &mut flow)? {
                    served.cancelled = true;
                    linger(c);
                    return Ok(served);
                }
                budget -= size;
                served.bytes += size;
                served.sent.push(id.clone());
            }
            Err(why) => {
                send(
                    c,
                    Down::Refused {
                        id: id.clone(),
                        why,
                    },
                )?;
                served.refused.push(id.clone());
            }
        }
    }
    send(c, Down::Done)?;
    linger(c);
    Ok(served)
}

/// The file to send for `source`, open, and its size: the `.hmod` itself, or
/// its folder packed. An error is why the client is refused it.
fn ready(
    c: &mut Connection,
    source: &Source,
    cache: &Path,
    budget: u64,
) -> Result<(fs::File, u64), String> {
    let path = if source.path.is_dir() {
        keep_alive_while(c, || packed(source, cache))??
    } else {
        source.path.clone()
    };
    // What a client may install: the mod announced, with no code.
    let package = Package::read(&path)?;
    let manifest = &package.manifest;
    if manifest.id != source.entry.id || manifest.version != source.entry.version {
        return Err("it changed since the server started".to_owned());
    }
    if package.wasm.is_some() {
        return Err("it holds code, which a server does not send".to_owned());
    }
    let file = fs::File::open(&path).map_err(|e| e.to_string())?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    if size > mod_files::MAX_MOD {
        return Err(format!(
            "it is over {} MiB",
            mod_files::MAX_MOD >> 20
        ));
    }
    if size > budget {
        return Err("it is over what one request may ask for: ask for it alone".to_owned());
    }
    Ok((file, size))
}

/// Runs `work` on another thread, sending the client a keep-alive every
/// [`KEEP_ALIVE`] until it is done: packing a big folder outlasts a read
/// timeout.
fn keep_alive_while<T: Send>(
    c: &mut Connection,
    work: impl FnOnce() -> T + Send,
) -> Result<T, String> {
    std::thread::scope(|scope| {
        let handle = scope.spawn(work);
        let mut last = Instant::now();
        while !handle.is_finished() {
            std::thread::sleep(Duration::from_millis(10));
            if last.elapsed() >= KEEP_ALIVE {
                last = Instant::now();
                c.send(&cfg::Clientbound::KeepAlive(cfg::KeepAlive { id: 0 }).encode())
                    .map_err(|e| e.to_string())?;
            }
        }
        handle
            .join()
            .map_err(|_| "packing the mod failed".to_owned())
    })
}

/// `text` as part of a file name: letters, digits, `.`, `_` and `-`, at most 32.
fn file_safe(text: &str) -> String {
    let safe: String = text
        .chars()
        .take(32)
        .map(|c| match c {
            c if c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-' => c,
            _ => '_',
        })
        .collect();
    match safe.chars().all(|c| c == '.') {
        true => "_".to_owned(),
        false => safe,
    }
}

/// The folder mod of `source` as a `.hmod` in `cache`, packed when its files
/// changed since the last time (their names, sizes and times).
fn packed(source: &Source, cache: &Path) -> Result<PathBuf, String> {
    let _one = PACKING.lock().unwrap_or_else(|e| e.into_inner());
    let entry = &source.entry;
    let stamp = folder_stamp(&source.path)?;
    let name = format!("{}-{}-{stamp}.hmod", entry.id, file_safe(&entry.version));
    let target = cache.join(&name);
    if target.is_file() {
        return Ok(target);
    }
    fs::create_dir_all(cache).map_err(|e| format!("{}: {e}", cache.display()))?;
    let part = cache.join(format!("{name}.part"));
    let write = || -> Result<(), String> {
        let file = fs::File::create(&part).map_err(|e| e.to_string())?;
        let mut out = std::io::BufWriter::new(file);
        Package::pack_dir(&source.path, &source.declared, &mut out)?;
        out.flush().map_err(|e| e.to_string())
    };
    if let Err(e) = write() {
        let _ = fs::remove_file(&part);
        return Err(e);
    }
    // This mod's earlier packs are of no use any more: the cache's own files.
    let earlier = format!("{}-", entry.id);
    for old in fs::read_dir(cache).into_iter().flatten().flatten() {
        let old_name = old.file_name();
        let old_name = old_name.to_string_lossy();
        if old_name.starts_with(&earlier) && old_name.ends_with(".hmod") {
            let _ = fs::remove_file(old.path());
        }
    }
    fs::rename(&part, &target).map_err(|e| format!("{}: {e}", target.display()))?;
    Ok(target)
}

/// A mark of a mod folder's state: the manifest and every file under
/// `assets/` and `data/` by name, size and time.
fn folder_stamp(dir: &Path) -> Result<String, String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
        let Ok(read) = fs::read_dir(dir) else {
            return Ok(());
        };
        for entry in read.flatten() {
            let path = entry.path();
            let meta = fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if meta.is_dir() {
                walk(root, &path, out)?;
            } else if meta.is_file() {
                let modified = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_nanos());
                let name = path.strip_prefix(root).unwrap_or(&path);
                out.push(format!("{} {} {modified}", name.display(), meta.len()));
            }
            if out.len() > horizon_mods::content::MAX_FILES + 64 {
                return Err("too many files".to_owned());
            }
        }
        Ok(())
    }
    let mut lines = Vec::new();
    for top in ["assets", "data"] {
        walk(dir, &dir.join(top), &mut lines)?;
    }
    let mut top = vec![horizon_mods::manifest::MANIFEST.to_owned()];
    top.extend(horizon_mods::manifest::notices(dir));
    for name in top {
        let meta = fs::symlink_metadata(dir.join(&name)).map_err(|e| format!("{name}: {e}"))?;
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos());
        lines.push(format!("{name} {} {modified}", meta.len()));
    }
    lines.sort();
    let digest = Sha256::digest(lines.join("\n"));
    Ok(digest[..8].iter().map(|b| format!("{b:02x}")).collect())
}

/// The chunks sent and acknowledged over a whole request.
#[derive(Default)]
struct Flow {
    sent: u32,
    acked: u32,
}
impl Flow {
    /// Reads one packet from the client: true to go on, false when it
    /// cancelled. Anything but this channel's messages is passed over (a
    /// keep-alive's answer, the client's settings).
    fn read(&mut self, c: &mut Connection) -> Result<bool, String> {
        let (id, payload) = c.recv().map_err(|e| e.to_string())?;
        let packet = cfg::Serverbound::decode(id, &payload).map_err(|e| e.to_string())?;
        let cfg::Serverbound::CustomPayload(p) = packet else {
            return Ok(true);
        };
        if p.channel != mod_files::CHANNEL {
            return Ok(true);
        }
        if p.data.len() > horizon_protocol::quiet::MAX_PAYLOAD {
            return Err("Payload too large".to_owned());
        }
        match Up::decode(&p.data).map_err(|e| e.to_string())? {
            Up::Ack { seq } => {
                let acked = seq
                    .checked_add(1)
                    .filter(|acked| *acked <= self.sent)
                    .ok_or("mod files: a chunk was acknowledged that was not sent")?;
                self.acked = self.acked.max(acked);
                Ok(true)
            }
            Up::Cancel => Ok(false),
            Up::Request { .. } => Err("mod files: a second request".to_owned()),
        }
    }
}

/// Sends one mod's file: false when the client cancelled.
fn send_file(
    c: &mut Connection,
    entry: &Entry,
    mut file: fs::File,
    size: u64,
    flow: &mut Flow,
) -> Result<bool, String> {
    let chunks = mod_files::chunks_for(size);
    send(
        c,
        Down::Begin {
            id: entry.id.clone(),
            version: entry.version.clone(),
            size,
            chunks,
        },
    )?;
    let mut hasher = Sha256::new();
    let mut left = size;
    let mut buffer = vec![0u8; mod_files::CHUNK];
    for seq in 0..chunks {
        // Never more than a window past what the client said it has.
        while flow.sent - flow.acked >= mod_files::WINDOW {
            if !flow.read(c)? {
                return Ok(false);
            }
        }
        let len = left.min(mod_files::CHUNK as u64) as usize;
        file.read_exact(&mut buffer[..len])
            .map_err(|e| format!("{}: {e}", entry.id))?;
        hasher.update(&buffer[..len]);
        send(
            c,
            Down::Chunk {
                seq,
                bytes: buffer[..len].to_vec(),
            },
        )?;
        left -= len as u64;
        flow.sent += 1;
    }
    send(
        c,
        Down::End {
            sha256: hasher.finalize().into(),
        },
    )?;
    Ok(true)
}

fn send(c: &mut Connection, message: Down) -> Result<(), String> {
    c.send(
        &cfg::Clientbound::CustomPayload(cfg::CustomPayload {
            channel: mod_files::CHANNEL.into(),
            data: message.encode(),
        })
        .encode(),
    )
    .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- the client

/// Downloads waiting for their checks, and those waiting to be installed, in `horizon-mods`.
pub const DOWNLOADS: &str = ".downloads";
/// The player's copies of mods a download replaced, in `horizon-mods`: `find` never looks here.
pub const REPLACED: &str = ".replaced";
/// The mods in [`DOWNLOADS`] that passed their checks and wait to be installed.
const PENDING: &str = "pending.json";
const CANCELLED: &str = "The download was cancelled.";
const NOT_OFFERED: &str = "This server does not share its mods' files: ask its owner for them.";
const NOTHING_SENT: &str = "The server did not send any mods.";
/// What a join through a relay is told: its files would all go through the relay.
pub const RELAYED: &str = "This server is reached through a relay, which mods are not downloaded over: ask the host for the files.";

/// The mod arriving and how far it is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    pub id: String,
    /// Bytes of it here, and bytes it has.
    pub received: u64,
    pub size: u64,
    /// Which of the mods asked for it is, from 1, and how many were asked for.
    pub index: usize,
    pub count: usize,
}

/// Where a download is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum State {
    Connecting,
    Receiving(Progress),
    Installing,
    Done(Installed),
    Failed(String),
}

/// What a finished download brought.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Installed {
    /// Every mod that passed its checks.
    pub ids: Vec<String>,
    /// Those of them still in [`DOWNLOADS`]: the player's copy is held open
    /// by the running game, so they are installed at the next start.
    pub pending: Vec<String>,
}

/// A download running on its own thread, as the screen watches it.
#[derive(Clone)]
pub struct Job {
    state: Arc<Mutex<State>>,
    cancel: Arc<AtomicBool>,
}
impl std::fmt::Debug for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Job({:?})", self.state())
    }
}
impl Job {
    fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(State::Connecting)),
            cancel: Arc::default(),
        }
    }
    pub fn state(&self) -> State {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
    fn set(&self, state: State) {
        *self.state.lock().unwrap_or_else(|e| e.into_inner()) = state;
    }
    /// Stops it at the next packet; it ends as [`State::Failed`].
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
    /// Whether it is still at work.
    pub fn running(&self) -> bool {
        !matches!(self.state(), State::Done(_) | State::Failed(_))
    }
    /// A job in this state with no thread behind it, for a screen's tests.
    pub fn in_state(state: State) -> Self {
        let job = Self::new();
        job.set(state);
        job
    }
}

/// What a download needs.
pub struct Fetch {
    pub address: String,
    pub name: String,
    pub auth: Option<Arc<AuthSession>>,
    pub lang: Arc<Language>,
    pub info: cfg::ClientInformation,
    /// The server's entries to get.
    pub wanted: Vec<Entry>,
    /// `horizon-mods` in the game folder.
    pub mods_dir: PathBuf,
    /// The item ids this game's native mods declare (`Content::read_declaring`).
    pub declared: Vec<String>,
    /// The server was picked from LAN discovery (`Session::connect_lan`).
    pub lan: bool,
}

/// Starts downloading `request.wanted` from the server at `request.address`
/// and installing them.
pub fn fetch(request: Fetch) -> Job {
    let job = Job::new();
    let thread_job = job.clone();
    let spawned = std::thread::Builder::new()
        .name("horizon-mod-fetch".into())
        .spawn(move || {
            let downloads = request.mods_dir.join(DOWNLOADS);
            let state = match fetch_now(&request, &thread_job) {
                Ok(installed) => State::Done(installed),
                Err(why) => {
                    clean_parts(&downloads);
                    State::Failed(why)
                }
            };
            thread_job.set(state);
        });
    if let Err(e) = spawned {
        job.set(State::Failed(e.to_string()));
    }
    job
}

fn fetch_now(request: &Fetch, job: &Job) -> Result<Installed, String> {
    if request.wanted.is_empty() {
        return Err(NOTHING_SENT.to_owned());
    }
    let (send, statuses) = std::sync::mpsc::channel();
    let joined = crate::net::connect_and_login(
        None,
        &request.address,
        &request.name,
        request.auth.as_deref(),
        &request.lang,
        &send,
        &crate::net::Cookies::default(),
        false,
        request.lan,
        &mut |_: &Connection| {},
    );
    let (mut c, _profile) = joined.map_err(|r| r.text)?.map_err(|r| r.text)?;
    let relayed = crate::net::relayed_status(&request.lang);
    if statuses
        .try_iter()
        .any(|event| matches!(event, crate::net::Event::Status(text) if text == relayed))
    {
        return Err(RELAYED.to_owned());
    }
    let e = |e: horizon_protocol::Error| e.to_string();
    c.send(
        &cfg::Serverbound::CustomPayload(cfg::CustomPayload {
            channel: "minecraft:brand".into(),
            data: crate::net::brand(),
        })
        .encode(),
    )
    .map_err(e)?;
    c.send(&cfg::Serverbound::ClientInformation(request.info.clone()).encode())
        .map_err(e)?;
    let downloads = request.mods_dir.join(DOWNLOADS);
    let staged = download(
        &mut c,
        &request.wanted,
        &downloads,
        &request.declared,
        &request.lang,
        job,
    )?;
    drop(c);
    job.set(State::Installing);
    let ids: Vec<String> = staged.iter().map(|s| s.entry.id.clone()).collect();
    record_pending(&request.mods_dir, staged)?;
    let pending = apply_pending(&request.mods_dir);
    Ok(Installed { ids, pending })
}

/// A mod that arrived whole and passed its checks, still in [`DOWNLOADS`].
#[derive(Debug)]
struct Staged {
    entry: Entry,
    part: PathBuf,
}

/// A mod arriving.
struct Incoming {
    entry: Entry,
    part: PathBuf,
    file: std::io::BufWriter<fs::File>,
    size: u64,
    chunks: u32,
    next: u32,
    received: u64,
    hasher: Sha256,
}

/// The configuration phase of a download's connection, from the login on:
/// answers what a join answers, asks for `wanted` when the server names its
/// mods, and takes the files into `downloads`. Every mod asked for must
/// arrive and pass its checks, or nothing is kept.
fn download(
    c: &mut Connection,
    wanted: &[Entry],
    downloads: &Path,
    declared: &[String],
    lang: &Language,
    job: &Job,
) -> Result<Vec<Staged>, String> {
    let result = receive(c, wanted, downloads, declared, lang, job);
    if result.is_err() {
        clean_parts(downloads);
    }
    result
}

fn receive(
    c: &mut Connection,
    wanted: &[Entry],
    downloads: &Path,
    declared: &[String],
    lang: &Language,
    job: &Job,
) -> Result<Vec<Staged>, String> {
    let e = |e: horizon_protocol::Error| e.to_string();
    let reply = |c: &mut Connection, p: cfg::Serverbound| c.send(&p.encode()).map_err(e);
    let up = |c: &mut Connection, message: Up| {
        reply(
            c,
            cfg::Serverbound::CustomPayload(cfg::CustomPayload {
                channel: mod_files::CHANNEL.into(),
                data: message.encode(),
            }),
        )
    };
    let broken = |what: &str| format!("The server's mod files do not add up ({what}).");
    // An id is part of a file's name here: only a mod's id is.
    if let Some(bad) = wanted.iter().find(|w| !mod_files::valid_id(&w.id)) {
        return Err(format!("{:?} is not a mod's id.", bad.id));
    }
    let mut offered: Option<u64> = None;
    let mut requested = false;
    let mut incoming: Option<Incoming> = None;
    let mut staged: Vec<Staged> = Vec::new();
    let mut refused: Vec<(String, String)> = Vec::new();
    let (mut chunks_total, mut bytes_total) = (0u32, 0u64);
    loop {
        if job.cancelled() {
            if requested {
                let _ = up(c, Up::Cancel);
            }
            return Err(CANCELLED.to_owned());
        }
        let (id, payload) = c.recv().map_err(e)?;
        let payload = match cfg::Clientbound::decode(id, &payload).map_err(e)? {
            cfg::Clientbound::KeepAlive(k) => {
                // Unanswered when the server is gone already: the next read says so.
                let _ = reply(c, cfg::Serverbound::KeepAlive(k));
                continue;
            }
            // The ping after the mod list waits for the list's answer, which
            // this connection never gives: the server sends the files instead.
            cfg::Clientbound::Ping(p) => {
                if !requested {
                    reply(c, cfg::Serverbound::Pong(p))?;
                }
                continue;
            }
            cfg::Clientbound::SelectKnownPacks(p) => {
                reply(
                    c,
                    cfg::Serverbound::SelectKnownPacks(cfg::SelectKnownPacks {
                        packs: crate::net::select_known_packs(p.packs),
                    }),
                )?;
                continue;
            }
            cfg::Clientbound::CookieRequest(q) => {
                reply(
                    c,
                    cfg::Serverbound::CookieResponse(cfg::CookieResponse {
                        key: q.key,
                        payload: None,
                    }),
                )?;
                continue;
            }
            cfg::Clientbound::Disconnect(d) => {
                return Err(horizon_protocol::text::plain(&d.reason, lang));
            }
            // The server went on to the world: it has no mods to send.
            cfg::Clientbound::RegistryData(_) | cfg::Clientbound::FinishConfiguration(_) => {
                return Err(NOTHING_SENT.to_owned());
            }
            cfg::Clientbound::CustomPayload(p) => p,
            _ => continue,
        };
        if payload.channel == "minecraft:brand" {
            // A Horizon server offers its extensions to a client that says it is one.
            if horizon_protocol::buf::Reader::new(&payload.data)
                .string(32767)
                .is_ok_and(|brand| brand == "horizon")
            {
                let mut w = horizon_protocol::buf::Writer::new();
                w.string("horizon");
                reply(
                    c,
                    cfg::Serverbound::CustomPayload(cfg::CustomPayload {
                        channel: "minecraft:brand".into(),
                        data: w.data,
                    }),
                )?;
            }
            continue;
        }
        if payload.channel == mod_list::CHANNEL {
            if requested {
                continue;
            }
            if offered.is_none() {
                return Err(NOT_OFFERED.to_owned());
            }
            up(
                c,
                Up::Request {
                    ids: wanted.iter().map(|w| w.id.clone()).collect(),
                },
            )?;
            requested = true;
            job.set(State::Receiving(Progress {
                count: wanted.len(),
                ..Progress::default()
            }));
            continue;
        }
        if payload.channel == crate::content_identity::CHANNEL && !requested {
            return Err(NOTHING_SENT.to_owned());
        }
        if payload.channel != mod_files::CHANNEL {
            continue;
        }
        let message = Down::decode(&payload.data).map_err(e)?;
        if let Down::Offer { max_total } = message {
            offered = Some(max_total.min(mod_files::MAX_REQUEST));
            continue;
        }
        if !requested {
            return Err(broken("files nobody asked for"));
        }
        match message {
            Down::Offer { .. } => {}
            Down::Begin {
                id,
                version,
                size,
                chunks,
            } => {
                if incoming.is_some() {
                    return Err(broken("a mod began inside another"));
                }
                let entry = wanted
                    .iter()
                    .find(|w| w.id == id)
                    .ok_or_else(|| broken("a mod nobody asked for"))?;
                if staged.iter().any(|s| s.entry.id == id) || refused.iter().any(|r| r.0 == id) {
                    return Err(broken("a mod sent twice"));
                }
                if version != entry.version {
                    return Err(format!(
                        "{id}: the server sent version {version}, not the {} it named.",
                        entry.version
                    ));
                }
                bytes_total += size;
                if bytes_total > offered.unwrap_or(0) {
                    return Err(broken("more than the server offered"));
                }
                fs::create_dir_all(downloads)
                    .map_err(|e| format!("{}: {e}", downloads.display()))?;
                let part = downloads.join(format!("{id}.hmod.part"));
                let file =
                    fs::File::create(&part).map_err(|e| format!("{}: {e}", part.display()))?;
                job.set(State::Receiving(Progress {
                    id: id.clone(),
                    received: 0,
                    size,
                    index: staged.len() + refused.len() + 1,
                    count: wanted.len(),
                }));
                incoming = Some(Incoming {
                    entry: entry.clone(),
                    part,
                    file: std::io::BufWriter::new(file),
                    size,
                    chunks,
                    next: 0,
                    received: 0,
                    hasher: Sha256::new(),
                });
            }
            Down::Chunk { seq, bytes } => {
                let i = incoming
                    .as_mut()
                    .ok_or_else(|| broken("a chunk of no mod"))?;
                let expected = (i.size - i.received).min(mod_files::CHUNK as u64);
                if seq != i.next || seq >= i.chunks || bytes.len() as u64 != expected {
                    return Err(broken("a chunk out of place"));
                }
                i.file
                    .write_all(&bytes)
                    .map_err(|e| format!("{}: {e}", i.part.display()))?;
                i.hasher.update(&bytes);
                i.received += bytes.len() as u64;
                i.next += 1;
                chunks_total += 1;
                if chunks_total % mod_files::ACK_EVERY == 0 {
                    // A server that sent everything may be gone before this
                    // acknowledgement: what it sent is still here to read.
                    let _ = up(
                        c,
                        Up::Ack {
                            seq: chunks_total - 1,
                        },
                    );
                }
                job.set(State::Receiving(Progress {
                    id: i.entry.id.clone(),
                    received: i.received,
                    size: i.size,
                    index: staged.len() + refused.len() + 1,
                    count: wanted.len(),
                }));
            }
            Down::End { sha256 } => {
                let mut i = incoming.take().ok_or_else(|| broken("an end of no mod"))?;
                if i.next != i.chunks || i.received != i.size {
                    return Err(broken("a mod that ended early"));
                }
                i.file
                    .flush()
                    .map_err(|e| format!("{}: {e}", i.part.display()))?;
                drop(i.file);
                let hash: [u8; 32] = i.hasher.finalize().into();
                let id = &i.entry.id;
                if hash != sha256 || i.entry.sha256.is_some_and(|named| named != hash) {
                    return Err(format!(
                        "{id}: the file is not the one the server named (its checksum differs)."
                    ));
                }
                check(&i.part, &i.entry, declared).map_err(|why| format!("{id}: {why}."))?;
                staged.push(Staged {
                    entry: i.entry,
                    part: i.part,
                });
            }
            Down::Refused { id, why } => {
                if incoming.is_some() {
                    return Err(broken("a refusal inside a mod"));
                }
                if wanted.iter().any(|w| w.id == id) && !refused.iter().any(|r| r.0 == id) {
                    refused.push((id, why));
                }
            }
            Down::Done => break,
        }
    }
    if incoming.is_some() {
        return Err(broken("a mod that never ended"));
    }
    for w in wanted {
        let here = staged.iter().any(|s| s.entry.id == w.id);
        if !here && !refused.iter().any(|r| r.0 == w.id) {
            refused.push((w.id.clone(), "it was not sent".to_owned()));
        }
    }
    if !refused.is_empty() {
        let list: Vec<String> = refused
            .iter()
            .map(|(id, why)| format!("{id} ({why})"))
            .collect();
        return Err(format!("The server did not send {}.", list.join(", ")));
    }
    Ok(staged)
}

/// Whether the downloaded file at `path` is the mod `entry` names and one a
/// server may hand a client: the readers a mod on disk passes, its id and
/// version, for both sides, and no code.
fn check(path: &Path, entry: &Entry, declared: &[String]) -> Result<(), String> {
    let package = Package::read(path)?;
    let manifest = &package.manifest;
    if manifest.id != entry.id {
        return Err(format!("the file is the mod {}", manifest.id));
    }
    if manifest.version != entry.version {
        return Err(format!(
            "the file is version {}, not {}",
            manifest.version, entry.version
        ));
    }
    if package.wasm.is_some() {
        return Err("the file holds code, which is never installed from a server".to_owned());
    }
    if manifest.side != Side::Both {
        return Err("the file is not a mod for both sides".to_owned());
    }
    let own: Vec<String> = declared
        .iter()
        .filter(|item| item.split_once(':').is_some_and(|(ns, _)| ns == manifest.id))
        .cloned()
        .collect();
    Content::read_declaring(path, manifest, &own)?;
    Ok(())
}

/// Removes the unfinished downloads in `downloads`: this module's own files.
fn clean_parts(downloads: &Path) {
    for entry in fs::read_dir(downloads).into_iter().flatten().flatten() {
        if entry.file_name().to_string_lossy().ends_with(".hmod.part") {
            let _ = fs::remove_file(entry.path());
        }
    }
}

/// [`PENDING`]: the checked mods in [`DOWNLOADS`] to install.
#[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
struct Pending {
    installs: Vec<Install>,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
struct Install {
    id: String,
    version: String,
}

impl Pending {
    fn load(downloads: &Path) -> Self {
        fs::read_to_string(downloads.join(PENDING))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }
    /// Writes it, or removes the file when nothing waits.
    fn save(&self, downloads: &Path) -> Result<(), String> {
        let path = downloads.join(PENDING);
        if self.installs.is_empty() {
            if path.exists() {
                fs::remove_file(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            }
            return Ok(());
        }
        let part = downloads.join(format!("{PENDING}.part"));
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&part, text).map_err(|e| format!("{}: {e}", part.display()))?;
        fs::rename(&part, &path).map_err(|e| format!("{}: {e}", path.display()))
    }
}

/// Keeps the checked downloads as `<id>.hmod` in [`DOWNLOADS`] and notes them
/// in [`PENDING`], for [`apply_pending`].
fn record_pending(mods_dir: &Path, staged: Vec<Staged>) -> Result<(), String> {
    let downloads = mods_dir.join(DOWNLOADS);
    let mut pending = Pending::load(&downloads);
    for s in staged {
        let kept = downloads.join(format!("{}.hmod", s.entry.id));
        // An earlier download of the same mod that never got installed.
        if kept.exists() {
            fs::remove_file(&kept).map_err(|e| format!("{}: {e}", kept.display()))?;
        }
        fs::rename(&s.part, &kept).map_err(|e| format!("{}: {e}", kept.display()))?;
        pending.installs.retain(|i| i.id != s.entry.id);
        pending.installs.push(Install {
            id: s.entry.id,
            version: s.entry.version,
        });
    }
    pending.save(&downloads)
}

/// Why a waiting mod was not installed.
enum Stuck {
    /// Try again at the next start: something holds the files.
    Later(String),
    /// The download is not what was noted: it is dropped from the list.
    Never(String),
}

/// Installs the downloads waiting in `mods_dir`'s [`DOWNLOADS`] (the game's
/// `horizon-mods`): each becomes `<id>.hmod`, and a copy of that mod already
/// there, a folder or a file under any name, is moved to [`REPLACED`], never
/// deleted. Run after a download and at every start, before the mods are
/// read: what the running game held open then is free now. Returns the ids
/// that still wait.
pub fn apply_pending(mods_dir: &Path) -> Vec<String> {
    let downloads = mods_dir.join(DOWNLOADS);
    let pending = Pending::load(&downloads);
    if pending.installs.is_empty() {
        return Vec::new();
    }
    let mut left = Pending::default();
    for install in pending.installs {
        match install_one(mods_dir, &install) {
            Ok(()) => log::info!(
                "Horizon: installed mod {} {} from a server",
                install.id,
                install.version
            ),
            Err(Stuck::Later(why)) => {
                log::warn!(
                    "Horizon: mod {} waits for the next start: {why}",
                    install.id
                );
                left.installs.push(install);
            }
            Err(Stuck::Never(why)) => {
                log::warn!("Horizon: downloaded mod {:?} not installed: {why}", install.id);
            }
        }
    }
    if let Err(e) = left.save(&downloads) {
        log::warn!("Horizon: {e}");
    }
    left.installs.into_iter().map(|i| i.id).collect()
}

fn install_one(mods_dir: &Path, install: &Install) -> Result<(), Stuck> {
    let id = install.id.as_str();
    // The note is a file anyone could have edited: its id is part of a path
    // only when it is a mod's id.
    if !mod_files::valid_id(id) {
        return Err(Stuck::Never("not a mod's id".to_owned()));
    }
    let staged = mods_dir.join(DOWNLOADS).join(format!("{id}.hmod"));
    let package = Package::read(&staged).map_err(Stuck::Never)?;
    let manifest = &package.manifest;
    if manifest.id != id
        || manifest.version != install.version
        || package.wasm.is_some()
        || manifest.side != Side::Both
    {
        return Err(Stuck::Never(
            "the file is not the mod that was checked".to_owned(),
        ));
    }
    let target = mods_dir.join(format!("{id}.hmod"));
    // The player's copies of this mod: a folder would be read before the new
    // file, and the file's own name must be free.
    let mut old: Vec<PathBuf> = horizon_mods::find(mods_dir)
        .into_iter()
        .filter(|path| Package::read(path).is_ok_and(|p| p.manifest.id == id))
        .collect();
    if fs::symlink_metadata(&target).is_ok() && !old.contains(&target) {
        old.push(target.clone());
    }
    if !old.is_empty() {
        let version = old
            .iter()
            .find_map(|path| Package::read(path).ok())
            .map_or_else(|| "unknown".to_owned(), |p| p.manifest.version);
        let aside = replaced_dir(mods_dir, id, &version);
        fs::create_dir_all(&aside)
            .map_err(|e| Stuck::Later(format!("{}: {e}", aside.display())))?;
        for path in old {
            let Some(name) = path.file_name() else {
                continue;
            };
            if let Err(e) = fs::rename(&path, aside.join(name)) {
                // Nothing moved there yet: no empty folder is left behind.
                let _ = fs::remove_dir(&aside);
                return Err(Stuck::Later(format!(
                    "{} is in use ({e})",
                    path.display()
                )));
            }
        }
    }
    fs::rename(&staged, &target)
        .map_err(|e| Stuck::Later(format!("{}: {e}", target.display())))?;
    if let Some(home) = mods_dir.parent() {
        crate::mod_registry::forget_file(home, id);
    }
    Ok(())
}

/// A new folder in [`REPLACED`] for the copies of `id` at `version`.
fn replaced_dir(mods_dir: &Path, id: &str, version: &str) -> PathBuf {
    let time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let base = format!("{id}-{}-{time}", file_safe(version));
    let replaced = mods_dir.join(REPLACED);
    let mut dir = replaced.join(&base);
    let mut n = 1;
    while dir.exists() {
        n += 1;
        dir = replaced.join(format!("{base}-{n}"));
    }
    dir
}

/// `bytes` as a player reads a size: `3.1 MB`.
pub fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_000_000.0)
}

#[cfg(test)]
mod tests;
