use super::support::*;

#[test]
fn policy_rejects_observations_outside_the_measurement_contract() {
    let measurement = measurement(
        "decimal",
        "",
        &one_condition("kind = \"lt\"\nthreshold = \"2\""),
    );
    let integer = observation("integer", "", "1");
    let contract = PolicyContract::new(
        measurement.declared_type(),
        measurement.type_params(),
        measurement.conditions(),
    );
    assert!(evaluate_conditions(&contract, &integer, &context(None, None, None, false),).is_err());
}
