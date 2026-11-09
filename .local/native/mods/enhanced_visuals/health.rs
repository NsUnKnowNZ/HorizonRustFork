fn enhanced_visuals_health(inputs: &horizon_client::hud_layout::HealthInputs) {
    crate::game::enhanced_visuals::observe(inputs.health, inputs.max_health);
}
