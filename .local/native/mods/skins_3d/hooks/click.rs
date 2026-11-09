fn skins3d_click(game: &mut crate::game::Game<'_>, mod_id: &str, button: usize) {
    crate::game::umbx_skins3d_glue::click(game, mod_id, button);
}
