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

        fn get_frustrum_polygon(probe: IVec2, size: i32, index: i32, extent: i32) -> Polygon {
            let lower_dir = IVec2::new(size, index);
            let upper_dir = IVec2::new(size, index + 1);

            Polygon::new(
                &[
                    probe + lower_dir,
                    probe + lower_dir * extent,
                    probe + upper_dir * extent,
                    probe + upper_dir,
                ]
                .map(|v| v.as_vec2()),
            )
        }

        fn get_transmission_recursive(
            probe: IVec2,
            emitter: Vec2,
            frustrum_size: i32,
            frustrum_index: i32,
            mut ctx: Option<&mut DrawContext>,
        ) -> f32 {
            let source = Polygon::from(Rect::center_size(emitter, Vec2::splat(0.01)));
            let full_frustrum =
                get_frustrum_polygon(probe, frustrum_size, frustrum_index, 64 / frustrum_size);
            let intersection_area = source.intersect(&full_frustrum).area();
            if intersection_area < 0.000001 {
                return 0.0;
            }

            // odd merging
            if (probe.x / frustrum_size).rem_euclid(2) == 0 {
                let traced_frustrum = get_frustrum_polygon(probe, frustrum_size, frustrum_index, 2);
                if let Some(ref mut ctx) = ctx {
                    ctx.polygon(&traced_frustrum.vertices)
                        .fill(rgba(1.0, 1.0, 0.0, 0.3))
                        .stroke("yellow");
                }

                let traced_intersection_area = source.intersect(&traced_frustrum).area();
                let top = get_transmission_recursive(
                    probe,
                    emitter,
                    frustrum_size * 2,
                    frustrum_index * 2,
                    ctx.as_deref_mut(),
                );
                let bottom = get_transmission_recursive(
                    probe,
                    emitter,
                    frustrum_size * 2,
                    frustrum_index * 2 + 1,
                    ctx,
                );
                transmission(probe.as_vec2(), emitter) * traced_intersection_area + top + bottom
            } else {
                if let Some(ref mut ctx) = ctx {
                    ctx.polygon(&full_frustrum.vertices)
                        .fill(rgba(0.0, 1.0, 1.0, 0.3))
                        .stroke("cyan");
                }

                let traced_frustrum = get_frustrum_polygon(probe, frustrum_size, frustrum_index, 3);
                let traced_intersection_area = source.intersect(&traced_frustrum).area();
                if traced_intersection_area > 0.000001 {
                    let actual_area = source
                        .intersect(&get_frustrum_polygon(
                            probe,
                            frustrum_size,
                            frustrum_index,
                            4,
                        ))
                        .area();
                    if let Some(ref mut ctx) = ctx {
                        ctx.polygon(&traced_frustrum.vertices)
                            .fill(rgba(1.0, 0.0, 0.0, 0.3))
                            .stroke("red");
                    }

                    return transmission(probe.as_vec2(), emitter) * actual_area;
                }

                let closer_top = get_transmission_recursive(
                    probe - IVec2::new(frustrum_size, frustrum_index),
                    emitter,
                    frustrum_size * 2,
                    frustrum_index * 2,
                    ctx.as_deref_mut(),
                );
                let closer_bottom = get_transmission_recursive(
                    probe - IVec2::new(frustrum_size, frustrum_index + 1),
                    emitter,
                    frustrum_size * 2,
                    frustrum_index * 2 + 1,
                    ctx.as_deref_mut(),
                );
                let further_top = get_transmission_recursive(
                    probe + IVec2::new(frustrum_size, frustrum_index),
                    emitter,
                    frustrum_size * 2,
                    frustrum_index * 2,
                    ctx.as_deref_mut(),
                );
                let further_bottom = get_transmission_recursive(
                    probe + IVec2::new(frustrum_size, frustrum_index + 1),
                    emitter,
                    frustrum_size * 2,
                    frustrum_index * 2 + 1,
                    ctx,
                );
                (closer_top + closer_bottom + further_top + further_bottom) / 2.0
            }
        }

        ctx.responsive_point(&mut emitter);
        ctx.circle(emitter, 0.2).fill("blue");

        let emitter2 = emitter.as_ivec2();
        for cell in range2d(emitter2 - IVec2::splat(32), emitter2 + IVec2::Y * 16) {
            let transmission = get_transmission_recursive(
                cell, emitter, 1, 0,
                None, // if cell == probe { Some(&mut ctx) } else { None },
            );
            ctx.rect(Rect::center_size(cell.as_vec2(), Vec2::splat(1.0)))
                .fill(lumaa(10.0 * 100.0 * 100.0 * transmission, 1.0));
        }

        /*
        let frustrum_size = 8;
        let extent = 6;
        let target_frustrum = get_frustrum_polygon(probe, frustrum_size, 0, 2 * extent);

        let next_frustrums = [
            (
                probe + IVec2::new(-frustrum_size, 0),
                get_frustrum_polygon(
                    probe + IVec2::new(-frustrum_size, 0),
                    2 * frustrum_size,
                    0,
                    extent,
                ),
            ),
            (
                probe + IVec2::new(-frustrum_size, -1),
                get_frustrum_polygon(
                    probe + IVec2::new(-frustrum_size, -1),
                    2 * frustrum_size,
                    1,
                    extent,
                ),
            ),
            (
                probe + IVec2::new(frustrum_size, 0),
                get_frustrum_polygon(
                    probe + IVec2::new(frustrum_size, 0),
                    2 * frustrum_size,
                    0,
                    extent,
                ),
            ),
            (
                probe + IVec2::new(frustrum_size, 1),
                get_frustrum_polygon(
                    probe + IVec2::new(frustrum_size, 1),
                    2 * frustrum_size,
                    1,
                    extent,
                ),
            ),
        ];
        for (focus, frustrum) in &next_frustrums {
            add_frustrum_weights(*focus, 10.0, frustrum);
        }

        add_frustrum_weights(probe, -20.0, &target_frustrum);

        /*
        for active_frustrum in -8..8 {
            let polygon = get_frustrum_polygon(probe, 8, active_frustrum, 4);

            ctx.polygon(&polygon.vertices)
                .fill(rgba(0.0, 1.0, 1.0, 0.3))
                .stroke("cyan");

            add_frustrum_weights(probe, 5.0, &polygon);

            /*let aabb = polygon.bounding_rect();
            // ctx.rect(aabb).stroke("blue");

            let grid_offset = Vec2::splat(0.0);
            let start = (aabb.min - grid_offset).floor().as_ivec2();
            let end = (aabb.max - grid_offset).ceil().as_ivec2();
            for cell in range2d(start, end + 1) {
                let center = cell.as_vec2() + grid_offset;
                let rect = Polygon::from(Rect::center_size(center, Vec2::splat(1.0)));
                let intersection = rect.intersect(&polygon);
                if intersection.area() > 0.001 {
                    ctx.polygon(&intersection.vertices)
                        .fill(luma(intersection.area()))
                        .stroke("white");
                    // ctx.rect(Rect::center_size(center, Vec2::splat(1.0)))
                    // .fill(lumaa(intersection.area(), 1.0))
                    //     .stroke("white");
                }
            }
            */
        } */

        for (cell, weight) in pixel_weights {
            ctx.rect(Rect::center_size(cell.as_vec2(), Vec2::splat(1.0)))
                .fill(luma(weight.abs()));
        }

        ctx.polygon(&target_frustrum.vertices)
            // .fill(rgba(1.0, 1.0, 0.0, 0.3))
            .stroke("yellow");

        for (focus, frustrum) in &next_frustrums {
            ctx.circle(focus.as_vec2(), 0.1).fill("magenta");
            ctx.polygon(&frustrum.vertices)
                .fill(rgba(0.0, 1.0, 1.0, 0.1))
                .stroke("cyan");
        }
        */

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
