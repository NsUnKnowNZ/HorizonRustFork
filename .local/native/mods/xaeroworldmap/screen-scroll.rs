fn xaeroworldmap_screen_scroll(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, scroll: [f64; 2], canceled: &mut bool) {
    crate::game::xaeroworldmap::screen_scroll(game, screen, scroll, canceled);
}
