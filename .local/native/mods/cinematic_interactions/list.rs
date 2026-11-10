// Cinematic Interactions' row in Options > Mods.
fn cinematic_interactions_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::cinematic_interactions as m;
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.into(),
        title: format!("Cinematic Interactions {}", m::VERSION),
        description: m::DESCRIPTION.into(),
        keys: vec![[m::SIT_KEY.into(), "Sit by a campfire".into(), m::SIT_DEFAULT.into()]],
    });
}
