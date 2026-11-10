// Hands-On Delight's row in Options > Mods.
fn handson_delight_listed(mods: &mut Vec<crate::game::native_hooks::NativeMod>) {
    use crate::game::handson_delight as h;
    mods.push(crate::game::native_hooks::NativeMod {
        id: h::ID.into(),
        title: format!("Hands-On Delight {}", h::VERSION),
        description: h::DESCRIPTION.into(),
        keys: Vec::new(),
    });
}
