use super::support::*;

#[test]
fn contract_digest_binds_every_measurement_and_policy_input() {
    let document = target("decimal", "");
    let digest = document.contract_digest_sha256().expect("base digest");

    let changed = mutate_target(&document, |wire| {
        wire["target"] = serde_json::json!({
            "kind": "file",
            "file_path": crate::test_support::absolute_file_path("other-source.json"),
        });
    });
    assert_ne!(
        changed.contract_digest_sha256().expect("source digest"),
        digest
    );

    let changed = mutate_target(&document, |wire| {
        wire["fetch"] = serde_json::json!({"engine": "file", "max_bytes": 4_096});
    });
    assert_ne!(
        changed.contract_digest_sha256().expect("fetch digest"),
        digest
    );

    let changed = mutate_target(&document, |wire| {
        wire["projection"] = serde_json::json!({"kind": "json_pointer", "pointer": "/other"});
    });
    assert_ne!(
        changed.contract_digest_sha256().expect("projection digest"),
        digest
    );

    let changed = mutate_target(&document, |wire| {
        wire["type_params"] = serde_json::json!({"locale": "en_us"});
    });
    assert_ne!(
        changed
            .contract_digest_sha256()
            .expect("type parameter digest"),
        digest
    );

    let changed = mutate_target(&document, |wire| {
        wire["declared_type"] = serde_json::json!("money");
        wire["type_params"] = serde_json::json!({"currency": "USD"});
    });
    assert_ne!(
        changed
            .contract_digest_sha256()
            .expect("declared type digest"),
        digest
    );

    let changed = mutate_target(&document, |wire| {
        wire["conditions"] = serde_json::json!([{
            "condition_id": "price-change",
            "predicate": {
                "kind": "changed",
                "reference": "last_accepted_observation"
            }
        }]);
    });
    changed.validate().expect("valid policy target");
    assert_ne!(
        changed.contract_digest_sha256().expect("condition digest"),
        digest
    );

    let two_conditions = mutate_target(&document, |wire| {
        wire["conditions"] = serde_json::json!([
            {
                "condition_id": "first",
                "predicate": {"kind": "lt", "threshold": "1"}
            },
            {
                "condition_id": "second",
                "predicate": {"kind": "gt", "threshold": "0"}
            }
        ]);
    });
    two_conditions
        .validate()
        .expect("valid two-condition target");
    let reordered = mutate_target(&two_conditions, |wire| {
        wire["conditions"]
            .as_array_mut()
            .expect("conditions")
            .reverse();
    });
    reordered
        .validate()
        .expect("valid reprioritized condition target");
    assert_eq!(
        reordered
            .contract_digest_sha256()
            .expect("reprioritized digest"),
        two_conditions
            .contract_digest_sha256()
            .expect("original two-condition digest")
    );

    let changed = mutate_target(&document, |wire| {
        wire["escalate_after"] = serde_json::json!(4);
    });
    assert_ne!(
        changed.contract_digest_sha256().expect("escalation digest"),
        digest
    );
}
