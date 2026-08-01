"""Reviewed constants for the Sumeragi v2 in-flight formal contract."""

from __future__ import annotations

from pathlib import Path

INFLIGHT_LAYOUT_CLAIM = "composed_state_action_relation_no_trace_extraction"
INFLIGHT_LAYOUT_MODULE = "SumeragiV2InFlightFirstRelease"
INFLIGHT_LAYOUT_POSITIVE_CONFIG = "inflight_first_release_fixed.cfg"
INFLIGHT_LAYOUT_RUNNER = Path(
    "scripts/formal/run_sumeragi_v2_inflight_first_release.sh"
)
INFLIGHT_LAYOUT_TEST = Path(
    "pytests/scripts/sumeragi_v2_multilane_models_test.py"
)
INFLIGHT_LAYOUT_EVIDENCE = Path(
    "formal/sumeragi_v2/INFLIGHT_FIRST_RELEASE_EVIDENCE.md"
)
INFLIGHT_LAYOUT_REQUIRED_INVARIANTS = (
    "FirstReleaseTypeInvariant",
    "MLPayloadSchemaV2CarriesExactAdmissionPreimage",
    "MLValidatorCarrierOwnership",
    "MLSelectedQueuePlanV4ConjunctionBeforeReservationV5",
    "MLReservationV5BeforeKuraActive",
    "MLKuraActiveBeforeExecutionInput",
    "MLExecutionInputBeforeReadyAuthorization",
    "MLReadyAuthorizationBeforeLocalSignature",
    "MLLocalSignaturesBeforeDurableReadyQc",
    "MLCrashDurableFactsRecoverable",
    "MLVolatileSessionLostOnCrash",
    "MLCommitAndReleaseRetainExactScope",
    "MLLaneCommitBeforeAtomicWsvCarrierApplication",
    "MLExactlyOnceCarrierApplication",
    "MLPostCarrierCommitCleanupOrder",
    "MLReleasePrefixesRecoverable",
    "MLReleaseStageOrder",
    "MLQueuePlanV4SelectedConjunctionBound4096",
)
INFLIGHT_LAYOUT_REQUIRED_ACTIONS = (
    "SelectQueuePlanV4Conjunction",
    "FsyncReservationV5",
    "ActivateKura",
    "FanoutFromProducer",
    "ServeLateBody",
    "PersistExecutionInput",
    "AuthorizeReady",
    "SignReady",
    "PersistReadyQc",
    "Crash",
    "Recover",
    "RecoverReservationSnapshot",
    "ReleaseReservationDirect",
    "LaneCommit",
    "ApplyCarrier",
    "PersistReservationCommitted",
    "PersistPlanTombstone",
    "ForgetReservationCommit",
    "PersistKuraRetirement",
    "AdvanceReleasePendingPrefix",
    "PrepareReservationRelease",
    "AdvanceReleasedPrefix",
    "CompleteReservationRelease",
    "RestoreReleasedFifo",
    "ForgetReservationRelease",
    "RepairPostCarrierEvidence",
)
INFLIGHT_COMPOSED_TLA_ALIGNMENT_TOKENS = (
    "Its three-validator states embed into the 1..128-validator fixed-width\n"
    "Rust/Verus `ProductionInFlightFirstReleaseStateProjection` and transition\n"
    "kernel.",
    "ReadyQuorum == 3",
    "CanonicalKeyPrefix(keys, bound) ==\n"
    "  /\\ keys \\subseteq PrefixThrough(bound)\n"
    "  /\\ keys = PrefixThrough(Cardinality(keys))",
    "PersistReservationCommitted(key) ==",
    "PersistPlanTombstone(key) ==",
    "ForgetReservationCommit(key) ==",
    "IF p = Producer THEN BindingA ELSE \"None\"",
    "/\\ payloadBinding[Producer] = BindingA\n"
    "  /\\ \\A p \\in Validators:\n"
    "       payloadBinding[p] = \"None\" \\/ payloadBinding[p] = BindingA",
    "FifoRestoredReservationStates ==\n"
    "  CompletedReleaseStates \\union {\"DirectReleased\"}",
    "RecoverReservationSnapshot ==\n  UNCHANGED vars",
    "ReleaseReservationDirect ==\n"
    "  /\\ queue.plan = \"SelectedConjunction\"\n"
    "  /\\ queue.reservation = \"Live\"\n"
    "  /\\ decision.laneCommitOwner = \"None\"\n"
    "  /\\ decision.releaseOwner = \"None\"\n"
    "  /\\ queue' = [queue EXCEPT !.reservation = \"DirectReleased\"]\n"
    "  /\\ release' = [release EXCEPT !.fifoRestored = TRUE]",
    "  \\/ \\E p \\in Validators \\ {Producer}: FanoutFromProducer(p)\n"
    "  \\/ \\E source \\in Validators, target \\in Validators:\n"
    "       ServeLateBody(source, target)",
    "  \\/ RecoverReservationSnapshot\n"
    "  \\/ ReleaseReservationDirect",
    "queue.reservation = \"DirectReleased\" =>\n"
    "       release.fifoRestored",
)
INFLIGHT_LAYOUT_MUTATIONS = (
    (
        "inflight_first_release_reservation_before_selected_queue_plan_bug.cfg",
        "MLSelectedQueuePlanV4ConjunctionBeforeReservationV5",
    ),
    (
        "inflight_first_release_kura_before_reservation_bug.cfg",
        "MLReservationV5BeforeKuraActive",
    ),
    (
        "inflight_first_release_ready_authorization_before_input_bug.cfg",
        "MLExecutionInputBeforeReadyAuthorization",
    ),
    (
        "inflight_first_release_ready_signature_before_authorization_bug.cfg",
        "MLReadyAuthorizationBeforeLocalSignature",
    ),
    (
        "inflight_first_release_ready_qc_before_signatures_bug.cfg",
        "MLLocalSignaturesBeforeDurableReadyQc",
    ),
    (
        "inflight_first_release_crash_drops_durable_bug.cfg",
        "MLCrashDurableFactsRecoverable",
    ),
    (
        "inflight_first_release_crash_retains_volatile_body_bug.cfg",
        "MLVolatileSessionLostOnCrash",
    ),
    (
        "inflight_first_release_payload_conflict_bug.cfg",
        "MLPayloadSchemaV2CarriesExactAdmissionPreimage",
    ),
    (
        "inflight_first_release_lane_commit_scope_conflict_bug.cfg",
        "MLCommitAndReleaseRetainExactScope",
    ),
    (
        "inflight_first_release_release_scope_conflict_bug.cfg",
        "MLCommitAndReleaseRetainExactScope",
    ),
    (
        "inflight_first_release_duplicate_apply_bug.cfg",
        "MLExactlyOnceCarrierApplication",
    ),
    (
        "inflight_first_release_reservation_commit_before_carrier_bug.cfg",
        "MLPostCarrierCommitCleanupOrder",
    ),
    (
        "inflight_first_release_plan_tombstone_before_reservation_commit_bug.cfg",
        "MLPostCarrierCommitCleanupOrder",
    ),
    (
        "inflight_first_release_forget_commit_before_plan_tombstone_bug.cfg",
        "MLPostCarrierCommitCleanupOrder",
    ),
    (
        "inflight_first_release_commit_prefix_skipped_key_bug.cfg",
        "MLPostCarrierCommitCleanupOrder",
    ),
    (
        "inflight_first_release_commit_prefix_decrease_bug.cfg",
        "MLPostCarrierCommitCleanupOrder",
    ),
    (
        "inflight_first_release_release_pending_before_retirement_bug.cfg",
        "MLReleaseStageOrder",
    ),
    (
        "inflight_first_release_release_prepare_before_pending_bug.cfg",
        "MLReleaseStageOrder",
    ),
    (
        "inflight_first_release_released_claims_before_prepare_bug.cfg",
        "MLReleaseStageOrder",
    ),
    (
        "inflight_first_release_release_complete_before_released_bug.cfg",
        "MLReleaseStageOrder",
    ),
    (
        "inflight_first_release_forget_release_before_fifo_bug.cfg",
        "MLReleaseStageOrder",
    ),
    (
        "inflight_first_release_oversize_selected_queue_plan_bug.cfg",
        "MLQueuePlanV4SelectedConjunctionBound4096",
    ),
)
INFLIGHT_LAYOUT_FORBIDDEN_TOKENS = (
    "LaneExecutablePayloadV3",
    "PutBatchV4",
    "MLPutBatchV4BeforeReservationV5",
    "MLQueuePlanV4PutBatchBound4096",
    "QueuePlanV5",
    "QueuePlan V5",
    "queuePlanV5",
    "everQueuePlanV5",
    "PutBatchV5",
    "reservationV9",
    "everReservationV9",
    "FsyncReservationV9",
    "reservation V9",
    "MLPayloadV3CarriesExactAdmissionPreimage",
    "MLPutBatchV5BeforeReservationV9",
    "MLReservationV9BeforeKuraActive",
    "PrunedRetired",
    "PruneRetiredReservation",
)
INFLIGHT_LAYOUT_FORBIDDEN_SOURCE_CHECKS = (
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "CheckedReplayAuthorizationDomain::clone",
        (
            "self.0.clone()",
            "Arc::clone(&self.0)",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::apply_checked_transition",
        (
            "self.clone()",
            "Clone::clone(self)",
            "(*self).clone()",
            "self.to_owned()",
            "ToOwned::to_owned(self)",
            "candidate.transition_semantics(",
            "*self = candidate",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::transition_release_batch",
        ("remove_live_unchecked",),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::transition_commit",
        ("remove_live_unchecked",),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "enum",
        "LaneQueueReservationJournalFrameV5",
        ("Prune", "RetiredLaneWideRemoval"),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::transition_semantics",
        ("LaneQueueReservationJournalFrameV5::Prune", "transition_prune"),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::check_in_flight_transition",
        (
            "LaneQueueReservationJournalFrameV5::Prune",
            "IN_FLIGHT_RESERVATION_ACTION_PRUNE_RETIRED",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::transition_complete_release",
        ("remove_live_unchecked",),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::remove_preflighted_live",
        (
            "expect(",
            "unwrap(",
            "panic!(",
            "unreachable!(",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "macro",
        "production_in_flight_reservation_transition_body",
        ("IN_FLIGHT_RESERVATION_ACTION_PRUNE_RETIRED",),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "macro",
        "production_in_flight_first_release_state_body",
        ("IN_FLIGHT_FIRST_RELEASE_RESERVATION_PRUNED_RETIRED",),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "macro",
        "production_in_flight_first_release_transition_body",
        ("IN_FLIGHT_FIRST_RELEASE_ACTION_PRUNE_RETIRED_RESERVATION",),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "fn",
        "production_in_flight_first_release_terminal_owner",
        ("IN_FLIGHT_FIRST_RELEASE_RESERVATION_PRUNED_RETIRED",),
    ),
)
INFLIGHT_LAYOUT_PRODUCTION_BINDINGS = (
    (
        "crates/iroha_core/src/lane_consensus.rs",
        "struct",
        "LaneExecutablePayloadV1",
        (
            "pub version: u8",
            "pub entrypoint_hashes: Vec<Hash>",
            "pub entrypoints: Vec<TransactionEntrypoint>",
            "pub reservation_keys: Vec<LaneQueueReservationKeyV2>",
            "pub routing_plans: Vec<RoutingPlan>",
            "pub native_amx_receipts: Vec<Option<NativeAmxReceipt>>",
            "pub payload_hash: Hash",
            "pub producer_signature: Vec<u8>",
        ),
    ),
    (
        "crates/iroha_core/src/lane_consensus.rs",
        "method",
        "LaneExecutablePayloadV1::new_signed_with_reservations",
        (
            "version: LANE_EXECUTABLE_PAYLOAD_VERSION_V2",
            "reservation_keys",
            "routing_plans",
            "native_amx_receipts",
            "payload.computed_payload_hash()?",
            "Signature::try_new",
            "payload.validate(chain_id_hash, epoch)?",
        ),
    ),
    (
        "crates/iroha_core/src/lane_consensus.rs",
        "fn",
        "deterministic_lane_author",
        (
            "lane_block_height.checked_sub(1)?",
            "u64::try_from(validator_set.len()).ok()?",
            "if validator_count == 0",
            "usize::try_from(author_offset % validator_count).ok()?",
            "validator_set.get(author_index)",
        ),
    ),
    (
        "crates/iroha_core/src/lane_consensus.rs",
        "method",
        "LaneExecutablePayloadV1::validate",
        (
            "deterministic_lane_author(",
            "&self.origin_proposal.descriptor.validator_set",
            "self.origin_proposal.descriptor.lane_block_height",
            ") != Some(&self.producer)",
            "LaneAutonomousArtifactError::ProducerNotDeterministicAuthor",
        ),
    ),
    (
        "crates/iroha_core/src/lane_consensus.rs",
        "fn",
        "validate_lane_executable_payload_body",
        (
            "version != LANE_EXECUTABLE_PAYLOAD_VERSION_V2",
            "entrypoints.is_empty()",
            "entrypoints.len() > MAX_LANE_EXECUTABLE_ENTRYPOINTS",
            "reservation_keys.len() != entrypoints.len()",
            "routing_plans.len() != entrypoints.len()",
            "native_amx_receipts.len() != entrypoints.len()",
            "key.validate().is_err()",
            "key.lane_incarnation != descriptor.lane_incarnation",
            "routing_plan.digest() != key.routing_plan_digest",
            "compute_lane_executable_payload_hash(",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/lane_planner.rs",
        "fn",
        "assemble_autonomous_lane_reservation_slot",
        (
            "let lane_block_height = previous_lane_block_height",
            ".checked_add(1)",
            "canonical_validator_set",
            "crate::lane_consensus::deterministic_lane_author(",
            "&validator_set",
            "lane_block_height",
            "AutonomousLaneReservationSlotPlanError::InvalidQuorum",
            "author,",
        ),
    ),
    (
        "crates/iroha_core/src/queue/journal.rs",
        "struct",
        "QueuePlanJournalRecordV4",
        (
            "pub version: u16",
            "pub entrypoint: TransactionEntrypoint",
            "pub entrypoint_hash: HashOf<TransactionEntrypoint>",
            "pub routing_plan: RoutingPlan",
            "pub admission_context: QueuePlanAdmissionContextV2",
            "pub global_admission_identity: Option<QueuePlanGlobalAdmissionIdentityV2>",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "struct",
        "LaneQueueReservationKeyV2",
        (
            "pub version: u16",
            "pub entrypoint_hash: HashOf<TransactionEntrypoint>",
            "pub queue_plan_admission_binding_hash: Hash",
            "pub routing_plan_digest: Hash",
            "pub coordinator_leg: RouteLeg",
            "pub lane_incarnation: Hash",
            "pub proposal_height: u64",
            "pub lane_block_height: u64",
            "pub lane_block_view: u64",
            "pub reservation_owner_hash: Hash",
            "pub proposal_identity_hash: Hash",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "struct",
        "LaneQueueReservationRecordV5",
        (
            "version: u16",
            "key: LaneQueueReservationKeyV2",
            "enqueue_timestamp_ms: u64",
            "fifo_order: LaneQueueFifoOrderV5",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "enum",
        "LaneQueueReservationJournalFrameV5",
        (
            "Bootstrap {",
            "Snapshot {",
            "PutBatch(Vec<LaneQueueReservationRecordV5>)",
            "ReleaseBatch(Vec<LaneQueueReservationKeyV2>)",
            "Commit(LaneQueueReservationKeyV2)",
            "ForgetCommit(LaneQueueReservationKeyV2)",
            "PrepareRelease(LaneQueueReservationReleaseBarrierV3)",
            "CompleteRelease(LaneQueueReservationReleaseCompletionV5)",
            "ForgetRelease(LaneQueueReservationReleaseBarrierV3)",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "fn",
        "bootstrap_frame",
        (
            "RESERVATION_JOURNAL_OPERATION_SCHEMA_V5",
            "RESERVATION_JOURNAL_FRAME_FORMAT_VERSION",
            "RESERVATION_JOURNAL_BOOTSTRAP_DOMAIN",
            "RESERVATION_JOURNAL_FRAME_MAGIC",
            "RESERVATION_JOURNAL_FRAME_COMMIT",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "CheckedReplayAuthorizationDomain::clone",
        (
            "fn clone(&self) -> Self",
            "Self::default()",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "CheckedReplayAuthorizationDomain::authorizes",
        (
            "fn authorizes(&self, authorization: &Arc<()>) -> bool",
            "Arc::ptr_eq(&self.0, authorization)",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "struct",
        "CheckedReplayStateShape",
        (
            "live: usize",
            "committed: usize",
            "release_barriers: usize",
            "completed_releases: usize",
            "ownership: usize",
            "fifo_ordinals: usize",
            "live_lane_incarnations: usize",
            "next_order: u64",
            "CheckedReplayStateShape {\n"
            "    live: usize,\n"
            "    committed: usize,\n"
            "    release_barriers: usize,\n"
            "    completed_releases: usize,\n"
            "    ownership: usize,\n"
            "    fifo_ordinals: usize,\n"
            "    live_lane_incarnations: usize,\n"
            "    next_order: u64,\n"
            "}",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::checked_shape",
        (
            "live: self.live.len()",
            "committed: self.committed.len()",
            "release_barriers: self.release_barriers.len()",
            "completed_releases: self.completed_releases.len()",
            "ownership: self.ownership.len()",
            "fifo_ordinals: self.fifo_ordinals.len()",
            "live_lane_incarnations: self.live_by_lane_incarnation.len()",
            "next_order: self.next_order",
            "CheckedReplayStateShape {\n"
            "            live: self.live.len(),\n"
            "            committed: self.committed.len(),\n"
            "            release_barriers: self.release_barriers.len(),\n"
            "            completed_releases: self.completed_releases.len(),\n"
            "            ownership: self.ownership.len(),\n"
            "            fifo_ordinals: self.fifo_ordinals.len(),\n"
            "            live_lane_incarnations: self.live_by_lane_incarnation.len(),\n"
            "            next_order: self.next_order,\n"
            "        }",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "struct",
        "PreparedReservationJournalTransition",
        (
            "authorization_domain: Arc<()>",
            "frame_digest: Hash",
            "maximum_owned_transactions: usize",
            "expected_generation: u64",
            "next_generation: u64",
            "expected_shape: CheckedReplayStateShape",
            "expected_state_identity: Hash",
            "resulting_state_identity: Hash",
            "owner_transition_count: usize",
            "owner_transition_coverage_identity: Hash",
            "owner_transitions:",
            "Vec<CheckedProductionTransition<ProductionInFlightReservationTransitionProjection>>",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::prepare_checked_transition",
        (
            "validate_frame_cardinality(frame, maximum)?",
            "self.transition_semantics(frame, maximum, false)?",
            ".checked_add(1)",
            "checked_transition_frame_digest(frame)?",
            "self.check_in_flight_transition(frame, maximum)?",
            "checked_transition_coverage_identity(&owner_transitions)?",
            "resulting_checked_state_identity(",
            "authorization_domain: self.authorization_domain.authorization()",
            "expected_shape: self.checked_shape()",
            "expected_state_identity: self.checked_state_identity",
            "owner_transition_count: owner_transitions.len()",
            "owner_transitions",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::check_in_flight_transition",
        (
            "IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT",
            "IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT,\n"
            "                        key,\n"
            "                        release_digest,\n"
            "                        self.ownership.get(&hash).copied(),\n"
            "                        candidate.ownership.get(&hash).copied(),",
            "let after = before.or(Some(DurableReservationOwnership::Live(key)));",
            "IN_FLIGHT_RESERVATION_ACTION_RESERVE",
            "IN_FLIGHT_RESERVATION_ACTION_RESERVE,\n"
            "                        key,\n"
            "                        None,\n"
            "                        before,\n"
            "                        after,",
            "let after = if before == Some(DurableReservationOwnership::Live(*key)) {\n"
            "                        None\n"
            "                    } else {\n"
            "                        before\n"
            "                    };",
            "IN_FLIGHT_RESERVATION_ACTION_RELEASE_DIRECT",
            "IN_FLIGHT_RESERVATION_ACTION_RELEASE_DIRECT,\n"
            "                        *key,\n"
            "                        None,\n"
            "                        before,\n"
            "                        after,",
            "Some(DurableReservationOwnership::Committed(*key))",
            "IN_FLIGHT_RESERVATION_ACTION_COMMIT",
            "IN_FLIGHT_RESERVATION_ACTION_COMMIT,\n"
            "                    *key,\n"
            "                    None,\n"
            "                    before,\n"
            "                    Some(DurableReservationOwnership::Committed(*key)),",
            "let after = if before == Some(DurableReservationOwnership::Committed(*key)) {\n"
            "                    None\n"
            "                } else {\n"
            "                    before\n"
            "                };",
            "IN_FLIGHT_RESERVATION_ACTION_FORGET_COMMIT",
            "IN_FLIGHT_RESERVATION_ACTION_FORGET_COMMIT,\n"
            "                    *key,\n"
            "                    None,\n"
            "                    before,\n"
            "                    after,",
            "Some(DurableReservationOwnership::Live(existing)) if existing == *key => {\n"
            "                            Some(DurableReservationOwnership::Prepared {\n"
            "                                key: *key,\n"
            "                                barrier_digest: release_digest,\n"
            "                            })\n"
            "                        }",
            "owner @ (DurableReservationOwnership::Prepared { .. }\n"
            "                            | DurableReservationOwnership::Completed { .. })",
            "IN_FLIGHT_RESERVATION_ACTION_PREPARE_RELEASE",
            "IN_FLIGHT_RESERVATION_ACTION_PREPARE_RELEASE,\n"
            "                        *key,\n"
            "                        Some(release_digest),\n"
            "                        before,\n"
            "                        after,",
            "Some(DurableReservationOwnership::Prepared {\n"
            "                            key: existing,\n"
            "                            barrier_digest,\n"
            "                        }) if existing == key && barrier_digest == release_digest => {\n"
            "                            Some(DurableReservationOwnership::Completed {\n"
            "                                key,\n"
            "                                barrier_digest: release_digest,\n"
            "                            })\n"
            "                        }",
            "Some(owner @ DurableReservationOwnership::Completed { .. }) => Some(owner)",
            "IN_FLIGHT_RESERVATION_ACTION_COMPLETE_RELEASE",
            "IN_FLIGHT_RESERVATION_ACTION_COMPLETE_RELEASE,\n"
            "                        key,\n"
            "                        Some(release_digest),\n"
            "                        before,\n"
            "                        after,",
            "let has_completion = self.completed_releases.contains_key(&release_digest);",
            "let after = if has_completion\n"
            "                        && before\n"
            "                            == Some(DurableReservationOwnership::Completed {\n"
            "                                key: *key,\n"
            "                                barrier_digest: release_digest,\n"
            "                            }) {\n"
            "                        None\n"
            "                    } else {\n"
            "                        before\n"
            "                    };",
            "IN_FLIGHT_RESERVATION_ACTION_FORGET_RELEASE",
            "IN_FLIGHT_RESERVATION_ACTION_FORGET_RELEASE,\n"
            "                        *key,\n"
            "                        Some(release_digest),\n"
            "                        before,\n"
            "                        after,",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::apply_checked_transition",
        (
            "checked_transition_frame_digest(frame)? != prepared.frame_digest",
            "maximum != prepared.maximum_owned_transactions",
            ".authorizes(&prepared.authorization_domain)",
            "self.transition_generation != prepared.expected_generation",
            "self.checked_shape() != prepared.expected_shape",
            "self.checked_state_identity != prepared.expected_state_identity",
            "prepared.expected_generation.checked_add(1)",
            "prepared.owner_transition_count != prepared.owner_transitions.len()",
            "checked_transition_coverage_identity(&prepared.owner_transitions)?",
            "resulting_checked_state_identity(",
            "self.transition_semantics(frame, maximum, false)?;",
            "let current_owner_transitions = self.check_in_flight_transition(frame, maximum)?;",
            "current_owner_transitions.len() != prepared.owner_transition_count",
            ".map(|checked| checked.accepted_projection())",
            "for checked in current_owner_transitions",
            "for checked in prepared.owner_transitions",
            "checked.into_projection()",
            "self.transition_semantics(frame, maximum, true)?;",
            "self.transition_generation = prepared.next_generation;",
            "self.checked_state_identity = prepared.resulting_state_identity;",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::transition_release_batch",
        (
            "let mut removals = Vec::new();",
            ".push(self.validate_live_secondary_indexes(key.signed_transaction_hash, *key)?)",
            "if apply {",
            "for record in &removals",
            "self.remove_preflighted_live(record);",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::transition_commit",
        (
            "key.validate().map_err(invalid_data)?;",
            "let owner = self.ownership.get(&key.signed_transaction_hash).copied();",
            "reservation commit conflicts with a different live reservation identity",
            "reservation commit conflicts with an existing commit barrier",
            "reservation commit overlaps an ordered release claim",
            "reservation commit requires an exact live reservation",
            "let live_removal = match owner",
            "Some(self.validate_live_secondary_indexes(key.signed_transaction_hash, existing)?)",
            "let needs_commit = !matches!(",
            "self.ensure_owner_capacity(0, maximum)?;",
            "self.order_range(if needs_commit { 1 } else { 0 })?;",
            "if !apply {",
            "if let Some(record) = &live_removal",
            "self.remove_preflighted_live(record);",
            "self.committed.insert(",
            "DurableReservationOwnership::Committed(key)",
            "self.next_order = next_order;",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::transition_complete_release",
        (
            "let mut removals = Vec::with_capacity(completion.ordered_records.len());",
            "let live_record = self.validate_live_secondary_indexes(hash, record.key)?;",
            "if live_record != *record",
            "removals.push(live_record);",
            "self.order_range(1)?;",
            "if apply {",
            "for record in &removals",
            "self.remove_preflighted_live(record);",
            "DurableReservationOwnership::Completed",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::validate_live_secondary_indexes",
        (
            "fn validate_live_secondary_indexes(",
            ") -> io::Result<LaneQueueReservationRecordV5>",
            "expected_key.signed_transaction_hash != hash",
            "live reservation index key differs from the exact reservation hash",
            ".live",
            ".get(&hash)",
            "live reservation index has no exact record",
            "record.value.key != expected_key",
            "self.fifo_ordinals.get(&record.value.fifo_order.ordinal) != Some(&hash)",
            ".live_by_lane_incarnation",
            ".get(&lane)",
            ".is_some_and(|hashes| hashes.contains(&hash))",
            "Ok(record.value.clone())",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::remove_preflighted_live",
        (
            "fn remove_preflighted_live(&mut self, record: &LaneQueueReservationRecordV5)",
            "debug_assert_eq!(self.live.get(&hash).map(|entry| &entry.value), Some(record));",
            "self.fifo_ordinals.get(&record.fifo_order.ordinal)",
            "debug_assert!(",
            "self.live_by_lane_incarnation",
            "self.live.remove(&hash);",
            "self.fifo_ordinals.remove(&record.fifo_order.ordinal);",
            "self.ownership.remove(&hash);",
            ".get_mut(&lane)",
            "hashes.remove(&hash);",
            "if remove_lane {",
            "self.live_by_lane_incarnation.remove(&lane);",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "LaneQueueReservationJournal::append_durable",
        (
            "lane reservation journal is poisoned after a failed durability boundary",
            "self.verify_cached_storage_unchanged()",
            "lane reservation bootstrap and snapshots cannot be appended as runtime operations",
            ".prepare_checked_transition(frame, self.limits.max_owned_transactions)?",
            "encode_frame_with_limit(frame, self.limits.max_frame_payload_bytes)?",
            "self.preflight_append_end(&encoded)?",
            "self.append_staged(&encoded, expected_end, prepared)",
            "if let Err(error) = self.replay_state.apply_checked_transition(",
            "replay instead of panicking or attempting an in-process retry.",
            "self.poisoned = true;",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "LaneQueueReservationJournal::compact_if_needed",
        (
            "let snapshot = canonical_snapshot(",
            "let mut compacted_replay_state = IndexedReservationReplayState::default();",
            "validate_snapshot_frame(frame, self.limits)?;",
            ".prepare_checked_transition(frame, self.limits.max_owned_transactions)",
            "let canonical_replay = replay_path(&self.path, self.limits)?;",
            "lane reservation compaction input does not match the exact durable journal state",
            "persist_atomic_replacement(&tmp, &self.path)",
            "tmp_file.sync_all()",
            "self.parent.sync_all()",
            "compacted_replay_state.apply_checked_transition(",
            "The replacement is already durable. Keep the previous",
            "self.replay_state = compacted_replay_state;",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "fn",
        "check_production_in_flight_reservation_transition",
        (
            "production_in_flight_reservation_transition_kernel(projection)",
            "Some(CheckedProductionTransition { projection })",
            "None",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "struct",
        "ProductionInFlightFirstReleaseHistoryProjection",
        (
            "pub(crate) reservation_committed_prefix: u64",
            "pub(crate) queue_plan_tombstoned_prefix: u64",
            "pub(crate) reservation_commit_forgotten_prefix: u64",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "struct",
        "ProductionInFlightFirstReleaseStateProjection",
        (
            "pub(crate) validator_count: u8",
            "pub(crate) producer: u128",
            "pub(crate) producer_selected_owner: u128",
            "pub(crate) replicated_carrier_owners: u128",
            "pub(crate) payload_binding_a: u128",
            "pub(crate) binding_a: CanonicalIdentityProjection",
            "pub(crate) queue: ProductionInFlightFirstReleaseQueueProjection",
            "pub(crate) carrier: ProductionInFlightFirstReleaseCarrierProjection",
            "pub(crate) session: ProductionInFlightFirstReleaseSessionProjection",
            "pub(crate) history: ProductionInFlightFirstReleaseHistoryProjection",
            "pub(crate) decision: ProductionInFlightFirstReleaseDecisionProjection",
            "pub(crate) release: ProductionInFlightFirstReleaseReleaseProjection",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "struct",
        "ProductionInFlightFirstReleaseTransitionProjection",
        (
            "pub(crate) action: u8",
            "pub(crate) actor: u128",
            "pub(crate) target: u128",
            "pub(crate) before: ProductionInFlightFirstReleaseStateProjection",
            "pub(crate) after: ProductionInFlightFirstReleaseStateProjection",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "macro",
        "production_in_flight_first_release_state_body",
        (
            "in_flight_first_release_validator_mask_body!(state.validator_count)",
            "in_flight_first_release_ready_quorum_body!(state.validator_count)",
            "state.validator_count >= 1u8",
            "state.validator_count <= 128u8",
            "state.producer_selected_owner == state.producer",
            "state.replicated_carrier_owners == (validator_mask & !state.producer)",
            "(state.payload_binding_a & !validator_mask) == 0u128",
            "(state.payload_binding_a & state.producer) == state.producer",
            "in_flight_first_release_bitmap_count_body!(history.ready_signed)",
            ">= ready_quorum",
            "queue.selected_count > 0u64 && queue.selected_count <= 4096u64",
            "queue.reservation_state\n"
            "                == refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_RESERVATION_ABSENT)\n"
            "                || queue.plan_state\n"
            "                    != refinement_tag_value!(IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_ABSENT)",
            "(carrier.execution_input_durable & !carrier.kura_active) == 0u128",
            "(carrier.kura_active == 0u128 || history.ever_reservation_v5)",
            "(session.ready_authorized & !carrier.execution_input_durable) == 0u128",
            "(history.ready_signed & !history.ever_ready_authorized) == 0u128",
            "decision.application_count <= 1u8",
            "decision.wsv_committed == (decision.application_count == 1u8)",
            "history.reservation_committed_prefix <= queue.selected_count",
            "history.queue_plan_tombstoned_prefix\n"
            "                <= history.reservation_committed_prefix",
            "history.reservation_commit_forgotten_prefix\n"
            "                <= history.queue_plan_tombstoned_prefix",
            "release.released_prefix <= release.pending_prefix",
            "IN_FLIGHT_FIRST_RELEASE_RESERVATION_DIRECT_RELEASED",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "macro",
        "production_in_flight_first_release_transition_body",
        (
            "production_in_flight_first_release_state_body!(before)",
            "production_in_flight_first_release_state_body!(after)",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_SELECT_QUEUE_PLAN_V4",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_FSYNC_RESERVATION_V5",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_ACTIVATE_KURA",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_FANOUT_FROM_PRODUCER",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_SERVE_LATE_BODY",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_EXECUTION_INPUT",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_AUTHORIZE_READY",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_SIGN_READY",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_READY_QC",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_CRASH",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_LANE_COMMIT",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_APPLY_CARRIER",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_RESERVATION_COMMITTED",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_PLAN_TOMBSTONE",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_FORGET_RESERVATION_COMMIT",
            "== (before.history.reservation_committed_prefix + 1u64) as u128",
            "== (before.history.queue_plan_tombstoned_prefix + 1u64) as u128",
            "== (before.history.reservation_commit_forgotten_prefix + 1u64) as u128",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PERSIST_KURA_RETIREMENT",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASE_PENDING",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_PREPARE_RESERVATION_RELEASE",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_ADVANCE_RELEASED",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_COMPLETE_RESERVATION_RELEASE",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_RESTORE_RELEASED_FIFO",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_FORGET_RESERVATION_RELEASE",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_REPAIR_POST_CARRIER",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_RECOVER_RESERVATION_SNAPSHOT",
            "in_flight_first_release_state_equal_body!(before, after)",
            "// exact stutter, never a new reservation acquisition.\n"
            "                projection.actor == 0u128\n"
            "                    && projection.target == 0u128\n"
            "                    && in_flight_first_release_state_equal_body!(before, after)",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_RELEASE_RESERVATION_DIRECT",
            "IN_FLIGHT_FIRST_RELEASE_RESERVATION_DIRECT_RELEASED",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "fn",
        "check_production_in_flight_first_release_transition",
        (
            "production_in_flight_first_release_transition_kernel(projection)",
            "Some(CheckedProductionTransition { projection })",
            "None",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "fn",
        "production_in_flight_first_release_terminal_owner",
        (
            "production_in_flight_first_release_state_kernel(projection)",
            "IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMIT_FORGOTTEN",
            "projection.history.reservation_commit_forgotten_prefix\n"
            "            == projection.queue.selected_count",
            "IN_FLIGHT_FIRST_RELEASE_RESERVATION_RELEASE_FORGOTTEN",
            "IN_FLIGHT_FIRST_RELEASE_RESERVATION_DIRECT_RELEASED",
            "ordinary_fifo_owner: false",
            "canonical_wsv_owner: true",
            "ordinary_fifo_owner: true",
            "canonical_wsv_owner: false",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/lane_planner.rs",
        "struct",
        "AutonomousLaneReservationSelectionAuthorization",
        (
            "scope: LaneQueueReservationScopeV1",
            "validator_count: u8",
            "producer: u128",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/lane_planner.rs",
        "method",
        "AutonomousLaneReservationSlotPlan::selection_authorization",
        (
            "u8::try_from(self.validator_set.len())",
            ".position(|peer| peer == &self.author)",
            ".checked_shl(",
            "scope: self.reservation_scope()",
            "validator_count",
            "producer",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "fn",
        "reserve_transactions_for_lane_bounded",
        (
            "AutonomousLaneReservationSelectionAuthorization",
            "MAX_MERGE_EXECUTION_ENTRYPOINTS",
            "durable_claim.global_admission_binding()",
            "QueuePlanAdmissionRegistryMatch::Exact",
            "version: LANE_QUEUE_RESERVATION_JOURNAL_VERSION",
            "canonical_lane_queue_reservation_group_identity_projection",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_SELECT_QUEUE_PLAN_V4",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_FSYNC_RESERVATION_V5",
            "check_production_in_flight_first_release_transition",
            "checked_reservation_fsync.into_projection()",
            "journal.put_batch(",
            "restore_popped_hash_locked",
            "live_by_hash",
        ),
    ),
    (
        "crates/iroha_core/src/kura/autonomous_merge_bundle_support.rs",
        "struct",
        "LaneReadyAuthorization",
        (
            "durable_execution_input_hash: Hash",
            "reservation_group: LaneQueueReservationGroupBindingV1",
            "producer: PeerId",
            "signer: PeerId",
            "height_context_id: HeightContextId",
        ),
    ),
    (
        "crates/iroha_core/src/kura/autonomous_merge_bundle_support.rs",
        "method",
        "LaneReadyAuthorization::consume_signing_request",
        (
            "self.matches_signing_request(",
            "u8::try_from(descriptor.validator_set.len())",
            ".position(|peer| peer == &self.producer)",
            ".position(|peer| peer == signer)",
            "canonical_lane_queue_reservation_group_identity_projection(",
            "IN_FLIGHT_FIRST_RELEASE_ACTION_SIGN_READY",
            "check_production_in_flight_first_release_transition(projection)",
            ".is_some_and(|checked| checked.into_projection() == projection)",
        ),
    ),
    (
        "crates/iroha_core/src/lane_consensus.rs",
        "method",
        "LanePayloadAvailabilityVoteV1::new_signed_with_authorization",
        (
            "authorization.consume_signing_request(",
            "Signature::try_new(private_key, &body.signature_preimage())",
            "vote.validate_against_validator_set(&proposal.descriptor.validator_set)?",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "struct",
        "PreKuraDirectReleaseContext",
        (
            "validator_count: u8",
            "producer: u128",
            "expected_group: LaneQueueReservationGroupBindingV1",
            "ordered_keys: Vec<LaneQueueReservationKeyV2>",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        "method",
        "PendingAutonomousReservationBatch::pre_kura_direct_release_context",
        (
            "validator_count == 0 || validator_count > 128",
            "HashOf::new(&self.slot.validator_set)",
            "commit_quorum_from_len(validator_count)",
            ".position(|peer| peer == &self.slot.author)",
            "lane_queue_reservation_group_binding_from_ordered_keys(&ordered_keys)",
            "expected_group.identity != expected_identity",
            "Ok(PreKuraDirectReleaseContext {",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "struct",
        "LaneBlockExecutionInputArtifact",
        (
            "pub format: LaneBlockExecutionInputArtifactFormat",
            "pub proposal: LaneBlockProposalV1",
            "pub autonomous_chain_id_hash: Option<Hash>",
            "pub autonomous_epoch: Option<u64>",
            "pub autonomous_payload_hash: Option<Hash>",
            "pub entrypoint_hashes: Vec<Hash>",
            "pub entrypoints: Vec<TransactionEntrypoint>",
            "pub reservation_keys: Vec<LaneQueueReservationKeyV2>",
            "pub routing_plans: Vec<RoutingPlan>",
            "pub native_amx_receipts: Vec<Option<NativeAmxReceipt>>",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::validate_lane_block_execution_input_artifact",
        (
            "artifact.reservation_keys.len() != artifact.entrypoints.len()",
            "artifact.routing_plans.len() != artifact.entrypoints.len()",
            "artifact.native_amx_receipts.len() != artifact.entrypoints.len()",
            "key.validate().is_err()",
            "artifact.entrypoint_hashes != descriptor.accepted_transaction_hashes",
            "Hash::from(entrypoint.hash()) != expected_hash",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::recover_lane_block_execution_input_source",
        (
            "recover_autonomous_lane_block_payload_with_sidecar_repair",
            "recover_lane_block_payload_with_sidecar_repair",
            "LaneBlockPayloadAvailability::DescriptorMismatch",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::persist_lane_block_execution_input",
        (
            "recover_lane_block_execution_input_source(",
            "if &verified != recovered",
            "LaneBlockExecutionInputArtifact::new(verified)",
            "write_lane_block_execution_input_artifact(&artifact)",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::release_pre_kura_autonomous_reservation_batch",
        (
            "let _reservation_transition_guard = self.lane_reservation_transition_lock.lock();",
            "let queue_guard = self.push_remove_lock.lock();",
            "validate_for_lane_reservation_commit(&record.key)",
            "Self::reconciliation_record_from_durable_claim(record, claim.value())?",
            "check_production_in_flight_first_release_transition(projection)",
            "production_in_flight_first_release_terminal_owner(after)",
            "let restored_fifo = self.fifo_with_released_reservations_locked(&records)?;",
            "drop(store);",
            "self.apply_lane_reservation_journal(|journal| {",
            "let authorized_projection = checked.into_projection();",
            "journal.release_batch(release_keys)",
            "let mut store = self.lane_reservations.lock();",
            "self.replace_fifo_locked(&restored_fifo);",
            "drop(queue_guard);",
        ),
    ),
)
INFLIGHT_LAYOUT_ORDERED_SOURCE_CHECKS = (
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "struct",
        "CheckedReplayStateShape",
        (
            "live: usize",
            "committed: usize",
            "release_barriers: usize",
            "completed_releases: usize",
            "ownership: usize",
            "fifo_ordinals: usize",
            "live_lane_incarnations: usize",
            "next_order: u64",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::checked_shape",
        (
            "live: self.live.len()",
            "committed: self.committed.len()",
            "release_barriers: self.release_barriers.len()",
            "completed_releases: self.completed_releases.len()",
            "ownership: self.ownership.len()",
            "fifo_ordinals: self.fifo_ordinals.len()",
            "live_lane_incarnations: self.live_by_lane_incarnation.len()",
            "next_order: self.next_order",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "struct",
        "PreparedReservationJournalTransition",
        (
            "authorization_domain: Arc<()>",
            "frame_digest: Hash",
            "maximum_owned_transactions: usize",
            "expected_generation: u64",
            "next_generation: u64",
            "expected_shape: CheckedReplayStateShape",
            "expected_state_identity: Hash",
            "resulting_state_identity: Hash",
            "owner_transition_count: usize",
            "owner_transition_coverage_identity: Hash",
            "owner_transitions:",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::prepare_checked_transition",
        (
            "validate_frame_cardinality(frame, maximum)?",
            "self.transition_semantics(frame, maximum, false)?",
            ".checked_add(1)",
            "checked_transition_frame_digest(frame)?",
            "self.check_in_flight_transition(frame, maximum)?",
            "checked_transition_coverage_identity(&owner_transitions)?",
            "let resulting_state_identity = resulting_checked_state_identity(",
            "Ok(PreparedReservationJournalTransition {",
            "authorization_domain: self.authorization_domain.authorization()",
            "expected_shape: self.checked_shape()",
            "expected_state_identity: self.checked_state_identity",
            "owner_transition_count: owner_transitions.len()",
            "owner_transitions",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::apply_checked_transition",
        (
            "checked_transition_frame_digest(frame)? != prepared.frame_digest",
            "maximum != prepared.maximum_owned_transactions",
            ".authorizes(&prepared.authorization_domain)",
            "self.transition_generation != prepared.expected_generation",
            "self.checked_shape() != prepared.expected_shape",
            "self.checked_state_identity != prepared.expected_state_identity",
            "prepared.expected_generation.checked_add(1)",
            "prepared.owner_transition_count != prepared.owner_transitions.len()",
            "checked_transition_coverage_identity(&prepared.owner_transitions)?",
            "resulting_checked_state_identity(",
            "self.transition_semantics(frame, maximum, false)?;",
            "let current_owner_transitions = self.check_in_flight_transition(frame, maximum)?;",
            "current_owner_transitions.len() != prepared.owner_transition_count",
            ".map(|checked| checked.accepted_projection())",
            "for checked in current_owner_transitions",
            "for checked in prepared.owner_transitions",
            "checked.into_projection()",
            "self.transition_semantics(frame, maximum, true)?;",
            "self.transition_generation = prepared.next_generation;",
            "self.checked_state_identity = prepared.resulting_state_identity;",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::check_in_flight_transition",
        (
            "LaneQueueReservationJournalFrameV5::Snapshot {",
            "IN_FLIGHT_RESERVATION_ACTION_RECOVER_SNAPSHOT",
            "self.ownership.get(&hash).copied()",
            "candidate.ownership.get(&hash).copied()",
            "LaneQueueReservationJournalFrameV5::PutBatch(records) => {",
            "let after = before.or(Some(DurableReservationOwnership::Live(key)));",
            "IN_FLIGHT_RESERVATION_ACTION_RESERVE",
            "LaneQueueReservationJournalFrameV5::ReleaseBatch(keys) => {",
            "let after = if before == Some(DurableReservationOwnership::Live(*key)) {",
            "IN_FLIGHT_RESERVATION_ACTION_RELEASE_DIRECT",
            "LaneQueueReservationJournalFrameV5::Commit(key) => {",
            "IN_FLIGHT_RESERVATION_ACTION_COMMIT",
            "Some(DurableReservationOwnership::Committed(*key))",
            "LaneQueueReservationJournalFrameV5::ForgetCommit(key) => {",
            "IN_FLIGHT_RESERVATION_ACTION_FORGET_COMMIT",
            "LaneQueueReservationJournalFrameV5::PrepareRelease(barrier) => {",
            "IN_FLIGHT_RESERVATION_ACTION_PREPARE_RELEASE",
            "LaneQueueReservationJournalFrameV5::CompleteRelease(completion) => {",
            "IN_FLIGHT_RESERVATION_ACTION_COMPLETE_RELEASE",
            "LaneQueueReservationJournalFrameV5::ForgetRelease(barrier) => {",
            "IN_FLIGHT_RESERVATION_ACTION_FORGET_RELEASE",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::transition_release_batch",
        (
            "let mut removals = Vec::new();",
            ".push(self.validate_live_secondary_indexes(key.signed_transaction_hash, *key)?)",
            "if apply {",
            "for record in &removals",
            "self.remove_preflighted_live(record);",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::transition_commit",
        (
            "key.validate().map_err(invalid_data)?;",
            "let owner = self.ownership.get(&key.signed_transaction_hash).copied();",
            "reservation commit requires an exact live reservation",
            "let live_removal = match owner",
            "Some(self.validate_live_secondary_indexes(key.signed_transaction_hash, existing)?)",
            "let needs_commit = !matches!(",
            "self.ensure_owner_capacity(0, maximum)?;",
            "self.order_range(if needs_commit { 1 } else { 0 })?;",
            "if !apply {",
            "if let Some(record) = &live_removal",
            "self.remove_preflighted_live(record);",
            "self.committed.insert(",
            "DurableReservationOwnership::Committed(key)",
            "self.next_order = next_order;",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::transition_complete_release",
        (
            "let mut removals = Vec::with_capacity(completion.ordered_records.len());",
            "let live_record = self.validate_live_secondary_indexes(hash, record.key)?;",
            "if live_record != *record",
            "removals.push(live_record);",
            "self.order_range(1)?;",
            "if apply {",
            "self.release_barriers.remove(&digest);",
            "for record in &removals",
            "self.remove_preflighted_live(record);",
            "self.completed_releases.insert(",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::validate_live_secondary_indexes",
        (
            "expected_key.signed_transaction_hash != hash",
            ".live",
            ".get(&hash)",
            "record.value.key != expected_key",
            "self.fifo_ordinals.get(&record.value.fifo_order.ordinal) != Some(&hash)",
            "let lane = (expected_key.lane_id, expected_key.lane_incarnation);",
            ".live_by_lane_incarnation",
            ".get(&lane)",
            ".is_some_and(|hashes| hashes.contains(&hash))",
            "Ok(record.value.clone())",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "IndexedReservationReplayState::remove_preflighted_live",
        (
            "debug_assert_eq!(self.live.get(&hash).map(|entry| &entry.value), Some(record));",
            "debug_assert_eq!(",
            "debug_assert!(",
            "self.live.remove(&hash);",
            "self.fifo_ordinals.remove(&record.fifo_order.ordinal);",
            "self.ownership.remove(&hash);",
            ".get_mut(&lane)",
            "hashes.remove(&hash);",
            "if remove_lane {",
            "self.live_by_lane_incarnation.remove(&lane);",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "LaneQueueReservationJournal::append_durable",
        (
            ".prepare_checked_transition(frame, self.limits.max_owned_transactions)?",
            "encode_frame_with_limit(frame, self.limits.max_frame_payload_bytes)?",
            "self.preflight_append_end(&encoded)?",
            "self.append_staged(&encoded, expected_end, prepared)",
            "if let Err(error) = self.replay_state.apply_checked_transition(",
            "replay instead of panicking or attempting an in-process retry.",
            "self.poisoned = true;",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        "method",
        "LaneQueueReservationJournal::compact_if_needed",
        (
            "let snapshot = canonical_snapshot(",
            "let mut compacted_replay_state = IndexedReservationReplayState::default();",
            "validate_snapshot_frame(frame, self.limits)?;",
            ".prepare_checked_transition(frame, self.limits.max_owned_transactions)",
            "let canonical_replay = replay_path(&self.path, self.limits)?;",
            "lane reservation compaction input does not match the exact durable journal state",
            "persist_atomic_replacement(&tmp, &self.path)",
            "tmp_file.sync_all()",
            "self.parent.sync_all()",
            "compacted_replay_state.apply_checked_transition(",
            "The replacement is already durable. Keep the previous",
            "self.poisoned = true;",
            "self.replay_state = compacted_replay_state;",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_core/refinement.rs",
        "fn",
        "check_production_in_flight_reservation_transition",
        (
            "production_in_flight_reservation_transition_kernel(projection)",
            "Some(CheckedProductionTransition { projection })",
            "None",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "fn",
        "reserve_transactions_for_lane_bounded",
        (
            "durable_claim.global_admission_binding()",
            "QueuePlanAdmissionRegistryMatch::Exact",
            "journal.put_batch(",
            "live_by_hash",
        ),
    ),
    (
        "crates/iroha_core/src/lane_consensus.rs",
        "method",
        "LanePayloadAvailabilityVoteV1::new_signed_with_authorization",
        (
            "authorization.consume_signing_request(",
            "Signature::try_new(private_key, &body.signature_preimage())",
            "let vote = Self {",
            "vote.validate_against_validator_set(&proposal.descriptor.validator_set)?",
        ),
    ),
    (
        "crates/iroha_core/src/kura.rs",
        "method",
        "Kura::persist_lane_block_execution_input",
        (
            "recover_lane_block_execution_input_source(",
            "if &verified != recovered",
            "LaneBlockExecutionInputArtifact::new(verified)",
            "write_lane_block_execution_input_artifact(&artifact)",
        ),
    ),
    (
        "crates/iroha_core/src/queue.rs",
        "method",
        "Queue::release_pre_kura_autonomous_reservation_batch",
        (
            "let _reservation_transition_guard = self.lane_reservation_transition_lock.lock();",
            "let queue_guard = self.push_remove_lock.lock();",
            "validate_for_lane_reservation_commit(&record.key)",
            "Self::reconciliation_record_from_durable_claim(record, claim.value())?",
            "check_production_in_flight_first_release_transition(projection)",
            "production_in_flight_first_release_terminal_owner(after)",
            "let restored_fifo = self.fifo_with_released_reservations_locked(&records)?;",
            "drop(store);",
            "self.apply_lane_reservation_journal(|journal| {",
            "let authorized_projection = checked.into_projection();",
            "journal.release_batch(release_keys)",
            "let mut store = self.lane_reservations.lock();",
            "self.replace_fifo_locked(&restored_fifo);",
            "drop(queue_guard);",
        ),
    ),
)
INFLIGHT_LAYOUT_SOURCE_CHECKS = (
    (
        "crates/iroha_core/src/lane_consensus.rs",
        (
            "pub(crate) const MAX_LANE_EXECUTABLE_ENTRYPOINTS: usize = "
            "MAX_MERGE_EXECUTION_ENTRYPOINTS;",
            "pub(crate) const LANE_EXECUTABLE_PAYLOAD_VERSION_V2: u8 = 2;",
            "unknown.version = LANE_EXECUTABLE_PAYLOAD_VERSION_V2 + 1;",
            "fn autonomous_payload_requires_height_rotated_committee_author()",
            "committee membership and a valid signature must not confer slot authorship",
            "global carrier view must not change autonomous authorship",
        ),
    ),
    (
        "crates/iroha_data_model/src/merge.rs",
        ("pub const MAX_MERGE_EXECUTION_ENTRYPOINTS: usize = 4_096;",),
    ),
    (
        "crates/iroha_core/src/queue/journal.rs",
        (
            "pub const QUEUE_PLAN_JOURNAL_VERSION: u16 = 4;",
            "unsupported.version = QUEUE_PLAN_JOURNAL_VERSION + 1;",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal.rs",
        (
            "pub const LANE_QUEUE_RESERVATION_JOURNAL_VERSION: u16 = 5;",
            "const RESERVATION_JOURNAL_OPERATION_SCHEMA_V5: &[u8] =",
            "fn retired_v5_lane_wide_removal_fails_closed_at_bootstrap_and_operation_decode()",
            "retired V5 operation tag must not decode as an ordered release",
            "legacy.fifo_order.version = "
            "LANE_QUEUE_RESERVATION_JOURNAL_VERSION - 1;",
        ),
    ),
    (
        "crates/iroha_core/src/queue/reservation_journal_recovery_tests.rs",
        (
            "fn post_sync_append_publication_failure_is_poisoned_and_replayed_on_reopen()",
            "fn post_sync_compaction_publication_failure_is_poisoned_and_replayed_on_reopen()",
            "fn runtime_commit_requires_live_owner_but_snapshot_recovery_may_restore_commit_barrier()",
            "fn prepared_checked_transition_is_bound_to_frame_and_state_generation()",
            "fn prepared_checked_transition_rejects_same_generation_cross_state_substitution()",
            "fn prepared_checked_transition_binds_exact_ordered_owner_token_coverage()",
            "fn checked_transition_result_identity_and_candidate_application_are_atomic()",
            "fn checked_transition_generation_overflow_is_rejected_without_mutation()",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_core/refinement_cases.rs",
        (
            "fn in_flight_first_release_dynamic_committees_bind_masks_custody_and_canonical_quorum()",
            "fn in_flight_first_release_composed_commit_path_is_exact_and_terminal()",
            "fn in_flight_first_release_commit_cleanup_prefixes_cover_bounds_and_crash_recovery()",
            "fn in_flight_first_release_commit_cleanup_rejects_skips_decreases_and_stage_reordering()",
            "fn in_flight_first_release_composed_four_stage_release_is_exact_and_terminal()",
            "fn in_flight_first_release_snapshot_and_direct_release_are_exactly_aligned()",
        ),
    ),
    (
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs",
        (
            "fn pre_kura_direct_release_projection_is_four_validator_bound_and_fail_closed()",
            "mismatched_group.expected_group.reservation_group_hash =",
            "outside_committee.producer = 1_u128 << 4;",
            "the checked transition must append before FIFO ownership is published",
        ),
    ),
    (
        "crates/iroha_sumeragi_core/src/verus_proofs.rs",
        (
            "pub struct ProductionInFlightFirstReleaseStateProjection {",
            "pub struct ProductionInFlightFirstReleaseTransitionProjection {",
            "pub reservation_committed_prefix: u64",
            "pub queue_plan_tombstoned_prefix: u64",
            "pub reservation_commit_forgotten_prefix: u64",
            "pub closed spec fn production_in_flight_first_release_transition_kernel(",
            "pub proof fn production_in_flight_first_release_transition_refines_named_next(",
            "pub proof fn production_in_flight_first_release_snapshot_recovery_is_stutter(",
            "pub proof fn production_in_flight_first_release_terminal_owner_is_exclusive(",
            "terminal.canonical_wsv_owner ==> (",
        ),
    ),
    (
        "scripts/formal/run_sumeragi_v2_tlc.sh",
        (
            'readonly INFLIGHT_FIRST_RELEASE_RUNNER="${REPO_ROOT}/scripts/formal/'
            'run_sumeragi_v2_inflight_first_release.sh"',
            'bash "$INFLIGHT_FIRST_RELEASE_RUNNER"',
        ),
    ),
    (
        "scripts/write_sumeragi_v2_release_receipt.py",
        (
            "_APALACHE_LAYOUT_ONLY_RESULTS = (",
            '"kura-replica-retention"',
            '"SumeragiV2KuraReplicaRetention"',
            '"kura_replica_retention_fixed.cfg"',
            '"kura_replica_retention_fixed.cfg",\n        "8",',
            '"inflight-first-release-layout"',
            '"SumeragiV2InFlightFirstRelease"',
            '"inflight_first_release_fixed.cfg"',
            '"inflight_first_release_fixed.cfg",\n        "18",',
            "*_APALACHE_REFINEMENT_RESULTS",
            "*_APALACHE_LAYOUT_ONLY_RESULTS",
        ),
    ),
    (
        "pytests/scripts/sumeragi_v2_release_receipt_test.py",
        (
            'canonical.replace("result_count\\t6", "result_count\\t5", 1)',
            '"result\\tkura-replica-retention\\t"',
            '"kura_replica_retention_fixed.cfg\\t8\\tNoError\\t"',
            '"inflight_first_release_fixed.cfg\\t18\\tNoError\\t"',
            '"result\\tinflight-first-release-refinement\\t"',
            '"is not exact source-bound NoError evidence"',
        ),
    ),
)
