pub(crate) const TOOL_NAME: &str = "ffhn";
pub(crate) const FFHN_VERSION: &str = env!("CARGO_PKG_VERSION");
pub(crate) const FFHN_DESCRIPTION: &str = env!("CARGO_PKG_DESCRIPTION");

pub(crate) fn version_banner() -> String {
    format!("{TOOL_NAME} {FFHN_VERSION}\n{FFHN_DESCRIPTION}")
}
