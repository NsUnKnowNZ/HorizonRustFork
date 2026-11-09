fn jade_screen_render_post(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch, screen: &crate::game::native_hooks::NativeScreenView<'_>) {
    crate::game::jade_config::draw(game, batch, screen);
}
