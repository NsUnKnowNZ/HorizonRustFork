pub const CONTENT_ID: &str = "eatinganimation";

const TICKS_PER_STAGE: f32 = 30.0;
const STAGES: [f32; 3] = [0.35, 0.70, 0.90];

pub fn animates(item: &str, has_food: bool) -> bool {
    has_food || matches!(item, "minecraft:potion" | "minecraft:milk_bucket")
}

pub fn stage(elapsed_ticks: i32) -> Option<usize> {
    let progress = elapsed_ticks.max(0) as f32 / TICKS_PER_STAGE;
    STAGES.iter().rposition(|&stage| progress >= stage)
}

pub fn frame_model(item: &str, base: &str, index: usize) -> String {
    let vanilla = match item {
        "minecraft:potion" => format!("minecraft:item/potion_overlay_drinking_{index}"),
        "minecraft:enchanted_golden_apple" => format!("minecraft:item/golden_apple_eating_{index}"),
        "minecraft:milk_bucket" | "minecraft:honey_bottle" => format!("{base}_drinking_{index}"),
        _ => format!("{base}_eating_{index}"),
    };
    own_namespace(&vanilla)
}

fn own_namespace(model: &str) -> String {
    let (ns, path) = model.split_once(':').unwrap_or(("minecraft", model));
    format!("{CONTENT_ID}:{ns}/{path}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn potions_milk_and_food_animate() {
        assert!(animates("minecraft:potion", false));
        assert!(animates("minecraft:milk_bucket", false));
        assert!(animates("minecraft:apple", true));
        assert!(!animates("minecraft:stone", false));
    }

    #[test]
    fn stages_start_at_the_original_thresholds() {
        assert_eq!(stage(-4), None);
        assert_eq!(stage(10), None);
        assert_eq!(stage(11), Some(0));
        assert_eq!(stage(20), Some(0));
        assert_eq!(stage(21), Some(1));
        assert_eq!(stage(26), Some(1));
        assert_eq!(stage(27), Some(2));
        assert_eq!(stage(40), Some(2));
    }

    #[test]
    fn a_meal_shows_the_three_frames_in_order() {
        let stages: Vec<Option<usize>> = (0..=32).map(stage).collect();
        assert!(stages.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(stages.last(), Some(&Some(2)));
    }

    #[test]
    fn frame_names_live_in_the_mods_namespace() {
        assert_eq!(
            frame_model("minecraft:apple", "minecraft:item/apple", 1),
            "eatinganimation:minecraft/item/apple_eating_1"
        );
        assert_eq!(
            frame_model("minecraft:golden_apple", "minecraft:item/golden_apple", 0),
            "eatinganimation:minecraft/item/golden_apple_eating_0"
        );
        assert_eq!(
            frame_model(
                "minecraft:enchanted_golden_apple",
                "minecraft:item/enchanted_golden_apple",
                2
            ),
            "eatinganimation:minecraft/item/golden_apple_eating_2"
        );
        assert_eq!(
            frame_model("minecraft:milk_bucket", "minecraft:item/milk_bucket", 1),
            "eatinganimation:minecraft/item/milk_bucket_drinking_1"
        );
        assert_eq!(
            frame_model("minecraft:honey_bottle", "minecraft:item/honey_bottle", 0),
            "eatinganimation:minecraft/item/honey_bottle_drinking_0"
        );
        assert_eq!(
            frame_model("minecraft:potion", "minecraft:item/potion", 2),
            "eatinganimation:minecraft/item/potion_overlay_drinking_2"
        );
        assert_eq!(
            frame_model("foodplusid:turnip", "foodplusid:item/turnip", 2),
            "eatinganimation:foodplusid/item/turnip_eating_2"
        );
    }
}
