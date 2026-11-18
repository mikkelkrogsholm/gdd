//! Portable sorted UTF-8 JSON for model comparisons and revision identities.

use serde_json::Value as Json;

// Align integer and decimal spelling with the portable Python consumer for its
// supported values. Work directly on the coefficient so finer finite decimals
// accepted by the native parser retain every digit.
pub(crate) fn comparison_number(raw: &str) -> String {
    number(raw, true)
}

fn number(raw: &str, compare_wide: bool) -> String {
    if !raw.contains(['.', 'e', 'E']) {
        return if raw == "-0" { "0" } else { raw }.into();
    }
    let (sign, mag) = raw.strip_prefix('-').map_or(("", raw), |s| ("-", s));
    let (mantissa, exp) = mag.split_once(['e', 'E']).unwrap_or((mag, "0"));
    let (integer, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let digits = format!("{integer}{fraction}");
    let leading = digits.len() - digits.trim_start_matches('0').len();
    let digits = digits.trim_matches('0');
    if digits.is_empty() {
        return format!("{sign}0.0");
    }
    let shift = integer.len() as i64 - leading as i64 - 1;
    let Some(adjusted) = exp.parse::<i64>().ok().and_then(|n| n.checked_add(shift)) else {
        if !compare_wide {
            // Existing workplan and journal identities preserve this spelling.
            return raw.into();
        }
        // Keep very large exponent spellings comparable without expanding the
        // represented number. Only the exponent itself needs big integers.
        let adjusted = exp.parse::<num_bigint::BigInt>().expect("JSON exponent") + shift;
        return scientific(sign, digits, &format!("{adjusted:+}"));
    };
    if !(-4..16).contains(&adjusted) {
        return scientific(sign, digits, &format!("{adjusted:+03}"));
    }
    let point = adjusted + 1;
    if point <= 0 {
        format!("{sign}0.{}{digits}", "0".repeat((-point) as usize))
    } else if point as usize >= digits.len() {
        format!(
            "{sign}{digits}{}.0",
            "0".repeat(point as usize - digits.len())
        )
    } else {
        let (integer, fraction) = digits.split_at(point as usize);
        format!("{sign}{integer}.{fraction}")
    }
}

fn scientific(sign: &str, digits: &str, exponent: &str) -> String {
    let (first, rest) = digits.split_at(1);
    let separator = if rest.is_empty() { "" } else { "." };
    format!("{sign}{first}{separator}{rest}e{exponent}")
}

pub(super) fn bytes(value: &Json) -> Vec<u8> {
    fn append(value: &Json, bytes: &mut Vec<u8>) {
        match value {
            Json::Array(items) => {
                bytes.push(b'[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        bytes.push(b',');
                    }
                    append(item, bytes);
                }
                bytes.push(b']');
            }
            Json::Object(fields) => {
                bytes.push(b'{');
                let mut sorted: Vec<_> = fields.iter().collect();
                sorted.sort_unstable_by_key(|(key, _)| *key);
                for (index, (key, item)) in sorted.into_iter().enumerate() {
                    if index > 0 {
                        bytes.push(b',');
                    }
                    serde_json::to_writer(&mut *bytes, key).expect("string serializes");
                    bytes.push(b':');
                    append(item, bytes);
                }
                bytes.push(b'}');
            }
            Json::Number(n) => bytes.extend(number(n.as_str(), false).bytes()),
            _ => serde_json::to_writer(bytes, value).expect("scalar serializes"),
        }
    }
    let mut bytes = Vec::new();
    append(value, &mut bytes);
    bytes
}

/// Revision of a value encoded as canonical sorted UTF-8 JSON.
pub fn digest(value: &Json) -> String {
    format!("sha256:{}", crate::sha256(&bytes(value)))
}
