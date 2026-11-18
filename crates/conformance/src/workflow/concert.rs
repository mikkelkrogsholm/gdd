//! Private concert bundle with its original record and advisory source bytes.

use super::*;
use gdd::workflow::{WorkflowResult, prepare_workflow, verify_workflow};

pub(super) const RESERVE: &str = "action.reserve";
pub(super) const CANCEL: &str = "action.cancel";

pub(super) struct Concert {
    pub temp: tempfile::TempDir,
    pub path: PathBuf,
    pub request: Value,
}

impl Concert {
    pub fn new(directory: &Path, scopes: &[&str]) -> Result<Self> {
        let examples = directory.join("../examples");
        let temp = tempfile::tempdir()?;
        fs::create_dir(temp.path().join("sources"))?;
        for asset in [
            "concert.gdd.yaml",
            "concert.records.yaml",
            "sources/legacy-concert.gdd.yaml",
        ] {
            fs::copy(examples.join(asset), temp.path().join(asset))?;
        }
        let path = temp.path().join("concert.gdd.yaml");
        let ledger = read_document(&temp.path().join("concert.records.yaml"))?;
        let checks: std::collections::BTreeSet<_> = ledger["records"]
            .as_object()
            .context("missing concert ledger")?
            .iter()
            .filter(|(_, record)| record["kind"] == "check")
            .map(|(id, _)| id)
            .collect();
        let mut packages = Vec::new();
        for &scope in scopes {
            let name = scope.strip_prefix("action.").unwrap_or(scope);
            let workspace = temp.path().join("workspaces").join(name);
            fs::create_dir_all(&workspace)?;
            packages.push(json!({
                "id":format!("package.{name}"), "scope":[scope],
                "write_resources":[format!("src/{name}"), format!("tests/{name}")],
                "read_resources":["src/contracts"],"workspace":workspace,
                "deliverables":["Implement the scoped contract and its checks"],
                "integration_plan":"Integrate against the selected shared-rule checkplans"
            }));
        }
        let request = json!({
            "format":"gdd-work-request/0.1","id":"workflow.synthetic",
            "objective":"Implement the selected concert contracts",
            "implementation":"implementation.synthetic","base_code_revision":"code.synthetic.base",
            "packages":packages,"serialized":[],"integration":{"owner":"actor.synthetic","checks":checks}
        });
        let case = Self {
            temp,
            path,
            request,
        };
        case.backend_ready()?;
        Ok(case)
    }

    fn host(&self) -> Result<Input> {
        Ok(Input::from(&json!({
            "implementation":"implementation.synthetic","code_revision":"code.synthetic.base",
            "policy":{"revision":"policy.synthetic","actor":"actor.synthetic","activity":"implementation",
                "spec_revision":revision(&self.path)?,"scope":[],"requirement":"standing",
                "authorities":["actor.synthetic"],"basis":"Synthetic fixture mandate for the packet checks."}
        })))
    }

    pub fn prepare(&self) -> Result<WorkflowResult> {
        let result =
            prepare_workflow(&self.path, &Input::from(&self.request), Some(&self.host()?))?;
        ensure!(result.format == "gdd-workflow-result/0.1");
        Ok(result)
    }

    pub fn integrity(&self, plan: &Value) -> Result<CheckResult> {
        let report = verify_workflow(&self.path, &Input::from(plan), Some(&self.host()?))?;
        let checks: Vec<_> = report
            .checks
            .iter()
            .filter(|c| c.check_id == "workflow.packet_integrity")
            .collect();
        ensure!(
            checks.len() == 1 && checks[0].dimension == "readiness",
            "expected a single readiness packet-integrity check"
        );
        Ok(checks[0].result)
    }

    pub fn current_plan(&self) -> Result<Value> {
        let result = self.prepare()?;
        for dimension in ["model", "readiness"] {
            ensure!(
                result.report.result(dimension) == CheckResult::Pass,
                "fixture {dimension} did not pass"
            );
        }
        ensure!(self.integrity(&result.artifact)? == CheckResult::Pass);
        Ok(result.artifact)
    }

    pub fn backend_ready(&self) -> Result<()> {
        let report = validation::validate_path(
            &self.path,
            ValidationOptions {
                profiles: &[Profile::Readiness],
                scope: &[RESERVE.into()],
                ..Default::default()
            },
            None,
        )?;
        ensure!(
            report.result("model") == CheckResult::Pass
                && report.result("readiness") == CheckResult::Pass
        );
        Ok(())
    }

    pub fn rebase_planned_records(&self) -> Result<()> {
        let spec = revision(&self.path)?;
        let path = self.temp.path().join("concert.records.yaml");
        let mut ledger = read_document(&path)?;
        for r in ledger["records"]
            .as_object_mut()
            .context("expected fixture ledger")?
            .values_mut()
        {
            ensure!(
                r["kind"] == "check" || r["kind"] == "binding",
                "can only rebase planned records"
            );
            r["spec_revision"] = json!(spec);
        }
        write_json(&path, &ledger)
    }

    pub fn rewrite(&self, model: &Value) -> Result<()> {
        write_json(&self.path, model)?;
        self.rebase_planned_records()
    }
}
