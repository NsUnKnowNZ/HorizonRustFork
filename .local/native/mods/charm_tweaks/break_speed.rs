fn charm_tweaks_break_speed(game: &crate::game::Game<'_>, original: f32, speed: &mut f32) {
    crate::game::charm_tweaks::aerial_affinity(game, original, speed);
}
