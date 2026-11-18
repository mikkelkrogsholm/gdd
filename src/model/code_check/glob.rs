//! Relative POSIX path patterns. Each segment follows case-sensitive fnmatch.
use std::io;

pub(super) fn validate(pattern: &str) -> io::Result<()> {
    let fail = |msg| io::Error::new(io::ErrorKind::InvalidInput, msg);
    if pattern.is_empty() || pattern.starts_with('/') || pattern.contains(['\\', ':', '\0']) {
        return Err(fail("Use a normalized relative POSIX resource or glob."));
    }
    if pattern
        .trim_end_matches('/')
        .split('/')
        .any(|p| matches!(p, "" | "." | "..") || p.contains("**") && p != "**")
    {
        return Err(fail(
            "Resources cannot contain empty, dot or parent segments; ** must be a whole segment.",
        ));
    }
    Ok(())
}

pub(super) fn matches(name: &str, pattern: &str) -> bool {
    if pattern.ends_with('/') {
        return name.starts_with(pattern);
    }
    let parts: Vec<_> = name.split('/').collect();
    let mut row = vec![false; parts.len() + 1];
    row[0] = true;
    for segment in pattern.split('/') {
        if segment == "**" {
            for i in 1..row.len() {
                row[i] |= row[i - 1];
            }
        } else {
            for i in (1..row.len()).rev() {
                row[i] = row[i - 1] && component(parts[i - 1], segment);
            }
            row[0] = false;
        }
    }
    row[parts.len()]
}

fn component(text: &str, pat: &str) -> bool {
    if !pat.contains(['*', '?', '[']) {
        return text == pat;
    }
    let chars: Vec<_> = text.chars().collect();
    let pat: Vec<_> = pat.chars().collect();
    let mut row = vec![false; chars.len() + 1];
    row[0] = true;
    let mut i = 0;
    while i < pat.len() {
        let token = pat[i];
        i += 1;
        if token == '*' {
            for j in 1..row.len() {
                row[j] |= row[j - 1];
            }
            continue;
        }
        let mut class = None;
        if token == '[' {
            let negated = pat.get(i) == Some(&'!');
            let start = i + usize::from(negated);
            let from = start + usize::from(pat.get(start) == Some(&']'));
            if let Some(end) = pat[from..].iter().position(|&c| c == ']') {
                class = Some((negated, &pat[start..from + end]));
                i = from + end + 1;
            }
        }
        for j in (1..row.len()).rev() {
            let c = chars[j - 1];
            row[j] = row[j - 1]
                && if let Some((neg, mut members)) = class {
                    let mut member = false;
                    while let Some((&first, rest)) = members.split_first() {
                        if let ['-', last, remaining @ ..] = rest {
                            member |= (first..=*last).contains(&c);
                            members = remaining;
                        } else {
                            member |= c == first;
                            members = rest;
                        }
                    }
                    member != neg
                } else {
                    token == '?' || c == token
                };
        }
        row[0] = false;
    }
    row[chars.len()]
}

pub(super) fn overlaps(pattern: &str, omitted: &[String]) -> bool {
    let prefix = pattern
        .trim_end_matches('/')
        .split('/')
        .take_while(|s| !s.contains(['*', '?', '[']))
        .collect::<Vec<_>>()
        .join("/");
    omitted.iter().any(|name| {
        matches(name.trim_end_matches('/'), pattern)
            || matches(&format!("{name}placeholder.py"), pattern)
            || name.ends_with('/')
                && (prefix.is_empty()
                    || prefix.starts_with(name)
                    || name.starts_with(&format!("{prefix}/")))
    })
}
