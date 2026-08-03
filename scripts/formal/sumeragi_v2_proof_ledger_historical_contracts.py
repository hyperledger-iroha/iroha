# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

REQUIRED_MODEL_MODULES = (
    "SumeragiV2Revision4",
    "SumeragiV2Revision4AdversarialSafety",
    "SumeragiV2",
    "SumeragiV2Quorums",
    "SumeragiV2QuorumProofs",
    "SumeragiV2Availability",
    "SumeragiV2Core",
    "SumeragiV2ResumeVoteWitness",
    "SumeragiV2CrashRecovery",
    "SumeragiV2Reconfiguration",
    "SumeragiV2VocabularyProofs",
    "SumeragiV2SafetyDefinitions",
    "SumeragiV2SafetyLemmas",
    "SumeragiV2AgreementLemmas",
    "SumeragiV2Inductive",
    "SumeragiV2InductiveProofs",
    "SumeragiV2Proofs",
    "SumeragiV2InstalledTcSelectorProofs",
    "SumeragiV2TimeoutDurability",
    "SumeragiV2TimeoutSigningInvariant",
    "SumeragiV2TimeoutViewInvariant",
    "SumeragiV2TimeoutWireAuthorization",
    "SumeragiV2ChainEpoch",
    "SumeragiV2ChainEpochProofs",
    "SumeragiV2ChainEpochRefinement",
    "SumeragiV2ChainReceiptAgreementProofs",
    "SumeragiV2SuccessorActivationRefinementProofs",
    "SumeragiV2ChainLivenessProofs",
    "SumeragiV2TemporalLemmas",
    "SumeragiV2LivenessProofs",
    "SumeragiV2ServiceRankLemmas",
    "SumeragiV2FiniteProducerEpisodes",
    "SumeragiV2FiniteProducerEpisodeProofs",
    "SumeragiV2EffectiveLockAcquisition",
    "SumeragiV2EffectiveLockAcquisitionProofs",
    "SumeragiV2AsyncNetwork",
    "SumeragiV2BeginTimeoutReadyProofs",
    "SumeragiV2RegularCommandFramedReadyProofs",
    "SumeragiV2RegularCommandExecutionReadyProofs",
    "SumeragiV2NonRegularCommandExecutionReadyProofs",
    "SumeragiV2CommandExecutionReadyProofs",
    "SumeragiV2ReplyRouteOwnership",
    "SumeragiV2ReplyRouteOwnershipProofs",
    "SumeragiV2ReplyRoutePipeline",
    "SumeragiV2ReplyRoutePipelineProofs",
    "SumeragiV2AsyncNetworkReplyRoutes",
    "SumeragiV2AsyncNetworkReplyRouteProofs",
    "SumeragiV2ReplyWriterDeadline",
    "SumeragiV2ReplyWriterDeadlineProofs",
    "SumeragiV2TypedRolloverHandoff",
    "SumeragiV2TypedRolloverHandoffProofs",
    "SumeragiV2AsyncFairnessRefinementProofs",
    "SumeragiV2CertifiedRequestHashAuthorityProofs",
    "SumeragiV2DurableDecisionRecoveryProofs",
    *(module for module, _ in ASYNC_LIVENESS_SHARDS),
    *ASYNC_CAUSAL_EPISODE_PROOF_MODULES,
    "SumeragiV2AsyncFiniteProducerEpisodes",
    *ASYNC_TEMPORAL_CLOSURE_PROOF_MODULES,
    *ADEQUATE_LEADER_CONTINUATION_PROOF_MODULES,
    ASYNC_LIVENESS_FACADE,
    "SumeragiV2AsyncHistoricalRecoveryLivenessProofs",
    "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
    "SumeragiV2LockedBodyProposalActionProofs",
    "SumeragiV2TerminalIngressLifecycleProofs",
    "SumeragiV2AutoscaleLifecycle",
    "SumeragiV2NativeApplicationEvidence",
    "SumeragiV2AutonomousReservationCarrier",
    "SumeragiTimeoutIngressGuardTest",
)

REQUIRED_TLC_CONFIGS = (
    "SumeragiV2Revision4.cfg",
    "SumeragiV2Revision4AdversarialSafety.cfg",
    "SumeragiV2Revision4Liveness.cfg",
    "quorum_count.cfg",
    "quorum_stake.cfg",
    "safety_count.cfg",
    "safety_stake.cfg",
    "chain_epoch.cfg",
    "liveness.cfg",
    "effective_lock_acquisition.cfg",
    "resume_locked_commit_witness.cfg",
)

REQUIRED_TLC_CONFIG_HEADERS = {
    "SumeragiV2Revision4.cfg": "SPECIFICATION Spec",
    "SumeragiV2Revision4AdversarialSafety.cfg": "SPECIFICATION Spec",
    "SumeragiV2Revision4Liveness.cfg": "SPECIFICATION PostGSTSpec",
    "quorum_count.cfg": "INIT Init\nNEXT QuorumCheckNext",
    "quorum_stake.cfg": "INIT Init\nNEXT QuorumCheckNext",
    "safety_count.cfg": "INIT Init\nNEXT Next",
    "safety_stake.cfg": "INIT Init\nNEXT Next",
    "chain_epoch.cfg": "SPECIFICATION ChainEpochTlcSpec",
    "liveness.cfg": "SPECIFICATION AsyncFiniteSpec",
    "effective_lock_acquisition.cfg": "SPECIFICATION AcquisitionSpec",
    "resume_locked_commit_witness.cfg": "SPECIFICATION CoreSpec",
}

_REPLY_ROUTE_FORMAL_SOURCE_SHA256 = {
    "SumeragiV2TemporalLemmas.tla": (
        "7ee323b1fb76922eca0addfc373bdd666723ad5380ce8b2c7800936af7a50eb3"
    ),
    "SumeragiV2ReplyRouteOwnership.tla": (
        "933aad7b639e91229c3e2056296ea1b2bb96f86f116fb39e41a2461b26f21e1f"
    ),
    "SumeragiV2ReplyRouteOwnershipProofs.tla": (
        "027d760650ce893c41badfdb2c12fc1f5dd470b43f39adbcb98ce31de6dd0680"
    ),
    "SumeragiV2ReplyRoutePipeline.tla": (
        "86ba03012eb3d25cb26fd4cd447430d0e324ee8fe75a2be4d53878d71a764d35"
    ),
    "SumeragiV2ReplyRoutePipelineProofs.tla": (
        "e58c5bab1f59dc3e9c08c999fc2df2b7c7b9c008a8bcd2a0dbc1b3ee90c8d77c"
    ),
    "SumeragiV2AsyncNetworkReplyRoutes.tla": (
        "3856e434243093287b94ce0799fc769ae458b156a694a1694165fb2fb2ff1c74"
    ),
    "SumeragiV2AsyncNetworkReplyRouteProofs.tla": (
        "d09f0029f5dc280359f222040cac6184f2ef3e85a4e1cef2e570416207a2befa"
    ),
    "SumeragiV2ReplyRouteOwnershipMutation.tla": (
        "9c5dd145561e0d9715623e1839ffc95d00fb75fa748ed67814a2a5458c4bb533"
    ),
    "SumeragiV2ReplyRoutePipelineMutation.tla": (
        "e371ba7e972ad99a7d1a73e06c0407b5935a7b9c2622b2de0044723840f5ed7f"
    ),
    "reply_route_fixed.cfg": (
        "36bff1ec06f4b517ec080a563faf72e32706bf7c656a63fef79e5af503b195cc"
    ),
    "reply_route_close_lifecycle_fixed.cfg": (
        "bccb8b2715951eec5e0e8383fda4704914a4fc004e9fc967ea56e575864a0e68"
    ),
    "reply_route_generation_epoch_fixed.cfg": (
        "bb7c3ec7da43e0355a1fdd1205d13221ecb9622dc7f8a796ba5f27bf32c24e74"
    ),
    "reply_route_capacity_overflow_fixed.cfg": (
        "8c624f5b3281a03b91e4a3c5ec162011bc12247a733b429d64dd2ee6acb735cb"
    ),
    "reply_route_cursor_reset_bug.cfg": (
        "4b668adc3f6e9ceef0e9deb7f7b8cc9644b15bbf3ee842e4c1ab725348a4410c"
    ),
    "reply_route_source_replacement_bug.cfg": (
        "3888e0ccd84c70cd9dc8cb0803d1f41f85585034be5c0e7ddfefd5d85d68fd84"
    ),
    "reply_route_target_substitution_bug.cfg": (
        "9d1b9c6e4925ec059ea2bea9db1e421e5a932d982698072331f983332903bda0"
    ),
    "reply_route_ticket_payload_reuse_bug.cfg": (
        "6d7ee13a9fc9104e3cfa7659e32061e44c0e978f677555f777a7878213bbd7eb"
    ),
    "reply_route_reconnect_sibling_ticket_bug.cfg": (
        "509834a75b68945542d8b9992ddea0b42f65d764fa86d1a165eb5b7b5b055b73"
    ),
    "reply_route_retired_ordinal_collision_bug.cfg": (
        "5d73de48101fdd195d3259225a22f07ecdf2e5d37c503aaf6432dda7d3e23cd4"
    ),
    "reply_route_intrinsic_tenure_substitution_bug.cfg": (
        "6d88a80acb87011388c07379b0a9f62d813a1d8b36fa1bc4c1aac859d2e8018f"
    ),
    "reply_route_source_capacity_substitution_bug.cfg": (
        "953e589ee5838982f683bc44e9d6a9764ddb9633b54d842901cf930544f18097"
    ),
    "reply_route_pipeline_fixed.cfg": (
        "cf061676c290d54b0d65683b426cf812bbcdca5fc88ed095384e2d81dcd2d308"
    ),
    "reply_route_pipeline_generation_epoch_fixed.cfg": (
        "194522935927aa5bb9c1fd72ee05c2d52ffdabb0c5ad6ddff5e43ae229515014"
    ),
    "reply_route_pipeline_capacity_overflow_fixed.cfg": (
        "f9a4b631a8b733a65799517b923bae98c3efbabab770d2a223eda8ac7c7d7ff7"
    ),
    "reply_route_pipeline_replay_isolation_fixed.cfg": (
        "bfeaa5d6f7c31c5b8b0ec28e47f54e89a124dbdc8c862ad70957e605bf68f830"
    ),
    "reply_route_pipeline_replay_step_bug.cfg": (
        "2eba555c8c84c195feb92898bfb6fea602f838739a087d77311af4dbbed795d1"
    ),
    "reply_route_pipeline_source_isolation_bug.cfg": (
        "91ac8058ae1d70b54333c084f509930ee490b611d6d5087c2906673305b36582"
    ),
    "reply_route_pipeline_unfair_attach_bug.cfg": (
        "98fcbf63fd63e7411e2c66578ef42a7d4acbb23d15b597fb2c49aa17daaf71e2"
    ),
    "reply_route_pipeline_fifo_bypass_bug.cfg": (
        "77ce9ac783e6e88f17852ca111785a7420b870cb79eee7753589ebc0949020e9"
    ),
    "reply_route_pipeline_cursor_regression_bug.cfg": (
        "494649a8ef88926ce046d291122d28bfe635c0abd93ad5e28a6a272db31fa3fa"
    ),
    "reply_route_pipeline_ticket_reuse_bug.cfg": (
        "169bcbdfad9d0bf9f3bea6d3cf6973cec56b8364d698ff5a90240a1ed65f3f62"
    ),
    "reply_route_pipeline_premature_reconnect_bug.cfg": (
        "6f6e0a816212cf1b4b1c4ce4aec4ecc3ec64368412fb76223939916ae9d7114e"
    ),
    "reply_route_pipeline_reconnect_observation_not_ready_bug.cfg": (
        "112a119aa7edbc70b0550b77c813058c1a0f17ad2b293efc877ab06672e486b1"
    ),
    "reply_route_pipeline_old_flush_double_apply_bug.cfg": (
        "7a4830d27f15e4036986397fd46e8195fc8cc769a05f8d4c669f405543b28be5"
    ),
    "reply_route_pipeline_source_class_writer_fixed.cfg": (
        "f9bb6f2858b7611039cd3c8c32165b31f2ed3c416506278cb74ad3eab3f1354a"
    ),
    "reply_route_pipeline_cross_semantic_close_cycle_bug.cfg": (
        "951d89813fd017ce79d49ae4053fb5099001a610c7190f9925f0bd771b282571"
    ),
}

