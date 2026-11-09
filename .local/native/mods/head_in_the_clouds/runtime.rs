// Head in the Clouds for Horizon. The cloud height is the Overworld's
// (EnvironmentAttributes.CLOUD_HEIGHT, 192.33); the other vanilla dimensions
// have no weather.
pub const CLOUD_HEIGHT: f64 = 192.33;
const ADDITIONAL_CLOUD_HEIGHT: f64 = 3.0;
const GRADIENT_HEIGHT: f64 = 6.0;

/// HeadInTheCloudsAPI.allowRainAtPos, negated: a block at or above the clouds.
pub fn above_clouds(y: i32) -> bool {
    f64::from(y) >= CLOUD_HEIGHT
}

/// HeadInTheClouds.getRainGradient's factor for a camera at `y`.
pub fn rain_factor(y: f64) -> f32 {
    let max_y = CLOUD_HEIGHT + ADDITIONAL_CLOUD_HEIGHT;
    if y < max_y - GRADIENT_HEIGHT {
        1.0
    } else if y < max_y {
        ((max_y - y) / GRADIENT_HEIGHT) as f32
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gradient() {
        assert_eq!(rain_factor(100.0), 1.0);
        assert_eq!(rain_factor(300.0), 0.0);
        assert!(rain_factor(192.0) > 0.0 && rain_factor(192.0) < 1.0);
        assert!(above_clouds(193) && !above_clouds(192));
    }
}
