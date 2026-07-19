use super::support::*;

#[test]
fn condition_id_is_stable_target_local_identity() {
    let identifier = ConditionId::new("price_drop").expect("identifier");
    assert_eq!(identifier.as_str(), "price_drop");
    assert_eq!(identifier.to_string(), "price_drop");
    assert_eq!(identifier.as_ref(), "price_drop");
    assert_eq!(String::from(identifier.clone()), "price_drop");
    assert_eq!(
        "price_drop".parse::<ConditionId>().expect("parsed"),
        identifier
    );
    assert_eq!(
        ConditionId::new("1price")
            .expect("digit-led identifier")
            .as_str(),
        "1price"
    );
    assert_eq!(
        ConditionId::new("a-1")
            .expect("mixed separator identifier")
            .as_str(),
        "a-1"
    );
    assert_eq!(
        serde_json::from_str::<ConditionId>("\"price-drop\"")
            .expect("deserialized")
            .as_str(),
        "price-drop"
    );
    for invalid in [
        "",
        "Price",
        "price--drop",
        "price__drop",
        "price-_drop",
        "price_-drop",
        "price_",
        "-price",
        &"a".repeat(65),
    ] {
        assert!(ConditionId::new(invalid).is_err(), "{invalid}");
    }
    assert!(serde_json::from_str::<ConditionId>("\"Price\"").is_err());
}
