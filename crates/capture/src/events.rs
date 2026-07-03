//! The Win32 input-event hook thread for event-driven capture (`docs/0.2.0.md`;
//! `07` #47). The **only** `unsafe` part of the feature; the decision logic lives in
//! the pure, tested [`crate::trigger`] module.
//!
//! A dedicated thread runs a Win32 message pump, because `SetWinEventHook`
//! (out-of-context) callbacks are only delivered to a thread that pumps messages. We
//! install exactly one source:
//!
//! - `SetWinEventHook(EVENT_SYSTEM_FOREGROUND, …, WINEVENT_OUTOFCONTEXT)` — foreground
//!   / app-switch. Out-of-context = no DLL injection into other processes.
//!
//! *(0.3.0 PR2 trimmed event-driven capture to foreground + idle; the clipboard
//! listener and the global `WH_MOUSE_LL` mouse hook — click / scroll-stop — were
//! removed, and with them a whole `unsafe` path and a privacy-optics landmine —
//! `docs/0.3.0.md`, `02 §5c`.)*
//!
//! No keyboard hooks, no key content, no clipboard, no pointer input — only "foreground
//! changed" signals (`docs/0.2.0.md` privacy posture; idle is derived separately from
//! the polled idle time, not hooked). The callback forwards an [`InputEventKind`] over a
//! bounded channel with `try_send`, so a slow consumer can never block the OS message
//! pump (a blocked pump would freeze hook delivery for this thread); a dropped event just
//! means one fewer trigger.

use std::cell::RefCell;

use anyhow::{anyhow, bail, Result};
use tokio::sync::mpsc::{Receiver, Sender};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, PeekMessageW, PostThreadMessageW, TranslateMessage,
    EVENT_SYSTEM_FOREGROUND, MSG, PM_NOREMOVE, WINEVENT_OUTOFCONTEXT, WM_QUIT, WM_USER,
};

use crate::trigger::InputEventKind;

thread_local! {
    /// The bounded sender the hook callback forwards events on. Set on the hook thread
    /// before the message loop; the callback runs on that same thread, so a
    /// thread-local is the correct (and `extern "system"`-callback-friendly) home — the
    /// callback takes no user pointer.
    static EVENT_TX: RefCell<Option<Sender<InputEventKind>>> = const { RefCell::new(None) };
}

/// A running input-event hook thread. Dropping it tears the thread down cleanly
/// (`WM_QUIT` → unhook → join).
pub(crate) struct InputEventSource {
    /// Single-consumer receiver behind a `tokio::sync::Mutex` so the source stays
    /// `Sync` (the [`crate::CaptureSource`] bound) while `recv` can still be held
    /// across `.await`. Only the capture loop consumes it, so the lock never contends.
    rx: tokio::sync::Mutex<Receiver<InputEventKind>>,
    /// Hook-thread id, used to `PostThreadMessageW(WM_QUIT)` on shutdown.
    thread_id: u32,
    join: Option<std::thread::JoinHandle<()>>,
}

impl InputEventSource {
    /// Spawns the hook thread and returns once the foreground `SetWinEventHook` is
    /// registered (or errors if setup failed). The caller only starts the source when
    /// event mode + foreground capture are both enabled (`crate::WgcCapture`); the
    /// tested [`crate::trigger::TriggerMachine`] owns the finer per-trigger semantics.
    pub(crate) fn start() -> Result<Self> {
        let (tx, rx) = tokio::sync::mpsc::channel::<InputEventKind>(64);
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<u32>>();

        let join = std::thread::Builder::new()
            .name("input-events".to_string())
            .spawn(move || hook_thread_main(tx, ready_tx))
            .map_err(|e| anyhow!("failed to spawn input-events thread: {e}"))?;

        let thread_id = match ready_rx.recv() {
            Ok(Ok(id)) => id,
            Ok(Err(e)) => {
                let _ = join.join();
                return Err(e);
            }
            Err(e) => return Err(anyhow!("input-events thread exited during init: {e}")),
        };

        Ok(Self {
            rx: tokio::sync::Mutex::new(rx),
            thread_id,
            join: Some(join),
        })
    }

    /// Await the next discrete input event, or `None` once the hook thread has stopped.
    pub(crate) async fn recv(&self) -> Option<InputEventKind> {
        self.rx.lock().await.recv().await
    }
}

