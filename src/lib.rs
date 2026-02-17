use std::collections::HashMap;

use glam::*;
use serde::{Deserialize, Serialize};

pub fn range2d(start: IVec2, end: IVec2) -> impl Iterator<Item = IVec2> {
    (start.x..end.x).flat_map(move |x| (start.y..end.y).map(move |y| IVec2::new(x, y)))
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

pub fn transmission(probe: Vec2, emitter: Vec2) -> f32 {
    let distance = probe.distance(emitter).max(1.0);
    1.0 / distance
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Frustrum {
    pub focus: IVec2,
    pub size: i32,
    pub index: i32,
}
impl Frustrum {
    pub fn lower_dir(&self) -> IVec2 {
        IVec2::new(self.size, self.index)
    }
    pub fn upper_dir(&self) -> IVec2 {
        IVec2::new(self.size, self.index + 1)
    }
    pub fn polygon(&self, min: i32, max: i32) -> Polygon {
        Polygon::new(
            &[
                self.focus + self.lower_dir() * min,
                self.focus + self.lower_dir() * max,
                self.focus + self.upper_dir() * max,
                self.focus + self.upper_dir() * min,
            ]
            .map(|v| v.as_vec2()),
        )
    }
    pub fn parity(&self) -> i32 {
        (self.focus.x / self.size).rem_euclid(2)
    }
}

pub fn merge_odd(frustrum: Frustrum) -> [(Frustrum, f32); 2] {
    let next_size = frustrum.size * 2;
    [
        Frustrum {
            focus: frustrum.focus,
            size: next_size,
            index: frustrum.index * 2,
        },
        Frustrum {
            focus: frustrum.focus,
            size: next_size,
            index: frustrum.index * 2 + 1,
        },
    ]
    .map(|f| (f, 1.0))
}
pub fn merge_even(frustrum: Frustrum) -> [(Frustrum, f32); 4] {
    let next_size = frustrum.size * 2;
    [
        Frustrum {
            focus: frustrum.focus - frustrum.lower_dir(),
            size: next_size,
            index: frustrum.index * 2,
        },
        Frustrum {
            focus: frustrum.focus - frustrum.upper_dir(),
            size: next_size,
            index: frustrum.index * 2 + 1,
        },
        Frustrum {
            focus: frustrum.focus + frustrum.lower_dir(),
            size: next_size,
            index: frustrum.index * 2,
        },
        Frustrum {
            focus: frustrum.focus + frustrum.upper_dir(),
            size: next_size,
            index: frustrum.index * 2 + 1,
        },
    ]
    .map(|f| (f, 0.5))
}
pub fn merge_odd_errors(frustrum: Frustrum) -> Polygon {
    frustrum.polygon(1, 2)
}

pub fn merge_even_errors(frustrum: Frustrum, ext: i32) -> [Polygon; 2] {
    let mid_dir = frustrum.lower_dir() + frustrum.upper_dir();
    let lower = frustrum.focus + frustrum.lower_dir() + mid_dir;
    let upper = frustrum.focus + frustrum.upper_dir() + mid_dir;
    [
        frustrum.polygon(1, 3),
        Polygon::new(
            &[upper, lower, lower + mid_dir * ext, upper + mid_dir * ext].map(|v| v.as_vec2()),
        ),
    ]
}

pub type MergingSchema = HashMap<MergingSchemeKey, MergingScheme>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MergingSchemeKey {
    pub even: bool,
    pub size: i32,
    pub index: i32,
}
pub type MergingScheme = HashMap<IVec2, f32>;

pub fn add_frustrum_weights(
    focus: IVec2,
    multiplier: f32,
    polygon: &Polygon,
    pixel_weights: &mut HashMap<IVec2, f32>,
) {
    let aabb = polygon.bounding_rect();
    let start = aabb.min.floor().as_ivec2();
    let end = aabb.max.ceil().as_ivec2();
    for cell in range2d(start, end + 1) {
        let center = cell.as_vec2();
        let rect = Polygon::from(Rect::center_size(center, Vec2::splat(1.0)));
        let intersection = rect.intersect(polygon);
        if intersection.area() > 0.0001 {
            let weight = transmission(focus.as_vec2(), center) * multiplier * intersection.area();
            *pixel_weights.entry(cell).or_insert(0.0) += weight;
        }
    }
}

pub fn generate_merging_scheme(levels: u32) -> MergingSchema {
    let mut merging_scheme = HashMap::new();
    for level in 0..levels {
        let frustrum_size = 2_i32.pow(level);
        for index in -frustrum_size..frustrum_size {
            let frustrum = Frustrum {
                focus: IVec2::new(0, 0),
                size: frustrum_size,
                index,
            };
            let mut pixel_weights = HashMap::new();
            add_frustrum_weights(
                frustrum.focus,
                1.0,
                &frustrum.polygon(1, 12),
                &mut pixel_weights,
            );
            for (frustrum, weight) in merge_odd(frustrum) {
                add_frustrum_weights(
                    frustrum.focus,
                    -weight,
                    &frustrum.polygon(1, 7),
                    &mut pixel_weights,
                );
            }
            let mask = merge_odd_errors(frustrum);
            pixel_weights.retain(|i, w| {
                let rect = Polygon::from(Rect::center_size(i.as_vec2(), Vec2::splat(1.0)));
                rect.intersect(&mask).area() > 0.0001 && w.abs() > 0.00001
            });
            merging_scheme.insert(
                MergingSchemeKey {
                    even: false,
                    size: frustrum.size,
                    index: frustrum.index,
                },
                pixel_weights,
            );

            let mut pixel_weights = HashMap::new();
            add_frustrum_weights(
                frustrum.focus,
                1.0,
                &frustrum.polygon(1, 12),
                &mut pixel_weights,
            );
            for (frustrum, weight) in merge_even(frustrum) {
                add_frustrum_weights(
                    frustrum.focus,
                    -weight,
                    &frustrum.polygon(1, 7),
                    &mut pixel_weights,
                );
            }
            let mask = merge_even_errors(frustrum, 4);
            pixel_weights.retain(|i, w| {
                let rect = Polygon::from(Rect::center_size(i.as_vec2(), Vec2::splat(1.0)));
                mask.iter().any(|mask| rect.intersect(mask).area() > 0.0001) && w.abs() > 0.00001
            });
            merging_scheme.insert(
                MergingSchemeKey {
                    even: true,
                    size: frustrum.size,
                    index: frustrum.index,
                },
                pixel_weights,
            );
        }
    }
    merging_scheme
}
