fn handson_delight_screen_render_post(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch, screen: &horizon_client::mod_api::NativeScreenView<'_>) {
    crate::game::handson_delight::screen_draw(game, batch, screen);
}
