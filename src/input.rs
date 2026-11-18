//! Parse one UTF-8 GDD document into JSON-compatible values.

use std::collections::BTreeMap;
use std::fmt;
use std::mem;

use json_event_parser::{JsonEvent, SliceJsonParser};
use libyaml_safer::{EventData, Parser, ScalarStyle};

pub const MAX_CONTAINER_DEPTH: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value {
    Null,
    Boolean(bool),
    /// An exact JSON number lexeme. A YAML float tag on an integer spelling
    /// adds a decimal point to preserve the explicit type in JSON identities.
    Number(String),
    String(String),
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>),
}

impl From<&serde_json::Value> for Value {
    fn from(json: &serde_json::Value) -> Self {
        use serde_json::Value as Json;
        match json {
            Json::Null => Self::Null,
            Json::Bool(value) => Self::Boolean(*value),
            Json::Number(value) => Self::Number(value.to_string()),
            Json::String(value) => Self::String(value.clone()),
            Json::Array(values) => Self::Array(values.iter().map(Self::from).collect()),
            Json::Object(fields) => Self::Object(
                fields
                    .iter()
                    .map(|(key, value)| (key.clone(), Self::from(value)))
                    .collect(),
            ),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum InputError {
    Invalid(String),
    DuplicateKey(String),
    UnsupportedDepth,
}

impl fmt::Display for InputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::DuplicateKey(key) => write!(formatter, "Duplicate mapping key {key:?}."),
            Self::UnsupportedDepth => write!(
                formatter,
                "Input exceeds the {MAX_CONTAINER_DEPTH}-container nesting limit."
            ),
        }
    }
}

impl std::error::Error for InputError {}

/// Parse JSON first, then try YAML on a JSON syntax error. The result contains
/// one document's values. This function applies syntax and value restrictions.
pub fn parse(raw: &[u8]) -> Result<Value, InputError> {
    let source = std::str::from_utf8(raw)
        .map_err(|error| InputError::Invalid(format!("Input must be UTF-8. {error}")))?;
    match read_json(source) {
        Ok(value) => Ok(value),
        Err(JsonError::Input(error)) => Err(error),
        Err(JsonError::Syntax) => read_yaml(source),
    }
}

/// Parse strict JSON without admitting YAML-only syntax.
pub fn parse_json(source: &str) -> Result<Value, InputError> {
    read_json(source).map_err(|error| match error {
        JsonError::Input(error) => error,
        JsonError::Syntax => InputError::Invalid("Invalid JSON syntax.".into()),
    })
}

enum JsonError {
    Syntax,
    Input(InputError),
}

impl From<InputError> for JsonError {
    fn from(error: InputError) -> Self {
        Self::Input(error)
    }
}

fn read_json(source: &str) -> Result<Value, JsonError> {
    let mut parser = SliceJsonParser::new(source.as_bytes());
    let mut document = Builder::default();
    loop {
        match parser.parse_next().map_err(|error| {
            if error.message().contains("surrogate") {
                JsonError::Input(InputError::Invalid("Invalid Unicode scalar.".into()))
            } else {
                JsonError::Syntax
            }
        })? {
            JsonEvent::StartArray => document.start(Frame::array())?,
            JsonEvent::StartObject => document.start(Frame::object())?,
            JsonEvent::EndArray | JsonEvent::EndObject => document.end()?,
            JsonEvent::ObjectKey(key) | JsonEvent::String(key) => {
                document.value(Value::String(key.into_owned()))?;
            }
            JsonEvent::Number(number) => document.value(Value::Number(number.into_owned()))?,
            JsonEvent::Boolean(value) => document.value(Value::Boolean(value))?,
            JsonEvent::Null => document.value(Value::Null)?,
            JsonEvent::Eof => return Ok(document.finish()?),
        }
    }
}

fn read_yaml(source: &str) -> Result<Value, InputError> {
    let mut parser = Parser::new();
    parser.set_input(source.as_bytes());
    let mut document = Builder::default();
    let mut has_document = false;
    loop {
        let event = parser
            .parse()
            .map_err(|error| InputError::Invalid(format!("Invalid YAML. {error}")))?;
        match event.data {
            EventData::StreamStart { .. } | EventData::DocumentEnd { .. } => {}
            EventData::StreamEnd => return document.finish(),
            EventData::DocumentStart { .. } => {
                if mem::replace(&mut has_document, true) {
                    return Err(InputError::Invalid("Expected a single document.".into()));
                }
            }
            EventData::Alias { .. } => return Err(anchor_error()),
            EventData::Scalar {
                anchor,
                tag,
                value,
                style,
                ..
            } => {
                if anchor.is_some() {
                    return Err(anchor_error());
                }
                document.value(yaml_scalar(value, style, tag.as_deref())?)?;
            }
            EventData::SequenceStart { anchor, tag, .. } => {
                check_container_tag(anchor, tag.as_deref(), "tag:yaml.org,2002:seq")?;
                document.start(Frame::array())?;
            }
            EventData::MappingStart { anchor, tag, .. } => {
                check_container_tag(anchor, tag.as_deref(), "tag:yaml.org,2002:map")?;
                document.start(Frame::object())?;
            }
            EventData::SequenceEnd | EventData::MappingEnd => document.end()?,
        }
    }
}

