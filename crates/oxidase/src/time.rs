//! Cross-target monotonic and wall-clock time primitives across Web (`wasm32`) and Native hosts.

use std::time::Duration;

#[cfg(target_arch = "wasm32")]
#[inline]
fn wasm_monotonic_ms() -> f64 {
    web_sys::window()
        .and_then(|w| w.performance())
        .map(|p| p.now())
        .unwrap_or_else(js_sys::Date::now)
}

#[cfg(target_arch = "wasm32")]
#[inline]
fn ms_to_duration(ms: f64) -> Duration {
    if !ms.is_finite() || ms <= 0.0 {
        Duration::ZERO
    } else {
        Duration::from_secs_f64(ms / 1000.0)
    }
}

/// Cross-host monotonic timestamp for measuring elapsed durations without wall-clock skew.
///
/// - On non-WASM targets, wraps [`std::time::Instant`].
/// - On `wasm32`, wraps [`Duration`] sampled from `window.performance.now()` (falling back to `Date.now()`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MonotonicInstant {
    #[cfg(not(target_arch = "wasm32"))]
    inner: std::time::Instant,
    #[cfg(target_arch = "wasm32")]
    inner: Duration,
}

impl MonotonicInstant {
    /// Captures the current monotonic timestamp.
    #[inline]
    pub fn now() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self {
                inner: std::time::Instant::now(),
            }
        }
        #[cfg(target_arch = "wasm32")]
        {
            Self {
                inner: ms_to_duration(wasm_monotonic_ms()),
            }
        }
    }

    /// Returns the elapsed duration from `earlier` to `self`, saturating to [`Duration::ZERO`] if `earlier` is later.
    #[inline]
    pub fn saturating_duration_since(&self, earlier: Self) -> Duration {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.inner.saturating_duration_since(earlier.inner)
        }
        #[cfg(target_arch = "wasm32")]
        {
            self.inner.saturating_sub(earlier.inner)
        }
    }

    /// Returns the elapsed duration since this timestamp was captured.
    #[inline]
    pub fn elapsed(&self) -> Duration {
        Self::now().saturating_duration_since(*self)
    }
}

/// Returns a monotonically non-decreasing timestamp in milliseconds relative to the process/page origin.
#[inline]
pub fn monotonic_now_ms() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        static PROCESS_EPOCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
        let epoch = PROCESS_EPOCH.get_or_init(std::time::Instant::now);
        epoch.elapsed().as_secs_f64() * 1000.0
    }
    #[cfg(target_arch = "wasm32")]
    {
        wasm_monotonic_ms()
    }
}

/// Returns the current wall-clock timestamp in milliseconds since the Unix epoch.
#[inline]
pub fn unix_epoch_ms() -> f64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64() * 1000.0)
            .unwrap_or(0.0)
    }
    #[cfg(target_arch = "wasm32")]
    {
        js_sys::Date::now()
    }
}
