use std::{cell::RefCell, collections::HashMap, rc::Rc};

use glam::{Affine2, IVec2, Vec2};
use log::debug;
use palette::Srgb;
use wasm_bindgen::{JsCast, prelude::Closure};
use web_sys::{CanvasRenderingContext2d, window};

use crate::draw::{DrawContext, luma, lumaa, rgb, rgba};

use hrcviz2::*;

pub mod draw;
pub mod input;

fn get_transmission_recursive(
    frustrum: Frustrum,
    emitter: IVec2,
    merging_scheme: &MergingSchema,
    mut ctx: Option<&mut DrawContext>,
) -> f32 {
    let source = Polygon::from(Rect::center_size(emitter.as_vec2(), Vec2::splat(1.0)));
    let intersection_area = source
        .intersect(&frustrum.polygon(1, 64 / frustrum.size))
        .area();
    if intersection_area < 0.000001 {
        return 0.0;
    }

    let Some(scheme) = merging_scheme.get(&MergingSchemeKey {
        even: frustrum.parity() != 0,
        size: frustrum.size,
        index: frustrum.index,
    }) else {
        return 0.0;
    };

    let adjustment = scheme
        .get(&(emitter - frustrum.focus))
        .copied()
        .unwrap_or(0.0);

    // odd merging
    if frustrum.parity() == 0 {
        let traced_frustrum = frustrum.polygon(1, 2);
        if let Some(ref mut ctx) = ctx {
            ctx.polygon(&traced_frustrum.vertices)
                .fill(rgba(1.0, 1.0, 0.0, 0.1))
                .stroke("yellow");
        }

        // let traced_intersection_area = source.intersect(&traced_frustrum).area();

        let mut merged_radiance = 0.0;
        for (f, w) in merge_odd(frustrum) {
            merged_radiance +=
                get_transmission_recursive(f, emitter, merging_scheme, ctx.as_deref_mut()) * w;
        }

        adjustment + merged_radiance
        // transmission(frustrum.focus.as_vec2(), emitter) * traced_intersection_area + merged_radiance
    } else {
        if let Some(ref mut ctx) = ctx {
            ctx.polygon(&frustrum.polygon(1, 2).vertices)
                .fill(rgba(0.0, 1.0, 1.0, 0.1))
                .stroke("cyan");
        }
        /*
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
        }*/

        let mut merged_radiance = 0.0;
        for (f, w) in merge_even(frustrum) {
            merged_radiance +=
                get_transmission_recursive(f, emitter, merging_scheme, ctx.as_deref_mut()) * w;
        }

        adjustment + merged_radiance
    }
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
        Affine2::from_scale(Vec2::splat(20.0)) * Affine2::from_translation(Vec2::splat(10.0)),
    );

    let mut emitter = Vec2::splat(20.0);

    let mut probe = IVec2::splat(5);

    let mut size_control = IVec2::new(1, 0);

    let merging_scheme =
        postcard::from_bytes::<MergingSchema>(include_bytes!("../merging_scheme.bin")).unwrap();

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

        size_control.x = size_control.x.clamp(1, 5);
        let frustrum_size = 2_i32.pow(size_control.x as u32);
        size_control.y = size_control.y.clamp(-frustrum_size, frustrum_size - 1);

        let frustrum_index = size_control.y;

        ctx.responsive_grid_point(&mut size_control);
        ctx.circle(size_control.as_vec2(), 0.2).fill("green");

        ctx.responsive_grid_point(&mut probe);

        ctx.circle(probe.as_vec2(), 0.2).fill("red");

        ctx.responsive_point(&mut emitter);
        ctx.circle(emitter, 0.2).fill("blue");

        let emitter2 = emitter.as_ivec2();
        for cell in range2d(emitter2 - IVec2::splat(64), emitter2 + IVec2::Y * 16) {
            let transmission = get_transmission_recursive(
                Frustrum {
                    focus: cell,
                    size: 1,
                    index: 0,
                },
                emitter2,
                &merging_scheme,
                None, // if cell == probe { Some(&mut ctx) } else { None },
            );
            ctx.rect(Rect::center_size(cell.as_vec2(), Vec2::splat(1.0)))
                .fill(lumaa(10.0 * transmission, 1.0));
        }

        // let frustrum_size = 4;
        // let extent = IVec2::new(2, 6);
        let target_frustrum = Frustrum {
            focus: probe,
            size: frustrum_size,
            index: frustrum_index,
        };

        /*if let Some(merging_scheme) = merging_scheme.get(&MergingSchemeKey {
            even: true,
            size: frustrum_size,
            index: frustrum_index,
        }) {
            for (pos, weight) in merging_scheme {
                let cell = pos + probe;
                ctx.rect(Rect::center_size(cell.as_vec2(), Vec2::splat(1.0)))
                    .fill(luma(weight * 20.0));
            }
        }*/

        /*
        let mut pixel_weights = HashMap::new();

        let next_frustrums = merge_even(target_frustrum);
        for (frustrum, weight) in &next_frustrums {
            add_frustrum_weights(
                frustrum.focus,
                *weight,
                &frustrum.polygon(2, 8),
                &mut pixel_weights,
            );
        }

        add_frustrum_weights(
            probe,
            -1.0,
            &target_frustrum.polygon(2, 14),
            &mut pixel_weights,
        );

        for (cell, weight) in pixel_weights {
            let a = ctx
                .rect(Rect::center_size(cell.as_vec2(), Vec2::splat(1.0)))
                .fill(luma(weight.abs() * 20.0));
            /*let is_allowed_error = error_polys.iter().any(|poly| {
                Polygon::from(Rect::center_size(cell.as_vec2(), Vec2::splat(1.0)))
                    .intersect(poly)
                    .area()
                    > 0.0001
            });
            if is_allowed_error {
                a.stroke("red");
            }*/
        }

        for poly in merge_even_errors(target_frustrum, 4) {
            ctx.polygon(&poly.vertices)
                //.fill(rgba(1.0, 0.0, 1.0, 0.))
                .stroke("yellow");
        }

        ctx.polygon(&target_frustrum.polygon(2, 4).vertices)
            // .fill(rgba(1.0, 1.0, 0.0, 0.3))
            .stroke("yellow");

        for (frustrum, _weight) in &next_frustrums {
            ctx.circle(frustrum.focus.as_vec2(), 0.1).fill("magenta");
            ctx.polygon(&frustrum.polygon(2, 4).vertices)
                .fill(rgba(0.0, 1.0, 1.0, 0.05))
                .stroke(rgba(0.0, 1.0, 1.0, 0.3));
        }*/

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
