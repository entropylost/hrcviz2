use glam::Vec2;

use crate::shape::{LineSegment, Polygon};

#[derive(Debug, Clone, Copy)]
pub struct Emitter {
    pub brightness: f32,
    pub position: Vec2,
}

#[derive(Debug, Clone)]
pub struct Scene {
    pub occluders: Vec<Polygon>,
    pub emitters: Vec<Emitter>,
}

impl Scene {
    // Returns true if the line segment is not blocked by any occluder.
    pub fn trace(&self, start: Vec2, end: Vec2) -> bool {
        let ray = LineSegment::new(start, end);
        self.occluders.iter().all(|occluder| {
            // If the ray crosses no edge it is either entirely inside or entirely outside the occluder.
            occluder.iter_edges().all(|edge| ray.intersect(edge).is_none()) && !occluder.contains(start)
        })
    }
    // Traces all lines start.at(t) to end.at(t) for t in [0, 1] and returns the amount of the segment that is unblocked.
    // The result is measured in t, so it lies in [0, 1].
    pub fn trace_rect(&self, start: LineSegment, end: LineSegment) -> f32 {
        // Whether the line P(t) -> Q(t) is blocked can only change at a t where an endpoint crosses an occluder edge,
        // or where the line sweeps over an occluder vertex. Visibility is constant between consecutive such events,
        // so tracing a single line per interval gives the exact answer.
        let mut events = vec![0.0, 1.0];

        // P(t) = start.start + t * d, Q(t) - P(t) = w + t * k.
        let d = start.direction();
        let w = end.start - start.start;
        let k = end.direction() - d;

        for occluder in &self.occluders {
            for edge in occluder.iter_edges() {
                events.extend(start.intersect_t(edge).map(|(t, _)| t));
                events.extend(end.intersect_t(edge).map(|(t, _)| t));
            }
            for &vertex in &occluder.vertices {
                // The line passes through the vertex when (Q(t) - P(t)) × (vertex - P(t)) = 0, and with
                // vertex - P(t) = c - t * d this expands to a quadratic in t.
                let c = vertex - start.start;
                let qa = -k.perp_dot(d);
                let qb = k.perp_dot(c) - w.perp_dot(d);
                let qc = w.perp_dot(c);
                events.extend(solve_quadratic(qa, qb, qc));
            }
        }

        // This also discards the NaN / infinite roots of degenerate quadratics.
        events.retain(|t| (0.0..=1.0).contains(t));
        events.sort_by(f32::total_cmp);

        events
            .windows(2)
            .filter(|interval| {
                let t = (interval[0] + interval[1]) * 0.5;
                self.trace(start.at(t), end.at(t))
            })
            .map(|interval| interval[1] - interval[0])
            .sum()
    }
}

// Roots of a * t^2 + b * t + c = 0. Missing roots (no real solutions, or a degenerate equation) come back as NaN or
// infinity. Uses the numerically stable form, which also handles a = 0 without a special case.
fn solve_quadratic(a: f32, b: f32, c: f32) -> [f32; 2] {
    let q = -0.5 * (b + b.signum() * (b * b - 4.0 * a * c).sqrt());
    [q / a, c / q]
}
