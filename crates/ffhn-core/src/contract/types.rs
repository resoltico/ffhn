use crate::RunMode;

/// One user-facing CLI operation argument kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CliArgumentValueKind {
    /// Boolean flag.
    Flag,
    /// Filesystem path value.
    Path,
    /// Free-form string value.
    String,
    /// Positive integer value.
    PositiveInteger,
}

/// One canonical user-facing CLI argument description.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CliArgumentContract {
    /// Stable argument id.
    pub id: &'static str,
    /// Long-option name without the `--` prefix.
    pub long_name: &'static str,
    /// Human-facing label.
    pub display_label: &'static str,
    /// Value placeholder shown in help output when applicable.
    pub value_name: Option<&'static str>,
    /// User-facing help summary.
    pub help_summary: &'static str,
    /// Parser/value shape.
    pub value_kind: CliArgumentValueKind,
    /// Whether the argument may repeat.
    pub repeatable: bool,
    /// Whether the argument is required.
    pub required: bool,
    /// Conflicting argument ids.
    pub conflicts_with: &'static [&'static str],
    /// Default value shown in help when applicable.
    pub default_value: Option<&'static str>,
}

/// One canonical user-facing CLI invocation summary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CliInvocationContract {
    /// Stable invocation id.
    pub id: &'static str,
    /// Owning CLI operation id.
    pub operation_id: &'static str,
    /// Canonical usage pattern.
    pub usage: &'static str,
    /// Structured stdout document id.
    pub output_document_id: &'static str,
    /// Machine-usable summary used in catalog tables.
    pub analysis_summary: &'static str,
}

/// One canonical user-facing CLI operation description.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CliOperationContract {
    /// Stable operation id.
    pub id: &'static str,
    /// Human-facing label.
    pub display_label: &'static str,
    /// Canonical help summary.
    pub help_summary: &'static str,
    /// Canonical usage synopsis shown in help and usage errors.
    pub usage: &'static str,
    /// Operation arguments in display order.
    pub arguments: &'static [CliArgumentContract],
    /// Canonical invocation patterns for the operation.
    pub invocations: &'static [CliInvocationContract],
    /// Canonical runnable command examples for the operation.
    pub examples: &'static [&'static str],
    /// Canonical structured-stdout notes for the operation.
    pub output_notes: &'static [&'static str],
    /// Canonical operational notes that are not part of the command grammar.
    pub operational_notes: &'static [&'static str],
}

/// One canonical execution-mode description.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExecutionModeContract {
    /// Stable serialized mode value.
    pub mode: RunMode,
    /// Stable mode id.
    pub id: &'static str,
    /// Human-facing label.
    pub display_label: &'static str,
    /// User-facing mode summary.
    pub summary: &'static str,
    /// Whether the mode persists `state.json`.
    pub writes_state: bool,
}

/// One canonical CLI hard limitation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CliHardLimitContract {
    /// Stable hard-limit id.
    pub id: &'static str,
    /// Owning operation id when the limit is operation-specific.
    pub operation_id: Option<&'static str>,
    /// Human-facing label.
    pub display_label: &'static str,
    /// User-facing summary.
    pub summary: &'static str,
    /// CLI-usage error template. Use `{detail}` for one appended value.
    pub cli_usage_error_template: &'static str,
}

impl CliHardLimitContract {
    /// Renders the canonical CLI-usage error for this hard limit.
    pub fn render_cli_usage_error(self, detail: Option<&str>) -> String {
        match detail {
            Some(detail) if self.cli_usage_error_template.contains("{detail}") => {
                self.cli_usage_error_template.replace("{detail}", detail)
            }
            _ => self.cli_usage_error_template.to_owned(),
        }
    }
}

/// One user-facing document contract owned by FFHN core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UserFacingDocumentContract {
    /// Stable document id.
    pub id: &'static str,
    /// Human-facing label.
    pub display_label: &'static str,
}

impl UserFacingDocumentContract {
    /// Renders the canonical CLI write-failure text for this document.
    pub fn render_cli_write_error(self) -> String {
        format!(
            "could not write {}",
            self.display_label.to_ascii_lowercase()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rendered_contract_messages_use_detail_only_when_the_template_supports_it() {
        let with_detail = CliHardLimitContract {
            id: "limit",
            operation_id: Some("run"),
            display_label: "Limit",
            summary: "Summary",
            cli_usage_error_template: "duplicate {detail}",
        };
        assert_eq!(
            with_detail.render_cli_usage_error(Some("demo")),
            "duplicate demo"
        );
        assert_eq!(
            with_detail.render_cli_usage_error(None),
            "duplicate {detail}"
        );
        let without_detail = CliHardLimitContract {
            cli_usage_error_template: "fixed",
            ..with_detail
        };
        assert_eq!(
            without_detail.render_cli_usage_error(Some("ignored")),
            "fixed"
        );
        assert_eq!(
            UserFacingDocumentContract {
                id: "doc",
                display_label: "Run Report"
            }
            .render_cli_write_error(),
            "could not write run report"
        );
    }
}

/// Canonical user-facing FFHN CLI contract catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CliContractCatalog {
    /// Registered CLI operations.
    pub operations: &'static [CliOperationContract],
    /// Registered execution modes.
    pub execution_modes: &'static [ExecutionModeContract],
    /// Registered hard limitations.
    pub hard_limits: &'static [CliHardLimitContract],
    /// Registered user-facing document ids.
    pub documents: &'static [UserFacingDocumentContract],
}
