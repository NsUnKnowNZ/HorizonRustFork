fn yo_hooks_use(game: &mut crate::game::Game<'_>, button: i32, swing_hand: &mut bool, canceled: &mut bool) {
    crate::game::yo_hooks::use_key(game, button, swing_hand, canceled);
}
