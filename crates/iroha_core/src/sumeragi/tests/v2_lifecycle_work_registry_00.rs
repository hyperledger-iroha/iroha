#[cfg(feature = "bls")]
#[derive(Clone, Copy, Debug)]
enum ProductionReadyValidateDispatchRow {
    ValidatedBusy,
    ValidatedInactive,
    ValidatedNoEffect,
    ValidatedApply,
    ValidatedPersist,
    RejectedBusy,
    RejectedInactive,
    RejectedNoEffect,
    RejectedReport,
}

#[cfg(feature = "bls")]
impl ProductionReadyValidateDispatchRow {
    const ALL: [Self; 9] = [
        Self::ValidatedBusy,
        Self::ValidatedInactive,
        Self::ValidatedNoEffect,
        Self::ValidatedApply,
        Self::ValidatedPersist,
        Self::RejectedBusy,
        Self::RejectedInactive,
        Self::RejectedNoEffect,
        Self::RejectedReport,
    ];

    const fn publication_kind(self) -> ReadyDurableValidateAdapterPublicationKind {
        match self {
            Self::ValidatedBusy => ReadyDurableValidateAdapterPublicationKind::ValidatedBusy,
            Self::ValidatedInactive => {
                ReadyDurableValidateAdapterPublicationKind::ValidatedInactive
            }
            Self::ValidatedNoEffect => {
                ReadyDurableValidateAdapterPublicationKind::ValidatedNoEffect
            }
            Self::ValidatedApply => ReadyDurableValidateAdapterPublicationKind::ValidatedApply,
            Self::ValidatedPersist => ReadyDurableValidateAdapterPublicationKind::ValidatedPersist,
            Self::RejectedBusy => ReadyDurableValidateAdapterPublicationKind::RejectedBusy,
            Self::RejectedInactive => ReadyDurableValidateAdapterPublicationKind::RejectedInactive,
            Self::RejectedNoEffect => ReadyDurableValidateAdapterPublicationKind::RejectedNoEffect,
            Self::RejectedReport => ReadyDurableValidateAdapterPublicationKind::RejectedReport,
        }
    }

    const fn fixture_outcome(self) -> ReadyDurableValidateFixtureOutcome {
        match self {
            Self::ValidatedBusy
            | Self::ValidatedInactive
            | Self::ValidatedNoEffect
            | Self::ValidatedApply
            | Self::ValidatedPersist => ReadyDurableValidateFixtureOutcome::Validated,
            Self::RejectedBusy
            | Self::RejectedInactive
            | Self::RejectedNoEffect
            | Self::RejectedReport => ReadyDurableValidateFixtureOutcome::Rejected,
        }
    }

    const fn uses_local_origin(self) -> bool {
        matches!(self, Self::ValidatedInactive | Self::RejectedInactive)
    }

    const fn requires_set_b(self) -> bool {
        matches!(self, Self::ValidatedNoEffect)
    }

    const fn view(self) -> wire::View {
        0
    }

    const fn local_validator(
        self,
        active_validator: wire::ValidatorIndex,
        set_b_validator: wire::ValidatorIndex,
    ) -> wire::ValidatorIndex {
        if self.requires_set_b() {
            set_b_validator
        } else {
            active_validator
        }
    }

    const fn is_busy(self) -> bool {
        matches!(self, Self::ValidatedBusy | Self::RejectedBusy)
    }

    const fn certificate_phase(self) -> Option<wire::GlobalPhase> {
        match self {
            Self::ValidatedApply => Some(wire::GlobalPhase::Commit),
            Self::ValidatedPersist | Self::RejectedReport => Some(wire::GlobalPhase::Prepare),
            Self::ValidatedBusy
            | Self::ValidatedInactive
            | Self::ValidatedNoEffect
            | Self::RejectedBusy
            | Self::RejectedInactive
            | Self::RejectedNoEffect => None,
        }
    }

    const fn grows_safety_wal(self) -> bool {
        matches!(self, Self::ValidatedPersist)
    }