_REPLY_ROUTE_LIFECYCLE_INSTANCE_CONTRACTS = (
    (
        "SumeragiV2ReplyRouteOwnershipMutation.tla",
        "MutationRoute",
        "SumeragiV2ReplyRouteOwnership",
        (
            ("rrSemanticSequence", "semanticSequence"),
            ("rrSemanticHash", "semanticHash"),
            ("rrRequesterNextSequence", "requesterNextSequence"),
            ("rrRequesterClosedThrough", "requesterClosedThrough"),
            ("rrClosePendingThrough", "closePendingThrough"),
            ("rrCloseSentThrough", "closeSentThrough"),
            ("rrCloseAcknowledgedThrough", "closeAcknowledgedThrough"),
            ("rrCloseRetryGeneration", "closeRetryGeneration"),
            ("rrServiceGeneration", "serviceGeneration"),
            ("rrResponderGeneration", "responderGeneration"),
            (
                "rrDurableResponderGeneration",
                "durableResponderGeneration",
            ),
            (
                "rrRequesterNextStreamEpoch",
                "requesterNextStreamEpoch",
            ),
            ("rrRequesterStreamEpoch", "requesterStreamEpoch"),
            ("rrCloseStreamEpoch", "closeStreamEpoch"),
            ("rrClosedPrefix", "closedPrefix"),
            (
                "rrAttemptLifecycleIdentities",
                "attemptLifecycleIdentities",
            ),
            ("rrPendingHintResets", "pendingHintResets"),
            (
                "rrDiscardedPartialIdentities",
                "discardedPartialIdentities",
            ),
        ),
    ),
    (
        "SumeragiV2ReplyRoutePipelineMutation.tla",
        "MutationPipeline",
        "SumeragiV2ReplyRoutePipeline",
        (
            ("rrSemanticSequence", "semanticSequence"),
            ("rrSemanticHash", "semanticHash"),
            ("rrRequesterNextSequence", "requesterNextSequence"),
            ("rrRequesterClosedThrough", "requesterClosedThrough"),
            ("rrClosePendingThrough", "closePendingThrough"),
            ("rrCloseSentThrough", "closeSentThrough"),
            ("rrCloseAcknowledgedThrough", "closeAcknowledgedThrough"),
            ("rrCloseRetryGeneration", "closeRetryGeneration"),
            ("rrServiceGeneration", "serviceGeneration"),
            ("rrResponderGeneration", "responderGeneration"),
            (
                "rrDurableResponderGeneration",
                "durableResponderGeneration",
            ),
            (
                "rrRequesterNextStreamEpoch",
                "requesterNextStreamEpoch",
            ),
            ("rrRequesterStreamEpoch", "requesterStreamEpoch"),
            ("rrCloseStreamEpoch", "closeStreamEpoch"),
            ("rrClosedPrefix", "closedPrefix"),
            (
                "rrAttemptLifecycleIdentities",
                "attemptLifecycleIdentities",
            ),
            ("rrPendingHintResets", "pendingHintResets"),
            (
                "rrDiscardedPartialIdentities",
                "discardedPartialIdentities",
            ),
        ),
    ),
    (
        "SumeragiV2AsyncNetworkReplyRoutes.tla",
        "AsyncReplyRoute",
        "SumeragiV2ReplyRouteOwnership",
        (
            ("rrSemanticSequence", "asyncReplySemanticSequence"),
            ("rrSemanticHash", "asyncReplySemanticHash"),
            (
                "rrRequesterNextSequence",
                "asyncReplyRequesterNextSequence",
            ),
            (
                "rrRequesterClosedThrough",
                "asyncReplyRequesterClosedThrough",
            ),
            ("rrClosePendingThrough", "asyncReplyClosePendingThrough"),
            ("rrCloseSentThrough", "asyncReplyCloseSentThrough"),
            (
                "rrCloseAcknowledgedThrough",
                "asyncReplyCloseAcknowledgedThrough",
            ),
            (
                "rrCloseRetryGeneration",
                "asyncReplyCloseRetryGeneration",
            ),
            ("rrServiceGeneration", "asyncReplyServiceGeneration"),
            ("rrResponderGeneration", "asyncReplyResponderGeneration"),
            (
                "rrDurableResponderGeneration",
                "asyncReplyDurableResponderGeneration",
            ),
            (
                "rrRequesterNextStreamEpoch",
                "asyncReplyRequesterNextStreamEpoch",
            ),
            (
                "rrRequesterStreamEpoch",
                "asyncReplyRequesterStreamEpoch",
            ),
            ("rrCloseStreamEpoch", "asyncReplyCloseStreamEpoch"),
            ("rrClosedPrefix", "asyncReplyClosedPrefix"),
            (
                "rrAttemptLifecycleIdentities",
                "asyncReplyAttemptLifecycleIdentities",
            ),
            ("rrPendingHintResets", "asyncReplyPendingHintResets"),
            (
                "rrDiscardedPartialIdentities",
                "asyncReplyDiscardedPartialIdentities",
            ),
        ),
    ),
    (
        "SumeragiV2AsyncNetworkReplyRouteProofs.tla",
        "AsyncReplyRouteProofs",
        "SumeragiV2ReplyRouteOwnershipProofs",
        (
            ("rrSemanticSequence", "asyncReplySemanticSequence"),
            ("rrSemanticHash", "asyncReplySemanticHash"),
            (
                "rrRequesterNextSequence",
                "asyncReplyRequesterNextSequence",
            ),
            (
                "rrRequesterClosedThrough",
                "asyncReplyRequesterClosedThrough",
            ),
            ("rrClosePendingThrough", "asyncReplyClosePendingThrough"),
            ("rrCloseSentThrough", "asyncReplyCloseSentThrough"),
            (
                "rrCloseAcknowledgedThrough",
                "asyncReplyCloseAcknowledgedThrough",
            ),
            (
                "rrCloseRetryGeneration",
                "asyncReplyCloseRetryGeneration",
            ),
            ("rrServiceGeneration", "asyncReplyServiceGeneration"),
            ("rrResponderGeneration", "asyncReplyResponderGeneration"),
            (
                "rrDurableResponderGeneration",
                "asyncReplyDurableResponderGeneration",
            ),
            (
                "rrRequesterNextStreamEpoch",
                "asyncReplyRequesterNextStreamEpoch",
            ),
            (
                "rrRequesterStreamEpoch",
                "asyncReplyRequesterStreamEpoch",
            ),
            ("rrCloseStreamEpoch", "asyncReplyCloseStreamEpoch"),
            ("rrClosedPrefix", "asyncReplyClosedPrefix"),
            (
                "rrAttemptLifecycleIdentities",
                "asyncReplyAttemptLifecycleIdentities",
            ),
            ("rrPendingHintResets", "asyncReplyPendingHintResets"),
            (
                "rrDiscardedPartialIdentities",
                "asyncReplyDiscardedPartialIdentities",
            ),
        ),
    ),
)

