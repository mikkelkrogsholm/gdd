use crate::{DisplayFormat, write_output, write_text};
use clap::Args;
use gdd::knowledge::KnowledgeIndex;
use std::{collections::BTreeSet, fmt::Write, io, path::PathBuf, process::ExitCode};

#[derive(Args)]
pub struct WikiArgs {
    /// GDD model containing the project's knowledge source declarations.
    path: PathBuf,
    /// Create or refresh the Markdown navigation page next to the model.
    #[arg(long, conflicts_with_all = ["scope", "source", "search", "check", "output", "format"])]
    publish_navigation: bool,
    /// Select graph IDs. Repeat to include multiple targets.
    #[arg(long)]
    scope: Vec<String>,
    /// Select a declared wiki source by its stable ID.
    #[arg(long)]
    source: Option<String>,
    /// Search the selected material for text or an identifier.
    #[arg(long)]
    search: Option<String>,
    /// Return a nonzero exit code if selected links or sources need attention.
    #[arg(long)]
    check: bool,
    #[arg(long, value_enum, default_value = "json")]
    format: DisplayFormat,
    #[arg(long)]
    output: Option<PathBuf>,
}

fn text(index: &KnowledgeIndex) -> String {
    let mut out = format!("Project wiki for {}\n", index.project_id);
    if index.pages.is_empty() && index.questions.is_empty() {
        out.push_str("No matching knowledge sources or questions.\n");
    }
    for page in &index.pages {
        writeln!(
            out,
            "\n{}  {}  [{}]\n  {}",
            page.id, page.title, page.status, page.uri
        )
        .unwrap();
        if let Some(actual) = &page.sha256 {
            writeln!(out, "  SHA-256 {actual}").unwrap();
        }
        writeln!(out, "  {}", page.detail).unwrap();
        for origin in &page.derived_from {
            let revision = origin
                .sha256
                .as_deref()
                .or(origin.producer_revision.as_deref())
                .unwrap_or("unspecified revision");
            writeln!(
                out,
                "  Derived from {} at {revision} [{}]",
                origin.source, origin.status
            )
            .unwrap();
        }
        let mut seen_excerpts = BTreeSet::new();
        for link in index.links.iter().filter(|link| link.source == page.id) {
            let heading = link.heading.as_deref().unwrap_or("(document)");
            writeln!(
                out,
                "  {}  {}  {}  {}  [{}]",
                link.target, heading, link.kind, link.purpose, link.status
            )
            .unwrap();
            if let Some(excerpt) = &link.excerpt {
                if seen_excerpts.insert(excerpt.as_str()) {
                    writeln!(out, "    {}", excerpt.trim()).unwrap();
                    if link.truncated {
                        writeln!(
                            out,
                            "    Preview ends here. Read the full section in {}.",
                            page.uri
                        )
                        .unwrap();
                    }
                }
            } else {
                writeln!(out, "    {}", link.detail).unwrap();
            }
        }
    }
    for hit in &index.matches {
        writeln!(
            out,
            "\nMatching section in {}  {}\n  {}",
            hit.source,
            hit.heading.as_deref().unwrap_or("(document)"),
            hit.excerpt
        )
        .unwrap();
    }
    for question in &index.questions {
        writeln!(
            out,
            "\n{}  [{}]  {}\n  Affects {}",
            question.id,
            question.status,
            question.statement,
            question.targets.join(", ")
        )
        .unwrap();
    }
    out
}

pub fn show(args: WikiArgs) -> io::Result<ExitCode> {
    if args.publish_navigation {
        let path = gdd::knowledge::navigation::publish_navigation(&args.path)?;
        println!("{}", path.display());
        return Ok(ExitCode::SUCCESS);
    }
    let bundle = gdd::bundle::load_for_knowledge(&args.path, &args.scope);
    let mut report =
        KnowledgeIndex::from_bundle(&bundle)?.select(&args.scope, args.source.as_deref())?;
    if let Some(query) = args.search {
        if query.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Search text must be nonempty.",
            ));
        }
        if args.scope.is_empty() {
            report.search_bundle(&bundle, &query, args.source.as_deref())?;
        } else {
            report.search(&query);
        }
    }
    let rate = |status: &str| match status {
        "pinned" | "unpinned" => 0,
        "stale" | "broken" => 1,
        _ => 2,
    };
    let exit = if args.check {
        let outcomes = report
            .pages
            .iter()
            .map(|p| rate(&p.status))
            .chain(report.links.iter().map(|l| {
                if l.purpose == "contract" && l.status == "unpinned" {
                    1
                } else {
                    rate(&l.status)
                }
            }));
        outcomes.fold(0, |acc, x| if acc == 1 || x == 1 { 1 } else { acc.max(x) })
    } else {
        0
    };
    match args.format {
        DisplayFormat::Json => write_output(args.output.as_deref(), &report)?,
        DisplayFormat::Text => write_text(args.output.as_deref(), &text(&report))?,
    }
    Ok(ExitCode::from(exit))
}
