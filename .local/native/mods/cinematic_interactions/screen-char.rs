fn cinematic_interactions_screen_char(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, c: char, canceled: &mut bool) {
    crate::game::cinematic_interactions::char_typed(game, screen, c, canceled);
}
