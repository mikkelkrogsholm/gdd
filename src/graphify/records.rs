use super::{Fail, GraphifyOptions, ImportReport, Pass, Unknown, issue};
use crate::report::{Diagnostic, quoted};
use num_bigint::BigInt;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

#[derive(Ord, PartialOrd, Eq, PartialEq)]
pub(super) enum Identity {
    Text(String),
    // Signed coefficient with no trailing zeros, and its power of ten.
    Number(String, BigInt),
}

pub(super) fn identity(value: &Value) -> Option<Identity> {
    match value {
        Value::String(s) if !s.is_empty() => Some(Identity::Text(s.clone())),
        Value::Number(n) => {
            let raw = n.as_str();
            let magnitude = raw.strip_prefix('-').unwrap_or(raw);
            let (mantissa, exp) = magnitude.split_once(['e', 'E']).unwrap_or((magnitude, "0"));
            let (integer, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
            let digits = format!("{integer}{fraction}");
            let digits = digits.trim_start_matches('0');
            if digits.is_empty() {
                return Some(Identity::Number("0".into(), BigInt::from(0)));
            }
            let significant = digits.trim_end_matches('0');
            let exponent = exp.parse::<BigInt>().expect("validated JSON exponent")
                + (digits.len() - significant.len())
                - fraction.len();
            let coefficient = if raw.starts_with('-') {
                format!("-{significant}")
            } else {
                significant.into()
            };
            Some(Identity::Number(coefficient, exponent))
        }
        _ => None,
    }
}

fn classification(node: &Value) -> &'static str {
    match node["file_type"].as_str() {
        Some("document" | "paper" | "image" | "rationale" | "concept" | "video" | "audio") => {
            "noncode"
        }
        Some("code") if node["source_file"].as_str().is_some_and(code_extension) => {
            "code_candidate"
        }
        _ => "unknown",
    }
}

fn code_extension(path: &str) -> bool {
    let name = path
        .rsplit('/')
        .find(|part| !matches!(*part, "" | "."))
        .unwrap_or("");
    let Some((stem, extension)) = name.rsplit_once('.') else {
        return false;
    };
    const EXTENSIONS: &[&str] = &[
        "py", "pyi", "js", "jsx", "ts", "tsx", "mjs", "cjs", "go", "rs", "java", "kt", "kts", "c",
        "h", "cpp", "hpp", "cs", "rb", "php", "swift", "scala", "sh", "bash", "zsh", "sql", "vue",
        "svelte", "ex", "exs", "erl", "hs", "lua", "r", "R", "dart", "html", "css", "scss",
    ];
    !stem.is_empty() && EXTENSIONS.contains(&extension)
}

fn record(kind: &str, revision: &str, path: &str, raw: &Value) -> Value {
    let hash = crate::sha256(format!("{revision}{path}").as_bytes());
    let mut item = json!({
        "id": format!("observation.{kind}.h{hash}"),
        "identity": {"artifact_revision": revision, "pointer": path}
    });
    // Assign producer-owned values directly, including keys reserved by serde.
    item["raw"] = raw.clone();
    item
}

fn nonempty(value: &Value) -> bool {
    value.as_str().is_some_and(|s| !s.is_empty())
}

struct NodeMatch {
    first: usize,
    count: usize,
}

type Identities = BTreeMap<Identity, NodeMatch>;

