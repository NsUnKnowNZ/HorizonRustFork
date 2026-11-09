fn damage_numbers_world(
    game: &crate::game::Game<'_>,
    partial: f32,
    camera: [f64; 3],
    primitives: &mut horizon_client::world_primitives::WorldPrimitives,
) {
    crate::game::damage_numbers::frame(game, partial, camera, primitives);
}
