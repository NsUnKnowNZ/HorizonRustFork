// Name Pain: a named mob's name in a colour for the health left.
use super::*;
use horizon_client::text::Text;
use std::collections::HashMap;
use std::sync::Mutex;

/// Per mob: the name as the server sent it, and the coloured one shown.
static NAMES: Mutex<Option<HashMap<u32, (Text, String)>>> = Mutex::new(None);

/// White, yellow, gold, red, dark red as the health falls.
fn colour(ratio: f32) -> char {
    match ratio {
        r if r > 0.8 => 'f',
        r if r > 0.6 => 'e',
        r if r > 0.4 => '6',
        r if r > 0.2 => 'c',
        _ => '4',
    }
}

pub(super) fn frame(game: &mut Game<'_>) {
    let Ok(mut guard) = NAMES.lock() else {
        return;
    };
    let names = guard.get_or_insert_with(HashMap::new);
    let mut kept = HashMap::with_capacity(names.len());
    for m in game.mobs.iter_mut() {
        let Some(current) = m
            .custom_name_text
            .clone()
            .or_else(|| m.custom_name.as_deref().map(Text::literal))
        else {
            continue;
        };
        let shown = current.plain();
        // The server's own name: the one kept, unless it sent a new one since.
        let original = match names.remove(&m.id) {
            Some((original, applied)) if applied == shown => original,
            _ => current,
        };
        let max = m.kind.max_health.max(1.0);
        let ratio = (m.health / max).clamp(0.0, 1.0);
        if ratio >= 1.0 || !m.alive() {
            m.custom_name_text = Some(original);
            continue;
        }
        let applied = format!("§{}{}", colour(ratio), original.plain());
        m.custom_name_text = Some(Text::literal(applied.clone()));
        kept.insert(m.id, (original, applied));
    }
    *names = kept;
}
