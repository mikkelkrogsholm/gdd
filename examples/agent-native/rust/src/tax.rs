use crate::{InvalidInput, validate};
use num_bigint::BigInt;

pub fn tax(amount: &BigInt, rate_bps: &BigInt) -> Result<BigInt, InvalidInput> {
    validate(amount, &[rate_bps])?;
    Ok(amount * rate_bps / 10_000)
}
