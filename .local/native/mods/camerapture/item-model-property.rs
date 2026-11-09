fn camerapture_item_model_property(property: &str, stack: &horizon_assets::item_model::StackView, value: &mut bool) {
    crate::game::camerapture_client::item_model_property(property, stack, value);
}