fn anchor_error() -> InputError {
    InputError::Invalid("Anchors and aliases are forbidden.".into())
}

fn tag_error() -> InputError {
    InputError::Invalid("Expected a JSON-compatible YAML tag matching its value.".into())
}

fn check_container_tag(
    anchor: Option<String>,
    tag: Option<&str>,
    expected: &str,
) -> Result<(), InputError> {
    if anchor.is_some() {
        return Err(anchor_error());
    }
    match tag {
        None | Some("!") => Ok(()),
        Some(tag) if tag == expected => Ok(()),
        Some(_) => Err(tag_error()),
    }
}

fn yaml_scalar(value: String, style: ScalarStyle, tag: Option<&str>) -> Result<Value, InputError> {
    match tag {
        Some("!" | "tag:yaml.org,2002:str") => Ok(Value::String(value)),
        Some("tag:yaml.org,2002:null") if value.is_empty() || value == "null" => Ok(Value::Null),
        Some("tag:yaml.org,2002:bool") if matches!(value.as_str(), "true" | "false") => {
            Ok(Value::Boolean(value == "true"))
        }
        Some("tag:yaml.org,2002:int") if !value.contains(['.', 'e', 'E']) && is_number(&value) => {
            Ok(Value::Number(value))
        }
        Some("tag:yaml.org,2002:float") if is_number(&value) => {
            let mut value = value;
            if !value.contains(['.', 'e', 'E']) {
                value.push_str(".0");
            }
            Ok(Value::Number(value))
        }
        Some(_) => Err(tag_error()),
        None if style != ScalarStyle::Plain => Ok(Value::String(value)),
        None => match value.as_str() {
            "" | "null" => Ok(Value::Null),
            "true" => Ok(Value::Boolean(true)),
            "false" => Ok(Value::Boolean(false)),
            _ if is_number(&value) => Ok(Value::Number(value)),
            // Reject the YAML spellings and the non-finite literals accepted by
            // some JSON readers, including when JSON syntax falls back to YAML.
            _ if ["NaN", "Infinity", "-Infinity"].contains(&value.as_str())
                || [".nan", ".inf", "+.inf", "-.inf"]
                    .iter()
                    .any(|spelling| value.eq_ignore_ascii_case(spelling)) =>
            {
                Err(InputError::Invalid("Non-finite number.".into()))
            }
            _ => Ok(Value::String(value)),
        },
    }
}

fn is_number(value: &str) -> bool {
    if !matches!(value.as_bytes().first(), Some(b'-' | b'0'..=b'9')) {
        return false;
    }
    let mut parser = SliceJsonParser::new(value.as_bytes());
    matches!(parser.parse_next(), Ok(JsonEvent::Number(number)) if number == value)
        && matches!(parser.parse_next(), Ok(JsonEvent::Eof))
}

#[derive(Default)]
struct Builder {
    stack: Vec<Frame>,
    root: Option<Value>,
}

enum Frame {
    Array(Vec<Value>),
    Object(BTreeMap<String, Value>, Option<String>),
}

impl Frame {
    fn array() -> Self {
        Self::Array(Vec::new())
    }

    fn object() -> Self {
        Self::Object(BTreeMap::new(), None)
    }
}

impl Builder {
    fn start(&mut self, frame: Frame) -> Result<(), InputError> {
        if self.stack.len() == MAX_CONTAINER_DEPTH {
            return Err(InputError::UnsupportedDepth);
        }
        if let Some(Frame::Object(_, None)) = self.stack.last() {
            return Err(InputError::Invalid("String mapping keys required.".into()));
        }
        self.stack.push(frame);
        Ok(())
    }

    fn end(&mut self) -> Result<(), InputError> {
        match self.stack.pop() {
            Some(Frame::Array(items)) => self.value(Value::Array(items)),
            Some(Frame::Object(items, None)) => self.value(Value::Object(items)),
            _ => Err(InputError::Invalid("Incomplete container.".into())),
        }
    }

    fn value(&mut self, value: Value) -> Result<(), InputError> {
        match self.stack.last_mut() {
            Some(Frame::Array(items)) => items.push(value),
            Some(Frame::Object(items, key)) => {
                if let Some(name) = key.take() {
                    items.insert(name, value);
                } else {
                    let Value::String(name) = value else {
                        return Err(InputError::Invalid("String mapping keys required.".into()));
                    };
                    if name == "<<" {
                        return Err(InputError::Invalid("Merge keys are forbidden.".into()));
                    }
                    if items.contains_key(&name) {
                        return Err(InputError::DuplicateKey(name));
                    }
                    *key = Some(name);
                }
            }
            None => {
                if self.root.replace(value).is_some() {
                    return Err(InputError::Invalid("Expected a single document.".into()));
                }
            }
        }
        Ok(())
    }

    fn finish(self) -> Result<Value, InputError> {
        if !self.stack.is_empty() {
            return Err(InputError::Invalid("Incomplete document.".into()));
        }
        Ok(self.root.unwrap_or(Value::Null))
    }
}
