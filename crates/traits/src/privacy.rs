//! Pure privacy-gate helpers shared across crates (`03 §8`).
//!
//! The excluded-apps matcher is pure and unit-tested. It lives here — not in
//! `capture` — so the kernel can apply the *identical* excluded-apps semantics for
//! the where-was-i candidacy filter (`03 §7b`, D9) without depending on the
//! `capture` crate (`03 §2`). The Win32 foreground/lock probes stay in
//! `capture::privacy` (the Windows-API crate); `capture` re-exports this matcher so
//! its call sites and tests are unchanged.

/// Case-insensitive substring match of any `excluded` entry against the foreground
/// app/process name or window title (`privacy.excluded_apps`). Empty entries are
/// ignored so a stray `""` can't match everything.
pub fn is_excluded(app: Option<&str>, title: Option<&str>, excluded: &[String]) -> bool {
    let app = app.unwrap_or_default().to_ascii_lowercase();
    let title = title.unwrap_or_default().to_ascii_lowercase();
    excluded.iter().any(|e| {
        let needle = e.trim().to_ascii_lowercase();
        !needle.is_empty() && (app.contains(&needle) || title.contains(&needle))
    })
}

#[cfg(test)]
mod tests {
    use super::is_excluded;

    fn excluded() -> Vec<String> {
        vec![
            "1Password".to_string(),
            "KeePass".to_string(),
            "Bitwarden".to_string(),
        ]
    }

    #[test]
    fn matches_process_name_case_insensitively() {
        assert!(is_excluded(Some("1password"), None, &excluded()));
        assert!(is_excluded(Some("KeePassXC"), None, &excluded()));
    }

    #[test]
    fn matches_window_title() {
        assert!(is_excluded(
            Some("explorer"),
            Some("Bitwarden — Vault"),
            &excluded()
        ));
    }

    #[test]
    fn allows_unrelated_apps() {
        assert!(!is_excluded(Some("firefox"), Some("Inbox"), &excluded()));
        assert!(!is_excluded(None, None, &excluded()));
    }

    #[test]
    fn empty_excluded_entry_never_matches() {
        assert!(!is_excluded(
            Some("anything"),
            Some("at all"),
            &["".to_string(), "  ".to_string()]
        ));
    }
}
