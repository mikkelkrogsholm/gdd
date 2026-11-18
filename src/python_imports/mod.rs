//! In-memory observation of static Python imports in a single regular-package root.
//! Callers supply the complete source inventory, bindings, and runtime module names.

//!
//! `analyze_imports` uses Python 3.11 grammar. `analyze_imports_for_version`
//! also accepts 3.12, 3.13, and 3.14. The caller supplies normalized relative
//! paths ending in `.py`. Both builtin/frozen full names and their roots take
//! precedence for absolute imports. Relative imports use the local package.
//!
//! Sources default to UTF-8. A UTF-8 BOM and first- or second-line encoding
//! declarations are supported. Single-byte decoders cover ASCII, Latin-1,
//! ISO-8859-2/15, Windows-1251/1252/1255, KOI8-R, Mac Roman, and OEM code pages
//! 437, 720, 737, 775, 850, 852, 855, 857, 858, 860, 861, 862, 863, 865, and 866.
//! Shift_JIS, EUC-JP, EUC-KR, Big5 and GB18030 support their common ranges.
//! Vendor-specific Japanese/Korean extensions, Big5 variants, JIS X 0212,
//! GB18030 four-byte BMP mappings, and other ambiguous byte assignments produce
//! `CODE_SOURCE_UNPARSEABLE` rather than observations from uncertain text.
//! Files with unsupported encodings or syntax also receive this diagnostic.
//!
//! The returned JSON contains `imports`, `edges`, `diagnostics` and `inert_files`.
//! The module performs the observation stage of architecture checking. Filesystem
//! inventory, binding validation, and architecture verdicts belong to the caller.

mod encoding;
mod source;
mod syntax;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write;
use std::sync::LazyLock;

use regex::Regex;

use ruff_python_ast::{Expr, Stmt};
use serde_json::{Value, json};

/// Python grammar used by the in-memory source observer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PythonVersion {
    PY311,
    PY312,
    PY313,
    PY314,
}

impl PythonVersion {
    // CPython 3.11 through 3.14 ship UCD versions 14.0, 15.0, 15.1,
    // and 16.0. Unicode 15.1 also added four existing characters to XID_Continue.
    fn invalid_identifier_offset(self, spelling: &str) -> Option<usize> {
        if spelling.is_ascii() {
            return None;
        }
        static FILTERS: LazyLock<[Regex; 4]> = LazyLock::new(|| {
            [
                r"[\P{Age=14.0}\u{200c}\u{200d}\u{30fb}\u{ff65}]",
                r"[\P{Age=15.0}\u{200c}\u{200d}\u{30fb}\u{ff65}]",
                r"\P{Age=15.1}",
                r"\P{Age=16.0}",
            ]
            .map(|pattern| Regex::new(pattern).expect("supported Unicode age property"))
        });
        FILTERS[self as usize]
            .find(spelling)
            .map(|found| found.start())
    }

    fn grammar(self) -> ruff_python_ast::PythonVersion {
        use ruff_python_ast::PythonVersion as Ruff;
        match self {
            Self::PY311 => Ruff::PY311,
            Self::PY312 => Ruff::PY312,
            Self::PY313 => Ruff::PY313,
            Self::PY314 => Ruff::PY314,
        }
    }
}

/// Observe imports against Python 3.11 grammar and the caller's builtin/frozen names.
pub fn analyze_imports(
    sources: &BTreeMap<String, Vec<u8>>,
    components: &BTreeMap<String, String>,
    runtime_modules: &BTreeSet<String>,
) -> Value {
    analyze_imports_for_version(sources, components, runtime_modules, PythonVersion::PY311)
}

