fn cinematiczoom_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::cinematiczoom as z;
    mods.push(crate::game::native_hooks::NativeMod {
        id: z::ID.into(),
        title: format!("Cinematic Zoom {}", z::VERSION),
        description: z::DESCRIPTION.into(),
        keys: vec![[z::KEY.into(), "Zoom".into(), z::DEFAULT_KEY.into()]],
    });
}
