use crate::{DisplayFormat, tool_input, write_output, write_text};
use clap::{Args, Subcommand};
use gdd::issues::{self, GitHub};
use gdd::work_overview::{WorkOverview, work_overview, work_overview_text};
use std::io;
use std::path::PathBuf;

#[derive(Args)]
pub struct RunArgs {
    /// GDD model manifest.
    path: PathBuf,
    #[arg(long)]
    run: PathBuf,
    #[arg(long)]
    host_context: Option<PathBuf>,
}

impl RunArgs {
    fn overview(&self) -> io::Result<WorkOverview> {
        let host = tool_input(self.host_context.clone())?;
        work_overview(&self.path, &self.run, host.as_ref())
    }
}

#[derive(Args)]
pub struct WorkArgs {
    #[command(flatten)]
    run: RunArgs,
    /// Limit the overview to packages related to these model nodes.
    #[arg(long)]
    scope: Vec<String>,
    /// Directory of stable work-package links.
    #[arg(long)]
    issues: Option<PathBuf>,
    /// Read current GitHub issue titles, assignees and open/closed state.
    #[arg(long, requires = "issues")]
    refresh_issues: bool,
    #[arg(long, value_enum, default_value = "json")]
    format: DisplayFormat,
    #[arg(long)]
    output: Option<PathBuf>,
}

fn github() -> GitHub {
    let token = ["GITHUB_TOKEN", "GH_TOKEN"]
        .iter()
        .filter_map(|key| std::env::var(key).ok())
        .find(|value| !value.is_empty());
    GitHub::new(token)
}

pub fn show(args: WorkArgs) -> io::Result<()> {
    let mut report = args.run.overview()?;
    report.retain_scope(&args.scope)?;
    if let Some(root) = args.issues {
        if !root.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Issue links directory does not exist: {}", root.display()),
            ));
        }
        let github = github();
        for package in &mut report.packages {
            if let Some(mut link) = issues::lookup(&root, &report.project_id, &package.id)? {
                if args.refresh_issues && link.is_github() {
                    link.github = Some(github.inspect(&link)?);
                }
                package.issue = Some(link);
            }
        }
    }
    match args.format {
        DisplayFormat::Json => write_output(args.output.as_deref(), &report),
        DisplayFormat::Text => write_text(
            args.output.as_deref(),
            &work_overview_text(&report, &args.scope),
        ),
    }
}

#[derive(Args)]
pub struct PackageArgs {
    #[command(flatten)]
    run: RunArgs,
    #[arg(long)]
    package: String,
    #[arg(long)]
    issues: PathBuf,
}

#[derive(Subcommand)]
pub enum IssueCommand {
    /// Link an existing tracker issue to a project and work-package ID.
    Link {
        #[command(flatten)]
        args: PackageArgs,
        #[arg(long)]
        url: String,
    },
    /// Publish or update the authenticated user's GDD result comment on GitHub.
    Publish {
        #[command(flatten)]
        args: PackageArgs,
        /// Public or team-accessible HTTPS address of the work result.
        #[arg(long)]
        result_url: String,
    },
}

pub fn issue_command(command: IssueCommand) -> io::Result<()> {
    let args = match &command {
        IssueCommand::Link { args, .. } | IssueCommand::Publish { args, .. } => args,
    };
    let overview = args.run.overview()?;
    let package = overview
        .packages
        .iter()
        .find(|p| p.id == args.package)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Unknown package: {}", args.package),
            )
        })?;
    match command {
        IssueCommand::Link { args, url } => write_output(
            None,
            &issues::link(&args.issues, &overview.project_id, &package.id, &url)?,
        ),
        IssueCommand::Publish { args, result_url } => {
            let status = github().publish_linked(&args.issues, &overview, package, &result_url)?;
            write_output(
                None,
                &serde_json::json!({"comment": status, "package": package.id}),
            )
        }
    }
}
