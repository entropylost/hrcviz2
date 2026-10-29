use std::collections::HashMap;
use std::f32::consts::FRAC_PI_2;
use std::str::FromStr;

use glam::{Affine2, IVec2, Vec2};
use log::debug;

use crate::frustrum::{ConeFrustrum, Frustrum, merge_even, merge_odd};
use crate::range2d;
use crate::scene::{RayBundle, Scene};
use crate::shape::LineSegment;
use crate::transmission::{self, Transmission};

macro_rules! transmissions {
    ($($name:ident),* $(,)?) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum TransmissionKind {
            $($name),*
        }

        impl TransmissionKind {
            pub const ALL: &[Self] = &[$(Self::$name),*];

            pub fn name(self) -> &'static str {
                match self {
                    $(Self::$name => stringify!($name)),*
                }
            }

            pub fn render(self, scene: &Scene, frustrum_start: f32) -> HashMap<IVec2, f32> {
                match self {
                    $(Self::$name => render::<transmission::$name>(scene, frustrum_start)),*
                }
            }
        }
    };
}

transmissions! {
    Constant1d,
    Constant,
    Quadratic,
    QuadraticProjected,
    LinearProjected,
    Gradient,
    Hessian,
    AveragedLinear,
}

impl Default for TransmissionKind {
    fn default() -> Self {
        Self::QuadraticProjected
    }
}

impl FromStr for TransmissionKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .iter()
            .copied()
            .find(|kind| kind.name() == s)
            .ok_or_else(|| format!("Unknown transmission: {s}"))
    }
}

pub const DEFAULT_FRUSTRUM_START: f32 = 2.0;

// Nudges the scene off the probe grid so that geometry never lies exactly on a frustrum boundary.
const DISAMBIGUATION_OFFSET: Vec2 = Vec2::new(0.0013, 0.0017);

fn get_radiance_recursive<T: Transmission>(
    frustrum: Frustrum,
    scene: &Scene,
    frustrum_start: f32,
    cache: &mut HashMap<Frustrum, T>,
) -> T {
    if let Some(&cached) = cache.get(&frustrum) {
        return cached;
    }

    if frustrum.size() > 128 {
        debug!("Frustrum too large: {:?}", frustrum);
        return T::default();
    }
    let frustrum_poly = frustrum.polygon(frustrum_start, 128.0 / frustrum.size() as f32);
    if scene
        .emitters
        .iter()
        .all(|emitter| !frustrum_poly.contains_one_sided(emitter.position))
    {
        return T::default();
    }

    let near = frustrum.at(frustrum_start);
    let mut contribution = |(f, w): (Frustrum, f32)| {
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
        let next_radiance = get_radiance_recursive(f, scene, frustrum_start, cache);
        (next_radiance.reproject(f, frustrum) * transmittance + emitted_radiance) * w
    };
    let mut sum =
        |merged: Vec<(Frustrum, f32)>| merged.into_iter().map(&mut contribution).reduce(|a, b| a + b).unwrap();
    let result = if frustrum.parity() == 0 {
        sum(merge_even(frustrum))
    } else {
        sum(merge_odd(frustrum, false)) * 0.5 + sum(merge_odd(frustrum, true)) * 0.5
    };

    cache.insert(frustrum, result);
    result
}

// Returns the total radiance arriving at each cell in [1, 128)^2.
pub fn render<T: Transmission>(scene: &Scene, frustrum_start: f32) -> HashMap<IVec2, f32> {
    let scene = scene.transform(Affine2::from_translation(DISAMBIGUATION_OFFSET));
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

    let mut image = HashMap::new();
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
                    end: frustrum.at(frustrum_start),
                };
                let transmission = scene.trace_bundle(bundle);
                let radiance = get_radiance_recursive(frustrum, scene, frustrum_start, caches.entry(k).or_default());
                total_radiance += radiance.value() * transmission;
            }
        }
        image.insert(cell, total_radiance);
    }
    image
}
