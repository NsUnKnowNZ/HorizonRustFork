// Freecam's row in Options > Mods, with its keys.
fn freecam_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::freecam as freecam;
    mods.push(crate::game::native_hooks::NativeMod {
        id: freecam::ID.into(),
        title: format!("Freecam {}", freecam::VERSION),
        description: freecam::DESCRIPTION.into(),
        keys: vec![
            [
                freecam::TOGGLE.into(),
                "Toggle Freecam".into(),
                freecam::DEFAULT_TOGGLE_KEY.into(),
            ],
            [
                freecam::TRIPOD_RESET.into(),
                "Reset Tripod".into(),
                freecam::DEFAULT_RESET_KEY.into(),
            ],
        ],
    });
}