_REPLY_WRITER_DEADLINE_FORMAL_SOURCE_SHA256 = {
    "SumeragiV2ReplyWriterDeadline.tla": (
        "5caf1211e920b52c1520026f180a6cb467887a51e181c3ec5a43da737e3e7832"
    ),
    "SumeragiV2ReplyWriterDeadlineProofs.tla": (
        "e0e966240be32dff05947504de4d9d78c9028bc323cf37628b419d6d649d9ed2"
    ),
    "SumeragiV2ReplyWriterDeadlineMutation.tla": (
        "c40644faef6753a253139b713f8b19f575118357afd470e1491ff10c9795c79f"
    ),
    "reply_writer_deadline_fixed.cfg": (
        "7f235f5c76745d8eb3dfa80ff5e9cd03668483fbc0b600c6155279c47e6f861a"
    ),
    "reply_writer_deadline_responsive_fixed.cfg": (
        "55b57e27ab16d282f0247d1185f4f514c55b52f761dc9cc992e6bc1f431c26f5"
    ),
    "reply_writer_deadline_mutation_fixed.cfg": (
        "0a3465127a4968e41ad6f48122eb984a51c0c53cec58f408258814cb01593c07"
    ),
    "reply_writer_deadline_retry_reset_bug.cfg": (
        "134f3d0f036d5d52fde56fd03cc654ee6f7df68167a2bcacd2e61d68039aaf03"
    ),
    "reply_writer_deadline_timeout_as_flush_bug.cfg": (
        "e0c3e0f3265733442bd5a34c0b49b16605f22b463cffec1e741efd7950cc4983"
    ),
    "reply_writer_deadline_closed_attempt_bug.cfg": (
        "4c6371d8a0ec1d3e29c885852be010b2bf9915bf49c6795c1c6eef048ecef977"
    ),
    "reply_writer_deadline_reconnect_reset_bug.cfg": (
        "a96e9f5b4bb96bdade968f5e2a39a6f3457f439a63bfd0d87be74a4416541683"
    ),
    "reply_writer_deadline_uncapped_attempt_bug.cfg": (
        "0d78801e07703e6b43e31c4865915ba3b6147994668fbdf855d7edc54c7dce9e"
    ),
    "reply_writer_deadline_topology_deadline_bug.cfg": (
        "f5c9fb260a806076ded895a477b0dc06e0188d3f196fa80beb2c8e03a70a5cba"
    ),
    "reply_writer_deadline_replacement_kill_bug.cfg": (
        "f9bf8d0a2e8ebbdd9c4fbb88354fc8158fc85c4bada2c7a2dea659c0fd6ed1a2"
    ),
    "reply_writer_deadline_timeout_beats_flush_bug.cfg": (
        "a779bfe7e8a4988e117efb47bd5c9ce0b0ac707eaa22412970c816fb4c62da0b"
    ),
    "reply_writer_deadline_wrong_attempt_flush_bug.cfg": (
        "d480a12b8fe811451a58332e41a7474e7335ed76bc00bc5dedb1f030a9486214"
    ),
    "reply_writer_deadline_close_ready_flush_bug.cfg": (
        "d9785bfa9a4e7be0bd302c25251c1a091e1220375350445aca9cb6758a9131b5"
    ),
    "reply_writer_deadline_retire_ready_flush_bug.cfg": (
        "d7856fbbc697ec857f0ff2762afa802945a6a9c1efbb1d6c02e38416d80d1ac6"
    ),
    "reply_writer_deadline_erase_ready_witness_bug.cfg": (
        "c0337e89cafc899943faf2dd899bf542ed5780c1c431aecbedaf2d89f1957f33"
    ),
}

_REPLY_WRITER_DEADLINE_MUTATION_RUNNER_SHA256 = (
    "b18a14944ca8ece675b70307f3d091112d07b92ca31513612c47cdb152722729"
)

DORMANT_REPLY_CLOCK_MUTATION_ARTIFACTS = (
    "SumeragiV2DormantReplyClockMutation.tla",
    "dormant_reply_clock_fixed.cfg",
    "dormant_reply_clock_all_due_bug.cfg",
)

