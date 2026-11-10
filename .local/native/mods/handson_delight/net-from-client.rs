fn handson_delight_net_from_client(game: &mut crate::game::Game<'_>, player: u128, channel: &str, data: &[u8]) {
    let _ = game;
    crate::game::handson_delight::net_from_client(player, channel, data);
}
