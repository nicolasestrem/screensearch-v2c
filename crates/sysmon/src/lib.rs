//! `sysmon` — the Windows-native system-pressure probe for the smart enrichment throttle
//! (`03 §5/§8`, `docs/0.2.0.md` former PR5).
//!
//! Reads whole-machine CPU busy % (`GetSystemTimes`) and GPU utilization (PDH
//! `\GPU Engine(*)\Utilization Percentage`) and exposes them through
//! [`traits::PressureProbe`]. The kernel forbids `unsafe`, so this crate owns the only
//! Win32 here and the composition root injects it as `dyn PressureProbe` — the same seam
//! as `capture`'s `IdleSource` (`03 §2`).
//!
//! The probe is **stateful** (CPU busy % is a delta between two `GetSystemTimes` readings;
//! GPU is a PDH rate counter needing repeated collects on one open query) and
//! **infallible by contract** — a failed read degrades to a truthful sample, never an
//! error.

use std::sync::{Arc, Mutex};

use traits::{PressureProbe, PressureSample};

mod cpu;
mod gpu;

use cpu::CpuTimes;
use gpu::GpuProbe;

/// The native CPU+GPU pressure probe. Construct once via [`native_probe`]; sampling is
/// cheap (two syscalls) and serialized behind a `Mutex` since the trait samples through
/// `&self` while the underlying readings are stateful.
struct NativeProbe {
    state: Mutex<ProbeState>,
    /// Cached so [`PressureProbe::gpu_monitored`] answers without locking or sampling.
    gpu_monitored: bool,
}

struct ProbeState {
    prev_cpu: Option<CpuTimes>,
    /// Last good CPU %, returned when a single `GetSystemTimes` delta can't be formed.
    last_cpu_pct: f32,
    gpu: GpuProbe,
}

impl NativeProbe {
    fn new() -> Self {
        let gpu = GpuProbe::new();
        let gpu_monitored = gpu.monitored();
        // Prime the CPU baseline so the first real sample can form a delta.
        let prev_cpu = cpu::read_times();
        NativeProbe {
            state: Mutex::new(ProbeState {
                prev_cpu,
                last_cpu_pct: 0.0,
                gpu,
            }),
            gpu_monitored,
        }
    }
}

impl PressureProbe for NativeProbe {
    fn sample(&self) -> PressureSample {
        let mut st = self.state.lock().expect("sysmon probe lock poisoned");
        let cpu_pct = match cpu::read_times() {
            Some(now) => {
                let pct = st
                    .prev_cpu
                    .and_then(|prev| cpu::busy_pct(prev, now))
                    .unwrap_or(st.last_cpu_pct);
                st.prev_cpu = Some(now);
                st.last_cpu_pct = pct;
                pct
            }
            None => st.last_cpu_pct,
        };
        let gpu_pct = st.gpu.sample();
        let gpu_monitored = st.gpu.monitored();
        PressureSample {
            cpu_pct,
            gpu_pct,
            gpu_monitored,
            sampled_at: now_ms(),
        }
    }

    fn gpu_monitored(&self) -> bool {
        self.gpu_monitored
    }
}

/// Builds the native pressure probe (opens PDH + primes the CPU baseline) and returns it
/// as a `dyn PressureProbe` for the kernel. Logs once whether the GPU is monitored.
pub fn native_probe() -> Arc<dyn PressureProbe> {
    let probe = NativeProbe::new();
    tracing::info!(
        gpu_monitored = probe.gpu_monitored,
        "sysmon pressure probe initialized"
    );
    Arc::new(probe)
}

/// Current unix time in milliseconds (the kernel's clock unit, `03 §4`).
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_is_well_formed() {
        // The probe is infallible: a sample always lands with CPU in range and a
        // gpu_monitored flag consistent with gpu_pct (Some iff monitored is plausible).
        let probe = native_probe();
        let s = probe.sample();
        assert!(
            (0.0..=100.0).contains(&s.cpu_pct),
            "cpu out of range: {s:?}"
        );
        if let Some(g) = s.gpu_pct {
            assert!((0.0..=100.0).contains(&g), "gpu out of range: {s:?}");
        }
        // gpu_monitored() must agree with the per-sample flag.
        assert_eq!(probe.gpu_monitored(), s.gpu_monitored);
    }
}
