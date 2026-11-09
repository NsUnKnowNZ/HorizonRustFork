fn camerapture_server_tick(game: &mut crate::game::Game<'_>, phase: &horizon_client::mod_api::ServerTickPhase) {
    if matches!(phase, horizon_client::mod_api::ServerTickPhase::ServerTickEnd { .. }) {
        crate::game::camerapture::server_tick(game);
        crate::game::camerapture_frames::server_tick(game);
    }
}
