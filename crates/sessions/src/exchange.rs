//! Conservative best-effort exchange extraction over filtered `content_text` (D8).

use std::collections::HashSet;

use traits::{ExtractedExchange, SessionArtifactRole, SessionContent};

/// Tool-specific, explicit role markers only. Unmarked prose is ignored and consecutive
/// duplicate captures collapse, so repeated screen frames do not become repeated exchanges.
pub fn extract_exchanges(tool_id: &str, contents: &[SessionContent]) -> Vec<ExtractedExchange> {
    let mut ordered: Vec<&SessionContent> = contents.iter().collect();
    ordered.sort_by_key(|c| (c.captured_at, c.frame_id));
    let mut out = Vec::new();
    let mut seen: HashSet<(SessionArtifactRole, String)> = HashSet::new();
    for frame in ordered {
        let candidates = if tool_id == "codex" {
            codex_desktop_blocks(&frame.content_text)
        } else {
            blocks(tool_id, &frame.content_text)
        };
        for (role, content) in candidates {
            let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
            let duplicate = seen.iter().any(|(seen_role, seen_content)| {
                *seen_role == role
                    && (seen_content.contains(&normalized) || normalized.contains(seen_content))
            });
            if normalized.is_empty() || duplicate {
                continue;
            }
            seen.insert((role, normalized));
            out.push(ExtractedExchange {
                role,
                content,
                frame_id: frame.frame_id,
            });
        }
    }
    out
}

/// Codex desktop's flattened UIA/OCR text has no reliable `You:`/`Codex:` headings: a bare
/// `Codex` in the navigation chrome caused the original generic parser to consume the whole
/// screen as an agent turn. Use two app-specific, observed structural markers instead:
/// - the prompt icon `Q`, bounded by the next `File` menu label, inside the Codex nav signature;
/// - `Working/Worked for <duration>`, followed by at most four response lines and stopped by
///   known controls/tool rows.
///
/// If the signature or a boundary is absent, emit nothing. D8 favors omission over invented roles.
fn codex_desktop_blocks(text: &str) -> Vec<(SessionArtifactRole, String)> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();
    let has_nav_signature = ["Codex", "New task", "Projects"]
        .iter()
        .all(|needle| lines.contains(needle));
    if !has_nav_signature {
        return Vec::new();
    }

    let mut out = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if *line == "Q" {
            let tail = &lines[index + 1..lines.len().min(index + 9)];
            if let Some(file_index) = tail.iter().position(|candidate| *candidate == "File") {
                let content = tail[..file_index].join("\n");
                if !content.is_empty() {
                    out.push((SessionArtifactRole::User, content));
                }
            }
            continue;
        }
        if !is_codex_work_status(line) {
            continue;
        }
        let content = lines[index + 1..]
            .iter()
            .take_while(|candidate| !is_codex_response_stop(candidate))
            .take(4)
            .copied()
            .collect::<Vec<_>>()
            .join("\n");
        if !content.is_empty() {
            out.push((SessionArtifactRole::Agent, content));
        }
    }
    out
}

fn is_codex_work_status(line: &str) -> bool {
    ["Working for ", "Worked for "].iter().any(|prefix| {
        line.strip_prefix(prefix).is_some_and(|duration| {
            duration.ends_with('s')
                && duration
                    .chars()
                    .all(|c| c.is_ascii_digit() || matches!(c, 'h' | 'm' | 's' | ' '))
        })
    })
}

fn is_codex_response_stop(line: &&str) -> bool {
    line.starts_with('@')
        || line.chars().all(|c| c.is_ascii_digit())
        || matches!(
            *line,
            "File"
                | "Codex"
                | "New task"
                | "Scheduled"
                | "Plugins"
                | "Chat"
                | "Projects"
                | "Tasks"
                | "Outputs"
                | "Sources"
                | "Web search"
                | "View all"
                | "Ask for follow-up changes"
                | "Full access"
        )
}

fn blocks(tool_id: &str, text: &str) -> Vec<(SessionArtifactRole, String)> {
    let mut out = Vec::new();
    let mut current: Option<(SessionArtifactRole, Vec<String>)> = None;

    let flush = |current: &mut Option<(SessionArtifactRole, Vec<String>)>,
                 out: &mut Vec<(SessionArtifactRole, String)>| {
        if let Some((role, lines)) = current.take() {
            let content = lines.join("\n").trim().to_string();
            if !content.is_empty() {
                out.push((role, content));
            }
        }
    };

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if let Some((_, lines)) = &mut current {
                lines.push(String::new());
            }
            continue;
        }
        if let Some((role, inline)) = marker(tool_id, trimmed) {
            flush(&mut current, &mut out);
            current = Some((role, inline.into_iter().collect()));
        } else if let Some((_, lines)) = &mut current {
            lines.push(trimmed.to_string());
        }
    }
    flush(&mut current, &mut out);
    out
}

fn marker(tool_id: &str, line: &str) -> Option<(SessionArtifactRole, Option<String>)> {
    if tool_id == "claude-code" {
        if let Some(rest) = line.strip_prefix("❯") {
            return nonempty(rest.trim().to_string())
                .map(|content| (SessionArtifactRole::User, Some(content)));
        }
        for prefix in ["⏺", "●"] {
            if let Some(rest) = line.strip_prefix(prefix) {
                return Some((
                    SessionArtifactRole::Agent,
                    nonempty(rest.trim().to_string()),
                ));
            }
        }
    }

    // Desktop/browser role names without punctuation are common navigation labels. Require the
    // explicit colon form outside the stronger Claude-Code and Codex-specific structures.
    let (head, tail_text) = line.split_once(':')?;
    let tail = nonempty(tail_text.trim().to_string());
    let head = head.trim().to_ascii_lowercase();
    let role = match tool_id {
        "claude-code" => match head.as_str() {
            "user" | "human" | "you" => Some(SessionArtifactRole::User),
            "assistant" | "claude" => Some(SessionArtifactRole::Agent),
            _ => None,
        },
        "codex" => match head.as_str() {
            "user" | "you" => Some(SessionArtifactRole::User),
            "assistant" | "codex" | "chatgpt" => Some(SessionArtifactRole::Agent),
            _ => None,
        },
        "claude-desktop" => match head.as_str() {
            "user" | "you" => Some(SessionArtifactRole::User),
            "assistant" | "claude" => Some(SessionArtifactRole::Agent),
            _ => None,
        },
        "browser-ai" => match head.as_str() {
            "user" | "you" => Some(SessionArtifactRole::User),
            "assistant" | "claude" | "chatgpt" | "gemini" => Some(SessionArtifactRole::Agent),
            _ => None,
        },
        _ => None,
    }?;
    Some((role, tail))
}

fn nonempty(value: String) -> Option<String> {
    (!value.is_empty()).then_some(value)
}