_TYPED_ROLLOVER_HANDOFF_FORMAL_SOURCE_SHA256 = {
    "SumeragiV2TypedRolloverHandoff.tla": (
        "e81da94ee4ead2b9183819a5eb733082267429ee89b7caea23b686dd007ca77c"
    ),
    "SumeragiV2TypedRolloverHandoffProofs.tla": (
        "aca0335d9f48ddcca007f2e30e1f175a152ee05f199de89f4bc208e2d79d0fbe"
    ),
    "SumeragiV2TypedRolloverHandoffMutation.tla": (
        "21de53c3bf6d7853e47faf5a0009ebad453966dd36e919b88481c0bca0f53378"
    ),
    "SumeragiV2TypedRolloverHandoffLivenessMutation.tla": (
        "390f09fcb7528438314673f184012cad37262b81a0907665e7408b04c9f0e1ad"
    ),
    "SumeragiV2TypedRolloverHandoffRepeatedHandoffMutation.tla": (
        "8995252082e80cbab0649f98aded2c98b59b7ab74839e02457ca73f5bb4a3d23"
    ),
    "typed_rollover_handoff_fixed.cfg": (
        "7e0f8d0f8b27a3266725a00350e49a5f24b0f6433f0467203a4d2dc578f162b9"
    ),
    "typed_rollover_handoff_responsive_durable_liveness.cfg": (
        "559e03ec54e45e1395e2a9fdf7ed629d9a1e769a64034a58ec095c721a18953e"
    ),
    "typed_rollover_handoff_responsive_restart_restore_liveness.cfg": (
        "272919a959f883e20c013e60adc05b2b3175eb6601727d72b2471214cb65f840"
    ),
    "typed_rollover_handoff_accept_semantic_invalid_lifecycle_state_bug.cfg": (
        "c3659f678de35e046075a4b917c0ff946b3945d6da818c5d62ba712ab424bad3"
    ),
    "typed_rollover_handoff_active_state_roll_bug.cfg": (
        "6e54a37be02494c53de0c27d282c8996064e89703ca45232aaf8429db8d7465c"
    ),
    "typed_rollover_handoff_changed_roster_without_generation_advance_bug.cfg": (
        "e342421faaf96607a0127680fa97a9329b0bff9d4633cc84b10a0d010d328e38"
    ),
    "typed_rollover_handoff_clean_state_slot_v3_persistence_failure_bug.cfg": (
        "4ec02dd05d719f732346cfc09ba2423ce07b71e0dce6ffd912a7b5e8cdbf6d2d"
    ),
    "typed_rollover_handoff_clean_crash_after_lifecycle_root_v3_commit_bug.cfg": (
        "77b166d9480ce848e9b818ced2eb025d4cfaeb47acb2745ca9fc86adb968044e"
    ),
    "typed_rollover_handoff_clean_foreign_owner_reject_bug.cfg": (
        "9e9fc9e5c1b98b010089a21921b45a4ab4af653fcb542d92c9388c6bd09d06ec"
    ),
    "typed_rollover_handoff_clean_late_enqueue_reject_bug.cfg": (
        "16085a16c185d7c009046f112e8ac35902b7b506a7f28780bd9f2891ee90c967"
    ),
    "typed_rollover_handoff_clean_predecessor_artifact_reject_bug.cfg": (
        "9cdfccfe85d63ad27492168e650859005188b9b6cbda7134c12379a6fcaaf591"
    ),
    "typed_rollover_handoff_clean_predecessor_context_reject_bug.cfg": (
        "0d62a12bb77eb622594368554e4af00282f1c044d4e4b953b27618e786d51962"
    ),
    "typed_rollover_handoff_clean_wrong_successor_reject_bug.cfg": (
        "b30f0be811e137ad56880e644f3e1a6deb90a113c54e0e44738e7d86469d1b4d"
    ),
    "typed_rollover_handoff_cleanup_before_root_parent_resync_bug.cfg": (
        "b33f2ddd6417c8b43eaca226687edcd28bcddabc2d64efc74eaded6c83c2971f"
    ),
    "typed_rollover_handoff_cleanup_before_validation_bug.cfg": (
        "abe8de889668f65a9e47e721c32bbd57d5ecbf52f6ff2cde9dafe2b5408241ab"
    ),
    "typed_rollover_handoff_cleanup_retains_inactive_slot_bug.cfg": (
        "baa273c35b7a7f2d031988bd984498a16e556ce46ca7b930ca7712782a11c09a"
    ),
    "typed_rollover_handoff_cross_service_transport_owner_pair_bug.cfg": (
        "ad51095a5ede9c012752e8287ef3519e2ad77365f67402a3fee4140180194d23"
    ),
    "typed_rollover_handoff_crossed_root_shape_bug.cfg": (
        "77c3e97f34ca11c01418495b1e95e7999267ffb95499764eace57ca77d07febc"
    ),
    "typed_rollover_handoff_epoch_overflow_bug.cfg": (
        "ba66428dbb7f621a75246892029a8bb48c8cd086b6cb12b5c934f1e082a6ba06"
    ),
    "typed_rollover_handoff_lose_requester_incarnation_after_crash_bug.cfg": (
        "a9585ebdaa2f3ba3e5ce694eecde958abb2f7ddd1ca2bc3a22f91df0345e007f"
    ),
    "typed_rollover_handoff_epoch_use_before_persist_bug.cfg": (
        "597fe537a1fdc98005171c056eb35b38f0a62dd78855ca13e5678df2c9cc7da8"
    ),
    "typed_rollover_handoff_foreign_candidate_ignored_bug.cfg": (
        "047275a974264b953baf70238e493325099c2460b005dbb7a04ae12d5413f465"
    ),
    "typed_rollover_handoff_foreign_receipt_bug.cfg": (
        "6c4bc9b411e21f150a23dfa2f709bc2a34589e7a895e930decf82df6ad14054a"
    ),
    "typed_rollover_handoff_foreign_successor_bug.cfg": (
        "d2c662e48d933b2e8118fcce3b8e78fbdd2cb9fddea82b59f14dde82f7e556f5"
    ),
    "typed_rollover_handoff_forged_authenticated_close_prefix_bug.cfg": (
        "b5b69ccaa8766b0a7f210aedb000d55db0cc851a2e1ee25ee62de1dd49a36757"
    ),
    "typed_rollover_handoff_generation_overflow_bug.cfg": (
        "1a91a8ad2158774310e18251447b6643863e50068169bfc62530263fc6b9bb2e"
    ),
    "typed_rollover_handoff_late_callback_bug.cfg": (
        "34ee3c2d0c682de0925613783449d5dfbb15254142f34a85b0188128fe93d31b"
    ),
    "typed_rollover_handoff_late_enqueue_bug.cfg": (
        "13016842c0df21e6d5efdd7a6f858619770a2c26304347afe4926ae4456bdb3e"
    ),
    "typed_rollover_handoff_missing_selected_state_bug.cfg": (
        "24b4db0f1fa54017035999b389464141627f51ed1279e460f4e837bfb84470c2"
    ),
    "typed_rollover_handoff_missing_validated_cleanup_fairness_bug.cfg": (
        "9a3923176670b6ade2393062cdf33cf6e6f92360e2dd25b3e446ec126161b641"
    ),
    "typed_rollover_handoff_missing_worker_clear_fairness_bug.cfg": (
        "8306d4b381b4b11fc259816e053067be2205451d972def82070ad4079efee676"
    ),
    "typed_rollover_handoff_predecessor_artifact_accept_bug.cfg": (
        "4d337f7b55fe2ea2dd3d5ecbba8768f9880f54c33709369692103422e01c1318"
    ),
    "typed_rollover_handoff_predecessor_context_accept_bug.cfg": (
        "f362f2f8faa173ea7da8572237e80f098e5020418e17fd4ec2aa9ae5181e19bf"
    ),
    "typed_rollover_handoff_premature_mint_bug.cfg": (
        "191e34d9b4e3897f281e63a15999e9ca761d579a11308d8c37edb5bbb5dfdc0d"
    ),
    "typed_rollover_handoff_preserve_process_receipt_across_crash_bug.cfg": (
        "dc6828172645ec6cac3f57de2a8a4f507dafaabf98230a817730c108d8d23cb5"
    ),
    "typed_rollover_handoff_publish_memory_before_lifecycle_root_v3_commit_bug.cfg": (
        "66b9b2ce6045be01c9225ed904ec397cdc43a7ee82decba080cb8198f5afe08f"
    ),
    "typed_rollover_handoff_retry_loss_bug.cfg": (
        "7c30dbe724a751aec3c012083493c1dee4fde6d192e4a88f50b144c6dfda3f35"
    ),
    "typed_rollover_handoff_reuse_root_selected_state_slot_bug.cfg": (
        "b8f82d925fdb7b8e98033943640b711aa596991f8bdcb3cdca337cb9dbe9bd0c"
    ),
    "typed_rollover_handoff_root_commit_before_state_slot_bug.cfg": (
        "9d94abe53148e124adacef888e2f54a75648c7e521fac23da5d07da866d3ec63"
    ),
    "typed_rollover_handoff_root_generation_overflow_bug.cfg": (
        "94242eac961422c567fed5da2f4c72479d719ab7887be9bcfaf3cdc5c1dd2e29"
    ),
    "typed_rollover_handoff_same_roster_generation_roll_bug.cfg": (
        "c1f8e1ecba1e2e7ef1c5aa5e77287795f6514f0c7d7a5dabf298dd707fb219a6"
    ),
    "typed_rollover_handoff_skip_bootstrap_crash_history_bug.cfg": (
        "5a0c0045f81b636c42d42b9a32129c2f3301be1e7be7d32f7a26af5d72d1cbc3"
    ),
    "typed_rollover_handoff_skip_lifecycle_root_v3_crash_history_bug.cfg": (
        "641626c1a0cc96125649d40dbd73b4432e153ef275a7567453c7c095f5a61d48"
    ),
    "typed_rollover_handoff_recover_uncommitted_state_slot_bug.cfg": (
        "24ed862807885af3f8f398a76cd5699ffac6a4d41a682a891d0ec76d358595f3"
    ),
    "typed_rollover_handoff_repeated_handoff_after_restart_restore_bug.cfg": (
        "2f826575880769249c8b9a72482d1dfc41a91feab31f41ca770d0b77771ea2ba"
    ),
    "typed_rollover_handoff_split_generation_hash_bug.cfg": (
        "f2e349bc658353a272add46cf9e3c246150c9e10c6182eecce27caec56c11a3d"
    ),
    "typed_rollover_handoff_untyped_force_bug.cfg": (
        "c725dc6aa339a63eea289cdcbebf557cb45c6df45a1f63aea4ca32f8b22655b7"
    ),
    "typed_rollover_handoff_wrong_bootstrap_lifecycle_projection_bug.cfg": (
        "a916601a43baa2a7d85fe77670966890b81e85ea741eeabeaadab3170f9ee871"
    ),
}

_TYPED_ROLLOVER_HANDOFF_MUTATION_RUNNER_SHA256 = (
    "e1ac1e03c10f2667eb4254b99fa9cd60955417fc39540b7697f8721364313340"
)

_TYPED_ROLLOVER_MODEL_SAFETY_PROOFLESS_THEOREMS: tuple[str, ...] = ()
_TYPED_ROLLOVER_MODEL_SAFETY_PROVED_THEOREMS = (
    "TypedRolloverInitEstablishesSafetyObligation",
    "BootstrapRootHasExactGenerationZeroShapeObligation",
    "FreshBootstrapUsesTargetGeometryEpochZeroObligation",
    "BootstrapStateReplacementRequiresDirectorySyncObligation",
    "BootstrapCrashRecoveryObligation",
    "BootstrapFirstCommitSelectsExactInitialPairObligation",
    "ExactOwnerPairRequiredForRetainedHandoffObligation",
    "TypedRolloverNextPreservesSafetyObligation",
    "EveryCrashDropsProcessLocalAuthorityObligation",
    "OnlyValidatedRestartMayFenceAfterCrashObligation",
    "StateReplacementRequiresDirectorySyncBeforeRootReplacementObligation",
    "RootReplacementRequiresStoreSyncBeforeMemoryPublicationObligation",
    "RootSelectedPairBindsGenerationAndDigestObligation",
    "MissingRootSelectedStateCannotValidateOrCleanupObligation",
    "ValidationFailurePreservesArtifactsObligation",
    "SemanticValidationPrecedesArtifactCleanupObligation",
    "ValidatedCleanupRemovesInactiveSlotObligation",
    "SecondCrashBeforeRootResyncPreservesPredecessorObligation",
    "RootGenerationAdvancesExactlyOnceAndAlternatesSlotObligation",
    "MemoryPublicationRequiresCommittedV3RootObligation",
    "OrdinaryRolloverRequiresAuthenticatedTerminalityObligation",
    "DurableExactOutputAuthorityMayFenceActiveStateObligation",
    "ValidatedRestartAuthorityMayFenceActiveStateObligation",
    "ActiveOrdinaryRolloverReturnsCapacityAtomicallyObligation",
    "SameRosterFullTableReturnsCapacityAtomicallyObligation",
    "ServiceGenerationOverflowReturnsCapacityAtomicallyObligation",
    "RootGenerationExhaustionPoisonsJournalFailAtomicallyObligation",
    "EpochOverflowReturnsCapacityAtomicallyObligation",
    "CrashBeforeRootCommitRestoresPredecessorObligation",
    "CrashAfterRootCommitRestoresSuccessorObligation",
    "FreshEpochPersistencePrecedesExactUseObligation",
    "CrashRestoresExactRequesterIncarnationObligation",
    "SameRosterPreservesTransportWithoutGenerationRollObligation",
    "ForcedFenceCannotForgeAuthenticatedClosePrefixObligation",
    "LateOldCallbackCannotMutateSuccessorObligation",
    "TypedRolloverSpecAlwaysSafeObligation",
    "ResponsiveDurableExactOutputRolloverLivenessObligation",
    "ResponsiveRestartRestoreRolloverLivenessObligation",
)
_TYPED_ROLLOVER_LOCAL_LIVENESS_PROOFLESS_THEOREMS: tuple[str, ...] = ()
_PROOFLESS_RELEASE_SUPPORT_BY_THEOREM = {
    (
        "SumeragiV2TypedRolloverHandoffProofs",
        symbol,
    ): "typed-rollover-handoff-model-safety"
    for symbol in _TYPED_ROLLOVER_MODEL_SAFETY_PROOFLESS_THEOREMS
}
_PROOFLESS_RELEASE_SUPPORT_BY_THEOREM.update(
    {
        ("SumeragiV2TypedRolloverHandoffProofs", symbol): (
            "typed-rollover-handoff-conditional-local-liveness"
        )
        for symbol in _TYPED_ROLLOVER_LOCAL_LIVENESS_PROOFLESS_THEOREMS
    }
)
_PROOFLESS_RELEASE_SUPPORT_BY_THEOREM.update(
    {
        (module, symbol): support_id
        for support_id, (module, symbols) in (
            SUPPORT_PROOF_OBLIGATION_INVENTORY.items()
        )
        if support_id not in DEDUCTIVELY_PROVED_SUPPORT_IDS
        for symbol in symbols.split(" / ")
    }
)
_PROOFLESS_RELEASE_DIRECT_CONSUMER_BY_THEOREM: dict[
    tuple[str, str], str
] = {}

