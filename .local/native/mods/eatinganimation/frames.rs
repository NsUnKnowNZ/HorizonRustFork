fn official_eatinganimation_frame(
    renderer: &crate::item_render::ItemRenderer,
    stack: &horizon_assets::item_model::StackView,
    display: horizon_assets::item_model::DisplayContext,
    owner: Option<&horizon_assets::item_model::OwnerState>,
    level: Option<&horizon_assets::item_model::LevelState>,
    seed: i32,
    env: &dyn horizon_assets::item_model::ItemEnv,
) -> std::ops::ControlFlow<ItemRenderState> {
    use crate::item_render::official_eatinganimation as ea;
    use horizon_assets::item_bake::SpriteLookup;
    use horizon_assets::item_model::{Context, FoilType, LayerContent};
    use std::ops::ControlFlow::{Break, Continue};

    let Some(in_use) = owner.filter(|o| o.using_this_item) else {
        return Continue(());
    };
    let components = renderer.models.components(stack);
    if !ea::animates(&stack.item, components.contains_key("minecraft:food")) {
        return Continue(());
    }
    let Some(index) = ea::stage(in_use.use_duration - in_use.use_item_remaining_ticks) else {
        return Continue(());
    };
    let resolved = renderer.models.resolve(
        stack,
        &Context {
            display,
            owner,
            level,
            seed,
        },
        env,
    );
    let Some(first) = resolved.layers.first() else {
        return Continue(());
    };
    let LayerContent::Model { model: base } = &first.content else {
        return Continue(());
    };
    let frame = ea::frame_model(&stack.item, base, index);
    if !renderer
        .library
        .resolve(&frame)
        .is_ok_and(|m| m.uses_item_generator())
    {
        return Continue(());
    }
    let mut layers = Vec::with_capacity(resolved.layers.len());
    for (i, layer) in resolved.layers.iter().enumerate() {
        let model = if i == 0 {
            frame.as_str()
        } else if let LayerContent::Model { model } = &layer.content {
            model.as_str()
        } else {
            return Continue(());
        };
        let baked = renderer.bake(model);
        if i == 0
            && (baked.quads.is_empty() || renderer.sprites.mask(&baked.particle.sprite).is_none())
        {
            return Continue(());
        }
        layers.push(LayerRender {
            geometry: LayerGeometry::Quads(baked.clone()),
            tints: layer.tints.clone(),
            foil: layer.foil,
            matrix: glam_matrix(&baked.layer_matrix(display, &layer.transform)),
            particle: sprite_name(&baked.particle.sprite).to_owned(),
            uses_block_light: baked.uses_block_light,
            extents: renderer.quad_extents(model, &baked),
        });
    }
    let animated = resolved.layers.iter().any(|l| l.foil != FoilType::None);
    Break(ItemRenderState {
        display,
        layers,
        oversized_in_gui: resolved.oversized_in_gui,
        animated,
        cit_glint: None,
        hdr_tier: 0,
        hdr_light: 0,
    })
}
