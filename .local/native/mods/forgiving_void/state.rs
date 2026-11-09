// Forgiving Void: the players (by entity id) falling back from the sky.
use std::collections::HashSet;
use std::sync::Mutex;

static FALLING: Mutex<Option<HashSet<i32>>> = Mutex::new(None);

pub fn set_falling(id: i32, falling: bool) {
    let mut set = FALLING.lock().unwrap_or_else(|e| e.into_inner());
    let set = set.get_or_insert_with(HashSet::new);
    if falling { set.insert(id); } else { set.remove(&id); }
}

pub fn is_falling(id: i32) -> bool {
    FALLING
        .lock()
        .map(|s| s.as_ref().is_some_and(|s| s.contains(&id)))
        .unwrap_or(false)
}
