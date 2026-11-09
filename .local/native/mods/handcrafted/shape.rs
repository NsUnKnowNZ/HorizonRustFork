fn handcrafted_shape(
    level: &crate::game::GameLevel<'_>,
    pos: horizon_core::position::BlockPos,
    state: horizon_world::registry::BlockStateId,
    neighbour_pos: horizon_core::position::BlockPos,
    neighbour: horizon_world::registry::BlockStateId,
    new: &mut Option<horizon_world::registry::BlockStateId>,
) {
    crate::game::handcrafted::update_shape(level, pos, state, neighbour_pos, neighbour, new);
}
