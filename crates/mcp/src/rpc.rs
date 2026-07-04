//! Minimal JSON-RPC 2.0 over newline-delimited stdio (the MCP stdio transport).
//!
//! Hand-rolled (no SDK) per the PR8 decision: six static tools and no server-initiated
//! messages, so the protocol surface is small and fully owned here. Every outgoing message
//! is a single UTF-8 line; `serde_json` compact output escapes any newline **inside** a
//! string as `\n`, so a response never contains a raw newline — satisfying the transport's
//! "MUST NOT contain embedded newlines" rule.

use serde_json::{json, Value};

/// Invalid JSON was received.
pub const PARSE_ERROR: i64 = -32700;
/// The JSON was not a valid JSON-RPC request object.
pub const INVALID_REQUEST: i64 = -32600;
/// The method does not exist.
pub const METHOD_NOT_FOUND: i64 = -32601;
/// Invalid method parameters (used for an unknown tool name in `tools/call`).
pub const INVALID_PARAMS: i64 = -32602;

/// A well-formed JSON-RPC request (has a non-null `id`, so it must be answered).
#[derive(Debug, Clone)]
pub struct Request {
    pub id: Value,
    pub method: String,
    pub params: Value,
}

/// A JSON-RPC notification (no `id`, never answered).
#[derive(Debug, Clone)]
pub struct Notification {
    pub method: String,
    pub params: Value,
}

/// The classification of one incoming line.
#[derive(Debug, Clone)]
pub enum Message {
    /// A request to dispatch.
    Request(Request),
    /// A notification to handle silently.
    Notification(Notification),
    /// Send this response object verbatim (parse error, batch rejection, or an invalid
    /// request that still carried an id).
    Error(Value),
    /// Malformed with no id to answer — drop silently (JSON-RPC forbids responding).
    Drop,
}

/// Classifies one line of stdin into a [`Message`].
pub fn classify(line: &str) -> Message {
    let value: Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(_) => return Message::Error(error(Value::Null, PARSE_ERROR, "Parse error")),
    };

    // Batches (arrays) were removed in MCP 2025-06-18 and no stdio client emits them.
    if value.is_array() {
        return Message::Error(error(
            Value::Null,
            INVALID_REQUEST,
            "JSON-RPC batching is not supported",
        ));
    }

    let obj = match value.as_object() {
        Some(o) => o,
        None => return Message::Error(error(Value::Null, INVALID_REQUEST, "Invalid Request")),
    };

    let id = obj.get("id").cloned();
    let method = obj.get("method").and_then(|m| m.as_str());
    let jsonrpc_ok = obj.get("jsonrpc").and_then(|v| v.as_str()) == Some("2.0");

    match (method, jsonrpc_ok) {
        (Some(method), true) => {
            let params = obj.get("params").cloned().unwrap_or(Value::Null);
            match id {
                // MCP requests always carry a non-null id; notifications never carry one.
                Some(id) if !id.is_null() => Message::Request(Request {
                    id,
                    method: method.to_string(),
                    params,
                }),
                _ => Message::Notification(Notification {
                    method: method.to_string(),
                    params,
                }),
            }
        }
        // Malformed (missing/invalid method or wrong jsonrpc). Answer only if there's an id.
        _ => match id {
            Some(id) if !id.is_null() => {
                Message::Error(error(id, INVALID_REQUEST, "Invalid Request"))
            }
            _ => Message::Drop,
        },
    }
}

/// Builds a JSON-RPC success response.
pub fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

/// Builds a JSON-RPC error response.
pub fn error(id: Value, code: i64, message: &str) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_error_yields_32700_with_null_id() {
        match classify("not json") {
            Message::Error(v) => {
                assert_eq!(v["id"], Value::Null);
                assert_eq!(v["error"]["code"], PARSE_ERROR);
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn batch_array_yields_32600() {
        match classify("[{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}]") {
            Message::Error(v) => {
                assert_eq!(v["id"], Value::Null);
                assert_eq!(v["error"]["code"], INVALID_REQUEST);
                assert!(v["error"]["message"].as_str().unwrap().contains("batching"));
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn valid_request_is_parsed() {
        match classify(r#"{"jsonrpc":"2.0","id":7,"method":"tools/list","params":{}}"#) {
            Message::Request(r) => {
                assert_eq!(r.id, json!(7));
                assert_eq!(r.method, "tools/list");
            }
            other => panic!("expected Request, got {other:?}"),
        }
    }

    #[test]
    fn notification_has_no_id() {
        match classify(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#) {
            Message::Notification(n) => assert_eq!(n.method, "notifications/initialized"),
            other => panic!("expected Notification, got {other:?}"),
        }
    }

    #[test]
    fn malformed_object_with_id_is_invalid_request() {
        // Missing `method`.
        match classify(r#"{"jsonrpc":"2.0","id":3}"#) {
            Message::Error(v) => {
                assert_eq!(v["id"], json!(3));
                assert_eq!(v["error"]["code"], INVALID_REQUEST);
            }
            other => panic!("expected Error, got {other:?}"),
        }
    }

    #[test]
    fn malformed_object_without_id_is_dropped() {
        // No method, no id → nothing to answer.
        assert!(matches!(classify(r#"{"jsonrpc":"2.0"}"#), Message::Drop));
    }

    #[test]
    fn responses_are_single_line_even_with_newlines_in_strings() {
        // A result carrying a raw newline inside a string must serialize without a raw
        // newline in the emitted line (it becomes an escaped \n).
        let resp = success(json!(1), json!({ "text": "line1\nline2" }));
        let line = serde_json::to_string(&resp).unwrap();
        assert!(
            !line.contains('\n'),
            "serialized line must not contain a raw newline"
        );
        assert!(line.contains("\\n"));
    }
}
