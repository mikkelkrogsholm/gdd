//! Deterministic structural replay of a validated run journal.

use super::JournalError;
use crate::input::Value;
use crate::model::{get, items, planning, string, strings, text};
use crate::validation::to_json;
use serde::Serialize;
use serde_json::Value as Json;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum RunPhase {
    Active,
    Cancelled,
    Accepted,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PackagePhase {
    Pending,
    Dispatched,
    Submitted,
    Integrated,
}

#[derive(Clone, Debug, Serialize)]
pub struct PackageState {
    pub status: PackagePhase,
    pub attempt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_code_revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Json>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub integration: Option<Json>,
}
impl Default for PackageState {
    fn default() -> Self {
        Self {
            status: PackagePhase::Pending,
            attempt: None,
            base_code_revision: None,
            result: None,
            integration: None,
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct RunState {
    pub status: RunPhase,
    pub code_revision: String,
    pub packages: BTreeMap<String, PackageState>,
    pub attempts: Vec<String>,
}

impl RunState {
    pub(super) fn initial(plan: &Value) -> Self {
        let request = get(plan, "request").expect("validated workplan");
        Self {
            status: RunPhase::Active,
            code_revision: string(request, "base_code_revision").into(),
            packages: items(plan, "packages")
                .iter()
                .map(|p| (string(p, "id").into(), PackageState::default()))
                .collect(),
            attempts: Vec::new(),
        }
    }

    fn prerequisites_met(&self, plan: &Value, package: &str) -> Result<bool, JournalError> {
        let request = get(plan, "request").expect("validated workplan");
        let mut integrated = true;
        for pair in items(request, "serialized") {
            let Value::Array(pair) = pair else {
                unreachable!("validated pair")
            };
            if text(&pair[1]) == Some(package) {
                let predecessor = self
                    .packages
                    .get(text(&pair[0]).expect("validated package ID"))
                    .ok_or_else(|| JournalError::invalid("Invalid prerequisite package."))?;
                integrated &= predecessor.status == PackagePhase::Integrated;
            }
        }
        Ok(integrated)
    }

    pub(super) fn available(&self, plan: &Value) -> Result<Vec<String>, JournalError> {
        let mut available = Vec::new();
        for (id, package) in &self.packages {
            if package.status == PackagePhase::Pending && self.prerequisites_met(plan, id)? {
                available.push(id.clone());
            }
        }
        Ok(available)
    }

    pub(super) fn apply(
        &mut self,
        plan: &Value,
        event: &Value,
        used_attempts: &mut BTreeSet<String>,
    ) -> Result<(), JournalError> {
        let kind = string(event, "type");
        let package_id = get(event, "package").and_then(text);
        if package_id.is_some_and(|id| !self.packages.contains_key(id)) {
            return Err(JournalError::invalid("Unknown workflow package."));
        }
        if self.status == RunPhase::Accepted {
            return Err(JournalError::invalid(
                "Accepted runs are terminal; create a new run for new work.",
            ));
        }
        if self.status == RunPhase::Cancelled && !matches!(kind, "submit" | "resume") {
            return Err(JournalError::invalid(
                "Cancelled runs preserve results but cannot advance work.",
            ));
        }
        if let Some(id) = package_id {
            let met = kind != "dispatch" || self.prerequisites_met(plan, id)?;
            let package = self.packages.get_mut(id).expect("known package");
            let declaration = || {
                items(plan, "packages")
                    .iter()
                    .find(|p| string(p, "id") == id)
                    .expect("declared package")
            };
            match kind {
                "dispatch" => {
                    if package.status != PackagePhase::Pending || !met {
                        return Err(JournalError::invalid(
                            "Package is not available; prerequisites must be integrated.",
                        ));
                    }
                    let attempt = string(event, "attempt");
                    if !used_attempts.insert(attempt.into()) {
                        return Err(JournalError::invalid("Attempt tokens cannot be reused."));
                    }
                    package.status = PackagePhase::Dispatched;
                    package.attempt = Some(attempt.into());
                    package.base_code_revision = Some(self.code_revision.clone());
                    self.attempts.push(attempt.into());
                }
                "submit" => {
                    if package.status != PackagePhase::Dispatched
                        || package.attempt.as_deref() != Some(string(event, "attempt"))
                    {
                        return Err(JournalError::invalid(
                            "Result is not for the current dispatched attempt.",
                        ));
                    }
                    if !planning::owns_files(declaration(), strings(event, "changed_resources")) {
                        return Err(JournalError::invalid(
                            "Reported changes escape package write ownership.",
                        ));
                    }
                    package.result = Some(to_json(event).expect("validated event"));
                    package.status = PackagePhase::Submitted;
                }
                "integrate" => {
                    if package.status != PackagePhase::Submitted
                        || package.attempt.as_deref() != Some(string(event, "attempt"))
                    {
                        return Err(JournalError::invalid(
                            "Integration needs the current submitted attempt.",
                        ));
                    }
                    if string(event, "base_code_revision") != self.code_revision {
                        return Err(JournalError::invalid(
                            "Integration base differs from current integrated code.",
                        ));
                    }
                    let changed =
                        package.result.as_ref().expect("submitted package")["changed_resources"]
                            .as_array()
                            .expect("submitted changes")
                            .iter()
                            .filter_map(Json::as_str)
                            .collect::<BTreeSet<_>>();
                    let verified = strings(event, "verified_resources").collect::<BTreeSet<_>>();
                    if changed != verified
                        || !planning::owns_files(declaration(), verified.iter().copied())
                    {
                        return Err(JournalError::invalid(
                            "Host-verified changes must match submitted changes and ownership.",
                        ));
                    }
                    package.integration = Some(to_json(event).expect("validated event"));
                    package.status = PackagePhase::Integrated;
                    self.code_revision = string(event, "code_revision").into();
                }
                "retry" => {
                    if !matches!(
                        package.status,
                        PackagePhase::Dispatched | PackagePhase::Submitted
                    ) {
                        return Err(JournalError::invalid(
                            "Only unfinished attempts can be retried.",
                        ));
                    }
                    *package = PackageState::default();
                }
                _ => unreachable!("validated package event"),
            }
        } else {
            self.status = match kind {
                "cancel" => RunPhase::Cancelled,
                "resume" if self.status == RunPhase::Cancelled => RunPhase::Active,
                "resume" => {
                    return Err(JournalError::invalid("Only cancelled runs can be resumed."));
                }
                "accept"
                    if self
                        .packages
                        .values()
                        .all(|p| p.status == PackagePhase::Integrated) =>
                {
                    RunPhase::Accepted
                }
                "accept" => {
                    return Err(JournalError::invalid(
                        "All packages must be integrated before acceptance.",
                    ));
                }
                _ => unreachable!("validated run event"),
            };
        }
        Ok(())
    }
}
