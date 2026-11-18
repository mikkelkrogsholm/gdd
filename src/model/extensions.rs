use std::collections::BTreeSet;
use std::error::Error;
use std::io;

use jsonschema::{Draft, Registry, Retrieve, Uri, Validator, draft202012, uri};
use serde_json::{Map, Value as JsonValue};

use super::{Graph, Model, entries, escape, get, has_cycle, has_namespace, string, strings};
use crate::input::Value;
use crate::report::{CheckResult, Diagnostic};
use crate::validation::{CORE_KINDS, CORE_RELATIONS, to_json};

pub(super) struct Definition<'a> {
    declaration: &'a Value,
    file: &'a str,
    validator: Option<Validator>,
}

impl<'a> Model<'a> {
    pub(super) fn extensions(&mut self) {
        for (namespace, document) in self.bundle.extensions() {
            if self
                .unsupported_extensions
                .iter()
                .any(|name| name == namespace)
            {
                continue;
            }
            let file = &document.asset.path;
            for registry in ["node_kinds", "relations"] {
                for (name, declaration) in entries(&document.value, registry) {
                    let definitions = if registry == "node_kinds" {
                        &mut self.custom_kinds
                    } else {
                        &mut self.custom_relations
                    };
                    let pointer = format!("/{registry}/{}", escape(name));
                    if !has_namespace(name, namespace)
                        || definitions.contains_key(name.as_str())
                        || CORE_KINDS.contains(&name.as_str())
                        || CORE_RELATIONS.contains(&name.as_str())
                    {
                        self.push(
                            Diagnostic::new(
                                "EXTENSION_OVERRIDE",
                                CheckResult::Fail,
                                format!("Invalid extension name {name}."),
                                Some(file),
                                pointer,
                            ),
                            CheckResult::Fail,
                        );
                        continue;
                    }
                    let schema =
                        get(declaration, "data_schema").expect("schema-validated definition");
                    let validator = match data_schema(schema) {
                        Ok(validator) => validator,
                        Err(error) => {
                            self.push(
                                Diagnostic::new(
                                    "EXTENSION_DATA_INVALID",
                                    CheckResult::Fail,
                                    error,
                                    Some(file),
                                    format!("{pointer}/data_schema"),
                                ),
                                CheckResult::Fail,
                            );
                            continue;
                        }
                    };
                    let unresolved = validator.is_none();
                    definitions.insert(
                        name,
                        Definition {
                            declaration,
                            file,
                            validator,
                        },
                    );
                    if unresolved {
                        self.push(Diagnostic::new("EXTENSION_UNSUPPORTED", CheckResult::Unknown,
                            "Extension schema contains a local reference cycle that does not descend into data.", Some(file), format!("{pointer}/data_schema")), CheckResult::Unknown);
                    }
                }
            }
        }
        let mut diagnostics = Vec::new();
        for (&name, definition) in &self.custom_relations {
            for field in ["from", "to"] {
                for (index, kind) in strings(definition.declaration, field).enumerate() {
                    if !CORE_KINDS.contains(&kind)
                        && !self.custom_kinds.contains_key(kind)
                        && !self.unsupported_kind(kind)
                    {
                        diagnostics.push(Diagnostic::new(
                            "ENDPOINT_KIND_INVALID",
                            CheckResult::Fail,
                            format!("Unknown extension endpoint kind {kind}."),
                            Some(definition.file),
                            format!("/relations/{}/{field}/{index}", escape(name)),
                        ));
                    }
                }
            }
        }
        for diagnostic in diagnostics {
            self.push(diagnostic, CheckResult::Fail);
        }
    }

    pub(super) fn unsupported_kind(&self, name: &str) -> bool {
        self.namespaces
            .iter()
            .copied()
            .filter(|namespace| has_namespace(name, namespace))
            .max_by_key(|namespace| namespace.len())
            .is_some_and(|namespace| {
                self.unsupported_extensions
                    .iter()
                    .any(|item| item == namespace)
            })
    }

    pub(super) fn custom(&mut self, name: &str, relation: bool, id: &str, record: &Value) {
        let definitions = if relation {
            &self.custom_relations
        } else {
            &self.custom_kinds
        };
        let Some(definition) = definitions.get(name) else {
            if self.unsupported_kind(name) {
                self.unresolved(
                    "EXTENSION_UNSUPPORTED",
                    format!("Unsupported semantics for {name}."),
                    id,
                    "",
                );
            } else {
                self.issue(
                    "EXTENSION_UNDECLARED",
                    format!("No extension defines {name}."),
                    id,
                    "",
                );
            }
            return;
        };

        let declaration = definition.declaration;
        if let Some(validator) = &definition.validator {
            let payload = get(record, "data").map(to_json).transpose();
            match payload {
                Ok(payload) => {
                    let payload = payload.unwrap_or_else(|| JsonValue::Object(Map::new()));
                    let issues: Vec<_> = validator
                        .iter_errors(&payload)
                        .map(|error| (error.to_string(), format!("/data{}", error.instance_path())))
                        .collect();
                    for (message, pointer) in issues {
                        self.issue("EXTENSION_DATA_INVALID", message, id, &pointer);
                    }
                }
                Err(error) => {
                    self.unresolved("EXTENSION_UNSUPPORTED", error.to_string(), id, "/data")
                }
            }
        } else {
            self.unresolved(
                "EXTENSION_UNSUPPORTED",
                format!("Schema for {name} is unresolved."),
                id,
                "/data",
            );
        }

        if relation {
            for field in ["from", "to"] {
                let target = string(record, field);
                if self.kind(target).is_some() {
                    let allowed: Vec<_> = strings(declaration, field).collect();
                    self.reference(target, &allowed, id, &format!("/{field}"));
                }
            }
        }
    }
}

