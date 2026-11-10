//! Content mods' files, sent by a Horizon server to a client whose mods
//! differ from its own (docs/mods.md, "Playing on a server with mods").
//!
//! In the configuration phase a server that shares its mods says so with an
//! [`Clientbound::Offer`] on [`CHANNEL`]. A client that wants some answers the
//! server's mod list (`horizon:mod_list`) with a [`Serverbound::Request`]
//! naming them instead of its own list. The server then sends each as one
//! `.hmod`: [`Clientbound::Begin`], its [`Clientbound::Chunk`]s in order and
//! [`Clientbound::End`] with the SHA-256 of the bytes, or
//! [`Clientbound::Refused`]; then [`Clientbound::Done`], and it closes the
//! connection, which never reaches play. The client acknowledges every
//! [`ACK_EVERY`] chunks and the server never runs more than [`WINDOW`] ahead.
//!
//! The ids a client asks for are only ever looked up in the server's own
//! list of loaded mods: none of them becomes a path.
use crate::{
    Error, Result,
    buf::{Reader, Writer},
};

/// The custom payload channel, both ways.
pub const CHANNEL: &str = "horizon:mod_files";
/// Bytes of a mod's file in one chunk at most: under the clientbound custom
/// payload's 1 MiB.
pub const CHUNK: usize = 256 << 10;
/// Bytes of one mod's file at most (a mod's content cap, `horizon_mods::content::MAX_TOTAL`).
pub const MAX_MOD: u64 = 512 << 20;
/// Bytes one request is sent at most, over all its mods.
pub const MAX_REQUEST: u64 = 1 << 30;
/// Mods in one request at most (a mod list's cap).
pub const MAX_IDS: usize = crate::mod_list::MAX_ENTRIES;
/// Characters in a mod's id or version at most.
pub const MAX_TEXT: usize = crate::mod_list::MAX_TEXT;
/// Characters in a refusal's reason at most.
pub const MAX_WHY: usize = 256;
/// The client acknowledges each chunk whose number plus one divides by this.
pub const ACK_EVERY: u32 = 8;
/// Chunks the server sends past the last acknowledged one at most.
pub const WINDOW: u32 = 16;

/// What the server sends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Clientbound {
    /// The server shares its mods' files: up to `max_total` bytes a request.
    Offer { max_total: u64 },
    /// A mod's file follows: `size` bytes in `chunks` chunks.
    Begin {
        id: String,
        version: String,
        size: u64,
        chunks: u32,
    },
    /// The next [`CHUNK`] bytes (fewer in the last one), numbered from 0 for each mod.
    Chunk { seq: u32, bytes: Vec<u8> },
    /// The mod's file is whole: the SHA-256 of its bytes.
    End { sha256: [u8; 32] },
    /// The server does not send this mod.
    Refused { id: String, why: String },
    /// Every requested mod was sent or refused.
    Done,
}

/// What the client sends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Serverbound {
    /// The mods wanted, by id.
    Request { ids: Vec<String> },
    /// Every chunk up to `seq` of the mod being sent has arrived.
    Ack { seq: u32 },
    /// Stop sending.
    Cancel,
}

/// The chunks a file of `size` bytes takes.
pub fn chunks_for(size: u64) -> u32 {
    size.div_ceil(CHUNK as u64) as u32
}

