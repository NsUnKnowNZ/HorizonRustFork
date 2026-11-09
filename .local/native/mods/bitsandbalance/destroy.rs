fn bitsandbalance_destroy(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    state: horizon_world::registry::BlockStateId,
    canceled: &mut bool,
) {
    crate::game::bitsandbalance::destroyed(game, pos, state, canceled);
}
