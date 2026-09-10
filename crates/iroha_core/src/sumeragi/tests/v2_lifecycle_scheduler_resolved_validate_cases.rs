/// Immutable production terminal identity observed by the retry integration regression.
pub(in crate::sumeragi) struct ResolvedValidateOwnerSnapshotForTest {
    record: super::LifecycleRecord,
    metadata: super::schema::DurableRecordMetadata,
    ledger_record: Vec<u8>,
}

impl ProductionLifecycleOwnerV1 {
    /// Observe the single physical slot before and after the genuine worker publication.
    pub(in crate::sumeragi) fn validate_slot_digest_for_retry_test(
        &self,
        ordinal: u128,
    ) -> super::LifecycleDigest {
        let record = &self.coordinator.records[&ordinal];
        assert_eq!(record.work_class, LifecycleWorkClass::Validate);
        assert_eq!(record.physical_slots.len(), 1);
        *record
            .physical_slots
            .values()
            .next()
            .expect("the exact Validate slot")
    }

    /// Authenticate and retain the actual no-successor row, including its result-bound digest.
    pub(in crate::sumeragi) fn resolved_validate_owner_snapshot_for_test(
        &self,
        ordinal: u128,
        pending_digest: super::LifecycleDigest,
        root: &std::path::Path,
    ) -> ResolvedValidateOwnerSnapshotForTest {
        use norito::codec::Encode as _;
        let record = self.coordinator.records[&ordinal].clone();
        let metadata = self.coordinator.durable_records[&ordinal].clone();
        assert_eq!(
            record.state,
            LifecycleState::Terminal(super::TerminalOutcome::Advanced)
        );
        assert_eq!(
            metadata.continuation,
            super::schema::DurableContinuation::AdvancedNoSuccessor
        );
        assert_ne!(
            self.validate_slot_digest_for_retry_test(ordinal),
            pending_digest,
            "the fixture must retain the real result digest, not the original pending identity"
        );
        assert_eq!(self.coordinator.key_index.get(&record.key), Some(&ordinal));
        assert_eq!(
            self.coordinator
                .owner_index
                .get(&record.owner.causal_root()),
            Some(&record.owner)
        );
        assert!(
            self.registry
                .registry_for_test()
                .finalization_entry_kind_census()
                .1
                .iter()
                .all(|(entry, _)| *entry != ordinal)
        );
        assert!(self.all_live_registry_census_is_exact_for_test());
        let (_, ledger) =
            super::ledger::LifecycleLedgerStoreV1::open(root, self.coordinator.active_context)
                .expect("cold-open the actual terminal LedgerV1");
        let ledger_record = ledger
            .records()
            .iter()
            .find(|row| row.ordinal() == ordinal)
            .expect("the terminal row remains in durable recovery")
            .encode();
        ResolvedValidateOwnerSnapshotForTest {
            record,
            metadata,
            ledger_record,
        }
    }

    /// Return actual Apply rows rather than the executor's pending-admission projection.
    pub(in crate::sumeragi) fn apply_ordinals_for_retry_test(&self) -> Vec<u128> {
        self.coordinator
            .records
            .values()
            .filter(|row| row.work_class == LifecycleWorkClass::Apply)
            .map(|row| row.ordinal)
            .collect()
    }

    /// Prove publication preserved the immutable terminal and cold-reconstructible Apply census.
    pub(in crate::sumeragi) fn assert_resolved_validate_owner_retained_for_test(
        &self,
        snapshot: &ResolvedValidateOwnerSnapshotForTest,
        root: &std::path::Path,
        applies: usize,
    ) {
        use norito::codec::Encode as _;
        let ordinal = snapshot.record.ordinal;
        assert_eq!(self.coordinator.records[&ordinal], snapshot.record);
        assert_eq!(
            self.coordinator.durable_records[&ordinal],
            snapshot.metadata
        );
        assert_eq!(
            self.coordinator.key_index.get(&snapshot.record.key),
            Some(&ordinal)
        );
        assert_eq!(
            self.coordinator
                .owner_index
                .get(&snapshot.record.owner.causal_root()),
            Some(&snapshot.record.owner)
        );
        assert!(self.all_live_registry_census_is_exact_for_test());
        assert!(self.coordinator.fault.is_none());
        assert!(self.coordinator.active_lease.is_none());
        assert_eq!(self.apply_ordinals_for_retry_test().len(), applies);
        let (_, ledger) =
            super::ledger::LifecycleLedgerStoreV1::open(root, self.coordinator.active_context)
                .expect("cold-open the actual terminal and successor rows");
        assert_eq!(
            ledger
                .records()
                .iter()
                .find(|row| row.ordinal() == ordinal)
                .expect("the old Validate tombstone remains durable")
                .encode(),
            snapshot.ledger_record
        );
        assert_eq!(
            ledger
                .records()
                .iter()
                .filter(|row| row.work_class() == Some(LifecycleWorkClass::Apply))
                .count(),
            applies
        );
    }
}

