fn smoothie_capes_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    mods.push(crate::game::native_hooks::NativeMod {
        id: "smoothie_capes".into(),
        title: "Smoothie Capes 0.5.1".into(),
        description: "Flexible capes with wind, inertia, and optional elytra animation.".into(),
        keys: Vec::new(),
    });
}
