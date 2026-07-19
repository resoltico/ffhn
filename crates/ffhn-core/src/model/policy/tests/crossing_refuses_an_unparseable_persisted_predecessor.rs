use super::support::*;

#[test]
fn crossing_refuses_an_unparseable_persisted_predecessor() {
    let target = target(
        "integer",
        "",
        &one_condition("kind = \"crosses\"\nthreshold = \"10\"\ndirection = \"rising\""),
    );
    let current = observation("integer", "", "10");
    let prior = observation("integer", "", "9");
    let mut wire = serde_json::to_value(prior).expect("prior JSON");
    wire["canonical_value"] = serde_json::json!("not-an-integer");
    let prior = serde_json::from_value(wire).expect("structurally valid predecessor");
    assert!(
        target
            .stage_policy_run(
                PolicyRunInput::ValidObservation {
                    observation: &current,
                },
                &context(Some(&prior), None, None, false),
            )
            .is_err()
    );
}
