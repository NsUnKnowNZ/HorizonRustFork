// Xaero's World Map's row in Options > Mods, with its key.
fn xaeroworldmap_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::xaeroworldmap as m;
    mods.push(crate::game::native_hooks::NativeMod {
        id: m::ID.into(),
        title: format!("Xaero's World Map {}", m::VERSION),
        description: m::DESCRIPTION.into(),
        keys: vec![[m::KEY.into(), "Open World Map".into(), m::DEFAULT_KEY.into()]],
    });
}
