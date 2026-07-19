//! Target-contract validation and HTMLCut preflight classification.

use std::path::Path;

use htmlcut_core::interop::v1::{AttributeName, Selection, StrategyKind, prepare_plan};
use time::format_description;

use crate::{CoreError, HtmlcutDiagnosticCode, HtmlcutErrorClass, HtmlcutFailureDetails};

use super::super::PermanentErrorCode;
use super::super::observation::parse::parse_offset;
use super::schema::{
    DeclaredType, FetchConfig, HtmlSelection, PermanentTargetError, Projection, TargetSource,
    TypeParams,
};

pub(super) fn validate_source(source: &TargetSource) -> Result<(), CoreError> {
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

pub(super) fn validate_fetch(source: &TargetSource, fetch: &FetchConfig) -> Result<(), CoreError> {
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

pub(super) fn projection_permanent_error(
    projection: &Projection,
) -> Result<Option<PermanentTargetError>, CoreError> {
    match projection {
        Projection::JsonPointer { pointer } => {
            Ok(validate_json_pointer(pointer).err().map(|message| {
                PermanentTargetError::plain(PermanentErrorCode::InvalidJsonPointer, message)
            }))
        }
        Projection::HtmlText { selection } => {
            if selection.dom_canonicalization().is_some() {
                return html_selection_permanent_error(selection);
            }
            if selection.strategy().kind() != StrategyKind::CssSelector {
                return Ok(Some(PermanentTargetError::plain(
                    PermanentErrorCode::HtmlTextRequiresCssSelector,
                    "projection.kind = html_text requires selection.strategy.kind = css_selector because plain DOM text has no delimiter-fragment projection".to_owned(),
                )));
            }
            html_selection_permanent_error(selection)
        }
        Projection::HtmlRenderedText { selection } => html_selection_permanent_error(selection),
        Projection::HtmlAttribute { selection, name } => {
            if selection.dom_canonicalization().is_some() {
                return html_attribute_canonicalization_error(selection, name).map(Some);
            }
            if selection.strategy().kind() != StrategyKind::CssSelector {
                return Ok(Some(PermanentTargetError::plain(
                    PermanentErrorCode::HtmlAttributeRequiresCssSelector,
                    "projection.kind = html_attribute requires selection.strategy.kind = css_selector"
                        .to_owned(),
                )));
            }
            html_selection_permanent_error(selection)
        }
    }
}

pub(super) fn htmlcut_input_permanent_error(
    source: &TargetSource,
    projection: &Projection,
) -> Option<PermanentTargetError> {
    if !matches!(
        projection,
        Projection::HtmlText { .. }
            | Projection::HtmlRenderedText { .. }
            | Projection::HtmlAttribute { .. }
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

pub(super) fn html_selection_permanent_error(
    selection: &HtmlSelection,
) -> Result<Option<PermanentTargetError>, CoreError> {
    if matches!(selection.selection(), Selection::All) {
        return Ok(Some(PermanentTargetError::plain(
            PermanentErrorCode::HtmlSelectionMustSelectOne,
            "HTML selection must select exactly one match; selection.mode = all is unsupported"
                .to_owned(),
        )));
    }
    prepare_plan(&selection.structured_plan())
        .err()
        .map(|error| PermanentTargetError::from_htmlcut(*error))
        .transpose()
}

pub(super) fn html_attribute_canonicalization_error(
    selection: &HtmlSelection,
    name: &AttributeName,
) -> Result<PermanentTargetError, CoreError> {
    let plan = selection.attribute_plan(name);
    let error = prepare_plan(&plan)
        .expect_err("HTMLCut v12 must reject canonicalization of direct attribute output");
    PermanentTargetError::from_htmlcut(*error)
}

pub(crate) fn permanent_code_for_htmlcut_failure(
    failure: &HtmlcutFailureDetails,
) -> PermanentErrorCode {
    match (failure.error_class(), failure.core_diagnostic_code()) {
        (HtmlcutErrorClass::PlanInvalid, Some(HtmlcutDiagnosticCode::InvalidSelector)) => {
            PermanentErrorCode::HtmlcutInvalidSelector
        }
        (HtmlcutErrorClass::PlanInvalid, Some(HtmlcutDiagnosticCode::InvalidSlicePattern)) => {
            PermanentErrorCode::HtmlcutInvalidSlicePattern
        }
        _ => PermanentErrorCode::HtmlcutPlanInvalid,
    }
}

pub(in crate::model) fn validate_json_pointer(pointer: &str) -> Result<(), String> {
    if !pointer.is_empty() && !pointer.starts_with('/') {
        return Err("projection.pointer must be an RFC 6901 JSON Pointer".to_owned());
    }
    for token in pointer.split('/').skip(1) {
        let mut chars = token.chars();
        while let Some(character) = chars.next() {
            if character == '~' && !matches!(chars.next(), Some('0' | '1')) {
                return Err(
                    "projection.pointer must use only RFC 6901 ~0 and ~1 escapes".to_owned(),
                );
            }
        }
    }
    Ok(())
}

pub(in crate::model) fn validate_type_params(
    kind: DeclaredType,
    params: &TypeParams,
) -> Result<(), CoreError> {
    match kind {
        DeclaredType::Text | DeclaredType::Integer | DeclaredType::Semver
            if params == &TypeParams::default() =>
        {
            Ok(())
        }
        DeclaredType::Text | DeclaredType::Integer | DeclaredType::Semver => Err(
            CoreError::contract("this declared_type does not accept type_params"),
        ),
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
                format_description::parse_borrowed::<3>(format).map_err(|_| {
                    CoreError::contract("type_params.format is not a valid datetime format")
                })?;
            }
            if let Some(offset) = &params.assumed_offset {
                parse_offset(offset).map_err(CoreError::contract)?;
            }
            Ok(())
        }
    }
}

pub(in crate::model) fn require_text(field: &str, value: &str) -> Result<(), CoreError> {
    if value.trim().is_empty() {
        Err(CoreError::contract(format!("{field} must not be empty")))
    } else {
        Ok(())
    }
}
pub(in crate::model) fn validate_max_bytes(value: usize) -> Result<(), CoreError> {
    if !(1_024..=104_857_600).contains(&value) {
        Err(CoreError::contract(
            "fetch.max_bytes must be in 1024..=104857600",
        ))
    } else {
        Ok(())
    }
}
pub(in crate::model) fn default_timeout_ms() -> u64 {
    15_000
}
pub(in crate::model) fn default_max_bytes() -> usize {
    2_000_000
}
pub(in crate::model) fn default_follow_redirects() -> bool {
    true
}
