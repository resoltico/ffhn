use std::collections::BTreeMap;
use std::path::Path;

use htmlcut_core::interop::v1::{
    self as htmlcut, AttributeName, DomCanonicalization, ErrorCode,
    HTMLCUT_EXTRACTION_SEMANTICS_VERSION, Output, Plan, PlanStrategy, Rendering, Selection,
    StrategyKind, prepare_plan,
};
use serde::{Deserialize, Serialize};
use time::format_description;
use url::Url;

use crate::CoreError;

use super::delivery::validate_routes;
use super::observation::{
    HtmlObservationInput, PARSER_GRAMMAR_VERSION, PARSER_ID, parse_html_projection,
    parse_json_scalar_token, parse_offset,
};
use super::policy::{
    Condition, ConditionContext, ConditionId, PolicyRunInput, StagedPolicyRun, stage_policy_run,
    validate_conditions,
};
use super::{
    DeliveryRoute, HtmlcutFailureDetails, OutboxPolicy, PermanentErrorCode, ProcessErrorDetail,
    ProcessErrorKind, RouteFamily, RouteId, TargetId,
};

/// Canonical schema name for target documents.
pub const TARGET_SCHEMA_NAME: &str = "ffhn.target";
/// Canonical target-schema version.
pub const TARGET_SCHEMA_VERSION: u32 = 9;

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
    fn attribute_plan(&self, name: &AttributeName) -> Plan {
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
    code: PermanentErrorCode,
    message: String,
    htmlcut_failure: Option<HtmlcutFailureDetails>,
}

