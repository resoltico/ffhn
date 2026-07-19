use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;
use time::{Date, OffsetDateTime};

use crate::model::DynResult;

use super::metrics::Metrics;

#[derive(Debug, Deserialize)]
pub(super) struct Policy {
    version: u32,
    #[serde(default)]
    rules: Vec<Rule>,
}

#[derive(Debug, Deserialize)]
pub(super) struct Rule {
    path: String,
    #[serde(rename = "match")]
    match_kind: MatchKind,
    role: String,
    owner: String,
    rationale: String,
    split_trigger: String,
    review_expires_on: Option<String>,
    max_physical_lines: usize,
    max_items: usize,
    max_public_items: usize,
    max_imports: usize,
    max_functions: usize,
    max_decision_points: usize,
    max_match_arms: usize,
    #[serde(default)]
    allowed_internal_dependencies: BTreeSet<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum MatchKind {
    Exact,
    Prefix,
}

impl Policy {
    pub(super) fn load(path: &Path) -> DynResult<Self> {
        let source = std::fs::read_to_string(path).map_err(|error| {
            format!(
                "cannot read Rust source-structure policy {}: {error}",
                path.display()
            )
        })?;
        let policy: Self = toml::from_str(&source).map_err(|error| {
            format!(
                "cannot parse Rust source-structure policy {}: {error}",
                path.display()
            )
        })?;
        policy.validate()?;
        Ok(policy)
    }

    pub(super) fn rule_for(&self, relative_path: &str) -> Option<&Rule> {
        self.rules
            .iter()
            .filter(|rule| rule.matches(relative_path))
            .max_by_key(|rule| rule.path.len())
    }

    pub(super) fn expired_rule_findings(&self) -> DynResult<Vec<String>> {
        let today = OffsetDateTime::now_utc().date();
        let mut findings = Vec::new();
        for rule in &self.rules {
            let Some(expiry) = rule.review_expiry()? else {
                continue;
            };
            if expiry < today {
                findings.push(format!(
                    "{}: ownership rule review expired on {expiry}; reassess the budget or split the module ({})",
                    rule.path, rule.split_trigger
                ));
            }
        }
        Ok(findings)
    }

    pub(super) fn unmatched_rule_findings(
        &self,
        matched_rule_paths: &BTreeSet<String>,
    ) -> Vec<String> {
        self.rules
            .iter()
            .filter(|rule| !matched_rule_paths.contains(rule.path()))
            .map(|rule| {
                format!(
                    "{}: ownership rule matches no maintained Rust source file; delete it or give the file an explicit rule",
                    rule.path()
                )
            })
            .collect()
    }

    fn validate(&self) -> DynResult<()> {
        if self.version != 1 {
            return Err(format!(
                "unsupported Rust source-structure policy version {}; expected 1",
                self.version
            )
            .into());
        }
        if self.rules.is_empty() {
            return Err("Rust source-structure policy must declare at least one rule".into());
        }
        let mut declared_rules = BTreeSet::new();
        for rule in &self.rules {
            rule.validate()?;
            if !declared_rules.insert((rule.path.as_str(), rule.match_kind)) {
                return Err(format!(
                    "duplicate Rust source-structure rule for {} `{}`",
                    rule.match_kind.name(),
                    rule.path
                )
                .into());
            }
        }
        Ok(())
    }
}

impl Rule {
    pub(super) fn path(&self) -> &str {
        &self.path
    }

    pub(super) fn role(&self) -> &str {
        &self.role
    }

    pub(super) fn budget_findings(&self, path: &str, metrics: &Metrics) -> Vec<String> {
        let checks = [
            (
                "physical lines",
                metrics.physical_lines,
                self.max_physical_lines,
            ),
            ("top-level items", metrics.item_count, self.max_items),
            (
                "public items",
                metrics.public_item_count,
                self.max_public_items,
            ),
            ("imports", metrics.import_count, self.max_imports),
            ("functions", metrics.function_count, self.max_functions),
            (
                "decision points",
                metrics.decision_points,
                self.max_decision_points,
            ),
            ("match arms", metrics.match_arms, self.max_match_arms),
        ];
        checks
            .into_iter()
            .filter(|(_, actual, maximum)| actual > maximum)
            .map(|(name, actual, maximum)| {
                format!(
                    "{path}: {name} {actual} exceeds {maximum} for role `{}`; split when {}",
                    self.role, self.split_trigger
                )
            })
            .collect()
    }

    pub(super) fn dependency_findings(
        &self,
        path: &str,
        dependencies: &BTreeSet<String>,
    ) -> Vec<String> {
        dependencies
            .iter()
            .filter(|dependency| !self.allowed_internal_dependencies.contains(*dependency))
            .map(|dependency| {
                format!(
                    "{path}: internal dependency `{dependency}` is forbidden for role `{}` owned by {}; {}",
                    self.role, self.owner, self.rationale
                )
            })
            .collect()
    }

    fn matches(&self, relative_path: &str) -> bool {
        match self.match_kind {
            MatchKind::Exact => self.path == relative_path,
            MatchKind::Prefix => relative_path.starts_with(&self.path),
        }
    }

    fn validate(&self) -> DynResult<()> {
        if self.path.is_empty()
            || self.role.is_empty()
            || self.owner.is_empty()
            || self.rationale.is_empty()
            || self.split_trigger.is_empty()
        {
            return Err("every Rust source-structure rule must declare path, role, owner, rationale, and split_trigger".into());
        }
        if self.match_kind == MatchKind::Prefix && !self.path.ends_with('/') {
            return Err(format!(
                "source-structure prefix rule `{}` must end with `/`",
                self.path
            )
            .into());
        }
        if self.path.starts_with('/')
            || self.path.contains('\\')
            || self.path.contains("//")
            || self
                .path
                .split('/')
                .any(|component| component == "." || component == "..")
        {
            return Err(format!(
                "source-structure rule path `{}` must be a normalized workspace-relative path",
                self.path
            )
            .into());
        }
        if self.match_kind == MatchKind::Exact && !self.path.ends_with(".rs") {
            return Err(format!(
                "source-structure exact rule `{}` must name a Rust source file",
                self.path
            )
            .into());
        }
        self.review_expiry()?;
        Ok(())
    }

    fn review_expiry(&self) -> DynResult<Option<Date>> {
        self.review_expires_on
            .as_deref()
            .map(|expiry| {
                Date::parse(expiry, &time::format_description::well_known::Iso8601::DATE).map_err(
                    |error| {
                        format!(
                            "source-structure rule `{}` has invalid review_expires_on: {error}",
                            self.path
                        )
                        .into()
                    },
                )
            })
            .transpose()
    }
}

impl MatchKind {
    const fn name(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Prefix => "prefix",
        }
    }
}
