fn smoothie_capes_settings_click(game: &crate::game::Game<'_>, mod_id: &str, button: usize) {
    if mod_id == "smoothie_capes" && button == 0 {
        crate::game::smoothie_elytra_config::toggle(game);
    }
}
