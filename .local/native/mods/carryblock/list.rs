// native_hooks::Game::native_mods (a hook point): Carry Block's row in Options > Mods.
fn carryblock_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::carryblock as ca;
    mods.push(crate::game::native_hooks::NativeMod {
        id: ca::ID.into(),
        title: format!("Carry Block {}", ca::VERSION),
        description: ca::DESCRIPTION.into(),
        keys: Vec::new(),
    });
}
