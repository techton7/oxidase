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

/// Returns the current local calendar date as `(year, month, day)` (`month` in `1..=12`, `day` in `1..=31`).
pub fn local_today_ymd() -> (i32, u8, u8) {
    #[cfg(target_arch = "wasm32")]
    {
        let date = js_sys::Date::new_0();
        (
            date.get_full_year() as i32,
            (date.get_month() + 1) as u8,
            date.get_date() as u8,
        )
    }

    #[cfg(all(not(target_arch = "wasm32"), unix))]
    {
        // SAFETY: `libc::time` and `libc::localtime_r` operate on valid stack pointers.
        unsafe {
            let mut now: libc::time_t = 0;
            if libc::time(&mut now) != -1 {
                let mut tm: libc::tm = std::mem::zeroed();
                if !libc::localtime_r(&now, &mut tm).is_null() {
                    return (
                        tm.tm_year + 1900,
                        (tm.tm_mon + 1) as u8,
                        tm.tm_mday as u8,
                    );
                }
            }
        }
        utc_today_ymd_fallback()
    }

    #[cfg(all(not(target_arch = "wasm32"), windows))]
    {
        #[repr(C)]
        struct SystemTimeWin {
            w_year: u16,
            w_month: u16,
            w_day_of_week: u16,
            w_day: u16,
            w_hour: u16,
            w_minute: u16,
            w_second: u16,
            w_milliseconds: u16,
        }
        unsafe extern "system" {
            fn GetLocalTime(lpSystemTime: *mut SystemTimeWin);
        }
        let mut st = std::mem::MaybeUninit::<SystemTimeWin>::uninit();
        // SAFETY: `GetLocalTime` initializes all fields of `SystemTimeWin`.
        unsafe {
            GetLocalTime(st.as_mut_ptr());
            let st = st.assume_init();
            (st.w_year as i32, st.w_month as u8, st.w_day as u8)
        }
    }

    #[cfg(all(not(target_arch = "wasm32"), not(unix), not(windows)))]
    {
        utc_today_ymd_fallback()
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(windows)))]
fn utc_today_ymd_fallback() -> (i32, u8, u8) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    civil_from_days(secs.div_euclid(86_400))
}

#[cfg(all(not(target_arch = "wasm32"), not(windows)))]
fn civil_from_days(days_since_unix_epoch: i64) -> (i32, u8, u8) {
    let z = days_since_unix_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = y + i64::from(m <= 2);
    (y as i32, m as u8, d as u8)
}

#[cfg(all(test, not(target_arch = "wasm32"), not(windows)))]
mod tests {
    use super::civil_from_days;

    #[test]
    fn test_civil_from_days_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(10_957), (2000, 1, 1));
        assert_eq!(civil_from_days(19_782), (2024, 2, 29));
    }
}
