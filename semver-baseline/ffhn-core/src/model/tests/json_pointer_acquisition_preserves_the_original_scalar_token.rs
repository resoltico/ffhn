use super::support::*;

#[test]
fn json_pointer_acquisition_preserves_the_original_scalar_token() {
    assert_eq!(
        select_json_scalar_token(r#"{"value":"1.2.3\u002bbuild.7"}"#, "/value")
            .expect("escaped semver token"),
        r#""1.2.3\u002bbuild.7""#
    );
    assert_eq!(
        select_json_scalar_token(r#"{"a/b":{"~key":"1.00"}}"#, "/a~1b/~0key")
            .expect("escaped pointer token"),
        r#""1.00""#
    );
    assert_eq!(
        select_json_scalar_token(r#"{"items":[0,1.00]}"#, "/items/1").expect("array token"),
        "1.00"
    );
    assert_eq!(parse_json_array_index("0"), Some(0));
    assert!(decode_json_pointer_token("~2").is_err());
    assert!(normalized_raw_token(" ").is_err());
    assert!(json_scalar_value(" 1 ").is_err());
    assert!(select_json_pointer_child("{", "value").is_err());
    assert!(select_json_pointer_child("[", "0").is_err());
    assert!(select_json_pointer_child("1", "value").is_err());
    assert!(select_json_scalar_token(r#"{"value":[]}"#, "/value").is_err());
    assert!(select_json_scalar_token(r#"{"value":{}}"#, "/value").is_err());
    assert!(select_json_scalar_token(r#"{"items":[0]}"#, "/items/01").is_err());
}
