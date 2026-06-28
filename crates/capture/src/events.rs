//! The Win32 input-event hook thread for event-driven capture (`docs/0.2.0.md`;
//! `07` #47). The **only** `unsafe` part of the feature; the decision logic lives in
//! the pure, tested [`crate::trigger`] module.
//!
//! A dedicated thread owns a **message-only window** (`HWND_MESSAGE`) and runs a Win32
//! message pump, because both `SetWinEventHook` (out-of-context) callbacks and the
//! clipboard listener's `WM_CLIPBOARDUPDATE` are only delivered to a thread that pumps
//! messages. We register only the **enabled** privacy-safe sources (the caller passes
//! the per-trigger flags), and nothing else:
//!
//! - `SetWinEventHook(EVENT_SYSTEM_FOREGROUND, …, WINEVENT_OUTOFCONTEXT)` — foreground
//!   / app-switch. Out-of-context = no DLL injection into other processes.
//! - `AddClipboardFormatListener` — fires on a clipboard **change**; we never call
//!   `GetClipboardData`/`OpenClipboard`, so clipboard *contents* are never read.
//!
//! No keyboard/mouse hooks, no key content, no clipboard text — only "the foreground
//! changed" / "the clipboard changed" signals (`docs/0.2.0.md` privacy posture). The
//! callbacks forward an [`InputEventKind`] over a bounded channel with `try_send`, so a
//! slow consumer can never block the OS message pump (a blocked pump would freeze hook
//! delivery for this thread); a dropped event just means one fewer trigger.

use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

use anyhow::{anyhow, bail, Result};
use tokio::sync::mpsc::{Receiver, Sender};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, RemoveClipboardFormatListener,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW,
    PostThreadMessageW, RegisterClassW, TranslateMessage, EVENT_SYSTEM_FOREGROUND, HWND_MESSAGE,
    MSG, WINDOW_EX_STYLE, WINDOW_STYLE, WINEVENT_OUTOFCONTEXT, WM_CLIPBOARDUPDATE, WM_QUIT,
    WNDCLASSW,
};

use crate::trigger::InputEventKind;

/// The window class for our message-only window (registered once per process).
const CLASS_NAME: PCWSTR = w!("ScreenSearchInputEventsV1");

thread_local! {
    /// The bounded sender the hook callbacks forward events on. Set on the hook thread
    /// before the message loop; both callbacks run on that same thread, so a
    /// thread-local is the correct (and `extern "system"`-callback-friendly) home — the
    /// callbacks take no user pointer.
    static EVENT_TX: RefCell<Option<Sender<InputEventKind>>> = const { RefCell::new(None) };
}

/// A running input-event hook thread. Dropping it tears the thread down cleanly
/// (`WM_QUIT` → unhook → remove listener → destroy window → join).
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
    /// Spawns the hook thread and returns once the requested hooks are registered (or
    /// errors if setup failed). Installs **only** the enabled sources: the foreground
    /// hook iff `on_foreground`, the clipboard listener iff `on_clipboard`. This keeps a
    /// disabled source from pushing unwanted events into the bounded queue (where they
    /// could crowd out an enabled trigger) and decouples the two install paths' failure
    /// modes. The caller only starts the source when at least one is enabled; the tested
    /// [`crate::trigger::TriggerMachine`] still owns the finer per-trigger semantics.
    pub(crate) fn start(on_foreground: bool, on_clipboard: bool) -> Result<Self> {
        let (tx, rx) = tokio::sync::mpsc::channel::<InputEventKind>(64);
        let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<u32>>();

        let join = std::thread::Builder::new()
            .name("input-events".to_string())
            .spawn(move || hook_thread_main(tx, ready_tx, on_foreground, on_clipboard))
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
        // hook + listener + window are released before we return. By the time we get here
        // the thread's message queue exists (we waited for the ready handshake), so the
        // post can't be lost.
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

/// Window procedure for the message-only window: only `WM_CLIPBOARDUPDATE` is handled;
/// everything else falls through to the default handler.
unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_CLIPBOARDUPDATE {
        dispatch(InputEventKind::Clipboard);
        return LRESULT(0);
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
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

/// Registers the window class exactly once per process (the class persists for the
/// process lifetime, so restarting the source reuses it).
fn register_class_once() -> Result<()> {
    static REGISTER: Once = Once::new();
    static OK: AtomicBool = AtomicBool::new(false);
    REGISTER.call_once(|| {
        // SAFETY: standard window-class registration with a static class name.
        let ok = unsafe {
            let Ok(module) = GetModuleHandleW(None) else {
                return;
            };
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: HINSTANCE(module.0),
                lpszClassName: CLASS_NAME,
                ..Default::default()
            };
            RegisterClassW(&wc) != 0
        };
        OK.store(ok, Ordering::SeqCst);
    });
    if OK.load(Ordering::SeqCst) {
        Ok(())
    } else {
        bail!("RegisterClassW failed for the input-events window")
    }
}

/// Creates the message-only window (always — it hosts the pump and owns the clipboard
/// listener) and installs **only** the requested hooks: the clipboard listener iff
/// `on_clipboard`, the foreground hook iff `on_foreground`. Returns the foreground hook
/// (`None` when not installed) and whether the clipboard listener was installed, so
/// teardown only releases what was actually registered. On any failure, everything
/// created so far is torn down before returning.
///
/// SAFETY: all calls are standard Win32 window/hook setup on the calling thread.
unsafe fn setup_window_and_hooks(
    on_foreground: bool,
    on_clipboard: bool,
) -> Result<(HWND, Option<HWINEVENTHOOK>, bool)> {
    register_class_once()?;
    let hinstance = HINSTANCE(GetModuleHandleW(None)?.0);

    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        CLASS_NAME,
        w!("ScreenSearch Input Events"),
        WINDOW_STYLE(0),
        0,
        0,
        0,
        0,
        Some(HWND_MESSAGE),
        None,
        Some(hinstance),
        None,
    )?;

    if on_clipboard {
        if let Err(e) = AddClipboardFormatListener(hwnd) {
            let _ = DestroyWindow(hwnd);
            return Err(anyhow!("AddClipboardFormatListener failed: {e}"));
        }
    }

    let hook = if on_foreground {
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
            if on_clipboard {
                let _ = RemoveClipboardFormatListener(hwnd);
            }
            let _ = DestroyWindow(hwnd);
            bail!("SetWinEventHook(EVENT_SYSTEM_FOREGROUND) failed");
        }
        Some(hook)
    } else {
        None
    };

    Ok((hwnd, hook, on_clipboard))
}