impl PermanentTargetError {
    fn plain(code: PermanentErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            htmlcut_failure: None,
        }
    }

    fn from_htmlcut(error: htmlcut::InteropError) -> Self {
        Self {
            code: permanent_code_for_htmlcut_error(&error),
            message: error.message.clone(),
            htmlcut_failure: Some(HtmlcutFailureDetails::from_interop_error(&error)),
        }
    }

    /// Returns the stable permanent taxonomy member.
    pub(crate) const fn code(&self) -> PermanentErrorCode {
        self.code
    }

    /// Materializes this target error in FFHN's stable report-error vocabulary.
    pub(crate) fn into_process_error_detail(self, path: Option<String>) -> ProcessErrorDetail {
        let detail = ProcessErrorDetail::new(ProcessErrorKind::Contract, self.message, path);
        match self.htmlcut_failure {
            Some(failure) => detail.with_htmlcut_failure(failure),
            None => detail,
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
    /// Render exactly one HTMLCut-selected match as text.
    HtmlText {
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
    schema_name: String,
    schema_version: u32,
    target_id: TargetId,
    display_name: String,
    enabled: bool,
    escalate_after: u32,
    target: TargetSource,
    fetch: FetchConfig,
    projection: Projection,
    declared_type: DeclaredType,
    #[serde(default)]
    type_params: TypeParams,
    conditions: Vec<Condition>,
    #[serde(default)]
    outbox: OutboxPolicy,
    #[serde(default)]
    routes: Vec<DeliveryRoute>,
}

impl TargetDocument {
    /// Validates the target as one complete measurement contract.
    pub fn validate(&self) -> Result<(), CoreError> {
        self.validate_without_projection()?;
        if let Some(error) = self.permanent_error() {
            return Err(CoreError::contract(error.message));
        }
        Ok(())
    }

    /// Validates every target requirement other than projection syntax.
    ///
    /// Runtime classification uses this boundary to persist a permanent invalid-JSON-Pointer
    /// episode while still refusing all other malformed target contracts before execution.
    pub(crate) fn validate_without_projection(&self) -> Result<(), CoreError> {
        if self.schema_name != TARGET_SCHEMA_NAME || self.schema_version != TARGET_SCHEMA_VERSION {
            return Err(CoreError::contract(format!(
                "target must use schema_name = {TARGET_SCHEMA_NAME:?} and schema_version = {TARGET_SCHEMA_VERSION}"
            )));
        }
        require_text("display_name", &self.display_name)?;
        if self.escalate_after == 0 {
            return Err(CoreError::contract("escalate_after must be positive"));
        }
        validate_source(&self.target)?;
        validate_fetch(&self.target, &self.fetch)?;
        validate_type_params(self.declared_type, &self.type_params)?;
        validate_conditions(self.declared_type, &self.type_params, &self.conditions)?;
        self.outbox.validate()?;
        validate_routes(&self.routes)
    }

    /// Returns the stable permanent projection error and its public diagnostic when present.
    pub(crate) fn permanent_error(&self) -> Option<PermanentTargetError> {
        htmlcut_input_permanent_error(&self.target, &self.projection)
            .or_else(|| projection_permanent_error(&self.projection))
    }

    /// Returns whether a persisted observation belongs to this target's type contract.
    pub(crate) fn observation_matches(&self, observation: &super::Observation) -> bool {
        observation.declared_type_for_policy() == self.declared_type
            && observation.type_params_for_policy() == &self.type_params
    }

    /// Returns the configured target id.
    pub fn target_id(&self) -> &str {
        self.target_id.as_str()
    }
    /// Returns the display name.
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    /// Returns whether live runs are enabled.
    pub const fn enabled(&self) -> bool {
        self.enabled
    }
    /// Returns the number of consecutive source-suspect runs that reaches escalation.
    pub const fn escalate_after(&self) -> u32 {
        self.escalate_after
    }
    /// Returns the configured source.
    pub const fn source(&self) -> &TargetSource {
        &self.target
    }
    /// Returns the fetch policy.
    pub const fn fetch(&self) -> &FetchConfig {
        &self.fetch
    }
    /// Returns the acquisition projection.
    pub const fn projection(&self) -> &Projection {
        &self.projection
    }
    /// Returns the declared type.
    pub const fn declared_type(&self) -> DeclaredType {
        self.declared_type
    }
    /// Returns the declared type parameters.
    pub const fn type_params(&self) -> &TypeParams {
        &self.type_params
    }
    /// Returns all named conditions in canonical identifier order.
    pub fn conditions(&self) -> &[Condition] {
        &self.conditions
    }
    /// Returns the operational outbox policy, which does not affect measurement identity.
    pub const fn outbox(&self) -> &OutboxPolicy {
        &self.outbox
    }
    /// Returns configured delivery routes in stable route-identifier order.
    pub fn routes(&self) -> &[DeliveryRoute] {
        &self.routes
    }

    pub(crate) fn routes_for(&self, family: RouteFamily) -> impl Iterator<Item = &DeliveryRoute> {
        self.routes
            .iter()
            .filter(move |route| route.route_family() == family)
    }

    pub(crate) fn route(&self, id: &RouteId) -> Option<&DeliveryRoute> {
        self.routes.iter().find(|route| route.id() == id)
    }

    pub(crate) fn condition(&self, id: &ConditionId) -> Option<&Condition> {
        self.conditions
            .iter()
            .find(|condition| condition.id() == id)
    }

    /// Stages one classified policy branch without persisting state or delivering events.
    ///
    /// The target and current observation are validated, and the observation must use this
    /// target's declared type and type parameters. Contexts are keyed by the stable typed
    /// [`ConditionId`] and may contain only conditions declared by this target. Failure inputs
    /// carry the state-owned episode transition that determines whether an immediate `on_run`
    /// event is eligible. A later temporal coordinator owns persistence of the staged active
    /// state and references, while M4 materializes these eligibilities into durable delivery.
    pub fn stage_policy_run<'a>(
        &self,
        input: PolicyRunInput<'a>,
        contexts: &BTreeMap<ConditionId, ConditionContext<'a>>,
    ) -> Result<StagedPolicyRun, CoreError> {
        stage_policy_run(self, input, contexts)
    }

    /// Computes the source-kind-specific measurement contract digest.
    pub fn contract_digest_sha256(&self) -> Result<String, CoreError> {
        #[derive(Serialize)]
        struct JsonContract<'a> {
            source_kind: &'static str,
            target: &'a TargetSource,
            fetch: &'a FetchConfig,
            projection: &'a Projection,
            declared_type: DeclaredType,
            parser_id: &'static str,
            parser_grammar_version: u32,
            type_params: &'a TypeParams,
            conditions: &'a [Condition],
            escalate_after: u32,
        }
        #[derive(Serialize)]
        struct HtmlContract<'a> {
            source_kind: &'static str,
            target: &'a TargetSource,
            fetch: &'a FetchConfig,
            projection: &'a Projection,
            declared_type: DeclaredType,
            parser_id: &'static str,
            parser_grammar_version: u32,
            type_params: &'a TypeParams,
            conditions: &'a [Condition],
            escalate_after: u32,
            htmlcut_extraction_semantics_version: u32,
        }
        match self.projection {
            Projection::JsonPointer { .. } => crate::stable_json::stable_digest(&JsonContract {
                source_kind: "json_pointer",
                target: &self.target,
                fetch: &self.fetch,
                projection: &self.projection,
                declared_type: self.declared_type,
                parser_id: PARSER_ID,
                parser_grammar_version: PARSER_GRAMMAR_VERSION,
                type_params: &self.type_params,
                conditions: &self.conditions,
                escalate_after: self.escalate_after,
            }),
            Projection::HtmlText { .. } => crate::stable_json::stable_digest(&HtmlContract {
                source_kind: "html_text",
                target: &self.target,
                fetch: &self.fetch,
                projection: &self.projection,
                declared_type: self.declared_type,
                parser_id: PARSER_ID,
                parser_grammar_version: PARSER_GRAMMAR_VERSION,
                type_params: &self.type_params,
                conditions: &self.conditions,
                escalate_after: self.escalate_after,
                htmlcut_extraction_semantics_version: HTMLCUT_EXTRACTION_SEMANTICS_VERSION,
            }),
            Projection::HtmlAttribute { .. } => crate::stable_json::stable_digest(&HtmlContract {
                source_kind: "html_attribute",
                target: &self.target,
                fetch: &self.fetch,
                projection: &self.projection,
                declared_type: self.declared_type,
                parser_id: PARSER_ID,
                parser_grammar_version: PARSER_GRAMMAR_VERSION,
                type_params: &self.type_params,
                conditions: &self.conditions,
                escalate_after: self.escalate_after,
                htmlcut_extraction_semantics_version: HTMLCUT_EXTRACTION_SEMANTICS_VERSION,
            }),
        }
    }

    /// Parses one raw JSON scalar token into a valid persisted observation.
    pub(crate) fn parse_json_scalar_token(
        &self,
        raw_selected: String,
    ) -> Result<super::Observation, super::ProcessErrorDetail> {
        parse_json_scalar_token(self, raw_selected)
    }

    /// Parses one HTMLCut projection into a valid persisted observation.
    pub(crate) fn parse_html_projection(
        &self,
        input: HtmlObservationInput,
    ) -> Result<super::Observation, super::ProcessErrorDetail> {
        parse_html_projection(self, input)
    }
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

