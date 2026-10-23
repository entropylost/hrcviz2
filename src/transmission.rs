use std::ops::{Add, Mul};

use glam::Vec2;

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
pub struct Gradient2dTransmission {
    pub value: f32,
    // spatial derivatives in both directions
    pub gradient: Vec2,
}
impl Transmission for Gradient2dTransmission {
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
    // TODO: This doesn't rotate properly.
    fn reproject(self, source: Frustrum, target: Frustrum) -> Self {
        let source_focus = source.focus();
        let target_focus = target.focus();
        Self {
            value: self.value + self.gradient.dot(target_focus - source_focus),
            gradient: self.gradient,
        }
    }
}
impl Add for Gradient2dTransmission {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self {
            value: self.value + other.value,
            gradient: self.gradient + other.gradient,
        }
    }
}
impl Mul<f32> for Gradient2dTransmission {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self {
            value: self.value * rhs,
            gradient: self.gradient * rhs,
        }
    }
}
