#[cfg(test)]
mod corpus_tests;
mod files;
mod sync;

use anyhow::{Context, Result, ensure};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const COPIES: &str = "references/gdd";
const INDEX: &str = "references/source-index.json";
const CATALOG: &str = "references/catalog.md";
const FORMAT: &str = "gdd-skill-assets/1";

#[derive(Parser)]
#[command(about = "Maintain the portable GDD skill's reference files")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate references from the maintained source checkout.
    Sync {
        /// Checkout containing the source documents and skills/gdd.
        #[arg(long)]
        root: Option<PathBuf>,
        /// Report stale assets without writing.
        #[arg(long)]
        check: bool,
        /// Override the source repository URL recorded in the index.
        #[arg(long)]
        repository: Option<String>,
        /// Override the source Git base recorded in the index.
        #[arg(long)]
        git_base_revision: Option<String>,
    },
    /// Compare bundled reference files with the portable skill's local index.
    Verify {
        /// Installed skill directory containing references/source-index.json.
        skill_dir: Option<PathBuf>,
    },
}

#[derive(Deserialize, Serialize)]
struct Entry {
    path: String,
    // Local verification uses only path, SHA-256 and size.
    #[serde(default)]
    source: String,
    sha256: String,
    bytes: u64,
}

fn digest(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn record_path<'a>(seen: &mut BTreeSet<&'a str>, relative: &'a str) -> Result<()> {
    ensure!(
        relative.starts_with("references/gdd/") && files::is_relative_name(relative),
        "Generated reference escapes its directory: {relative}"
    );
    ensure!(
        seen.insert(relative),
        "Duplicate generated reference: {relative}"
    );
    Ok(())
}

fn verify(skill: &Path) -> Result<Vec<String>> {
    #[derive(Deserialize)]
    struct LocalIndex {
        format: String,
        files: Vec<Entry>,
    }
    let skill = skill
        .canonicalize()
        .with_context(|| skill.display().to_string())?;
    let index: LocalIndex = serde_json::from_slice(&files::read(&files::output(&skill, INDEX)?)?)
        .context("Invalid skill asset index")?;
    ensure!(index.format == FORMAT, "Unsupported skill asset index");
    let actual = files::generated_files(&skill)?;
    let mut expected = BTreeSet::new();
    let mut errors = Vec::new();
    for entry in &index.files {
        record_path(&mut expected, &entry.path)?;
        if !actual.contains(&entry.path) {
            errors.push(format!("Missing reference: {}", entry.path));
            continue;
        }
        let data = files::read(&skill.join(&entry.path))?;
        if entry.bytes != data.len() as u64 || entry.sha256 != digest(&data) {
            errors.push(format!("Reference hash mismatch: {}", entry.path));
        }
    }
    for path in actual {
        if !expected.contains(path.as_str()) {
            errors.push(format!("Unindexed reference: {path}"));
        }
    }
    Ok(errors)
}

fn is_skill(dir: &Path) -> bool {
    dir.join("SKILL.md").is_file()
        && (dir.join("scripts").is_dir() || dir.join("references").is_dir())
}

fn default_skill() -> Result<PathBuf> {
    // A distributed binary in the skill's scripts directory belongs to that
    // copy, including when its reference directory is damaged or missing.
    let exe = std::env::current_exe()?;
    for scripts in exe.ancestors().filter(|dir| dir.ends_with("scripts")) {
        if let Some(skill) = scripts.parent()
            && (is_skill(skill) || skill.join(INDEX).is_file())
        {
            return Ok(skill.to_owned());
        }
    }
    // An explicit cargo run --manifest-path selects that skill even from a
    // different checkout. This uses the active invocation, never a build path.
    if let Some(manifest) = std::env::var_os("CARGO_MANIFEST_DIR")
        && Path::new(&manifest).ends_with("scripts")
        && let Some(skill) = Path::new(&manifest).parent()
        && is_skill(skill)
    {
        return Ok(skill.to_owned());
    }
    let cwd = std::env::current_dir()?;
    for directory in cwd.ancestors() {
        if is_skill(directory) {
            return Ok(directory.to_owned());
        }
        let bundled = directory.join("skills/gdd");
        if is_skill(&bundled) {
            return Ok(bundled);
        }
    }
    anyhow::bail!("Cannot locate the GDD skill. Supply verify SKILL_DIR or sync --root PATH.")
}

fn run(args: Args) -> Result<ExitCode> {
    let (messages, success) = match args.command {
        Command::Verify { skill_dir } => {
            let skill = match skill_dir {
                Some(path) => path,
                None => default_skill()?,
            };
            (
                verify(&skill).context("Skill integrity check failed")?,
                "Bundled reference hashes match. This checks local integrity, not authority or upstream freshness.",
            )
        }
        Command::Sync {
            root,
            check,
            repository,
            git_base_revision,
        } => {
            let root = match root {
                Some(path) => path,
                None => {
                    let skill = default_skill()?;
                    ensure!(
                        skill.ends_with("skills/gdd"),
                        "Supply the source checkout with --root"
                    );
                    skill.parent().unwrap().parent().unwrap().to_owned()
                }
            };
            let messages = sync::sync(&root, check, repository, git_base_revision)
                .context("Skill synchronization failed")?;
            (
                messages,
                if check {
                    "Skill assets are current."
                } else {
                    "Skill assets synchronized."
                },
            )
        }
    };
    if messages.is_empty() {
        println!("{success}");
        Ok(ExitCode::SUCCESS)
    } else {
        eprintln!("{}", messages.join("\n"));
        Ok(ExitCode::FAILURE)
    }
}

fn main() -> ExitCode {
    run(Args::parse()).unwrap_or_else(|error| {
        eprintln!("{error:#}");
        ExitCode::FAILURE
    })
}
