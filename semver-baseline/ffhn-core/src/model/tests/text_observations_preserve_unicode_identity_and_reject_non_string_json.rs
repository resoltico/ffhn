use super::support::*;
use crate::CoreError;

#[test]
fn text_observations_preserve_unicode_identity_and_reject_non_string_json() {
    let text_target = target("text", "");
    assert_eq!(text_target.declared_type(), DeclaredType::Text);

    let whitespace_and_case = text_target
        .parse_json_scalar_token(r#""  In StOcK  ""#.to_owned())
        .expect("text JSON string");
    assert_eq!(whitespace_and_case.raw_selected(), r#""  In StOcK  ""#);
    assert_eq!(whitespace_and_case.canonical_value(), "  In StOcK  ");
    let round_tripped: Observation = serde_json::from_value(
        serde_json::to_value(&whitespace_and_case).expect("text observation JSON"),
    )
    .expect("round-tripped text observation");
    assert_eq!(round_tripped, whitespace_and_case);

    let escaped = text_target
        .parse_json_scalar_token(r#""\u00e9""#.to_owned())
        .expect("escaped text JSON string");
    let literal = text_target
        .parse_json_scalar_token(r#""é""#.to_owned())
        .expect("literal text JSON string");
    let decomposed = text_target
        .parse_json_scalar_token(r#""e\u0301""#.to_owned())
        .expect("decomposed text JSON string");
    assert_eq!(escaped.canonical_value(), "é");
    assert_eq!(escaped.canonical_value(), literal.canonical_value());
    assert_ne!(literal.canonical_value(), decomposed.canonical_value());

    for raw_token in ["1", "true", "null"] {
        let error = text_target
            .parse_json_scalar_token(raw_token.to_owned())
            .expect_err("text rejects non-string JSON scalar");
        assert_eq!(error.kind(), DiagnosticKind::ValueUnparseable);
        assert_eq!(error.message(), "text declared_type requires a JSON string");
    }

    let unexpected_params = mutate_target(&text_target, |wire| {
        wire["type_params"] = serde_json::json!({"locale": "en_us"});
    });
    assert!(matches!(
        unexpected_params.validate(),
        Err(CoreError::Contract(message)) if message == "this declared_type does not accept type_params"
    ));

    let accepted = text_target
        .parse_json_scalar_token(r#""In Stock""#.to_owned())
        .expect("accepted text observation");
    let mut state = StateDocument::new(
        TargetId::new("demo").expect("target id"),
        text_target.contract_digest_sha256().expect("digest"),
    );
    let staged = text_target
        .stage_policy_run(
            PolicyRunInput::ValidObservation {
                observation: &accepted,
            },
            &state.condition_contexts(&text_target),
        )
        .expect("text policy stage");
    state
        .apply_valid_observation(
            &text_target,
            accepted,
            staged
                .condition_evaluations()
                .expect("condition evaluations"),
            "2026-07-19T00:00:00Z",
        )
        .expect("persist text observation");

    for raw_token in ["1", "true", "null"] {
        let mut wire = serde_json::to_value(&state).expect("state JSON");
        for observation_key in ["accepted_observation", "fixed_initial_baseline"] {
            wire[observation_key]["raw_selected"] = serde_json::json!(raw_token);
            wire[observation_key]["comparison_projection"] = serde_json::json!(raw_token);
            wire[observation_key]["canonical_value"] = serde_json::json!(raw_token);
        }
        assert!(
            serde_json::from_value::<StateDocument>(wire).is_err(),
            "persisted text observation rejects {raw_token}"
        );
    }
}
