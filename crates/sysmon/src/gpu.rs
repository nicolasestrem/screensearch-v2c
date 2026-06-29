//! GPU utilization via PDH `\GPU Engine(*)\Utilization Percentage`, summed across every
//! GPU-engine instance (3D, compute, copy, video, …) and clamped to 100.
//!
//! Two correctness choices matter most:
//! 1. **`PdhAddEnglishCounterW`** (not `PdhAddCounterW`) — the English variant ignores the
//!    system UI language, so a localized Windows (which renames "GPU Engine") can't break
//!    the counter path.
//! 2. **Truthful "unmonitored"** — if the counter can't be added (no GPU perf-counter
//!    provider, a VM, a disabled provider), the probe latches `monitored = false` once and
//!    never touches PDH again. Every sample then reports `gpu_pct = None`, and the throttle
//!    runs on CPU pressure alone. A dead counter is a status, never an error.

/// Sums per-engine utilization values into a single `0..=100` GPU %, ignoring non-finite
/// readings. Pure — unit-testable without PDH.
pub(crate) fn sum_engine_utilization(values: &[f64]) -> f32 {
    let sum: f64 = values.iter().copied().filter(|v| v.is_finite()).sum();
    (sum as f32).clamp(0.0, 100.0)
}

#[cfg(windows)]
pub(crate) use imp::GpuProbe;

#[cfg(not(windows))]
pub(crate) use stub::GpuProbe;

#[cfg(windows)]
mod imp {
    use super::sum_engine_utilization;
    use windows::core::{w, PCWSTR};
    use windows::Win32::System::Performance::{
        PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
        PdhOpenQueryW, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
    };

    const ERROR_SUCCESS: u32 = 0;
    const PDH_MORE_DATA: u32 = 0x8000_07D2;
    const PDH_CSTATUS_VALID_DATA: u32 = 0x0000_0000;
    const PDH_CSTATUS_NEW_DATA: u32 = 0x0000_0001;

    // PDH handles are `*mut c_void` newtypes (not `Send`). We store the pointer value as
    // `isize` so the probe stays `Send + Sync` behind the `Arc`, and reconstruct the typed
    // handle for each call (the handles are stable for the query's lifetime).
    fn hquery(v: isize) -> PDH_HQUERY {
        PDH_HQUERY(v as *mut core::ffi::c_void)
    }
    fn hcounter(v: isize) -> PDH_HCOUNTER {
        PDH_HCOUNTER(v as *mut core::ffi::c_void)
    }

    /// The live PDH query + counter handles, owned for the process lifetime. Present only
    /// when GPU monitoring is live; `None` is the truthful "unmonitored" state.
    pub(crate) struct GpuProbe {
        inner: Option<Query>,
        /// Last good GPU %, returned when a single collect transiently fails (the dwell
        /// gate absorbs it) — avoids a false "pressure dropped" blip.
        last: f32,
    }

    struct Query {
        query: isize,
        counter: isize,
    }

    impl GpuProbe {
        /// Opens the query, adds the English GPU-Engine counter, and primes one collect
        /// (rate counters need two collects to yield a value). Any failure latches
        /// unmonitored and logs once.
        pub(crate) fn new() -> Self {
            match Self::open() {
                Some(q) => GpuProbe {
                    inner: Some(q),
                    last: 0.0,
                },
                None => {
                    tracing::info!(
                        "sysmon: GPU performance counters unavailable; throttle runs on CPU pressure only"
                    );
                    GpuProbe {
                        inner: None,
                        last: 0.0,
                    }
                }
            }
        }

