fn ping_wheel_world(
    game: &crate::game::Game<'_>,
    camera: [f64; 3],
    primitives: &mut horizon_client::world_primitives::WorldPrimitives,
) {
    crate::game::ping_wheel::frame(game, camera, primitives);
}
