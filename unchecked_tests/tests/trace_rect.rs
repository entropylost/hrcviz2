use std::f32::consts::TAU;

use glam::Vec2;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};
use twilight::scene::{RayBundle, Scene};
use twilight::shape::{LineSegment, Polygon};

fn random_point(rng: &mut StdRng) -> Vec2 {
    Vec2::new(rng.random_range(0.0..10.0), rng.random_range(0.0..10.0))
}

// A random convex polygon with counterclockwise winding.
fn random_convex_polygon(rng: &mut StdRng) -> Polygon {
    let center = random_point(rng);
    let radius = rng.random_range(0.5..2.5);
    let n = rng.random_range(3..7);
    let mut angles: Vec<f32> = (0..n).map(|_| rng.random_range(0.0..TAU)).collect();
    angles.sort_by(f32::total_cmp);
    let vertices: Vec<Vec2> = angles.iter().map(|a| center + radius * Vec2::from_angle(*a)).collect();
    Polygon::new(&vertices)
}

// Compares the analytic trace_rect against dense sampling of trace.
#[test]
fn trace_rect_matches_sampling() {
    let mut rng = StdRng::seed_from_u64(12345);
    let samples = 20000;
    let mut worst: f32 = 0.0;
    let mut partial = 0;
    for case in 0..300 {
        let scene = Scene {
            occluders: (0..3).map(|_| random_convex_polygon(&mut rng)).collect(),
            emitters: vec![],
        };
        let bundle = RayBundle {
            start: LineSegment::new(random_point(&mut rng), random_point(&mut rng)),
            end: LineSegment::new(random_point(&mut rng), random_point(&mut rng)),
        };

        let exact = scene.trace_bundle(bundle);
        let sampled = (0..samples)
            .filter(|i| {
                let t = (*i as f32 + 0.5) / samples as f32;
                scene.trace(bundle.at(t))
            })
            .count() as f32
            / samples as f32;

        let error = (exact - sampled).abs();
        if error > 1e-3 {
            println!("case {case}: exact {exact}, sampled {sampled}");
        }
        if exact > 0.01 && exact < 0.99 {
            partial += 1;
        }
        worst = worst.max(error);
    }
    println!("worst error {worst}, {partial} partially occluded cases");
    assert!(worst < 1e-3);
}
