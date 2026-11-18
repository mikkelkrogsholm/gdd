use super::{Position, source::SourceError};

// PEP 263 recognizes a cookie on the first line, or after a blank/comment first
// line. Read the declaration from bytes so a non-UTF-8 comment cannot hide it.
fn cookie(mut bytes: &[u8]) -> Option<&str> {
    for _ in 0..2 {
        let end = bytes
            .iter()
            .position(|&c| matches!(c, b'\n' | b'\r'))
            .unwrap_or(bytes.len());
        let (line, tail) = bytes.split_at(end);
        bytes = match tail {
            [b'\r', b'\n', rest @ ..] => rest,
            [_, rest @ ..] => rest,
            [] => &[],
        };
        let begin = line
            .iter()
            .position(|c| !matches!(*c, b' ' | b'\t' | 0x0c))
            .unwrap_or(line.len());
        let line = &line[begin..];
        if line.is_empty() {
            continue;
        }
        if !line.starts_with(b"#") {
            break;
        }
        for (i, text) in line.windows(7).enumerate() {
            if &text[..6] != b"coding" || !matches!(text[6], b':' | b'=') {
                continue;
            }
            let rest = &line[i + 7..];
            let start = rest
                .iter()
                .position(|c| !matches!(*c, b' ' | b'\t'))
                .unwrap_or(rest.len());
            let rest = &rest[start..];
            let end = rest
                .iter()
                .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_'))
                .count();
            if end != 0 {
                return std::str::from_utf8(&rest[..end]).ok();
            }
        }
    }
    None
}

pub(super) fn decode(bytes: &[u8], basename: &str) -> Result<String, SourceError> {
    let codec_error = |message: String| SourceError {
        position: None,
        message: format!("{message} ({basename}, line 0)"),
    };
    let without_bom = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]);
    let bytes = without_bom.unwrap_or(bytes);
    let label = cookie(bytes).unwrap_or("utf-8");
    let normalized_label = label.to_ascii_lowercase().replace('_', "-");
    let label_kind = match normalized_label.as_str() {
        "utf-8" => "utf-8",
        s if s.starts_with("utf-8-") => "utf-8",
        "latin-1" | "iso-8859-1" | "iso-latin-1" => "iso-8859-1",
        s if ["latin-1-", "iso-8859-1-", "iso-latin-1-"]
            .iter()
            .any(|p| s.starts_with(p)) =>
        {
            "iso-8859-1"
        }
        _ => normalized_label.as_str(),
    };
    if without_bom.is_some() && label_kind != "utf-8" {
        return Err(codec_error(format!(
            "encoding problem: {label_kind} with BOM"
        )));
    }
    match label_kind {
        "utf-8" | "utf8" | "u8" | "utf" | "cp65001" | "utf8-ucs2" | "utf8-ucs4" => {
            std::str::from_utf8(bytes)
                .map(str::to_owned)
                .map_err(|err| {
                    let offset = err.valid_up_to();
                    let prefix = std::str::from_utf8(&bytes[..offset]).unwrap();
                    let normalized = prefix.replace("\r\n", "\n").replace('\r', "\n");
                    let line = 1 + normalized.bytes().filter(|&b| b == b'\n').count();
                    let column = normalized.rsplit('\n').next().unwrap().len();
                    SourceError {
                        position: Some(Position { line, column }),
                        message: format!(
                            "Invalid UTF-8 source at byte {offset} ({basename}, line {line})"
                        ),
                    }
                })
        }
        "ascii" | "us-ascii" | "646" | "us" | "ansi-x3.4-1968" | "ansi-x3-4-1968"
        | "ansi-x3.4-1986" | "cp367" | "csascii" | "ibm367" | "iso646-us" | "iso-646.irv-1991"
        | "iso-ir-6" => {
            if let Some(offset) = bytes.iter().position(|b| !b.is_ascii()) {
                Err(codec_error(format!(
                    "'ascii' codec can't decode byte 0x{:02x} in position {offset}: ordinal not in range(128)",
                    bytes[offset]
                )))
            } else {
                Ok(std::str::from_utf8(bytes).unwrap().into())
            }
        }
        "iso-8859-1" | "iso8859-1" | "iso8859" | "latin1" | "latin" | "l1" | "8859" | "cp819"
        | "ibm819" | "csisolatin1" | "iso-8859-1-1987" | "iso-ir-100" => {
            Ok(bytes.iter().map(|b| char::from(*b)).collect())
        }
        _ => legacy_codec(label_kind, label, bytes).map_err(|message| {
            if message.starts_with("unknown encoding:") || message.starts_with("'charmap' codec") {
                codec_error(message)
            } else {
                SourceError {
                    position: None,
                    message: format!("{message} ({basename})"),
                }
            }
        }),
    }
}

