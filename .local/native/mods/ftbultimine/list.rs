// FTB Ultimine's row in Options > Mods, with its key.
fn ftbultimine_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::ftbultimine as u;
    mods.push(crate::game::native_hooks::NativeMod {
        id: u::ID.into(),
        title: format!("FTB Ultimine {}", u::VERSION),
        description: u::DESCRIPTION.into(),
        keys: vec![[u::KEY.into(), "Ultimine".into(), u::DEFAULT_KEY.into()]],
    });
}
