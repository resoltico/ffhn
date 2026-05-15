use std::collections::BTreeSet;
use std::path::Path;

use crate::CoreError;

use super::super::schema::{TARGET_SCHEMA_NAME, TARGET_SCHEMA_VERSION};
use super::super::validate::{
    apply_regex_flag, require_non_empty, validate_absolute_file_path, validate_absolute_url,
    validate_identity,
};
use super::super::{CanonicalizerKind, DelimiterMode};
use super::types::{
    CanonicalizerSpec, CompareConfig, FetchConfig, FileFetchConfig, NetworkFetchConfig,
    NotificationAdapter, NotificationEndpoint, NotificationRoute, SelectionConfig, StorageConfig,
    TargetDocument, TargetSource,
};

mod htmlcut;
use htmlcut::validate_htmlcut_selection_contract;

impl TargetDocument {
    /// Validates one target document against the frozen FFHN schema contract.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the schema identity, target id, fetch/source pairing, storage
    /// policy, notification hooks, selection contract, or compare pipeline violates FFHN's frozen
    /// target-document contract.
    pub fn validate(&self) -> Result<(), CoreError> {
        validate_identity(
            &self.schema_name,
            TARGET_SCHEMA_NAME,
            self.schema_version,
            TARGET_SCHEMA_VERSION,
        )?;
        require_non_empty("display_name", &self.display_name)?;
        self.target.validate()?;
        self.fetch.validate_for_source(&self.target)?;
        self.storage.validate()?;
        validate_unique_endpoint_names(&self.notification_endpoints)?;
        validate_unique_route_names(&self.notification_routes)?;
        for endpoint in &self.notification_endpoints {
            endpoint.validate()?;
        }
        for route in &self.notification_routes {
            route.validate()?;
        }
        validate_route_endpoint_links(&self.notification_routes, &self.notification_endpoints)?;

        self.selection.validate()?;
        self.compare.validate()
    }
}

impl TargetSource {
    /// Validates the source discriminator-specific fields.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the configured source URL or file path is not absolute and
    /// valid for the selected target kind.
    pub fn validate(&self) -> Result<(), CoreError> {
        match self {
            TargetSource::Http { source_url } => {
                validate_absolute_url(source_url)?;
            }
            TargetSource::File { file_path } => {
                validate_absolute_file_path(file_path)?;
            }
        }
        Ok(())
    }
}

impl FetchConfig {
    fn validate_for_source(&self, target: &TargetSource) -> Result<(), CoreError> {
        match (target, self) {
            (TargetSource::Http { .. }, FetchConfig::Http(config)) => config.validate(),
            (TargetSource::Http { .. }, FetchConfig::File(_)) => Err(CoreError::contract(
                "http targets require fetch.engine = http",
            )),
            (TargetSource::File { .. }, FetchConfig::File(config)) => config.validate(),
            (TargetSource::File { .. }, FetchConfig::Http(_)) => {
                Err(contract_error("file targets require fetch.engine = file"))
            }
        }
    }
}

impl NetworkFetchConfig {
    fn validate(&self) -> Result<(), CoreError> {
        validate_fetch_max_bytes(self.max_bytes)?;
        if self.timeout_ms < 1_000 || self.timeout_ms > 600_000 {
            return Err(CoreError::contract(
                "fetch.timeout_ms must be in 1000..600000",
            ));
        }
        require_non_empty("fetch.user_agent", &self.user_agent)?;
        require_non_empty("fetch.accept", &self.accept)?;
        for (name, value) in &self.headers {
            require_non_empty("fetch.headers key", name)?;
            require_non_empty("fetch.headers value", value)?;
        }
        Ok(())
    }
}

impl FileFetchConfig {
    fn validate(&self) -> Result<(), CoreError> {
        validate_fetch_max_bytes(self.max_bytes)
    }
}

impl StorageConfig {
    /// Validates one rolling storage policy.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when `history_limit` falls outside FFHN's supported range.
    pub fn validate(&self) -> Result<(), CoreError> {
        if !(1..=256).contains(&self.history_limit) {
            return Err(CoreError::contract(
                "storage.history_limit must be in 1..=256",
            ));
        }
        Ok(())
    }
}

impl NotificationRoute {
    /// Validates one notification route.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the route label is empty, when the trigger list is empty or
    /// duplicated, or when the endpoint reference is empty.
    pub fn validate(&self) -> Result<(), CoreError> {
        require_non_empty("notification_routes.name", &self.name)?;
        if self.on.is_empty() {
            return Err(contract_error(
                "notification_routes.on must list at least one run outcome",
            ));
        }
        let mut seen = BTreeSet::new();
        for outcome in &self.on {
            if !seen.insert(outcome.as_str()) {
                return Err(contract_error(
                    "notification_routes.on values must be unique",
                ));
            }
        }
        require_non_empty("notification_routes.endpoint", &self.endpoint)
    }
}

impl NotificationEndpoint {
    pub(crate) fn validate(&self) -> Result<(), CoreError> {
        require_non_empty("notification_endpoints.name", &self.name)?;
        self.adapter.validate()
    }
}