fn validate_source(source: &TargetSource) -> Result<(), CoreError> {
    match source {
        TargetSource::Http { source_url } if matches!(source_url.scheme(), "http" | "https") => {
            Ok(())
        }
        TargetSource::Http { .. } => Err(CoreError::contract(
            "target.source_url must use http or https",
        )),
        TargetSource::File { file_path } if Path::new(file_path).is_absolute() => Ok(()),
        TargetSource::File { .. } => Err(CoreError::contract("target.file_path must be absolute")),
    }
}

fn validate_fetch(source: &TargetSource, fetch: &FetchConfig) -> Result<(), CoreError> {
    match (source, fetch) {
        (
            TargetSource::Http { .. },
            FetchConfig::Http {
                timeout_ms,
                max_bytes,
                user_agent,
                accept,
                headers,
                ..
            },
        ) => {
            validate_max_bytes(*max_bytes)?;
            if !(1_000..=600_000).contains(timeout_ms) {
                return Err(CoreError::contract(
                    "fetch.timeout_ms must be in 1000..=600000",
                ));
            }
            require_text("fetch.user_agent", user_agent)?;
            require_text("fetch.accept", accept)?;
            for (name, value) in headers {
                require_text("fetch.headers key", name)?;
                require_text("fetch.headers value", value)?;
            }
            Ok(())
        }
        (TargetSource::File { .. }, FetchConfig::File { max_bytes }) => {
            validate_max_bytes(*max_bytes)
        }
        _ => Err(CoreError::contract(
            "target source and fetch.engine must agree",
        )),
    }
}