RETIRED_PATHS = (
    ROOT_DIR / "docs" / "formal" / "sumeragi",
    ROOT_DIR / "scripts" / "formal" / "sumeragi_apalache.sh",
    ROOT_DIR / "scripts" / "formal" / "sumeragi_tlc.sh",
    ROOT_DIR / "scripts" / "formal" / "check_sumeragi_formal_coverage.py",
    ROOT_DIR / "ci" / "check_sumeragi_formal_expected_failures.sh",
    ROOT_DIR / "pytests" / "scripts" / "sumeragi_formal_coverage_test.py",
)

# The first-release proof models the scheduler and transport explicitly in
# AsyncSpec.  These names belonged to the former favourable-network corridor,
# which encoded the desired progress steps directly into a second protocol
# relation and could therefore make a circular liveness claim look proved.
RETIRED_LIVENESS_SYMBOLS = (
    "ReliableBeginTimeout",
    "ReliableNext",
    "ReliableNextV2",
    "ReliableActionFairness",
    "LivenessSpec",
    "StableProgressContracts",
)

# These predicates mention proof-only global history.  They may be used in
# inductive lemmas, but never as executable guards on the protocol actions
# whose provenance the proof is supposed to derive.
REACHABLE_ACTION_ORACLES = {
    "FormPrepareQC": ("CertificateHonestIntentBacked", "QcValid"),
    "FormCommitQC": ("CertificateHonestIntentBacked", "QcValid"),
    "DeliverQC": ("CertificateHonestIntentBacked", "QcValid"),
    "BeginTimeout": ("HighRefValid", "CertificateHonestIntentBacked"),
}

# Release safety is proved for one arbitrary frozen height context.  The old
# genesis-only ``Spec``/``NextV2`` wrappers include a global application
# barrier and therefore cannot discharge any of these obligations.
ARBITRARY_CONTEXT_SAFETY_OBLIGATIONS = {
    "durable-vote-uniqueness": "DurableVoteUniquenessObligation",
    "lock-monotonicity": "LockMonotonicityObligation",
    "external-validity": "ExternalValidityObligation",
    "certified-body-availability": "AvailabilityObligation",
    "certificate-uniqueness": "CertificateUniquenessObligation",
    "same-round-lock-and-commit-authorization": (
        "SameRoundLockAndCommitAuthorizationObligation"
    ),
    "timeout-protection": "TimeoutProtectionObligation",
    "agreement": "AgreementObligation",
    "no-conflicting-commit-qcs": "NoConflictingCommitCertificatesObligation",
    "crash-restart": "CrashRecoveryObligation",
}
ARBITRARY_CONTEXT_SAFETY_PROPERTY_WRAPPERS = {
    "durable-vote-uniqueness": "DurableVoteUniquenessProperty",
    "lock-monotonicity": "LockMonotonicityProperty",
    "external-validity": "ExternalValidityProperty",
    "certified-body-availability": "CertifiedBodyAvailabilityProperty",
    "certificate-uniqueness": "CertificateUniquenessProperty",
    "same-round-lock-and-commit-authorization": (
        "SameRoundLockAndCommitAuthorizationProperty"
    ),
    "timeout-protection": "TimeoutProtectionProperty",
    "agreement": "AgreementProperty",
    "no-conflicting-commit-qcs": "NoConflictingCommitCertificatesProperty",
    "crash-restart": "CrashRecoveryProperty",
}

# These are properties of the concrete asynchronous scheduler and transport,
# not wrappers that may be stated in an upstream safety module.
ASYNC_LIVENESS_OBLIGATIONS = {
    "post-decision-timeout-exclusion": "PostDecisionTimeoutExclusionObligation",
    "decision-recovery-across-restart": "DecisionRecoveryAcrossRestartObligation",
    "generation-scoped-vote-delivery": "GenerationScopedVoteDeliveryObligation",
    "post-gst-deadlock-freedom": "DeadlockFreedomObligation",
    "protected-service-rank-stage4-ready-causal": (
        "ProtectedStage4RankProgressFromFairScheduler"
    ),
    "protected-service-rank-serve-fifo": (
        "ProtectedServeRankProgressFromFairFifo"
    ),
    "protected-service-rank-stage5-consensus-fifo": (
        "ProtectedStage5RankProgressFromFairFifo"
    ),
    "protected-service-rank": "ProtectedServiceRankProgressObligation",
    "post-gst-starvation-freedom": "StarvationFreedomObligation",
    "timeout-view-liveness": (
        "AsyncTemporalClosureTimeoutViewProgressObligation"
    ),
    "rotating-leader-liveness": (
        "AsyncTemporalClosureRotatingLeaderProgressObligation"
    ),
    "locked-body-reproposal": (
        "AsyncTemporalClosureLockedBodyReproposalProgressObligation"
    ),
    "application-liveness": (
        "AsyncTemporalClosureApplicationCompletionProgressObligation"
    ),
}
ASYNC_LIVENESS_OBLIGATION_MODULES = {
    "timeout-view-liveness": "SumeragiV2AsyncTemporalClosureProofs",
    "rotating-leader-liveness": "SumeragiV2AsyncTemporalClosureProofs",
    "locked-body-reproposal": "SumeragiV2AsyncTemporalClosureProofs",
    "application-liveness": "SumeragiV2AsyncTemporalClosureProofs",
}
ASYNC_LIVENESS_PROPERTY_WRAPPERS = {
    "post-decision-timeout-exclusion": "PostDecisionTimeoutExclusionProperty",
    "decision-recovery-across-restart": "DecisionRecoveryAcrossRestartProperty",
    "generation-scoped-vote-delivery": "GenerationScopedVoteDeliveryProperty",
    "progress-witness-preservation": (
        "AsyncProgressWitnessAndHistoricalRecoveryProperty"
    ),
    "post-gst-deadlock-freedom": "DeadlockFreedomProperty",
    "protected-service-rank-stage4-ready-causal": (
        "ProtectedStage4RankProgressProperty"
    ),
    "protected-service-rank-serve-fifo": "ProtectedServeRankProgressProperty",
    "protected-service-rank-stage5-consensus-fifo": (
        "ProtectedStage5RankProgressProperty"
    ),
    "protected-service-rank": "ProtectedServiceRanksProgressProperty",
    "post-gst-starvation-freedom": "StarvationFreedomProperty",
    "timeout-view-liveness": "TimeoutViewProgressProperty",
    "rotating-leader-liveness": "RotatingLeaderProgressProperty",
    "locked-body-reproposal": "LockedBodyReproposalProgressProperty",
    "application-liveness": "ApplicationCompletionProgressProperty",
}

# Most asynchronous obligations apply one property wrapper directly to the
# frozen-context specification. Productive deadlock freedom intentionally
# instantiates the parameterized wrapper with the exact terminating-local-work
# projection instead; accepting the legacy one-argument wrapper here would
# disconnect the per-validator runner-service contract from the ledgered
# theorem.
ASYNC_LIVENESS_EXACT_STATEMENTS = {
    "post-gst-deadlock-freedom": (
        "\\A initialContext: "
        "DeadlockFreedomWithLocalWorkProperty(AsyncSpecAt(initialContext), "
        "ENABLED PostGstProductiveStepWith("
        "AsyncTerminatingLocalWorkDecreaseStep))"
    ),
    "protected-service-rank-stage4-ready-causal": (
        "\\A initialContext: "
        "Stage4RefinementFiniteServeEpisodeResidualProperty( "
        "AsyncSpecAt(initialContext)) "
        "=> ProtectedStage4RankProgressProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    "protected-service-rank": (
        "\\A initialContext: "
        "ProtectedServiceFiniteRunnerEpisodeClosureProperty( "
        "AsyncSpecAt(initialContext)) "
        "=> ProtectedServiceRanksProgressProperty( "
        "AsyncSpecAt(initialContext))"
    ),
    "rotating-leader-liveness": (
        "\\A initialContext: "
        "RotatingLeaderProgressProperty(AsyncLiveSpecAt(initialContext))"
    ),
    "locked-body-reproposal": (
        "\\A initialContext: "
        "LockedBodyReproposalProgressProperty(AsyncLiveSpecAt(initialContext))"
    ),
}

