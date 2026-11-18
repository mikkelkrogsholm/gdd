use std::collections::BTreeMap;

use gdd::input::{InputError, MAX_CONTAINER_DEPTH, Value, parse};

fn string(value: &str) -> Value {
    Value::String(value.into())
}

#[test]
fn json_strings_take_precedence_over_yaml_folding() {
    let separators = "\u{2028}\u{2029}";
    let source = format!(r#"{{"text":"\/\uD83D\uDE00{separators}"}}"#);
    assert_eq!(
        parse(source.as_bytes()),
        Ok(Value::Object(BTreeMap::from([(
            "text".into(),
            string(&format!("/😀{separators}"))
        )])))
    );

    assert_eq!(
        parse(b"{text: yes, other: 'value',}"),
        Ok(Value::Object(BTreeMap::from([
            ("other".into(), string("value")),
            ("text".into(), string("yes"))
        ])))
    );
}

#[test]
fn yaml_resolves_only_json_scalars() {
    let source = br#"
yes: yes
no: 0x30
date: 2026-11-18
legacy: 012
signed: +1
tilde: ~
quoted: "true"
bool: true
null_value: null
missing:
number: -12.345e-001
tagged: !!int "7"
forced_string: !!str null
non_specific: ! true
verbatim: !<tag:yaml.org,2002:bool> "false"
array: !!seq [1, 2]
object: !!map {"true": ok}
"#;
    assert_eq!(
        parse(source),
        Ok(Value::Object(BTreeMap::from([
            ("yes".into(), string("yes")),
            ("no".into(), string("0x30")),
            ("date".into(), string("2026-11-18")),
            ("legacy".into(), string("012")),
            ("signed".into(), string("+1")),
            ("tilde".into(), string("~")),
            ("quoted".into(), string("true")),
            ("bool".into(), Value::Boolean(true)),
            ("null_value".into(), Value::Null),
            ("missing".into(), Value::Null),
            ("number".into(), Value::Number("-12.345e-001".into())),
            ("tagged".into(), Value::Number("7".into())),
            ("forced_string".into(), string("null")),
            ("non_specific".into(), string("true")),
            ("verbatim".into(), Value::Boolean(false)),
            (
                "array".into(),
                Value::Array(vec![Value::Number("1".into()), Value::Number("2".into())])
            ),
            (
                "object".into(),
                Value::Object(BTreeMap::from([("true".into(), string("ok"))]))
            ),
        ])))
    );
    assert_eq!(
        parse(b"%YAML 1.2\n%TAG !num! tag:yaml.org,2002:\n---\n!num!float 1.00E+09"),
        Ok(Value::Number("1.00E+09".into()))
    );
}

#[test]
fn keeps_exact_number_lexemes_across_both_formats() {
    let long_integer = "9".repeat(5000);
    for number in [
        "-0",
        "9007199254740993",
        "1.000000000000000000000000001",
        "9.99E+99999999999999999999",
        "2e-000000003",
        &long_integer,
    ] {
        let expected = Value::Object(BTreeMap::from([(
            "value".into(),
            Value::Number(number.into()),
        )]));
        assert_eq!(
            parse(format!(r#"{{"value":{number}}}"#).as_bytes()),
            Ok(expected)
        );
        let expected = Value::Object(BTreeMap::from([(
            "value".into(),
            Value::Number(number.into()),
        )]));
        assert_eq!(parse(format!("value: {number}").as_bytes()), Ok(expected));
    }
}

#[test]
fn rejects_duplicate_keys_at_any_depth() {
    for source in [
        r#"{"value": {"key": 1, "\u006bey": 2}}"#,
        r#"{value: {key: 1, "key": 2}}"#,
        "value:\n  !!str key: 1\n  'key': 2",
    ] {
        assert_eq!(
            parse(source.as_bytes()),
            Err(InputError::DuplicateKey("key".into())),
            "{source}"
        );
    }
}

#[test]
fn rejects_forbidden_document_features() {
    for source in [
        r#"{"unknown": {"\u003c\u003c": []}}"#,
        "value: { '<<': {} }",
        "value: &unused 1",
        "value: &unused {}",
        "value: &unused []",
        "value: *alias",
        "value: !custom 1",
        "value: !!timestamp 2026-11-18",
        "%TAG !! tag:example.test,2026:\n---\nvalue: !!str 1",
        "value: !!map []",
        "value: !!seq {}",
        "value: !!str []",
        "value: !!map 1",
        "value: !!int 0xff",
        "value: !!int 1e2",
        "value: !!float .inf",
        "value: !!bool yes",
        "value: !!null NULL",
        "true: value",
        "3: value",
        "null: value",
        "? [x, y]\n: value",
        "value: .NaN",
        r#"{"value": NaN}"#,
        r#"{"value": Infinity}"#,
        r#"{"value": -Infinity}"#,
        r#"{"value": "\uD800"}"#,
        r#"{"\uDFFF": "value"}"#,
        r#"value: "\U00110000""#,
        "a\n---\nb",
        r#"{"value":[}"#,
    ] {
        assert!(
            matches!(parse(source.as_bytes()), Err(InputError::Invalid(_))),
            "{source}"
        );
    }
    for spelling in ["NaN", "Infinity", "-Infinity"] {
        assert!(matches!(
            parse(format!("value: {spelling}").as_bytes()),
            Err(InputError::Invalid(_))
        ));
        assert_eq!(
            parse(format!("!!str {spelling}").as_bytes()),
            Ok(string(spelling))
        );
    }
    for invalid_utf8 in [&b"\xff"[..], &b"{\"x\":\"\xed\xa0\x80\"}"[..]] {
        assert!(matches!(parse(invalid_utf8), Err(InputError::Invalid(_))));
    }
}

#[test]
fn limits_container_depth_in_json_and_yaml() {
    for depth in [MAX_CONTAINER_DEPTH, MAX_CONTAINER_DEPTH + 1, 2_000] {
        let json_array = format!("{}0{}", "[".repeat(depth), "]".repeat(depth));
        let json_object = format!(r#"{}0{}"#, r#"{"value":"#.repeat(depth), "}".repeat(depth));
        let yaml_flow = format!("---\n{}yes{}", "[".repeat(depth), "]".repeat(depth));
        let yaml_array = (0..depth)
            .map(|level| format!("{}-\n", "  ".repeat(level)))
            .collect::<String>();
        let yaml_object = (0..depth)
            .map(|level| format!("{}value:\n", "  ".repeat(level)))
            .collect::<String>();
        let yaml_indentless = format!(
            "values:\n{}",
            (0..depth - 1)
                .map(|level| format!("{}-\n", "  ".repeat(level)))
                .collect::<String>()
        );
        for source in [
            json_array,
            json_object,
            yaml_flow,
            yaml_array,
            yaml_object,
            yaml_indentless,
        ] {
            let parsed = parse(source.as_bytes());
            if depth == MAX_CONTAINER_DEPTH {
                assert!(parsed.is_ok(), "{}", parsed.unwrap_err());
            } else {
                assert_eq!(parsed, Err(InputError::UnsupportedDepth));
            }
        }
    }
}

#[test]
fn containers_inside_strings_do_not_count_toward_depth() {
    let quoted = format!("{}\\\"{}", "[{".repeat(1_000), "]}".repeat(1_000));
    let json = serde_json::to_vec(&quoted).unwrap();
    assert_eq!(parse(&json), Ok(Value::String(quoted)));

    let brackets = "[".repeat(1_000);
    let yaml = format!("|\n  {brackets}\n");
    assert_eq!(parse(yaml.as_bytes()), Ok(string(&format!("{brackets}\n"))));
}
