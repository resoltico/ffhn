use super::super::HttpMethod;

pub(super) const fn default_fetch_method() -> HttpMethod {
    HttpMethod::GET
}

pub(super) const fn default_fetch_timeout_ms() -> u64 {
    15_000
}

pub(super) const fn default_fetch_max_bytes() -> usize {
    2_000_000
}

pub(super) const fn default_follow_redirects() -> bool {
    true
}

pub(super) const fn default_history_limit() -> usize {
    10
}

pub(super) fn default_notification_shell() -> String {
    "/bin/sh".to_owned()
}

pub(super) const fn default_notification_timeout_ms() -> u64 {
    5_000
}
