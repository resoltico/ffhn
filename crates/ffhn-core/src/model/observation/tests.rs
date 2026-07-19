use super::parse::{
    JsonAcquisitionFailure, JsonScalarError, json_string_value, validated_scalar_selection,
};

#[test]
fn validated_scalar_selection_classifies_internal_invalid_and_non_scalar_tokens() {
    assert!(matches!(
        validated_scalar_selection(" 1".to_owned()),
        Err(JsonAcquisitionFailure::Malformed(_))
    ));
    assert!(matches!(
        validated_scalar_selection("[]".to_owned()),
        Err(JsonAcquisitionFailure::NonScalarPointerTarget(_))
    ));
    assert_eq!(
        JsonScalarError::NonScalar.message(),
        "projection.pointer must select a scalar JSON leaf"
    );
}

#[test]
fn text_json_values_require_an_unpadded_string_scalar() {
    assert!(matches!(
        json_string_value(" \"text\""),
        Err(JsonScalarError::Invalid(_))
    ));
    assert!(matches!(
        json_string_value("42"),
        Err(JsonScalarError::Invalid(_))
    ));
    assert!(matches!(
        json_string_value("{}"),
        Err(JsonScalarError::NonScalar)
    ));
}
