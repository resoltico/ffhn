//! Shared typed-measurement configuration vocabulary used by the observation graph.

use htmlcut_core::interop::v1::{
    AttributeName, DomCanonicalization, Output, Plan, PlanStrategy, Rendering, Selection,
};
use serde::{Deserialize, Serialize};
use time::format_description;

use crate::CoreError;

/// Selection settings translated into one validated HTMLCut plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HtmlSelection {
    strategy: PlanStrategy,
    selection: Selection,
    rendering: Rendering,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dom_canonicalization: Option<DomCanonicalization>,
}

impl HtmlSelection {
    /// Returns the configured HTMLCut strategy.
    pub const fn strategy(&self) -> &PlanStrategy {
        &self.strategy
    }

    /// Returns the configured candidate-selection mode.
    pub const fn selection(&self) -> &Selection {
        &self.selection
    }

    /// Returns the configured text-rendering policy.
    pub const fn rendering(&self) -> &Rendering {
        &self.rendering
    }

    /// Returns the optional detached-clone canonicalization policy.
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
        self.dom_canonicalization
            .as_ref()
            .map_or(plan.clone(), |policy| {
                plan.with_dom_canonicalization(policy.clone())
            })
    }
}

/// Acquisition projection for one typed measurement.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Projection {
    /// Select one scalar JSON leaf with RFC 6901 JSON Pointer.
    JsonPointer {
        /// JSON Pointer; empty selects the scalar root.
        pointer: String,
    },
    /// Extract one selected element's plain DOM descendant text.
    HtmlText {
        /// HTML selection and rendering configuration.
        selection: HtmlSelection,
    },
    /// Extract one selected element's semantic rendered text.
    HtmlRenderedText {
        /// HTML selection and rendering configuration.
        selection: HtmlSelection,
    },
    /// Read one named attribute from one selected element.
    HtmlAttribute {
        /// HTML selection and rendering configuration.
        selection: HtmlSelection,
        /// Selected attribute name.
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
    /// Exact decimal with explicit currency.
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
    /// No grouping and dot decimal separator.
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
    /// Numeric presentation grammar.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locale: Option<NumericLocale>,
    /// RFC 3339 or a `time` format description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    /// Numeric UTC offset for formats without one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assumed_offset: Option<String>,
}

pub(crate) fn validate_json_pointer(pointer: &str) -> Result<(), String> {
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

pub(crate) fn validate_type_params(
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
            CoreError::contract("this measurement declared_type does not accept type_params"),
        ),
        DeclaredType::Decimal
            if params.currency.is_none()
                && params.format.is_none()
                && params.assumed_offset.is_none() =>
        {
            Ok(())
        }
        DeclaredType::Decimal => Err(CoreError::contract(
            "decimal measurement accepts only type_params.locale",
        )),
        DeclaredType::Money => {
            let currency = params.currency.as_deref().ok_or_else(|| {
                CoreError::contract("money measurement requires type_params.currency")
            })?;
            if currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_uppercase()) {
                return Err(CoreError::contract(
                    "measurement type_params.currency must be three uppercase letters",
                ));
            }
            if params.format.is_some() || params.assumed_offset.is_some() {
                return Err(CoreError::contract(
                    "money measurement accepts only currency and locale type_params",
                ));
            }
            Ok(())
        }
        DeclaredType::Datetime => {
            if params.currency.is_some() || params.locale.is_some() || params.format.is_none() {
                return Err(CoreError::contract(
                    "datetime measurement requires format and accepts only format plus assumed_offset",
                ));
            }
            let format = params.format.as_deref().expect("checked format");
            if format == "rfc3339" && params.assumed_offset.is_some() {
                return Err(CoreError::contract(
                    "datetime measurement rfc3339 format cannot use assumed_offset",
                ));
            }
            if format != "rfc3339" {
                format_description::parse_borrowed::<3>(format).map_err(|_| {
                    CoreError::contract("type_params.format is not a valid datetime format")
                })?;
            }
            if let Some(offset) = &params.assumed_offset {
                super::observation::parse::parse_offset(offset).map_err(CoreError::contract)?;
            }
            Ok(())
        }
    }
}