impl Drop for InputEventSource {
    fn drop(&mut self) {
        // Break the message pump (`GetMessageW` returns 0 on `WM_QUIT`), then join so the
        // hook is released before we return. By the time we get here the thread's message
        // queue exists (the hook thread forces it via `PeekMessageW` before signaling
        // ready), so the post can't be lost.
        // SAFETY: posting `WM_QUIT` to a known thread id; no pointers involved.
        unsafe {
            let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
        }
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

/// Forward an event to the consumer, dropping it if the channel is full (never block
/// the OS message pump).
fn dispatch(kind: InputEventKind) {
    EVENT_TX.with(|cell| {
        if let Some(tx) = cell.borrow().as_ref() {
            let _ = tx.try_send(kind);
        }
    });
}

/// `SetWinEventHook` callback: we registered only `EVENT_SYSTEM_FOREGROUND`, but guard
/// the event id anyway.
unsafe extern "system" fn winevent_proc(
    _hook: HWINEVENTHOOK,
    event: u32,
    _hwnd: HWND,
    _id_object: i32,
    _id_child: i32,
    _event_thread: u32,
    _event_time: u32,
) {
    if event == EVENT_SYSTEM_FOREGROUND {
        dispatch(InputEventKind::Foreground);
    }
}

/// Installs the out-of-context foreground WinEvent hook.
///
/// SAFETY: a standard `SetWinEventHook` call on the calling thread.
unsafe fn install_foreground_hook() -> Result<HWINEVENTHOOK> {
    let hook = SetWinEventHook(
        EVENT_SYSTEM_FOREGROUND,
        EVENT_SYSTEM_FOREGROUND,
        None,
        Some(winevent_proc),
        0,
        0,
        WINEVENT_OUTOFCONTEXT,
    );
    if hook.0.is_null() {
        bail!("SetWinEventHook(EVENT_SYSTEM_FOREGROUND) failed");
    }
    Ok(hook)
}

/// Hook-thread entry point: force the thread message queue to exist, install the
/// foreground hook, report readiness, pump messages until `WM_QUIT`, then unhook.
fn hook_thread_main(tx: Sender<InputEventKind>, ready: std::sync::mpsc::Sender<Result<u32>>) {
    EVENT_TX.with(|cell| *cell.borrow_mut() = Some(tx));

    // SAFETY: setup + pump + teardown all run on this single thread.
    let hook = unsafe {
        // Force the thread to own a message queue *before* we signal ready: `Drop`
        // posts `WM_QUIT` via `PostThreadMessageW`, which is silently lost if the
        // target thread has no queue yet. With the message-only window gone (0.3.0
        // PR2), this `PeekMessageW` is what guarantees the queue (the documented
        // idiom); don't rely on `SetWinEventHook` to create it.
        let mut msg = MSG::default();
        let _ = PeekMessageW(&mut msg, None, WM_USER, WM_USER, PM_NOREMOVE);

        match install_foreground_hook() {
            Ok(hook) => hook,
            Err(e) => {
                let _ = ready.send(Err(e));
                EVENT_TX.with(|cell| *cell.borrow_mut() = None);
                return;
            }
        }
    };

    // SAFETY: thread-id read with no arguments.
    let thread_id = unsafe { GetCurrentThreadId() };
    let _ = ready.send(Ok(thread_id));

    // Message pump: GetMessageW returns 0 on WM_QUIT and -1 on error — break on both.
    // SAFETY: standard Win32 message loop driving the out-of-context hook.
    unsafe {
        let mut msg = MSG::default();
        loop {
            let ret = GetMessageW(&mut msg, None, 0, 0);
            if ret.0 <= 0 {
                break;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = UnhookWinEvent(hook);
    }

    EVENT_TX.with(|cell| *cell.borrow_mut() = None);
}

#[cfg(test)]
mod tests {
    use super::InputEventSource;

    /// The #1 lifecycle risk: a message-pump thread that leaks or fails to terminate
    /// would leak the hook and could hang `stop`/`reload`. Start and drop the source
    /// many times; `Drop` must post `WM_QUIT`, the thread must exit, and the join must
    /// return — no leak, no hang, no panic. Needs a real desktop (USER32 message pump),
    /// so it is `#[ignore]`d in CI; run locally with
    /// `cargo test -p capture -- --ignored`.
    #[test]
    #[ignore = "requires a real desktop (USER32 message pump); run locally"]
    fn source_starts_and_stops_cleanly_repeatedly() {
        for i in 0..50 {
            let source = InputEventSource::start()
                .unwrap_or_else(|e| panic!("start input-events source on iteration {i}: {e}"));
            // `Drop` posts WM_QUIT and joins the hook thread; a leak/hang surfaces as a
            // hung test, a panic as a failure.
            drop(source);
        }
    }
}
