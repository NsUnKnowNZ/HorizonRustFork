fn jade_screen_key(game: &crate::game::Game<'_>, screen: &crate::game::native_hooks::NativeScreenView<'_>, key: &str, repeat: bool, modifiers: u32, canceled: &mut bool) {
    crate::game::jade_config::key(game, screen, key, repeat, modifiers, canceled);
}
