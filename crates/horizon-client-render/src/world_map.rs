//! A world map that remembers (Xaero's World Map's): a tile of map colours per chunk the
//! player has seen, kept in region files of 32 by 32 chunks so the map grows as they explore
//! and is there again next time. Tiles are scanned as the minimap's are
//! ([`crate::minimap`]: `MapItem.update`'s column colour, height and water depth) and the
//! image is shaded as a map item is.
//!
//! A region file is `r.<x>.<z>.hmap`: `HMAP0001`, then per chunk of the region (`z * 32 + x`)
//! a byte (1: seen) followed, when seen, by 256 colours, 256 little-endian heights and 256
//! depths. A file that doesn't read is left out (the area shows as unexplored).
use crate::{
    blocks::BlockFacts,
    fast_hash::{FxHashMap, FxHashSet},
    minimap::{self, Layer, Palette, WALL},
    world::World,
};
use std::{
    io,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

const MAGIC: &[u8; 8] = b"HMAP0001";
/// A chunk rescanned at most this often after the world changed.
const RESCAN: Duration = Duration::from_secs(2);
/// An unexplored pixel, and the space round the map.
pub const UNEXPLORED: [u8; 4] = [16, 16, 18, 255];

#[derive(Clone, PartialEq)]
struct Tile {
    color: [u8; 256],
    height: [i16; 256],
    depth: [u8; 256],
}

#[derive(Default)]
pub struct WorldMap {
    dir: Option<PathBuf>,
    tiles: FxHashMap<(i32, i32), Tile>,
    /// When each chunk was last scanned this session, and the world's revision then.
    scanned: FxHashMap<(i32, i32), (u64, Instant)>,
    loaded: FxHashSet<(i32, i32)>,
    dirty: FxHashSet<(i32, i32)>,
    palette: Option<Palette>,
    /// Goes up whenever a tile changes: an image composed before is out of date.
    pub revision: u64,
}

fn region(chunk: (i32, i32)) -> (i32, i32) {
    (chunk.0 >> 5, chunk.1 >> 5)
}

impl WorldMap {
    /// The folder the map is kept in (one per world and dimension), None when nothing is open.
    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }
    /// Keeps what changed and starts on the map in `dir` (another world or dimension), or
    /// none. The palette is kept.
    pub fn open(&mut self, dir: Option<PathBuf>) -> io::Result<()> {
        let saved = self.save();
        let palette = self.palette.take();
        *self = Self { dir, palette, revision: self.revision + 1, ..Self::default() };
        saved
    }
    /// The palette was made for other resources: it is made again.
    pub fn forget_palette(&mut self) {
        self.palette = None;
    }
    /// Writes the regions that changed since the last save.
    pub fn save(&mut self) -> io::Result<()> {
        let Some(dir) = self.dir.clone() else {
            self.dirty.clear();
            return Ok(());
        };
        if self.dirty.is_empty() {
            return Ok(());
        }
        std::fs::create_dir_all(&dir)?;
        for r in std::mem::take(&mut self.dirty) {
            let mut out = Vec::with_capacity(8 + 1024);
            out.extend_from_slice(MAGIC);
            for z in 0..32 {
                for x in 0..32 {
                    match self.tiles.get(&(r.0 * 32 + x, r.1 * 32 + z)) {
                        Some(t) => {
                            out.push(1);
                            out.extend_from_slice(&t.color);
                            for h in t.height {
                                out.extend_from_slice(&h.to_le_bytes());
                            }
                            out.extend_from_slice(&t.depth);
                        }
                        None => out.push(0),
                    }
                }
            }
            let path = dir.join(format!("r.{}.{}.hmap", r.0, r.1));
            let tmp = path.with_extension("hmap.tmp");
            std::fs::write(&tmp, &out)?;
            std::fs::rename(&tmp, &path)?;
        }
        Ok(())
    }
    /// Reads region `r` from its file, once.
    fn load_region(&mut self, r: (i32, i32)) {
        if !self.loaded.insert(r) {
            return;
        }
        let Some(dir) = &self.dir else {
            return;
        };
        let Ok(bytes) = std::fs::read(dir.join(format!("r.{}.{}.hmap", r.0, r.1))) else {
            return;
        };
        if bytes.len() < 8 || &bytes[..8] != MAGIC {
            return;
        }
        let mut at = 8;
        for z in 0..32 {
            for x in 0..32 {
                let Some(&seen) = bytes.get(at) else {
                    return;
                };
                at += 1;
                if seen == 0 {
                    continue;
                }
                let Some(body) = bytes.get(at..at + 1024) else {
                    return;
                };
                at += 1024;
                let mut t = Tile { color: [0; 256], height: [0; 256], depth: [0; 256] };
                t.color.copy_from_slice(&body[..256]);
                for i in 0..256 {
                    t.height[i] = i16::from_le_bytes([body[256 + i * 2], body[257 + i * 2]]);
                }
                t.depth.copy_from_slice(&body[768..1024]);
                // A chunk scanned this session keeps its newer tile.
                self.tiles.entry((r.0 * 32 + x, r.1 * 32 + z)).or_insert(t);
            }
        }
        self.revision += 1;
    }
    /// Scans the loaded chunks within `radius` of chunk `center`, nearest first, that this
    /// session hasn't seen or that changed since, until `budget` is spent. Returns how many.
    pub fn update(
        &mut self,
        world: &World,
        facts: &BlockFacts,
        center: (i32, i32),
        radius: i32,
        layer: Layer,
        budget: Duration,
    ) -> usize {
        if self.dir.is_none() {
            return 0;
        }
        let palette = self.palette.get_or_insert_with(|| Palette::new(facts));
        let started = Instant::now();
        let air = world.air.0;
        let mut scanned = 0;
        let mut regions = Vec::new();
        for ring in 0..=radius {
            for (dx, dz) in minimap::ring_offsets(ring) {
                let key = (center.0 + dx, center.1 + dz);
                let Some(chunk) = world.chunks.get(&key) else {
                    continue;
                };
                let stale = match self.scanned.get(&key) {
                    None => true,
                    Some(&(rev, at)) => rev != world.revision && at.elapsed() >= RESCAN,
                };
                if !stale {
                    continue;
                }
                self.scanned.insert(key, (world.revision, Instant::now()));
                let (color, height, depth) = minimap::scan_columns(chunk, palette, air, layer);
                // An empty chunk (not sent yet, or all air) keeps what was seen before.
                if color.iter().all(|&c| c == 0) {
                    continue;
                }
                let tile = Tile { color, height, depth };
                if self.tiles.get(&key) != Some(&tile) {
                    self.tiles.insert(key, tile);
                    self.dirty.insert(region(key));
                    regions.push(region(key));
                    self.revision += 1;
                }
                scanned += 1;
                if started.elapsed() >= budget {
                    break;
                }
            }
            if started.elapsed() >= budget {
                break;
            }
        }
        // A region first seen this session: its file's other chunks join the map.
        for r in regions {
            self.load_region(r);
        }
        scanned
    }
    /// The map as `width` x `height` RGBA, centred on block (`x`, `z`), `blocks_per_pixel`
    /// blocks to a pixel, north up. Unexplored land is [`UNEXPLORED`].
    pub fn compose(&mut self, x: f64, z: f64, blocks_per_pixel: f64, width: usize, height: usize) -> Vec<u8> {
        let bpp = blocks_per_pixel.max(1.0 / 16.0);
        let (x0, z0) = (x - width as f64 / 2.0 * bpp, z - height as f64 / 2.0 * bpp);
        let (x1, z1) = (x0 + width as f64 * bpp, z0 + height as f64 * bpp);
        let (r0, r1) = (((x0.floor() as i32) >> 9, (z0.floor() as i32) >> 9), ((x1.ceil() as i32) >> 9, (z1.ceil() as i32) >> 9));
        for rx in r0.0..=r1.0 {
            for rz in r0.1..=r1.1 {
                self.load_region((rx, rz));
            }
        }
        let mut out = vec![0u8; width * height * 4];
        let mut cached: Option<((i32, i32), &Tile)> = None;
        let tiles = &self.tiles;
        let mut column = |bx: i32, bz: i32| -> Option<(u8, i16, u8)> {
            let key = (bx >> 4, bz >> 4);
            let t = match cached {
                Some((k, t)) if k == key => t,
                _ => {
                    let t = tiles.get(&key)?;
                    cached = Some((key, t));
                    t
                }
            };
            let i = ((bz & 15) * 16 + (bx & 15)) as usize;
            Some((t.color[i], t.height[i], t.depth[i]))
        };
        for j in 0..height {
            let bz = (z0 + (j as f64 + 0.5) * bpp).floor() as i32;
            for i in 0..width {
                let bx = (x0 + (i as f64 + 0.5) * bpp).floor() as i32;
                let rgba = match column(bx, bz) {
                    Some((color, h, depth)) if color & !WALL != 0 => {
                        let north = column(bx, bz - 1).filter(|c| c.0 & !WALL != 0).map(|c| c.1);
                        minimap::packed_rgba(color & !WALL, minimap::brightness(color, h, north, depth, bx + bz))
                    }
                    _ => UNEXPLORED,
                };
                let o = (j * width + i) * 4;
                out[o..o + 4].copy_from_slice(&rgba);
            }
        }
        out
    }
    /// The chunks the map has seen (this session's and the loaded regions').
    pub fn tile_count(&self) -> usize {
        self.tiles.len()
    }
    /// The height the map has at block (`x`, `z`), when it has seen it.
    pub fn height_at(&self, x: i32, z: i32) -> Option<i16> {
        let t = self.tiles.get(&(x >> 4, z >> 4))?;
        Some(t.height[((z & 15) * 16 + (x & 15)) as usize])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_region_reads_back_as_it_was_written() {
        let dir = std::env::temp_dir().join(format!("horizon-world-map-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut map = WorldMap::default();
        map.open(Some(dir.clone())).unwrap();
        let mut t = Tile { color: [0; 256], height: [0; 256], depth: [0; 256] };
        t.color[5] = 12;
        t.height[5] = -42;
        t.depth[5] = 3;
        map.tiles.insert((-1, 33), t.clone());
        map.dirty.insert(region((-1, 33)));
        map.save().unwrap();
        let mut again = WorldMap::default();
        again.open(Some(dir.clone())).unwrap();
        again.load_region(region((-1, 33)));
        assert!(again.tiles.get(&(-1, 33)) == Some(&t));
        assert_eq!(again.height_at(-16 + 5, 33 * 16), Some(-42));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
