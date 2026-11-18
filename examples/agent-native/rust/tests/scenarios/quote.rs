include!("quote_common.rs");

#[test]
fn test_cli_large_integer_floor_composition() {
    let actual = cli_quote(&["1000000000000000000000000000001", "5000", "5000"]);
    let expected: Value = serde_json::from_str(
        r#"{
        "subtotal": 1000000000000000000000000000001,
        "discount": 500000000000000000000000000000,
        "net": 500000000000000000000000000001,
        "tax": 250000000000000000000000000000,
        "total": 750000000000000000000000000001
    }"#,
    )
    .unwrap();
    assert_eq!(actual, expected);

    // Go beyond u128 as well. At 100% tax the total has twice the subtotal.
    let zeros = "0".repeat(80);
    let subtotal = format!("1{zeros}");
    let total = format!("2{zeros}");
    let actual = cli_quote(&[&subtotal, "0", "10000"]);
    let subtotal: Value = serde_json::from_str(&subtotal).unwrap();
    let total: Value = serde_json::from_str(&total).unwrap();
    assert_eq!(
        actual,
        json!({
            "subtotal": subtotal, "discount": 0, "net": subtotal, "tax": subtotal, "total": total
        })
    );
}
