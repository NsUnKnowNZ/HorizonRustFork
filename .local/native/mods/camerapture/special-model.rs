fn camerapture_special_model(game: &crate::game::Game<'_>, kind: &str, argument: &serde_json::Value, pose: glam::Mat4, sink: &mut dyn horizon_client::item_render::ItemSink) {
    crate::game::camerapture_client::special_item_model(game, kind, argument, pose, sink);
}
