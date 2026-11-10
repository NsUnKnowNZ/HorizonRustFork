// A player breaks a block: with the Ultimine key held, the joined matching blocks go too.
fn ftbultimine_destroy(
    game: &mut crate::game::Game<'_>,
    pos: [i32; 3],
    state: horizon_world::registry::BlockStateId,
    canceled: &mut bool,
) {
    let _ = state;
    if !*canceled {
        crate::game::ftbultimine::destroy_block(game, pos);
    }
}
