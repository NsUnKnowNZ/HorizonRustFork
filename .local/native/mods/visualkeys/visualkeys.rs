// VisualKeys for Horizon: a keyboard of the key bindings on the Key Binds page.
use super::Game;
use horizon_client::hud::HudBatch;
use horizon_client::mod_api::NativeScreenView;
use winit::keyboard::KeyCode;

/// A key: its label, its binding name (`key.keyboard.q`) and its width in key units.
type Key = (&'static str, &'static str, f32);

const ROWS: [&[Key]; 6] = [
    &[
        ("Esc", "key.keyboard.escape", 1.0), ("", "", 0.5), ("F1", "key.keyboard.f1", 1.0), ("F2", "key.keyboard.f2", 1.0),
        ("F3", "key.keyboard.f3", 1.0), ("F4", "key.keyboard.f4", 1.0), ("", "", 0.5), ("F5", "key.keyboard.f5", 1.0),
        ("F6", "key.keyboard.f6", 1.0), ("F7", "key.keyboard.f7", 1.0), ("F8", "key.keyboard.f8", 1.0), ("", "", 0.5),
        ("F9", "key.keyboard.f9", 1.0), ("F10", "key.keyboard.f10", 1.0), ("F11", "key.keyboard.f11", 1.0), ("F12", "key.keyboard.f12", 1.0),
    ],
    &[
        ("`", "key.keyboard.grave.accent", 1.0), ("1", "key.keyboard.1", 1.0), ("2", "key.keyboard.2", 1.0), ("3", "key.keyboard.3", 1.0),
        ("4", "key.keyboard.4", 1.0), ("5", "key.keyboard.5", 1.0), ("6", "key.keyboard.6", 1.0), ("7", "key.keyboard.7", 1.0),
        ("8", "key.keyboard.8", 1.0), ("9", "key.keyboard.9", 1.0), ("0", "key.keyboard.0", 1.0), ("-", "key.keyboard.minus", 1.0),
        ("=", "key.keyboard.equal", 1.0), ("Back", "key.keyboard.backspace", 2.0),
    ],
    &[
        ("Tab", "key.keyboard.tab", 1.5), ("Q", "key.keyboard.q", 1.0), ("W", "key.keyboard.w", 1.0), ("E", "key.keyboard.e", 1.0),
        ("R", "key.keyboard.r", 1.0), ("T", "key.keyboard.t", 1.0), ("Y", "key.keyboard.y", 1.0), ("U", "key.keyboard.u", 1.0),
        ("I", "key.keyboard.i", 1.0), ("O", "key.keyboard.o", 1.0), ("P", "key.keyboard.p", 1.0), ("[", "key.keyboard.left.bracket", 1.0),
        ("]", "key.keyboard.right.bracket", 1.0), ("\\", "key.keyboard.backslash", 1.5),
    ],
    &[
        ("Caps", "key.keyboard.caps.lock", 1.75), ("A", "key.keyboard.a", 1.0), ("S", "key.keyboard.s", 1.0), ("D", "key.keyboard.d", 1.0),
        ("F", "key.keyboard.f", 1.0), ("G", "key.keyboard.g", 1.0), ("H", "key.keyboard.h", 1.0), ("J", "key.keyboard.j", 1.0),
        ("K", "key.keyboard.k", 1.0), ("L", "key.keyboard.l", 1.0), (";", "key.keyboard.semicolon", 1.0), ("'", "key.keyboard.apostrophe", 1.0),
        ("Enter", "key.keyboard.enter", 2.25),
    ],
    &[
        ("Shift", "key.keyboard.left.shift", 2.25), ("Z", "key.keyboard.z", 1.0), ("X", "key.keyboard.x", 1.0), ("C", "key.keyboard.c", 1.0),
        ("V", "key.keyboard.v", 1.0), ("B", "key.keyboard.b", 1.0), ("N", "key.keyboard.n", 1.0), ("M", "key.keyboard.m", 1.0),
        (",", "key.keyboard.comma", 1.0), (".", "key.keyboard.period", 1.0), ("/", "key.keyboard.slash", 1.0), ("Shift", "key.keyboard.right.shift", 2.75),
    ],
    &[
        ("Ctrl", "key.keyboard.left.control", 1.5), ("Win", "key.keyboard.left.win", 1.25), ("Alt", "key.keyboard.left.alt", 1.25),
        ("Space", "key.keyboard.space", 6.25), ("Alt", "key.keyboard.right.alt", 1.25), ("Win", "key.keyboard.right.win", 1.25),
        ("Menu", "key.keyboard.menu", 1.0), ("Ctrl", "key.keyboard.right.control", 1.5),
    ],
];

