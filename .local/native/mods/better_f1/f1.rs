fn better_f1_toggle(ui: &mut UiState) -> core::ops::ControlFlow<()> {
    use super::native_client_services::HudMode;
    ui.set_hud_mode(if ui.hide_gui { HudMode::AllVisible } else { HudMode::NoHud });
    core::ops::ControlFlow::Break(())
}
