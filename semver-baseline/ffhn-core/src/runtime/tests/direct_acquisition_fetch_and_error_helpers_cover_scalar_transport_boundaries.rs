use super::support::*;

#[test]
fn direct_acquisition_fetch_and_error_helpers_cover_scalar_transport_boundaries() {
    let source_path = crate::test_support::absolute_file_path("source.json");
    let _document = toml::from_str::<TargetDocument>(&format!(
            "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = {source_path:?}\n\n[fetch]\nengine = \"file\"\nmax_bytes = 1024\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n",
        ))
        .expect("target");
    assert_eq!(
        acquire_json_scalar("/value", r#"{"value":"text"}"#).expect("string"),
        r#""text""#
    );
    assert_eq!(
        acquire_json_scalar("/value", r#"{"value":"1.2.3\u002bbuild.7"}"#).expect("escaped string"),
        r#""1.2.3\u002bbuild.7""#
    );
    assert_eq!(
        acquire_json_scalar("/value", r#"{"value":7}"#).expect("number"),
        "7"
    );
    assert_eq!(
        acquire_json_scalar("/value", r#"{"value":true}"#).expect("boolean"),
        "true"
    );
    assert_eq!(
        acquire_json_scalar("/value", r#"{"value":null}"#).expect("null"),
        "null"
    );
    assert!(acquire_json_scalar("/value", r#"{"value":[]}"#).is_err());
    assert!(acquire_json_scalar("/value", r#"{"other":1}"#).is_err());
    assert!(acquire_json_scalar("/value", "not JSON").is_err());

    let temporary = tempdir().expect("temporary directory");
    let file = temporary.path().join("source");
    fs::write(&file, "123").expect("source");
    assert_eq!(
        read_file_source(&file.to_string_lossy(), 3).expect("file"),
        "123"
    );
    let oversized = read_file_source(&file.to_string_lossy(), 2).expect_err("bounded file read");
    assert_eq!(
        oversized.fetch_failure(),
        Some(&FetchFailureDetails::BodyBytesExceeded {
            configured_max_bytes: 2,
            observed_bytes: 3,
        })
    );
    assert_eq!(
        oversized.message(),
        "the file source exceeded its configured byte limit"
    );
    fs::write(&file, [0xff]).expect("invalid UTF-8");
    let invalid_utf8 =
        read_file_source(&file.to_string_lossy(), 10).expect_err("invalid UTF-8 file source");
    assert_eq!(
        invalid_utf8.fetch_failure(),
        Some(&FetchFailureDetails::InvalidUtf8)
    );
    assert_eq!(invalid_utf8.message(), "file contents are not valid UTF-8");
    assert!(read_file_source("/does/not/exist", 10).is_err());
    assert!(read_file_source(&temporary.path().to_string_lossy(), 10).is_err());
    let mismatched: TargetDocument = toml::from_str(&format!(
            "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"file\"\nfile_path = {source_path:?}\n\n[fetch]\nengine = \"http\"\nuser_agent = \"test\"\naccept = \"application/json\"\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n",
        ))
        .expect("mismatched target");
    assert!(fetch_source(&mismatched).is_err());
    let http: TargetDocument = toml::from_str(
            "schema_name = \"ffhn.target\"\nschema_version = 12\ntarget_id = \"demo\"\ndisplay_name = \"Demo\"\nenabled = true\nescalate_after = 3\ndeclared_type = \"integer\"\nconditions = []\n\n[target]\nkind = \"http\"\nsource_url = \"http://127.0.0.1:1/value\"\n\n[fetch]\nengine = \"http\"\nuser_agent = \"ffhn-test\"\naccept = \"application/json\"\n\n[projection]\nkind = \"json_pointer\"\npointer = \"/value\"\n",
        )
        .expect("HTTP target");
    let fetch_error = match fetch_source(&http) {
        Ok(_) => panic!("closed port must fail HTTP acquisition"),
        Err(error) => error,
    };
    assert_eq!(fetch_error.kind(), DiagnosticKind::Io);
    assert_eq!(fetch_error.operation(), DiagnosticOperation::HttpFetch);
    assert_eq!(
        fetch_error.io_error_class(),
        Some(IoErrorClass::ConnectionRefused)
    );
    assert_eq!(
        fetch_error.message(),
        "the HTTP request could not be completed"
    );
    assert!(
        !serde_json::to_string(&fetch_error)
            .expect("fetch error JSON")
            .contains("Connection refused")
    );

    let json_error = serde_json::from_str::<serde_json::Value>("not JSON").expect_err("JSON error");
    assert_eq!(
        detail_from_error_for_operation(
            &CoreError::Json(json_error),
            DiagnosticOperation::TargetLoad,
            None,
        )
        .kind(),
        DiagnosticKind::Json
    );
    let toml_error = toml::from_str::<TargetDocument>("not TOML").expect_err("TOML error");
    assert_eq!(
        detail_from_error_for_operation(
            &CoreError::Toml(toml_error),
            DiagnosticOperation::TargetLoad,
            None,
        )
        .kind(),
        DiagnosticKind::Toml
    );
    let contract_detail = detail_from_error_for_operation(
        &CoreError::contract("bad"),
        DiagnosticOperation::TargetValidation,
        None,
    );
    assert_eq!(contract_detail.kind(), DiagnosticKind::Contract);
    assert_eq!(contract_detail.message(), "bad");

    let io_detail = detail_from_error_for_operation(
        &CoreError::io(
            "target.toml",
            std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        ),
        DiagnosticOperation::TargetLoad,
        None,
    );
    assert_eq!(io_detail.kind(), DiagnosticKind::Io);
    assert_eq!(
        io_detail.io_error_class(),
        Some(IoErrorClass::PermissionDenied)
    );
    assert_eq!(
        io_detail.message(),
        "the operating-system I/O operation did not complete"
    );
}
