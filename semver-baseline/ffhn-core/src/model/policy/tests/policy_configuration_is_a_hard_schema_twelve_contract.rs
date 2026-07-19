use super::support::*;

#[test]
fn policy_configuration_is_a_hard_schema_twelve_contract() {
    let source_path = crate::test_support::absolute_file_path("source.json");
    let without_conditions = toml::from_str::<TargetDocument>(&format!(
        "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\n\n[target]\nkind = \"file\"\nfile_path = {source_path:?}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n",
    ));
    assert!(without_conditions.is_err());

    let mut wrong_version = target("integer", "", "conditions = []");
    let mut wire = serde_json::to_value(&wrong_version).expect("wire");
    wire["schema_version"] = serde_json::json!(5);
    wrong_version = serde_json::from_value(wire).expect("structural target");
    assert!(wrong_version.validate().is_err());
    assert!(
        wrong_version
            .stage_policy_run(
                PolicyRunInput::PermanentContractError {
                    error_code: PermanentErrorCode::InvalidJsonPointer,
                    episode_began: false,
                },
                &BTreeMap::new(),
            )
            .is_err()
    );

    let declaration_order = toml::from_str::<TargetDocument>(&target_toml(
        "integer",
        "",
        "[[conditions]]\ncondition_id = \"zeta\"\n[conditions.predicate]\nkind = \"lt\"\nthreshold = \"1\"\n\n[[conditions]]\ncondition_id = \"alpha\"\n[conditions.predicate]\nkind = \"gt\"\nthreshold = \"0\"",
    ))
    .expect("structural target");
    declaration_order
        .validate()
        .expect("declaration order is operational priority");
    assert_eq!(
        declaration_order
            .conditions()
            .iter()
            .map(Condition::condition_id)
            .collect::<Vec<_>>(),
        ["zeta", "alpha"]
    );

    let duplicate = toml::from_str::<TargetDocument>(&target_toml(
        "integer",
        "",
        "[[conditions]]\ncondition_id = \"same\"\n[conditions.predicate]\nkind = \"lt\"\nthreshold = \"1\"\n\n[[conditions]]\ncondition_id = \"same\"\n[conditions.predicate]\nkind = \"gt\"\nthreshold = \"0\"",
    ))
    .expect("structural target");
    assert!(duplicate.validate().is_err());

    let ordered = target(
        "integer",
        "",
        "[[conditions]]\ncondition_id = \"alpha\"\n[conditions.predicate]\nkind = \"lt\"\nthreshold = \"1\"\n\n[[conditions]]\ncondition_id = \"beta\"\n[conditions.predicate]\nkind = \"gt\"\nthreshold = \"0\"",
    );
    assert_eq!(
        ordered
            .conditions()
            .iter()
            .map(Condition::condition_id)
            .collect::<Vec<_>>(),
        ["alpha", "beta"]
    );
}
