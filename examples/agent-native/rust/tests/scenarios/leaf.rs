use num_bigint::BigInt;
use quote_demo::InvalidInput;

fn check(amount: u128, rate: u16, expected: u128) {
    assert_eq!(
        calculate(&BigInt::from(amount), &BigInt::from(rate)),
        Ok(expected.into()),
    );
}

#[test]
fn test_normal() {
    check(10_000, 1000, 1000);
}

#[test]
fn test_floor_half() {
    check(101, 5000, 50);
}

#[test]
fn test_floor_fraction() {
    check(103, 2500, 25);
}

#[test]
fn test_boundaries() {
    check(0, 10_000, 0);
    check(101, 0, 0);
    check(101, 10_000, 101);
    check(10u128.pow(30), 5000, 5 * 10u128.pow(29));
}

#[test]
fn test_invalid() {
    for (amount, rate) in [(-1, 0), (1, -1), (1, 10_001)] {
        assert_eq!(calculate(&amount.into(), &rate.into()), Err(InvalidInput));
    }
}