    const fn successor(
        self,
    ) -> Option<(
        super::super::schema::DurableContinuationEdge,
        LifecycleWorkClass,
    )> {
        use super::super::schema::DurableContinuationEdge as Edge;

        match self {
            Self::ValidatedApply => Some((Edge::ValidateToApply, LifecycleWorkClass::Apply)),
            Self::ValidatedPersist => {
                Some((Edge::ValidateToSignCommit, LifecycleWorkClass::SignVote))
            }
            Self::RejectedReport => Some((
                Edge::ValidateToInvalidBodyReport,
                LifecycleWorkClass::InvalidBodyReport,
            )),
            Self::ValidatedBusy
            | Self::ValidatedInactive
            | Self::ValidatedNoEffect
            | Self::RejectedBusy
            | Self::RejectedInactive
            | Self::RejectedNoEffect => None,
        }
    }

    fn expected_dispatch(self, ordinal: u128) -> super::super::ProductionCompletionDispatchV1 {
        use super::super::ProductionCompletionDispatchV1 as Dispatch;

        match self {
            Self::ValidatedBusy | Self::RejectedBusy => Dispatch::ReducerFenceWait { ordinal },
            Self::ValidatedInactive
            | Self::ValidatedNoEffect
            | Self::RejectedInactive
            | Self::RejectedNoEffect => Dispatch::ValidateNoSuccessor { ordinal },
            Self::ValidatedApply => Dispatch::BodyStageAdvanced {
                parent_ordinal: ordinal,
                child: LifecycleWorkClass::Apply,
            },
            Self::ValidatedPersist => Dispatch::BodyStageAdvanced {
                parent_ordinal: ordinal,
                child: LifecycleWorkClass::SignVote,
            },
            Self::RejectedReport => Dispatch::BodyStageAdvanced {
                parent_ordinal: ordinal,
                child: LifecycleWorkClass::InvalidBodyReport,
            },
        }
    }
}

#[cfg(feature = "bls")]
fn assert_production_ready_validate_cold_open(
    row: ProductionReadyValidateDispatchRow,
    ledger_root: &std::path::Path,
    active_context: LifecycleContext,
    parent_ordinal: u128,
) {
    use super::super::schema::DurableContinuation;

    let (_store, ledger) =
        super::super::ledger::LifecycleLedgerStoreV1::open(ledger_root, active_context)
            .unwrap_or_else(|error| panic!("{row:?}: cold-open published LedgerV1: {error}"));
    let records = ledger.records();
    let parent = records
        .first()
        .unwrap_or_else(|| panic!("{row:?}: cold-open must retain the Validate parent"));
    assert_eq!(parent.ordinal(), parent_ordinal, "{row:?}");
    assert_eq!(
        parent.work_class(),
        Some(LifecycleWorkClass::Validate),
        "{row:?}"
    );
    if row.is_busy() {
        assert_eq!(records.len(), 1, "{row:?}");
        assert_eq!(ledger.high_water(), parent_ordinal, "{row:?}");
        assert_eq!(parent.terminal(), Some(None), "{row:?}");
        assert_eq!(
            parent.continuation(),
            Some(DurableContinuation::None),
            "{row:?}"
        );
        return;
    }

    assert_eq!(
        parent.terminal(),
        Some(Some(TerminalOutcome::Advanced)),
        "{row:?}"
    );
    match row.successor() {
        None => {
            assert_eq!(records.len(), 1, "{row:?}");
            assert_eq!(ledger.high_water(), parent_ordinal, "{row:?}");
            assert_eq!(
                parent.continuation(),
                Some(DurableContinuation::AdvancedNoSuccessor),
                "{row:?}"
            );
        }
        Some((edge, child_class)) => {
            let child_ordinal = parent_ordinal
                .checked_add(1)
                .expect("fixture successor ordinal remains representable");
            assert_eq!(records.len(), 2, "{row:?}");
            assert_eq!(ledger.high_water(), child_ordinal, "{row:?}");
            assert_eq!(
                parent.continuation(),
                Some(DurableContinuation::successor(edge, child_ordinal)),
                "{row:?}"
            );
            let child = &records[1];
            assert_eq!(child.ordinal(), child_ordinal, "{row:?}");
            assert_eq!(child.work_class(), Some(child_class), "{row:?}");
            let child_terminal = matches!(row, ProductionReadyValidateDispatchRow::ValidatedApply)
                .then_some(TerminalOutcome::Advanced);
            assert_eq!(child.terminal(), Some(child_terminal), "{row:?}");
            assert_eq!(
                child.continuation(),
                Some(DurableContinuation::None),
                "{row:?}"
            );
        }
    }
}

#[cfg(feature = "bls")]
fn production_ready_validate_dispatch_marker() -> u8 {
    0xE0
}