pub(super) fn project(
    data: &Value,
    revision: &str,
    edge_key: &str,
    options: GraphifyOptions<'_>,
    revisions: &Map<String, Value>,
    index: &mut Value,
    report: &mut ImportReport,
) {
    let mut nodes = Vec::new();
    let mut opaque = Vec::new();
    let mut candidates = Vec::new();
    let mut identities = Identities::new();
    let mut errors = Vec::new();
    let mut code = 0;
    let mut unknown = 0;
    let ast = edge_key == "edges";
    for (number, node) in data["nodes"].as_array().unwrap().iter().enumerate() {
        let path = format!("/nodes/{number}");
        let mut item = record("node", revision, &path, node);
        let class = classification(node);
        code += usize::from(class == "code_candidate");
        unknown += usize::from(class == "unknown");
        item["producer_id"] = node["id"].clone();
        item["classification"] = class.into();
        nodes.push(item);
        if ast
            && node.is_object()
            && (!nonempty(&node["label"])
                || !nonempty(&node["source_file"])
                || !matches!(
                    node["file_type"].as_str(),
                    Some("code" | "document" | "paper" | "image" | "rationale" | "concept")
                ))
        {
            errors.push(issue(
                "SCHEMA_INVALID",
                Fail,
                "AST extraction node requires label, source_file and a known file_type.",
                &path,
            ));
        }
        let Some(key) = identity(&node["id"]) else {
            errors.push(issue("SCHEMA_INVALID", Fail, "Node needs a nonempty string or finite numeric id; booleans/null/objects are not identities.", &path));
            continue;
        };
        identities
            .entry(key)
            .or_insert(NodeMatch {
                first: number,
                count: 0,
            })
            .count += 1;
        if nonempty(&node["source_file"]) {
            let mut candidate = json!({
                "observation": nodes[number]["id"], "resource_claim": node["source_file"],
                "selector_claim": null, "status": "unverified", "classification": class,
                "code_revision_claims": {"caller": options.code_revision}
            });
            candidate["selector_claim"] = node["source_location"].clone();
            candidate["code_revision_claims"]["producer"] = Value::Object(revisions.clone());
            candidates.push(candidate);
        }
        for field in ["community", "community_name"] {
            if let Some(value) = node.get(field) {
                opaque.push(record(
                    "community",
                    revision,
                    &format!("{path}/{field}"),
                    value,
                ));
            }
        }
    }
    let mut duplicates: Vec<_> = identities
        .values()
        .filter(|entry| entry.count > 1)
        .collect();
    duplicates.sort_unstable_by_key(|entry| entry.first);
    for entry in duplicates {
        let id = &nodes[entry.first]["producer_id"];
        let label = match id.as_str() {
            Some(s) => format!("('string', {})", quoted(s)),
            None => format!("('number', Decimal('{}'))", id),
        };
        errors.push(issue(
            "ID_COLLISION",
            Fail,
            format!("Duplicate producer node identity {label}; no first/last-wins resolution."),
            "/nodes",
        ));
    }
    report.check(
        "nodes",
        if errors.is_empty() { Pass } else { Fail },
        "Each supplied node retained; typed producer identities checked.",
        errors,
    );

    let edges = observe_edges(data, revision, edge_key, &nodes, &identities, report);
    for field in ["hyperedges", "communities"] {
        for (container, prefix) in [(data, ""), (&data["graph"], "/graph")] {
            if let Some(value) = container.get(field) {
                opaque.push(record(
                    "opaque",
                    revision,
                    &format!("{prefix}/{field}"),
                    value,
                ));
            }
        }
    }
    let noncode = nodes.len() - code - unknown;
    let incomplete = unknown > 0 || code == 0;
    report.result("code_candidates", if incomplete {Unknown} else {Pass},
        format!("{code} producer-claimed code candidates; {noncode} noncode observations retained; {unknown} unclassified. No implementation verdict is produced."),
        incomplete.then_some("GRAPHIFY_INPUT_PROVENANCE_UNKNOWN"));
    index["nodes"] = Value::Array(nodes);
    index["edges"] = Value::Array(edges);
    index["opaque"] = Value::Array(opaque);
    index["binding_candidates"] = Value::Array(candidates);
}

