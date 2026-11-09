fn head_in_the_clouds_rain(value: (f32, f32), game: &Game<'_>) -> (f32, f32) {
    // HeadInTheClouds.getRainGradient for the camera: full rain below the
    // clouds, fading over the 6 blocks under cloud height + 3, none above.
    const MAX_Y: f64 = 192.33 + 3.0;
    if game.remote.is_none() {
        return value;
    }
    let Some(eye) = game.eye_position() else { return value };
    let f = if eye[1] < MAX_Y - 6.0 {
        1.0
    } else if eye[1] < MAX_Y {
        ((MAX_Y - eye[1]) / 6.0) as f32
    } else {
        0.0
    };
    (value.0 * f, value.1 * f)
}
