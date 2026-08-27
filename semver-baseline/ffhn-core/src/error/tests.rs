use super::CoreError;

#[test]
fn core_error_keeps_foreign_errors_as_sources_until_the_owned_diagnostic_boundary() {
    assert!(matches!(
        CoreError::contract("bad target"),
        CoreError::Contract(_)
    ));
    assert!(matches!(
        CoreError::internal("broken invariant"),
        CoreError::Internal(_)
    ));
    assert!(matches!(
        CoreError::policy_invariant("exact arithmetic proof"),
        CoreError::PolicyInvariant(_)
    ));
}