impl NotificationAdapter {
    fn validate(&self) -> Result<(), CoreError> {
        match self {
            Self::ProcessStdin {
                program,
                args,
                timeout_ms,
            } => {
                require_non_empty("notification_endpoints.adapter.program", program)?;
                if !Path::new(program).is_absolute() {
                    return Err(CoreError::contract(
                        "notification_endpoints.adapter.program must be an absolute path",
                    ));
                }
                for arg in args {
                    require_non_empty("notification_endpoints.adapter.args entry", arg)?;
                }
                if *timeout_ms < 100 || *timeout_ms > 60_000 {
                    return Err(contract_error(
                        "notification_endpoints.adapter.timeout_ms must be in 100..60000",
                    ));
                }
                Ok(())
            }
        }
    }
}

impl SelectionConfig {
    /// Validates one target selection section.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when the configured selection fields are empty, incompatible with the
    /// chosen selection kind or delimiter mode, or rejected by FFHN's HTMLCut plan boundary.
    pub fn validate(&self) -> Result<(), CoreError> {
        match self {
            SelectionConfig::CssSelector { selector, .. } => {
                require_non_empty("selection.selector", selector)?;
            }
            SelectionConfig::DelimiterPair {
                start,
                end,
                mode,
                flags,
                ..
            } => {
                require_non_empty("selection.start", start)?;
                require_non_empty("selection.end", end)?;
                if *mode == DelimiterMode::Literal && !flags.is_empty() {
                    return Err(CoreError::contract(
                        "selection.flags are forbidden for literal delimiter mode",
                    ));
                }
            }
        }

        validate_htmlcut_selection_contract(self)
    }
}

impl CompareConfig {
    /// Validates one compare section.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when any canonicalizer entry violates FFHN's compare-pipeline
    /// contract.
    pub fn validate(&self) -> Result<(), CoreError> {
        for canonicalizer in &self.canonicalization {
            canonicalizer.validate()?;
        }
        Ok(())
    }
}

impl CanonicalizerSpec {
    /// Validates one canonicalizer entry.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError`] when `pattern` or `flags` are present for non-regex canonicalizers,
    /// or when a `strip_regex` canonicalizer is missing a pattern or carries an invalid regex.
    pub fn validate(&self) -> Result<(), CoreError> {
        match self.kind {
            CanonicalizerKind::Trim
            | CanonicalizerKind::CollapseWhitespace
            | CanonicalizerKind::NormalizeNewlines
            | CanonicalizerKind::Lowercase => {
                if self.pattern.is_some() {
                    return Err(CoreError::contract(
                        "canonicalizer pattern/flags are only valid for strip_regex",
                    ));
                }
                if !self.flags.is_empty() {
                    return Err(CoreError::contract(
                        "canonicalizer pattern/flags are only valid for strip_regex",
                    ));
                }
            }
            CanonicalizerKind::StripRegex => {
                let pattern = self.pattern.as_deref().ok_or_else(|| {
                    CoreError::contract("strip_regex canonicalizer requires pattern")
                })?;
                require_non_empty("compare.canonicalization.pattern", pattern)?;
                let mut builder = regex::RegexBuilder::new(pattern);
                builder.unicode(true);
                for flag in &self.flags {
                    apply_regex_flag(flag, &mut builder);
                }
                builder.build().map_err(|error| {
                    CoreError::contract(format!("invalid strip_regex pattern: {error}"))
                })?;
            }
        }
        Ok(())
    }
}

fn validate_unique_route_names(routes: &[NotificationRoute]) -> Result<(), CoreError> {
    let mut names = BTreeSet::new();
    for route in routes {
        if !names.insert(route.name.as_str()) {
            return Err(CoreError::contract(
                "notification_routes.name values must be unique",
            ));
        }
    }
    Ok(())
}

fn validate_unique_endpoint_names(endpoints: &[NotificationEndpoint]) -> Result<(), CoreError> {
    let mut names = BTreeSet::new();
    for endpoint in endpoints {
        if !names.insert(endpoint.name.as_str()) {
            return Err(CoreError::contract(
                "notification_endpoints.name values must be unique",
            ));
        }
    }
    Ok(())
}

fn validate_route_endpoint_links(
    routes: &[NotificationRoute],
    endpoints: &[NotificationEndpoint],
) -> Result<(), CoreError> {
    let endpoint_names = endpoints
        .iter()
        .map(|endpoint| endpoint.name.as_str())
        .collect::<BTreeSet<_>>();
    for route in routes {
        if !endpoint_names.contains(route.endpoint.as_str()) {
            return Err(CoreError::contract(
                "notification_routes.endpoint must reference notification_endpoints.name",
            ));
        }
    }
    Ok(())
}

fn validate_fetch_max_bytes(max_bytes: usize) -> Result<(), CoreError> {
    if !(1_024..=104_857_600).contains(&max_bytes) {
        return Err(CoreError::contract(
            "fetch.max_bytes must be in 1024..104857600",
        ));
    }
    Ok(())
}

fn contract_error(message: &'static str) -> CoreError {
    CoreError::contract(message)
}