fn observe_edges(
    data: &Value,
    revision: &str,
    edge_key: &str,
    nodes: &[Value],
    identities: &Identities,
    report: &mut ImportReport,
) -> Vec<Value> {
    let ast = edge_key == "edges";
    let mut edges = Vec::new();
    let mut errors = Vec::new();
    let mut unknown_direction = Vec::new();
    for (number, edge) in data[edge_key].as_array().unwrap().iter().enumerate() {
        let path = format!("/{edge_key}/{number}");
        let mut item = record("edge", revision, &path, edge);
        item["producer_id"] = edge["id"].clone();
        item["source_observation"] = Value::Null;
        item["target_observation"] = Value::Null;
        item["direction"] = json!({"value": "unknown", "basis": null});
        if ast
            && edge.is_object()
            && (!nonempty(&edge["source_file"])
                || !matches!(
                    edge["confidence"].as_str(),
                    Some("EXTRACTED" | "INFERRED" | "AMBIGUOUS")
                ))
        {
            errors.push(issue(
                "SCHEMA_INVALID",
                Fail,
                "AST extraction edge requires source_file and a known confidence label.",
                &path,
            ));
        }
        if !nonempty(&edge["relation"]) {
            errors.push(issue(
                "SCHEMA_INVALID",
                Fail,
                "Edge needs an object with a nonempty relation label.",
                &path,
            ));
        } else if let Some((source, target)) = endpoints(
            data,
            edge,
            &path,
            ast,
            &mut item,
            &mut errors,
            &mut unknown_direction,
        ) {
            for (side, key) in [("source", source), ("target", target)] {
                match key.and_then(|k| identities.get(&k)) {
                    Some(matched) if matched.count == 1 => {
                        item[format!("{side}_observation")] = nodes[matched.first]["id"].clone();
                    }
                    _ => errors.push(issue(
                        "REFERENCE_UNRESOLVED",
                        Fail,
                        format!("{side} endpoint is missing, invalid or ambiguous."),
                        &format!("{path}/{side}"),
                    )),
                }
            }
        }
        edges.push(item);
    }
    let edges_ok = errors.is_empty();
    report.check("edges", if edges_ok {Pass} else {Fail}, "Every edge indexed independently, including identical records and duplicate producer edge IDs.", errors);
    report.check(
        "direction",
        if edges_ok && unknown_direction.is_empty() {
            Pass
        } else {
            Unknown
        },
        "Direction is a producer/profile assertion, never proof of implementation behavior.",
        unknown_direction,
    );

    edges
}

fn endpoints(
    data: &Value,
    edge: &Value,
    path: &str,
    ast: bool,
    item: &mut Value,
    errors: &mut Vec<Diagnostic>,
    unknown: &mut Vec<Diagnostic>,
) -> Option<(Option<Identity>, Option<Identity>)> {
    let mut source = identity(&edge["source"]);
    let mut target = identity(&edge["target"]);
    let directed = data["directed"] == true;
    let basis = if edge.get("_src").is_some() || edge.get("_tgt").is_some() {
        let src = identity(&edge["_src"]);
        let tgt = identity(&edge["_tgt"]);
        let same_order = src == source && tgt == target;
        let reversed = src == target && tgt == source;
        if src.is_none() || tgt.is_none() || !(same_order || reversed) {
            errors.push(issue(
                "SCHEMA_INVALID",
                Fail,
                "Direction markers must be complete and refer to the stored endpoints.",
                path,
            ));
            return None;
        }
        if directed && !same_order {
            errors.push(issue(
                "SCHEMA_INVALID",
                Fail,
                "Direction markers contradict the explicitly directed stored endpoints.",
                path,
            ));
            return None;
        }
        (source, target) = (src, tgt);
        format!("{path}/_src")
    } else if directed {
        "/directed".into()
    } else if ast && data.get("directed").is_none() {
        format!("profile:{}", super::AST_PROFILE)
    } else {
        unknown.push(issue("GRAPHIFY_DIRECTION_UNKNOWN", Unknown, "Stored endpoint order is preserved, but original direction is not established by this export.", path));
        return Some((source, target));
    };
    item["direction"] = json!({"value": "source_to_target", "basis": basis});
    Some((source, target))
}