impl ProductionLifecycleOwnerV1 {
    /// Substitute only the terminal outcome slot to test exact proof rejection before publication.
    pub(in crate::sumeragi) fn substitute_resolved_validate_digest_for_test(
        &mut self,
        snapshot: &ResolvedValidateOwnerSnapshotForTest,
        corrupt: bool,
    ) {
        let record = self
            .coordinator
            .records
            .get_mut(&snapshot.record.ordinal)
            .expect("the terminal row remains installed");
        assert_eq!(record.key, snapshot.record.key);
        record.physical_slots = snapshot.record.physical_slots.clone();
        if corrupt {
            for digest in record.physical_slots.values_mut() {
                let replacement = super::LifecycleDigest::new([0xAD; 32]);
                assert_ne!(*digest, replacement);
                *digest = replacement;
            }
        }
    }
}

impl ProductionLifecycleOwnerV1 {
    /// Prove the actual later Commit Validate addresses the retained terminal key.
    pub(in crate::sumeragi) fn assert_resolved_validate_key_collision_for_test(
        &self,
        snapshot: &ResolvedValidateOwnerSnapshotForTest,
        tag: crate::sumeragi::v2_core::EventTag,
        certificate: &iroha_data_model::block::consensus_v2::QuorumCertificate,
    ) {
        use crate::sumeragi::v2_runtime::{
            RuntimeEffectOwnership, bind_adapter_effect_batch_ownership,
        };
        let fetch = crate::sumeragi::v2::AdapterEffect::FetchBody {
            tag,
            round: certificate.proposal_round,
            subject: certificate.subject,
            manifest: None,
            certified_sources: self
                .verified
                .context()
                .roster
                .iter()
                .map(|row| row.validator.clone())
                .collect(),
            certificate: Some(certificate.clone()),
        };
        let validate = crate::sumeragi::v2::AdapterEffect::ValidateBody {
            tag,
            round: certificate.proposal_round,
            subject: certificate.subject,
        };
        // This is only an inert shape projection. It is never admitted and
        // cannot manufacture the terminal result or its publication authority.
        let ownership = bind_adapter_effect_batch_ownership(
            core::slice::from_ref(&fetch),
            vec![RuntimeEffectOwnership::fresh_for_test(tag, 9_084)],
        )
        .expect("bind the later Commit shape")
        .pop()
        .expect("one inert shape")
        .rebind_as_inherited_adapter_effect(&validate)
        .expect("inherit exact Commit coordinates");
        let binding = ownership
            .exact_pending_adapter_effect_binding(&validate)
            .expect("bind exact Validate shape");
        let projected = super::projection::authority_free_admission_projection(
            self.coordinator.active_context,
            &self.verified,
            &validate,
            &binding,
        )
        .expect("project the actual later Commit Validate");
        assert_eq!(
            projected.key, snapshot.record.key,
            "this regression must cross the real terminal-key collision, not a distinct Validate identity"
        );
        assert_eq!(
            self.coordinator.key_index.get(&projected.key),
            Some(&snapshot.record.ordinal)
        );
    }
}

impl ProductionLifecycleOwnerV1 {
    /// Publish one actual worker result and retain its physical successor authority.
    pub(in crate::sumeragi) fn publish_validate_successor_for_retry_test(
        &mut self,
        completion: crate::sumeragi::v2_worker::PreparedLifecycleValidateCompletionV1,
        ordinal: u128,
        rejected: bool,
    ) -> super::ReadyValidateSuccessorV1 {
        let (executed, ack) = completion.into_publication_parts();
        let publication = self
            .coordinator
            .complete_durable_validate_dispatch(&mut self.registry, executed)
            .expect("publish the genuine physical Validate result");
        let physical = ack.physical_completion();
        let successor = match publication {
            super::DurableValidateCompletionPublication::PublishedValidated(published) => {
                assert!(!rejected);
                assert_eq!(published.lifecycle_ordinal(), ordinal);
                super::ReadyValidateSuccessorV1::from_validated(published, physical)
            }
            super::DurableValidateCompletionPublication::PublishedRejected(published) => {
                assert!(rejected);
                assert_eq!(published.lifecycle_ordinal(), ordinal);
                super::ReadyValidateSuccessorV1::from_rejected(published, physical)
            }
            _ => panic!("the held worker must publish success or deterministic rejection"),
        };
        ack.acknowledge_after_publication();
        successor
    }
}