const DRAFT: &str = "https://json-schema.org/draft/2020-12/schema";
const BASE: &str = "https://gdd.example/extension-data";

struct LocalOnly;

impl Retrieve for LocalOnly {
    fn retrieve(&self, uri: &Uri<String>) -> Result<JsonValue, Box<dyn Error + Send + Sync>> {
        Err(io::Error::other(format!("External schema references are forbidden: {uri}.")).into())
    }
}

fn data_schema(schema: &Value) -> Result<Option<Validator>, String> {
    let schema = to_json(schema).map_err(|error| error.to_string())?;
    let registry = Registry::new()
        .draft(Draft::Draft202012)
        .retriever(LocalOnly)
        .add(BASE, &schema)
        .and_then(|builder| builder.prepare())
        .map_err(|error| error.to_string())?;
    let base = uri::from_str(BASE).expect("the extension base URI is valid");
    let resolver = registry.resolver(base);
    let root = resolver
        .lookup("#")
        .map_err(|error| error.to_string())?
        .contents();

    // References can activate an object under any keyword. Walk only schema
    // positions and resolved targets so unreferenced annotations stay inert.
    let address = |value: &JsonValue| std::ptr::from_ref(value).addr();
    let mut pending = vec![root];
    let mut visited = BTreeSet::new();
    let mut evaluated = Graph::new();
    let mut same_instance = Graph::new();
    while let Some(value) = pending.pop() {
        let key = address(value);
        if !visited.insert(key) {
            continue;
        }
        draft202012::meta::validate(value).map_err(|error| error.to_string())?;
        let Some(object) = value.as_object() else {
            continue;
        };
        if object.contains_key("$id")
            || object.contains_key("x-executable")
            || object
                .get("$schema")
                .is_some_and(|value| value.as_str() != Some(DRAFT))
        {
            return Err(
                "Active schemas must use Draft 2020-12 without IDs or executable hooks.".into(),
            );
        }
        let mut evaluated_children = Vec::new();
        let mut same_instance_children = Vec::new();
        for (keyword, child) in object {
            match keyword.as_str() {
                "$ref" | "$dynamicRef" => {
                    let reference = child
                        .as_str()
                        .filter(|reference| reference.starts_with('#'))
                        .ok_or("Schema references must remain local.")?;
                    let target = resolver
                        .lookup(reference)
                        .map_err(|error| error.to_string())?
                        .contents();
                    same_instance_children.push(target);
                }
                "not" | "if" | "then" | "else" => same_instance_children.push(child),
                "allOf" | "anyOf" | "oneOf" => {
                    same_instance_children.extend(child.as_array().into_iter().flatten())
                }
                "dependentSchemas" => same_instance_children
                    .extend(child.as_object().into_iter().flat_map(Map::values)),
                "properties" | "patternProperties" => {
                    evaluated_children.extend(child.as_object().into_iter().flat_map(Map::values))
                }
                "prefixItems" => evaluated_children.extend(child.as_array().into_iter().flatten()),
                "additionalProperties"
                | "unevaluatedProperties"
                | "propertyNames"
                | "contains"
                | "items"
                | "unevaluatedItems" => evaluated_children.push(child),
                "$defs" | "definitions" => {
                    pending.extend(child.as_object().into_iter().flat_map(Map::values))
                }
                "contentSchema" => pending.push(child),
                _ => {}
            }
        }
        same_instance.insert(
            key,
            same_instance_children
                .iter()
                .map(|child| address(child))
                .collect(),
        );
        evaluated_children.extend(same_instance_children);
        evaluated.insert(
            key,
            evaluated_children
                .iter()
                .map(|child| address(child))
                .collect(),
        );
        pending.extend(evaluated_children);
    }

    // A recursion through a property or array item consumes data. A cycle of
    // references/applicators at the same instance must not silently pass.
    let mut reachable = BTreeSet::new();
    let mut pending = vec![address(root)];
    while let Some(key) = pending.pop() {
        if reachable.insert(key)
            && let Some(children) = evaluated.get(&key)
        {
            pending.extend(children.iter().copied());
        }
    }
    same_instance.retain(|key, _| reachable.contains(key));
    if has_cycle(&same_instance) {
        return Ok(None);
    }

    draft202012::options()
        .offline()
        .should_validate_formats(false)
        .with_base_uri(BASE)
        .with_registry(&registry)
        .build(&schema)
        .map(Some)
        .map_err(|error| error.to_string())
}