# These obligations are release-architecture seams, not declarations that may
# drift between proof modules.  Type closure belongs to the concrete async
# proof; successor activation, exact-recovery production refinement, genesis
# handoff, and
# multi-height progress belong to the current receipt-driven indexed chain
# product.
FIXED_PROOF_OBLIGATION_TARGETS = {
    "timeout-wire-authorization": (
        "SumeragiV2TimeoutWireAuthorization",
        "CoreSpecAtAlwaysStrongTimeoutWireAuthorizationInvariant / "
        "StrongWireInvariantAuthorizesPendingTimeoutSignature / "
        "StrongWireInvariantAuthorizesHonestTimeoutEnvelope",
    ),
    "same-round-lock-and-commit-authorization": (
        "SumeragiV2Proofs",
        "SameRoundLockAndCommitAuthorizationObligation",
    ),
    "effective-lock-body-acquisition-model": (
        "SumeragiV2EffectiveLockAcquisitionProofs",
        "EffectiveLockAcquisitionModelObligation",
    ),
    "effective-lock-body-acquisition-production-refinement": (
        "SumeragiV2AsyncLivenessProofs",
        "EffectiveLockBodyAcquisitionProductionRefinementObligation",
    ),
    "async-runner-scheduler-preservation": (
        "SumeragiV2AsyncLivenessProofs",
        "AsyncRunnerStepPreservesSchedulerType",
    ),
    "post-decision-timeout-exclusion": (
        "SumeragiV2AsyncLivenessProofs",
        "PostDecisionTimeoutExclusionObligation",
    ),
    "decision-recovery-across-restart": (
        "SumeragiV2AsyncLivenessProofs",
        "DecisionRecoveryAcrossRestartObligation",
    ),
    "progress-witness-production-refinement": (
        "SumeragiV2AsyncTemporalClosureProofs",
        "ProgressWitnessProductionRefinementObligation",
    ),
    "async-type-invariant": (
        "SumeragiV2AsyncLivenessProofs",
        "AsyncTypeInvariantObligation",
    ),
    "async-progress-ownership-invariant": (
        "SumeragiV2AsyncLivenessProofs",
        "AsyncSpecAlwaysProgressOwnershipInvariant",
    ),
    "protected-service-rank-stage4-ready-causal": (
        "SumeragiV2AsyncLivenessProofs",
        "ProtectedStage4RankProgressFromFairScheduler",
    ),
    "protected-service-rank-serve-fifo": (
        "SumeragiV2AsyncLivenessProofs",
        "ProtectedServeRankProgressFromFairFifo",
    ),
    "protected-service-rank-stage5-consensus-fifo": (
        "SumeragiV2AsyncLivenessProofs",
        "ProtectedStage5RankProgressFromFairFifo",
    ),
    "successor-activation-starvation-freedom": (
        "SumeragiV2SuccessorActivationRefinementProofs",
        "SuccessorActivationStarvationFreedomObligation",
    ),
    "successor-activation-exact-recovery-production-refinement": (
        "SumeragiV2ChainEpochRefinement",
        "SuccessorActivationAndExactHistoricalRecoveryProductionRefinementObligation",
    ),
    "genesis-height-successor-handoff": (
        "SumeragiV2ChainEpochRefinement",
        "GenesisHeightSuccessorHandoffObligation",
    ),
    "height-liveness": (
        "SumeragiV2ChainLivenessProofs",
        "HeightLivenessObligation",
    ),
}

# The historical temporal closure is source-bound to the executable indexed
# Async product.  Exact EXTENDS and INSTANCE substitutions prevent a new
# liveness import or a stale tuple projection from silently strengthening one
# of the reviewed historical LiveChainSpec residual boundaries.
HISTORICAL_TEMPORAL_REVIEWED_EXTENDS = {
    "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs": (
        "SumeragiV2AsyncHistoricalRecoveryTransportClosureProofs",
        "SumeragiV2AsyncHistoricalRecoveryClockTemporalProofs",
        "SumeragiV2AsyncHistoricalCandidateProducerContinuationProofs",
    ),
    "SumeragiV2AsyncHistoricalFiniteRunnerEpisodeProofs": (
        "SumeragiV2AsyncHistoricalRecoveryTemporalSupportProofs",
    ),
    "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs": (
        "SumeragiV2ChainEpochRefinement",
    ),
    "SumeragiV2HistoricalRecoveryTemporalClosureProofs": (
        "SumeragiV2SuccessorActivationRefinementProofs",
        "SumeragiV2AsyncHistoricalRecoveryClockTemporalProofs",
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
    ),
}

HISTORICAL_TEMPORAL_FORBIDDEN_DIRECT_IMPORTS = {
    "SumeragiV2AsyncHistoricalFiniteRunnerEpisodeProofs": (
        "SumeragiV2ChainEpochRefinement",
        "SumeragiV2ChainLivenessProofs",
        "SumeragiV2AsyncTemporalClosureProofs",
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
    ),
    "SumeragiV2HistoricalRecoveryTemporalClosureProofs": (
        "SumeragiV2ChainEpochRefinement",
        "SumeragiV2ChainLivenessProofs",
        "SumeragiV2AsyncHistoricalRecoveryLivenessProofs",
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
    ),
}

# The finite-runner and physical-provider cone must stay below indexed
# height/application liveness and may never obtain a local owner by assuming
# that every responsive peer has joined.  The inventory is intentionally
# theorem-scoped because the historical temporal closure module also contains
# later, reviewed height-composition theorems which legitimately mention the
# global join boundary.
HISTORICAL_FINITE_RUNNER_PROVIDER_THEOREMS = {
    "SumeragiV2AsyncHistoricalFiniteRunnerEpisodeProofs": (
        "HistoricalRunnerEpisodeRankOrderingIsWellFounded",
        "HistoricalRunnerEpisodeResidualFacts",
        "HistoricalRunnerEpisodeStepIsGoalDescentOrFrame",
        "HistoricalRunnerEpisodeSelectedOwnerIsConcreteAndEnabled",
        "HistoricalRunnerEpisodeSelectedActionConsumesCell",
        "HistoricalRunnerEpisodeOwnerPersistsInRankCell",
        "HistoricalRunnerEpisodeOwnerUsesAsyncFairness",
        "AsyncSpecProvidesHistoricalRunnerEpisodeRankStep",
        "AsyncSpecProvidesHistoricalRunnerEpisodeClosure",
        "AsyncSpecProvidesHistoricalFiniteRunnerEpisodeClosure",
        "AsyncSpecProvidesHistoricalProtectedServiceRankLeaves",
        "AsyncSpecProvidesHistoricalProtectedCandidateStarvation",
        "AsyncSpecProvidesHistoricalDiscoveryTimedCandidateStarvation",
        "HistoricalDiscoveryExactRunnerStepIsGoalNonDescentOrFrame",
        "AsyncSpecProvidesHistoricalDiscoveryCandidateExactRunnerStep",
        "HistoricalDiscoveryCausalDagFrontierHasProtectedWitness",
        "HistoricalDiscoveryCausalDagWitnessStepIsGoalDescentOrFrame",
        "AsyncSpecProvidesHistoricalDiscoveryCandidateCausalDagBudgetDescent",
        "AsyncSpecProvidesHistoricalDiscoveryCandidateExactRunnerService",
        "HistoricalDiscoveryServeExactWorkerStepIsModeGoalOrFrame",
        "HistoricalDiscoveryServeExactFairActionConsumesModeCell",
        "HistoricalDiscoveryServeExactWorkerUsesAsyncFairness",
        "AsyncSpecProvidesHistoricalDiscoveryServeExactWorkerStep",
    ),
    "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs": (
        "IndexedHistoricalRunnerEpisodeRankOrderingIsWellFounded",
        "IndexedHistoricalRunnerEpisodeResidualFacts",
        "IndexedHistoricalRunnerEpisodeStepIsGoalDescentOrFrame",
        "IndexedHistoricalRunnerEpisodeSelectedOwnerIsEnabled",
        "IndexedHistoricalRunnerEpisodeSelectedActionConsumesCell",
        "IndexedHistoricalRunnerEpisodeOwnerPersistsInRankCell",
        "IndexedHistoricalRunnerEpisodeOwnerUsesIndexedFairness",
        "IndexedChainSpecProvidesHistoricalRunnerEpisodeRankStep",
        "IndexedChainSpecProvidesHistoricalRunnerEpisodeClosure",
        "IndexedChainSpecProvidesHistoricalStage3FiniteRunnerEpisode",
        "IndexedChainSpecProvidesHistoricalStage4FiniteRunnerEpisode",
        "IndexedChainSpecProvidesHistoricalStage6FiniteRunnerEpisode",
        "IndexedChainSpecProvidesHistoricalFiniteRunnerEpisodeClosure",
        "IndexedHistoricalCommitLifecycleHasActivatedArchiveOwner",
        "IndexedChainSpecClosesHistoricalCommitArchiveActivation",
        "IndexedHistoricalCommitAdmissionReadyStepIsOutcomeOrFrame",
        "IndexedHistoricalCommitAdmissionReadyEnablesProductOccurrence",
        "IndexedHistoricalCommitAdmissionProductOccurrenceCreatesOutcome",
        "IndexedHistoricalCommitAdmissionReadyHasProductFairness",
        "IndexedChainSpecClosesHistoricalCommitAdmissionHandoff",
        "IndexedHistoricalDecisionLifecycleHasActivatedArchiveOwner",
        "IndexedChainSpecClosesHistoricalDecisionArchiveActivation",
        "IndexedHistoricalDecisionAdmissionReadyStepIsOutcomeOrFrame",
        "IndexedHistoricalDecisionAdmissionReadyEnablesProductOccurrence",
        "IndexedHistoricalDecisionAdmissionProductOccurrenceCreatesOutcome",
        "IndexedHistoricalDecisionAdmissionReadyHasProductFairness",
        "IndexedChainSpecClosesHistoricalDecisionAdmissionHandoff",
        "IndexedHistoricalDecisionIngressResidualsCloseKernel",
        "IndexedChainSpecProvidesCurrentVoterCausalEpisodeOwnerFairness",
        "IndexedChainSpecProvidesCurrentVoterRunnerEpisodeRankSteps",
        "IndexedChainSpecProvidesCurrentVoterFiniteRunnerEpisodeClosure",
        "IndexedChainSpecClosesCurrentVoterProtectedServiceRankProgress",
        "IndexedChainSpecClosesCurrentVoterProtectedCandidateStarvation",
        "IndexedChainSpecClosesHistoricalTimedCandidateStarvation",
        "IndexedChainSpecAndTimedCandidateStarvationProvideExactRunnerStep",
        "IndexedChainSpecAndTimedCandidateStarvationProvideCausalDagDescent",
        "IndexedChainSpecAndTimedCandidateStarvationProvideCausalDagResidual",
        "IndexedChainSpecProvidesHistoricalServeExactWorkerTemporalProperties",
        "IndexedHistoricalPacketConcreteActionStepIsGoalOrFrame",
        "IndexedHistoricalPacketConcreteProductOccurrenceReachesGoal",
        "IndexedHistoricalPacketPendingHasFairProductDomain",
        "IndexedHistoricalPacketPendingEnablesExactProductOccurrence",
        "IndexedChainSpecProvidesHistoricalPacketConcreteActionFairness",
        "IndexedChainSpecClosesHistoricalPacketConcreteActionService",
        "IndexedChainSpecClosesHistoricalFixedClockPacketRemainingResidual",
        "IndexedChainSpecAndRemainingPacketResidualProvideFixedClockPacketCorridor",
        "IndexedChainSpecClosesHistoricalFixedClockPacketCorridor",
        "IndexedChainSpecClosesHistoricalCommitEmissionEpisode",
        "IndexedChainSpecClosesHistoricalCommitEmissionResiduals",
        "IndexedChainSpecClosesHistoricalDecisionEmissionEpisode",
        "IndexedChainSpecClosesHistoricalDecisionEmissionResiduals",
        "IndexedChainSpecClosesHistoricalCommitIngressEpisode",
        "IndexedChainSpecClosesHistoricalCommitIngressResiduals",
        "IndexedChainSpecClosesHistoricalDecisionIngressEpisode",
        "IndexedChainSpecClosesHistoricalDecisionIngressResiduals",
        "IndexedChainSpecClosesHistoricalCommitResponseEpisode",
        "IndexedChainSpecClosesHistoricalCommitResponseResiduals",
        "IndexedChainSpecClosesHistoricalDecisionResponseEpisode",
        "IndexedChainSpecClosesHistoricalDecisionResponseResiduals",
        "IndexedChainSpecClosesHistoricalPhysicalTransportResiduals",
        "IndexedChainSpecClosesSixHistoricalPhysicalTransportKernels",
    ),
    "SumeragiV2HistoricalRecoveryTemporalClosureProofs": (
        "IndexedChainSpecClosesLocalExactDecisionOffSchedulerCorridor",
        "IndexedChainSpecProvidesLocalExactDecisionStageService",
        "IndexedHistoricalCertificateRankProgressResidualObligation",
        "IndexedHistoricalDecisionTargetCertifiedRequestResidualObligation",
    ),
}

