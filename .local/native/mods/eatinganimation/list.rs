fn official_eatinganimation_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    mods.push(crate::game::native_hooks::NativeMod {
        id: "eatinganimation".into(),
        title: "Eating Animation 0.2.0".into(),
        description:
            "Bitten frames for the food and potion in your hand while you eat or drink it.".into(),
        keys: Vec::new(),
    });
}