        fn open() -> Option<Query> {
            let mut query = PDH_HQUERY(std::ptr::null_mut());
            // SAFETY: out-pointer to a stack handle; NULL data source = live data.
            let st = unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) };
            if st != ERROR_SUCCESS {
                return None;
            }
            let mut counter = PDH_HCOUNTER(std::ptr::null_mut());
            // SAFETY: `query` is a valid open query; the counter path is a static wide
            // string; out-pointer to a stack handle. English name = locale-proof.
            let st = unsafe {
                PdhAddEnglishCounterW(
                    query,
                    w!("\\GPU Engine(*)\\Utilization Percentage"),
                    0,
                    &mut counter,
                )
            };
            if st != ERROR_SUCCESS {
                // SAFETY: close the query we just opened so the handle isn't leaked.
                unsafe {
                    let _ = PdhCloseQuery(query);
                }
                return None;
            }
            // Prime: the first collect establishes a baseline for the rate counter.
            // SAFETY: `query` is valid and open.
            unsafe {
                let _ = PdhCollectQueryData(query);
            }
            Some(Query {
                query: query.0 as isize,
                counter: counter.0 as isize,
            })
        }

        /// Current summed GPU % (`Some`) or `None` when unmonitored.
        pub(crate) fn sample(&mut self) -> Option<f32> {
            let q = self.inner.as_ref()?;
            match Self::collect(q.query, q.counter) {
                Some(values) => {
                    let pct = sum_engine_utilization(&values);
                    self.last = pct;
                    Some(pct)
                }
                // Transient collect failure: hold the last good reading (monitored stays true).
                None => Some(self.last),
            }
        }

        pub(crate) fn monitored(&self) -> bool {
            self.inner.is_some()
        }

        /// Collects the query and formats the wildcard counter into per-engine doubles.
        fn collect(query_v: isize, counter_v: isize) -> Option<Vec<f64>> {
            let query = hquery(query_v);
            let counter = hcounter(counter_v);
            // SAFETY: `query` is a valid open query.
            let st = unsafe { PdhCollectQueryData(query) };
            if st != ERROR_SUCCESS {
                return None;
            }

            // First call sizes the buffer (returns PDH_MORE_DATA).
            let mut buf_size: u32 = 0;
            let mut item_count: u32 = 0;
            // SAFETY: NULL item buffer asks PDH for the required size only.
            let st = unsafe {
                PdhGetFormattedCounterArrayW(
                    counter,
                    PDH_FMT_DOUBLE,
                    &mut buf_size,
                    &mut item_count,
                    None,
                )
            };
            if st != PDH_MORE_DATA || buf_size == 0 || item_count == 0 {
                // No data this round (or an unexpected status) → 0 engines this sample.
                return Some(Vec::new());
            }

            // Allocate as a Vec of the item type so the buffer is correctly aligned for
            // `PDH_FMT_COUNTERVALUE_ITEM_W` (it holds f64/i64/pointers — alignment 8). The
            // byte size PDH wants exceeds the array (it appends instance-name strings), so
            // size the capacity by bytes, not item_count.
            let item_size = std::mem::size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>();
            let cap = (buf_size as usize).div_ceil(item_size).max(1);
            let mut buffer: Vec<PDH_FMT_COUNTERVALUE_ITEM_W> = Vec::with_capacity(cap);

            // SAFETY: `buffer` has `buf_size` bytes of correctly aligned capacity; PDH
            // fills up to `item_count` items plus trailing name strings within `buf_size`.
            let st = unsafe {
                PdhGetFormattedCounterArrayW(
                    counter,
                    PDH_FMT_DOUBLE,
                    &mut buf_size,
                    &mut item_count,
                    Some(buffer.as_mut_ptr()),
                )
            };
            if st != ERROR_SUCCESS {
                return None;
            }

            // SAFETY: PDH wrote `item_count` initialized items at the buffer start.
            let items = unsafe { std::slice::from_raw_parts(buffer.as_ptr(), item_count as usize) };
            let mut values = Vec::with_capacity(items.len());
            for item in items {
                let cstatus = item.FmtValue.CStatus;
                if cstatus == PDH_CSTATUS_VALID_DATA || cstatus == PDH_CSTATUS_NEW_DATA {
                    // SAFETY: PDH_FMT_DOUBLE was requested, so the union holds doubleValue.
                    let v = unsafe { item.FmtValue.Anonymous.doubleValue };
                    values.push(v);
                }
            }
            Some(values)
        }
    }

    impl Drop for GpuProbe {
        fn drop(&mut self) {
            if let Some(q) = self.inner.take() {
                // SAFETY: `q.query` is a valid open query we own; closing also frees its
                // counters. Best-effort on teardown.
                unsafe {
                    let _ = PdhCloseQuery(hquery(q.query));
                }
            }
        }
    }
}

#[cfg(not(windows))]
mod stub {
    /// Non-Windows builds have no PDH; the probe is always unmonitored. The app is
    /// Windows-only (CI is Windows); this just lets the crate type-check anywhere.
    pub(crate) struct GpuProbe;

    impl GpuProbe {
        pub(crate) fn new() -> Self {
            GpuProbe
        }
        pub(crate) fn sample(&mut self) -> Option<f32> {
            None
        }
        pub(crate) fn monitored(&self) -> bool {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sum_engine_utilization;

    #[test]
    fn sums_engines() {
        // 3D 30% + compute 12% + copy 0% = 42%.
        assert_eq!(sum_engine_utilization(&[30.0, 12.0, 0.0]), 42.0);
    }

    #[test]
    fn clamps_to_hundred() {
        // Concurrent engines can each report high; the aggregate caps at 100.
        assert_eq!(sum_engine_utilization(&[80.0, 60.0]), 100.0);
    }

    #[test]
    fn ignores_non_finite() {
        // A NaN/inf reading (counter glitch) is dropped, not propagated.
        assert_eq!(
            sum_engine_utilization(&[f64::NAN, 25.0, f64::INFINITY]),
            25.0
        );
    }

    #[test]
    fn empty_is_zero() {
        assert_eq!(sum_engine_utilization(&[]), 0.0);
    }
}