impl ProductionLifecycleOwnerV1 {
    /// Observe actual durable invalid-body report owners after cached rejection replay.
    pub(in crate::sumeragi) fn invalid_body_report_ordinals_for_retry_test(&self) -> Vec<u128> {
        self.coordinator
            .records
            .values()
            .filter(|row| row.work_class == LifecycleWorkClass::InvalidBodyReport)
            .map(|row| row.ordinal)
            .collect()
    }
}

impl ProductionLifecycleOwnerV1 {
    /// Compare the exact persisted terminal across real recovery while allowing
    /// canonical removal of its process-local physical episode and slots.
    pub(in crate::sumeragi) fn resolved_validate_cold_snapshot_for_test(
        &self,
        live: &ResolvedValidateOwnerSnapshotForTest,
        root: &std::path::Path,
    ) -> ResolvedValidateOwnerSnapshotForTest {
        use norito::codec::Encode as _;
        let ordinal = live.record.ordinal;
        let record = &self.coordinator.records[&ordinal];
        assert_eq!(
            (
                record.key,
                record.owner,
                record.ordinal,
                record.work_class,
                record.stage,
                record.state
            ),
            (
                live.record.key,
                live.record.owner,
                live.record.ordinal,
                live.record.work_class,
                live.record.stage,
                live.record.state
            )
        );
        assert!(
            record.physical_slots.is_empty(),
            "cold terminal rows own no physical callback"
        );
        assert_eq!(self.coordinator.durable_records[&ordinal], live.metadata);
        assert_eq!(self.coordinator.key_index.get(&record.key), Some(&ordinal));
        assert_eq!(
            self.coordinator
                .owner_index
                .get(&record.owner.causal_root()),
            Some(&record.owner)
        );
        assert!(!self.coordinator.ready_index.contains(&ordinal));
        assert!(
            self.registry
                .registry_for_test()
                .finalization_entry_kind_census()
                .1
                .iter()
                .all(|(entry, _)| *entry != ordinal)
        );
        self.assert_recovered_output_and_registry_census_for_retry_test();
        let (_, ledger) =
            super::ledger::LifecycleLedgerStoreV1::open(root, self.coordinator.active_context)
                .expect("cold-open the actual recovered terminal ledger");
        let row = ledger
            .records()
            .iter()
            .find(|row| row.ordinal() == ordinal)
            .expect("same terminal ordinal");
        assert_eq!(row.encode(), live.ledger_record);
        ResolvedValidateOwnerSnapshotForTest {
            record: record.clone(),
            metadata: live.metadata.clone(),
            ledger_record: live.ledger_record.clone(),
        }
    }
}

/// Exact durable report identity observed before closing its real production owner.
pub(in crate::sumeragi) struct InvalidBodyReportOwnerSnapshotForTest {
    record: super::LifecycleRecord,
    metadata: super::schema::DurableRecordMetadata,
    ledger_record: Vec<u8>,
}

impl ProductionLifecycleOwnerV1 {
    /// Reuse the complete cold-start census, including separately owned output carriers.
    fn assert_recovered_output_and_registry_census_for_retry_test(&self) {
        let outputs = self
            .exact_lifecycle_output_ordinals_for_registry_census()
            .expect("every retained cold output authenticates its exact Ready row");
        let registry = self.registry.registry_for_test();
        assert!(
            registry.exactly_covers_recovered_ready_work_with_owner_held_outputs(
                &self.coordinator,
                &outputs,
            ) || registry
                .exactly_covers_recovered_ready_work_and_wal_authority_with_owner_held_outputs(
                    &self.coordinator,
                    &outputs,
                )
        );
    }

