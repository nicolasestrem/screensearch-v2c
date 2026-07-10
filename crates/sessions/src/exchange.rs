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
        for (role, content) in blocks(tool_id, &frame.content_text) {
            let normalized = content.split_whitespace().collect::<Vec<_>>().join(" ");
            if normalized.is_empty() || !seen.insert((role, normalized)) {
                continue;
            }
            out.push(ExtractedExchange {
                role,
                content,
                frame_id: frame.frame_id,
            });
        }
    }
    out
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
        for prefix in ["❯", ">"] {
            if let Some(rest) = line.strip_prefix(prefix) {
                return Some((SessionArtifactRole::User, nonempty(rest.trim().to_string())));
            }
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

    let (head, tail) = line
        .split_once(':')
        .map_or((line, None), |(h, t)| (h, nonempty(t.trim().to_string())));
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
