use std::collections::{BTreeMap, VecDeque};

use ruff_python_ast::visitor::source_order::{SourceOrderVisitor, TraversalSignal};
use ruff_python_ast::{
    Alias, AnyNodeRef as Node, ElifElseClause, Expr, ExprCall, ExprContext, Stmt, StmtImportFrom,
};
use ruff_text_size::Ranged;

use super::source::Source;
use super::{Import, Observer, Position};

const DYNAMIC_VALUE: &str = "Dynamic import-system namespaces or loader values require analysis outside this static import profile.";
const DYNAMIC_CALL: &str =
    "Dynamic loading or reflection requires analysis outside this static import profile.";

enum Pending<'a> {
    Node(Node<'a>),
    Elif(&'a ElifElseClause, &'a [ElifElseClause]),
}

#[derive(Default)]
struct Children<'a>(VecDeque<Pending<'a>>);

impl<'a> SourceOrderVisitor<'a> for Children<'a> {
    fn enter_node(&mut self, node: Node<'a>) -> TraversalSignal {
        self.0.push_back(Pending::Node(node));
        TraversalSignal::Skip
    }
}

impl<'a> Children<'a> {
    fn if_children(&mut self, test: &'a Expr, body: &'a [Stmt], clauses: &'a [ElifElseClause]) {
        self.visit_expr(test);
        self.visit_body(body);
        if let Some((clause, rest)) = clauses.split_first() {
            if clause.test.is_some() {
                self.0.push_back(Pending::Elif(clause, rest));
            } else {
                self.visit_body(&clause.body);
            }
        }
    }
}

// Match ast.walk breadth-first alias precedence. Ruff groups all elif/else
// clauses together; the Python AST nests each elif as an If in the else body.
fn walk(body: &[Stmt]) -> Vec<Node<'_>> {
    let mut queue = Children::default();
    queue.visit_body(body);
    let mut nodes = Vec::new();
    while let Some(next) = queue.0.pop_front() {
        match next {
            Pending::Elif(clause, rest) => {
                queue.if_children(clause.test.as_ref().unwrap(), &clause.body, rest)
            }
            Pending::Node(Node::StmtIf(stmt)) => {
                queue.if_children(&stmt.test, &stmt.body, &stmt.elif_else_clauses)
            }
            Pending::Node(node) => {
                node.visit_source_order(&mut queue);
                nodes.push(node);
            }
        }
    }
    nodes
}

type Aliases = BTreeMap<String, String>;

fn expanded(expr: &Expr, aliases: &Aliases) -> Option<String> {
    let mut parts = Vec::new();
    let mut base = expr;
    while let Expr::Attribute(node) = base {
        parts.push(node.attr.as_str());
        base = &node.value;
    }
    let Expr::Name(base) = base else { return None };
    let base = base.id.as_str();
    let mut result = aliases
        .get(base)
        .map(String::as_str)
        .unwrap_or(base)
        .to_owned();
    for part in parts.into_iter().rev() {
        result.push('.');
        result.push_str(part);
    }
    Some(result)
}

