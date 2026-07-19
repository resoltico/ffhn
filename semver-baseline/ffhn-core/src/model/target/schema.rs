//! Target-document schema vocabulary and public HTML selection contract.

use std::collections::BTreeMap;

use htmlcut_core::interop::v1::{
    self as htmlcut, AttributeName, DomCanonicalization, Output, Plan, PlanStrategy, Rendering,
    Selection,
};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::model::{htmlcut_detail, plain_detail};
use crate::{
    Condition, DeliveryRoute, DiagnosticDetail, DiagnosticKind, DiagnosticOperation,
    HtmlcutFailureDetails, OutboxPolicy, PermanentErrorCode, TargetId,
};

use super::validation::{
    default_follow_redirects, default_max_bytes, default_timeout_ms,
    permanent_code_for_htmlcut_failure,
};

/// Canonical schema name for target documents.
pub const TARGET_SCHEMA_NAME: &str = "ffhn.target";
/// Canonical target-schema version.
pub const TARGET_SCHEMA_VERSION: u32 = 12;

/// One configured source family.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TargetSource {
    /// HTTP or HTTPS source.
    Http {
        /// Absolute source URL.
        source_url: Url,
    },
    /// UTF-8 local-file source.
    File {
        /// Absolute source file path.
        file_path: String,
    },
}

/// Fetch engine paired with a configured source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "engine", rename_all = "snake_case", deny_unknown_fields)]
pub enum FetchConfig {
    /// Fetch an HTTP or HTTPS resource.
    Http {
        /// HTTP method. The current target schema supports GET only.
        #[serde(default)]
        method: HttpMethod,
        /// Request timeout in milliseconds.
        #[serde(default = "default_timeout_ms")]
        timeout_ms: u64,
        /// Maximum accepted response size.
        #[serde(default = "default_max_bytes")]
        max_bytes: usize,
        /// Required request user agent.
        user_agent: String,
        /// Whether redirects are followed.
        #[serde(default = "default_follow_redirects")]
        follow_redirects: bool,
        /// Required Accept header.
        accept: String,
        /// Extra request headers.
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        headers: BTreeMap<String, String>,
    },
    /// Read an absolute local file.
    File {
        /// Maximum accepted file size.
        #[serde(default = "default_max_bytes")]
        max_bytes: usize,
    },
}

/// HTTP method vocabulary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum HttpMethod {
    /// HTTP GET.
    #[default]
    GET,
}

/// Fetch-engine identifier used in reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FetchEngine {
    /// HTTP engine.
    Http,
    /// File engine.
    File,
}

/// Selection settings that FFHN translates into an HTMLCut structured-output plan.
///
/// FFHN owns the final HTMLCut output mode because the structured result is implementation
/// plumbing required to preserve match metadata; target authors select a measurement projection
/// separately through [`Projection`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HtmlSelection {
    strategy: PlanStrategy,
    selection: Selection,
    rendering: Rendering,
    /// Optional transformations for the detached CSS text-comparison clone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dom_canonicalization: Option<DomCanonicalization>,
}

impl HtmlSelection {
    /// Returns the configured public HTMLCut strategy.
    pub const fn strategy(&self) -> &PlanStrategy {
        &self.strategy
    }

    /// Returns the configured public HTMLCut candidate-selection mode.
    pub const fn selection(&self) -> &Selection {
        &self.selection
    }

    /// Returns the configured public HTMLCut text-rendering policy.
    pub const fn rendering(&self) -> &Rendering {
        &self.rendering
    }

    /// Returns the optional policy applied only to HTML text's detached comparison clone.
    pub const fn dom_canonicalization(&self) -> Option<&DomCanonicalization> {
        self.dom_canonicalization.as_ref()
    }

    pub(crate) fn structured_plan(&self) -> Plan {
        let plan = Plan::new(
            self.strategy.clone(),
            self.selection.clone(),
            Output::structured(),
            self.rendering.clone(),
        );
        match &self.dom_canonicalization {
            Some(dom_canonicalization) => {
                plan.with_dom_canonicalization(dom_canonicalization.clone())
            }
            None => plan,
        }
    }

    /// Builds the public direct-attribute plan used solely to obtain HTMLCut's exact rejection
    /// evidence when a target attempts clone canonicalization for an attribute measurement.
    pub(super) fn attribute_plan(&self, name: &AttributeName) -> Plan {
        let plan = Plan::new(
            self.strategy.clone(),
            self.selection.clone(),
            Output::attribute(name.clone()),
            self.rendering.clone(),
        );
        let dom_canonicalization = self
            .dom_canonicalization
            .as_ref()
            .expect("attribute canonicalization plans require a canonicalization policy");
        plan.with_dom_canonicalization(dom_canonicalization.clone())
    }
}

