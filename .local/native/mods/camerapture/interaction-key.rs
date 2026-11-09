fn camerapture_interaction_key(game: &mut crate::game::Game<'_>, button: i32, swing_hand: &mut bool, canceled: &mut bool) {
    crate::game::camerapture_client::interaction_key(game, button, swing_hand, canceled);
}