    /// Capture the actual report admitted by the completed rejection replay.
    pub(in crate::sumeragi) fn invalid_body_report_snapshot_for_retry_test(
        &self,
        ordinal: u128,
        root: &std::path::Path,
    ) -> InvalidBodyReportOwnerSnapshotForTest {
        use norito::codec::Encode as _;
        let record = self.coordinator.records[&ordinal].clone();
        assert_eq!(record.work_class, LifecycleWorkClass::InvalidBodyReport);
        assert_eq!(record.state, LifecycleState::Ready);
        assert!(self.coordinator.ready_index.contains(&ordinal));
        assert_eq!(
            self.registry
                .registry_for_test()
                .finalization_entry_kind_census()
                .1
                .iter()
                .filter(|(entry, _)| *entry == ordinal)
                .count(),
            1
        );
        assert!(self.all_live_registry_census_is_exact_for_test());
        let (_, ledger) =
            super::ledger::LifecycleLedgerStoreV1::open(root, self.coordinator.active_context)
                .expect("read the actual report ledger");
        let ledger_record = ledger
            .records()
            .iter()
            .find(|row| row.ordinal() == ordinal)
            .expect("the admitted report is durable")
            .encode();
        InvalidBodyReportOwnerSnapshotForTest {
            record,
            metadata: self.coordinator.durable_records[&ordinal].clone(),
            ledger_record,
        }
    }

    /// Prove restart retains one executable cold output and no duplicate live carrier.
    pub(in crate::sumeragi) fn assert_invalid_body_report_recovered_for_retry_test(
        &self,
        snapshot: &InvalidBodyReportOwnerSnapshotForTest,
        root: &std::path::Path,
    ) {
        use norito::codec::Encode as _;
        let ordinal = snapshot.record.ordinal;
        let record = &self.coordinator.records[&ordinal];
        assert_eq!(
            (
                record.key,
                record.owner,
                record.ordinal,
                record.work_class,
                record.stage,
                record.state
            ),
            (
                snapshot.record.key,
                snapshot.record.owner,
                snapshot.record.ordinal,
                snapshot.record.work_class,
                snapshot.record.stage,
                snapshot.record.state
            )
        );
        assert_eq!(
            self.coordinator.durable_records[&ordinal],
            snapshot.metadata
        );
        assert_eq!(self.coordinator.key_index.get(&record.key), Some(&ordinal));
        assert_eq!(
            self.coordinator
                .owner_index
                .get(&record.owner.causal_root()),
            Some(&record.owner)
        );
        assert!(self.coordinator.ready_index.contains(&ordinal));
        let outputs = self
            .exact_lifecycle_output_ordinals_for_registry_census()
            .expect("the cold output carrier authenticates its exact row and Ready geometry");
        assert!(outputs.contains(&ordinal));
        assert_eq!(
            outputs
                .iter()
                .filter(|candidate| {
                    self.coordinator.records[*candidate].work_class
                        == LifecycleWorkClass::InvalidBodyReport
                })
                .count(),
            1,
        );
        assert!(
            self.registry
                .registry_for_test()
                .finalization_entry_kind_census()
                .1
                .iter()
                .all(|(entry, _)| *entry != ordinal)
        );
        self.assert_recovered_output_and_registry_census_for_retry_test();
        assert!(self.coordinator.fault.is_none());
        assert!(self.coordinator.active_lease.is_none());
        let (_, ledger) =
            super::ledger::LifecycleLedgerStoreV1::open(root, self.coordinator.active_context)
                .expect("read the report after semantic cold recovery");
        assert_eq!(
            ledger
                .records()
                .iter()
                .find(|row| row.ordinal() == ordinal)
                .expect("the same report remains durable")
                .encode(),
            snapshot.ledger_record
        );
    }
}

/// Exact WAL-owned standalone Commit Sign admitted from one retained terminal result.
pub(in crate::sumeragi) struct ResolvedCommitSignOwnerSnapshotForTest {
    record: super::LifecycleRecord,
    metadata: super::schema::DurableRecordMetadata,
    ledger_record: Vec<u8>,
}

