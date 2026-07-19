use super::support::*;

#[test]
fn policy_staging_rejects_current_observations_outside_the_target_contract() {
    let decimal_target = target(
        "decimal",
        "",
        &one_condition("kind = \"lt\"\nthreshold = \"2\""),
    );
    let integer = observation("integer", "", "1");
    assert!(
        decimal_target
            .stage_policy_run(
                PolicyRunInput::ValidObservation {
                    observation: &integer,
                },
                &context(None, None, None, false),
            )
            .is_err()
    );
}
