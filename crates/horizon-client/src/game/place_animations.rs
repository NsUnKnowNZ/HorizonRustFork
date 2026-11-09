//! A Good Place (MIT), for blocks the player places: the new block settles into its spot, drawn
//! a little larger at first and shrinking to its size over a fifth of a second, over the block
//! itself (which the larger copy hides), in place of the mod's grow-in animation (the block's
//! own mesh can't be hidden while it plays). `HORIZON_PLACE_ANIMATIONS=0` turns it off.
use super::Game;
use horizon_client::mesher::Vertex;
use horizon_world::registry::BlockStateId;
use std::time::Instant;

/// How long a placement settles, in seconds, and how much larger it starts.
const LENGTH: f32 = 0.2;
const GROWTH: f32 = 0.16;

#[derive(Default)]
pub(super) struct PlaceAnimations {
    list: Vec<([i32; 3], BlockStateId, Instant)>,
}

fn enabled() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var("HORIZON_PLACE_ANIMATIONS").map_or(true, |v| v != "0"))
}

impl Game<'_> {
    /// The player placed the block at `pos`: it starts settling.
    pub(super) fn start_place_animation(&mut self, pos: [i32; 3]) {
        if !enabled() {
            return;
        }
        let state = self.world.block(pos[0], pos[1], pos[2]);
        let facts = self.res.facts().get(state);
        if facts.render_shape != horizon_client::blocks::RenderShape::Model || self.res.facts().is_air(state) {
            return;
        }
        let list = &mut self.place_animations.list;
        list.retain(|a| a.0 != pos);
        if list.len() < 32 {
            list.push((pos, state, Instant::now()));
        }
    }

    /// Each settling block, camera-relative, scaled about its centre.
    pub(super) fn place_animation_geometry(&mut self, camera: glam::DVec3, out: &mut Vec<Vertex>) {
        if self.place_animations.list.is_empty() {
            return;
        }
        let mut list = std::mem::take(&mut self.place_animations.list);
        list.retain(|(pos, state, start)| {
            start.elapsed().as_secs_f32() < LENGTH && self.world.block(pos[0], pos[1], pos[2]) == *state
        });
        for (pos, state, start) in &list {
            let t = (start.elapsed().as_secs_f32() / LENGTH).clamp(0.0, 1.0);
            let s = 1.0 + GROWTH * (1.0 - t) * (1.0 - t);
            let at = glam::DVec3::new(f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2])) - camera;
            let m = glam::Mat4::from_translation(at.as_vec3() + glam::Vec3::splat(0.5))
                * glam::Mat4::from_scale(glam::Vec3::splat(s))
                * glam::Mat4::from_translation(glam::Vec3::splat(-0.5));
            self.place_animation_block(*state, horizon_core::position::BlockPos::new(pos[0], pos[1], pos[2]), m, out);
        }
        self.place_animations.list = list;
    }

    /// `moving_block_vertices` with a whole transform: tinted and lit as the block at `seed`.
    fn place_animation_block(
        &self,
        state: BlockStateId,
        seed: horizon_core::position::BlockPos,
        m: glam::Mat4,
        out: &mut Vec<Vertex>,
    ) {
        let chunks = &self.world.chunks;
        let (cx, cz) = (seed.x >> 4, seed.z >> 4);
        let region: [[Option<&horizon_client::chunk::Chunk>; 3]; 3] = std::array::from_fn(|dz| {
            std::array::from_fn(|dx| chunks.get(&(cx + dx as i32 - 1, cz + dz as i32 - 1)).map(|c| &**c))
        });
        let tint = self.pack.context.tables.block_tints(
            state,
            [seed.x, seed.y, seed.z],
            &region,
            world_biomes!(self),
            self.streaming.biome_zoom,
            self.streaming.biome_blend,
        );
        let tables = &self.pack.context.tables;
        let light = |quad: &horizon_assets::bake::ModelQuad| {
            let coords = tables.flat_light(state, [seed.x, seed.y, seed.z], quad, |p| self.world.light(p[0], p[1], p[2]));
            (coords >> 16 & 0xF0) | ((coords & 0xF0) << 8)
        };
        self.tinted_block_model_vertices(state, horizon_core::random::position_seed(seed), m, light, true, tint, out);
    }
}