/// Observe all AST branches using the specified target grammar (Python 3.11–3.14).
/// Source positions use one-based lines and zero-based UTF-8 byte columns.
/// Statements use normalized import syntax. Sources are never executed.
pub fn analyze_imports_for_version(
    sources: &BTreeMap<String, Vec<u8>>,
    components: &BTreeMap<String, String>,
    runtime_modules: &BTreeSet<String>,
    version: PythonVersion,
) -> Value {
    let mut report = Report::default();
    let mut modules = Modules::new(sources, runtime_modules, version);
    let mut trees = Vec::new();
    for (file, bytes) in sources {
        match source::parse(file, bytes, version) {
            Ok(source) => {
                if file.rsplit('/').next() == Some("__init__.py")
                    && source.body.iter().enumerate().all(|(i, stmt)| {
                        matches!(stmt, Stmt::Pass(_))
                            || i == 0
                                && matches!(stmt,
                            Stmt::Expr(s) if matches!(s.value.as_ref(), Expr::StringLiteral(_)))
                    })
                {
                    modules.inert.insert(file);
                }
                trees.push((file, source));
            }
            Err(err) => report.diagnostic(
                file,
                components,
                err.position,
                "CODE_SOURCE_UNPARSEABLE",
                &err.message,
            ),
        }
    }
    for (file, source) in trees {
        syntax::observe(
            &source,
            &mut Observer {
                file,
                components,
                modules: &modules,
                report: &mut report,
            },
        );
    }
    json!({
        "imports": sorted(report.imports),
        "edges": sorted(report.edges),
        "diagnostics": sorted(report.diagnostics),
        "inert_files": modules.inert,
    })
}

#[derive(Clone, Copy)]
struct Position {
    line: usize,
    column: usize,
}

struct Modules<'a> {
    roots: BTreeSet<&'a str>,
    candidates: BTreeMap<String, Vec<&'a str>>,
    names: BTreeMap<&'a str, String>,
    inert: BTreeSet<&'a str>,
    runtime: &'a BTreeSet<String>,
}

impl<'a> Modules<'a> {
    fn new(
        sources: &'a BTreeMap<String, Vec<u8>>,
        runtime: &'a BTreeSet<String>,
        version: PythonVersion,
    ) -> Self {
        let mut index = Self {
            roots: Default::default(),
            candidates: Default::default(),
            names: Default::default(),
            inert: Default::default(),
            runtime,
        };
        for file in sources.keys() {
            let mut parts: Vec<_> = file.split('/').collect();
            let basename = parts.pop().unwrap();
            let stem = basename.strip_suffix(".py").unwrap_or("");
            index.roots.insert(parts.first().copied().unwrap_or(stem));
            let mut package = String::new();
            if !parts.iter().all(|part| {
                package.push_str(part);
                package.push('/');
                sources.contains_key(&format!("{package}__init__.py"))
            }) {
                continue;
            }
            if basename != "__init__.py" {
                parts.push(stem);
            }
            if parts.is_empty()
                || !parts.iter().all(|part| {
                    let mut chars = part.chars();
                    chars
                        .next()
                        .is_some_and(|c| c == '_' || unicode_ident::is_xid_start(c))
                        && chars.all(unicode_ident::is_xid_continue)
                        && version.invalid_identifier_offset(part).is_none()
                })
            {
                continue;
            }
            let name = parts.join(".");
            index.candidates.entry(name.clone()).or_default().push(file);
            index.names.insert(file, name);
        }
        index
    }

    fn only_candidate(&self, name: &str) -> Option<&'a str> {
        match self.candidates.get(name)?.as_slice() {
            [file] => Some(*file),
            _ => None,
        }
    }

    fn relative_name(&self, file: &str, level: u32, name: &str) -> String {
        let own = self.names.get(file).map(String::as_str).unwrap_or("");
        let mut package = if file.ends_with("/__init__.py") {
            own
        } else {
            own.rsplit_once('.').map_or("", |(parent, _)| parent)
        };
        for _ in 1..level {
            let Some((parent, _)) = package.rsplit_once('.') else {
                return String::new();
            };
            package = parent;
        }
        if package.is_empty() || name.is_empty() {
            package.to_owned()
        } else {
            format!("{package}.{name}")
        }
    }
}

#[derive(Default)]
struct Report {
    imports: Vec<Value>,
    edges: Vec<Value>,
    diagnostics: Vec<Value>,
}

