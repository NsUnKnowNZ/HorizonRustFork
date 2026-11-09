fn forgiving_void_fall(world: &mut MotionWorld<'_, '_, '_>) {
    if forgiving_void_state::is_falling(world.player.entity_id) {
        world.player.fall_distance = 0.0;
    }
}
