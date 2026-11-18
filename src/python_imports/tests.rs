use super::*;
use serde_json::json;

fn observe(files: &[(&str, &[u8])]) -> Value {
    let sources: BTreeMap<_, _> = files
        .iter()
        .map(|(p, b)| (p.to_string(), b.to_vec()))
        .collect();
    let components = sources.keys().map(|p| (p.clone(), p.clone())).collect();
    analyze_imports(
        &sources,
        &components,
        &BTreeSet::from(["sys".into(), "pkg.frozen".into()]),
    )
}

#[test]
fn every_regular_parent_and_relative_child_are_observed() {
    let result = observe(&[
        ("pkg/__init__.py", b"'docs' ' here'\npass"),
        ("pkg/nested/__init__.py", b"pass"),
        ("pkg/leaf.py", b""),
        (
            "pkg/nested/main.py",
            "raise RuntimeError()\nif False:\n  'é'; from .. import (\n leaf as thing,\n)"
                .as_bytes(),
        ),
    ]);
    assert_eq!(result["diagnostics"], json!([]));
    assert_eq!(
        result["inert_files"],
        json!(["pkg/__init__.py", "pkg/nested/__init__.py"])
    );
    assert_eq!(result["imports"].as_array().unwrap().len(), 3);
    let edges = result["edges"].as_array().unwrap();
    assert!(
        edges
            .iter()
            .any(|e| e["to"] == "pkg/leaf.py" && e["module"] == "pkg.leaf")
    );
    for edge in edges {
        assert_eq!(edge["from"], "pkg/nested/main.py");
        assert_eq!(edge["line"], 3);
        assert_eq!(edge["column"], 8);
        assert_eq!(edge["statement"], "from .. import leaf as thing");
    }
}

#[test]
fn collisions_noninert_initializers_and_namespace_packages_stay_unresolved() {
    let result = observe(&[
        ("main.py", b"import p.x\nfrom q import c\nimport n.x"),
        ("p.py", b""),
        ("p/__init__.py", b""),
        ("p/x.py", b""),
        ("q/__init__.py", b"pass\n'late'"),
        ("q/c.py", b""),
        ("n/x.py", b""),
    ]);
    let edges = result["edges"].as_array().unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0]["to"], "q/__init__.py");
    assert_eq!(result["diagnostics"].as_array().unwrap().len(), 3);
    assert!(
        result["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["code"] == "CODE_IMPORT_UNRESOLVED")
    );
}

#[test]
fn runtime_namespace_uses_full_and_root_names_for_absolute_imports() {
    let result = observe(&[
        ("sys.py", b""),
        ("pkg/__init__.py", b""),
        ("pkg/frozen.py", b""),
        (
            "pkg/main.py",
            b"import sys.sub, pkg.frozen\nfrom .frozen import f",
        ),
    ]);
    assert_eq!(result["diagnostics"], json!([]));
    for row in result["imports"].as_array().unwrap() {
        assert_eq!(row["external"], row["imported"].is_null());
    }
}

#[test]
fn bindings_preserve_self_edges_and_omit_unbound_inert_bootstrap() {
    let sources = BTreeMap::from([
        ("p/main.py".into(), b"import p.leaf, extra".to_vec()),
        ("p/__init__.py".into(), vec![]),
        ("p/leaf.py".into(), vec![]),
        ("extra.py".into(), vec![]),
    ]);
    let components = BTreeMap::from([
        ("p/main.py".into(), "same".into()),
        ("p/leaf.py".into(), "same".into()),
    ]);
    let result = analyze_imports(&sources, &components, &BTreeSet::new());
    assert_eq!(result["edges"].as_array().unwrap().len(), 1);
    assert_eq!(result["edges"][0]["from"], "same");
    assert_eq!(result["edges"][0]["to"], "same");
    assert_eq!(result["diagnostics"].as_array().unwrap().len(), 1);
    assert_eq!(result["diagnostics"][0]["code"], "CODE_UNBOUND_IMPORT");
    assert_eq!(result["diagnostics"][0]["related_ids"], json!(["same"]));
}

