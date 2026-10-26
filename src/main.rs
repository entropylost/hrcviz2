use std::cell::RefCell;
use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;
use std::rc::Rc;
use std::sync::Mutex;

use glam::{Affine2, IVec2, Vec2};
use log::debug;
use twilight::frustrum::{ConeFrustrum, Frustrum, merge_even, merge_odd};
use twilight::range2d;
use twilight::scene::{Emitter, RayBundle, Scene};
use twilight::shape::{LineSegment, Polygon, Rect};
use twilight::transmission::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::Closure;
use web_sys::window;

use crate::draw::{DrawContext, lumaa};

pub mod draw;
pub mod input;

type RenderFn = fn(&mut DrawContext, &Scene);

const TRANSMISSIONS: [(&str, RenderFn); 8] = [
    ("Constant1d", render::<Constant1dTransmission>),
    ("Constant", render::<ConstantTransmission>),
    ("Quadratic", render::<QuadraticTransmission>),
    ("QuadraticProjected", render::<QuadraticProjectedTransmission>),
    ("LinearProjected", render::<LinearProjectedTransmission>),
    ("Gradient", render::<GradientTransmission>),
    ("Hessian", render::<HessianTransmission>),
    ("AveragedLinear", render::<AveragedLinearTransmission>),
];
const DEFAULT_TRANSMISSION: &str = "QuadraticProjected";

static FRUSTRUM_START: Mutex<f32> = Mutex::new(2.0);

fn get_radiance_recursive<T: Transmission>(
    frustrum: Frustrum,
    scene: &Scene,
    // mut ctx: Option<&mut DrawContext>,
    cache: &mut HashMap<Frustrum, T>,
) -> T {
    if let Some(&cached) = cache.get(&frustrum) {
        return cached;
    }

    if frustrum.size() > 128 {
        debug!("Frustrum too large: {:?}", frustrum);
        return T::default();
    }
    let frustrum_start = *FRUSTRUM_START.lock().unwrap();
    let frustrum_poly = frustrum.polygon(frustrum_start, 128.0 / frustrum.size() as f32);
    if scene
        .emitters
        .iter()
        .all(|emitter| !frustrum_poly.contains_one_sided(emitter.position))
    {
        return T::default();
    }

    let frustrums = if frustrum.parity() == 0 {
        vec![merge_even(frustrum)]
    } else {
        vec![merge_odd(frustrum, false), merge_odd(frustrum, true)]
    };
    let contributions = frustrums
        .into_iter()
        .map(|next| {
            let contributions = next
                .into_iter()
                .map(|(f, w)| {
                    let near = frustrum.at(frustrum_start);
                    let next_near = f.at(frustrum_start);
                    let projected_near = LineSegment::new(
                        near.project_along(next_near.start, frustrum.ray_dir(next_near.start))
                            .unwrap(),
                        near.project_along(next_near.end, frustrum.ray_dir(next_near.end))
                            .unwrap(),
                    );
                    let bundle = RayBundle {
                        start: projected_near,
                        end: next_near,
                    };
                    let mut emitted_radiance = T::default();
                    for emitter in scene.visible_emitters_within(bundle) {
                        emitted_radiance = emitted_radiance + T::new(frustrum, emitter.position) * emitter.brightness;
                    }
                    let transmittance = scene.trace_bundle(bundle);
                    let next_radiance = get_radiance_recursive(f, scene, cache);
                    (next_radiance.reproject(f, frustrum) * transmittance + emitted_radiance) * w
                })
                .collect::<Vec<_>>();
            // if far {
            //     assert_eq!(contributions.len(), 1, "far overlap fail");
            // }
            // because overlap possible with near
            contributions.into_iter().reduce(|a, b| a + b).unwrap()
        })
        .collect::<Vec<_>>();
    let result = if frustrum.parity() == 0 {
        contributions[0]
    } else {
        contributions[0] * 0.5 + contributions[1] * 0.5
    };

    cache.insert(frustrum, result);
    result
}

fn render<T: Transmission>(ctx: &mut DrawContext, scene: &Scene) {
    let mut caches = HashMap::<_, HashMap<Frustrum, T>>::new();

    let rotate_transform = Affine2::from_translation(Vec2::splat(64.0))
        * Affine2::from_angle(FRAC_PI_2)
        * Affine2::from_translation(Vec2::splat(-64.0));
    let transforms = [
        Affine2::IDENTITY,
        rotate_transform,
        rotate_transform * rotate_transform,
        rotate_transform * rotate_transform * rotate_transform,
    ];
    let scenes = transforms.map(|t| scene.transform(t));

    for cell in range2d(IVec2::splat(1), IVec2::splat(128)) {
        let mut total_radiance = 0.0;
        for (k, (t, scene)) in transforms.iter().zip(&scenes).enumerate() {
            let probe = t.transform_point2(cell.as_vec2());
            assert!((probe - probe.round()).length() < 0.001, "{:?}", probe);
            let probe = probe.round();
            for i in [-1, 0] {
                let frustrum = Frustrum::Cone(ConeFrustrum {
                    focus: probe.as_ivec2(),
                    size: 1,
                    index: i,
                });
                let bundle = RayBundle {
                    start: LineSegment::new(probe, probe),
                    end: frustrum.at(*FRUSTRUM_START.lock().unwrap()),
                };
                let transmission = scene.trace_bundle(bundle);
                let radiance = get_radiance_recursive(frustrum, scene, caches.entry(k).or_default());
                total_radiance += radiance.value() * transmission;
            }
        }
        ctx.rect(Rect::center_size(cell.as_vec2(), Vec2::splat(1.0)))
            .fill(lumaa(total_radiance, 1.0));
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

    let selector = document
        .get_element_by_id("transmission")?
        .dyn_into::<web_sys::HtmlSelectElement>()
        .ok()?;
    for (name, _) in TRANSMISSIONS {
        let option = document
            .create_element("option")
            .ok()?
            .dyn_into::<web_sys::HtmlOptionElement>()
            .ok()?;
        option.set_text(name);
        option.set_selected(name == DEFAULT_TRANSMISSION);
        selector.add_with_html_option_element(&option).ok()?;
    }
    let frustrum_length_input = document
        .get_element_by_id("frustrum-len")
        .unwrap()
        .dyn_into::<web_sys::HtmlInputElement>()
        .unwrap();

    let ctx = DrawContext::new(ctx);

    let mut ctx = ctx.transform(Affine2::from_scale(Vec2::splat(8.0)));

    let mut scene = Scene {
        occluders: vec![Polygon::new(&[
            Vec2::new(60.0, 60.0),
            Vec2::new(63.5, 70.0),
            Vec2::new(56.5, 80.0),
        ])],
        emitters: vec![Emitter {
            brightness: 6.0,
            position: Vec2::new(40.0, 65.0),
        }],
    };
    let disambiguation_offset = Vec2::new(0.0013, 0.0017);

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

        if let Ok(length) = frustrum_length_input.value().parse::<f32>() {
            *FRUSTRUM_START.lock().unwrap() = length;
        }

        let (_, render) = TRANSMISSIONS[selector.selected_index().max(0) as usize];
        let offset_scene = scene.transform(Affine2::from_translation(disambiguation_offset));
        render(&mut ctx, &offset_scene);

        for emitter in &mut scene.emitters {
            ctx.responsive_point(&mut emitter.position);
            ctx.circle(emitter.position, 0.4).fill("red");
        }
        for occluder in &mut scene.occluders {
            ctx.polygon(&occluder.vertices).stroke("red");
            for vertex in &mut occluder.vertices {
                ctx.responsive_point(vertex);
            }
        }

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
