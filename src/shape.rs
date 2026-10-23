use glam::Vec2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    pub min: Vec2,
    pub max: Vec2,
}
impl Rect {
    pub fn min_max(min: Vec2, max: Vec2) -> Self {
        Self { min, max }
    }
    pub fn bounding(a: Vec2, b: Vec2) -> Self {
        Self {
            min: a.min(b),
            max: a.max(b),
        }
    }
    pub fn center_size(center: Vec2, size: Vec2) -> Self {
        let half_size = size * 0.5;
        Self {
            min: center - half_size,
            max: center + half_size,
        }
    }
    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }
}

#[derive(Debug, Clone)]
pub struct Polygon {
    pub vertices: Vec<Vec2>,
}
impl From<Rect> for Polygon {
    fn from(rect: Rect) -> Self {
        Self {
            vertices: vec![
                rect.min,
                Vec2::new(rect.max.x, rect.min.y),
                rect.max,
                Vec2::new(rect.min.x, rect.max.y),
            ],
        }
    }
}
impl Polygon {
    pub fn new(vertices: &[Vec2]) -> Self {
        Self {
            vertices: vertices.to_vec(),
        }
    }
    pub fn bounding_rect(&self) -> Rect {
        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);
        for vertex in &self.vertices {
            min = min.min(*vertex);
            max = max.max(*vertex);
        }
        Rect { min, max }
    }
    pub fn intersect_half_plane(&self, plane: HalfPlane) -> Self {
        let mut new_vertices = Vec::new();
        for i in 0..self.vertices.len() {
            let a = self.vertices[i];
            let b = self.vertices[(i + 1) % self.vertices.len()];
            let da = (a - plane.point).dot(plane.normal);
            let db = (b - plane.point).dot(plane.normal);
            if da >= 0.0 {
                new_vertices.push(a);
            }
            if da * db < 0.0 {
                let t = da / (da - db);
                new_vertices.push(a + t * (b - a));
            }
        }
        Self {
            vertices: new_vertices,
        }
    }
    pub fn area(&self) -> f32 {
        self.iter_edges()
            .map(|(a, b)| a.x * b.y - a.y * b.x)
            .sum::<f32>()
            * 0.5
    }
    pub fn iter_edges(&self) -> impl Iterator<Item = (Vec2, Vec2)> + '_ {
        self.vertices
            .iter()
            .copied()
            .zip(self.vertices.iter().copied().cycle().skip(1))
    }
    pub fn iter_half_planes(&self) -> impl Iterator<Item = HalfPlane> + '_ {
        self.iter_edges().map(|(a, b)| HalfPlane::from_points(a, b))
    }
    pub fn reverse(&self) -> Self {
        let mut vertices = self.vertices.clone();
        vertices.reverse();
        Self { vertices }
    }
    pub fn intersect(&self, other: &Self) -> Self {
        let mut result = self.clone();
        for plane in other.iter_half_planes() {
            result = result.intersect_half_plane(plane);
        }
        result
    }
    // TODO: This is unstable.
    pub fn contains(&self, point: Vec2) -> bool {
        self.iter_half_planes()
            .all(|plane| (point - plane.point).dot(plane.normal) >= 0.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HalfPlane {
    pub point: Vec2,
    // pointing into the filled area
    pub normal: Vec2,
}
impl HalfPlane {
    pub fn new(point: Vec2, normal: Vec2) -> Self {
        Self { point, normal }
    }
    pub fn from_points(a: Vec2, b: Vec2) -> Self {
        let normal = (b - a).perp().normalize();
        Self {
            point: a.midpoint(b),
            normal,
        }
    }
    pub fn invert(&self) -> Self {
        Self {
            point: self.point,
            normal: -self.normal,
        }
    }
}