#[test]
fn dynamic_capabilities_escape_through_branches_annotations_and_containers() {
    let result = observe(&[(
        "main.py",
        br#"def run():
    f('m')
from importlib import import_module as loader
f: object = g
g = loader
escape = [f]
getattr(thing, 'loader')('m')
import sys as system
vars(system)
system.version
"#,
    )]);
    let locations: BTreeSet<_> = result["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| {
            (
                d["location"]["line"].as_u64().unwrap(),
                d["message"]
                    .as_str()
                    .unwrap()
                    .starts_with("Dynamic loading"),
            )
        })
        .collect();
    assert!(locations.contains(&(2, true)));
    assert!(locations.contains(&(6, false)));
    assert!(locations.contains(&(7, true)));
    assert!(locations.contains(&(9, true)));
    assert!(!locations.iter().any(|p| p.0 == 10));
}

#[test]
fn invalid_source_never_contributes_a_partial_tree() {
    let result = observe(&[
        ("broken.py", b"import ok\ndef broken(:"),
        ("ok.py", b"import good"),
        ("good.py", b""),
    ]);
    assert_eq!(result["edges"].as_array().unwrap().len(), 1);
    assert_eq!(
        result["diagnostics"][0]["location"],
        json!({"file":"broken.py","pointer":"","line":2,"column":11})
    );
}

#[test]
fn known_encodings_preserve_utf8_byte_columns_and_module_names() {
    for (file, bytes, target) in [
        (
            "main.py",
            &b"\xef\xbb\xbf# coding:utf-8\r\nimport caf\xc3\xa9"[..],
            "café.py",
        ),
        (
            "latin.py",
            &b"# note\r# coding:latin-1\rimport caf\xe9"[..],
            "café.py",
        ),
        (
            "windows.py",
            &b"# coding:cp1251\nimport \xec\xe8\xf0"[..],
            "мир.py",
        ),
        (
            "jp.py",
            &b"# coding: shift_jis\nimport \x97\x5c\x96\xf1"[..],
            "予約.py",
        ),
        (
            "chinese.py",
            &b"# coding: gb18030\nimport \xd4\xa4\xb6\xa9"[..],
            "预订.py",
        ),
    ] {
        let result = observe(&[(file, bytes), (target, b"")]);
        assert_eq!(result["diagnostics"], json!([]), "{file}");
        assert_eq!(result["edges"][0]["to"], target);
    }
    let result = observe(&[("latin.py", b"\n# coding: latin-1\n'\xe6'; import os")]);
    assert_eq!(result["imports"][0]["column"], 6);
}

#[test]
fn unknown_conflicting_and_invalid_cookies_are_diagnostics() {
    for source in [
        &b"#!/bin/python\n  # -*- coding: unknown-codec -*-\nimport os"[..],
        b"\xef\xbb\xbf# coding:latin-1\nimport os",
        b"# coding:ascii\n#\xff",
        b"# coding=cp1252\n#\x81",
        b"# coding: shift_jis\n#\x87\x40",
        b"#\x00",
    ] {
        let result = observe(&[("bad.py", source)]);
        assert_eq!(result["imports"], json!([]));
        assert_eq!(result["diagnostics"][0]["code"], "CODE_SOURCE_UNPARSEABLE");
    }
    let result = observe(&[("ok.py", b"x = 1\n# coding:no-such-codec\nimport os")]);
    assert_eq!(result["diagnostics"], json!([]));
}

