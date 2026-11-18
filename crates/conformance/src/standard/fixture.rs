//! Prepare private example bundles with the standard suite's ordered JSON encoding.

use anyhow::{Context, Result, bail, ensure};
use gdd::{input, validation::to_json};
use json_patch::{Patch, PatchOperation};
use libyaml_safer::{Document, NodeData, Parser};
use serde_json::{Map, Value};
use std::{fs, path::Path};

/// GDD validation sorts mapping keys. Fixture revisions also cover the order in
/// which the original YAML author wrote those keys. Read that order from YAML
/// while taking all parsed scalar values from the validator's strict reader.
fn ordered_value(document: &Document, index: i32, value: &input::Value) -> Result<Value> {
    let node = document.get_node(index).context("missing YAML node")?;
    match (&node.data, value) {
        (NodeData::Mapping { pairs, .. }, input::Value::Object(fields)) => {
            let mut map = Map::with_capacity(fields.len());
            for pair in pairs {
                let key = document.get_node(pair.key).context("missing mapping key")?;
                let NodeData::Scalar { value: key, .. } = &key.data else {
                    bail!("expected a string key");
                };
                let field = fields.get(key).context("missing parsed field")?;
                map.insert(key.clone(), ordered_value(document, pair.value, field)?);
            }
            Ok(Value::Object(map))
        }
        (NodeData::Sequence { items, .. }, input::Value::Array(values)) => {
            ensure!(items.len() == values.len(), "inconsistent sequence length");
            items
                .iter()
                .zip(values)
                .map(|(&item, value)| ordered_value(document, item, value))
                .collect::<Result<Vec<_>>>()
                .map(Value::Array)
        }
        (NodeData::Scalar { .. }, _) => Ok(to_json(value)?),
        _ => bail!("inconsistent YAML fixture structure"),
    }
}

pub(super) fn read(path: &Path) -> Result<Value> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let value = input::parse(&bytes).with_context(|| format!("parse {}", path.display()))?;
    // JSON strings may contain surrogate-pair escapes that YAML does not accept.
    if let Ok(json) = gdd::graphify::decode_json(&bytes) {
        return Ok(json);
    }
    let mut parser = Parser::new();
    parser.set_input(bytes.as_slice());
    let doc = Document::load(&mut parser)?;
    ordered_value(&doc, 1, &value)
}

pub(super) fn write(path: &Path, value: &Value) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    fs::write(path, bytes).with_context(|| format!("write {}", path.display()))
}

/// Object removal in json-patch swaps in the last key. Preserve the order of the
/// surviving fields, since fixture serialization contributes to the snapshot.
pub(super) fn patch(data: &mut Value, patches: &Patch) -> Result<()> {
    for (index, operation) in patches.iter().enumerate() {
        if let PatchOperation::Remove(remove) = operation
            && let Some((parent, key)) = remove.path.split_back()
            && let Some(fields) = data
                .pointer_mut(parent.as_str())
                .and_then(Value::as_object_mut)
        {
            fields
                .shift_remove(key.decoded().as_ref())
                .with_context(|| format!("missing fixture field at {}", remove.path))?;
            continue;
        }
        json_patch::patch_unsafe(data, std::slice::from_ref(operation))
            .with_context(|| format!("apply fixture patch {index}"))?;
    }
    Ok(())
}

/// Stage the selected example and its fixture dependencies. The evidence cases
/// start from a ledger and validate the associated concert model after mutation.
pub(super) fn stage(source: &Path, target: &Path, base: &Path) -> Result<()> {
    let dependencies: &[&str] = match base.to_str() {
        Some("concert.gdd.yaml") => &["concert.records.yaml", "sources/legacy-concert.gdd.yaml"],
        Some("concert.records.yaml") => &["concert.gdd.yaml", "sources/legacy-concert.gdd.yaml"],
        Some("extension.gdd.yaml") => &["extensions/sensors.extension.json"],
        Some("fragmented.gdd.yaml") => &["note.fragment.yaml"],
        _ => &[],
    };
    for input in std::iter::once(base).chain(dependencies.iter().map(Path::new)) {
        let destination = target.join(input);
        fs::create_dir_all(destination.parent().context("expected fixture parent")?)?;
        fs::copy(source.join(input), destination)
            .with_context(|| format!("copy fixture {}", input.display()))?;
    }
    Ok(())
}
