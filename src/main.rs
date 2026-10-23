use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use glam::{Affine2, IVec2, Vec2, Vec2Swizzles};
use hrcviz2::frustrum::{ConeFrustrum, Frustrum, merge_even, merge_odd};
use hrcviz2::range2d;
use hrcviz2::shape::Rect;
use hrcviz2::transmission::{Transmission as _, *};
use log::debug;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;
use web_sys::window;

use crate::draw::{DrawContext, lumaa, rgba};

pub mod draw;
pub mod input;

pub type Transmission = QuadraticProjectedTransmission;

fn get_transmission_recursive(
    frustrum: Frustrum,
    emitter: Vec2,
    mut ctx: Option<&mut DrawContext>,
    cache: &mut HashMap<Frustrum, Transmission>,
) -> Option<Transmission> {
    if let Some(&cached) = cache.get(&frustrum)
        && ctx.is_none()
    {
        return Some(cached);
    }

    if frustrum.size() > 128 {
        debug!("Frustrum too large: {:?}", frustrum);
        return None;
    }
    let frustrum_poly = frustrum.polygon(2, 128 / frustrum.size());
    if !frustrum_poly.contains(emitter) {
        if let Some(ref mut ctx) = ctx {
            ctx.polygon(&frustrum_poly.vertices)
                .fill(rgba(1.0, 0.0, 0.0, 0.1))
                .stroke("red");
        }
        return None;
    }

    let result = if frustrum.parity() == 0 {
        let next = merge_even(frustrum);
        let contributions = next
            .into_iter()
            .filter_map(|(f, w)| {
                let transmission =
                    get_transmission_recursive(f, emitter, ctx.as_deref_mut(), cache);
                transmission.map(|t| t.reproject(f, frustrum) * w)
            })
            .collect::<Vec<_>>();
        if contributions.is_empty() {
            if let Some(ref mut ctx) = ctx {
                ctx.polygon(&frustrum.polygon(1, 2).vertices)
                    .fill(rgba(1.0, 1.0, 0.0, 0.1))
                    .stroke("yellow");
            }
            Transmission::new(frustrum, emitter)
        } else {
            // assert_eq!(contributions.len(), 1, "normal overlap fail");
            contributions.into_iter().reduce(|a, b| a + b).unwrap()
        }
    } else {
        let contributions = [false, true].map(|far| {
            let next = merge_odd(frustrum, far);
            let contributions = next
                .into_iter()
                .filter_map(|(f, w)| {
                    let transmission =
                        get_transmission_recursive(f, emitter, ctx.as_deref_mut(), cache);
                    transmission.map(|t| t.reproject(f, frustrum) * w)
                })
                .collect::<Vec<_>>();
            if contributions.is_empty() {
                // only happens if far
                if let Some(ref mut ctx) = ctx {
                    ctx.polygon(&frustrum.polygon(1, 3).vertices)
                        .fill(rgba(0.0, 1.0, 1.0, 0.1))
                        .stroke("cyan");
                }
                Transmission::new(frustrum, emitter)
            } else {
                // if far {
                //     assert_eq!(contributions.len(), 1, "far overlap fail");
                // }
                // because overlap possible with near
                contributions.into_iter().reduce(|a, b| a + b).unwrap()
            }
        });
        contributions[0] * 0.5 + contributions[1] * 0.5
    };

    cache.insert(frustrum, result);
    Some(result)
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
        Affine2::from_scale(Vec2::splat(8.0)) * Affine2::from_translation(Vec2::splat(0.0)),
    );

    let mut emitter = Vec2::new(40.3, 20.23);

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

        let mut caches = HashMap::new();

        debug!("Emitter: {:?}", emitter);

        for cell in range2d(IVec2::splat(1), IVec2::splat(128)) {
            let offset = emitter - cell.as_vec2();
            let facing = offset.x.abs() > offset.y.abs();
            let (mut probe, mut emitter) = if facing {
                (cell, emitter)
            } else {
                (cell.yx(), emitter.yx())
            };
            let flip = probe.x as f32 > emitter.x;
            if flip {
                probe.x = 128 - probe.x;
                emitter.x = 128.0 - emitter.x;
            }

            for i in [-1, 0] {
                let frustrum = Frustrum::Cone(ConeFrustrum {
                    focus: probe,
                    size: 1,
                    index: i,
                });
                let transmission = get_transmission_recursive(
                    frustrum,
                    emitter,
                    None,
                    caches.entry((facing, flip)).or_default(),
                );
                if let Some(transmission) = transmission {
                    ctx.rect(Rect::center_size(cell.as_vec2(), Vec2::splat(1.0)))
                        .fill(lumaa(6.0 * transmission.value(), 1.0));
                }
            }
        }

        ctx.responsive_point(&mut emitter);
        ctx.circle(emitter, 0.4).fill("red");

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
