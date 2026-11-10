fn farmersdelight_place(game: &mut crate::game::Game<'_>, pos: [i32; 3], state: horizon_world::registry::BlockStateId) {
    crate::game::farmersdelight::placed(game, pos, state);
}