/// Hook-thread entry point: set up the window + requested hooks, report readiness, pump
/// messages until `WM_QUIT`, then tear down only what was installed.
fn hook_thread_main(
    tx: Sender<InputEventKind>,
    ready: std::sync::mpsc::Sender<Result<u32>>,
    on_foreground: bool,
    on_clipboard: bool,
) {
    EVENT_TX.with(|cell| *cell.borrow_mut() = Some(tx));

    // SAFETY: the setup + pump + teardown all run on this single thread.
    let (hwnd, hook, clipboard_installed) =
        match unsafe { setup_window_and_hooks(on_foreground, on_clipboard) } {
            Ok(v) => v,
            Err(e) => {
                let _ = ready.send(Err(e));
                EVENT_TX.with(|cell| *cell.borrow_mut() = None);
                return;
            }
        };

    // SAFETY: thread-id read with no arguments.
    let thread_id = unsafe { GetCurrentThreadId() };
    let _ = ready.send(Ok(thread_id));

    // Message pump: GetMessageW returns 0 on WM_QUIT and -1 on error — break on both.
    // SAFETY: standard Win32 message loop driving our window + out-of-context hook.
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

        // Teardown — release only what was installed, in reverse order.
        if let Some(hook) = hook {
            let _ = UnhookWinEvent(hook);
        }
        if clipboard_installed {
            let _ = RemoveClipboardFormatListener(hwnd);
        }
        let _ = DestroyWindow(hwnd);
    }

    EVENT_TX.with(|cell| *cell.borrow_mut() = None);
}

#[cfg(test)]
mod tests {
    use super::InputEventSource;

    /// The #1 lifecycle risk: a message-pump thread that leaks or fails to terminate
    /// would leak the hook + window and could hang `stop`/`reload`. Start and drop the
    /// source many times; `Drop` must post `WM_QUIT`, the thread must exit, and the join
    /// must return — no leak, no hang, no panic. Needs a real desktop (USER32 message
    /// pump), so it is `#[ignore]`d in CI; run locally with
    /// `cargo test -p capture -- --ignored`.
    #[test]
    #[ignore = "requires a real desktop (USER32 message pump); run locally"]
    fn source_starts_and_stops_cleanly_repeatedly() {
        for i in 0..50 {
            // Exercise both install paths (foreground hook + clipboard listener).
            let source = InputEventSource::start(true, true)
                .unwrap_or_else(|e| panic!("start input-events source on iteration {i}: {e}"));
            // `Drop` posts WM_QUIT and joins the hook thread; a leak/hang surfaces as a
            // hung test, a panic as a failure.
            drop(source);
        }
    }
}
