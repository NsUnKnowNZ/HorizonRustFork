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
                    std::array::from_fn(|a| self.old[i - 1][a] + self.nodes[i + 1][a] - 2.0 * p[a])
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
            std::array::from_fn(|a| self.old[i][a] + (self.nodes[i][a] - self.old[i][a]) * blend)
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
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stationary_cape_settles_and_stays_anchored() {
        let mut c = Cloth::new(0.0, [0.0; 3], 0.0);
        for tick in 1..201 {
            c.advance(tick as f32, [0.0; 3], 0.0);
        }
        assert_eq!(c.sample(0.0), [0.0, 0.0, 0.025]);
        assert!(c.sample(1.0)[1] > 0.9);
        assert!(c.nodes.iter().flatten().all(|x| x.is_finite()));
        for i in 1..N {
            let len = (0..3)
                .map(|a| (c.nodes[i][a] - c.nodes[i - 1][a]).powi(2))
                .sum::<f32>()
                .sqrt();
            assert!((len - 0.0625).abs() < 0.01);
        }
    }
    #[test]
    fn running_lifts_hem_and_teleport_resets() {
        let mut c = Cloth::new(0.0, [0.0; 3], 0.0);
        for tick in 1..61 {
            c.advance(tick as f32, [0.0, 0.0, tick as f64 * 0.25], 0.0);
        }
        assert!(c.sample(1.0)[2] > 0.2);
        assert!(c.nodes.iter().all(|p| p[2] >= 0.025));
        c.advance(61.0, [100.0; 3], 0.0);
        assert_eq!(c.sample(1.0), [0.0, 1.0, 0.025]);
    }
    #[test]
    fn steady_motion_is_consistent_across_render_rates() {
        let simulate = |frames_per_tick: u32| {
            let mut cloth = Cloth::new(0.0, [0.0; 3], 0.0);
            for frame in 1..=40 * frames_per_tick {
                let time = frame as f32 / frames_per_tick as f32;
                cloth.advance(time, [0.0, 0.0, f64::from(time) * 0.25], 0.0);
            }
            cloth.sample(1.0)
        };
        assert_eq!(simulate(1), simulate(8));
    }

    #[test]
    fn motion_between_steps_is_preserved() {
        let mut a = Cloth::new(0.0, [0.0; 3], 0.0);
        let mut b = Cloth::new(0.0, [0.0; 3], 0.0);
        a.advance(0.125, [0.0, 0.0, 0.125], 0.0);
        a.advance(0.25, [0.0, 0.0, 0.125], 0.0);
        b.advance(0.25, [0.0, 0.0, 0.125], 0.0);
        assert_eq!(a.nodes, b.nodes);
    }

    #[test]
    fn same_frame_is_idempotent() {
        let mut c = Cloth::new(1.0, [0.0; 3], 0.0);
        let before = c.nodes;
        c.advance(1.0, [0.0; 3], 0.0);
        assert_eq!(before, c.nodes);
    }
}