/// A permanent target-contract failure with any public HTMLCut evidence that caused it.
///
/// FFHN-owned target rules have no HTMLCut failure to preserve. HTMLCut preflight failures retain
/// their complete public evidence until it is written into a run report and durable state.
#[derive(Clone, Debug)]
pub(crate) struct PermanentTargetError {
    pub(super) code: PermanentErrorCode,
    pub(super) message: String,
    pub(super) htmlcut_failure: Option<HtmlcutFailureDetails>,
}

impl PermanentTargetError {
    pub(super) fn plain(code: PermanentErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            htmlcut_failure: None,
        }
    }

    pub(super) fn from_htmlcut(error: htmlcut::InteropError) -> Result<Self, crate::CoreError> {
        let htmlcut_failure = HtmlcutFailureDetails::from_interop_error(&error)?;
        Ok(Self {
            code: permanent_code_for_htmlcut_failure(&htmlcut_failure),
            message: error.message.clone(),
            htmlcut_failure: Some(htmlcut_failure),
        })
    }

    /// Returns the stable permanent taxonomy member.
    pub(crate) const fn code(&self) -> PermanentErrorCode {
        self.code
    }

    /// Materializes this target error in FFHN's stable report-error vocabulary.
    pub(crate) fn into_diagnostic_detail(self, path: Option<String>) -> DiagnosticDetail {
        match self.htmlcut_failure {
            Some(failure) => htmlcut_detail(self.message, failure, None),
            None => plain_detail(
                DiagnosticKind::Contract,
                DiagnosticOperation::TargetValidation,
                self.message,
                path,
            ),
        }
    }
}

/// Acquisition projection for one typed FFHN measurement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Projection {
    /// Select one scalar JSON leaf using RFC 6901 JSON Pointer syntax.
    JsonPointer {
        /// JSON Pointer string. The empty pointer selects the root scalar.
        pointer: String,
    },
    /// Extract exactly one selected element's plain DOM descendant text.
    HtmlText {
        /// HTMLCut selection and rendering settings.
        selection: HtmlSelection,
    },
    /// Render exactly one HTMLCut-selected match with structural text decoration.
    HtmlRenderedText {
        /// HTMLCut selection and rendering settings.
        selection: HtmlSelection,
    },
    /// Read one named attribute from exactly one HTMLCut CSS selected-match metadata record.
    HtmlAttribute {
        /// HTMLCut selection and rendering settings.
        selection: HtmlSelection,
        /// Attribute name read from the original public CSS match metadata.
        name: AttributeName,
    },
}

/// One supported semantic value type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredType {
    /// Exact Unicode scalar-sequence text.
    Text,
    /// Signed 128-bit integer.
    Integer,
    /// Exact decimal value.
    Decimal,
    /// Exact decimal value with an explicit currency tag.
    Money,
    /// Semantic version.
    Semver,
    /// Offset-aware date-time.
    Datetime,
}

/// Explicit numeric presentation grammar.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NumericLocale {
    /// ASCII digits, optional leading sign, and dot decimal separator without grouping.
    Invariant,
    /// Comma grouping and dot decimal separator.
    EnUs,
    /// Dot grouping and comma decimal separator.
    DeDe,
}

/// Parameters owned by a declared type.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypeParams {
    /// Exact money currency tag.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    /// Explicit numeric presentation grammar.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<NumericLocale>,
    /// RFC 3339 or a `time` format-description string for date-time input.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Numeric UTC offset used only when the configured date-time format carries no offset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assumed_offset: Option<String>,
}

/// A complete target definition.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TargetDocument {
    pub(super) schema_name: String,
    pub(super) schema_version: u32,
    pub(super) target_id: TargetId,
    pub(super) display_name: String,
    pub(super) enabled: bool,
    pub(super) escalate_after: u32,
    pub(super) target: TargetSource,
    pub(super) fetch: FetchConfig,
    pub(super) projection: Projection,
    pub(super) declared_type: DeclaredType,
    #[serde(default)]
    pub(super) type_params: TypeParams,
    pub(super) conditions: Vec<Condition>,
    #[serde(default)]
    pub(super) outbox: OutboxPolicy,
    #[serde(default)]
    pub(super) routes: Vec<DeliveryRoute>,
}

impl FetchConfig {
    /// Returns the fetch-engine identifier.
    pub const fn engine(&self) -> FetchEngine {
        match self {
            Self::Http { .. } => FetchEngine::Http,
            Self::File { .. } => FetchEngine::File,
        }
    }
}