// Match Python codec names explicitly. WHATWG label lookup also accepts labels
// such as "ascii" for Windows-1252, which would change valid source text.
fn legacy_codec(name: &str, original: &str, bytes: &[u8]) -> Result<String, String> {
    use encoding_rs as enc;
    // These OEM tables match CPython byte-for-byte, including unassigned bytes.
    if let Some(page) = oem_page(name) {
        return oem_cp::code_table::DECODING_TABLE_CP_MAP[&page]
            .decode_string_checked(bytes)
            .ok_or_else(|| format!("Invalid {original} source"));
    }
    let decoder = match name {
        "cp1251" | "windows-1251" | "1251" => enc::WINDOWS_1251,
        "cp1252" | "windows-1252" | "1252" => enc::WINDOWS_1252,
        "cp1255" | "windows-1255" | "1255" => enc::WINDOWS_1255,
        "koi8-r" | "cskoi8r" => enc::KOI8_R,
        "iso-8859-2" | "iso8859-2" | "latin2" | "l2" | "csisolatin2" | "iso-ir-101"
        | "iso-8859-2-1987" => enc::ISO_8859_2,
        "iso-8859-15" | "iso8859-15" | "latin9" | "l9" => enc::ISO_8859_15,
        "mac-roman" | "macroman" | "macintosh" => enc::MACINTOSH,
        "shift-jis" | "shiftjis" | "sjis" | "s-jis" | "csshiftjis" | "x-mac-japanese" => {
            enc::SHIFT_JIS
        }
        "euc-jp" | "eucjp" | "u-jis" | "ujis" => enc::EUC_JP,
        "big5" | "big5-tw" | "csbig5" | "x-mac-trad-chinese" => enc::BIG5,
        "gb18030" | "gb18030-2000" => enc::GB18030,
        "euc-kr" | "euckr" | "korean" | "ksc5601" | "ks-c-5601-1987" | "ksx1001" | "ks-x-1001"
        | "ks-c-5601" | "x-mac-korean" => enc::EUC_KR,
        _ => return Err(format!("unknown encoding: {original}")),
    };

    if [
        enc::SHIFT_JIS,
        enc::EUC_JP,
        enc::BIG5,
        enc::GB18030,
        enc::EUC_KR,
    ]
    .contains(&decoder)
    {
        check_multibyte(decoder, bytes).map_err(|offset| {
            format!("Unsupported or invalid {original} sequence at byte {offset}")
        })?;
    }
    let decoded = decoder
        .decode_without_bom_handling_and_without_replacement(bytes)
        .ok_or_else(|| format!("Invalid {original} source"))?;

    // Windows decoders can assign C1 controls to otherwise unassigned bytes.
    if [enc::WINDOWS_1251, enc::WINDOWS_1252, enc::WINDOWS_1255].contains(&decoder)
        && let Some((i, _)) = decoded
            .chars()
            .enumerate()
            .find(|(_, c)| matches!(c, '\u{80}'..='\u{9f}'))
    {
        return Err(format!(
            "'charmap' codec can't decode byte 0x{:02x} in position {i}: character maps to <undefined>",
            bytes[i]
        ));
    }
    // Some WHATWG tables assign a newer character to a CPython undefined byte.
    if decoder == enc::WINDOWS_1255 && bytes.contains(&0xca) {
        return Err("Unassigned Windows-1255 source byte 0xca".into());
    }
    Ok(decoded.into_owned())
}