impl ProductionLifecycleOwnerV1 {
    /// Observe the actual current Commit Sign row and its canonical WAL reconstruction root.
    pub(in crate::sumeragi) fn resolved_commit_sign_snapshot_for_test(
        &self,
        prepare: &iroha_data_model::block::consensus_v2::QuorumCertificate,
        root: &std::path::Path,
    ) -> ResolvedCommitSignOwnerSnapshotForTest {
        use norito::codec::Encode as _;
        let rows = self
            .coordinator
            .records
            .values()
            .filter(|row| row.work_class == LifecycleWorkClass::SignVote)
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 1, "one actual lifecycle vote-sign owner");
        let record = rows[0].clone();
        assert_eq!(record.state, LifecycleState::Ready);
        assert_eq!(record.stage.kind, super::LifecycleStageKind::SignCommitVote);
        assert_eq!(record.key.phase, super::LifecyclePhase::Commit);
        assert_eq!(
            record.key.round,
            super::LifecycleRound::new(prepare.round.height, prepare.round.view)
        );
        assert_eq!(
            record.key.proposal_round,
            Some(super::LifecycleRound::new(
                prepare.proposal_round.height,
                prepare.proposal_round.view
            ))
        );
        assert_eq!(
            record.key.subject,
            Some(super::projection::block_subject(prepare.subject))
        );
        assert_eq!(
            record.key.execution_commitment,
            Some(super::projection::execution_commitment(
                prepare.execution_commitment
            ))
        );
        assert!(self.coordinator.ready_index.contains(&record.ordinal));
        assert_eq!(record.owner.first_admission_ordinal(), record.ordinal);
        let metadata = self.coordinator.durable_records[&record.ordinal].clone();
        assert!(metadata.replay_authority.is_live_wal_origin());
        assert_eq!(
            metadata.reconstruction_source,
            record.owner.causal_root().digest()
        );
        assert_eq!(
            self.registry
                .registry_for_test()
                .finalization_entry_kind_census()
                .1
                .iter()
                .filter(|(ordinal, _)| *ordinal == record.ordinal)
                .count(),
            1
        );
        assert!(self.all_live_registry_census_is_exact_for_test());
        let (_, ledger) =
            super::ledger::LifecycleLedgerStoreV1::open(root, self.coordinator.active_context)
                .expect("read exact terminal and standalone Commit Sign ledger");
        let ledger_record = ledger
            .records()
            .iter()
            .find(|row| row.ordinal() == record.ordinal)
            .expect("the real Commit Sign is durable before restart")
            .encode();
        ResolvedCommitSignOwnerSnapshotForTest {
            record,
            metadata,
            ledger_record,
        }
    }

    /// Prove real cold startup reconstructs the same sole executable Commit Sign.
    pub(in crate::sumeragi) fn assert_resolved_commit_sign_cold_for_test(
        &self,
        snapshot: &ResolvedCommitSignOwnerSnapshotForTest,
        root: &std::path::Path,
    ) {
        use norito::codec::Encode as _;
        let ordinal = snapshot.record.ordinal;
        let record = &self.coordinator.records[&ordinal];
        assert_eq!(
            (
                record.key,
                record.owner,
                record.ordinal,
                record.work_class,
                record.stage,
                record.state
            ),
            (
                snapshot.record.key,
                snapshot.record.owner,
                snapshot.record.ordinal,
                snapshot.record.work_class,
                snapshot.record.stage,
                snapshot.record.state
            )
        );
        assert_eq!(
            self.coordinator.durable_records[&ordinal],
            snapshot.metadata
        );
        assert_eq!(self.coordinator.key_index.get(&record.key), Some(&ordinal));
        assert_eq!(
            self.coordinator
                .owner_index
                .get(&record.owner.causal_root()),
            Some(&record.owner)
        );
        assert!(self.coordinator.ready_index.contains(&ordinal));
        let sign_rows = self
            .coordinator
            .records
            .values()
            .filter(|row| row.work_class == LifecycleWorkClass::SignVote)
            .map(|row| row.ordinal)
            .collect::<Vec<_>>();
        assert_eq!(sign_rows, vec![ordinal]);
        let census = self
            .registry
            .registry_for_test()
            .finalization_entry_kind_census()
            .1;
        let carrier = census
            .into_iter()
            .filter(|(entry, _)| *entry == ordinal)
            .collect::<Vec<_>>();
        assert_eq!(carrier, vec![(ordinal, "DurableRecoveredWalControlSign")]);
        self.assert_recovered_output_and_registry_census_for_retry_test();
        assert!(self.coordinator.active_lease.is_none());
        assert!(self.coordinator.fault.is_none());
        let (_, ledger) =
            super::ledger::LifecycleLedgerStoreV1::open(root, self.coordinator.active_context)
                .expect("read the reopened Commit Sign ledger");
        assert_eq!(
            ledger
                .records()
                .iter()
                .find(|row| row.ordinal() == ordinal)
                .expect("same persisted Sign ordinal")
                .encode(),
            snapshot.ledger_record
        );
    }
}
