# Better OneBlock → Horizon: mod port ledger

Status: `done` · `in progress` · `todo` · `reuse` (a Horizon port already exists: install + verify) ·
`built-in` (Horizon already has it) · `not needed` · `blocked`.
Size: S = hours, M = a day or so, L = weeks. Licence matters: ARR / no-derivatives mods are rewritten from
their described behaviour only, never from their code, and every port stays on this machine.

Build: `.local/native/mods/<id>/` bundles → `.local/native/build-native.ps1` → `.local/native/target/fast/`.
Horizon source changes (allowed since 2026-11-08) live as uncommitted edits under `crates/`; build-native.ps1 exports them with HEAD.

## Wave 1 — small gameplay mods (S)
| Mod | Licence | Status | Notes |
|---|---|---|---|
| What Durability? | ARR | done | hook `durability_loss`; config `horizon-mods/.horizon-mod-data/what_durability/config.json`. Tested: pickaxe 0 damage after 5 blocks |
| Stackable 127 | CC0 | done | pack config: 64→100, egg/snowball/golden apple 50, enchanted golden apple 10; hook `modify_default_components`. Tested |
| Forgiving Void | ARR | done (tested: players, and a cow dropped below the world came back) | y < bottom−32 → back from y=300 over last ground, blindness, 19.9 landing damage. Mobs: tamed ones and `entityAllowList` kinds (pets, horses, villagers, farm animals by default) drop back over their last ground with 2 min slow falling. Config `horizon-mods/.horizon-mod-data/forgiving_void/config.json` |
| Simple Death Improvements | MIT | done (tested; XP: 10 levels dropped 128 of 160 points as orbs) | keeps hotbar/armour/offhand; other drops fall in place, never despawn (tested); 80% XP via `death_reward`, from level + bar (`/xp set` leaves the running total at 0). Lava/void item saving, explosion-proof items: todo |
| Boat Break Fix | custom | not needed | Horizon's boats keep no fall distance, so they never break on landing (the bug the mod fixes can't happen) |
| Nether Portal Fix | ARR | done (tested; Horizon source) | every Nether trip links the portal left and the portal reached, both ways; going back leads to the portal you came from even when another is closer. Links saved in `<world>/data/horizon_portal_links.txt` (`game/nether_portal.rs` `portal_links`). Kept per portal, not per player |
| Head in the Clouds | LGPL-3 | done (tested: clear sky above clouds while raining below) | server: no precipitation at y≥192.33; client: rain fades near/above clouds |
| Better Days | LGPL-3 | done (tested: ~110× night while sleeping, wakes at morning) | sleep speeds the night (up to 110×) instead of skipping; weather/random-tick effects todo |
| Dried Ghast from Piglin Bartering | ARR | done (tested: 80 barter rolls gave 6 dried ghasts) | weight 10 (~2.1%) in the Better OneBlock world's datapacks |
| Random Mob Effects | ARR | done (tested: zombie got endless Darkness, creeper skipped) | one random effect (pack list) per hostile mob, level 1, endless; tagged once |
| Naturally Trimmed | LGPL-3 | done (tested at 100%: resin/vex on all pieces) | 33% of armoured mobs get a random trim (75% per piece); loot/trade trims todo |
| More Babies | ARR | todo (M) | baby variants of more mobs |
| Difficulty (So I Heard…) | MIT | todo (M) | |
| Biome Paint | CC0 | done (tested: the box between two clicks became cherry_grove, paint lost 1 durability, brush took the paint colour and cleaned itself) | content mod `horizon-mods/biome_paint` (brush + paint items, textures, 63 recipes from the jar; a paint keeps its biome in `custom_data` and colour in `dyed_color`, tinted with the `dye` tint) + native mod `biome_paint`: brush in one hand, paint in the other, right click two blocks → `/fillbiome` of the box (4-block cells, `max_block_modifications`), sneak + click cleans the brush; brush 512 / paint 128 durability. The new biomes reach players at the level's next tick |

