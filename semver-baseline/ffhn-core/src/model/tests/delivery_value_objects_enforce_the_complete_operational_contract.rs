use super::support::*;

#[test]
fn delivery_value_objects_enforce_the_complete_operational_contract() {
    assert_eq!(RouteFamily::OnRun.as_str(), "on_run");
    assert_eq!(RouteFamily::OnCondition.as_str(), "on_condition");

    let route_id = RouteId::new("primary-route").expect("valid route id");
    assert_eq!(route_id.as_str(), "primary-route");
    assert_eq!(route_id.as_ref(), "primary-route");
    assert_eq!(route_id.to_string(), "primary-route");
    assert_eq!(String::from(route_id.clone()), "primary-route");
    assert_eq!(
        "primary-route".parse::<RouteId>().expect("parse route id"),
        route_id
    );
    assert_eq!(
        RouteId::try_from("secondary".to_owned())
            .expect("try route id")
            .as_str(),
        "secondary"
    );
    assert_eq!(
        RouteId::new("1route").expect("digit-led route id").as_str(),
        "1route"
    );
    for invalid in [
        "",
        "-leading",
        "_leading",
        "trailing-",
        "double--separator",
        "double__separator",
        "mixed-_separator",
        "mixed_-separator",
        "upperCase",
        "has space",
        &"a".repeat(65),
    ] {
        assert!(
            RouteId::new(invalid).is_err(),
            "{invalid:?} must be rejected"
        );
    }

    let defaults = OutboxPolicy::default();
    assert_eq!(defaults.max_pending(), 100);
    assert_eq!(defaults.max_attempts(), 5);
    assert_eq!(defaults.base_backoff_ms(), 1_000);
    assert_eq!(defaults.max_backoff_ms(), 300_000);
    defaults.validate().expect("default policy");
    for (field, value) in [
        ("max_pending", serde_json::json!(0)),
        ("max_attempts", serde_json::json!(0)),
        ("base_backoff_ms", serde_json::json!(0)),
        ("max_backoff_ms", serde_json::json!(999)),
    ] {
        let mut wire = serde_json::to_value(&defaults).expect("policy JSON");
        wire[field] = value;
        let policy: OutboxPolicy = serde_json::from_value(wire).expect("policy shape");
        assert!(policy.validate().is_err(), "{field} must be bounded");
    }

    let successful_args = crate::test_support::SUCCESSFUL_PROCESS_ARGS
        .iter()
        .map(|argument| (*argument).to_owned())
        .collect::<Vec<_>>();
    let valid_route: DeliveryRoute = serde_json::from_value(serde_json::json!({
        "route_id": "primary",
        "route_family": "on_run",
        "adapter": {
            "kind": "process_stdin",
            "program": crate::test_support::PROCESS_PROGRAM,
            "args": successful_args,
            "timeout_ms": 100,
        }
    }))
    .expect("route shape");
    valid_route.validate().expect("valid route");
    assert_eq!(valid_route.route_id(), "primary");
    assert_eq!(valid_route.route_family(), RouteFamily::OnRun);
    assert_eq!(
        valid_route.adapter().process_stdin(),
        (
            crate::test_support::PROCESS_PROGRAM,
            &successful_args[..],
            100,
        )
    );
    validate_routes(&[]).expect("empty route list");
    validate_routes(std::slice::from_ref(&valid_route)).expect("one route");

    for (field, value) in [
        ("program", serde_json::json!("relative-program")),
        ("program", serde_json::json!("   ")),
        ("args", serde_json::json!([" "])),
        ("timeout_ms", serde_json::json!(99)),
    ] {
        let mut wire = serde_json::to_value(&valid_route).expect("route JSON");
        wire["adapter"][field] = value;
        let route: DeliveryRoute = serde_json::from_value(wire).expect("route shape");
        assert!(route.validate().is_err(), "{field} must be bounded");
    }
    assert!(validate_routes(&[valid_route.clone(), valid_route.clone()]).is_err());
    let later: DeliveryRoute = serde_json::from_value(serde_json::json!({
        "route_id": "zeta",
        "route_family": "on_condition",
        "adapter": {"kind": "process_stdin", "program": crate::test_support::PROCESS_PROGRAM, "timeout_ms": 60000}
    }))
    .expect("later route");
    validate_routes(&[later, valid_route]).expect("route declaration order is valid");
}