fn projection_permanent_error(projection: &Projection) -> Option<PermanentTargetError> {
    match projection {
        Projection::JsonPointer { pointer } => validate_json_pointer(pointer).err().map(|error| {
            PermanentTargetError::plain(PermanentErrorCode::InvalidJsonPointer, error.to_string())
        }),
        Projection::HtmlText { selection } => html_selection_permanent_error(selection),
        Projection::HtmlAttribute { selection, name } => {
            if selection.dom_canonicalization().is_some() {
                return Some(html_attribute_canonicalization_error(selection, name));
            }
            if selection.strategy().kind() != StrategyKind::CssSelector {
                return Some(PermanentTargetError::plain(
                    PermanentErrorCode::HtmlAttributeRequiresCssSelector,
                    "projection.kind = html_attribute requires selection.strategy.kind = css_selector"
                        .to_owned(),
                ));
            }
            html_selection_permanent_error(selection)
        }
    }
}

fn htmlcut_input_permanent_error(
    source: &TargetSource,
    projection: &Projection,
) -> Option<PermanentTargetError> {
    if !matches!(
        projection,
        Projection::HtmlText { .. } | Projection::HtmlAttribute { .. }
    ) {
        return None;
    }
    let TargetSource::Http { source_url } = source else {
        return None;
    };
    if source_url.username().is_empty() && source_url.password().is_none() {
        return None;
    }
    Some(PermanentTargetError::plain(
        PermanentErrorCode::HtmlcutInputInvalid,
        "HTML projections require target.source_url without URL userinfo because HTMLCut cannot use credentials in its input base URL".to_owned(),
    ))
}

fn html_selection_permanent_error(selection: &HtmlSelection) -> Option<PermanentTargetError> {
    if matches!(selection.selection(), Selection::All) {
        return Some(PermanentTargetError::plain(
            PermanentErrorCode::HtmlSelectionMustSelectOne,
            "HTML selection must select exactly one match; selection.mode = all is unsupported"
                .to_owned(),
        ));
    }
    prepare_plan(&selection.structured_plan())
        .err()
        .map(|error| PermanentTargetError::from_htmlcut(*error))
}

fn html_attribute_canonicalization_error(
    selection: &HtmlSelection,
    name: &AttributeName,
) -> PermanentTargetError {
    let plan = selection.attribute_plan(name);
    let error = prepare_plan(&plan)
        .expect_err("HTMLCut v11 must reject canonicalization of direct attribute output");
    PermanentTargetError::from_htmlcut(*error)
}

pub(crate) fn permanent_code_for_htmlcut_error(
    error: &htmlcut::InteropError,
) -> PermanentErrorCode {
    match error
        .details
        .get("core_diagnostic_code")
        .and_then(serde_json::Value::as_str)
    {
        Some("INVALID_SELECTOR") | Some("invalid_selector") => {
            PermanentErrorCode::HtmlcutInvalidSelector
        }
        Some("INVALID_SLICE_PATTERN") | Some("invalid_slice_pattern") => {
            PermanentErrorCode::HtmlcutInvalidSlicePattern
        }
        Some("UNSUPPORTED_VALUE_TYPE") | Some("unsupported_value_type") => {
            PermanentErrorCode::HtmlcutUnsupportedValueType
        }
        _ => match error.error_code {
            ErrorCode::PlanInvalid => PermanentErrorCode::HtmlcutPlanInvalid,
            ErrorCode::NoMatch
            | ErrorCode::AmbiguousMatch
            | ErrorCode::MissingAttribute
            | ErrorCode::InternalError => PermanentErrorCode::HtmlcutPlanInvalid,
        },
    }
}

