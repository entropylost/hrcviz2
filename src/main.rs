use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use glam::{Affine2, IVec2, Vec2};
use hrcviz2::*;
use log::debug;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;
use web_sys::window;

use crate::draw::{DrawContext, lumaa, rgba};

pub mod draw;
pub mod input;

// 1d merge
fn get_transmission_recursive_linear(
    frustrum: LinearFrustrum,
    emitter: IVec2,
    mut ctx: Option<&mut DrawContext>,
) -> f32 {
    let frustrum_poly = frustrum.polygon(1, 128 / frustrum.size);
    let source = Polygon::from(Rect::center_size(emitter.as_vec2(), Vec2::splat(1.0)));

    let intersection_area = source.intersect(&frustrum_poly).area();
    if intersection_area <= 0.0 {
        if let Some(ref mut ctx) = ctx {
            ctx.polygon(&frustrum_poly.vertices)
                .fill(rgba(1.0, 0.0, 0.0, 0.1))
                .stroke("red");
        }
        return 0.0;
    }

    if frustrum.parity() == 0 {
        let traced_frustrum = frustrum.polygon(1, 2);
        let traced_intersection_area = source.intersect(&traced_frustrum).area();
        let next = LinearFrustrum {
            focus: frustrum.focus,
            size: frustrum.size * 2,
            index: frustrum.index * 2.0,
        };

        if let Some(ref mut ctx) = ctx {
            ctx.polygon(&traced_frustrum.vertices)
                .fill(rgba(1.0, 1.0, 0.0, 0.1))
                .stroke("yellow");
        }

        get_transmission_recursive_linear(next, emitter, ctx.as_deref_mut())
            + traced_intersection_area * transmission(frustrum.focus, emitter.as_vec2())
    } else {
        let traced_frustrum = frustrum.polygon(1, 3);
        let traced_intersection_area = source.intersect(&traced_frustrum).area();

        if let Some(ref mut ctx) = ctx {
            ctx.polygon(&traced_frustrum.vertices)
                .fill(rgba(0.0, 1.0, 1.0, 0.1))
                .stroke("cyan");
        }

        let next_size = frustrum.size * 2;

        let next_near = LinearFrustrum {
            focus: frustrum.focus - frustrum.dir(),
            size: next_size,
            index: frustrum.index * 2.0,
        };
        let next_far = LinearFrustrum {
            focus: frustrum.focus + frustrum.dir(),
            size: next_size,
            index: frustrum.index * 2.0,
        };

        0.5 * get_transmission_recursive_linear(next_near, emitter, ctx.as_deref_mut())
            + 0.5
                * (get_transmission_recursive_linear(next_far, emitter, ctx)
                    + traced_intersection_area * transmission(frustrum.focus, emitter.as_vec2()))
    }
}

fn get_transmission_recursive_simple(
    frustrum: Frustrum,
    emitter: IVec2,
    mut ctx: Option<&mut DrawContext>,
    cache: &mut HashMap<(Frustrum, IVec2), f32>,
) -> f32 {
    if let Some(&cached) = cache.get(&(frustrum, emitter))
        && ctx.is_none()
    {
        return cached;
    }

    let frustrum_poly = frustrum.polygon(1, 128 / frustrum.size);
    let source = Polygon::from(Rect::center_size(emitter.as_vec2(), Vec2::splat(1.0)));

    let intersection_area = source.intersect(&frustrum_poly).area();
    if intersection_area <= 0.0 {
        if let Some(ref mut ctx) = ctx {
            ctx.polygon(&frustrum_poly.vertices)
                .fill(rgba(1.0, 0.0, 0.0, 0.1))
                .stroke("red");
        }
        return 0.0;
    }

    let mut merged_radiance = 0.0;
    let adjustment;

    if frustrum.parity() == 0 {
        let traced_frustrum = frustrum.polygon(1, 2);
        let traced_intersection_area = source.intersect(&traced_frustrum).area();
        adjustment =
            traced_intersection_area * transmission(frustrum.focus.as_vec2(), emitter.as_vec2());

        if let Some(ref mut ctx) = ctx {
            ctx.polygon(&traced_frustrum.vertices)
                .fill(rgba(1.0, 1.0, 0.0, 0.1))
                .stroke("yellow");
        }

        for (f, w) in merge_odd(frustrum) {
            merged_radiance +=
                get_transmission_recursive_simple(f, emitter, ctx.as_deref_mut(), cache) * w;
        }
    } else {
        let traced_frustrum = frustrum.polygon(1, 3);
        let traced_intersection_area = source.intersect(&traced_frustrum).area();
        adjustment =
            // further away only
            0.5 * traced_intersection_area * transmission(frustrum.focus.as_vec2(), emitter.as_vec2());
        // adjustment = 0.0;

        if let Some(ref mut ctx) = ctx {
            ctx.polygon(&traced_frustrum.vertices)
                .fill(rgba(0.0, 1.0, 1.0, 0.1))
                .stroke("cyan");
        }

        let next_size = frustrum.size * 2;
        let next_frustrums = [
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
        ];
        let mid_frustrum_near = LinearFrustrum {
            focus: frustrum.focus.as_vec2() - frustrum.dir(),
            size: next_size,
            index: 2.0 * frustrum.dir().y,
        };
        // overlap, subtract
        merged_radiance -=
            0.5 * get_transmission_recursive_linear(mid_frustrum_near, emitter, ctx.as_deref_mut());

        let mid_frustrum_far = LinearFrustrum {
            focus: frustrum.focus.as_vec2() + frustrum.dir(),
            size: next_size,
            index: 2.0 * frustrum.dir().y,
        };

        merged_radiance +=
            0.5 * get_transmission_recursive_linear(mid_frustrum_far, emitter, ctx.as_deref_mut());

        let merges = next_frustrums.map(|f| (f, 0.5));

        for (f, w) in merges {
            merged_radiance +=
                get_transmission_recursive_simple(f, emitter, ctx.as_deref_mut(), cache) * w;
        }
    }

    let result = adjustment + merged_radiance;

    cache.insert((frustrum, emitter), result);
    result
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

    let mut emitter = IVec2::new(40, 20);

    let mut probe = IVec2::new(3, 12);

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

        let mut cache = HashMap::new();

        for cell in range2d(emitter - IVec2::splat(64), emitter + IVec2::Y * 16) {
            let transmission = get_transmission_recursive_simple(
                Frustrum {
                    focus: cell,
                    size: 1,
                    index: 0,
                },
                emitter,
                None,
                &mut cache,
            );
            ctx.rect(Rect::center_size(cell.as_vec2(), Vec2::splat(1.0)))
                .fill(lumaa(10.0 * transmission, 1.0));
        }

        ctx.responsive_grid_point(&mut probe);

        ctx.circle(probe.as_vec2(), 0.2).fill("red");

        ctx.responsive_grid_point(&mut emitter);
        ctx.circle(emitter.as_vec2(), 0.2).fill("blue");

        get_transmission_recursive_simple(
            Frustrum {
                focus: probe,
                size: 1,
                index: 0,
            },
            emitter,
            Some(&mut ctx),
            &mut cache,
        );

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
