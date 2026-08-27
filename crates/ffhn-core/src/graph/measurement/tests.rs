use super::*;

const SOURCE: &str = r#"
schema_name = "ffhn.source"
schema_version = 1
source_id = "shop"
display_name = "Shop"
enabled = true
escalate_after = 2
[fetch]
engine = "file"
file_path = "/tmp/shop.json"
max_bytes = 1024
[conditional]
enabled = false
[schedule]
interval_ms = 100
min_interval_ms = 50
"#;

const MEASUREMENT: &str = r#"
schema_name = "ffhn.measurement"
schema_version = 1
measurement_id = "price"
display_name = "Price"
enabled = true
escalate_after = 2
declared_type = "decimal"
conditions = []
[projection]
kind = "json_pointer"
pointer = "/price"
[type_params]
locale = "invariant"
"#;

fn source() -> SourceDocument {
    toml::from_str(&SOURCE.replace(
        "/tmp/shop.json",
        &crate::graph::test_support::absolute_file_path("shop.json").replace('\\', "\\\\"),
    ))
    .expect("source")
}

#[test]
fn measurement_value_digest_binds_scalar_contract_but_not_measurement_operational_health() {
    let source = source();
    let base: MeasurementDocument = toml::from_str(MEASUREMENT).expect("measurement");
    let changed_health: MeasurementDocument =
        toml::from_str(&MEASUREMENT.replace("escalate_after = 2", "escalate_after = 9"))
            .expect("health change");
    let changed_projection: MeasurementDocument =
        toml::from_str(&MEASUREMENT.replace("/price", "/other")).expect("projection change");
    assert_eq!(
        base.measurement_value_digest(&source).expect("base MVD"),
        changed_health
            .measurement_value_digest(&source)
            .expect("health MVD")
    );
    assert_ne!(
        base.measurement_value_digest(&source).expect("base MVD"),
        changed_projection
            .measurement_value_digest(&source)
            .expect("projection MVD")
    );
}

#[test]
fn condition_definition_digest_normalizes_typed_and_percentage_thresholds() {
    let decimal_condition = |threshold: &str| {
        MEASUREMENT.replace(
            "conditions = []",
            &format!(
                "[[conditions]]\ncondition_id = \"change\"\n[conditions.predicate]\nkind = \"delta_abs\"\nreference = \"last_accepted_observation\"\nthreshold = {threshold:?}"
            ),
        )
    };
    let left: MeasurementDocument =
        toml::from_str(&decimal_condition("5.0")).expect("left condition");
    let right: MeasurementDocument =
        toml::from_str(&decimal_condition("5.00")).expect("right condition");
    assert_eq!(
        left.condition_definition_digests().expect("left digest"),
        right.condition_definition_digests().expect("right digest")
    );

    let percentage = |threshold: &str| {
        MEASUREMENT.replace(
            "conditions = []",
            &format!(
                "[[conditions]]\ncondition_id = \"change\"\n[conditions.predicate]\nkind = \"delta_pct\"\nreference = \"last_accepted_observation\"\nthreshold = {threshold:?}"
            ),
        )
    };
    let left: MeasurementDocument = toml::from_str(&percentage("5.0")).expect("left percentage");
    let right: MeasurementDocument = toml::from_str(&percentage("5.00")).expect("right percentage");
    assert_eq!(
        left.condition_definition_digests().expect("left digest"),
        right.condition_definition_digests().expect("right digest")
    );
}

#[test]
fn measurement_document_exposes_and_validates_its_complete_owned_contract() {
    let document: MeasurementDocument = toml::from_str(MEASUREMENT).expect("measurement");
    assert_eq!(document.measurement_id().as_str(), "price");
    assert_eq!(document.display_name(), "Price");
    assert!(document.enabled());
    assert_eq!(document.escalate_after(), 2);
    assert!(
        matches!(document.projection(), Projection::JsonPointer { pointer } if pointer == "/price")
    );
    assert_eq!(document.declared_type(), DeclaredType::Decimal);
    assert_eq!(
        document.type_params().locale,
        Some(crate::NumericLocale::Invariant)
    );
    assert!(document.conditions().is_empty());
    assert!(document.outbox().is_none());
    assert!(document.routes().is_empty());

    let base = serde_json::to_value(&document).expect("measurement wire");
    for (pointer, value) in [
        ("/schema_name", serde_json::json!("foreign.measurement")),
        ("/schema_version", serde_json::json!(2)),
        ("/display_name", serde_json::json!(" ")),
        ("/escalate_after", serde_json::json!(0)),
        ("/declared_type", serde_json::json!("text")),
    ] {
        let mut wire = base.clone();
        *wire.pointer_mut(pointer).expect("pointer") = value;
        assert!(
            serde_json::from_value::<MeasurementDocument>(wire).is_err(),
            "{pointer}"
        );
    }

    let duplicate = MEASUREMENT.replace(
        "conditions = []",
        r#"
[[conditions]]
condition_id = "same"
[conditions.predicate]
kind = "changed"
reference = "last_accepted_observation"
[[conditions]]
condition_id = "same"
[conditions.predicate]
kind = "changed"
reference = "fixed_initial_baseline"
"#,
    );
    assert!(toml::from_str::<MeasurementDocument>(&duplicate).is_err());
}

#[test]
fn condition_digests_cover_every_predicate_shape_and_fail_closed_internal_shape() {
    let source = source();
    for predicate in [
        "kind = \"changed\"\nreference = \"last_accepted_observation\"",
        "kind = \"delta_abs\"\nreference = \"last_accepted_observation\"\nthreshold = \"1.00\"",
        "kind = \"delta_pct\"\nreference = \"last_accepted_observation\"\nthreshold = \"1.00\"",
        "kind = \"crosses\"\nthreshold = \"1.00\"\ndirection = \"rising\"",
        "kind = \"lt\"\nthreshold = \"1.00\"",
        "kind = \"gt\"\nthreshold = \"1.00\"",
        "kind = \"band\"\nenter_threshold = \"2.00\"\nexit_threshold = \"1.00\"\ndirection = \"rising\"",
    ] {
        let body = MEASUREMENT.replace(
            "conditions = []",
            &format!(
                "[[conditions]]\ncondition_id = \"condition\"\n[conditions.predicate]\n{predicate}"
            ),
        );
        let measurement: MeasurementDocument = toml::from_str(&body).expect("predicate");
        assert_eq!(
            measurement
                .condition_definition_digests()
                .expect("digest")
                .len(),
            1
        );
        assert_eq!(
            measurement
                .measurement_value_digest(&source)
                .expect("MVD")
                .len(),
            64
        );
    }

    let mut omitted_predicate = serde_json::json!({});
    assert!(set_predicate_field(&mut omitted_predicate, "threshold", "1".to_owned()).is_err());
    let mut omitted_threshold = serde_json::json!({"predicate": {"kind": "lt"}});
    assert!(set_predicate_field(&mut omitted_threshold, "threshold", "1".to_owned()).is_err());
}
