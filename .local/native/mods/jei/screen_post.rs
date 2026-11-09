// The bookmark bar over a container screen, after the game drew it.
fn jei_screen_post(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &crate::game::native_hooks::NativeScreenView<'_>,
) {
    crate::game::jei_glue::screen_render_post(game, batch, screen);
}
