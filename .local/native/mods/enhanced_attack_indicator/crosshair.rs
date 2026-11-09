fn enhanced_attack_indicator_crosshair(w: i32, h: i32, attack: Option<f32>) -> core::ops::ControlFlow<Vec<GuiDraw>> {
    // Only where the crosshair shows the indicator, and when the mod has a progress.
    let (Some(_), Some(p)) = (attack, enhanced_attack_indicator::progress()) else {
        return core::ops::ControlFlow::Continue(());
    };
    let (value, full) = if p >= 2.0 { (1.0, true) } else { (p, false) };
    core::ops::ControlFlow::Break(
        crosshair_draws(w, h, Some(value), full)
            .into_iter()
            .map(crosshair_blend)
            .collect(),
    )
}
