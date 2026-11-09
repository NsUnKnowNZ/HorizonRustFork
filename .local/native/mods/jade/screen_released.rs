fn jade_screen_released(game: &crate::game::Game<'_>, screen: &crate::game::native_hooks::NativeScreenView<'_>, canceled: &mut bool) {
    crate::game::jade_config::released(game, screen, canceled);
}
