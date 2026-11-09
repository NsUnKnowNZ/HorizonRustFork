//! Container screen slots: item and decoration drawing, the carried stack and quick craft drag,
//! slot positions and hit tests, highlights, and the crafting result slot.
use super::*;

impl Game<'_> {
    pub(super) fn draw_item(&self, batch: &mut HudBatch, stack: &ItemStack, x: f32, y: f32) {
        // `GuiGraphics.renderItem`: the resolved item model (tints, foil and
        // special renderers included), then the decorations.
        self.draw_item_model(batch, stack, x, y);
        self.draw_item_decorations(batch, stack, x, y, None);
    }
    /// `extractSlot` of slot `slot` holding `stack` at (x, y): during a quick
    /// craft drag a dragged slot shows what it would hold over a white
    /// highlight (its count yellow when the slot caps it), and hides while
    /// it is the drag's only slot.
    pub(super) fn draw_slot_item(
        &self,
        batch: &mut HudBatch,
        drag: Option<&DragView>,
        slot: usize,
        stack: &Option<ItemStack>,
        x: f32,
        y: f32,
    ) {
        let carried;
        let (stack, capped) = match drag.and_then(|d| d.slots.get(&slot)) {
            Some(None) => return,
            Some(Some((count, capped))) => {
                batch.fill(&self.gui.atlas, x, y, 16.0, 16.0, rgba(255, 255, 255, 128));
                carried = self
                    .carried
                    .clone()
                    .map(|c| ItemStack { count: *count, ..c });
                (carried.as_ref(), *capped)
            }
            None => (stack.as_ref(), false),
        };
        if let Some(stack) = stack {
            self.draw_item_model(batch, stack, x, y);
            let text = capped.then(|| format!("\u{a7}e{}", stack.count));
            self.draw_item_decorations(batch, stack, x, y, text);
        }
    }
    /// `extractCarriedItem`: the carried stack at the cursor, showing what a
    /// quick craft drag over several slots leaves in hand.
    pub(super) fn draw_carried(&mut self, batch: &mut HudBatch) {
        let Some(mut stack) = self.carried.clone() else {
            return;
        };
        if let Some(left) = self.drag_view().and_then(|d| d.left) {
            if left == 0 {
                return;
            }
            stack.count = left;
        }
        self.draw_item(batch, &stack, self.cursor.0 - 8.0, self.cursor.1 - 8.0);
    }
    /// The quick craft drag in progress, as the menu engine previews it.
    pub(super) fn drag_view(&mut self) -> Option<DragView> {
        let (ty, slots) = self.inv_drag.clone()?;
        let mut view = None;
        self.menu_session(|menu, env, to_menu| {
            let menu_slots: Option<Vec<usize>> = slots.iter().map(|&g| to_menu(g)).collect();
            let shown = match menu.quick_craft_preview(env, ty, &menu_slots?) {
                Some((shown, left)) => {
                    let slots = slots.iter().copied().zip(shown).collect();
                    DragView {
                        slots,
                        left: Some(left),
                    }
                }
                // A single dragged slot is hidden.
                None => DragView {
                    slots: slots.iter().map(|&g| (g, None)).collect(),
                    left: None,
                },
            };
            view = Some(shown);
            Some(false)
        });
        view
    }
    /// Whether a quick craft drag takes `slot`
    /// (`shouldAddSlotToQuickCraft`); screens the menu engine doesn't
    /// model take every slot.
    pub(super) fn quick_craft_accepts(&mut self, slot: usize, ty: i32, dragged: usize) -> bool {
        let mut accepts = true;
        self.menu_session(|menu, env, to_menu| {
            accepts = to_menu(slot).is_some_and(|s| menu.quick_craft_accepts(env, s, ty, dragged));
            Some(false)
        });
        accepts
    }
    /// `GuiGraphicsExtractor.itemDecorations`: the count (or `count_text`)
    /// and damage bar.
    pub(super) fn draw_item_decorations(
        &self,
        batch: &mut HudBatch,
        stack: &ItemStack,
        x: f32,
        y: f32,
        count_text: Option<String>,
    ) {
        // GuiGraphicsExtractor.itemDecorations, laid out at the origin and
        // moved to (x, y): the damage bar, the cooldown shade and the count.
        let info = self.items().get(&stack.item);
        // ItemStack.isBarVisible: isDamaged, which an unbreakable stack never is.
        let max_damage = if horizon_client::stations::is_damageable(self.items(), stack) {
            info.map_or(0, |info| info.max_damage as i32)
        } else {
            0
        };
        let cooldown =
            horizon_client::cooldowns::group(stack, info.and_then(|i| i.use_cooldown.as_ref()));
        let partial = self.clock.fraction() as f32;
        // BundleItem overrides the bar with its fullness.
        let mut bar = horizon_client::bundle::Contents::of(self.items(), stack)
            .map(|c| c.bar())
            .unwrap_or_else(|| hud_overlays::damage_bar(stack.damage as i32, max_damage));
        // Client-only: a mod may replace or hide the bar vanilla just chose.
        self.native_item_bar(stack, &mut bar);
        let decorations = hud_overlays::ItemDecorations {
            count: stack.count,
            count_text,
            bar,
            cooldown: self.cooldowns.percent(&cooldown, partial),
        };
        let mut draws = vec![hud_layout::GuiDraw::Push(vec![
            hud_layout::PoseOp::Translate(x, y),
        ])];
        draws.extend(hud_overlays::item_decorations(
            0,
            0,
            &decorations,
            &self.gui.atlas.font,
        ));
        draws.push(hud_layout::GuiDraw::Pop);
        hud_layout::paint(batch, &self.gui.atlas, &draws);
        self.native_item_decorations(stack, x, y, batch);
    }

    pub(super) fn inventory_origin(&self, batch: &HudBatch) -> (f32, f32) {
        if self.screen == Screen::Creative {
            return Self::creative_origin(batch);
        }
        let width = self.native_screen_width().unwrap_or(176.0);
        let (x, y) = batch.centered_origin(width, self.screen_height());
        (x + self.recipe_book_shift(batch.width), y - self.smooth_gui_offset())
    }
    /// Smooth GUI (Better OneBlock: 220 ms, BACK easing, moving DOWN): how far
    /// above its place a container screen still is while it slides in.
    pub(super) fn smooth_gui_offset(&self) -> f32 {
        const TIME: f32 = 0.22;
        const DISTANCE: f32 = 30.0;
        let t = (self.smooth_gui_elapsed() / TIME).clamp(0.0, 1.0);
        // easeOutBack.
        let (c1, c3) = (1.70158f32, 2.70158f32);
        let eased = 1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2);
        (1.0 - eased) * DISTANCE
    }
    /// Seconds since the open screen (with its container) was opened: a new
    /// screen, or the same kind after a frame without one, starts it again.
    pub(super) fn smooth_gui_elapsed(&self) -> f32 {
        use std::sync::Mutex;
        use std::time::{Duration, Instant};
        static OPENED: Mutex<Option<(&'static str, Option<(i32, i32, i32)>, Instant, Instant)>> =
            Mutex::new(None);
        let now = Instant::now();
        let key = (self.screen.id(), self.container);
        let Ok(mut opened) = OPENED.lock() else {
            return 1.0;
        };
        let start = match *opened {
            Some((id, container, start, seen))
                if (id, container) == key && now.duration_since(seen) < Duration::from_millis(150) =>
            {
                start
            }
            _ => now,
        };
        *opened = Some((key.0, key.1, start, now));
        now.duration_since(start).as_secs_f32()
    }

    /// InventoryMenu / CraftingMenu slot layouts.
    pub(super) fn slot_position(&self, i: usize) -> (f32, f32) {
        if self.creative_inventory_open()
            && let Some(position) = Creative::inventory_slot_position(i)
        {
            return position;
        }
        if self.creative_tab_open()
            && let Some(position) = Creative::tab_slot_position(i)
        {
            return position;
        }
        if let Some(p) = self
            .station_slot_position(i)
            .or_else(|| {
                Self::stock_machine_slot(self.machine_slot_position(i), self.native_screen_shown())
            })
            .or_else(|| self.crafter_slot_position(i))
            .or_else(|| self.mount_slot_position(i))
        {
            return p;
        }
        let table = self.craft_size == 3;
        // ContainerScreen (3 rows) shifts the player inventory down by one pixel.
        // ChestMenu: the inventory sits `18 * (rows - 3) + 1` lower than the
        // 3-row layout of the other menus.
        let shift = match (self.chest_rows(), self.native_screen_inventory_y()) {
            (_, Some(y)) => y - 84.0,
            (Some(rows), None) => 1.0 + (rows as f32 - 3.0) * 18.0,
            (None, None) => 0.0,
        };
        let inventory_x = self.native_screen_inventory_x().unwrap_or(8.0);
        match i {
            0..9 => (inventory_x + i as f32 * 18.0, 142.0 + shift),
            9..36 => (
                inventory_x + ((i - 9) % 9) as f32 * 18.0,
                84.0 + shift + ((i - 9) / 9) as f32 * 18.0,
            ),
            500..504 => (8.0, 8.0 + (i - 500) as f32 * 18.0),
            // InventoryMenu: the shield slot.
            504 => (77.0, 62.0),
            300..500 if let Some(p) = self.native_screen_slot(i - 300) => p,
            300..500 => match self.container_kind() {
                Some(BlockEntity::Furnace(_)) => {
                    [(56.0, 17.0), (56.0, 53.0), (116.0, 35.0)][(i - 300).min(2)]
                }
                _ => (
                    8.0 + ((i - 300) % 9) as f32 * 18.0,
                    18.0 + ((i - 300) / 9) as f32 * 18.0,
                ),
            },
            200 => {
                if table {
                    (124.0, 35.0)
                } else {
                    (154.0, 28.0)
                }
            }
            _ => {
                let g = i - 100;
                let (r, c) = (g / self.craft_size, g % self.craft_size);
                if table {
                    (30.0 + c as f32 * 18.0, 17.0 + r as f32 * 18.0)
                } else {
                    (98.0 + c as f32 * 18.0, 18.0 + r as f32 * 18.0)
                }
            }
        }
    }
    /// The stock hopper/dispenser slot layout, unless a mod screen is shown for
    /// that menu: a mod screen places its own container slots
    /// (`native_screen_slot`) and the player's inventory (`native_screen_inventory_y`),
    /// and this layout answers first in `slot_position`, which kept a mod's screen
    /// on a hopper or a dispenser on the stock grid (#5141). The rest of those
    /// menus is already the mod's: `draw_native_screen` owns the background and
    /// `screen_height` its height.
    pub(super) fn stock_machine_slot(
        stock: Option<(f32, f32)>,
        mod_screen_shown: bool,
    ) -> Option<(f32, f32)> {
        if mod_screen_shown { None } else { stock }
    }

    pub(super) fn slot_at(&self, batch: &HudBatch, x: f32, y: f32) -> Option<usize> {
        if self.book_covers_menu(batch.width) {
            return None;
        }
        let (ox, oy) = self.inventory_origin(batch);
        self.slots().into_iter().find(|&i| {
            let (sx, sy) = self.slot_position(i);
            // AbstractContainerScreen.isHovering: the 16x16 slot grows by one
            // pixel on every side, so the gaps between slots still hover one.
            x >= ox + sx - 1.0 && x < ox + sx + 17.0 && y >= oy + sy - 1.0 && y < oy + sy + 17.0
        })
    }
    /// `AbstractContainerScreen.extractSlotHighlightBack` and
    /// `AbstractContainerScreen.extractSlotHighlightFront`: the 24 x 24
    /// `container/slot_highlight_back` sprite behind the slot at `x`, `y` (its
    /// 16 x 16 item box), the `_front` one over its item.
    pub(super) fn draw_slot_highlight(&self, batch: &mut HudBatch, front: bool, x: f32, y: f32) {
        let sprite = if front {
            "gui/sprites/container/slot_highlight_front"
        } else {
            "gui/sprites/container/slot_highlight_back"
        };
        horizon_client::ui::blit_sprite(
            batch,
            &self.gui.atlas,
            sprite,
            x - 4.0,
            y - 4.0,
            24.0,
            24.0,
            u32::MAX,
        );
    }
    /// The highlight of the slot under the cursor, for the screens whose
    /// slots all sit in `slot_at`'s grid. `Slot.isHighlightable` is false
    /// for the crafter's result (`NonInteractiveResultSlot`).
    pub(super) fn draw_hovered_slot_highlight(
        &self,
        batch: &mut HudBatch,
        front: bool,
        ox: f32,
        oy: f32,
    ) {
        let Some(slot) = self.slot_at(batch, self.cursor.0, self.cursor.1) else {
            return;
        };
        if slot == 309 && matches!(self.container_kind(), Some(BlockEntity::Crafter(_))) {
            return;
        }
        let (sx, sy) = self.slot_position(slot);
        self.draw_slot_highlight(batch, front, ox + sx, oy + sy);
    }
    pub(super) fn slot_mut(&mut self, i: usize) -> &mut Option<ItemStack> {
        if i >= mount_menu::SADDLE {
            return self.mount_slot_mut(i);
        }
        if i >= stations::INPUT {
            return self.station_slot_mut(i);
        }
        if (500..504).contains(&i) {
            return &mut self.armor[i - 500];
        }
        if i == 504 {
            return &mut self.offhand;
        }
        if i >= 300 {
            return self.container_slot_mut(i - 300);
        }
        if (100..109).contains(&i) {
            &mut self.craft_grid[i - 100]
        } else {
            &mut self.inventory[i]
        }
    }
    /// What a slot shows: the crafting result slots are computed.
    pub(super) fn slot_view(&mut self, i: usize) -> Option<ItemStack> {
        match i {
            200 => self.craft_result(),
            309 if matches!(self.container_kind(), Some(BlockEntity::Crafter(_))) => {
                self.crafter_result()
            }
            _ => self.slot_mut(i).clone(),
        }
    }
    pub(super) fn craft_result(&self) -> Option<ItemStack> {
        // A server's result slot is the server's (`CraftingMenu.slotChangedCraftingGrid`).
        if let Some(r) = &self.remote {
            return r.craft_result.clone();
        }
        self.craft_recipe().map(|(_, result)| result)
    }
    /// The recipe id and result of the crafting grid.
    /// `RecipeCraftingHolder.awardUsedRecipes`: a crafted recipe becomes known
    /// (special ones never do).
    pub(super) fn award_crafted(&mut self, crafts: &[(String, Vec<Option<ItemStack>>)]) {
        for (recipe, ingredients) in crafts {
            // The whole id: the advancements' `recipe_crafted` criteria name
            // `minecraft:x`, and the crafting copy names the recipe bare.
            let recipe = horizon_client::crafting::qualified(recipe);
            self.trigger_recipe_crafted(&recipe, ingredients);
        }
        let recipes: Vec<String> = crafts
            .iter()
            .map(|(r, _)| horizon_client::crafting::qualified(r))
            .collect();
        let index = &world_stations!(self).recipe_index;
        let (_, unlocked) = self.level.recipe_book.add_recipes(index, &recipes);
        self.recipe_unlocked(&unlocked);
    }
    pub(super) fn craft_recipe(&self) -> Option<(String, ItemStack)> {
        let n = self.craft_size;
        let grid: Vec<Option<ItemStack>> = (0..n * n).map(|i| self.craft_grid[i].clone()).collect();
        let scale = |id: i32| self.maps.data.get(&id).map(|m| m.scale);
        // The world's table, the one taking the result (`menu_click`) crafts from.
        self.recipes()
            .find_with(&grid, n, &scale)
            .map(|(name, result)| (name.to_owned(), result))
    }
    pub(super) fn draw_inventory_slots(&mut self, batch: &mut HudBatch, ox: f32, oy: f32) {
        let drag = self.drag_view();
        let swaps = self.smooth_swaps();
        let mut hovered: Option<(usize, Option<ItemStack>, f32, f32)> = None;
        for i in self.slots() {
            let (sx, sy) = self.slot_position(i);
            if i == 504 && self.offhand.is_none() {
                batch.sprite(
                    &self.gui.atlas,
                    "gui/sprites/container/slot/shield",
                    ox + sx,
                    oy + sy,
                );
            }
            if (500..504).contains(&i) && self.armor[i - 500].is_none() {
                let icon = ["helmet", "chestplate", "leggings", "boots"][i - 500];
                batch.sprite(
                    &self.gui.atlas,
                    &format!("gui/sprites/container/slot/{icon}"),
                    ox + sx,
                    oy + sy,
                );
            }
            let mut stack = self.slot_view(i);
            // Smooth Swapping: what is still on its way here isn't here yet.
            if let Some(owed) = swaps.owed(i) {
                stack = stack.and_then(|s| {
                    (s.count > owed).then(|| ItemStack {
                        count: s.count - owed,
                        ..s
                    })
                });
            }
            // Immersive UI (Better OneBlock: hovered item 1.4x, matching items
            // hover 0.8): the hovered stack is drawn last, larger; stacks of
            // the carried item bob.
            let (cx, cy) = self.cursor;
            let (x, y) = (ox + sx, oy + sy);
            if stack.is_some() && cx >= x - 1.0 && cx < x + 17.0 && cy >= y - 1.0 && cy < y + 17.0 {
                hovered = Some((i, stack, x, y));
                continue;
            }
            let bob = match (&self.carried, &stack) {
                (Some(c), Some(s)) if c.item == s.item => {
                    let t = self.started.elapsed().as_secs_f32();
                    ((t * 6.0 + i as f32 * 0.7).sin() * 0.8).round()
                }
                _ => 0.0,
            };
            self.draw_slot_item(batch, drag.as_ref(), i, &stack, x, y + bob);
        }
        if let Some((i, stack, x, y)) = hovered {
            let start = batch.vertices.len();
            self.draw_slot_item(batch, drag.as_ref(), i, &stack, x, y);
            if drag.is_none() && self.carried.is_none() {
                batch.scale_since(start, x + 8.0, y + 8.0, 1.4, 1.4);
            }
        }
        // Smooth Swapping: the moving stacks, over the slots.
        for (stack, x, y) in swaps.flying(ox, oy) {
            self.draw_item_model(batch, &stack, x, y);
            self.draw_item_decorations(batch, &stack, x, y, None);
        }
    }
    /// Smooth Swapping: the slots' contents against the last frame's; an item
    /// that left one slot and arrived in another this frame flies between them.
    fn smooth_swaps(&mut self) -> SmoothSwaps {
        use std::sync::Mutex;
        use std::time::{Duration, Instant};
        static STATE: Mutex<Option<SwapState>> = Mutex::new(None);
        let now = Instant::now();
        let key = (self.screen.id(), self.container);
        let slots: Vec<usize> = self.slots();
        let now_slots: Vec<(usize, Option<ItemStack>, (f32, f32))> = slots
            .into_iter()
            .map(|i| (i, self.slot_view(i), self.slot_position(i)))
            .collect();
        let Ok(mut guard) = STATE.lock() else {
            return SmoothSwaps::default();
        };
        let fresh = !matches!(&*guard, Some(st) if st.key == key
            && now.duration_since(st.seen) < Duration::from_millis(150));
        if fresh {
            *guard = Some(SwapState { key, seen: now, last: Vec::new(), moves: Vec::new() });
        }
        let st = guard.as_mut().expect("just set");
        st.seen = now;
        st.moves.retain(|m| now.duration_since(m.start).as_secs_f32() < SWAP_TIME);
        if !fresh && st.last.len() == now_slots.len() {
            let count = |s: &Option<ItemStack>, item: &str| {
                s.as_ref().filter(|s| &*s.item == item).map_or(0, |s| s.count)
            };
            let mut lost: Vec<(usize, std::sync::Arc<str>, u32, (f32, f32))> = Vec::new();
            let mut gained: Vec<(usize, ItemStack, u32)> = Vec::new();
            for ((i, before, at), (_, after, _)) in st.last.iter().zip(&now_slots) {
                if let Some(b) = before {
                    let left = count(after, &b.item);
                    if left < b.count {
                        lost.push((*i, b.item.clone(), b.count - left, *at));
                    }
                }
                if let Some(a) = after {
                    let had = count(before, &a.item);
                    if a.count > had {
                        gained.push((*i, a.clone(), a.count - had));
                    }
                }
            }
            for (to, stack, mut n) in gained {
                for l in lost.iter_mut().filter(|l| l.1 == stack.item && l.2 > 0) {
                    if n == 0 {
                        break;
                    }
                    let take = n.min(l.2);
                    l.2 -= take;
                    n -= take;
                    st.moves.push(SwapMove {
                        stack: ItemStack { count: take, ..stack.clone() },
                        from: l.3,
                        to,
                        start: now,
                    });
                }
            }
        }
        st.last = now_slots.clone();
        let positions: std::collections::HashMap<usize, (f32, f32)> =
            now_slots.iter().map(|(i, _, at)| (*i, *at)).collect();
        SmoothSwaps {
            moves: st
                .moves
                .iter()
                .filter_map(|m| {
                    let to = *positions.get(&m.to)?;
                    let t = (now.duration_since(m.start).as_secs_f32() / SWAP_TIME).clamp(0.0, 1.0);
                    // easeOutCubic.
                    let e = 1.0 - (1.0 - t).powi(3);
                    let at = (m.from.0 + (to.0 - m.from.0) * e, m.from.1 + (to.1 - m.from.1) * e);
                    Some((m.to, m.stack.clone(), at))
                })
                .collect(),
        }
    }
}

/// Smooth Swapping's flight time (Better OneBlock: animation speed 300).
const SWAP_TIME: f32 = 0.2;

struct SwapMove {
    stack: ItemStack,
    from: (f32, f32),
    to: usize,
    start: std::time::Instant,
}

struct SwapState {
    key: (&'static str, Option<(i32, i32, i32)>),
    seen: std::time::Instant,
    last: Vec<(usize, Option<ItemStack>, (f32, f32))>,
    moves: Vec<SwapMove>,
}

/// This frame's flights: (destination slot, stack, slot-space position).
#[derive(Default)]
struct SmoothSwaps {
    moves: Vec<(usize, ItemStack, (f32, f32))>,
}
impl SmoothSwaps {
    /// How many items of the slot's stack are still flying to it.
    fn owed(&self, slot: usize) -> Option<u32> {
        let n: u32 = self.moves.iter().filter(|m| m.0 == slot).map(|m| m.1.count).sum();
        (n > 0).then_some(n)
    }
    fn flying(&self, ox: f32, oy: f32) -> Vec<(ItemStack, f32, f32)> {
        self.moves
            .iter()
            .map(|(_, stack, (x, y))| (stack.clone(), ox + x, oy + y))
            .collect()
    }
}
