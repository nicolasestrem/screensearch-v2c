//! Whole-machine CPU busy % via `GetSystemTimes` (sysinfoapi.h — the same module
//! `capture/idle.rs` reads `GetTickCount` from).
//!
//! Windows folds the idle time **into** the kernel time, so over an interval the busy
//! fraction is `1 - Δidle / Δ(kernel + user)` (the denominator is kernel+user because
//! kernel already includes idle). The pure delta math is split out from the syscall so
//! it unit-tests without touching Win32.

/// A snapshot of the three system `FILETIME`s as u64 100-ns tick counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CpuTimes {
    pub idle: u64,
    pub kernel: u64,
    pub user: u64,
}

/// Busy % (`0..=100`) over the interval between `prev` and `now`. Returns `None` when
/// the total (kernel+user) delta is zero — no time elapsed or a counter glitch — so the
/// caller holds its prior reading instead of dividing by zero.
pub(crate) fn busy_pct(prev: CpuTimes, now: CpuTimes) -> Option<f32> {
    let d_idle = now.idle.saturating_sub(prev.idle);
    let d_kernel = now.kernel.saturating_sub(prev.kernel);
    let d_user = now.user.saturating_sub(prev.user);
    // kernel already includes idle (Windows semantics) → total busy+idle = kernel+user.
    let total = d_kernel.saturating_add(d_user);
    if total == 0 {
        return None;
    }
    let busy = total.saturating_sub(d_idle);
    let pct = (busy as f64 / total as f64) * 100.0;
    Some((pct as f32).clamp(0.0, 100.0))
}

/// Reads the three system `FILETIME`s now, or `None` if the syscall fails. The only
/// `unsafe` here is the well-formed out-pointer call, mirroring `capture/idle.rs`.
#[cfg(windows)]
pub(crate) fn read_times() -> Option<CpuTimes> {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::GetSystemTimes;

    let mut idle = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    // SAFETY: three valid, distinct out-pointers to stack `FILETIME`s; `GetSystemTimes`
    // fills them and returns a BOOL. No pointers are retained past the call.
    let ok = unsafe { GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)) };
    if ok.is_err() {
        return None;
    }
    Some(CpuTimes {
        idle: filetime_to_u64(idle),
        kernel: filetime_to_u64(kernel),
        user: filetime_to_u64(user),
    })
}

#[cfg(windows)]
fn filetime_to_u64(ft: windows::Win32::Foundation::FILETIME) -> u64 {
    ((ft.dwHighDateTime as u64) << 32) | (ft.dwLowDateTime as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(idle: u64, kernel: u64, user: u64) -> CpuTimes {
        CpuTimes { idle, kernel, user }
    }

    #[test]
    fn fully_idle_is_zero_pct() {
        // All of the kernel delta was idle → 0% busy.
        let prev = t(0, 0, 0);
        let now = t(100, 100, 0);
        assert_eq!(busy_pct(prev, now), Some(0.0));
    }

    #[test]
    fn fully_busy_is_hundred_pct() {
        // No idle delta, all kernel+user → 100% busy.
        let prev = t(0, 0, 0);
        let now = t(0, 60, 40);
        assert_eq!(busy_pct(prev, now), Some(100.0));
    }

    #[test]
    fn half_idle_is_fifty_pct() {
        // idle = 50 of the 100 kernel ticks (user 0) → 50% busy.
        let prev = t(0, 0, 0);
        let now = t(50, 100, 0);
        assert_eq!(busy_pct(prev, now), Some(50.0));
    }

    #[test]
    fn user_time_counts_as_busy() {
        // kernel 80 (idle 80) + user 20 → total 100, busy = 100 - 80 = 20 → 20%.
        let prev = t(0, 0, 0);
        let now = t(80, 80, 20);
        assert_eq!(busy_pct(prev, now), Some(20.0));
    }

    #[test]
    fn zero_total_delta_returns_none() {
        // No elapsed kernel+user time → cannot compute a rate; hold prior, don't divide.
        let same = t(10, 20, 30);
        assert_eq!(busy_pct(same, same), None);
    }

    #[test]
    fn clamps_when_idle_exceeds_total() {
        // Defensive: a counter glitch where Δidle > Δ(kernel+user) clamps to 0, not negative.
        let prev = t(0, 0, 0);
        let now = t(200, 100, 0);
        assert_eq!(busy_pct(prev, now), Some(0.0));
    }
}
