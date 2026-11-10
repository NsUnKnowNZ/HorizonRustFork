//! Crafting Tweaks (Better OneBlock): three buttons beside a crafting grid
//! (left of the inventory's 2x2; over the arrow of the crafting table's 3x3,
//! whose left side is the recipe book button's) that rotate the grid
//! clockwise, balance each item evenly over the slots holding it, and clear
//! the grid into the inventory. Each is a run of ordinary slot clicks
//! (`tweak_click`), so it works on a server as in a local world.
use super::{Game, Screen};
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::menu::Input;

const BUTTON: f32 = 9.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Rotate,
    Balance,
    Clear,
}

impl Game<'_> {
    /// The grid's slot ids, row by row, and its side.
    fn crafting_tweaks_grid(&self) -> Option<(Vec<usize>, usize)> {
        if self.screen != Screen::Inventory
            || self.station.is_some()
            || self.mount_active()
            || self.creative_inventory_open()
            || self.carried.is_some()
        {
            return None;
        }
        let size = self.craft_size;
        if !(2..=3).contains(&size) {
            return None;
        }
        let grid: Vec<usize> = (100..100 + size * size).collect();
        let slots = self.slots();
        grid.iter().all(|s| slots.contains(s)).then_some((grid, size))
    }
    /// The buttons, relative to the screen's origin.
    fn crafting_tweaks_buttons(&self) -> Vec<((f32, f32), Action)> {
        let Some((grid, size)) = self.crafting_tweaks_grid() else {
            return Vec::new();
        };
        let (gx, gy) = self.slot_position(grid[0]);
        [Action::Rotate, Action::Balance, Action::Clear]
            .into_iter()
            .enumerate()
            .map(|(k, a)| {
                let step = k as f32 * (BUTTON + 2.0);
                // The crafting table's recipe book button sits left of its grid: there the
                // buttons go in a row right of the grid's top, over the arrow.
                let at = if size == 3 {
                    (gx + 3.0 * 18.0 + 6.0 + step, gy)
                } else {
                    (gx - BUTTON - 3.0, gy + step)
                };
                (at, a)
            })
            .collect()
    }
    pub(super) fn draw_crafting_tweaks(&self, batch: &mut HudBatch, ox: f32, oy: f32) {
        let atlas = &self.gui.atlas;
        let (cx, cy) = self.cursor;
        let dark = rgba(55, 55, 55, 255);
        for ((x, y), action) in self.crafting_tweaks_buttons() {
            let (x, y) = (ox + x, oy + y);
            let hovered = cx >= x && cx < x + BUTTON && cy >= y && cy < y + BUTTON;
            let face = if hovered {
                rgba(220, 220, 220, 255)
            } else {
                rgba(160, 160, 160, 255)
            };
            batch.fill(atlas, x, y, BUTTON, BUTTON, dark);
            batch.fill(atlas, x + 1.0, y + 1.0, BUTTON - 2.0, BUTTON - 2.0, face);
            // Pixel icons: a turning arrow, two even bars, a cross.
            let px = |batch: &mut HudBatch, dx: f32, dy: f32, w: f32, h: f32| {
                batch.fill(atlas, x + dx, y + dy, w, h, dark);
            };
            match action {
                Action::Rotate => {
                    px(batch, 2.0, 2.0, 4.0, 1.0);
                    px(batch, 6.0, 2.0, 1.0, 4.0);
                    px(batch, 2.0, 6.0, 5.0, 1.0);
                    px(batch, 2.0, 4.0, 1.0, 2.0);
                    px(batch, 1.0, 4.0, 3.0, 1.0);
                }
                Action::Balance => {
                    px(batch, 2.0, 3.0, 5.0, 1.0);
                    px(batch, 2.0, 5.0, 5.0, 1.0);
                }
                Action::Clear => {
                    for i in 0..5 {
                        let i = i as f32;
                        px(batch, 2.0 + i, 2.0 + i, 1.0, 1.0);
                        px(batch, 6.0 - i, 2.0 + i, 1.0, 1.0);
                    }
                }
            }
        }
    }
    /// A left press on a Crafting Tweaks button runs it (true).
    pub(super) fn crafting_tweaks_press(&mut self) -> bool {
        let Some(r) = self.renderer.as_ref() else {
            return false;
        };
        let batch = HudBatch::new([r.config.width, r.config.height]);
        let (ox, oy) = self.inventory_origin(&batch);
        let (cx, cy) = self.cursor;
        let hit = self.crafting_tweaks_buttons().into_iter().find(|((x, y), _)| {
            let (x, y) = (ox + x, oy + y);
            cx >= x && cx < x + BUTTON && cy >= y && cy < y + BUTTON
        });
        let (Some((_, action)), Some((grid, size))) = (hit, self.crafting_tweaks_grid()) else {
            return false;
        };
        match action {
            Action::Rotate => self.crafting_tweaks_rotate(&grid, size),
            Action::Balance => self.crafting_tweaks_balance(&grid),
            Action::Clear => {
                for &s in &grid {
                    if self.slot_view(s).is_some() {
                        self.tweak_click(s, 0, Input::QuickMove);
                    }
                }
            }
        }
        true
    }
    /// Clockwise: each ring's stacks move one step on, by picking up the
    /// first and clicking round the ring (a click swaps the hand with the
    /// slot). Two neighbouring stacks of the same item would merge instead,
    /// so a ring like that is left as it is.
    fn crafting_tweaks_rotate(&mut self, grid: &[usize], size: usize) {
        let rings: Vec<Vec<usize>> = if size == 2 {
            vec![vec![0, 1, 3, 2]]
        } else {
            vec![vec![0, 2, 8, 6], vec![1, 5, 7, 3]]
        };
        for ring in rings {
            let stacks: Vec<_> = ring.iter().map(|&i| self.slot_view(grid[i])).collect();
            let merges = (0..ring.len()).any(|k| {
                let (a, b) = (&stacks[k], &stacks[(k + 1) % ring.len()]);
                matches!((a, b), (Some(a), Some(b)) if a.item == b.item)
            });
            if merges || stacks.iter().all(Option::is_none) {
                continue;
            }
            for k in 0..=ring.len() {
                self.tweak_click(grid[ring[k % ring.len()]], 0, Input::Pickup);
            }
        }
    }
    /// Each item spread evenly over the grid slots that hold it: take up a
    /// slot's surplus and put it down one at a time (right click) where
    /// there is too little, then put the rest back.
    fn crafting_tweaks_balance(&mut self, grid: &[usize]) {
        let stacks: Vec<_> = grid.iter().map(|&s| self.slot_view(s)).collect();
        let mut kinds: Vec<std::sync::Arc<str>> = Vec::new();
        for s in stacks.iter().flatten() {
            if !kinds.contains(&s.item) {
                kinds.push(s.item.clone());
            }
        }
        for kind in kinds {
            let held: Vec<(usize, u32)> = grid
                .iter()
                .zip(&stacks)
                .filter_map(|(&slot, s)| {
                    s.as_ref().filter(|s| s.item == kind).map(|s| (slot, s.count))
                })
                .collect();
            if held.len() < 2 {
                continue;
            }
            let total: u32 = held.iter().map(|(_, n)| n).sum();
            let n = held.len() as u32;
            let target = |k: usize| total / n + u32::from((k as u32) < total % n);
            let mut need: Vec<(usize, u32)> = held
                .iter()
                .enumerate()
                .filter(|(k, (_, have))| *have < target(*k))
                .map(|(k, (slot, have))| (*slot, target(k) - have))
                .collect();
            for (k, &(slot, have)) in held.iter().enumerate() {
                let surplus = have.saturating_sub(target(k));
                if surplus == 0 {
                    continue;
                }
                // The whole stack in hand, one at a time into the short slots,
                // and what is left back where it came from.
                self.tweak_click(slot, 0, Input::Pickup);
                let mut left = surplus;
                for (to, want) in need.iter_mut() {
                    while left > 0 && *want > 0 {
                        self.tweak_click(*to, 1, Input::Pickup);
                        *want -= 1;
                        left -= 1;
                    }
                }
                self.tweak_click(slot, 0, Input::Pickup);
            }
        }
    }
}