## Wave 2 — UI/client mods
| Mod | Licence | Status | Notes |
|---|---|---|---|
| AppleSkin | Unlicense | built in (partly seen 2026-11-09: the held food's name shows its hunger value as drumsticks; the bar preview not caught in a still) | food preview not seen in two screenshots (food at 6/20 holding steak): to check; registry `appleskin` (soufied/AppleSkin-Horizon) |
| Jade | CC-BY-NC-SA | done (tooltip verified) | registry `jade` (Joeypc6/Jade-Horizon) |
| BetterF3 | MIT | done (SmartF3; F3 screen verified; LAN-guest targeting dropped) | registry `smartf3` |
| Freecam | MIT | done (tested: F4 detaches, W flies, F4 back) | 2 targets moved to game::play_input;  registry `freecam` |
| Mod Menu | MIT | reuse / not needed | Horizon has a Mods page; registry `mod_menu` |
| Eating Animation | MIT | built in (tested: eating in first person shows the food raised with crumbs) | content packed from repo;  registry `eatinganimation` |
| 3D Skin Layers | tr7zw protective | built in, needs play-test | registry `skins_3d` |
| Wavey Capes | tr7zw protective | built in, needs play-test | registry `smoothie_capes` |
| Mouse Tweaks | BSD-3 | built-in | Extras → Mouse Tweaks |
| Entity Model / Texture Features | LGPL-3 | built-in | Horizon's OptiFine CEM, random + emissive entities |
| Better F1 Reborn | MIT | done (tested: F1 hides HUD, hand stays) | |
| Custom Window Title | Unlicense | done (tested: "Better OneBlock - Minecraft 26.3" + pack icon) | |
| Enhanced Attack Indicator | MIT | done (tested) | breaking (guest `mining` progress), bow/crossbow/trident draw, food/potions, cooldowns |
| Cut Through | MPL-2 | done (tested) | entity wins over a collision-less block in front |
| Reacharound | CC BY-NC-SA 3.0 (local only) | done (tested) | both axes, off hand, 2D "-   -"/"- -" mark; no 3D outline or fade animation |
| Crafting Tweaks | ARR | done (tested; Horizon source) | rotate / balance / clear buttons left of the 2x2 and 3x3 grids, made of ordinary slot clicks: `game/crafting_tweaks.rs`. Rotate leaves a ring alone where two neighbours hold the same item |
| Damage Numbers | (no licence file) | done (tested) | client health drops float up as numbers, orange→red→dark red, 32 ticks |
| NamePain | ARR | done (tested) | named mobs' tags white→yellow→gold→red→dark red with health (legacy colours; players' tags not coloured) |
| Mob Plaques | MPL-2 | done (tested) | ❤ health % (+ base armour) over hurt mobs ≤48 blocks, text glyphs instead of HUD sprites; no air plaque |
| Fancy Health Bar | MIT | done (tested) | vanilla style + pack's pixel splash on every lost half heart |
| TightFire | ARR | done (setting) | Horizon's own `lowFire` (Lower fire) set to 40% in horizon-extras.txt |
| Smooth Scroll / Smooth GUI / Smooth Swapping | GPL/MIT/LGPL | done (tested; Horizon source) | hotbar selector glides (0.25, rolls over the edges): `hud_layout::HotbarInputs.selected_at` + `game.rs` `smooth_hotbar_selection`; container screens slide down into place (220 ms, back easing) and the dimming fades in to 0.65 over 150 ms: `inventory_slots.rs` `smooth_gui_offset`, `screen_backgrounds.rs`; moved stacks fly slot to slot (200 ms): `inventory_slots.rs` `smooth_swaps`. Chat/creative-list smooth scrolling: not done |
| Seamless Loading Screen | MIT | todo (M) | the loading frame can only draw atlas sprites and 64 px icons; a full-screen screenshot background needs a new texture path in the renderer |
| Immersive Hotbar / Immersive UI | MIT / ARR | partly done (tested; Horizon source) | selected hotbar item grows to 1.2x with the gliding selector (`game.rs` hotbar draw); hovered inventory item 1.4x, stacks of the carried item bob (`inventory_slots.rs`). Todo: carried-item rotation, rarity/enchant particles, screen shake, pickup/use animations, XP bar glow |
| Fancy Toasts | MIT | done (tested: task, challenge and goal plaques, queued one at a time) | native mod `fancy_toasts`: advancement toasts become a plaque dropping in at the top centre (icon at 1.5x, the frame's colour and label, title, two lines of description, a shine), with a sound per frame; time follows Notification Time. Recipe/tutorial/system toasts unchanged |
| Advancements Reloaded | MIT | todo (L) | full-screen advancements screen with search |
| Chat Patches | LGPL-3 | built-in (mostly) | Horizon's Compact chat (repeat counter), Copy and Search turned on in horizon-extras.txt; chat log kept across sessions and hover timestamps: not available |
| Visual Keys | ARR | todo (M) | |
| Inventory HUD+ | ARR | built-in | the pack only turns on its potion-effects list; Horizon's HUD Effects turned on (top left: the frame-time overlay sits top right) |
| Zannagh's Armor Hider | MIT | done (tested: helmet hidden, "Helmet: hidden" shown; Horizon source) | in the inventory, H over an armour slot hides/shows that piece on your own model (still worn and protecting). `game/armor_hider.rs`, config `horizon-mods/.horizon-mod-data/armor_hider/config.json` |
| What Are They Up To | ARR | done (tested 2026-11-09 with two clients on a test server: "in inventory", "typing." and "paused" shown over the other player) | native mod `watut`: each Horizon client tells the server (`watut:state`) whether its player is typing, in an inventory, paused or away (still for 60 s); the server tells everyone (`watut:states`); a label shows over each other player's head. Arm/head poses: not yet |
| Ping Wheel | MIT | done (tested: beam + "6.8m" through walls) | native mod `ping_wheel`: ping key (default middle mouse, rebindable as `key.ping_wheel.ping`) marks the block looked at up to 256 away: beam, outline and distance through walls for 7 s, bell sound, up to 8. Wheel of ping kinds and sharing with others: not yet |
| FindMe | MIT | done (tested: 2 of 3 containers outlined, the dirt one not) | native mod `findme`: Y over an item (or with one carried) in any inventory asks the server (`findme:find` mod message) which chests/barrels/shulkers within 24 blocks hold it; they are outlined through walls for 10 s |
| A Good Place / FWA (block animations) | MIT | todo (M) | |
| Punchy FPA (first-person anims) | ARR | todo (M) | |
| Particle mods: Particle Rain, Biome Particle Weather, Particle Interactions, Particle Effects, Particle Tweaks, Effectual, Subtle Effects, Inventory Particles | various | todo (M each) | ND/NC licences: local only |
| Sound mods: AmbientSounds, Sound, Presence Footsteps | various | todo (M–L) | |
| LambdaBetterGrass | custom | todo (M) | |
| Fog | ARR | todo (M) | |
| Nuit (skyboxes) | MIT | done (tested: Better OneBlock's Fresh Skybox loads as 6 layers; day, sunset, night and sunrise skies drawn) | Horizon source `horizon-client-render/src/custom_sky.rs` `fabric_skies`: FabricSkyboxes/Nuit `assets/{fabricskyboxes,nuit}/sky/*.json` (schema 2, `single-sprite-square-textured`) become OptiFine sky layers after the pack's own: fade ticks, blend, rotation (with the sun or `rotationSpeed*`), weathers, worlds, biomes, `yRanges`, transition. Not read: `monocolor`, six-image `square-textured`, the decorations switches |
| Enhanced Visuals | LGPL-2.1 | done (tested 2026-11-09: a 5-damage hit reddens the screen edges) | native mod `enhanced_visuals`: red screen edges on damage (stronger with bigger hits), and below 30% health a pulsing red vignette with a heartbeat sound that quickens as health falls. Blood splatter, glass cracks, explosion blur: not yet. Fixed 2026-11-09: it drew nothing before (`hud_layout::paint` skips vignette blits); it now draws `misc/vignette` straight into the HUD batch with the vignette blend. The low-health pulse is faint |
| Beautiful Enchanted Books | ARR | todo (M) | mostly textures |
| Reliable EMI (needs EMI) | MIT | todo (L) | registry has `jei`: use as the recipe viewer |

## Wave 3 — medium gameplay
| Mod | Licence | Status | Notes |
|---|---|---|---|
| Carry On | LGPL-3 | done (tested: sneak + empty-hand right click picked up a chest, "Carrying: Chest" HUD) | 7 API fixes for Horizon e1f01af; command verified;  registry `carryblock` |
| ItemPhysic | LGPL-2.1 | partly done (tested; Horizon source) | dropped items no longer bob or spin: flat items lie on the ground, blocks stand on it, each turned its own way, tumbling while they fall (`game/item_draw.rs`). Todo: floating (swimming) items, fall sounds, cactus-proof items, pickup tooltip |
| Underlay | GPL-3 | todo (M) | |
| BedrockIfy | GPL-3 | todo (M) | |
| Terrastorage | MIT | partly built-in | Horizon's sort buttons turned on (Extras → Sort); quick stack to nearby storages, loot/deposit all and favourites: todo |
| Camerapture | MIT | todo (M–L) | |
| Mob AI Tweaks | MIT | partly done (tested) | native mod `mob_ai_tweaks`: animals panic in groups (same kind within 16, no one blamed), a hit sets a lit creeper's fuse back, a hurt squid's ink blinds players and mobs within 8 for 5 s. The other ~140 settings (reworked AI for many mobs): todo |
| Yori3o's Grappling Hooks | MIT | todo (M) | |

## Wave 4 — large
| Mod | Licence | Status | Notes |
|---|---|---|---|
| Supplementaries (+ Amendments) | custom | todo (L) | Supplementaries Squared's missing models are fixed apart from it: the 18 Supplementaries models/textures it builds on are copied into `horizon-mods/suppsquared` from supplementaries-1.21.1-4.0.2-fabric.jar, and the run-time ones (wall lantern templates, daub frame walls, shelf texture) written by hand |
| Handcrafted | Terrarium | todo (L) | |
| Charm (forked) | MIT | todo (L) | |
| Bits and Balance | ARR | todo (L) | 100+ small features |
| Faewuf's Diversity | CC-BY-NC-SA | todo (L) | |
| Mutant Monsters | AGPL-3 | todo (L) | |
| Friends & Foes | CC-BY-NC-ND | todo (L) | no-derivatives: local only, never shared |

## Extra — asked for, not in the pack
| Feature | Status | Notes |
|---|---|---|
| Horizon Portals stage 1: any-size and horizontal Nether portals | done (tested; Horizon source + content mod `horizon-mods/horizonportals`) | frames up to 64x64 (`horizon-world` `portal.rs` MAX_WIDTH/HEIGHT, `nether_portal.rs` rectangles); a flat obsidian ring of any outline lights with fire and fills with `horizonportals:horizontal_portal` (glass-template block, translucent purple plane); it drains when the frame breaks (`shapes.rs`); entities and players falling into it travel (`portal_axis`, `server_travel.rs` `nether_portal_inside`). Arrival is at a vertical portal on the other side |
| Horizon Portals stage 2: no loading screen through Nether portals | done (tested) | `remote.rs` `remote_screen_up`: a Nether-portal level load streams in behind the game instead of covering it |
| Horizon Portals stage 3: see the other side through a portal | done for vertical Nether portals (tested both ways) | Looking into a lit Nether portal shows the real other dimension, live, with parallax. **Client** (`game/portal_view.rs`): near a portal it asks the server about once a second on `horizon:portal_view`; the other side's columns are kept 2^20 chunks east (`VIRTUAL_X`, kept out of the storage-range drop and meshed via `mod_forced`), skyless levels seen from a sky level get sky light 9. **Server**: finds the exit (NetherPortalFix link, else closest portal to the scaled position, else where one would be), holds 7x7 chunks there with a `horizon:portal_view` mod ticket (released 4 s after the last ask), sends them 6 per tick encoded for that level. **Renderer** (`gfx.rs`): a second section list walked from the camera carried through the portal (turned a quarter when the portals face different ways), drawn into an offscreen target cut at the exit portal's plane, shown on the portal surface. `HORIZON_PORTAL_VIEW_TEST=dx,dy,dz` still tests the same world moved. Not yet: horizontal portals' view, entities/particles and translucent terrain on the other side, the other level's fog and sky colour, live block changes there (sent once per view) |
| Horizon Portals: packaging and polish | done (tested) | all portal changes are one patch, `.local/patches/horizon-portals/horizon-portals.patch` (README beside it), applied by build-native.ps1 when listed in `.local/patches/enabled.txt`; flat portals show the view looking down; no purple overlay/warp while standing in a portal; no portal particles while a view shows; translucent terrain (glass, stained glass, water) shows through; blocks changed on the other side are sent again (seen: glowstone → emerald within 3 s); the other level's fog colour as the background |
| Java packs import | built (Java folders empty on this PC; shader and resource packs tested directly) | `horizon-client-render/src/java_packs.rs`: at start, packs in Java's `.minecraft/resourcepacks` and `shaderpacks` (and folders listed in `horizon-java-folders.txt`) that Horizon lacks are copied in. Horizon already reads Java resource packs and Iris/OptiFine shader packs |
| Offline server (no Mojang account) | done (tested 2026-11-09: a no-account client joined) | `server-oneblock/`: `horizon-server` (now built with the mods) running the modded `horizon-client.exe` as its engine (`engine.path`, `engine.resources`), `online_mode = false`, whitelist off, fresh OneBlock world, new players start in spectator so the pack sets them up. Start/stop: `horizon-server --dir server-oneblock start|stop` (`.local/setup/stop-server.bat`). Horizon's own Play with Friends (pause menu → Friends → invite code) also needs no account. Clients whose content mods (`horizon-mods`) differ from the server's are turned away ("Your mods don't match this server's"), so `.local/setup/job-zip2.ps1` copies this build and `horizon-mods` into both `server-oneblock` (engine + mods, when the server is stopped) and the friends package before zipping |
| OneBlock world fixes | done (tested on a fresh world) | the 26.3 conversion (`convert/convert_oneblock.py`) left: a lava layer (noise settings' sea level 63 → lava below y=-54: now sea level = min y), Overworld structures floating in the void (ocean monuments, trial chambers…: every Overworld structure set now has frequency 0; Nether fortresses/fossils and End cities kept), raw JSON on the pack's floating texts (1.21.5 text components: `text:'{json}'` → `text:{...}`). The pack's progress bar needs players to join in spectator once (it then sets `ob-progressbar`); in an older world: `/scoreboard players set @s ob-progressbar 1`. Old chunks of an existing world keep the lava/structures: `.local/setup/prune_chunks.py` deletes chunks nobody spent time in |
| Friends package | done | `dist/Horizon-OneBlock.zip` (91 MB; built by `.local/setup/job-package.ps1`, zipped leaving out `.local`, `.horizon-cache`, the server jar and test leftovers). Tested 2026-11-09: first-time setup inside the dist folder, then the packaged client joined `server-oneblock` offline as FriendTest (first start about 4 min while it builds `.horizon-cache`). `dist/Horizon-OneBlock`: the modded client, server and preparer, the content mods, Better OneBlock's own packs, prebuilt exporters; `1 - First time setup.bat` builds the game files from the friend's own Minecraft 26.3 (server jar fetched from Mojang), `2 - Join a server.bat` asks a name and an address or invite code. No Mojang files are shipped; third-party shaders/packs are listed with their Modrinth links in `Shaders and packs.txt`. No PowerShell scripts any more (the setup downloads the server jar with `curl.exe`). Open problem: Chrome/Defender flag the zip on download as `Trojan:Script/Wacatac.B!ml` (a machine-learning guess on new unsigned exes; the files scan clean once on disk): needs a Microsoft false-positive report or code signing |
| Shader packs and resource packs | tested 2026-11-08/09 | Iris/OptiFine packs run through Horizon's own translator at 1440p on an RX 7800 XT. Shaders (12): Complementary Reimagined r5.9.3 (125–147 fps), Complementary Unbound r5.9.3 (~130), BSL 10.1.8 (151), Photon 1.4 (~105–140), Solas 3.9 (~65–80), Bliss 2.1.2 (~75), Rethinking Voxels r0.1-beta9 (~45), Super Duper Vanilla 1.3.8 (~175), Sildur's Vibrant 2.02 Extreme (177), Nostalgia 5.1 (~150; once lost the graphics device), MakeUp Ultra Fast 9.6 (205), Miniature 2.19 (~205). Resource packs checked on screen: Faithful 32x, Fresh Animations 1.11.2 + FA Extensions 1.10.1 + Fresh Moves 3.1.1 (mob/player models), Better Leaves 9.6 (bushy leaves), Fancy Crops 1.4 (3D crops), New Glowing Ores, Fast Better Grass, Low On Fire, Dramatic Skys 1.5.3 (OptiFine sky: clouds at sunset, stars and moon), Default Dark Mode 26.3 (dark inventory), Enchantment Outlines (glowing-glints), Bare Bones. All in `shaderpacks/` and `resourcepacks/`, off by default |
| Shader pack crash guard | done (Horizon source) | `game.rs` `recover_device` + `shader_packs.rs` `drop_pack_after_device_loss`: when the graphics device is lost while a shader pack runs, that pack is turned off for the rest of the run, the world comes back without it and the chat says which pack and why, instead of the game closing (Nostalgia 5.1 did this once) |

## Not needed / not possible
| Mod | Why |
|---|---|
| ~32 performance mods (Sodium, Lithium, …) | Horizon is the replacement |
| ~25 libraries | their features become Horizon code where a port needs them |
| Vanilla Backport | backports 26.x features to 1.21.1; Horizon is 26.3 |
| Neruina | crash guard for Java ticking; not applicable |
| Essential / Essential Patcher | closed online service |
| Iris, No Chat Reports | Horizon has its own shader packs; chat reporting n/a |
