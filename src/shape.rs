use glam::Vec2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineSegment {
    pub start: Vec2,
    pub end: Vec2,
}
impl LineSegment {
    pub fn new(start: Vec2, end: Vec2) -> Self {
        Self { start, end }
    }
    pub fn direction(&self) -> Vec2 {
        self.end - self.start
    }
    pub fn length(&self) -> f32 {
        self.direction().length()
    }
    pub fn at(&self, t: f32) -> Vec2 {
        self.start + t * self.direction()
    }
    // Returns the intersection point of the two segments, if any.
    // Parallel (including collinear) segments are treated as not intersecting.
    pub fn intersect(&self, other: Self) -> Option<Vec2> {
        self.intersect_t(other).map(|(t, _)| self.at(t))
    }
    // Like `intersect`, but returns the parameter `t` along `self` of the intersection point.
    pub fn intersect_t(&self, other: Self) -> Option<(f32, f32)> {
        let d = self.direction();
        let e = other.direction();
        let denom = d.perp_dot(e);
        if denom == 0.0 {
            return None;
        }
        let r = other.start - self.start;
        let t = r.perp_dot(e) / denom;
        let u = r.perp_dot(d) / denom;
        if (0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u) {
            Some((t, u))
        } else {
            None
        }
    }

    pub fn collinear(&self, other: Self) -> bool {
        const EPSILON: f32 = 1e-6;
        let d = self.direction();
        let e = other.direction();
        d.perp_dot(e).abs() <= EPSILON && (other.start - self.start).perp_dot(d).abs() <= EPSILON
    }
}
impl From<LineSegment> for [Vec2; 2] {
    fn from(segment: LineSegment) -> Self {
        [segment.start, segment.end]
    }
}

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
        Self { vertices: new_vertices }
    }
    pub fn area(&self) -> f32 {
        self.iter_edges()
            .map(|LineSegment { start, end }| start.perp_dot(end))
            .sum::<f32>()
            * 0.5
    }
    pub fn iter_edges(&self) -> impl Iterator<Item = LineSegment> + '_ {
        self.vertices
            .iter()
            .copied()
            .zip(self.vertices.iter().copied().cycle().skip(1))
            .map(|(a, b)| LineSegment::new(a, b))
    }
    pub fn iter_half_planes(&self) -> impl Iterator<Item = HalfPlane> + '_ {
        self.iter_edges().map(HalfPlane::from_line_segment)
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
    // invariant: two polygons sharing one edge should only have one polygon
    // return true to this for a point on the edge.
    pub fn contains_one_sided(&self, point: Vec2) -> bool {
        self.iter_edges().all(|LineSegment { start: a, end: b }| {
            // Evaluate each edge with its endpoints in a canonical order so the
            // polygon on the other side of a shared edge computes exactly the
            // negated value, regardless of rounding.
            let flipped = (a.x, a.y) > (b.x, b.y);
            let (lo, hi) = if flipped { (b, a) } else { (a, b) };
            let side = (hi - lo).perp_dot(point - lo);
            if side == 0.0 {
                // On the edge: only the polygon traversing it lo -> hi takes it.
                !flipped
            } else {
                (side > 0.0) != flipped
            }
        })
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
    pub fn from_line_segment(segment: LineSegment) -> Self {
        Self::from_points(segment.start, segment.end)
    }
    pub fn invert(&self) -> Self {
        Self {
            point: self.point,
            normal: -self.normal,
        }
    }
}
