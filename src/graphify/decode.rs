//! Graphify accepts producer-defined JSON keys and requires complete syntax
//! and duplicate-key checks even after the projection depth is exceeded.

use crate::report::quoted;
use json_event_parser::{JsonEvent, SliceJsonParser};
use serde_json::{Map, Value};

enum Frame {
    Object(Map<String, Value>, String),
    Array(Vec<Value>),
}

/// A valid oversized root retains its container kind but no contents.
pub(super) fn decode(raw: &[u8], limit: usize) -> Result<(Value, bool), String> {
    let mut parser = SliceJsonParser::new(raw);
    let mut frames = Vec::new();
    let mut root = Value::Null;
    let mut too_deep = false;
    loop {
        let value = match parser.parse_next().map_err(|error| {
            if crate::input::invalid_unicode_scalar(raw, &error) {
                "Invalid Unicode scalar.".to_owned()
            } else {
                error.to_string()
            }
        })? {
            JsonEvent::StartObject => {
                frames.push(Frame::Object(Map::new(), String::new()));
                too_deep |= frames.len() > limit;
                continue;
            }
            JsonEvent::StartArray => {
                frames.push(Frame::Array(Vec::new()));
                too_deep |= frames.len() > limit;
                continue;
            }
            JsonEvent::ObjectKey(name) => {
                let Some(Frame::Object(fields, key)) = frames.last_mut() else {
                    unreachable!("JSON parser matched an object key")
                };
                if fields.contains_key(name.as_ref()) {
                    return Err(format!("Duplicate JSON key: {}", quoted(&name)));
                }
                *key = name.into_owned();
                // Keys remain necessary for duplicate detection at every depth.
                if too_deep {
                    fields.insert(key.clone(), Value::Null);
                }
                continue;
            }
            JsonEvent::EndArray | JsonEvent::EndObject => match frames.pop().unwrap() {
                Frame::Array(_) if too_deep => Value::Array(Vec::new()),
                Frame::Object(_, _) if too_deep => Value::Object(Map::new()),
                Frame::Array(items) => Value::Array(items),
                Frame::Object(fields, _) => Value::Object(fields),
            },
            JsonEvent::Eof => return Ok((root, too_deep)),
            _ if too_deep => continue,
            JsonEvent::Null => Value::Null,
            JsonEvent::String(s) => Value::String(s.into_owned()),
            JsonEvent::Boolean(b) => Value::Bool(b),
            JsonEvent::Number(s) => {
                Value::Number(s.parse().map_err(|e: serde_json::Error| e.to_string())?)
            }
        };
        match frames.last_mut() {
            None => root = value,
            Some(Frame::Object(fields, key)) => {
                // A container may cross the limit after its key was read.
                fields.insert(
                    std::mem::take(key),
                    if too_deep { Value::Null } else { value },
                );
            }
            Some(Frame::Array(items)) if !too_deep => items.push(value),
            _ => {}
        }
    }
}
