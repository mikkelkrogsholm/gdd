use crate::{InvalidInput, discount, tax, validate};
use num_bigint::BigInt;

#[derive(Debug, Eq, PartialEq)]
pub struct Quote {
    pub subtotal: BigInt,
    pub discount: BigInt,
    pub net: BigInt,
    pub tax: BigInt,
    pub total: BigInt,
}

pub fn quote(
    subtotal: &BigInt,
    discount_bps: &BigInt,
    tax_bps: &BigInt,
) -> Result<Quote, InvalidInput> {
    validate(subtotal, &[discount_bps, tax_bps])?;
    let discount = discount(subtotal, discount_bps)?;
    let net = subtotal - &discount;
    let tax = tax(&net, tax_bps)?;
    let total = &net + &tax;
    Ok(Quote {
        subtotal: subtotal.clone(),
        discount,
        net,
        tax,
        total,
    })
}
