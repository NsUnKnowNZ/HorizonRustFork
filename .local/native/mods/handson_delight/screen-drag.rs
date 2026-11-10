fn handson_delight_screen_drag(game: &crate::game::Game<'_>, screen: &horizon_client::mod_api::NativeScreenView<'_>, button: i32, delta: [f32; 2], modifiers: u32, canceled: &mut bool) {
    let _ = (button, delta, modifiers);
    crate::game::handson_delight::drag(game, screen, canceled);
}
