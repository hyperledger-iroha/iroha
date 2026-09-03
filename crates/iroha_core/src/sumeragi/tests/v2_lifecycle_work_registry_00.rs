#[cfg(feature = "bls")]
#[derive(Clone, Copy, Debug)]
enum ProductionReadyValidateDispatchRow {
    ValidatedBusy,
    LocalValidatedBusy,
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
    const ALL: [Self; 10] = [
        Self::ValidatedBusy,
        Self::LocalValidatedBusy,
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
            Self::ValidatedBusy | Self::LocalValidatedBusy => {
                ReadyDurableValidateAdapterPublicationKind::ValidatedBusy
            }
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
            | Self::LocalValidatedBusy
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
        matches!(
            self,
            Self::LocalValidatedBusy | Self::ValidatedInactive | Self::RejectedInactive
        )
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
        matches!(
            self,
            Self::ValidatedBusy | Self::LocalValidatedBusy | Self::RejectedBusy
        )
    }

    const fn certificate_phase(self) -> Option<wire::GlobalPhase> {
        match self {
            Self::ValidatedApply => Some(wire::GlobalPhase::Commit),
            Self::ValidatedPersist | Self::RejectedReport => Some(wire::GlobalPhase::Prepare),
            Self::ValidatedBusy
            | Self::LocalValidatedBusy
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
            | Self::LocalValidatedBusy
            | Self::ValidatedInactive
            | Self::ValidatedNoEffect
            | Self::RejectedBusy
            | Self::RejectedInactive
            | Self::RejectedNoEffect => None,
        }
    }