#[cfg(feature = "bls")]
fn owned_production_ready_validate_fixture(
    row: ProductionReadyValidateDispatchRow,
    marker: u8,
) -> OwnedReadyDurableValidateFixture {
    let view = row.view();
    let waiting = if row.uses_local_origin() {
        waiting_durable_validate_fixture_from_store(durable_local_validate_store_fixture_at_view(
            marker, view,
        ))
    } else {
        waiting_durable_validate_fixture_at_view(marker, view)
    };
    owned_ready_durable_validate_fixture_from_waiting(waiting, row.fixture_outcome())
}

#[cfg(feature = "bls")]
struct ProductionRecoveredApplyReadyFixture {
    owned: OwnedReadyDurableValidateFixture,
    commit_qc: wire::QuorumCertificate,
    apply_service: crate::sumeragi::v2_apply::V2ApplyService,
    validator_keys: Vec<KeyPair>,
}

#[cfg(feature = "bls")]
fn production_recovered_apply_ready_fixture(
    marker: u8,
) -> ProductionRecoveredApplyReadyFixture {
    let crate::sumeragi::v2_apply::ProductionRecoveredDecisionApplyFixtureV1 {
        verified,
        manifest,
        canonical_wire,
        body_store,
        durable,
        commit_qc,
        apply_service,
        validator_keys,
        directory,
    } = crate::sumeragi::v2_apply::production_recovered_decision_apply_fixture_v1();
    let fixture = durable_validate_fixture_from_material(
        marker,
        verified,
        manifest,
        canonical_wire,
    );
    let waiting = waiting_durable_validate_fixture_from_store(
        durable_validate_store_fixture_from_existing(
            fixture,
            directory,
            body_store,
            durable,
            Some(commit_qc.execution_commitment),
        ),
    );
    let owned = owned_ready_durable_validate_fixture_from_waiting_with_commitment(
        waiting,
        ReadyDurableValidateFixtureOutcome::Validated,
        Some(commit_qc.execution_commitment),
    );
    ProductionRecoveredApplyReadyFixture {
        owned,
        commit_qc,
        apply_service,
        validator_keys,
    }
}

#[cfg(feature = "bls")]
fn signed_ready_validate_timeout_vote(
    context: &wire::HeightContext,
    keys: &[KeyPair],
    view: wire::View,
    signer: wire::ValidatorIndex,
) -> wire::ConsensusMessageV2 {
    let mut vote = wire::TimeoutVote {
        round: wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view,
        },
        highest_prepare_qc: None,
        signer,
        signature: Vec::new(),
    };
    vote.signature = iroha_crypto::Signature::new(
        keys[usize::try_from(signer).expect("small Ready Validate signer index")].private_key(),
        &vote.signature_preimage(),
    )
    .payload()
    .to_vec();
    wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::TimeoutVote(vote))
}

