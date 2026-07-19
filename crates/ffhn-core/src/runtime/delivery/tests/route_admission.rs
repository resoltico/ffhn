use super::*;

#[test]
fn materialization_preserves_route_declaration_order_for_bounded_admission() {
    let mut wire = serde_json::to_value(routed_target()).expect("target JSON");
    wire["routes"][0]["route_id"] = serde_json::json!("zeta");
    let first_route = wire["routes"][0].clone();
    let mut second_route = first_route.clone();
    second_route["route_id"] = serde_json::json!("alpha");
    wire["routes"] = serde_json::json!([first_route, second_route]);
    let target: TargetDocument = serde_json::from_value(wire).expect("target shape");
    target.validate().expect("route declaration order is valid");
    let digest = target.contract_digest_sha256().expect("contract digest");
    let state = state_with_condition_facts(&target);

    let records = materialize(
        &target,
        &state,
        &[StagedEventEligibility::OnCondition {
            condition_id: "changed".parse().expect("condition id"),
        }],
        &digest,
    )
    .expect("records");

    assert_eq!(
        records
            .iter()
            .map(|record| record.route_id.as_str())
            .collect::<Vec<_>>(),
        ["zeta", "alpha"]
    );
}