HISTORICAL_FINITE_RUNNER_PROVIDER_REQUIRED_PROOF_TOKENS = {
    (
        "SumeragiV2AsyncHistoricalFiniteRunnerEpisodeProofs",
        "AsyncSpecProvidesHistoricalRunnerEpisodeRankStep",
    ): (
        "HistoricalRunnerEpisodeResidualFacts",
        "HistoricalRunnerEpisodeStepIsGoalDescentOrFrame",
        "HistoricalRunnerEpisodeSelectedOwnerIsConcreteAndEnabled",
        "HistoricalRunnerEpisodeSelectedActionConsumesCell",
        "HistoricalRunnerEpisodeOwnerPersistsInRankCell",
        "HistoricalRunnerEpisodeOwnerUsesAsyncFairness",
    ),
    (
        "SumeragiV2AsyncHistoricalFiniteRunnerEpisodeProofs",
        "AsyncSpecProvidesHistoricalRunnerEpisodeClosure",
    ): (
        "AsyncSpecProvidesHistoricalRunnerEpisodeRankStep",
        "HistoricalRunnerEpisodeRankOrderingIsWellFounded",
        "WellFoundedLeadsTo",
    ),
    (
        "SumeragiV2AsyncHistoricalFiniteRunnerEpisodeProofs",
        "AsyncSpecProvidesHistoricalFiniteRunnerEpisodeClosure",
    ): ("AsyncSpecProvidesHistoricalRunnerEpisodeClosure",),
    (
        "SumeragiV2AsyncHistoricalFiniteRunnerEpisodeProofs",
        "AsyncSpecProvidesHistoricalProtectedServiceRankLeaves",
    ): (
        "AsyncSpecProvidesHistoricalFiniteRunnerEpisodeClosure",
        "AsyncSpecClosesAllHistoricalTemporalCandidateStageLeaves",
        "HistoricalTemporalCandidateStageLeavesAreExact",
    ),
    (
        "SumeragiV2AsyncHistoricalFiniteRunnerEpisodeProofs",
        "AsyncSpecProvidesHistoricalProtectedCandidateStarvation",
    ): (
        "AsyncSpecProvidesHistoricalProtectedServiceRankLeaves",
        "HistoricalProtectedServiceRankProgressFromStageLeaves",
        "HistoricalProtectedServiceRankProgressImpliesStarvation",
    ),
    (
        "SumeragiV2AsyncHistoricalFiniteRunnerEpisodeProofs",
        "AsyncSpecProvidesHistoricalDiscoveryTimedCandidateStarvation",
    ): (
        "StarvationFreedomObligation",
        "AsyncSpecProvidesHistoricalProtectedCandidateStarvation",
        "HistoricalDiscoveryTimedOwnerModeCannotIncreaseAfterGst",
    ),
    (
        "SumeragiV2AsyncHistoricalFiniteRunnerEpisodeProofs",
        "AsyncSpecProvidesHistoricalDiscoveryCandidateExactRunnerService",
    ): (
        "AsyncSpecProvidesHistoricalDiscoveryCandidateExactRunnerStep",
        "AsyncSpecProvidesHistoricalDiscoveryCandidateCausalDagBudgetDescent",
    ),
    (
        "SumeragiV2AsyncHistoricalFiniteRunnerEpisodeProofs",
        "AsyncSpecProvidesHistoricalDiscoveryServeExactWorkerStep",
    ): (
        "HistoricalDiscoveryServeExactWorkerStepIsModeGoalOrFrame",
        "HistoricalDiscoveryServeExactFairActionConsumesModeCell",
        "HistoricalDiscoveryServeExactWorkerUsesAsyncFairness",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecProvidesHistoricalRunnerEpisodeRankStep",
    ): (
        "IndexedHistoricalRunnerEpisodeResidualFacts",
        "IndexedHistoricalRunnerEpisodeStepIsGoalDescentOrFrame",
        "IndexedHistoricalRunnerEpisodeSelectedOwnerIsEnabled",
        "IndexedHistoricalRunnerEpisodeSelectedActionConsumesCell",
        "IndexedHistoricalRunnerEpisodeOwnerPersistsInRankCell",
        "IndexedHistoricalRunnerEpisodeOwnerUsesIndexedFairness",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecProvidesHistoricalRunnerEpisodeClosure",
    ): (
        "IndexedChainSpecProvidesHistoricalRunnerEpisodeRankStep",
        "IndexedHistoricalRunnerEpisodeRankOrderingIsWellFounded",
        "WellFoundedLeadsTo",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecProvidesHistoricalFiniteRunnerEpisodeClosure",
    ): (
        "IndexedChainSpecProvidesHistoricalStage3FiniteRunnerEpisode",
        "IndexedChainSpecProvidesHistoricalStage4FiniteRunnerEpisode",
        "IndexedChainSpecProvidesHistoricalStage6FiniteRunnerEpisode",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecProvidesCurrentVoterRunnerEpisodeRankSteps",
    ): (
        "IndexedChainSpecProvidesCurrentVoterCausalEpisodeOwnerFairness",
        "AsyncReadyRunnerEpisodeStepIsGoalDescentOrFrame",
        "AsyncCapacityRunnerEpisodeStepIsGoalDescentOrFrame",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecProvidesCurrentVoterFiniteRunnerEpisodeClosure",
    ): (
        "IndexedChainSpecProvidesCurrentVoterRunnerEpisodeRankSteps",
        "AsyncReadyRunnerEpisodeRankOrderingIsWellFounded",
        "AsyncCapacityRunnerEpisodeRankOrderingIsWellFounded",
        "WellFoundedLeadsTo",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesCurrentVoterProtectedServiceRankProgress",
    ): (
        "IndexedChainSpecProvidesCurrentVoterFiniteRunnerEpisodeClosure",
        "Stage2BusyKernelNextObligation",
        "ProtectedRankExitHasWellFoundedSuccessor",
        "OwnedServiceRankOrderingWellFounded",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesCurrentVoterProtectedCandidateStarvation",
    ): (
        "IndexedChainSpecClosesCurrentVoterProtectedServiceRankProgress",
        "ProtectedRankProgressSuppliesWellFoundedStep",
        "OwnedServiceRankOrderingWellFounded",
        "WellFoundedLeadsTo",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesHistoricalTimedCandidateStarvation",
    ): (
        "IndexedChainSpecClosesCurrentVoterProtectedCandidateStarvation",
        "IndexedChainSpecClosesHistoricalProtectedCandidateStarvation",
        "HistoricalDiscoveryTimedOwnerModeCannotIncreaseAfterGst",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesHistoricalPacketConcreteActionService",
    ): (
        "IndexedHistoricalPacketConcreteActionStepIsGoalOrFrame",
        "IndexedHistoricalPacketConcreteProductOccurrenceReachesGoal",
        "IndexedHistoricalPacketPendingEnablesExactProductOccurrence",
        "IndexedChainSpecProvidesHistoricalPacketConcreteActionFairness",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesHistoricalFixedClockPacketRemainingResidual",
    ): (
        "IndexedChainSpecClosesHistoricalPacketConcreteActionService",
        "IndexedChainSpecClosesHistoricalTimedCandidateStarvation",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecAndRemainingPacketResidualProvideFixedClockPacketCorridor",
    ): (
        "IndexedChainSpecAndTimedCandidateStarvationProvideCausalDagResidual",
        "IndexedChainSpecProvidesHistoricalServeExactWorkerTemporalProperties",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesHistoricalFixedClockPacketCorridor",
    ): (
        "IndexedChainSpecClosesHistoricalFixedClockPacketRemainingResidual",
        "IndexedChainSpecAndRemainingPacketResidualProvideFixedClockPacketCorridor",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesHistoricalCommitEmissionResiduals",
    ): ("IndexedChainSpecClosesHistoricalCommitEmissionEpisode",),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesHistoricalDecisionEmissionResiduals",
    ): ("IndexedChainSpecClosesHistoricalDecisionEmissionEpisode",),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesHistoricalCommitIngressResiduals",
    ): (
        "IndexedChainSpecClosesHistoricalCommitIngressEpisode",
        "IndexedHistoricalCommitLifecycleRankStepClosesLifecycle",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesHistoricalDecisionIngressResiduals",
    ): (
        "IndexedChainSpecClosesHistoricalDecisionIngressEpisode",
        "IndexedHistoricalDecisionLifecycleRankStepClosesLifecycle",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesHistoricalCommitResponseResiduals",
    ): ("IndexedChainSpecClosesHistoricalCommitResponseEpisode",),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesHistoricalDecisionResponseResiduals",
    ): ("IndexedChainSpecClosesHistoricalDecisionResponseEpisode",),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesHistoricalPhysicalTransportResiduals",
    ): (
        "IndexedChainSpecClosesHistoricalCommitEmissionResiduals",
        "IndexedChainSpecClosesHistoricalCommitIngressResiduals",
        "IndexedChainSpecClosesHistoricalCommitResponseResiduals",
        "IndexedChainSpecClosesHistoricalDecisionEmissionResiduals",
        "IndexedChainSpecClosesHistoricalDecisionIngressResiduals",
        "IndexedChainSpecClosesHistoricalDecisionResponseResiduals",
    ),
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedChainSpecClosesSixHistoricalPhysicalTransportKernels",
    ): (
        "IndexedChainSpecClosesHistoricalPhysicalTransportResiduals",
        "IndexedHistoricalPhysicalResidualsProvideSixKernels",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecClosesLocalExactDecisionOffSchedulerCorridor",
    ): (
        "ExactDecisionRequestClockOwnerConvergence",
        "ExactDecisionRequestRuntimePrefixConvergence",
        "ExactDecisionRequestHeadGateOwnerConvergence",
        "ExactDecisionRequestAdmissionCoalescingOutcomeIsDischarged",
        "ExactDecisionResponseNonPhysicalNonClaimHeadGateOwnerConvergence",
        "IndexedPostGstRunNodeFairnessTransfersLocally",
        "IndexedFairActionsRemainEnabledInProduct",
        "IndexedFairProductStepsProjectExactOccurrences",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedChainSpecProvidesLocalExactDecisionStageService",
    ): (
        "IndexedChainSpecProvidesCurrentVoterFiniteRunnerEpisodeClosure",
        "IndexedChainSpecClosesLocalExactDecisionOffSchedulerCorridor",
        "ExactDecisionOffSchedulerResidualConvergenceDischargesStageService",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalCertificateRankProgressResidualObligation",
    ): (
        "IndexedLiveChainSpecProjectsIndexedChainSpec",
        "IndexedChainSpecClosesHistoricalFixedClockPacketRemainingResidual",
        "IndexedChainSpecClosesSixHistoricalPhysicalTransportKernels",
        "IndexedHistoricalCertificatePhysicalKernelsCloseRankResidual",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedHistoricalDecisionTargetCertifiedRequestResidualObligation",
    ): (
        "IndexedLiveChainSpecProjectsIndexedChainSpec",
        "IndexedChainSpecClosesSixHistoricalPhysicalTransportKernels",
        "IndexedHistoricalDecisionTransportKernelsCloseExactLeaf",
        "IndexedHistoricalDecisionTransportLeafClosesTargetRankFive",
    ),
}