// Validate the shared Python/WHATWG subset before the strict Rust decoder.
// The Web encodings admit vendor extensions or assign different characters to
// certain sequences. Reporting uncertainty there avoids inventing module names.
fn check_multibyte(codec: &'static encoding_rs::Encoding, bytes: &[u8]) -> Result<(), usize> {
    use encoding_rs as enc;
    let mut i = 0;
    while i < bytes.len() {
        let lead = bytes[i];
        if lead.is_ascii() || (codec == enc::SHIFT_JIS && (0xa1..=0xdf).contains(&lead)) {
            i += 1;
            continue;
        }
        let trail = *bytes.get(i + 1).ok_or(i)?;
        let pair = u16::from_be_bytes([lead, trail]);
        let accepted = if codec == enc::SHIFT_JIS {
            lead != 0x80
                && lead != 0x87
                && lead < 0xed
                && !matches!(pair, 0x8160 | 0x8161 | 0x817c | 0x8191 | 0x8192 | 0x81ca)
        } else if codec == enc::EUC_JP {
            !matches!(lead, 0x8f | 0xad | 0xf9..=0xfc)
                && !matches!(pair, 0xa1c1 | 0xa1c2 | 0xa1dd | 0xa1f1 | 0xa1f2 | 0xa2cc)
        } else if codec == enc::EUC_KR {
            (0xa1..=0xfe).contains(&lead) && (0xa1..=0xfe).contains(&trail) && pair != 0xa4d4
        } else if codec == enc::BIG5 {
            (0xa140..=0xf9d5).contains(&pair)
                && !matches!(pair, 0xa145 | 0xa14e | 0xa1c2 | 0xa1e3 | 0xa1f2 | 0xa1f3 | 0xa241 | 0xa242
                | 0xa244 | 0xa246 | 0xa247 | 0xa3c0..=0xa3ff | 0xc6a1..=0xc8ff)
        } else if trail.is_ascii_digit() {
            // The supplementary-plane four-byte range is shared by the 2000,
            // 2005 and 2022 revisions of GB18030.
            if lead < 0x90 || bytes.len() - i < 4 {
                return Err(i);
            }
            i += 4;
            continue;
        } else {
            lead != 0x80
                && !matches!(pair, 0xa3a0 | 0xa6d9..=0xa6df | 0xa6ec..=0xa6ed | 0xa6f3
                | 0xa8bc | 0xfe59 | 0xfe61 | 0xfe66 | 0xfe67 | 0xfe6d | 0xfe7e | 0xfe90 | 0xfea0)
        };
        if !accepted {
            return Err(i);
        }
        i += 2;
    }
    Ok(())
}

fn oem_page(name: &str) -> Option<u16> {
    Some(match name {
        "cp437" | "437" | "ibm437" | "cspc8codepage437" => 437,
        "cp720" => 720,
        "cp737" => 737,
        "cp775" | "775" | "ibm775" | "cspc775baltic" => 775,
        "cp850" | "850" | "ibm850" | "cspc850multilingual" => 850,
        "cp852" | "852" | "ibm852" | "cspcp852" => 852,
        "cp855" | "855" | "ibm855" | "csibm855" => 855,
        "cp857" | "857" | "ibm857" | "csibm857" => 857,
        "cp858" | "858" | "ibm858" | "csibm858" => 858,
        "cp860" | "860" | "ibm860" | "csibm860" => 860,
        "cp861" | "861" | "ibm861" | "csibm861" | "cp-is" => 861,
        "cp862" | "862" | "ibm862" | "cspc862latinhebrew" => 862,
        "cp863" | "863" | "ibm863" | "csibm863" => 863,
        "cp865" | "865" | "ibm865" | "csibm865" => 865,
        "cp866" | "866" | "ibm866" | "csibm866" => 866,
        _ => return None,
    })
}
