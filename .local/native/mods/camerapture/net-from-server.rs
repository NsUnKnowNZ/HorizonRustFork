fn camerapture_net_from_server(game: &mut crate::game::Game<'_>, channel: &str, data: &[u8]) {
    crate::game::camerapture_client::from_server(game, channel, data);
}
