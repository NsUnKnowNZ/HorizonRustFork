fn jade_screen_typed(game: &crate::game::Game<'_>, screen: &crate::game::native_hooks::NativeScreenView<'_>, c: char, canceled: &mut bool) {
    crate::game::jade_config::typed(game, screen, c, canceled);
}
