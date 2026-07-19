use super::{CoreError, TargetDecodeError};

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

#[test]
fn target_decode_diagnostics_preserve_the_safe_value_only_when_it_is_known() {
    assert_eq!(
        TargetDecodeError::InvalidField {
            field: "projection.kind".to_owned(),
            received: Some("\"unsupported\"".to_owned()),
        }
        .diagnostic_message(),
        "target field \"projection.kind\" has unsupported value \"unsupported\""
    );
    assert_eq!(
        TargetDecodeError::InvalidField {
            field: "target_id".to_owned(),
            received: None,
        }
        .diagnostic_message(),
        "target field \"target_id\" could not be decoded"
    );
}