fn aliases(nodes: &[Node<'_>]) -> Aliases {
    let mut aliases: Aliases = ["__import__", "eval", "exec"]
        .into_iter()
        .map(|name| (name.into(), format!("builtins.{name}")))
        .collect();
    let mut assignments = Vec::new();
    for node in nodes {
        match node {
            Node::StmtImport(stmt) => {
                for alias in &stmt.names {
                    let name = alias.name.as_str();
                    let root = name.split('.').next().unwrap();
                    let (key, value) = alias
                        .asname
                        .as_ref()
                        .map_or((root, root), |asname| (asname.as_str(), name));
                    aliases.insert(key.to_owned(), value.to_owned());
                }
            }
            Node::StmtImportFrom(stmt) if stmt.level == 0 => {
                if let Some(module) = &stmt.module {
                    for alias in &stmt.names {
                        let key = alias.asname.as_ref().unwrap_or(&alias.name);
                        aliases.insert(key.to_string(), format!("{module}.{}", alias.name));
                    }
                }
            }
            Node::StmtAssign(stmt) => {
                assignments.push((stmt.targets.as_slice(), stmt.value.as_ref()))
            }
            Node::StmtAnnAssign(stmt) => {
                if let Some(value) = &stmt.value {
                    assignments.push((std::slice::from_ref(stmt.target.as_ref()), value.as_ref()));
                }
            }
            _ => {}
        }
    }
    for _ in 0..assignments.len() {
        let mut changed = false;
        for (targets, value) in &assignments {
            let Some(name) = expanded(value, &aliases) else {
                continue;
            };
            if !["importlib", "builtins", "runpy", "sys.modules"]
                .iter()
                .any(|prefix| name.starts_with(prefix))
            {
                continue;
            }
            for target in *targets {
                if let Expr::Name(target) = target
                    && aliases.get(target.id.as_str()) != Some(&name)
                {
                    aliases.insert(target.id.to_string(), name.clone());
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    aliases
}

fn sensitive(name: &str) -> bool {
    let mut parts = name.split('.');
    match parts.next() {
        Some("importlib" | "runpy" | "__builtins__" | "__loader__" | "__spec__") => true,
        Some("sys") => matches!(
            parts.next(),
            Some(
                "modules"
                    | "meta_path"
                    | "path"
                    | "path_hooks"
                    | "path_importer_cache"
                    | "__dict__"
            )
        ),
        _ => matches!(
            name,
            "builtins" | "builtins.__import__" | "builtins.eval" | "builtins.exec"
        ),
    }
}

fn dynamic_call(call: &ExprCall, aliases: &Aliases) -> bool {
    if let Expr::Call(func) = call.func.as_ref()
        && matches!(func.func.as_ref(), Expr::Name(n) if n.id.as_str() == "getattr")
    {
        return true;
    }
    let Some(name) = expanded(&call.func, aliases) else {
        return false;
    };
    sensitive(&name)
        || name.ends_with(".exec_module")
        || name.ends_with(".load_module")
        || (matches!(name.as_str(), "getattr" | "vars")
            && call
                .arguments
                .args
                .first()
                .and_then(|arg| expanded(arg, aliases))
                .is_some_and(|arg| sensitive(&arg) || arg == "sys"))
}

fn import_names(names: &[Alias]) -> String {
    names
        .iter()
        .map(|alias| {
            if let Some(asname) = &alias.asname {
                format!("{} as {asname}", alias.name)
            } else {
                alias.name.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn from_import(stmt: &StmtImportFrom, pos: Position, observer: &mut Observer<'_, '_>) {
    let name = stmt.module.as_ref().map_or("", |id| id.as_str());
    let statement = format!(
        "from {}{name} import {}",
        ".".repeat(stmt.level as usize),
        import_names(&stmt.names)
    );
    let module = if stmt.level == 0 {
        name.to_owned()
    } else {
        observer
            .modules
            .relative_name(observer.file, stmt.level, name)
    };
    for alias in &stmt.names {
        let member = alias.name.as_str();
        let child = format!("{module}.{member}");
        if stmt.level == 0
            && (sensitive(&child)
                || (matches!(module.as_str(), "sys" | "builtins") && member == "*"))
        {
            observer.diagnostic(pos, "CODE_DYNAMIC_IMPORT", DYNAMIC_VALUE);
        }
        let import = Import {
            position: pos,
            statement: &statement,
            module: &module,
            imported: Some(member),
        };
        if let Some(target) = observer.resolve(&import, stmt.level > 0)
            && target.ends_with("/__init__.py")
        {
            if observer.modules.inert.contains(target)
                && member != "*"
                && observer.modules.only_candidate(&child).is_some()
            {
                observer.resolve(
                    &Import {
                        module: &child,
                        ..import
                    },
                    true,
                );
            } else {
                observer.diagnostic(pos, "CODE_IMPORT_UNRESOLVED",
                    &format!("Package member '{child}' cannot be resolved without executing its initializer."));
            }
        }
    }
}

pub(super) fn observe(source: &Source, observer: &mut Observer<'_, '_>) {
    let nodes = walk(&source.body);
    let aliases = aliases(&nodes);
    for node in nodes {
        let pos = source.position(node.start().to_usize());
        match node {
            Node::StmtImport(stmt) => {
                let statement = format!("import {}", import_names(&stmt.names));
                for alias in &stmt.names {
                    let module = alias.name.as_str();
                    observer.resolve(
                        &Import {
                            position: pos,
                            statement: &statement,
                            module,
                            imported: None,
                        },
                        false,
                    );
                    if sensitive(module) {
                        observer.diagnostic(pos, "CODE_DYNAMIC_IMPORT", DYNAMIC_VALUE);
                    }
                }
            }
            Node::StmtImportFrom(stmt) => from_import(stmt, pos, observer),
            Node::ExprCall(call) if dynamic_call(call, &aliases) => {
                observer.diagnostic(pos, "CODE_DYNAMIC_IMPORT", DYNAMIC_CALL)
            }
            Node::ExprName(expr) if expr.ctx == ExprContext::Load => {
                let id = expr.id.as_str();
                if sensitive(aliases.get(id).map(String::as_str).unwrap_or(id)) {
                    observer.diagnostic(pos, "CODE_DYNAMIC_IMPORT", DYNAMIC_VALUE);
                }
            }
            Node::ExprAttribute(expr) if expr.ctx == ExprContext::Load => {
                if let Some(base) = expanded(&expr.value, &aliases)
                    && sensitive(&format!("{base}.{}", expr.attr))
                {
                    observer.diagnostic(pos, "CODE_DYNAMIC_IMPORT", DYNAMIC_VALUE);
                }
            }
            _ => {}
        }
    }
}