HISTORICAL_FINITE_RUNNER_PROVIDER_FORBIDDEN_DEPENDENCY_PATTERNS = (
    r"[A-Za-z0-9_]*HeightLiveness[A-Za-z0-9_]*",
    r"[A-Za-z0-9_]*Application(?:Completion)?(?:Progress|Liveness)"
    r"[A-Za-z0-9_]*",
    r"[A-Za-z0-9_]*AllResponsiveJoined[A-Za-z0-9_]*",
)

HISTORICAL_FINITE_RUNNER_PROVIDER_FORBIDDEN_DEPENDENCIES = (
    "AsyncAllResponsiveAppliedAt",
    "IndexedAllResponsiveExactApplicationsAt",
    "VerificationOneHeightCompletion",
    "VerificationOneHeightCompletionObligation",
    "AsyncTemporalClosureOneHeightCompletionObligation",
    "OneHeightCompletionLiveness",
    "IndexedHistoricalOneHeightCompletionAt",
    "IndexedOneHeightTemporalClosureIsExact",
)

HISTORICAL_INDEXED_CORE_FIELDS = (
    "height",
    "context",
    "contextHistory",
    "nodeView",
    "generation",
    "up",
    "gst",
    "availableBodies",
    "durableBodies",
    "retainedLockedBodies",
    "validatedBodies",
    "invalidBodies",
    "seenProposals",
    "receivedVotes",
    "receivedQCs",
    "receivedTimeoutVotes",
    "receivedTCs",
    "proposalIntents",
    "prepareIntents",
    "commitIntents",
    "timeoutIntents",
    "prepareQCs",
    "commitQCs",
    "formedTCs",
    "installedTCs",
    "lastInstalledTc",
    "lockPrepareQc",
    "highestPrepareQc",
    "lockRank",
    "lockSubject",
    "highestRank",
    "highestSubject",
    "pendingProposal",
    "pendingPrepare",
    "pendingObservePrepare",
    "pendingLockCommit",
    "pendingTimeout",
    "pendingInstallTC",
    "pendingDecision",
    "signProposals",
    "signVotes",
    "signTimeouts",
    "proposalNetwork",
    "voteNetwork",
    "qcNetwork",
    "timeoutNetwork",
    "tcNetwork",
    "decisions",
    "applied",
)

HISTORICAL_INDEXED_SCHEDULER_FIELDS = (
    "asyncNow",
    "asyncCommandQueues",
    "asyncNextCommandClass",
    "asyncFifoOwed",
    "asyncTimeoutEmitted",
    "asyncRunnerPhase",
    "asyncRunnerBudget",
    "asyncCausalAdmissionOwed",
    "asyncNextLocalSource",
    "asyncIoQueues",
    "asyncNextServeAdmissionOrdinal",
    "asyncNextServeIngressOrdinal",
    "asyncServeIngressAdmissions",
    "asyncServeAdmissions",
    "asyncServeReservations",
    "asyncServeTombstones",
    "asyncServeAttempts",
    "asyncOutstandingWork",
    "asyncIoReadyCompletions",
    "asyncLocalReadyCompletions",
    "asyncNextCompletionSource",
    "asyncIoControlAvailable",
    "asyncDeferredCompletionQueues",
    "asyncDeferredProgressQueues",
    "asyncDeferredNormalQueues",
    "asyncDeferredHandoffs",
    "asyncNextDeferredClass",
    "asyncDeferredDrainOwed",
    "asyncCausalQueues",
    "asyncOutstandingTags",
    "asyncNodeDeadlines",
    "asyncRetransmitDeadlines",
    "asyncNodeServiceDeadlines",
    "asyncIoServiceDeadlines",
    "asyncSentItems",
    "asyncRetainedControl",
    "asyncActiveRequests",
    "asyncCertifiedResponseClaim",
    "asyncTransport",
    "asyncIngressLanes",
    "asyncIngressReady",
    "asyncLeaderWireLifecycles",
    "asyncHeldChunks",
    "asyncHistoricalRecoveryTargets",
    "asyncControlServiceState",
    "asyncServiceActivationState",
)

HISTORICAL_INDEXED_RECOVERY_FIELDS = (
    "asyncRecoveryPhase",
    "asyncRecoveryNode",
    "asyncRecoveryGeneration",
    "asyncRecoveryReplayQueue",
    "asyncHistoricalLockRestartAuthorities",
)

HISTORICAL_INDEXED_PRODUCER_FIELDS = (
    "asyncProducerKnownObligations",
    "asyncProducerConsumedEpisodes",
    "asyncProducerOriginHistory",
)

HISTORICAL_INDEXED_INSTANCE_CONTRACTS = (
    (
        "SumeragiV2IndexedHistoricalRecoveryTransportClosureProofs",
        "IndexedHistoricalTransport",
        "SumeragiV2AsyncHistoricalFiniteRunnerEpisodeProofs",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedDecisionWitness",
        "SumeragiV2ProgressWitnessFinalClosureProofs",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedDecisionServiceWitness",
        "SumeragiV2ExactDecisionStageServiceClosureProofs",
    ),
    (
        "SumeragiV2HistoricalRecoveryTemporalClosureProofs",
        "IndexedAdequateLeaderWitness",
        "SumeragiV2AdequateLeaderAuthorityDeadlineServiceProofs",
    ),
)
