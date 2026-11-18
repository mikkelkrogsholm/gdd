//! Render the actual predecessor paths retained by structural readiness.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Write};

use super::{ReadinessContexts, entries, string, strings};
use crate::bundle::Bundle;
use crate::context::ContextScope;
use crate::report::{CheckResult, Diagnostic, Report};

pub(crate) fn readiness(
    bundle: &Bundle,
    report: &Report,
    contexts: Option<&ReadinessContexts<'_>>,
    output: &mut dyn Write,
) -> io::Result<()> {
    let mut groups: BTreeMap<_, Vec<_>> = BTreeMap::new();
    for check in report.checks.iter().filter(|c| c.dimension == "readiness") {
        let package = check.check_id == "context.package";
        for diagnostic in &check.diagnostics {
            groups
                .entry((package, diagnostic.related_ids.as_slice()))
                .or_default()
                .push(diagnostic);
        }
    }
    if groups.is_empty() {
        return Ok(());
    }

    // Model defects may be propagated to an implementation target. Such a
    // location does not establish a graph path to the underlying defect.
    let nodes: BTreeSet<_> = bundle.nodes().map(|node| node.id).collect();
    let contexts = contexts.filter(|_| {
        report.result("model") == CheckResult::Pass
            && report.spec_revision.is_some()
            && report.scope.iter().all(|id| nodes.contains(id.as_str()))
    });
    let Some(contexts) = contexts else {
        for diagnostic in report
            .checks
            .iter()
            .filter(|c| c.dimension == "model" && c.result != CheckResult::Pass)
            .flat_map(|c| &c.diagnostics)
            .chain(groups.into_values().flatten())
        {
            write_diagnostic(output, diagnostic)?;
        }
        return Ok(());
    };

    let edges: BTreeMap<_, _> = bundle.edges().map(|edge| (edge.id, edge.value)).collect();
    let questions: BTreeMap<_, _> = bundle
        .manifest()
        .into_iter()
        .flat_map(|manifest| entries(&manifest.value, "questions"))
        .map(|(id, value)| (id.as_str(), value))
        .collect();
    for ((package, ids), diagnostics) in groups {
        let context = if package {
            contexts
                .empty_package
                .as_ref()
                .unwrap_or(&contexts.structural)
        } else {
            &contexts.structural
        };

        for diagnostic in diagnostics {
            write_diagnostic(output, diagnostic)?;
        }
        for id in ids.iter().map(String::as_str) {
            if nodes.contains(id) {
                write_path(output, bundle, context, id)?;
            } else if let Some(question) = questions.get(id) {
                for target in strings(question, "targets")
                    .filter(|target| context.contains(target) && nodes.contains(target))
                {
                    writeln!(output, "  Question {id} targets {target}.")?;
                    write_path(output, bundle, context, target)?;
                }
            } else if let Some(edge) = edges.get(id) {
                let from = string(edge, "from");
                let to = string(edge, "to");
                writeln!(
                    output,
                    "  Edge {id} connects {from} -> {to} ({}).",
                    string(edge, "relation")
                )?;
                write_path(output, bundle, context, from)?;
                if to != from {
                    write_path(output, bundle, context, to)?;
                }
            }
        }
    }
    Ok(())
}

fn write_diagnostic(output: &mut dyn Write, diagnostic: &Diagnostic) -> io::Result<()> {
    write!(output, "{}", diagnostic.code)?;
    if let Some(file) = &diagnostic.location.file {
        write!(output, " {file}#{}", diagnostic.location.pointer)?;
    }
    writeln!(output, "  {}", diagnostic.message)
}

fn write_path(
    output: &mut dyn Write,
    bundle: &Bundle,
    context: &ContextScope<'_>,
    id: &str,
) -> io::Result<()> {
    let Some(path) = context.path_to(id) else {
        return Ok(());
    };
    if path.is_empty() {
        return writeln!(output, "  Selected scope includes {id}.");
    }
    if let Some(root) = path[0].from {
        writeln!(output, "  Scope path from {root}")?;
    }
    for step in path {
        write!(
            output,
            "    {} -> {} ({})",
            step.from.unwrap_or("<global>"),
            step.to,
            step.derivation
        )?;
        if let (Some(edge), Some(relation)) = (step.edge, step.relation) {
            write!(output, " [{} {relation}, {edge}]", step.orientation)?;
        }
        if let Some(location) = bundle.location(step.to) {
            write!(output, " {}#{}", location.file, location.pointer)?;
        }
        writeln!(output)?;
    }
    Ok(())
}
