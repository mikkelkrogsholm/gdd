//! The model and immutable check definition used by both tutorial examples.

use serde_json::{Value, json};

pub const MODEL: &str = include_str!("../../docs/tutorial/evidence.gdd.json");
pub const METHOD: &str = "rust_integer_addition_examples";
pub const CONTROL: &str = "isolated-subtraction-mutant";

pub fn model() -> Value {
    let mut model: Value = serde_json::from_str(MODEL).expect("embedded tutorial model");
    model["nodes"]["rule.sum"]["rule"]["verification"] = json!([METHOD]);
    model
}

pub fn check(spec_revision: &str) -> Value {
    json!({
        "kind": "check",
        "description": "Run the gdd-tutorial-add checker in docs/tutorial/rust on its five integer addition examples.",
        "spec_revision": spec_revision,
        "scope": ["rule.sum", "scenario.add"],
        "method": METHOD,
        "required_environment": ["rustc"],
        "negative_controls": [CONTROL]
    })
}
