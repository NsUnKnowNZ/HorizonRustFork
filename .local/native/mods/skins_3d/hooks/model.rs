fn skins3d_model(game: &crate::game::Game<'_>, kind: &std::sync::Arc<horizon_client::mobs::MobType>, skin: &str, slim: bool) -> std::ops::ControlFlow<Option<std::sync::Arc<horizon_client::mobs::Part>>> {
    crate::game::umbx_skins3d_glue::model(game, kind, skin, slim)
}
