use std::ops::{Add, Mul};

use glam::{Mat2, Vec2};

use crate::frustrum::Frustrum;

pub trait Transmission: Copy + Add<Output = Self> + Mul<f32, Output = Self> + Default {
    fn value(self) -> f32;
    fn new(frustrum: Frustrum, emitter: Vec2) -> Self;
    fn reproject(self, source: Frustrum, target: Frustrum) -> Self;
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Constant1dTransmission {
    pub value: f32,
}
impl Transmission for Constant1dTransmission {
    fn value(self) -> f32 {
        self.value
    }
    fn new(frustrum: Frustrum, emitter: Vec2) -> Self {
        Self {
            value: 1.0 / (emitter.x - frustrum.focus().x).abs(),
        }
    }
    fn reproject(self, _source: Frustrum, _target: Frustrum) -> Self {
        self
    }
}
impl Add for Constant1dTransmission {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            value: self.value + other.value,
        }
    }
}
impl Mul<f32> for Constant1dTransmission {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self {
            value: self.value * rhs,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ConstantTransmission {
    pub value: f32,
}
impl Transmission for ConstantTransmission {
    fn value(self) -> f32 {
        self.value
    }
    fn new(frustrum: Frustrum, emitter: Vec2) -> Self {
        Self {
            value: 1.0 / (emitter - frustrum.focus()).length(),
        }
    }
    fn reproject(self, _source: Frustrum, _target: Frustrum) -> Self {
        self
    }
}
impl Add for ConstantTransmission {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            value: self.value + other.value,
        }
    }
}
impl Mul<f32> for ConstantTransmission {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self {
            value: self.value * rhs,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct QuadraticTransmission {
    pub value: f32,
    // along frustrum direction; we don't need moments along perpendicular direction due to small cone size
    // currently just using both derivatives at the focus point
    pub moments: [f32; 2],
}
impl Transmission for QuadraticTransmission {
    fn value(self) -> f32 {
        self.value
    }
    fn new(frustrum: Frustrum, emitter: Vec2) -> Self {
        let focus = frustrum.focus();
        let dir = frustrum.dir();
        let dist = (emitter - focus).dot(dir.normalize());
        Self {
            value: 1.0 / dist,
            moments: [1.0 / (dist * dist), 2.0 / (dist * dist * dist)],
        }
    }
    // TODO: This doesn't rotate properly.
    fn reproject(self, source: Frustrum, target: Frustrum) -> Self {
        let source_focus = source.focus();
        let target_focus = target.focus();
        let dist = (target_focus - source_focus).dot(source.dir().normalize());
        // let ratio = target.dir().normalize().dot(source.dir().normalize());
        Self {
            value: self.value + self.moments[0] * dist + 0.5 * self.moments[1] * dist * dist,
            moments: [self.moments[0] + self.moments[1] * dist, self.moments[1]],
        }
    }
}
impl Add for QuadraticTransmission {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            value: self.value + other.value,
            moments: [
                self.moments[0] + other.moments[0],
                self.moments[1] + other.moments[1],
            ],
        }
    }
}
impl Mul<f32> for QuadraticTransmission {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self {
            value: self.value * rhs,
            moments: [self.moments[0] * rhs, self.moments[1] * rhs],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct QuadraticProjectedTransmission {
    pub value: f32,
    // along the x-axis
    pub moments: [f32; 2],
}
impl Transmission for QuadraticProjectedTransmission {
    fn value(self) -> f32 {
        self.value
    }
    fn new(frustrum: Frustrum, emitter: Vec2) -> Self {
        let focus = frustrum.focus();
        let dist = (emitter - focus).length();
        let projected_dist = (emitter - focus).x;
        let multiplier = projected_dist / dist;
        Self {
            value: multiplier / projected_dist,
            moments: [
                multiplier / (projected_dist * projected_dist),
                2.0 * multiplier / (projected_dist * projected_dist * projected_dist),
            ],
        }
    }
    fn reproject(self, source: Frustrum, target: Frustrum) -> Self {
        let source_focus = source.focus();
        let target_focus = target.focus();
        let dist = (target_focus - source_focus).x;
        Self {
            value: self.value + self.moments[0] * dist + 0.5 * self.moments[1] * dist * dist,
            moments: [self.moments[0] + self.moments[1] * dist, self.moments[1]],
        }
    }
}
impl Add for QuadraticProjectedTransmission {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            value: self.value + other.value,
            moments: [
                self.moments[0] + other.moments[0],
                self.moments[1] + other.moments[1],
            ],
        }
    }
}
impl Mul<f32> for QuadraticProjectedTransmission {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self {
            value: self.value * rhs,
            moments: [self.moments[0] * rhs, self.moments[1] * rhs],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct LinearProjectedTransmission {
    // approximate, at focus (also doing projected)
    pub value: f32,
    pub moment: f32,
}
impl Transmission for LinearProjectedTransmission {
    fn value(self) -> f32 {
        self.value
    }
    fn new(frustrum: Frustrum, emitter: Vec2) -> Self {
        let focus = frustrum.focus();
        let dist = (emitter - focus).length();
        let projected_dist = (emitter - focus).x; // a
        let multiplier = projected_dist / dist;

        // target function: multiplier / projected_dist wrt. projected_dist
        Self {
            value: multiplier / projected_dist,
            moment: multiplier / (projected_dist * projected_dist),
        }
    }
    fn reproject(self, source: Frustrum, target: Frustrum) -> Self {
        let source_focus = source.focus();
        let target_focus = target.focus();
        let dist = (target_focus - source_focus).x;
        Self {
            value: self.value + self.moment * dist,
            moment: self.moment,
        }
    }
}
impl Add for LinearProjectedTransmission {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            value: self.value + other.value,
            moment: self.moment + other.moment,
        }
    }
}
impl Mul<f32> for LinearProjectedTransmission {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self {
            value: self.value * rhs,
            moment: self.moment * rhs,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct GradientTransmission {
    pub value: f32,
    // spatial derivatives in both directions
    pub gradient: Vec2,
}
impl Transmission for GradientTransmission {
    fn value(self) -> f32 {
        self.value
    }
    fn new(frustrum: Frustrum, emitter: Vec2) -> Self {
        let focus = frustrum.focus();
        let dist = (emitter - focus).length();
        Self {
            value: 1.0 / dist,
            gradient: (emitter - focus).normalize() * (1.0 / (dist * dist)),
        }
    }
    fn reproject(self, source: Frustrum, target: Frustrum) -> Self {
        let source_focus = source.focus();
        let target_focus = target.focus();
        Self {
            value: self.value + self.gradient.dot(target_focus - source_focus),
            gradient: self.gradient,
        }
    }
}
impl Add for GradientTransmission {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            value: self.value + other.value,
            gradient: self.gradient + other.gradient,
        }
    }
}
impl Mul<f32> for GradientTransmission {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self {
            value: self.value * rhs,
            gradient: self.gradient * rhs,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HessianTransmission {
    pub value: f32,
    // spatial derivatives in both directions
    pub gradient: Vec2,
    // symmetric second spatial derivatives
    pub hessian: Mat2,
}
// Mat2::default() is the identity, so Default can't be derived.
impl Default for HessianTransmission {
    fn default() -> Self {
        Self {
            value: 0.0,
            gradient: Vec2::ZERO,
            hessian: Mat2::ZERO,
        }
    }
}
impl Transmission for HessianTransmission {
    fn value(self) -> f32 {
        self.value
    }
    fn new(frustrum: Frustrum, emitter: Vec2) -> Self {
        let focus = frustrum.focus();
        let dist = (emitter - focus).length();
        let dir = (emitter - focus).normalize();
        let outer = Mat2::from_cols(dir * dir.x, dir * dir.y);
        Self {
            value: 1.0 / dist,
            gradient: dir * (1.0 / (dist * dist)),
            hessian: (outer * 3.0 - Mat2::IDENTITY) * (1.0 / (dist * dist * dist)),
        }
    }
    fn reproject(self, source: Frustrum, target: Frustrum) -> Self {
        let source_focus = source.focus();
        let target_focus = target.focus();
        let delta = target_focus - source_focus;
        let hessian_delta = self.hessian * delta;
        Self {
            value: self.value + self.gradient.dot(delta) + 0.5 * delta.dot(hessian_delta),
            gradient: self.gradient + hessian_delta,
            hessian: self.hessian,
        }
    }
}
impl Add for HessianTransmission {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            value: self.value + other.value,
            gradient: self.gradient + other.gradient,
            hessian: self.hessian + other.hessian,
        }
    }
}
impl Mul<f32> for HessianTransmission {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self {
            value: self.value * rhs,
            gradient: self.gradient * rhs,
            hessian: self.hessian * rhs,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct AveragedLinearTransmission {
    // approximate, at focus (also doing projected)
    pub value: f32,
    pub slope: f32,
}
impl Transmission for AveragedLinearTransmission {
    fn value(self) -> f32 {
        self.value
    }
    // least squares optimal between -c and c.
    fn new(frustrum: Frustrum, emitter: Vec2) -> Self {
        let focus = frustrum.focus();
        let dist = (emitter - focus).length();
        let projected_dist = (emitter - focus).x; // a
        let multiplier = projected_dist / dist;

        let average_range = frustrum.size() as f32 * 0.5; // c

        let at = (average_range / projected_dist).atanh();

        // target function: multiplier / projected_dist wrt. projected_dist
        Self {
            value: multiplier * at / average_range,
            slope: multiplier * 3.0 * (projected_dist * at - average_range) / average_range.powi(3),
        }
    }
    fn reproject(self, source: Frustrum, target: Frustrum) -> Self {
        let source_focus = source.focus();
        let target_focus = target.focus();
        let dist = (target_focus - source_focus).x;
        Self {
            value: self.value + self.slope * dist,
            slope: self.slope,
        }
    }
}
impl Add for AveragedLinearTransmission {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            value: self.value + other.value,
            slope: self.slope + other.slope,
        }
    }
}
impl Mul<f32> for AveragedLinearTransmission {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self {
            value: self.value * rhs,
            slope: self.slope * rhs,
        }
    }
}
