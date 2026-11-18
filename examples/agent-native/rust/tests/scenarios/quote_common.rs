use quote_demo::{InvalidInput, Quote, quote};
use serde_json::{Value, json};
use std::process::{Command, Output};

fn check_quote(inputs: [i32; 3], expected: [i32; 5]) {
    let [subtotal, discount_bps, tax_bps] = inputs.map(Into::into);
    let [expected_subtotal, discount, net, tax, total] = expected.map(Into::into);
    assert_eq!(
        quote(&subtotal, &discount_bps, &tax_bps),
        Ok(Quote {
            subtotal: expected_subtotal,
            discount,
            net,
            tax,
            total
        }),
    );
}

#[test]
fn test_normal() {
    check_quote([10_000, 1000, 2500], [10_000, 1000, 9000, 2250, 11_250]);
}

#[test]
fn test_floor_after_discount() {
    check_quote([101, 5000, 5000], [101, 50, 51, 25, 76]);
}

#[test]
fn test_tax_floor_without_discount() {
    check_quote([101, 0, 5000], [101, 0, 101, 50, 151]);
}

#[test]
fn test_zero_and_full_discount() {
    check_quote([0, 0, 10_000], [0; 5]);
    check_quote([100, 10_000, 10_000], [100, 100, 0, 0, 0]);
}

#[test]
fn test_invalid() {
    for (subtotal, discount, tax) in [
        (-1, 0, 0),
        (1, -1, 0),
        (1, 10_001, 0),
        (1, 0, -1),
        (1, 0, 10_001),
    ] {
        assert_eq!(
            quote(&subtotal.into(), &discount.into(), &tax.into()),
            Err(InvalidInput)
        );
    }
}

fn cli(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_quote_demo"))
        .args(args)
        .output()
        .expect("start the quote_demo binary")
}

fn cli_quote(args: &[&str]) -> Value {
    let result = cli(args);
    assert_eq!(result.status.code(), Some(0), "{result:?}");
    assert!(result.stderr.is_empty(), "{result:?}");
    serde_json::from_slice(&result.stdout).expect("one JSON object")
}

#[test]
fn test_cli_normal() {
    for args in [
        ["10000", "1000", "2500"],
        ["  +1_0０0٠\t", "٠١_٠٠٠", "𝟮𝟱𝟬𝟬"],
    ] {
        assert_eq!(
            cli_quote(&args),
            json!({
                "subtotal": 10000, "discount": 1000, "net": 9000, "tax": 2250, "total": 11250
            })
        );
    }
    assert_eq!(
        cli_quote(&["-0", "+0", "0"]),
        json!({
            "subtotal": 0, "discount": 0, "net": 0, "tax": 0, "total": 0
        })
    );
}

#[test]
fn test_cli_floor() {
    assert_eq!(
        cli_quote(&["101", "5000", "5000"]),
        json!({
            "subtotal": 101, "discount": 50, "net": 51, "tax": 25, "total": 76
        })
    );
}

#[test]
fn test_cli_invalid() {
    for args in [
        &["-1", "0", "0"][..],
        &["1", "10001", "0"],
        &["1", "0", "-1"],
        &["1.5", "0", "0"],
        &["bad", "0", "0"],
        &["1__0", "0", "0"],
        &["+_1", "0", "0"],
        &["1_", "0", "0"],
        &["²", "0", "0"],
        &["+", "0", "0"],
        &["1", "0"],
        &["1", "0", "0", "2"],
    ] {
        let result = cli(args);
        assert_eq!(result.status.code(), Some(2), "{args:?}: {result:?}");
        assert!(result.stdout.is_empty(), "{args:?}: {result:?}");
        assert_eq!(result.stderr, b"InvalidInput\n", "{args:?}");
    }
}
