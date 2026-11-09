fn watut_world(
    game: &crate::game::Game<'_>,
    partial: f32,
    primitives: &mut horizon_client::world_primitives::WorldPrimitives,
) {
    crate::game::watut::frame(game, partial, primitives);
}