pub(super) fn validate_json_pointer(pointer: &str) -> Result<(), CoreError> {
    if !pointer.is_empty() && !pointer.starts_with('/') {
        return Err(CoreError::contract(
            "projection.pointer must be an RFC 6901 JSON Pointer",
        ));
    }
    for token in pointer.split('/').skip(1) {
        let mut chars = token.chars();
        while let Some(character) = chars.next() {
            if character == '~' && !matches!(chars.next(), Some('0' | '1')) {
                return Err(CoreError::contract(
                    "projection.pointer must use only RFC 6901 ~0 and ~1 escapes",
                ));
            }
        }
    }
    Ok(())
}

pub(super) fn validate_type_params(
    kind: DeclaredType,
    params: &TypeParams,
) -> Result<(), CoreError> {
    match kind {
        DeclaredType::Integer | DeclaredType::Semver if params == &TypeParams::default() => Ok(()),
        DeclaredType::Integer | DeclaredType::Semver => Err(CoreError::contract(
            "this declared_type does not accept type_params",
        )),
        DeclaredType::Decimal
            if params.currency.is_none()
                && params.format.is_none()
                && params.assumed_offset.is_none() =>
        {
            Ok(())
        }
        DeclaredType::Decimal => Err(CoreError::contract(
            "decimal accepts only type_params.locale",
        )),
        DeclaredType::Money => {
            let currency = params
                .currency
                .as_deref()
                .ok_or_else(|| CoreError::contract("money requires type_params.currency"))?;
            if currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_uppercase()) {
                return Err(CoreError::contract(
                    "type_params.currency must be a three-letter uppercase code",
                ));
            }
            if params.format.is_some() || params.assumed_offset.is_some() {
                return Err(CoreError::contract(
                    "money accepts only currency and locale type_params",
                ));
            }
            Ok(())
        }
        DeclaredType::Datetime => {
            let format = params
                .format
                .as_deref()
                .ok_or_else(|| CoreError::contract("datetime requires type_params.format"))?;
            require_text("type_params.format", format)?;
            if params.currency.is_some() || params.locale.is_some() {
                return Err(CoreError::contract(
                    "datetime accepts only format and assumed_offset type_params",
                ));
            }
            if format == "rfc3339" && params.assumed_offset.is_some() {
                return Err(CoreError::contract(
                    "datetime format rfc3339 already requires an explicit offset and cannot use assumed_offset",
                ));
            }
            if format != "rfc3339" {
                format_description::parse_borrowed::<3>(format).map_err(|error| {
                    CoreError::contract(format!("invalid datetime format: {error}"))
                })?;
            }
            if let Some(offset) = &params.assumed_offset {
                parse_offset(offset).map_err(CoreError::contract)?;
            }
            Ok(())
        }
    }
}

pub(super) fn require_text(field: &str, value: &str) -> Result<(), CoreError> {
    if value.trim().is_empty() {
        Err(CoreError::contract(format!("{field} must not be empty")))
    } else {
        Ok(())
    }
}
pub(super) fn validate_max_bytes(value: usize) -> Result<(), CoreError> {
    if !(1_024..=104_857_600).contains(&value) {
        Err(CoreError::contract(
            "fetch.max_bytes must be in 1024..=104857600",
        ))
    } else {
        Ok(())
    }
}
pub(super) fn default_timeout_ms() -> u64 {
    15_000
}
pub(super) fn default_max_bytes() -> usize {
    2_000_000
}
pub(super) fn default_follow_redirects() -> bool {
    true
}