#[test]
fn grammar_target_is_independent_of_the_installed_interpreter() {
    let sources = BTreeMap::from([(
        "main.py".into(),
        b"type Items[T] = list[T]\nimport os".to_vec(),
    )]);
    let old = analyze_imports(&sources, &BTreeMap::new(), &BTreeSet::new());
    assert_eq!(old["diagnostics"][0]["code"], "CODE_SOURCE_UNPARSEABLE");
    let new = analyze_imports_for_version(
        &sources,
        &BTreeMap::new(),
        &BTreeSet::new(),
        PythonVersion::PY312,
    );
    assert_eq!(new["diagnostics"], json!([]));
    assert_eq!(new["imports"][0]["module"], "os");
}

#[test]
fn aliases_preserve_python_breadth_first_order_across_elif_and_else() {
    let cases = [
        (
            "if x: pass\nelse: import importlib as a\nif y: import random as a\na()",
            false,
        ),
        (
            "if x: pass\nelif y: pass\nelif z: import importlib as a\nif y:\n if w:\n  import random as a\na()",
            true,
        ),
    ];
    for (source, is_dynamic) in cases {
        let report = observe(&[("app.py", source.as_bytes())]);
        let line = source.lines().count();
        assert_eq!(
            report["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["location"]["line"] == line),
            is_dynamic
        );
    }
}

#[test]
fn bad_utf8_and_too_deep_delimiters_have_source_positions() {
    let result = observe(&[("app.py", b"\xef\xbb\xbf#\r\na = '\xc3\xa6\xff")]);
    let loc = &result["diagnostics"][0]["location"];
    assert_eq!(loc["line"], 2);
    assert_eq!(loc["column"], 7);
    for (depth, accepted) in [(200, true), (201, false)] {
        let source = format!("x={}0{}\nimport os", "(".repeat(depth), ")".repeat(depth));
        let result = observe(&[("app.py", source.as_bytes())]);
        assert_eq!(
            result["imports"].as_array().unwrap().len(),
            usize::from(accepted)
        );
        if !accepted {
            assert_eq!(result["diagnostics"][0]["location"]["column"], 202);
        }
    }
}

#[test]
fn danish_oem_sources_keep_module_identity() {
    for label in ["cp850", "ibm850", "cp865", "865"] {
        let mut bytes = format!("# coding: {label}\nimport f").into_bytes();
        // CP850 and CP865 both map 0x9b to ø and 0x91 to æ.
        bytes.extend_from_slice(b"\x9brstehj\x91lp");
        let result = observe(&[("app.py", &bytes), ("førstehjælp.py", b"")]);
        assert_eq!(result["diagnostics"], json!([]), "{label}");
        assert_eq!(result["edges"][0]["to"], "førstehjælp.py");
    }
    let result = observe(&[("bad.py", b"# coding: ibm720\nimport os")]);
    assert_eq!(result["diagnostics"][0]["code"], "CODE_SOURCE_UNPARSEABLE");
}

#[test]
fn unicode_identifiers_follow_the_selected_cpython_version() {
    use PythonVersion::*;
    for (identifier, first_supported) in [
        ("a\u{11f02}", PY312),
        ("a\u{200d}", PY313),
        ("a\u{2ebf0}", PY313),
        ("a\u{1c89}", PY314),
    ] {
        let sources = BTreeMap::from([
            ("a.py".into(), format!("import {identifier}").into_bytes()),
            (format!("{identifier}.py"), vec![]),
        ]);
        let bindings = sources.keys().map(|k| (k.clone(), k.clone())).collect();
        for version in [PY311, PY312, PY313, PY314] {
            let result =
                analyze_imports_for_version(&sources, &bindings, &BTreeSet::new(), version);
            assert_eq!(
                result["edges"].as_array().unwrap().len(),
                usize::from(version as usize >= first_supported as usize)
            );
        }
    }
    let doubled_bom = observe(&[("bad.py", b"\xef\xbb\xbf\xef\xbb\xbfimport os")]);
    assert_eq!(
        doubled_bom["diagnostics"][0]["code"],
        "CODE_SOURCE_UNPARSEABLE"
    );
}