#[cfg(feature = "bls")]
fn register_production_ready_validate_remote_body(
    row: ProductionReadyValidateDispatchRow,
    marker: u8,
    ready: &ReadyDurableValidateFixture,
    commit_qc: Option<&wire::QuorumCertificate>,
    adapter: &mut SumeragiV2Adapter,
) {
    let (tag, round, subject) = match &ready.fixture.effect {
        AdapterEffect::ValidateBody {
            tag,
            round,
            subject,
        } => (*tag, *round, *subject),
        _ => unreachable!("Ready fixture retains one Validate effect"),
    };
    let proposal = wire::Proposal {
        round,
        proposer: ready.fixture.verified.context().leader(round.view),
        subject,
        manifest: ready.fixture.manifest.clone(),
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature: vec![marker],
    };
    let fetch = adapter
        .receive_authenticated(AuthenticatedConsensusMessage::for_test(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(proposal)),
        ))
        .unwrap_or_else(|error| panic!("{row:?}: admit exact Ready Validate Proposal: {error}"))
        .into_effects();
    if row.requires_set_b() {
        assert!(
            fetch.is_empty(),
            "{row:?}: inactive Set-B acquisition emits no Fetch: {fetch:?}"
        );
    } else {
        assert!(
            matches!(
                fetch.as_slice(),
                [AdapterEffect::FetchBody {
                    tag: effect_tag,
                    manifest: Some(effect_manifest),
                    ..
                }] if *effect_tag == tag && effect_manifest == &ready.fixture.manifest
            ),
            "{row:?}: unexpected Proposal effects: {fetch:?}"
        );
    }
    let store = adapter
        .body_available(tag, ready.fixture.manifest.clone())
        .unwrap_or_else(|error| panic!("{row:?}: advance exact body to Store: {error}"))
        .into_effects();
    assert!(
        matches!(
            store.as_slice(),
            [AdapterEffect::StoreBody {
                tag: effect_tag,
                round: effect_round,
                subject: effect_subject,
            }] if *effect_tag == tag && *effect_round == round && *effect_subject == subject
        ),
        "{row:?}: unexpected BodyAvailable effects: {store:?}"
    );
    let validate = adapter
        .body_stored(tag, round, subject, &ready.durable)
        .unwrap_or_else(|error| panic!("{row:?}: advance exact body to Validate: {error}"))
        .into_effects();
    assert!(
        matches!(
            validate.as_slice(),
            [AdapterEffect::ValidateBody {
                tag: effect_tag,
                round: effect_round,
                subject: effect_subject,
            }] if *effect_tag == tag && *effect_round == round && *effect_subject == subject
        ),
        "{row:?}: unexpected BodyStored effects: {validate:?}"
    );

    if let Some(phase) = row.certificate_phase() {
        let certificate = match phase {
            wire::GlobalPhase::Prepare => certified_pipeline_prepare_certificate_for_test(
                &ready.fixture.manifest,
                &ready.durable,
            ),
            wire::GlobalPhase::Commit => commit_qc
                .cloned()
                .expect("ValidatedApply retains its real aggregate-signed CommitQC"),
        };
        let observed = adapter
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(
                wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
                    certificate,
                )),
            ))
            .unwrap_or_else(|error| panic!("{row:?}: observe exact certificate: {error}"));
        assert!(
            observed.effects().is_empty(),
            "{row:?}: a certificate beside a Durable body has no immediate successor"
        );
    }
    if row.is_busy() {
        let sign = adapter
            .timeout_elapsed(adapter.current_tag())
            .unwrap_or_else(|error| panic!("{row:?}: open exact reducer fence: {error}"))
            .into_effects();
        assert!(
            matches!(
                sign.as_slice(),
                [AdapterEffect::Sign {
                    tag: effect_tag,
                    request: SignRequest::TimeoutVote(_),
                }] if *effect_tag == tag
            ),
            "{row:?}: timeout must retain one exact Sign fence: {sign:?}"
        );
    }
}

