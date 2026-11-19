use std::process::ExitCode;

use gdd_tutorial_add::add;
use num_bigint::BigInt;

fn check_examples() -> Result<(), String> {
    let large = BigInt::from(10).pow(100);
    let large_expected = format!("1{}1", "0".repeat(99)).parse::<BigInt>().unwrap();
    let examples = [
        ((-7).into(), 7.into(), 0.into()),
        (2.into(), 3.into(), 5.into()),
        (0.into(), 0.into(), 0.into()),
        ((-9).into(), (-4).into(), (-13).into()),
        (large, 1.into(), large_expected),
    ];
    for (left, right, expected) in examples {
        let actual = add(&left, &right);
        if actual != expected {
            return Err(format!(
                "{left} + {right}: expected {expected}, got {actual}"
            ));
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    match check_examples() {
        Ok(()) => {
            println!("PASS: all five integer sums matched");
            ExitCode::SUCCESS
        }
        Err(mismatch) => {
            eprintln!("FAIL: {mismatch}");
            ExitCode::FAILURE
        }
    }
}

#[test]
fn integer_addition_examples() {
    check_examples().unwrap();
}
