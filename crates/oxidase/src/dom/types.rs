//! Common geometry and event types for the cross-platform DOM abstraction.

use serde::{Deserialize, Serialize};

/// Bounding rectangle in viewport coordinates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }

    #[inline]
    pub fn top(&self) -> f32 {
        self.y
    }

    #[inline]
    pub fn bottom(&self) -> f32 {
        self.y + self.height
    }

    #[inline]
    pub fn left(&self) -> f32 {
        self.x
    }

    #[inline]
    pub fn right(&self) -> f32 {
        self.x + self.width
    }
}

/// Viewport dimensions and scroll offsets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Viewport {
    pub width: f32,
    pub height: f32,
    pub scroll_x: f32,
    pub scroll_y: f32,
}

impl Viewport {
    pub const fn new(width: f32, height: f32, scroll_x: f32, scroll_y: f32) -> Self {
        Self { width, height, scroll_x, scroll_y }
    }
}

/// 2D Point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}
