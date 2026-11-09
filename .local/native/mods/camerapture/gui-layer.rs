fn camerapture_gui_layer(game: &crate::game::Game<'_>, canceled: &mut bool) {
    crate::game::camerapture_client::gui_layer(game, canceled);
}
