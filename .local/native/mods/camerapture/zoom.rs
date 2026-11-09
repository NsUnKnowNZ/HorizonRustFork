fn camerapture_zoom(game: &crate::game::Game<'_>, zoom: &mut Option<horizon_client::mod_api::NativeZoom>) {
    crate::game::camerapture_client::zoom(game, zoom);
}
