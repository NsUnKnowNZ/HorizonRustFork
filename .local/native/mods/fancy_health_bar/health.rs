fn fancy_health_bar_health(inputs: &horizon_client::hud_layout::HealthInputs, wi: i32, hi: i32) {
    crate::game::fancy_health_bar::observe(inputs, wi, hi);
}
