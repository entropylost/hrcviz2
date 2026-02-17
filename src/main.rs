use std::{cell::RefCell, collections::HashMap, rc::Rc};

use glam::{Affine2, IVec2, Vec2};
use log::debug;
use palette::Srgb;
use wasm_bindgen::{JsCast, prelude::Closure};
use web_sys::{CanvasRenderingContext2d, window};

use crate::draw::{DrawContext, Rect, luma, lumaa, rgb, rgba};

pub mod draw;
pub mod input;

fn range2d(start: IVec2, end: IVec2) -> impl Iterator<Item = IVec2> {
    (start.x..end.x).flat_map(move |x| (start.y..end.y).map(move |y| IVec2::new(x, y)))
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
    fn new(vertices: &[Vec2]) -> Self {
        Self {
            vertices: vertices.to_vec(),
        }
    }
    fn bounding_rect(&self) -> Rect {
        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);
        for vertex in &self.vertices {
            min = min.min(*vertex);
            max = max.max(*vertex);
        }
        Rect { min, max }
    }
    fn intersect_half_plane(&self, plane: HalfPlane) -> Self {
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
    fn area(&self) -> f32 {
        self.iter_edges()
            .map(|(a, b)| a.x * b.y - a.y * b.x)
            .sum::<f32>()
            * 0.5
    }
    fn iter_edges(&self) -> impl Iterator<Item = (Vec2, Vec2)> + '_ {
        self.vertices
            .iter()
            .copied()
            .zip(self.vertices.iter().copied().cycle().skip(1))
    }
    fn iter_half_planes(&self) -> impl Iterator<Item = HalfPlane> + '_ {
        self.iter_edges().map(|(a, b)| HalfPlane::from_points(a, b))
    }
    fn reverse(&self) -> Self {
        let mut vertices = self.vertices.clone();
        vertices.reverse();
        Self { vertices }
    }
    fn intersect(&self, other: &Self) -> Self {
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

fn transmission(probe: Vec2, emitter: Vec2) -> f32 {
    let distance = probe.distance(emitter).max(1.0);
    1.0 / distance
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Frustrum {
    focus: IVec2,
    size: i32,
    index: i32,
}
impl Frustrum {
    fn lower_dir(&self) -> IVec2 {
        IVec2::new(self.size, self.index)
    }
    fn upper_dir(&self) -> IVec2 {
        IVec2::new(self.size, self.index + 1)
    }
    fn polygon(&self, min: i32, max: i32) -> Polygon {
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
    fn parity(&self) -> i32 {
        (self.focus.x / self.size).rem_euclid(2)
    }
}

fn merge_odd(frustrum: Frustrum) -> [(Frustrum, f32); 2] {
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
fn merge_even(frustrum: Frustrum) -> [(Frustrum, f32); 4] {
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

fn run() -> Option<()> {
    let document = window()?.document()?;

    let canvas = document
        .get_element_by_id("main")?
        .dyn_into::<web_sys::HtmlCanvasElement>()
        .ok()?;

    let ctx = canvas
        .get_context("2d")
        .ok()??
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .ok()?;

    let ctx = DrawContext::new(ctx);

    let mut ctx = ctx.transform(
        Affine2::from_scale(Vec2::splat(20.0)) * Affine2::from_translation(Vec2::splat(4.0)),
    );

    let mut emitter = Vec2::splat(20.0);

    let mut probe = Vec2::splat(5.0);

    anim_loop(move || {
        let window_width = window().unwrap().inner_width().unwrap().as_f64().unwrap() as u32;
        let window_height = window().unwrap().inner_height().unwrap().as_f64().unwrap() as u32;
        if canvas.width() != window_width || canvas.height() != window_height {
            debug!("Resizing canvas to {window_width}x{window_height}");
            canvas.set_width(window_width);
            canvas.set_height(window_height);
            ctx.update_transform();
        }

        ctx.set_line_width(0.1);

        ctx.clear();

        ctx.responsive_point(&mut probe);

        let probe = probe.round().as_ivec2();

        ctx.circle(probe.as_vec2(), 0.2).fill("red");

        let mut pixel_weights = HashMap::<IVec2, f32>::new();

        let mut add_frustrum_weights = |focus: IVec2, multiplier: f32, polygon: &Polygon| {
            let aabb = polygon.bounding_rect();
            let start = aabb.min.floor().as_ivec2();
            let end = aabb.max.ceil().as_ivec2();
            for cell in range2d(start, end + 1) {
                let center = cell.as_vec2();
                let rect = Polygon::from(Rect::center_size(center, Vec2::splat(1.0)));
                let intersection = rect.intersect(polygon);
                if intersection.area() > 0.0001 {
                    let weight =
                        transmission(focus.as_vec2(), center) * multiplier * intersection.area();
                    *pixel_weights.entry(cell).or_insert(0.0) += weight;
                }
            }
        };

        fn get_transmission_recursive(
            frustrum: Frustrum,
            emitter: Vec2,
            mut ctx: Option<&mut DrawContext>,
        ) -> f32 {
            let source = Polygon::from(Rect::center_size(emitter, Vec2::splat(1.0)));
            let intersection_area = source
                .intersect(&frustrum.polygon(1, 64 / frustrum.size))
                .area();
            if intersection_area < 0.000001 {
                return 0.0;
            }

            // odd merging
            if frustrum.parity() == 0 {
                let traced_frustrum = frustrum.polygon(1, 2);
                if let Some(ref mut ctx) = ctx {
                    ctx.polygon(&traced_frustrum.vertices)
                        .fill(rgba(1.0, 1.0, 0.0, 0.1))
                        .stroke("yellow");
                }

                let traced_intersection_area = source.intersect(&traced_frustrum).area();

                let mut merged_radiance = 0.0;
                for (f, w) in merge_odd(frustrum) {
                    merged_radiance +=
                        get_transmission_recursive(f, emitter, ctx.as_deref_mut()) * w;
                }

                transmission(frustrum.focus.as_vec2(), emitter) * traced_intersection_area
                    + merged_radiance
            } else {
                if let Some(ref mut ctx) = ctx {
                    ctx.polygon(&frustrum.polygon(1, 2).vertices)
                        .fill(rgba(0.0, 1.0, 1.0, 0.1))
                        .stroke("cyan");
                }

                let traced_frustrum = frustrum.polygon(1, 3);
                let traced_intersection_area = source.intersect(&traced_frustrum).area();
                if traced_intersection_area > 0.000001 {
                    let actual_area = source.intersect(&frustrum.polygon(1, 4)).area();
                    if let Some(ref mut ctx) = ctx {
                        ctx.polygon(&traced_frustrum.vertices)
                            .fill(rgba(1.0, 0.0, 0.0, 0.1))
                            .stroke("red");
                    }

                    return transmission(frustrum.focus.as_vec2(), emitter) * actual_area;
                }

                let mut merged_radiance = 0.0;
                for (f, w) in merge_even(frustrum) {
                    merged_radiance +=
                        get_transmission_recursive(f, emitter, ctx.as_deref_mut()) * w;
                }

                merged_radiance
            }
        }

        ctx.responsive_point(&mut emitter);
        ctx.circle(emitter, 0.2).fill("blue");

        let emitter2 = emitter.as_ivec2();
        for cell in range2d(emitter2 - IVec2::splat(32), emitter2 + IVec2::Y * 16) {
            let transmission = get_transmission_recursive(
                Frustrum {
                    focus: cell,
                    size: 1,
                    index: 0,
                },
                emitter,
                if cell == probe { Some(&mut ctx) } else { None },
            );
            ctx.rect(Rect::center_size(cell.as_vec2(), Vec2::splat(1.0)))
                .fill(lumaa(10.0 * transmission, 0.2));
        }

        /*
        let frustrum_size = 8;
        let extent = IVec2::new(2, 6);
        let target_frustrum = get_frustrum_polygon(probe, frustrum_size, 0, IVec2::new(2, 12));

        let weighted_frustrum = |offset: IVec2, index: i32, weight: f32| {
            (
                probe + offset,
                get_frustrum_polygon(probe + offset, 2 * frustrum_size, index, extent),
                weight,
            )
        };

        let next_frustrums = [
            weighted_frustrum(IVec2::new(-frustrum_size, 0), 0, 0.5),
            weighted_frustrum(IVec2::new(-frustrum_size, -1), 1, 0.5),
            weighted_frustrum(IVec2::new(frustrum_size, 0), 0, 0.5),
            weighted_frustrum(IVec2::new(frustrum_size, 1), 1, 0.5),
        ];
        for (focus, frustrum, weight) in &next_frustrums {
            add_frustrum_weights(*focus, *weight, frustrum);
        }

        add_frustrum_weights(probe, -1.0, &target_frustrum);

        for (cell, weight) in pixel_weights {
            let a = ctx
                .rect(Rect::center_size(cell.as_vec2(), Vec2::splat(1.0)))
                .fill(luma(weight.abs() * 20.0));
            if weight.abs() > 1.0 / 1000.0 {
                a.stroke("red");
            }
        }

        ctx.polygon(&target_frustrum.vertices)
            // .fill(rgba(1.0, 1.0, 0.0, 0.3))
            .stroke("yellow");

        for (focus, frustrum, _) in &next_frustrums {
            ctx.circle(focus.as_vec2(), 0.1).fill("magenta");
            ctx.polygon(&frustrum.vertices)
                // .fill(rgba(0.0, 1.0, 1.0, 0.05))
                .stroke(rgba(0.0, 1.0, 1.0, 0.3));
        } */

        ctx.finish();
    });

    Some(())
}

fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    window()
        .unwrap()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("failed to register `requestAnimationFrame`");
}

fn anim_loop(mut f: impl FnMut() + 'static) {
    let closure_ref = Rc::new(RefCell::new(None));
    let closure_ref_2 = closure_ref.clone();

    *closure_ref.borrow_mut() = Some(Closure::new(move || {
        f();
        request_animation_frame(closure_ref_2.borrow().as_ref().unwrap());
    }));

    request_animation_frame(closure_ref.borrow().as_ref().unwrap());
}

fn main() {
    wasm_logger::init(wasm_logger::Config::default());
    console_error_panic_hook::set_once();
    run().unwrap();
}
