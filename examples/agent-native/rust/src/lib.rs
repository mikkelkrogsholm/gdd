//! Integer arithmetic for the agent-native quote example.

mod discount;
mod quote;
mod tax;

use num_bigint::{BigInt, Sign};
use std::fmt;

pub use discount::discount;
pub use quote::{Quote, quote};
pub use tax::tax;

#[derive(Debug, Eq, PartialEq)]
pub struct InvalidInput;

impl fmt::Display for InvalidInput {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("InvalidInput")
    }
}

impl std::error::Error for InvalidInput {}

fn validate(amount: &BigInt, rates: &[&BigInt]) -> Result<(), InvalidInput> {
    let max_rate = BigInt::from(10_000);
    if amount.sign() == Sign::Minus
        || rates
            .iter()
            .any(|rate| rate.sign() == Sign::Minus || *rate > &max_rate)
    {
        return Err(InvalidInput);
    }
    Ok(())
}
