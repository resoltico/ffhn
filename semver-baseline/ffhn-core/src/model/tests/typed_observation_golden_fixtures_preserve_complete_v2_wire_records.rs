use super::support::*;

#[test]
fn typed_observation_golden_fixtures_preserve_complete_v2_wire_records() {
    #[derive(serde::Deserialize)]
    struct Fixture {
        name: String,
        declared_type: String,
        type_params_toml: String,
        raw_token: String,
        expected: serde_json::Value,
    }

    let fixtures: Vec<Fixture> = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/typed-observations.json"
    )))
    .expect("typed observation fixtures");
    for fixture in fixtures {
        let observation = target(&fixture.declared_type, &fixture.type_params_toml)
            .parse_json_scalar_token(fixture.raw_token)
            .expect("valid typed observation");
        assert_eq!(
            serde_json::to_value(observation).expect("observation JSON"),
            fixture.expected,
            "fixture {}",
            fixture.name
        );
    }
}
