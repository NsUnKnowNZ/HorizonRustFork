fn jade_screen_scrolled(game: &crate::game::Game<'_>, screen: &crate::game::native_hooks::NativeScreenView<'_>, scroll: [f64; 2], modifiers: u32, canceled: &mut bool) {
    crate::game::jade_config::scrolled(game, screen, scroll, modifiers, canceled);
}
