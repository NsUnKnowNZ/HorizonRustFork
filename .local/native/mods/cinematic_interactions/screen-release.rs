fn cinematic_interactions_screen_release(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, canceled: &mut bool) {
    crate::game::cinematic_interactions::release(game, screen, canceled);
}