#[cfg(feature = "bls")]
#[allow(clippy::too_many_lines)]
fn production_completion_dispatch_publishes_all_ready_validate_outcomes_fixture() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    crate::sumeragi::status::clear_v2_status();
    let marker = production_ready_validate_dispatch_marker();
    for row in ProductionReadyValidateDispatchRow::ALL {
        let (owned, recovered_apply) =
            if matches!(row, ProductionReadyValidateDispatchRow::ValidatedApply) {
                let ProductionRecoveredApplyReadyFixture {
                    owned,
                    commit_qc,
                    apply_service,
                    validator_keys,
                } = production_recovered_apply_ready_fixture(marker);
                (
                    owned,
                    Some((commit_qc, apply_service, validator_keys)),
                )
            } else {
                (owned_production_ready_validate_fixture(row, marker), None)
            };
        let OwnedReadyDurableValidateFixture {
            mut ready,
            store,
            coordinator,
        } = owned;
        let context = ready.fixture.verified.context().clone();
        let committee = crate::sumeragi::v2_core::Committee::project_indices(
            context.height,
            row.view(),
            context.roster.len(),
            context.leader(row.view()),
        )
        .expect("project exact Ready Validate committee");
        let active_validator = committee.leader();
        let set_b_validator = *committee
            .set_b()
            .first()
            .expect("four-validator committee has one Set-B validator");
        let local_validator = row.local_validator(active_validator, set_b_validator);
        let expected_role = if row.requires_set_b() {
            crate::sumeragi::v2_core::CommitteeRole::SetBValidator
        } else {
            crate::sumeragi::v2_core::CommitteeRole::Leader
        };
        assert_eq!(
            committee.role(local_validator),
            Ok(expected_role),
            "{row:?}"
        );
        let AdapterEffect::ValidateBody { tag, .. } = &ready.fixture.effect else {
            unreachable!("Ready fixture retains one Validate effect")
        };
        let adapter_directory =
            TempDir::new().expect("temporary production Ready Validate adapter");
        let wal_path = adapter_directory.path().join("safety.wal");
        let (mut adapter, startup) = SumeragiV2Adapter::open(
            &wal_path,
            ready.fixture.verified.clone(),
            Some(local_validator),
            tag.generation(),
            [marker; 32],
            AdapterFingerprints {
                node: Hash::new([marker, 0xD1]),
                build: Hash::new([marker, 0xD2]),
                config: Hash::new([marker, 0xD3]),
            },
            DeferredAdmissionOrdinalSource::new(
                ready
                    .lease
                    .ordinal()
                    .checked_add(1)
                    .expect("fixture lifecycle ordinal remains representable"),
            ),
        )
        .unwrap_or_else(|error| panic!("{row:?}: open exact adapter: {error}"));
        assert!(startup.is_empty(), "{row:?}: fixture WAL must be empty");
        if !row.uses_local_origin() {
            register_production_ready_validate_remote_body(
                row,
                marker,
                &ready,
                recovered_apply
                    .as_ref()
                    .map(|(commit_qc, _, _)| commit_qc),
                &mut adapter,
            );
        }
        {
            let prepared = ready
                .holder
                .registry_for_test_mut()
                .prepare_ready_durable_validate_execution(
                    &ready.lease,
                    ready.fixture.slot,
                    &ready.fixture.verified,
                )
                .unwrap_or_else(|error| {
                    panic!("{row:?}: prepare exact Ready Validate carrier: {error:?}")
                });
            assert_eq!(
                prepared
                    .preflight_adapter_publication_kind(&mut adapter)
                    .unwrap_or_else(|error| {
                        panic!("{row:?}: classify exact adapter publication: {error}")
                    }),
                row.publication_kind(),
                "{row:?}: fixture must force the requested adapter publication"
            );
        }
        let ReadyDurableValidateFixture {
            fixture,
            _directory,
            holder,
            lease,
            durable: _,
        } = ready;
        let wal_before = std::fs::read(&wal_path)
            .unwrap_or_else(|error| panic!("{row:?}: read pre-dispatch WAL: {error}"));
        let now = std::time::Instant::now();
        let lifecycle_ordinals =
            crate::sumeragi::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(
                lease.ordinal(),
            );
        let (runtime, returned_startup) =
            crate::sumeragi::v2_runtime::SerializedV2Runtime::new_with_lifecycle_ordinals(
                adapter,
                startup,
                now,
                std::time::Duration::from_secs(10),
                crate::sumeragi::v2_runtime::RuntimeQueueConfig::new(8, 2, 2),
                lifecycle_ordinals,
            )
            .unwrap_or_else(|error| panic!("{row:?}: wrap exact serialized runtime: {error}"));
        assert!(
            returned_startup.is_empty(),
            "{row:?}: empty WAL cannot return startup effects"
        );
        let owner_directory = TempDir::new().expect("temporary Ready Validate owner");
        let active_context = coordinator.active_context;
        let mut owner =
            super::super::ProductionLifecycleOwnerV1::ready_validate_completion_owner_for_test(
                fixture.verified.clone(),
                coordinator,
                holder,
                store,
                owner_directory.path(),
            );
        let ledger_root = owner_directory.path().join("ledger");
        let ledger_path = ledger_root.join("lifecycle-ledger-v1.norito");
        let ledger_before = std::fs::read(&ledger_path)
            .unwrap_or_else(|error| panic!("{row:?}: read pre-dispatch LedgerV1: {error}"));
        let (mut services, _keys) = crate::sumeragi::v2_worker::tests::fixture();
        let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
        let (mut executor, mut planner_io) = owner
            .bind_body_store_to_recovered_completion_io_for_test(
                &mut services,
                runtime,
                std::sync::Arc::clone(&output_guard),
                local_validator,
                2,
            );
        executor
            .arm_live_clocks(
                crate::sumeragi::v2_lifecycle_coordinator::ProductionLifecycleLiveClockActivationPermitV1::for_test(),
                now,
            )
            .unwrap_or_else(|error| panic!("{row:?}: arm exact runtime clocks: {error}"));
        let queued_apply_snapshot =
            matches!(row, ProductionReadyValidateDispatchRow::ValidatedApply).then(|| {
                let keys = &recovered_apply
                    .as_ref()
                    .expect("ValidatedApply retains its exact production fixture")
                    .2;
                let unrelated =
                    signed_ready_validate_timeout_vote(&context, keys, row.view(), 3);
                executor
                    .enqueue_network(unrelated)
                    .expect("queue authenticated runtime ingress beside typed live Apply");
                executor.runtime_queue_snapshot_for_test(now)
            });
        let dispatched = owner
            .dispatch_completion_for_test(&services, &mut executor, 0)
            .unwrap_or_else(|error| panic!("{row:?}: production Completion dispatch: {error:?}"));
        assert_eq!(
            dispatched,
            row.expected_dispatch(lease.ordinal()),
            "{row:?}"
        );
        if matches!(row, ProductionReadyValidateDispatchRow::ValidatedApply) {
            let apply_ordinal = lease
                .ordinal()
                .checked_add(1)
                .expect("Validate-to-Apply child ordinal remains representable");
            let dispatched = owner
                .dispatch_completion_for_test(&services, &mut executor, 0)
                .unwrap_or_else(|error| {
                    panic!("{row:?}: dispatch typed live Decision Apply: {error:?}")
                });
            assert_eq!(
                dispatched,
                super::super::ProductionCompletionDispatchV1::ApplyQueued {
                    ordinal: apply_ordinal,
                },
                "{row:?}: live Validate child must enter the dedicated Apply worker"
            );
            let (_, apply_service, _) = recovered_apply
                .as_ref()
                .expect("ValidatedApply retains its matching State/Kura service");
            planner_io.execute_one_recovered_decision_apply(
                &context,
                apply_service,
                std::sync::Arc::clone(&output_guard),
            );
            let completion = match services
                .take_next_lifecycle_completion()
                .expect("classify typed live Apply worker completion")
            {
                crate::sumeragi::v2_worker::LifecycleCompletionTakeV1::Apply(completion) => {
                    completion
                }
                other => {
                    drop(other);
                    panic!("{row:?}: typed live Apply lost physical completion priority");
                }
            };
            assert!(matches!(
                super::super::settle_applied_decision_apply_completion(
                    &mut owner,
                    &mut executor,
                    completion,
                ),
                Ok(super::super::ProductionRecoveredDecisionApplyCompletionV1::Applied)
            ));
            assert_eq!(
                Some(executor.runtime_queue_snapshot_for_test(now)),
                queued_apply_snapshot,
                "{row:?}: Apply completion must preserve runtime FIFO count and order"
            );
            crate::sumeragi::status::clear_v2_status();
        }

        let ledger_after = std::fs::read(&ledger_path)
            .unwrap_or_else(|error| panic!("{row:?}: read post-dispatch LedgerV1: {error}"));
        if row.is_busy() {
            assert_eq!(
                ledger_after, ledger_before,
                "{row:?}: reducer-fence parking is deliberately volatile"
            );
        } else {
            assert_ne!(
                ledger_after, ledger_before,
                "{row:?}: every terminal publication must fsync LedgerV1"
            );
        }
        let wal_after = std::fs::read(&wal_path)
            .unwrap_or_else(|error| panic!("{row:?}: read post-dispatch WAL: {error}"));
        if row.grows_safety_wal() {
            assert!(
                wal_after.len() > wal_before.len() && wal_after != wal_before,
                "{row:?}: Validate-to-Sign must append the safety WAL"
            );
        } else {
            assert_eq!(
                wal_after, wal_before,
                "{row:?}: only the Persist publication appends the safety WAL"
            );
        }
        assert!(
            !output_guard.restart_required(),
            "{row:?}: exact publication cannot trip fail-stop"
        );
        planner_io.detach(&mut services);
        drop(executor);
        drop(owner);
        assert_production_ready_validate_cold_open(
            row,
            &ledger_root,
            active_context,
            lease.ordinal(),
        );
        drop(adapter_directory);
        drop(_directory);
    }
    crate::sumeragi::status::clear_v2_status();
}

#[cfg(feature = "bls")]
#[test]
fn production_completion_dispatch_publishes_all_ready_validate_outcomes() {
    let handle = crate::sumeragi::sumeragi_thread_builder(
        "sumeragi-v2-production-ready-validate-dispatch-matrix",
    )
    .spawn(production_completion_dispatch_publishes_all_ready_validate_outcomes_fixture)
    .expect("spawn production Ready Validate dispatch matrix on the consensus stack");
    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}
