//! Pure Rust geometric engine and positioning substrate for Oxidase.
//!
//! Vendored and adapted from RustForWeb `floating-ui-core` and `floating-ui-utils`.
//! Provides pure geometric types, 12 placement calculations, overflow detection,
//! 8 positioning middlewares, and the host-neutral `FloatingPlatform` contract
//! with zero `web-sys`, `wasm-bindgen`, `js-sys`, or macro baggage.

pub mod compute_coords_from_placement;
pub mod compute_position;
pub mod detect_overflow;
pub mod error;
pub mod geometry;
pub mod middleware;
pub mod platform;
pub mod types;

#[cfg(test)]
pub mod test_utils;

pub use compute_coords_from_placement::compute_coords_from_placement;
pub use compute_position::compute_position;
pub use detect_overflow::*;
pub use error::*;
pub use geometry::*;
pub use platform::*;
pub use types::*;
