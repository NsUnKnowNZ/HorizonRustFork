fn camerapture_wheel(game: &mut crate::game::Game<'_>, steps: f32, claimed: &mut bool) {
    crate::game::camerapture_client::wheel(game, steps, claimed);
}