/// Whether `id` is a mod's id (`[a-z0-9_]`, 1 to 64 characters), the only
/// ids a request may name.
pub fn valid_id(id: &str) -> bool {
    (1..=MAX_TEXT).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn cut(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn size(r: &mut Reader, max: u64) -> Result<u64> {
    let size = r.i64()?;
    match u64::try_from(size) {
        Ok(size) if size <= max => Ok(size),
        _ => Err(Error::new(format!("mod files: size {size}"))),
    }
}

fn finished(r: &Reader) -> Result<()> {
    match r.remaining() {
        0 => Ok(()),
        _ => Err(Error::new("mod files: trailing bytes")),
    }
}

impl Clientbound {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        match self {
            Self::Offer { max_total } => {
                w.u8(0);
                w.i64((*max_total).min(i64::MAX as u64) as i64);
            }
            Self::Begin {
                id,
                version,
                size,
                chunks,
            } => {
                w.u8(1);
                w.string(&cut(id, MAX_TEXT));
                w.string(&cut(version, MAX_TEXT));
                w.i64((*size).min(i64::MAX as u64) as i64);
                w.u32(*chunks);
            }
            Self::Chunk { seq, bytes } => {
                w.u8(2);
                w.u32(*seq);
                w.bytes(bytes);
            }
            Self::End { sha256 } => {
                w.u8(3);
                w.bytes(sha256);
            }
            Self::Refused { id, why } => {
                w.u8(4);
                w.string(&cut(id, MAX_TEXT));
                w.string(&cut(why, MAX_WHY));
            }
            Self::Done => w.u8(5),
        }
        w.data
    }

    /// A payload's message; an error for one that breaks a cap.
    pub fn decode(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let message = match r.u8()? {
            0 => Self::Offer {
                max_total: size(&mut r, i64::MAX as u64)?,
            },
            1 => {
                let id = r.string(MAX_TEXT)?;
                let version = r.string(MAX_TEXT)?;
                let size = size(&mut r, MAX_MOD)?;
                let chunks = r.u32()?;
                if !valid_id(&id) {
                    return Err(Error::new("mod files: not a mod id"));
                }
                if chunks != chunks_for(size) {
                    return Err(Error::new(format!(
                        "mod files: {chunks} chunks for {size} bytes"
                    )));
                }
                Self::Begin {
                    id,
                    version,
                    size,
                    chunks,
                }
            }
            2 => {
                let seq = r.u32()?;
                let bytes = r.rest();
                if bytes.is_empty() || bytes.len() > CHUNK {
                    return Err(Error::new(format!(
                        "mod files: a chunk of {} bytes",
                        bytes.len()
                    )));
                }
                Self::Chunk {
                    seq,
                    bytes: bytes.to_vec(),
                }
            }
            3 => Self::End {
                sha256: r.bytes(32)?.try_into().expect("32 bytes"),
            },
            4 => Self::Refused {
                id: r.string(MAX_TEXT)?,
                why: r.string(MAX_WHY)?,
            },
            5 => Self::Done,
            kind => return Err(Error::new(format!("mod files: message {kind}"))),
        };
        finished(&r)?;
        Ok(message)
    }
}

impl Serverbound {
    /// The message as a payload. A request names at most [`MAX_IDS`] mods,
    /// which fits a serverbound payload's 32767 bytes.
    pub fn encode(&self) -> Vec<u8> {
        let mut w = Writer::new();
        match self {
            Self::Request { ids } => {
                let ids = &ids[..ids.len().min(MAX_IDS)];
                w.u8(0);
                w.varint(ids.len() as i32);
                for id in ids {
                    w.string(&cut(id, MAX_TEXT));
                }
            }
            Self::Ack { seq } => {
                w.u8(1);
                w.u32(*seq);
            }
            Self::Cancel => w.u8(2),
        }
        w.data
    }

    /// A payload's message; an error for one that breaks a cap or names
    /// something that is not a mod id.
    pub fn decode(data: &[u8]) -> Result<Self> {
        let mut r = Reader::new(data);
        let message = match r.u8()? {
            0 => {
                let count = r.varint()?;
                if !(1..=MAX_IDS as i32).contains(&count) {
                    return Err(Error::new(format!("mod files: {count} mods requested")));
                }
                let ids = (0..count)
                    .map(|_| r.string(MAX_TEXT))
                    .collect::<Result<Vec<_>>>()?;
                if !ids.iter().all(|id| valid_id(id)) {
                    return Err(Error::new("mod files: not a mod id"));
                }
                Self::Request { ids }
            }
            1 => Self::Ack { seq: r.u32()? },
            2 => Self::Cancel,
            kind => return Err(Error::new(format!("mod files: message {kind}"))),
        };
        finished(&r)?;
        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_message_round_trips() {
        let clientbound = [
            Clientbound::Offer {
                max_total: MAX_REQUEST,
            },
            Clientbound::Begin {
                id: "chipped".into(),
                version: "3.1.0".into(),
                size: CHUNK as u64 * 2 + 5,
                chunks: 3,
            },
            Clientbound::Begin {
                id: "empty".into(),
                version: "1".into(),
                size: 0,
                chunks: 0,
            },
            Clientbound::Chunk {
                seq: 7,
                bytes: vec![1, 2, 3],
            },
            Clientbound::Chunk {
                seq: u32::MAX,
                bytes: vec![9; CHUNK],
            },
            Clientbound::End { sha256: [7; 32] },
            Clientbound::Refused {
                id: "ruby".into(),
                why: "not a mod of this server".into(),
            },
            Clientbound::Done,
        ];
        for message in clientbound {
            assert_eq!(Clientbound::decode(&message.encode()).unwrap(), message);
        }
        let serverbound = [
            Serverbound::Request {
                ids: vec!["chipped".into(), "ruby_2".into()],
            },
            Serverbound::Ack { seq: 15 },
            Serverbound::Cancel,
        ];
        for message in serverbound {
            assert_eq!(Serverbound::decode(&message.encode()).unwrap(), message);
        }
    }

    #[test]
    fn the_biggest_request_fits_a_serverbound_payload() {
        let ids: Vec<String> = (0..MAX_IDS + 40)
            .map(|i| format!("{i:0>width$}", width = MAX_TEXT))
            .collect();
        let payload = Serverbound::Request { ids }.encode();
        assert!(payload.len() <= crate::quiet::MAX_PAYLOAD, "{}", payload.len());
        let Serverbound::Request { ids } = Serverbound::decode(&payload).unwrap() else {
            panic!("not a request");
        };
        assert_eq!(ids.len(), MAX_IDS);
        // And the biggest chunk a clientbound one (1 MiB).
        let chunk = Clientbound::Chunk {
            seq: 0,
            bytes: vec![0; CHUNK],
        };
        assert!(chunk.encode().len() < 1 << 20);
    }

    #[test]
    fn a_request_names_only_mod_ids() {
        let request = |ids: &[&str]| {
            let mut w = Writer::new();
            w.u8(0);
            w.varint(ids.len() as i32);
            for id in ids {
                w.string(id);
            }
            Serverbound::decode(&w.data)
        };
        assert!(request(&["ruby"]).is_ok());
        for bad in [
            "../ruby",
            "ruby/..",
            "C:\\mods",
            "Ruby",
            "ruby.hmod",
            "",
            "a b",
            "..",
        ] {
            assert!(request(&[bad]).is_err(), "{bad:?}");
            assert!(request(&["ruby", bad]).is_err(), "{bad:?}");
        }
        assert!(request(&[]).is_err());
        let too_many: Vec<&str> = vec!["a"; MAX_IDS + 1];
        assert!(request(&too_many).is_err());
        assert!(request(&["a".repeat(MAX_TEXT + 1).as_str()]).is_err());
        assert!(valid_id("ruby_2") && !valid_id("ruby-2"));
    }

    #[test]
    fn messages_over_the_caps_are_refused() {
        let begin = |size: u64, chunks: u32| {
            let mut w = Writer::new();
            w.u8(1);
            w.string("ruby");
            w.string("1");
            w.i64(size as i64);
            w.u32(chunks);
            Clientbound::decode(&w.data)
        };
        assert!(begin(MAX_MOD, chunks_for(MAX_MOD)).is_ok());
        assert!(begin(MAX_MOD + 1, chunks_for(MAX_MOD + 1)).is_err());
        assert!(begin(u64::MAX, 1).is_err());
        // The chunk count must be the size's.
        assert!(begin(CHUNK as u64, 2).is_err());
        assert!(begin(CHUNK as u64 + 1, 1).is_err());
        assert!(begin(0, 1).is_err());
        let chunk = |len: usize| {
            let mut w = Writer::new();
            w.u8(2);
            w.u32(0);
            w.bytes(&vec![0; len]);
            Clientbound::decode(&w.data)
        };
        assert!(chunk(CHUNK).is_ok());
        assert!(chunk(CHUNK + 1).is_err());
        assert!(chunk(0).is_err());
        // An id that is not a mod's never starts a file.
        let mut w = Writer::new();
        w.u8(1);
        w.string("../x");
        w.string("1");
        w.i64(0);
        w.u32(0);
        assert!(Clientbound::decode(&w.data).is_err());
        // Unknown kinds, short and trailing bytes.
        assert!(Clientbound::decode(&[]).is_err());
        assert!(Clientbound::decode(&[9]).is_err());
        assert!(Serverbound::decode(&[9]).is_err());
        assert!(Clientbound::decode(&[5, 0]).is_err());
        assert!(Serverbound::decode(&[2, 0]).is_err());
        assert!(Clientbound::decode(&[3, 1, 2]).is_err());
    }

    #[test]
    fn chunks_cover_the_size() {
        assert_eq!(chunks_for(0), 0);
        assert_eq!(chunks_for(1), 1);
        assert_eq!(chunks_for(CHUNK as u64), 1);
        assert_eq!(chunks_for(CHUNK as u64 + 1), 2);
        assert_eq!(chunks_for(MAX_MOD), 2048);
    }
}
