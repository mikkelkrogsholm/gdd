use crate::{input, python_imports::PythonVersion, validation::to_json};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeSet, io};

/// Caller-declared CPython target. Decoding this descriptor never runs an interpreter.
#[derive(Debug)]
pub struct PythonRuntime {
    pub(super) version: String,
    pub(super) grammar: PythonVersion,
    pub(super) names: BTreeSet<String>,
    provenance: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Descriptor {
    version: String,
    #[serde(rename = "impl")]
    implementation: String,
    platform: String,
    builtin_names: Vec<String>,
    frozen_names: Vec<String>,
}

impl PythonRuntime {
    /// Read the JSON object containing version, impl, platform, builtin_names and frozen_names.
    pub fn from_json(bytes: &[u8]) -> io::Result<Self> {
        let text = std::str::from_utf8(bytes).map_err(io::Error::other)?;
        let json = to_json(&input::parse_json(text).map_err(io::Error::other)?)?;
        let d: Descriptor = serde_json::from_value(json.clone())?;
        let invalid = |s| io::Error::new(io::ErrorKind::InvalidInput, s);
        if !d.implementation.eq_ignore_ascii_case("cpython") || d.platform.trim().is_empty() {
            return Err(invalid(
                "Select an explicit CPython target and nonempty platform.",
            ));
        }
        let version: Vec<_> = d.version.split('.').collect();
        let ["3", minor, patch] = version.as_slice() else {
            return Err(invalid(
                "Select a CPython 3.11–3.14 major.minor.patch version.",
            ));
        };
        if patch.is_empty()
            || !patch.bytes().all(|b| b.is_ascii_digit())
            || patch.len() > 1 && patch.starts_with('0')
        {
            return Err(invalid("Invalid CPython patch version."));
        }
        let grammar = match *minor {
            "11" => PythonVersion::PY311,
            "12" => PythonVersion::PY312,
            "13" => PythonVersion::PY313,
            "14" => PythonVersion::PY314,
            _ => {
                return Err(invalid(
                    "Select a supported CPython grammar from 3.11 through 3.14.",
                ));
            }
        };
        let mut names = BTreeSet::new();
        for name in d.builtin_names.into_iter().chain(d.frozen_names) {
            if !name.split('.').all(|part| {
                let mut chars = part.chars();
                chars
                    .next()
                    .is_some_and(|c| c == '_' || unicode_ident::is_xid_start(c))
                    && chars.all(unicode_ident::is_xid_continue)
            }) {
                return Err(invalid(
                    "Runtime module names must be dotted Python identifiers.",
                ));
            }
            names.insert(name);
        }
        Ok(Self {
            version: d.version,
            grammar,
            names,
            provenance: json!({"code":"CODE_RUNTIME_TARGET","message":"The caller supplied this CPython target. The native observer parses sources with Ruff.",
                "platform":d.platform,"descriptor_revision":crate::canonical_digest(&json),"parser":"ruff_python_parser","parser_version":"0.0.10"}),
        })
    }
    pub(super) fn provenance(&self) -> Value {
        self.provenance.clone()
    }
}