    fn expected_dispatch(
        self,
        ordinal: u128,
        reducer_fence_wait: Option<super::super::WaitToken>,
        successor_ordinal: Option<u128>,
    ) -> super::super::ProductionCompletionDispatchV1 {
        use super::super::ProductionCompletionDispatchV1 as Dispatch;

        match self {
            Self::ValidatedBusy | Self::LocalValidatedBusy | Self::RejectedBusy => {
                Dispatch::ReducerFenceWait {
                    ordinal,
                    wait: reducer_fence_wait
                        .expect("busy Ready Validate row retains its exact reducer fence"),
                }
            }
            Self::ValidatedInactive
            | Self::ValidatedNoEffect
            | Self::RejectedInactive
            | Self::RejectedNoEffect => Dispatch::ValidateNoSuccessor { ordinal },
            Self::ValidatedApply => Dispatch::BodyStageAdvanced {
                parent_ordinal: ordinal,
                child_ordinal: successor_ordinal
                    .expect("ValidatedApply reserves one actor-global successor"),
                child: LifecycleWorkClass::Apply,
            },
            Self::ValidatedPersist => Dispatch::BodyStageAdvanced {
                parent_ordinal: ordinal,
                child_ordinal: successor_ordinal
                    .expect("ValidatedPersist reserves one actor-global successor"),
                child: LifecycleWorkClass::SignVote,
            },
            Self::RejectedReport => Dispatch::BodyStageAdvanced {
                parent_ordinal: ordinal,
                child_ordinal: successor_ordinal
                    .expect("RejectedReport reserves one actor-global successor"),
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
    expected_successor_ordinal: Option<u128>,
    expected_apply_successor_broadcast_ordinal: Option<u128>,
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
        assert!(
            expected_apply_successor_broadcast_ordinal.is_none(),
            "{row:?}: a Busy Validate cannot retain a post-Apply Broadcast"
        );
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
            assert!(
                expected_apply_successor_broadcast_ordinal.is_none(),
                "{row:?}: a Validate without a successor cannot retain a post-Apply Broadcast"
            );
            assert_eq!(records.len(), 1, "{row:?}");
            assert_eq!(ledger.high_water(), parent_ordinal, "{row:?}");
            assert_eq!(
                parent.continuation(),
                Some(DurableContinuation::AdvancedNoSuccessor),
                "{row:?}"
            );
        }
        Some((edge, child_class)) => {
            let child_ordinal = expected_successor_ordinal
                .expect("successor publication sampled one actor-global ordinal");
            assert!(child_ordinal > parent_ordinal, "{row:?}");
            let apply_successor_broadcast_ordinal =
                if matches!(row, ProductionReadyValidateDispatchRow::ValidatedApply) {
                    Some(
                        expected_apply_successor_broadcast_ordinal
                            .expect("ValidatedApply retains its terminal periodic CommitQC row"),
                    )
                } else {
                    assert!(
                        expected_apply_successor_broadcast_ordinal.is_none(),
                        "{row:?}: only ValidatedApply can retain a post-Apply Broadcast"
                    );
                    None
                };
            assert_eq!(
                records.len(),
                2 + usize::from(apply_successor_broadcast_ordinal.is_some()),
                "{row:?}"
            );
            assert_eq!(
                ledger.high_water(),
                apply_successor_broadcast_ordinal.unwrap_or(child_ordinal),
                "{row:?}"
            );
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
            if let Some(broadcast_ordinal) = apply_successor_broadcast_ordinal {
                assert!(broadcast_ordinal > child_ordinal, "{row:?}");
                let broadcast = &records[2];
                assert_eq!(broadcast.ordinal(), broadcast_ordinal, "{row:?}");
                assert_eq!(
                    broadcast.work_class(),
                    Some(LifecycleWorkClass::Broadcast),
                    "{row:?}"
                );
                assert_eq!(
                    broadcast.key().map(|key| key.phase()),
                    Some(LifecyclePhase::BroadcastCommitQc),
                    "{row:?}"
                );
                assert_eq!(
                    broadcast.stage().map(|stage| stage.kind()),
                    Some(LifecycleStageKind::BroadcastCommitQc),
                    "{row:?}"
                );
                assert_eq!(
                    broadcast.terminal(),
                    Some(Some(TerminalOutcome::Advanced)),
                    "{row:?}"
                );
                assert_eq!(
                    broadcast.continuation(),
                    Some(DurableContinuation::None),
                    "{row:?}"
                );
            }
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
#[test]
fn ready_validate_rejects_foreign_physical_completion_key() {
    let OwnedReadyDurableValidateFixture {
        ready,
        coordinator,
        mut successor,
        ..
    } = owned_production_ready_validate_fixture(
        ProductionReadyValidateDispatchRow::ValidatedPersist,
        0xE1,
    );
    let attestation = coordinator
        .attest_ready_validate_demand(&ready.holder, ready.lease.ordinal())
        .expect("attest the exact Ready Validate carrier");
    let incumbent_digest = ready.fixture.lease.physical_slots()[&ready.fixture.slot];
    let incumbent_key = attestation
        .dispatch_key()
        .with_carrier_digest(incumbent_digest);
    successor.physical_completion = Some(
        crate::sumeragi::v2_worker::LifecycleValidatePhysicalCompletionV1::for_test(
            incumbent_key,
            std::time::Instant::now(),
        ),
    );
    assert!(
        successor.exactly_matches_ready_attestation(attestation),
        "the exact incumbent worker key must join its published replacement"
    );

    let foreign_digest = LifecycleDigest::new([0xFE; 32]);
    assert_ne!(foreign_digest, incumbent_digest);
    successor.physical_completion = Some(
        crate::sumeragi::v2_worker::LifecycleValidatePhysicalCompletionV1::for_test(
            incumbent_key.with_carrier_digest(foreign_digest),
            std::time::Instant::now(),
        ),
    );
    assert!(
        !successor.exactly_matches_ready_attestation(attestation),
        "a completion token for a foreign carrier digest must fail closed"
    );
}

#[cfg(feature = "bls")]
struct ProductionRecoveredApplyReadyFixture {
    owned: OwnedReadyDurableValidateFixture,
    commit_qc: wire::QuorumCertificate,
    retry_census: RecoveredDurableValidateRetryCensusV1,
}

#[cfg(feature = "bls")]
fn production_recovered_apply_ready_fixture(marker: u8) -> ProductionRecoveredApplyReadyFixture {
    let crate::sumeragi::v2_apply::ProductionRecoveredDecisionApplyFixtureV1 {
        verified,
        manifest,
        canonical_wire,
        body_store,
        durable,
        commit_qc,
        validator_keys,
        directory,
    } = crate::sumeragi::v2_apply::production_recovered_decision_apply_fixture_v1();
    let fixture =
        durable_validate_fixture_from_material(marker, verified, manifest, canonical_wire);
    assert_eq!(
        validator_keys.len(),
        fixture.verified.context().roster.len(),
        "recovered Apply fixture retains one signing key per validator"
    );
    assert!(
        validator_keys
            .iter()
            .zip(&fixture.verified.context().roster)
            .all(|(key, power)| {
                iroha_data_model::peer::PeerId::new(key.public_key().clone()) == power.validator
            }),
        "recovered Apply fixture keys must exactly match the ordered authenticated roster"
    );
    let store_fixture = durable_validate_store_fixture_from_existing(
        fixture,
        directory,
        body_store,
        durable,
        Some(commit_qc.execution_commitment),
    );
    let cold = ready_durable_validate_coordinator(&[&store_fixture.0]);
    let retry_census = store_fixture
        .0
        .registry
        .project_recovered_durable_validate_retry_census(
            &cold,
            Some((
                commit_qc.round,
                commit_qc.proposal_round,
                commit_qc.subject,
                commit_qc.execution_commitment,
            )),
        )
        .expect("project the exact pre-completion Validate retry owner");
    assert_eq!(retry_census.len_for_test(), 1);
    let waiting = waiting_durable_validate_fixture_from_store(store_fixture);
    let owned = owned_ready_durable_validate_fixture_from_waiting_with_commitment(
        waiting,
        ReadyDurableValidateFixtureOutcome::Validated,
        Some(commit_qc.execution_commitment),
    );
    ProductionRecoveredApplyReadyFixture {
        owned,
        commit_qc,
        retry_census,
    }
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
}

#[cfg(feature = "bls")]
#[allow(clippy::too_many_lines)]
fn production_completion_dispatch_publishes_all_ready_validate_outcomes_fixture() {
    let _status_guard = crate::sumeragi::status::rbc_status_test_guard();
    crate::sumeragi::status::clear_v2_status();
    let marker = production_ready_validate_dispatch_marker();
    for row in ProductionReadyValidateDispatchRow::ALL {
        let (owned, recovered_apply, recovered_validate_retry_census) =
            if matches!(row, ProductionReadyValidateDispatchRow::ValidatedApply) {
                let ProductionRecoveredApplyReadyFixture {
                    owned,
                    commit_qc,
                    retry_census,
                } = production_recovered_apply_ready_fixture(marker);
                (owned, Some(commit_qc), Some(retry_census))
            } else {
                (
                    owned_production_ready_validate_fixture(row, marker),
                    None,
                    None,
                )
            };
        let OwnedReadyDurableValidateFixture {
            mut ready,
            store,
            coordinator,
            successor,
            retry_owner,
        } = owned;
        assert_eq!(
            successor.lifecycle_ordinal(),
            ready.lease.ordinal(),
            "{row:?}: the publication successor must retain the Ready Validate row",
        );
        assert_eq!(
            retry_owner.lifecycle_ordinal(),
            ready.lease.ordinal(),
            "{row:?}: the retry owner must retain the Ready Validate row",
        );
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
        let AdapterEffect::ValidateBody {
            tag,
            round: validate_round,
            subject: validate_subject,
        } = &ready.fixture.effect
        else {
            unreachable!("Ready fixture retains one Validate effect")
        };
        let tag = *tag;
        let validate_round = *validate_round;
        let validate_subject = *validate_subject;
        let adapter_directory =
            TempDir::new().expect("temporary production Ready Validate adapter");
        let wal_path = adapter_directory.path().join("safety.wal");
        let (mut adapter, mut startup) = SumeragiV2Adapter::open(
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
                recovered_apply.as_ref(),
                &mut adapter,
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
            if matches!(row, ProductionReadyValidateDispatchRow::LocalValidatedBusy) {
                startup = sign;
            }
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
        let owner_directory = TempDir::new().expect("temporary Ready Validate owner");
        let active_context = coordinator.active_context;
        let (mut owner, runtime_ordinal_authority) =
            super::super::ProductionLifecycleOwnerV1::ready_validate_completion_owner_for_test(
                fixture.verified.clone(),
                coordinator,
                holder,
                store,
                owner_directory.path(),
            );
        let lifecycle_ordinals =
            crate::sumeragi::v2_runtime::RuntimeLifecycleOrdinalSource::from_authority(
                runtime_ordinal_authority,
            );
        let lifecycle_ordinal_observer = lifecycle_ordinals.clone();
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
        let retransmit_interval = runtime.retransmit_interval();
        let ledger_root = owner_directory.path().join("ledger");
        let ledger_path = ledger_root.join("lifecycle-ledger-v1.norito");
        let ledger_before = std::fs::read(&ledger_path)
            .unwrap_or_else(|error| panic!("{row:?}: read pre-dispatch LedgerV1: {error}"));
        let (mut services, _keys) = crate::sumeragi::v2_worker::tests::fixture();
        let output_guard = crate::sumeragi::output_guard::ConsensusOutputGuard::isolated();
        let (mut executor, mut planner_io) = if let Some(retry_census) =
            recovered_validate_retry_census
        {
            owner.bind_body_store_to_lifecycle_completion_io_with_validate_retry_census_for_test(
                &mut services,
                runtime,
                std::sync::Arc::clone(&output_guard),
                local_validator,
                2,
                retry_census,
            )
        } else {
            owner.bind_body_store_to_lifecycle_completion_io_for_test(
                &mut services,
                runtime,
                std::sync::Arc::clone(&output_guard),
                local_validator,
                2,
            )
        };
        let scenario = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            // Production retains this exact marker continuously from the
            // durable Store-to-Validate publication. This focused fixture
            // constructs its executor only after the volatile Validate worker
            // completion, so reinstall the same closed parent authority before
            // exercising any synchronous successor outcome.
            let work = owner
                .registry
                .registry_for_test()
                .entries
                .get(&fixture.address)
                .expect("Ready Validate fixture retains its exact completion carrier");
            let ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) = &work.kind
            else {
                panic!("Ready Validate fixture must retain a completed Validate parent")
            };
            let completion_incumbent_digest = completion.incumbent_digest;
            executor
                .install_recovered_published_lifecycle_validate_retry_marker(
                    &completion.incumbent.effect,
                    &completion.incumbent.pending,
                    &completion.incumbent.durable_receipt,
                    lease.ordinal(),
                )
                .expect("restore the exact long-lived Validate retry marker");
            assert_eq!(
                executor
                    .validate_retry_lifecycle_ordinal_for_test((validate_round, validate_subject,)),
                Some(Some(lease.ordinal())),
                "{row:?}: recovered retry authority must bind the Ready parent"
            );
            if matches!(row, ProductionReadyValidateDispatchRow::LocalValidatedBusy) {
                let keys = durable_store_keys(marker);
                let signer = usize::try_from(local_validator)
                    .expect("local Busy validator index is representable");
                crate::sumeragi::v2_worker::tests::install_local_signer_for_test(
                    &mut services,
                    &keys[signer],
                );
                assert_eq!(
                    executor
                        .consume_effects(returned_startup, &mut services)
                        .unwrap_or_else(|error| {
                            panic!("{row:?}: dispatch the real timeout Sign fence: {error}")
                        }),
                    1,
                    "{row:?}: the recovered timeout owns one physical Sign"
                );
                assert_eq!(executor.status().pending_signatures, 1, "{row:?}");
            } else {
                assert!(
                    returned_startup.is_empty(),
                    "{row:?}: only local Busy retains a startup Sign effect"
                );
            }
            executor
            .arm_live_clocks(
                crate::sumeragi::v2_lifecycle_coordinator::ProductionLifecycleLiveClockActivationPermitV1::for_test(),
                now,
            )
            .unwrap_or_else(|error| panic!("{row:?}: arm exact runtime clocks: {error}"));
            let validate_attestation = owner
                .coordinator
                .attest_ready_validate_demand(&owner.registry, lease.ordinal())
                .expect("attest the exact pre-publication Validate carrier");
            let replacement_dispatch_key = validate_attestation.dispatch_key();
            let incumbent_dispatch_key = replacement_dispatch_key
                .with_carrier_digest(completion_incumbent_digest);
            assert_ne!(
                incumbent_dispatch_key.digest(),
                replacement_dispatch_key.digest(),
                "{row:?}: publication must replace the same-address carrier digest"
            );
            executor
                .arm_live_lifecycle_validate_successor(
                    incumbent_dispatch_key,
                    None,
                    validate_round,
                    validate_subject,
                    true,
                )
                .expect("arm the exact sidecar-woken incumbent Validate owner");
            let publication_apply_is_authorized = matches!(
                row.fixture_outcome(),
                ReadyDurableValidateFixtureOutcome::Validated
            );
            let foreign_rollover = executor.arm_live_lifecycle_validate_successor(
                replacement_dispatch_key,
                Some(replacement_dispatch_key),
                validate_round,
                validate_subject,
                publication_apply_is_authorized,
            );
            assert!(
                foreign_rollover
                    .expect_err("a replacement key cannot authenticate itself as the incumbent")
                    .to_string()
                    .contains("a second Validate successor changed"),
                "{row:?}: foreign incumbent rollover must retain the fail-closed contract"
            );
            executor
                .arm_live_lifecycle_validate_successor(
                    replacement_dispatch_key,
                    Some(incumbent_dispatch_key),
                    validate_round,
                    validate_subject,
                    publication_apply_is_authorized,
                )
                .expect("roll the exact incumbent into its published Validate successor owner");
            if matches!(row, ProductionReadyValidateDispatchRow::ValidatedApply) {
                let commit_qc = recovered_apply
                    .as_ref()
                    .expect("ValidatedApply retains its exact production fixture");
                assert_eq!(
                    executor
                        .reconcile_reopened_decision_for_lifecycle_apply_lineage_test(
                            &mut services,
                            true,
                        )
                        .expect("import the adapter's exact durable Decision into the executor"),
                    (
                        commit_qc.round,
                        commit_qc.proposal_round,
                        commit_qc.subject,
                        commit_qc.execution_commitment,
                    ),
                    "{row:?}: executor protection must match the adapter's decided body"
                );
            }
            let queued_apply_snapshot =
                matches!(row, ProductionReadyValidateDispatchRow::ValidatedApply).then(|| {
                    let commit_qc = recovered_apply
                        .as_ref()
                        .expect("ValidatedApply retains its exact production fixture");
                    let queued_progress = wire::ConsensusMessageV2::new(
                        wire::ConsensusMessageV2Payload::QuorumCertificate(commit_qc.clone()),
                    );
                    executor
                        .enqueue_network(queued_progress)
                        .expect("queue authenticated runtime ingress beside typed live Apply");
                    let snapshot = executor.runtime_queue_snapshot_for_test(now);
                    assert_eq!(
                        snapshot.progress.depth, 1,
                        "ValidatedApply fixture retains one authentic Progress wire"
                    );
                    snapshot
                });
            let expected_reducer_fence_wait = row.is_busy().then(|| {
                let reducer_fence = executor.lifecycle_reducer_fence_observation();
                super::super::WaitToken::new(
                    super::super::reducer_fence_wait_source(active_context),
                    reducer_fence.generation(),
                )
            });
            let expected_successor_ordinal = row.successor().map(|_| {
                lifecycle_ordinal_observer
                    .next_ordinal_for_test()
                    .expect("inspect the paired actor-global ordinal source")
                    .expect("Ready Validate successor ordinal remains representable")
            });
            let dispatched = owner
                .dispatch_completion_for_test(&mut services, &mut executor, 0)
                .unwrap_or_else(|error| {
                    panic!("{row:?}: production Completion dispatch: {error:?}")
                });
            assert_eq!(
                dispatched,
                row.expected_dispatch(
                    lease.ordinal(),
                    expected_reducer_fence_wait,
                    expected_successor_ordinal,
                ),
                "{row:?}"
            );
            if matches!(row, ProductionReadyValidateDispatchRow::LocalValidatedBusy) {
                let parked_queue = executor.runtime_queue_snapshot_for_test(now);
                assert_eq!(
                    parked_queue.normal.depth, 0,
                    "{row:?}: local Busy publication cannot create ordinary ingress"
                );
                assert_eq!(
                    parked_queue.progress.depth, 0,
                    "{row:?}: local Busy publication cannot create progress ingress"
                );
                assert_eq!(
                    parked_queue.completion.depth, 1,
                    "{row:?}: local Busy publication retains one exact LocalProposalReady command"
                );
                let parked_status = executor.status();
                assert_eq!(parked_status.queued_runtime_completions, 1, "{row:?}");
                assert_eq!(parked_status.pending_stores, 0, "{row:?}");
                assert_eq!(parked_status.pending_validations, 0, "{row:?}");
                assert!(!parked_status.fail_closed, "{row:?}");
                assert!(!output_guard.restart_required(), "{row:?}");

                executor
                    .step(std::time::Instant::now(), &mut services)
                    .unwrap_or_else(|error| {
                        panic!("{row:?}: park LocalProposalReady behind the Sign fence: {error}")
                    });
                let initial_deferred_completion_depth = crate::sumeragi::status::v2_status()
                    .and_then(|status| {
                        status.liveness.queues.into_iter().find_map(|queue| {
                            (queue.queue == wire::SumeragiV2QueueKind::DeferredCompletion)
                                .then_some(queue.depth)
                        })
                    })
                    .unwrap_or(0);
                assert_eq!(
                    initial_deferred_completion_depth, 1,
                    "{row:?}: LocalProposalReady must enter DeferredCompletion before Sign service"
                );
                planner_io.execute_one_consensus_sign_fixture(&services);

                let mut wait = expected_reducer_fence_wait
                    .expect("local Busy retains its exact initial reducer fence");
                let completion_deadline = std::time::Instant::now()
                    .checked_add(std::time::Duration::from_secs(5))
                    .expect("local Busy completion deadline is representable");
                let resolved = loop {
                    loop {
                        services
                            .drain_completions(&mut executor)
                            .unwrap_or_else(|error| {
                                panic!("{row:?}: drain the real Sign worker completion: {error}")
                            });
                        executor
                            .step(std::time::Instant::now(), &mut services)
                            .unwrap_or_else(|error| {
                                panic!("{row:?}: advance the fenced serialized runtime: {error}")
                            });
                        let fence = executor.lifecycle_reducer_fence_observation();
                        let status = executor.status();
                        let deferred_completion_depth = crate::sumeragi::status::v2_status()
                            .and_then(|status| {
                                status.liveness.queues.into_iter().find_map(|queue| {
                                    (queue.queue == wire::SumeragiV2QueueKind::DeferredCompletion)
                                        .then_some(queue.depth)
                                })
                            })
                            .unwrap_or(0);
                        if fence.source() == wait.source()
                            && fence.generation() > wait.observed_generation()
                            && status.pending_signatures == 0
                            && status.queued_runtime_completions == 0
                            && deferred_completion_depth == 0
                        {
                            break;
                        }
                        assert!(!status.fail_closed, "{row:?}");
                        assert!(!output_guard.restart_required(), "{row:?}");
                        if std::time::Instant::now() >= completion_deadline {
                            panic!(
                                "{row:?}: timed out draining the Sign/fence bridge: \
                             fence={fence:?}, wait={wait:?}, status={status:?}"
                            );
                        }
                        std::thread::yield_now();
                    }

                    let next = owner
                        .dispatch_completion_for_test(&mut services, &mut executor, 0)
                        .unwrap_or_else(|error| {
                            panic!("{row:?}: retry the exact same-ordinal Validate: {error:?}")
                        });
                    match next {
                        super::super::ProductionCompletionDispatchV1::ReducerFenceWait {
                            ordinal,
                            wait: next_wait,
                        } => {
                            assert_eq!(ordinal, lease.ordinal(), "{row:?}");
                            assert_eq!(next_wait.source(), wait.source(), "{row:?}");
                            assert!(
                                next_wait.observed_generation() > wait.observed_generation(),
                                "{row:?}: a repeated Busy must bind a newly advanced fence"
                            );
                            wait = next_wait;
                        }
                        resolved => break resolved,
                    }
                };
                assert_eq!(
                    resolved,
                    super::super::ProductionCompletionDispatchV1::ValidateNoSuccessor {
                        ordinal: lease.ordinal(),
                    },
                    "{row:?}: the exact same-ordinal successor must resolve after Sign completion"
                );
                assert!(
                    matches!(
                        owner.coordinator.records[&lease.ordinal()].state,
                        super::super::LifecycleState::Terminal(_)
                    ),
                    "{row:?}: the parked Validate parent must terminalize exactly once"
                );
                let settled_queue =
                    executor.runtime_queue_snapshot_for_test(std::time::Instant::now());
                assert_eq!(settled_queue.completion.depth, 0, "{row:?}");
                let settled_status = executor.status();
                assert_eq!(settled_status.queued_runtime_completions, 0, "{row:?}");
                assert_eq!(settled_status.pending_signatures, 0, "{row:?}");
                assert!(!settled_status.fail_closed, "{row:?}");
                assert!(!output_guard.restart_required(), "{row:?}");
            }
            let expected_retry_owner = match row {
                ProductionReadyValidateDispatchRow::ValidatedBusy
                | ProductionReadyValidateDispatchRow::RejectedBusy => Some(Some(lease.ordinal())),
                ProductionReadyValidateDispatchRow::LocalValidatedBusy
                | ProductionReadyValidateDispatchRow::ValidatedInactive
                | ProductionReadyValidateDispatchRow::ValidatedNoEffect
                | ProductionReadyValidateDispatchRow::RejectedInactive
                | ProductionReadyValidateDispatchRow::RejectedNoEffect => Some(None),
                ProductionReadyValidateDispatchRow::ValidatedApply
                | ProductionReadyValidateDispatchRow::ValidatedPersist
                | ProductionReadyValidateDispatchRow::RejectedReport => None,
            };
            assert_eq!(
                executor
                    .validate_retry_lifecycle_ordinal_for_test((validate_round, validate_subject,)),
                expected_retry_owner,
                "{row:?}: retry state must distinguish an active owner, a released tombstone, and successor-consumed absence"
            );
            let mut expected_apply_successor_broadcast_ordinal = None;
            if let Some(child_ordinal) = expected_successor_ordinal {
                assert_eq!(owner.coordinator.high_water(), child_ordinal, "{row:?}");
                assert!(
                    owner.coordinator.records.contains_key(&child_ordinal),
                    "{row:?}: sampled actor-global successor must be the installed child"
                );
                for runtime_ordinal in lease.ordinal() + 1..child_ordinal {
                    assert!(
                        !owner.coordinator.records.contains_key(&runtime_ordinal),
                        "{row:?}: runtime-owned actor-global ordinals cannot enter LedgerV1"
                    );
                }
                assert_eq!(
                    owner
                        .coordinator
                        .ready_index
                        .iter()
                        .copied()
                        .collect::<Vec<_>>(),
                    vec![child_ordinal],
                    "{row:?}: the published successor must be the sole Ready lifecycle owner"
                );
            }
            if matches!(row, ProductionReadyValidateDispatchRow::ValidatedApply) {
                let apply_ordinal = expected_successor_ordinal
                    .expect("Validate-to-Apply sampled its actor-global child ordinal");
                let apply_attestation = owner
                    .registry
                    .attest_ready_lifecycle_decision_apply(&owner.coordinator, apply_ordinal)
                    .expect("attest the exact sampled live Apply child");
                assert_eq!(
                    apply_attestation.dispatch_key().lifecycle_ordinal(),
                    apply_ordinal,
                    "{row:?}: registry Apply authority must bind the sampled shared ordinal"
                );
                let blocked = owner
                    .dispatch_completion_for_test(&mut services, &mut executor, 0)
                    .unwrap_or_else(|error| {
                        panic!("{row:?}: classify live Apply behind runtime ingress: {error:?}")
                    });
                assert_eq!(
                    blocked,
                    super::super::ProductionCompletionDispatchV1::CapacityUnavailable {
                        protected_live_apply_ordinal: Some(apply_ordinal),
                    },
                    "{row:?}: live Apply must retain its exact Ready owner until the finite runtime FIFO drains"
                );
                assert_eq!(
                    Some(executor.runtime_queue_snapshot_for_test(now)),
                    queued_apply_snapshot,
                    "{row:?}: the blocked Apply probe cannot consume or reorder runtime ingress"
                );
                assert!(matches!(
                    owner.coordinator.records[&apply_ordinal].state,
                    super::super::LifecycleState::Ready
                ));

                executor.step(now, &mut services).unwrap_or_else(|error| {
                    panic!("{row:?}: drain the pre-Apply authenticated runtime command: {error}")
                });
                let drained_queue = executor.runtime_queue_snapshot_for_test(now);
                assert_eq!(drained_queue.progress.depth, 0, "{row:?}");
                assert_eq!(
                    executor.status().queued_runtime_completions,
                    0,
                    "{row:?}: normal Runtime must settle the finite pre-Apply FIFO"
                );

                let periodic_due = now
                    .checked_add(retransmit_interval)
                    .expect("ValidatedApply periodic deadline remains representable");
                let periodic_step =
                    executor
                        .step(periodic_due, &mut services)
                        .unwrap_or_else(|error| {
                            panic!(
                                "{row:?}: execute the exact decided-body periodic retry: {error}"
                            )
                        });
                assert!(
                    matches!(
                        periodic_step,
                        crate::sumeragi::v2_effects::EffectExecutorStep::Advanced { effects: 1 }
                    ),
                    "{row:?}: periodic [CommitQC Broadcast, Apply] must park its first effect and retain its second: {periodic_step:?}"
                );
                let periodic_observation = executor
                    .last_runtime_step_observation_for_test()
                    .expect("ValidatedApply periodic step retains its raw reducer observation");
                assert_eq!(
                    periodic_observation.selected(),
                    Some(crate::sumeragi::v2_runtime::RuntimeSelectedOwnerKind::PeriodicTimer),
                    "{row:?}"
                );
                assert_eq!(periodic_observation.effect_count(), 2, "{row:?}");
                assert_eq!(periodic_observation.validate_count(), 0, "{row:?}");
                assert_eq!(
                    periodic_observation.non_validate_class(),
                    Some(crate::sumeragi::v2_effects::RuntimeEffectClassV1::Multiple),
                    "{row:?}: the real periodic retry must retain its two distinct output/Apply effects"
                );
                let periodic_status = executor.status();
                assert_eq!(periodic_status.pending_outputs, 1, "{row:?}");
                assert_eq!(
                    periodic_status.effect_dispatch_queue.depth, 1,
                    "{row:?}: the exact retransmit Apply remains at the retained FIFO head"
                );

                let generic_settlement = executor
                    .settle_pending_lifecycle_output_admissions(&mut owner, &mut services)
                    .unwrap_or_else(|error| {
                        panic!("{row:?}: generically admit the periodic CommitQC output: {error}")
                    });
                assert_eq!(generic_settlement.newly_completed(), 0, "{row:?}");
                assert_eq!(generic_settlement.already_completed(), 0, "{row:?}");
                assert!(
                    executor.has_pending_lifecycle_output_admissions(),
                    "{row:?}: generic output service must defer behind the globally earlier Apply"
                );
                let mut exact_commit_qc_broadcasts =
                    owner.coordinator.records.iter().filter(|(_, record)| {
                        record.work_class == LifecycleWorkClass::Broadcast
                            && record.key.phase() == LifecyclePhase::BroadcastCommitQc
                            && record.stage.kind() == LifecycleStageKind::BroadcastCommitQc
                    });
                let (&broadcast_ordinal, broadcast_record) = exact_commit_qc_broadcasts
                    .next()
                    .expect("periodic CommitQC generic admission installs one exact Broadcast row");
                assert!(
                    exact_commit_qc_broadcasts.next().is_none(),
                    "{row:?}: the periodic episode installs one CommitQC Broadcast carrier"
                );
                assert!(broadcast_ordinal > apply_ordinal, "{row:?}");
                assert_eq!(
                    broadcast_record.state,
                    super::super::LifecycleState::Ready,
                    "{row:?}"
                );
                assert_eq!(
                    owner
                        .coordinator
                        .ready_index
                        .iter()
                        .copied()
                        .collect::<Vec<_>>(),
                    vec![apply_ordinal, broadcast_ordinal],
                    "{row:?}: lifecycle order must keep Apply ahead of its periodic CommitQC output"
                );
                expected_apply_successor_broadcast_ordinal = Some(broadcast_ordinal);
                let commit_qc = recovered_apply
                    .as_ref()
                    .expect("ValidatedApply retains its exact CommitQC");
                let commit_qc_envelope = wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::QuorumCertificate(commit_qc.clone()),
                );
                assert_eq!(
                    services.consensus_broadcast_count_for_test(&commit_qc_envelope),
                    0,
                    "{row:?}: ordinal deferral must precede CommitQC service I/O"
                );

                let dispatched = owner
                    .dispatch_completion_for_test(&mut services, &mut executor, 0)
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
                assert_eq!(
                    executor.status().effect_dispatch_queue.depth,
                    0,
                    "{row:?}: scheduler dispatch consumes only the byte-exact retransmit Apply suffix"
                );
                planner_io.execute_one_lifecycle_decision_apply_fixture(std::sync::Arc::clone(
                    &output_guard,
                ));
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
                    super::super::settle_applied_live_lifecycle_decision_apply_completion_for_test(
                        &mut owner,
                        &mut executor,
                        completion,
                    ),
                    Ok(super::super::ProductionLifecycleDecisionApplyCompletionV1::Applied)
                ));
                assert!(
                    !executor.ready_to_finish(),
                    "{row:?}: the attested post-Apply Broadcast must remain a rollover blocker"
                );
                let post_apply_blockers = executor.ready_to_finish_blockers();
                assert!(
                    post_apply_blockers.contains(&"lifecycle-output-admission")
                        && post_apply_blockers.contains(&"post-apply-output-census"),
                    "{row:?}: terminal Apply lost its exact successor-output blockers: {post_apply_blockers:?}"
                );
                assert!(matches!(
                    owner.coordinator.records[&apply_ordinal].state,
                    super::super::LifecycleState::Terminal(TerminalOutcome::Advanced)
                ));
                assert_eq!(
                    owner.coordinator.records[&broadcast_ordinal].state,
                    super::super::LifecycleState::Ready,
                    "{row:?}"
                );

                services.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
                let prepared_broadcast = owner
                    .prepare_apply_terminal_direct_broadcast()
                    .expect("bind the exact post-Apply CommitQC Broadcast carrier");
                assert_eq!(prepared_broadcast.ordinal(), broadcast_ordinal, "{row:?}");
                assert_eq!(
                executor
                    .settle_apply_terminal_direct_broadcast(
                        &mut owner,
                        &mut services,
                        prepared_broadcast,
                    )
                    .unwrap_or_else(|error| {
                        panic!("{row:?}: settle the dedicated post-Apply Broadcast: {error}")
                    }),
                crate::sumeragi::v2_effects::ProductionApplyTerminalDirectBroadcastSettlementV1::Completed,
                "{row:?}"
            );
                assert_eq!(
                    services.consensus_broadcast_count_for_test(&commit_qc_envelope),
                    1,
                    "{row:?}: dedicated settlement services the deferred CommitQC exactly once"
                );
                assert!(!executor.has_pending_lifecycle_output_admissions());
                assert!(matches!(
                    owner.coordinator.records[&broadcast_ordinal].state,
                    super::super::LifecycleState::Terminal(TerminalOutcome::Advanced)
                ));
                assert!(owner.coordinator.ready_index.is_empty(), "{row:?}");
                assert!(owner.coordinator.active_lease.is_none(), "{row:?}");
                assert!(
                    executor.ready_to_finish(),
                    "{row:?}: settled post-Apply Broadcast retained rollover blockers: {:?}",
                    executor.ready_to_finish_blockers()
                );
                crate::sumeragi::status::clear_v2_status();
            }

            let ledger_after = std::fs::read(&ledger_path)
                .unwrap_or_else(|error| panic!("{row:?}: read post-dispatch LedgerV1: {error}"));
            if row.is_busy()
                && !matches!(row, ProductionReadyValidateDispatchRow::LocalValidatedBusy)
            {
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
            if matches!(row, ProductionReadyValidateDispatchRow::LocalValidatedBusy) {
                assert!(
                    wal_after.starts_with(&wal_before),
                    "{row:?}: real Sign settlement may only append to the safety WAL"
                );
            } else if row.grows_safety_wal() {
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
            (
                expected_successor_ordinal,
                expected_apply_successor_broadcast_ordinal,
            )
        }));
        planner_io.detach(&mut services);
        let (expected_successor_ordinal, expected_apply_successor_broadcast_ordinal) =
            match scenario {
                Ok(ordinals) => ordinals,
                Err(payload) => std::panic::resume_unwind(payload),
            };
        drop(executor);
        drop(owner);
        if matches!(row, ProductionReadyValidateDispatchRow::LocalValidatedBusy) {
            let (_store, ledger) =
                super::super::ledger::LifecycleLedgerStoreV1::open(&ledger_root, active_context)
                    .unwrap_or_else(|error| panic!("{row:?}: cold-open settled LedgerV1: {error}"));
            let parent = ledger
                .records()
                .first()
                .unwrap_or_else(|| panic!("{row:?}: settled LedgerV1 retains its parent"));
            assert_eq!(parent.ordinal(), lease.ordinal(), "{row:?}");
            assert_eq!(
                parent.terminal(),
                Some(Some(TerminalOutcome::Advanced)),
                "{row:?}: the real fence bridge durably advances its parent"
            );
        } else {
            assert_production_ready_validate_cold_open(
                row,
                &ledger_root,
                active_context,
                lease.ordinal(),
                expected_successor_ordinal,
                expected_apply_successor_broadcast_ordinal,
            );
        }
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
