pub type Quad = ([f32; 3], [[f32; 5]; 4]);

pub const INNER: f32 = -0.2;
pub const OUTER: f32 = 0.55;
pub const MIN_ALPHA: u8 = 8;
pub const MAX_FACES: usize = 24_000;

pub struct Image<'a> {
    pub width: usize,
    pub height: usize,
    pub rgba: &'a [u8],
}

impl Image<'_> {
    pub fn new(width: usize, height: usize, rgba: &[u8]) -> Option<Image<'_>> {
        (width > 0 && height > 0 && rgba.len() == width * height * 4).then_some(Image { width, height, rgba })
    }

    fn solid(&self, x: i64, y: i64) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height && self.rgba[(y as usize * self.width + x as usize) * 4 + 3] > MIN_ALPHA
    }
}

fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i] + b[i])
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}

fn scale(a: [f32; 3], k: f32) -> [f32; 3] {
    a.map(|v| v * k)
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn normalized(a: [f32; 3]) -> [f32; 3] {
    let length = dot(a, a).sqrt();
    if length > 0.0 { scale(a, 1.0 / length) } else { a }
}

fn quad(points: [[f32; 3]; 4], uvs: [[f32; 2]; 4], normal: [f32; 3], winding: f32) -> Quad {
    let facing = dot(cross(sub(points[1], points[0]), sub(points[2], points[0])), normal);
    let mut vertices: [[f32; 5]; 4] = std::array::from_fn(|i| [points[i][0], points[i][1], points[i][2], uvs[i][0], uvs[i][1]]);
    if facing * winding < 0.0 {
        vertices.reverse();
    }
    (normal, vertices)
}

struct Face {
    origin: [f32; 3],
    across: [f32; 3],
    down: [f32; 3],
    start: [f32; 2],
    x: (i64, i64),
    y: (i64, i64),
    normal: [f32; 3],
    winding: f32,
    links: [Option<(usize, usize, bool)>; 4],
}

impl Face {
    fn new((normal, v): &Quad, image: &Image) -> Option<Face> {
        let (w, h) = (image.width as f32, image.height as f32);
        let position = |i: usize| [v[i][0], v[i][1], v[i][2]];
        let (p0, p1, p2) = (position(0), position(1), position(2));
        let (ua, ub, va, vb) = (v[1][3] * w, v[0][3] * w, v[1][4] * h, v[2][4] * h);
        if (ub - ua).abs() < 0.5 || (vb - va).abs() < 0.5 {
            return None;
        }
        let n = normalized(*normal);
        let winding = dot(cross(sub(p1, p0), sub(p2, p0)), n).signum();
        if winding == 0.0 {
            return None;
        }
        Some(Face {
            origin: p1,
            across: scale(sub(p0, p1), 1.0 / (ub - ua)),
            down: scale(sub(p2, p1), 1.0 / (vb - va)),
            start: [ua, va],
            x: (ua.min(ub).round() as i64, ua.max(ub).round() as i64),
            y: (va.min(vb).round() as i64, va.max(vb).round() as i64),
            normal: n,
            winding,
            links: [None; 4],
        })
    }

    fn at(&self, tx: f32, ty: f32) -> [f32; 3] {
        add(self.origin, add(scale(self.across, tx - self.start[0]), scale(self.down, ty - self.start[1])))
    }

    fn inside(&self, tx: i64, ty: i64) -> bool {
        (self.x.0..self.x.1).contains(&tx) && (self.y.0..self.y.1).contains(&ty)
    }

    fn span(&self, side: usize) -> i64 {
        if side % 2 == 0 { self.x.1 - self.x.0 } else { self.y.1 - self.y.0 }
    }

    fn edge(&self, side: usize) -> ([f32; 3], [f32; 3]) {
        let (x0, x1, y0, y1) = (self.x.0 as f32, self.x.1 as f32, self.y.0 as f32, self.y.1 as f32);
        match side {
            0 => (self.at(x0, y0), self.at(x1, y0)),
            1 => (self.at(x1, y0), self.at(x1, y1)),
            2 => (self.at(x0, y1), self.at(x1, y1)),
            _ => (self.at(x0, y0), self.at(x0, y1)),
        }
    }

    fn border(&self, side: usize, index: i64) -> (i64, i64) {
        match side {
            0 => (self.x.0 + index, self.y.0),
            1 => (self.x.1 - 1, self.y.0 + index),
            2 => (self.x.0 + index, self.y.1 - 1),
            _ => (self.x.0, self.y.0 + index),
        }
    }

    fn outward(&self, side: usize) -> [f32; 3] {
        normalized(match side {
            0 => scale(self.down, -1.0),
            1 => self.across,
            2 => self.down,
            _ => scale(self.across, -1.0),
        })
    }
}

fn close(a: [f32; 3], b: [f32; 3]) -> bool {
    let d = sub(a, b);
    dot(d, d) < 1e-6
}

fn link(faces: &mut [Face]) {
    for f in 0..faces.len() {
        for side in 0..4 {
            let (a, b) = faces[f].edge(side);
            let found = (0..faces.len()).filter(|g| *g != f).find_map(|g| {
                (0..4).find_map(|other| {
                    let (c, d) = faces[g].edge(other);
                    let same = faces[g].span(other) == faces[f].span(side);
                    if same && close(a, c) && close(b, d) {
                        Some((g, other, false))
                    } else if same && close(a, d) && close(b, c) {
                        Some((g, other, true))
                    } else {
                        None
                    }
                })
            });
            faces[f].links[side] = found;
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Rim {
    Hidden,
    Wall,
    Seal(usize),
}

fn rim(faces: &[Face], image: &Image, f: usize, side: usize, index: i64, tx: i64, ty: i64) -> Rim {
    let face = &faces[f];
    if !image.solid(tx, ty) {
        return Rim::Hidden;
    }
    let (nx, ny) = match side {
        0 => (tx, ty - 1),
        1 => (tx + 1, ty),
        2 => (tx, ty + 1),
        _ => (tx - 1, ty),
    };
    if face.inside(nx, ny) {
        return if image.solid(nx, ny) { Rim::Hidden } else { Rim::Wall };
    }
    if let Some((g, other, flip)) = face.links[side] {
        let along = if flip { face.span(side) - 1 - index } else { index };
        let (ox, oy) = faces[g].border(other, along);
        if image.solid(ox, oy) {
            return Rim::Seal(g);
        }
    }
    Rim::Wall
}

fn rectangles(face: &Face, image: &Image) -> Vec<(i64, i64, i64, i64)> {
    let (width, height) = ((face.x.1 - face.x.0) as usize, (face.y.1 - face.y.0) as usize);
    let mut used = vec![false; width * height];
    let index = |x: i64, y: i64| (y - face.y.0) as usize * width + (x - face.x.0) as usize;
    let mut out = Vec::new();
    for ty in face.y.0..face.y.1 {
        for tx in face.x.0..face.x.1 {
            let free = |x: i64, y: i64, used: &[bool]| image.solid(x, y) && !used[index(x, y)];
            if !free(tx, ty, &used) {
                continue;
            }
            let mut right = tx + 1;
            while right < face.x.1 && free(right, ty, &used) {
                right += 1;
            }
            let mut bottom = ty + 1;
            while bottom < face.y.1 && (tx..right).all(|x| free(x, bottom, &used)) {
                bottom += 1;
            }
            for y in ty..bottom {
                for x in tx..right {
                    used[index(x, y)] = true;
                }
            }
            out.push((tx, ty, right, bottom));
        }
    }
    out
}

fn rims(faces: &[Face], image: &Image, f: usize, out: &mut Vec<Quad>) {
    let face = &faces[f];
    let (w, h) = (image.width as f32, image.height as f32);
    let uv = |tx: f32, ty: f32| [tx / w, ty / h];
    for side in 0..4 {
        let horizontal = side % 2 == 0;
        let (along, rows) = if horizontal { (face.x, face.y) } else { (face.y, face.x) };
        for row in rows.0..rows.1 {
            let state = |at: i64| {
                let (tx, ty) = if horizontal { (at, row) } else { (row, at) };
                rim(faces, image, f, side, at - along.0, tx, ty)
            };
            let mut at = along.0;
            while at < along.1 {
                let kind = state(at);
                let mut end = at + 1;
                while end < along.1 && state(end) == kind {
                    end += 1;
                }
                let (a, b, middle) = (at as f32, end as f32, row as f32 + 0.5);
                let line = if side == 1 || side == 2 { row as f32 + 1.0 } else { row as f32 };
                let (edge_a, edge_b, uvs) = if horizontal {
                    (face.at(a, line), face.at(b, line), [uv(a, middle), uv(b, middle)])
                } else {
                    (face.at(line, a), face.at(line, b), [uv(middle, a), uv(middle, b)])
                };
                let (outer_a, outer_b) = (add(edge_a, scale(face.normal, OUTER)), add(edge_b, scale(face.normal, OUTER)));
                let spread = [uvs[0], uvs[1], uvs[1], uvs[0]];
                match kind {
                    Rim::Hidden => {}
                    Rim::Wall => {
                        let (inner_a, inner_b) = (add(edge_a, scale(face.normal, INNER)), add(edge_b, scale(face.normal, INNER)));
                        out.push(quad([outer_a, outer_b, inner_b, inner_a], spread, face.outward(side), face.winding));
                    }
                    Rim::Seal(g) => {
                        let lift = scale(faces[g].normal, OUTER);
                        out.push(quad([outer_a, outer_b, add(outer_b, lift), add(outer_a, lift)], spread, face.normal, face.winding));
                    }
                }
                at = end;
            }
        }
    }
}

pub fn voxels(polygons: &[Quad], image: &Image, limit: usize) -> Option<Vec<Quad>> {
    let mut faces: Vec<Face> = polygons.iter().filter_map(|p| Face::new(p, image)).collect();
    link(&mut faces);
    let (w, h) = (image.width as f32, image.height as f32);
    let mut out = Vec::new();
    for (f, face) in faces.iter().enumerate() {
        for (x0, y0, x1, y1) in rectangles(face, image) {
            let (a, b, c, d) = (x0 as f32, y0 as f32, x1 as f32, y1 as f32);
            let cell = [face.at(a, b), face.at(c, b), face.at(c, d), face.at(a, d)];
            let uvs = [[a / w, b / h], [c / w, b / h], [c / w, d / h], [a / w, d / h]];
            out.push(quad(cell.map(|p| add(p, scale(face.normal, OUTER))), uvs, face.normal, face.winding));
        }
        rims(&faces, image, f, &mut out);
        if out.len() > limit {
            return None;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRONT: Quad = ([0.0, 0.0, -1.0], [[4.0, -8.0, -4.0, 0.25, 0.125], [-4.0, -8.0, -4.0, 0.125, 0.125], [-4.0, 0.0, -4.0, 0.125, 0.25], [4.0, 0.0, -4.0, 0.25, 0.25]]);
    const RIGHT: Quad = ([1.0, 0.0, 0.0], [[4.0, -8.0, 4.0, 0.375, 0.125], [4.0, -8.0, -4.0, 0.25, 0.125], [4.0, 0.0, -4.0, 0.25, 0.25], [4.0, 0.0, 4.0, 0.375, 0.25]]);

    fn skin(opaque: &[(usize, usize)]) -> Vec<u8> {
        let mut rgba = vec![0; 64 * 64 * 4];
        for (x, y) in opaque {
            rgba[(y * 64 + x) * 4..][..4].copy_from_slice(&[200, 100, 50, 255]);
        }
        rgba
    }

    fn faces(opaque: &[(usize, usize)]) -> Vec<Quad> {
        let rgba = skin(opaque);
        voxels(&[FRONT], &Image::new(64, 64, &rgba).unwrap(), MAX_FACES).unwrap()
    }

    fn outer_area(out: &[Quad]) -> f32 {
        let span = |v: &[[f32; 5]; 4], i: usize| v.iter().map(|p| p[i]).fold(f32::MIN, f32::max) - v.iter().map(|p| p[i]).fold(f32::MAX, f32::min);
        out.iter().filter(|(n, _)| *n == [0.0, 0.0, -1.0]).map(|(_, v)| span(v, 0) * span(v, 1)).sum()
    }

    #[test]
    fn a_lone_pixel_becomes_a_closed_voxel() {
        let out = faces(&[(12, 12)]);
        assert_eq!(out.len(), 5);
        let (normal, outer) = out[0];
        assert_eq!(normal, [0.0, 0.0, -1.0]);
        assert!(outer.iter().all(|v| (v[2] - (-4.0 - OUTER)).abs() < 1e-5));
        assert!((outer_area(&out) - 1.0).abs() < 1e-5);
    }

    #[test]
    fn a_full_face_is_one_quad_with_four_walls() {
        let every: Vec<(usize, usize)> = (8..16).flat_map(|x| (8..16).map(move |y| (x, y))).collect();
        let out = faces(&every);
        assert_eq!(out.len(), 5);
        assert!((outer_area(&out) - 64.0).abs() < 1e-3);
        let (_, v) = out[0];
        assert_eq!(v.iter().map(|p| p[3]).fold(f32::MIN, f32::max), 0.25);
        assert_eq!(v.iter().map(|p| p[3]).fold(f32::MAX, f32::min), 0.125);
    }

    #[test]
    fn merged_quads_cover_exactly_the_painted_texels() {
        let shape = [(8, 8), (9, 8), (10, 8), (8, 9), (10, 9), (8, 10), (9, 10), (10, 10), (13, 13)];
        let out = faces(&shape);
        assert!((outer_area(&out) - shape.len() as f32).abs() < 1e-3);
        assert!(faces(&[]).is_empty());
        assert!(faces(&[(2, 2)]).is_empty());
    }

    #[test]
    fn a_wall_reads_the_texels_along_its_edge() {
        let out = faces(&[(12, 12), (13, 12)]);
        assert_eq!(out.len(), 5);
        let (_, v) = out.iter().find(|(n, _)| n[1] != 0.0).unwrap();
        let us: Vec<f32> = v.iter().map(|p| p[3]).collect();
        assert!(us.contains(&(12.0 / 64.0)) && us.contains(&(14.0 / 64.0)));
    }

    #[test]
    fn faces_keep_the_source_winding_and_point_out() {
        let source = dot(cross(sub([-4.0, -8.0, -4.0], [4.0, -8.0, -4.0]), sub([-4.0, 0.0, -4.0], [4.0, -8.0, -4.0])), [0.0, 0.0, -1.0]).signum();
        for (normal, v) in faces(&[(12, 12)]) {
            let p = |i: usize| [v[i][0], v[i][1], v[i][2]];
            assert_eq!(dot(cross(sub(p(1), p(0)), sub(p(2), p(0))), normal).signum(), source);
        }
        let sides: Vec<[f32; 3]> = faces(&[(12, 12)]).iter().skip(1).map(|f| f.0).collect();
        assert!(sides.iter().all(|n| n[2] == 0.0 && (dot(*n, *n) - 1.0).abs() < 1e-5));
    }

    #[test]
    fn a_box_edge_painted_on_both_sides_is_sealed() {
        let mut paint: Vec<(usize, usize)> = (8..16).map(|y| (15, y)).collect();
        paint.extend((8..16).map(|y| (16, y)));
        let rgba = skin(&paint);
        let image = Image::new(64, 64, &rgba).unwrap();
        let out = voxels(&[FRONT, RIGHT], &image, MAX_FACES).unwrap();
        let tip = |p: &[f32; 5]| p[0] > 4.0 + OUTER - 1e-4 && p[2] < -4.0 - OUTER + 1e-4;
        assert_eq!(out.iter().filter(|(_, v)| v.iter().filter(|p| tip(p)).count() == 2).count(), 2);
        let open = |n: [f32; 3]| n != [0.0, 0.0, -1.0] && n != [1.0, 0.0, 0.0];
        assert_eq!(out.iter().filter(|(n, v)| open(*n) && v.iter().all(|p| (p[0] - 4.0).abs() < 1e-4)).count(), 0);
    }

    #[test]
    fn oversized_or_broken_input_is_refused() {
        let rgba = skin(&[(10, 10), (12, 12), (14, 14)]);
        assert!(voxels(&[FRONT], &Image::new(64, 64, &rgba).unwrap(), 4).is_none());
        assert!(Image::new(64, 64, &rgba[..100]).is_none());
        assert!(Image::new(0, 64, &[]).is_none());
    }
}
