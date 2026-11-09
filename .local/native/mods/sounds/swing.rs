fn sounds_swing(game: &mut crate::game::Game<'_>, button: i32, canceled: &mut bool) {
    if !*canceled {
        crate::game::sounds_extra::swing(game, button);
    }
}
