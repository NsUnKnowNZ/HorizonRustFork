fn smoothie_cape(
    m: &Mob,
    alpha: f32,
    base: glam::Mat4,
    uv: [f32; 4],
    light: u32,
    data: &MobData,
    out: &mut Vec<Vertex>,
) {
    const N: usize = 17;
    const STEP: f32 = 0.25;
    struct Cloth {
        nodes: [[f32; 3]; N],
        old: [[f32; 3]; N],
        time: f32,
        remainder: f32,
        wind_phase: f32,
        motion: [f32; 4],
        water: f32,
        position: [f64; 3],
        yaw: f32,
    }
    impl Cloth {
        fn new(time: f32, position: [f64; 3], yaw: f32) -> Self {
            let nodes = std::array::from_fn(|i| [0.0, i as f32 / (N - 1) as f32, 0.025]);
            Self {
                nodes,
                old: nodes,
                time,
                remainder: 0.0,
                wind_phase: time * 0.12,
                motion: [0.0; 4],
                water: 0.0,
                position,
                yaw,
            }
        }
        #[cfg(test)]
        fn advance(&mut self, time: f32, position: [f64; 3], yaw: f32) {
            self.advance_with_pose(time, position, yaw, 0.0, 0.0);
        }
        fn advance_with_pose(
            &mut self,
            time: f32,
            position: [f64; 3],
            yaw: f32,
            swim: f32,
            glide: f32,
        ) {
            let elapsed = time - self.time;
            let delta = std::array::from_fn::<_, 3, _>(|i| (position[i] - self.position[i]) as f32);
            if elapsed < 0.0 || elapsed > 10.0 || delta.iter().any(|d| d.abs() > 4.0) {
                *self = Self::new(time, position, yaw);
                return;
            }
            if elapsed < 0.001 {
                return;
            }
            let turn = (yaw - self.yaw + 180.0).rem_euclid(360.0) - 180.0;
            let motion = [delta[0], delta[1], delta[2], turn];
            self.time = time;
            self.position = position;
            self.yaw = yaw;

            let mut remaining = elapsed;
            let dt = STEP * 0.05;
            while remaining > 0.0 {
                let needed = STEP - self.remainder;
                let consumed = remaining.min(needed);
                for (total, distance) in self.motion.iter_mut().zip(motion) {
                    *total += distance * (consumed / elapsed);
                }
                if remaining < needed {
                    self.remainder += consumed;
                    break;
                }
                remaining -= consumed;
                self.remainder = 0.0;
                let [x, y, z, turn] = self.motion.map(|distance| distance / STEP);
                self.motion = [0.0; 4];
                let (sin, cos) = yaw.to_radians().sin_cos();
                let forward = -x * sin + z * cos;
                let lateral = x * cos + z * sin;
                self.water += (swim - self.water) * 0.14;
                let damping = (-(7.0 + self.water * 14.0) * dt).exp();
                let flow = 1.0 - 0.65 * self.water;
                let side = (-lateral * 26.0 - turn * 0.045).clamp(-7.0, 7.0) * flow;
                let gravity = (9.8 + (y * 15.0).clamp(-8.0, 8.0))
                    * (1.0 - 0.65 * glide - 0.45 * self.water).max(0.2);
                let drag = 0.8 + forward.clamp(-0.12, 0.65) * 42.0 * flow + glide * 8.0;
                self.wind_phase = (self.wind_phase + STEP * (0.12 - self.water * 0.07))
                    .rem_euclid(std::f32::consts::TAU);
                for i in 1..N {
                    let p = self.nodes[i];
                    let depth = i as f32 / (N - 1) as f32;
                    let (wave, crosswind) = (self.wind_phase - depth * 4.0).sin_cos();
                    let bend = if i + 1 < N {
                        std::array::from_fn(|a| {
                            self.old[i - 1][a] + self.nodes[i + 1][a] - 2.0 * p[a]
                        })
                    } else {
                        [0.0; 3]
                    };
                    let forces = [
                        side + crosswind * depth * 0.65 * flow + bend[0] * 240.0,
                        gravity + bend[1] * 240.0,
                        drag + wave * depth * 1.1 * flow + bend[2] * 240.0,
                    ];
                    for axis in 0..3 {
                        self.nodes[i][axis] +=
                            (p[axis] - self.old[i][axis]) * damping + forces[axis] * dt * dt;
                    }
                    self.old[i] = p;
                }
                for _ in 0..7 {
                    self.nodes[0] = [0.0, 0.0, 0.025];
                    for i in 1..N {
                        let d: [f32; 3] =
                            std::array::from_fn(|a| self.nodes[i][a] - self.nodes[i - 1][a]);
                        let len = d.iter().map(|x| x * x).sum::<f32>().sqrt().max(0.00001);
                        let error = (len - 1.0 / (N - 1) as f32) / len;
                        for (a, value) in d.iter().enumerate() {
                            let correction = value * error;
                            if i == 1 {
                                self.nodes[i][a] -= correction;
                            } else {
                                self.nodes[i][a] -= correction * 0.5;
                                self.nodes[i - 1][a] += correction * 0.5;
                            }
                        }
                        self.nodes[i][1] = self.nodes[i][1].max(self.nodes[i - 1][1] + 0.005);
                        self.nodes[i][2] = self.nodes[i][2].max(0.025);
                    }
                }
            }
        }
        fn interpolated(&self) -> [[f32; 3]; N] {
            let blend = self.remainder / STEP;
            std::array::from_fn(|i| {
                std::array::from_fn(|a| {
                    self.old[i][a] + (self.nodes[i][a] - self.old[i][a]) * blend
                })
            })
        }
        #[cfg(test)]
        fn sample(&self, depth: f32) -> [f32; 3] {
            sample(&self.interpolated(), depth)
        }
    }
    fn sample(nodes: &[[f32; 3]; N], depth: f32) -> [f32; 3] {
        let index = depth.clamp(0.0, 1.0) * (N - 1) as f32;
        let lo = (index as usize).min(N - 2);
        let t = index - lo as f32;
        std::array::from_fn(|a| nodes[lo][a] * (1.0 - t) + nodes[lo + 1][a] * t)
    }

    let Some(model) = m.kind.layer("player#cape") else {
        return;
    };
    let s = anim::state(m, alpha);
    let mut poses = Poses::new(model);
    anim::setup(m, &s, &mut poses, &data.animations);
    let a = f64::from(alpha);
    let at = [
        m.previous.x + (m.position.x - m.previous.x) * a,
        m.previous.y + (m.position.y - m.previous.y) * a,
        m.previous.z + (m.position.z - m.previous.z) * a,
    ];
    let body = m.previous_body_yaw + wrap(m.body_yaw - m.previous_body_yaw) * alpha;
    let ticks = m.extra.fall_fly_ticks as f32 + alpha;
    let gliding = (ticks * ticks / 100.0).clamp(0.0, 1.0);
    let swim = if m.extra.swim.in_water { 1.0 } else { 0.0 };
    let start = out.len();
    draw(model, base, &poses, uv, 0xFFFF_FFFF, light, 0, out);
    let end = out.len();
    if end == start || (end - start) % 4 != 0 {
        return;
    }
    let inverse = base.inverse();
    let (top, bottom) =
        out[start..end]
            .iter()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), v| {
                let y = inverse.transform_point3(glam::Vec3::from_array(v.pos)).y;
                (lo.min(y), hi.max(y))
            });
    let height = (bottom - top).max(0.001);
    let time = m.age as f32 + alpha;
    static CLOTH: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<u32, Cloth>>> =
        std::sync::OnceLock::new();
    let cloth = {
        let mut cache = CLOTH
            .get_or_init(Default::default)
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if cache.len() > 256 {
            cache.clear();
        }
        let state = cache
            .entry(m.id)
            .or_insert_with(|| Cloth::new(time, at, body));
        state.advance_with_pose(time, at, body, swim, gliding);
        state.interpolated()
    };
    for offset in (start..end).step_by(4) {
        let mut quad: [Vertex; 4] = out[offset..offset + 4].try_into().unwrap();
        let mut corners = quad.map(|v| inverse.transform_point3(glam::Vec3::from_array(v.pos)));
        if (corners[1].y - corners[0].y).abs() > height * 0.5 {
            quad.rotate_left(1);
            corners.rotate_left(1);
        }
        let strips = if (corners[3].y - corners[0].y).abs() > height * 0.5 {
            N - 1
        } else {
            1
        };
        let edge = |from: usize, to: usize, t: f32| {
            let mut vertex = quad[from];
            let mut local = corners[from].lerp(corners[to], t);
            let depth = ((local.y - top) / height).clamp(0.0, 1.0);
            let chain = sample(&cloth, depth);
            local.x += chain[0] * height;
            local.y = top + chain[1] * height;
            local.z += (chain[2] - 0.025) * height;
            vertex.pos = base.transform_point3(local).to_array();
            vertex.uv = glam::Vec2::from_array(quad[from].uv)
                .lerp(glam::Vec2::from_array(quad[to].uv), t)
                .to_array();
            vertex
        };
        let [nx, ny, nz, ..] = quad[0].normal;
        let original = glam::Vec3::new(f32::from(nx), f32::from(ny), f32::from(nz));
        let normal = |left: Vertex, right: Vertex, next: Vertex| {
            let a = glam::Vec3::from_array(left.pos);
            let b = glam::Vec3::from_array(right.pos);
            let d = glam::Vec3::from_array(next.pos);
            let n = (b - a).cross(d - a).normalize_or_zero();
            if n.dot(original) < 0.0 { -n } else { n }
        };
        let pack = |n: glam::Vec3| {
            n.normalize_or_zero()
                .to_array()
                .map(|v| (v * 127.0).round() as i8)
        };
        let mut left = edge(0, 3, 0.0);
        let mut right = edge(1, 2, 0.0);
        let mut next_left = edge(0, 3, 1.0 / strips as f32);
        let mut next_right = edge(1, 2, 1.0 / strips as f32);
        let mut current = normal(left, right, next_left);
        let mut start_normal = pack(current);
        for row in 1..=strips {
            let (after_left, after_right, following) = if row < strips {
                let t = (row + 1) as f32 / strips as f32;
                let after_left = edge(0, 3, t);
                (
                    after_left,
                    edge(1, 2, t),
                    normal(next_left, next_right, after_left),
                )
            } else {
                (next_left, next_right, current)
            };
            let end_normal = pack(current + following);
            if original.length_squared() > 0.0 {
                left.normal[..3].copy_from_slice(&start_normal);
                right.normal[..3].copy_from_slice(&start_normal);
                next_left.normal[..3].copy_from_slice(&end_normal);
                next_right.normal[..3].copy_from_slice(&end_normal);
            }
            out.extend([left, right, next_right, next_left]);
            left = next_left;
            right = next_right;
            next_left = after_left;
            next_right = after_right;
            current = following;
            start_normal = end_normal;
        }
    }
    out.drain(start..end);
}