impl Report {
    fn diagnostic(
        &mut self,
        file: &str,
        components: &BTreeMap<String, String>,
        position: Option<Position>,
        code: &str,
        message: &str,
    ) {
        self.diagnostics.push(json!({
            "code": code, "severity": "warning", "message": message,
            "location": {"file": file, "pointer": "", "line": position.map(|p| p.line),
                         "column": position.map(|p| p.column)},
            "related_ids": components.get(file).into_iter().collect::<Vec<_>>(),
        }));
    }
}

struct Observer<'a, 'r> {
    file: &'a str,
    components: &'a BTreeMap<String, String>,
    modules: &'a Modules<'a>,
    report: &'r mut Report,
}

struct Import<'a> {
    position: Position,
    statement: &'a str,
    module: &'a str,
    imported: Option<&'a str>,
}

impl<'a> Observer<'a, '_> {
    fn diagnostic(&mut self, pos: Position, code: &str, message: &str) {
        self.report
            .diagnostic(self.file, self.components, Some(pos), code, message);
    }

    fn record(&mut self, import: &Import<'_>, target: Option<&str>, external: bool) {
        let Import {
            position,
            statement,
            module,
            imported,
        } = import;
        self.report.imports.push(json!({
            "source_file": self.file, "line": position.line, "column": position.column,
            "module": module, "imported": imported, "statement": statement,
            "target_file": target, "external": external,
        }));
        let Some(target) = target else { return };
        let source_id = self.components.get(self.file).filter(|id| !id.is_empty());
        let target_id = self.components.get(target).filter(|id| !id.is_empty());
        if let (Some(from), Some(to)) = (source_id, target_id) {
            self.report
                .edges
                .push(json!({"from": from, "to": to, "source_file": self.file,
                "line": position.line, "column": position.column, "module": module,
                "target_file": target, "statement": statement}));
        } else if target_id.is_some()
            || (source_id.is_some() && !self.modules.inert.contains(target))
        {
            self.diagnostic(
                *position,
                "CODE_UNBOUND_IMPORT",
                &format!("Local dependency '{target}' has no complete component mapping."),
            );
        }
    }

    fn resolve(&mut self, import: &Import<'_>, relative: bool) -> Option<&'a str> {
        let module = import.module;
        if module.is_empty() {
            self.diagnostic(
                import.position,
                "CODE_IMPORT_UNRESOLVED",
                "Relative import has no regular package context.",
            );
            self.record(import, None, false);
            return None;
        }
        let root = module.split('.').next().unwrap();
        if !relative
            && (self.modules.runtime.contains(root)
                || self.modules.runtime.contains(module)
                || !self.modules.roots.contains(root))
        {
            self.record(import, None, true);
            return None;
        }
        for end in module
            .match_indices('.')
            .map(|(offset, _)| offset)
            .chain([module.len()])
        {
            let name = &module[..end];
            let target = self
                .modules
                .only_candidate(name)
                .filter(|path| end == module.len() || path.ends_with("/__init__.py"));
            let Some(target) = target else {
                self.diagnostic(import.position, "CODE_IMPORT_UNRESOLVED",
                    &format!("Local module '{name}' is missing, ambiguous, or outside the regular-package profile."));
                self.record(import, None, false);
                return None;
            };
            self.record(import, Some(target), false);
            if end == module.len() {
                return Some(target);
            }
        }
        None
    }
}

// Preserve the reference observer's lexicographic ordering of ASCII-escaped,
// sorted-key JSON, including supplementary-plane identifiers and path names.
fn sorted(rows: Vec<Value>) -> Vec<Value> {
    let mut indexed = BTreeMap::new();
    for row in rows {
        let mut canonical = row.clone();
        canonical.sort_all_objects();
        let mut key = String::new();
        for c in canonical.to_string().chars() {
            if c.is_ascii() && c != '\u{7f}' {
                key.push(c);
            } else {
                for unit in c.encode_utf16(&mut [0; 2]) {
                    write!(key, "\\u{unit:04x}").unwrap();
                }
            }
        }
        indexed.insert(key, row);
    }
    indexed.into_values().collect()
}

#[cfg(test)]
mod tests;
