fn skins3d_page(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::umbx_skins3d_glue::page(game, mod_id, buttons);
}
