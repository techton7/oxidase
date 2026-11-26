//! Platform adapters for Floating UI.
//!
//! Provides concrete implementations of [`FloatingPlatform`][crate::FloatingPlatform] and
//! [`Platform`][crate::Platform] for Web (`web-sys`) and Blitz Native (`blitz-dom`).

pub use crate::types::FloatingPlatform;

pub(crate) const DOCUMENT_ELEMENT_SENTINEL: &str = "__oxidase_document_element__";

#[cfg(target_arch = "wasm32")]
pub mod web;

#[cfg(not(target_arch = "wasm32"))]
pub mod native;

#[cfg(target_arch = "wasm32")]
pub use web::*;

#[cfg(not(target_arch = "wasm32"))]
pub use native::*;
