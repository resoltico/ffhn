use super::support::*;

#[test]
fn policy_staging_keeps_the_four_t0_branches_disjoint_and_side_effect_free() {
    let target = target(
        "integer",
        "",
        &one_condition("kind = \"gt\"\nthreshold = \"1\""),
    );
    let current = observation("integer", "", "2");
    let contexts = context(None, None, None, false);

    assert_eq!(
        target
            .stage_policy_run(
                PolicyRunInput::PermanentContractError {
                    error_code: PermanentErrorCode::InvalidJsonPointer,
                    episode_began: true,
                },
                &contexts,
            )
            .expect("permanent stage"),
        StagedPolicyRun::PermanentContractError {
            error_code: PermanentErrorCode::InvalidJsonPointer,
            event_eligibilities: vec![StagedEventEligibility::OnRun {
                cause: OnRunEventCause::PermanentContractErrorEpisodeBegan {
                    error_code: PermanentErrorCode::InvalidJsonPointer,
                },
            }],
        }
    );
    let continued_permanent = target
        .stage_policy_run(
            PolicyRunInput::PermanentContractError {
                error_code: PermanentErrorCode::InvalidJsonPointer,
                episode_began: false,
            },
            &contexts,
        )
        .expect("continued permanent stage");
    assert!(continued_permanent.event_eligibilities().is_empty());
    assert_eq!(
        target
            .stage_policy_run(
                PolicyRunInput::IntegrationFault {
                    integration_fault_code: IntegrationFaultCode::HtmlcutInternalError,
                    episode_began: true,
                },
                &contexts,
            )
            .expect("integration stage"),
        StagedPolicyRun::IntegrationFault {
            integration_fault_code: IntegrationFaultCode::HtmlcutInternalError,
            event_eligibilities: vec![StagedEventEligibility::OnRun {
                cause: OnRunEventCause::IntegrationFaultEpisodeBegan {
                    integration_fault_code: IntegrationFaultCode::HtmlcutInternalError,
                },
            }],
        }
    );
    assert!(
        target
            .stage_policy_run(
                PolicyRunInput::IntegrationFault {
                    integration_fault_code: IntegrationFaultCode::HtmlcutInternalError,
                    episode_began: false,
                },
                &contexts,
            )
            .expect("continued integration stage")
            .event_eligibilities()
            .is_empty()
    );
    let invalid_pointer: TargetDocument = toml::from_str(
        &target_toml("integer", "", "conditions = []")
            .replace("pointer = \"/value\"", "pointer = \"not-a-pointer\""),
    )
    .expect("structurally readable target");
    assert!(invalid_pointer.validate().is_err());
    assert!(matches!(
        invalid_pointer
            .stage_policy_run(
                PolicyRunInput::PermanentContractError {
                    error_code: PermanentErrorCode::InvalidJsonPointer,
                    episode_began: false,
                },
                &BTreeMap::new(),
            )
            .expect("permanent invalid-pointer stage"),
        StagedPolicyRun::PermanentContractError { error_code, .. }
            if error_code == PermanentErrorCode::InvalidJsonPointer
    ));
    assert_eq!(
        target
            .stage_policy_run(
                PolicyRunInput::SourceSuspect {
                    reason_class: SourceSuspectReason::ValueUnparseable,
                    escalation_reached: true,
                },
                &contexts,
            )
            .expect("source stage"),
        StagedPolicyRun::SourceSuspect {
            reason_class: SourceSuspectReason::ValueUnparseable,
            event_eligibilities: vec![StagedEventEligibility::OnRun {
                cause: OnRunEventCause::SourceSuspectEscalated {
                    reason_class: SourceSuspectReason::ValueUnparseable,
                },
            }],
        }
    );
    let pre_escalation_source = target
        .stage_policy_run(
            PolicyRunInput::SourceSuspect {
                reason_class: SourceSuspectReason::ValueUnparseable,
                escalation_reached: false,
            },
            &contexts,
        )
        .expect("pre-escalation source stage");
    assert!(pre_escalation_source.event_eligibilities().is_empty());
    assert_eq!(
        PermanentErrorCode::InvalidJsonPointer.as_str(),
        "invalid_json_pointer"
    );
    assert_eq!(
        SourceSuspectReason::ValueUnparseable.as_str(),
        "value_unparseable"
    );
    assert!(matches!(
        target.stage_policy_run(
            PolicyRunInput::ValidObservation {
                observation: &current,
            },
            &contexts,
        )
        .expect("valid stage"),
        StagedPolicyRun::ValidObservation {
            condition_evaluations,
            ..
        } if condition_evaluations[0].outcome() == ConditionOutcome::Satisfied
    ));

    let unknown_context = BTreeMap::from([(
        ConditionId::new("unknown").expect("condition id"),
        ConditionContext::new(None, None, None, false),
    )]);
    assert!(
        target
            .stage_policy_run(
                PolicyRunInput::ValidObservation {
                    observation: &current,
                },
                &unknown_context,
            )
            .is_err()
    );
}
