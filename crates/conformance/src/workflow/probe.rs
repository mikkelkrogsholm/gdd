//! Synthetic workflow protocol fixtures. Evidence and authority here are test inputs.

use super::*;
use gdd::workflow::prepare_workflow;
use gdd::workflow_run::{AdvanceResult, RunStatus, advance_run, create_run, run_status};

pub(super) struct Probe {
    pub temp: tempfile::TempDir,
    pub path: PathBuf,
    pub run: PathBuf,
    pub request: Value,
    pub host: Value,
    pub records: Value,
}

impl Probe {
    pub fn new(multi: bool, question: bool) -> Result<Self> {
        let temp = tempfile::tempdir()?;
        let path = temp.path().join("model.json");
        let run = temp.path().join("run");
        let mut model = json!({
            "format":"gdd/0.2", "project":{"id":"workflow.probe","title":"Workflow","goal":"Bounded protocol"},
            "nodes":{},"edges":[],"record_files":["records.json"]
        });
        if question {
            model["questions"] = json!({"question.accept":{"statement":"Resolve acceptance",
                "targets":["entity.x"],"blocks":["acceptance"],"status":"open"}});
        }
        let names: &[&str] = if multi { &["x", "y"] } else { &["x"] };
        let mut packages = Vec::new();
        let mut checks = Vec::new();
        for &name in names {
            let entity = format!("entity.{name}");
            let rule = format!("rule.{name}");
            let scenario = format!("scenario.{name}");
            model["nodes"][&entity] = json!({"kind":"entity","description":"Value","fields":{}});
            model["nodes"][&rule] = json!({"kind":"rule","description":"Invariant","rule":{
                "category":"invariant","statement":"Required behavior","scope":"targeted",
                "blocking":true,"verification":["probe"]
            }});
            model["nodes"][&scenario] = json!({"kind":"scenario","description":"Verify invariant","scenario":{
                "category":"normal","given":"Value","when":"Observed","then":"Valid"
            }});
            model["edges"].as_array_mut().context("missing fixture edges")?.extend([
                json!({"id":format!("edge.constraint.{name}"),"from":rule,"relation":"constrains","to":entity}),
                json!({"id":format!("edge.verify.{name}"),"from":scenario,"relation":"verifies","to":rule}),
            ]);
            packages.push(json!({"id":format!("package.{name}"),"scope":[entity],"write_resources":[format!("{name}.py")],
                "read_resources":[],"workspace":format!("workspaces/{name}"),
                "deliverables":["Entity implementation"],"integration_plan":"Merge and check"}));
            checks.push(format!("check.{name}"));
        }
        write_json(&path, &model)?;
        let spec = revision(&path)?;
        let mut records = json!({});
        for &name in names {
            records[format!("check.{name}")] = json!({"kind":"check","spec_revision":spec,
                "scope":[format!("rule.{name}")],"method":"probe","required_environment":[],"negative_controls":[]});
        }
        let request = json!({
            "format":"gdd-work-request/0.1","id":"run.probe","objective":"Implement entities",
            "implementation":"implementation.probe","base_code_revision":"code.base","packages":packages,
            "serialized":[],"integration":{"owner":"actor.probe","checks":checks}
        });
        let host = json!({
            "implementation":"implementation.probe","code_revision":"code.base",
            "policy":{"revision":"policy.probe","actor":"actor.probe","activity":"implementation",
                "spec_revision":spec,"scope":[],"requirement":"standing","authorities":["actor.probe"],
                "basis":"Synthetic bounded host mandate"}
        });
        let case = Self {
            temp,
            path,
            run,
            request,
            host,
            records,
        };
        case.write_records()?;
        Ok(case)
    }

    pub fn write_records(&self) -> Result<()> {
        write_json(
            &self.temp.path().join("records.json"),
            &json!({
                "format":"gdd-records/0.2","project_id":"workflow.probe","records":self.records
            }),
        )
    }

    pub fn plan(&self) -> Result<Value> {
        Ok(prepare_workflow(
            &self.path,
            &Input::from(&self.request),
            Some(&Input::from(&self.host)),
        )?
        .artifact)
    }

    pub fn start(&self) -> Result<RunStatus> {
        Ok(create_run(
            &self.path,
            &Input::from(&self.plan()?),
            &self.run,
            Some(&Input::from(&self.host)),
        )?)
    }

    pub fn status(&self) -> Result<RunStatus> {
        Ok(run_status(
            &self.path,
            &self.run,
            Some(&Input::from(&self.host)),
        )?)
    }

    pub fn advance(&self, event: &Value) -> std::io::Result<AdvanceResult> {
        advance_run(
            &self.path,
            &self.run,
            &Input::from(event),
            Some(&Input::from(&self.host)),
        )
    }

    pub fn event(&self, mut fields: Value) -> Result<Value> {
        let status = self.status()?;
        fields["idempotency_key"] = json!(format!(
            "{}.{}",
            fields["type"].as_str().context("missing type")?,
            status.event_count
        ));
        fields["expected_revision"] = json!(status.revision);
        Ok(fields)
    }

    pub fn step(&self, fields: Value) -> Result<AdvanceResult> {
        Ok(self.advance(&self.event(fields)?)?)
    }

    pub fn dispatch(&self, attempt: &str) -> Result<AdvanceResult> {
        self.step(json!({"type":"dispatch","package":"package.x","attempt":attempt}))
    }

    pub fn submit(&self) -> Result<AdvanceResult> {
        self.step(json!({"type":"submit","package":"package.x","attempt":"attempt.1",
            "summary":"Untrusted synthetic completion report","artifact_uri":"fixture://patch","changed_resources":["x.py"]}))
    }

    pub fn submitted(&mut self) -> Result<()> {
        self.start()?;
        ensure!(self.dispatch("attempt.1")?.applied && self.submit()?.applied);
        self.host["code_revision"] = json!("code.result");
        Ok(())
    }

    pub fn integration() -> Value {
        json!({"type":"integrate","package":"package.x","attempt":"attempt.1",
            "base_code_revision":"code.base","code_revision":"code.result","verified_resources":["x.py"],
            "review":{"reviewer":"reviewer.probe","verdict":"PASS","evidence_uri":"fixture://synthetic-review"}})
    }

    pub fn attach_evidence(&mut self) -> Result<()> {
        let spec = &self.host["policy"]["spec_revision"];
        self.records["binding.x"] = json!({"kind":"binding","spec_revision":spec,"targets":["rule.x"],
            "implementation":"implementation.probe","role":"test","resource":"x.py",
            "selector":{"kind":"whole-document"},"status":"checked","revision":"resource.result","code_revision":"code.result"});
        self.records["evidence.x"] = json!({"kind":"evidence","check":"check.x",
            "check_revision":gdd::canonical_digest(&self.records["check.x"]),"implementation":"implementation.probe",
            "spec_revision":spec,"code_revision":"code.result","checker":{"name":"probe","version":"1"},
            "scope":["rule.x"],"result":"PASS","executed":true,"reason":"Synthetic test assertion",
            "environment":{},"evidence_uri":"fixture://synthetic-raw"});
        self.write_records()?;
        self.host["supported_methods"] = json!(["probe"]);
        self.host["trusted_evidence"] = json!(["evidence.x"]);
        self.host["verified_bindings"] = json!(["binding.x"]);
        self.host["environment"] = json!({});
        self.host["negative_controls"] = json!([]);
        self.host["policy"]["activity"] = json!("acceptance");
        Ok(())
    }
}