/// The mouse row drawn beside the keyboard.
const MOUSE: [Key; 5] = [
    ("LMB", "key.mouse.left", 1.5), ("MMB", "key.mouse.middle", 1.5), ("RMB", "key.mouse.right", 1.5),
    ("M4", "key.mouse.4", 1.5), ("M5", "key.mouse.5", 1.5),
];

pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if screen.id != "options"
        || game.ui.options.page() != horizon_client::options_ui::Page::KeyBinds
    {
        return;
    }
    let atlas = &game.gui.atlas;
    let held = game.physical_keys.contains(&KeyCode::AltLeft);
    if !held {
        batch.text(atlas, "Hold Left Alt: keyboard (VisualKeys)", 4.0, batch.height - 12.0, 0xFFA0_A0A0, true);
        return;
    }
    let unit = ((batch.width - 40.0) / 16.5).clamp(14.0, 26.0).floor();
    let gap = 2.0;
    let total_w = unit * 16.5;
    let total_h = unit * 7.6;
    let x0 = ((batch.width - total_w) / 2.0).round();
    let y0 = ((batch.height - total_h) / 2.0).round();
    batch.fill(atlas, x0 - 8.0, y0 - 8.0, total_w + 16.0, total_h + 16.0, 0xE010_1014);
    let mut hovered: Option<(String, Vec<&'static str>)> = None;
    let [mx, my] = screen.mouse;
    let mut draw_key = |batch: &mut HudBatch, x: f32, y: f32, w: f32, label: &str, name: &str| {
        if name.is_empty() {
            return;
        }
        let actions: Vec<&'static str> = game.settings.keys.mappings_for(name).collect();
        let color = match actions.len() {
            0 => 0xFF3A_3A40,
            1 => 0xFF2E_7D32,
            _ => 0xFF1C_1CB7,
        };
        let over = mx >= x && mx < x + w && my >= y && my < y + unit - gap;
        batch.fill(atlas, x, y, w, unit - gap, if over { 0xFF60_6070 } else { color });
        let tw = atlas.font.width(label);
        batch.text(atlas, label, (x + (w - tw) / 2.0).round(), (y + (unit - gap - 8.0) / 2.0).round(), 0xFFFF_FFFF, true);
        if over {
            hovered = Some((label.to_owned(), actions));
        }
    };
    let mut y = y0;
    for row in ROWS {
        let mut x = x0;
        for &(label, name, w) in row {
            draw_key(batch, x, y, w * unit - gap, label, name);
            x += w * unit;
        }
        y += unit * if std::ptr::eq(row, ROWS[0]) { 1.3 } else { 1.0 };
    }
    let mut x = x0;
    y += unit * 0.3;
    for &(label, name, w) in &MOUSE {
        draw_key(batch, x, y, w * unit - gap, label, name);
        x += w * unit;
    }
    if let Some((label, actions)) = hovered {
        let lines: Vec<String> = if actions.is_empty() {
            vec![format!("{label}: not bound")]
        } else {
            actions.iter().map(|a| horizon_client::lang::get_or(a, a)).collect()
        };
        let w = lines.iter().map(|l| atlas.font.width(l)).fold(0.0f32, f32::max) + 8.0;
        let (tx, ty) = ((mx + 10.0).min(batch.width - w - 2.0), my + 10.0);
        batch.fill(atlas, tx, ty, w, 10.0 * lines.len() as f32 + 6.0, 0xF020_1028);
        for (i, l) in lines.iter().enumerate() {
            batch.text(atlas, l, tx + 4.0, ty + 4.0 + 10.0 * i as f32, 0xFFFF_FFFF, false);
        }
    }
}
