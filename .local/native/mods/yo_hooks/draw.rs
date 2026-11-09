fn yo_hooks_draw(
    game: &crate::game::Game<'_>,
    partial: f32,
    primitives: &mut horizon_client::world_primitives::WorldPrimitives,
) {
    crate::game::yo_hooks::draw(game, partial, primitives);
}
