use num_bigint::BigInt;
use quote_demo::{InvalidInput, Quote, quote};
use std::{env, ffi::OsString, process::ExitCode};
use unicode_general_category::{GeneralCategory::DecimalNumber, get_general_category};

// Unicode decimal digits occur in ordered groups of ten. Several groups can
// touch (for example, the mathematical digits), so take the offset modulo ten.
fn ascii_digit(ch: char) -> Option<char> {
    if ch.is_ascii_digit() {
        return Some(ch);
    }
    if get_general_category(ch) != DecimalNumber {
        return None;
    }
    let code = u32::from(ch);
    let mut start = code;
    while char::from_u32(start - 1).is_some_and(|prev| get_general_category(prev) == DecimalNumber)
    {
        start -= 1;
    }
    char::from_digit((code - start) % 10, 10)
}

fn integer(arg: Option<OsString>) -> Result<BigInt, InvalidInput> {
    let arg = arg.ok_or(InvalidInput)?;
    let token = arg.to_str().ok_or(InvalidInput)?.trim();
    let mut digits = String::with_capacity(token.len());
    let mut chars = token.chars().peekable();
    if let Some(sign) = chars.next_if(|ch| matches!(ch, '+' | '-')) {
        digits.push(sign);
    }
    let mut need_digit = true;
    for ch in chars {
        if ch == '_' && !need_digit {
            need_digit = true;
        } else {
            digits.push(ascii_digit(ch).ok_or(InvalidInput)?);
            need_digit = false;
        }
    }
    if need_digit {
        return Err(InvalidInput);
    }
    digits.parse().map_err(|_| InvalidInput)
}

fn run() -> Result<Quote, InvalidInput> {
    let mut args = env::args_os().skip(1);
    let subtotal = integer(args.next())?;
    let discount_bps = integer(args.next())?;
    let tax_bps = integer(args.next())?;
    if args.next().is_some() {
        return Err(InvalidInput);
    }
    quote(&subtotal, &discount_bps, &tax_bps)
}

fn main() -> ExitCode {
    match run() {
        Ok(Quote {
            subtotal,
            discount,
            net,
            tax,
            total,
        }) => {
            // BigInt's decimal Display writes exact JSON integer tokens.
            println!(
                r#"{{"discount": {discount}, "net": {net}, "subtotal": {subtotal}, "tax": {tax}, "total": {total}}}"#
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}
