use super::support::*;

#[test]
fn policy_values_cover_each_typed_comparison_and_exact_arithmetic_path() {
    let dollar = PolicyValue::Money {
        amount: Decimal::from(2),
        currency: "USD".to_owned(),
    };
    let dollar_three = PolicyValue::Money {
        amount: Decimal::from(3),
        currency: "USD".to_owned(),
    };
    let euro = PolicyValue::Money {
        amount: Decimal::from(2),
        currency: "EUR".to_owned(),
    };
    let semver_one = PolicyValue::Semver(Version::parse("1.0.0+left").expect("semver"));
    let semver_two = PolicyValue::Semver(Version::parse("1.0.1").expect("semver"));
    let instant = OffsetDateTime::parse("2026-01-01T00:00:00Z", &Rfc3339).expect("time");
    let later = OffsetDateTime::parse("2026-01-02T00:00:00Z", &Rfc3339).expect("time");
    let datetime = PolicyValue::Datetime(instant);
    let later_datetime = PolicyValue::Datetime(later);
    let text = PolicyValue::Text("In Stock".to_owned());
    let integer_one = PolicyValue::Integer(1);
    let integer_two = PolicyValue::Integer(2);

    assert_eq!(dollar.compare(&dollar_three), Some(Ordering::Less));
    assert_eq!(dollar.compare(&euro), None);
    assert_eq!(semver_one.compare(&semver_two), Some(Ordering::Less));
    assert_eq!(datetime.compare(&later_datetime), Some(Ordering::Less));
    assert_eq!(text.compare(&text), None);
    assert!(!dollar.is_negative_numeric());
    assert!(PolicyValue::Integer(-1).is_negative_numeric());
    assert!(PolicyValue::Decimal(Decimal::NEGATIVE_ONE).is_negative_numeric());
    assert!(
        PolicyValue::Money {
            amount: Decimal::NEGATIVE_ONE,
            currency: "USD".to_owned(),
        }
        .is_negative_numeric()
    );
    assert!(!semver_one.is_negative_numeric());
    assert!(!datetime.is_negative_numeric());
    assert!(!text.is_negative_numeric());
    assert!(integer_one.canonical_identity_eq(&integer_one));
    assert!(!integer_one.canonical_identity_eq(&integer_two));
    assert!(dollar.canonical_identity_eq(&dollar));
    assert!(!dollar.canonical_identity_eq(&euro));
    assert!(!dollar.canonical_identity_eq(&dollar_three));
    assert!(semver_one.canonical_identity_eq(&semver_one));
    assert!(datetime.canonical_identity_eq(&datetime));
    assert!(text.canonical_identity_eq(&text));
    assert!(!dollar.canonical_identity_eq(&PolicyValue::Integer(2)));

    assert_eq!(
        PolicyValue::Decimal(Decimal::from(3)).exact_abs_delta_at_least(
            &PolicyValue::Decimal(Decimal::ONE),
            &PolicyValue::Decimal(Decimal::from(2)),
        ),
        Ok(ArithmeticResult::Decision(true))
    );
    assert_eq!(
        PolicyValue::Decimal(Decimal::MAX).exact_abs_delta_at_least(
            &PolicyValue::Decimal(-Decimal::MAX),
            &PolicyValue::Decimal(Decimal::ZERO),
        ),
        Ok(ArithmeticResult::Decision(true))
    );
    assert_eq!(
        dollar_three.exact_abs_delta_at_least(&dollar, &dollar),
        Ok(ArithmeticResult::Decision(false))
    );
    assert_eq!(
        dollar.exact_abs_delta_at_least(&euro, &dollar),
        Ok(ArithmeticResult::Unavailable)
    );
    assert_eq!(
        dollar_three.exact_abs_delta_at_least(&dollar, &euro),
        Ok(ArithmeticResult::Unavailable)
    );
    assert_eq!(
        PolicyValue::Integer(i128::MIN)
            .exact_abs_delta_at_least(&PolicyValue::Integer(0), &PolicyValue::Integer(0),),
        Ok(ArithmeticResult::Overflow)
    );
    let dollar_max = PolicyValue::Money {
        amount: Decimal::MAX,
        currency: "USD".to_owned(),
    };
    let dollar_negative_max = PolicyValue::Money {
        amount: -Decimal::MAX,
        currency: "USD".to_owned(),
    };
    assert_eq!(
        dollar_max.exact_abs_delta_at_least(
            &dollar_negative_max,
            &PolicyValue::Money {
                amount: Decimal::ZERO,
                currency: "USD".to_owned(),
            },
        ),
        Ok(ArithmeticResult::Decision(true))
    );

    assert_eq!(
        PolicyValue::Integer(105)
            .exact_percentage_delta_at_least(&PolicyValue::Integer(100), Decimal::from(5),),
        Ok(ArithmeticResult::Decision(true))
    );
    assert_eq!(
        PolicyValue::Integer(i128::MAX)
            .exact_percentage_delta_at_least(&PolicyValue::Integer(1), Decimal::ONE,),
        Ok(ArithmeticResult::Decision(true))
    );
    assert_eq!(
        PolicyValue::Integer(i128::MAX)
            .exact_percentage_delta_at_least(&PolicyValue::Integer(i128::MAX), Decimal::ONE,),
        Ok(ArithmeticResult::Decision(false))
    );
    assert_eq!(
        PolicyValue::Integer(i128::MAX)
            .exact_percentage_delta_at_least(&PolicyValue::Integer(i128::MIN), Decimal::from(199),),
        Ok(ArithmeticResult::Decision(true))
    );
    assert_eq!(
        PolicyValue::Integer(i128::MAX)
            .exact_percentage_delta_at_least(&PolicyValue::Integer(i128::MIN), Decimal::from(200),),
        Ok(ArithmeticResult::Decision(false))
    );
    assert_eq!(
        PolicyValue::Integer(i128::MAX).exact_percentage_delta_at_least(
            &PolicyValue::Integer(i128::MIN),
            "199.9999999999999999999999999"
                .parse::<Decimal>()
                .expect("fractional percentage"),
        ),
        Ok(ArithmeticResult::Decision(true))
    );
    assert_eq!(
        PolicyValue::Integer(i128::MAX)
            .exact_percentage_delta_at_least(&PolicyValue::Integer(1), Decimal::ZERO,),
        Ok(ArithmeticResult::Decision(true))
    );
    assert_eq!(
        PolicyValue::Decimal(Decimal::MAX)
            .exact_percentage_delta_at_least(&PolicyValue::Decimal(-Decimal::MAX), Decimal::ONE,),
        Ok(ArithmeticResult::Decision(true))
    );
    assert_eq!(
        PolicyValue::Decimal(Decimal::MAX)
            .exact_percentage_delta_at_least(&PolicyValue::Decimal(Decimal::ONE), Decimal::ONE,),
        Ok(ArithmeticResult::Decision(true))
    );
    assert_eq!(
        PolicyValue::Decimal(Decimal::ONE)
            .exact_percentage_delta_at_least(&PolicyValue::Decimal(Decimal::MAX), Decimal::MAX,),
        Ok(ArithmeticResult::Decision(false))
    );
    assert_eq!(
        PolicyValue::Decimal(Decimal::from(3)).exact_percentage_delta_at_least(
            &PolicyValue::Decimal(Decimal::from(2)),
            Decimal::MAX,
        ),
        Ok(ArithmeticResult::Decision(false))
    );
    assert_eq!(
        dollar_three.exact_percentage_delta_at_least(&dollar, Decimal::from(50)),
        Ok(ArithmeticResult::Decision(true))
    );
    assert_eq!(
        dollar_three.exact_percentage_delta_at_least(&euro, Decimal::from(50)),
        Ok(ArithmeticResult::Unavailable)
    );
    assert_eq!(
        PolicyValue::Integer(105).exact_percentage_delta_at_least(
            &PolicyValue::Decimal(Decimal::from(100)),
            Decimal::from(5)
        ),
        Ok(ArithmeticResult::Unavailable)
    );
    assert_eq!(
        semver_one.exact_percentage_delta_at_least(&semver_two, Decimal::ONE),
        Ok(ArithmeticResult::Unavailable)
    );

    assert!(parse_percentage(" ").is_err());
    assert!(parse_percentage("").is_err());
    assert!(parse_percentage("not-a-number").is_err());
    assert!(
        parse_config_value(
            crate::DeclaredType::Money,
            &crate::TypeParams::default(),
            "1"
        )
        .is_err()
    );
}
