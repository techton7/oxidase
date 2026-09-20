//! Web (`wasm32`) DOM backend implementation backed by `web-sys`.

#[allow(unused_imports)]
use super::types::*;

/// Web browser Document implementation.
#[derive(Clone, Debug)]
pub struct Document {
    doc: web_sys::Document,
}

impl Document {
    /// Returns the current active browser document.
    pub fn current() -> Option<Self> {
        let win = web_sys::window()?;
        let doc = win.document()?;
        Some(Self { doc })
    }

    /// Creates a Document from a raw `web_sys::Document`.
    #[inline]
    pub fn from_raw(doc: web_sys::Document) -> Self {
        Self { doc }
    }

    /// Access the underlying `web_sys::Document`.
    #[inline]
    pub fn raw(&self) -> &web_sys::Document {
        &self.doc
    }
}

/// Web browser Element implementation.
#[derive(Clone, Debug)]
pub struct Element {
    pub(crate) el: web_sys::Element,
}

impl Element {
    /// Creates an Element from a raw `web_sys::Element`.
    #[inline]
    pub fn from_raw(el: web_sys::Element) -> Self {
        Self { el }
    }

    /// Access the underlying `web_sys::Element`.
    #[inline]
    pub fn raw(&self) -> &web_sys::Element {
        &self.el
    }
}
