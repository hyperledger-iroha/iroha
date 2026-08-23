use super::*;

#[test]
fn claimed_qpcs_source_carrier_pins_exact_retention_and_local_work() {
    let ledger = RNS_NATIVE_CLAIMED_QPCS_SOURCE_CARRIER_LOCAL_RESOURCE_LEDGER_V2;
    assert_eq!(ledger.relations, 200);
    assert_eq!(ledger.retained_public_evaluation_bytes, 140_800);
    assert_eq!(ledger.retained_numeric_tail_bytes, 4_800);
    assert_eq!(ledger.retained_numeric_cache_bytes, 145_600);
    assert_eq!(ledger.retained_terminal_chronology_bytes, 5_384);
    assert_eq!(ledger.retained_commitment_digest_bytes, 1_344);
    assert_eq!(ledger.retained_payload_bytes, 152_328);
    assert_eq!(ledger.canonical_checks, 18_200);
    assert_eq!(ledger.ring_power_squarings, 3_400);
    assert_eq!(ledger.modular_multiplications, 3_600);
    assert_eq!(ledger.modular_additions, 200);
    assert_eq!(ledger.numeric_tail_retention_work_units, 4_800);
    assert_eq!(ledger.pre_binding_local_work_units, 28_144);
    assert_eq!(ledger.claimed_source_binding_hash_bytes, 5_284);
    assert_eq!(ledger.carrier_binding_hash_bytes, 1_678);
    assert_eq!(ledger.combined_binding_hash_bytes, 6_962);
    assert_eq!(ledger.local_work_units, 35_106);
    // Historical compatibility floor, not the pre-binding subtotal.
    assert!(ledger.local_work_units >= 28_808);
    assert_eq!(ledger.new_heap_bytes, 0);
    assert_eq!(ledger.new_spool_bytes, 0);
    assert_eq!(ledger.new_wire_bytes, 0);
    assert_eq!(ledger.new_authenticated_io_bytes, 0);
}

#[test]
fn claimed_qpcs_source_carrier_keeps_every_downstream_gate_closed() {
    const {
        assert!(RNS_NATIVE_CLAIMED_QPCS_SOURCE_CARRIER_SOURCE_SETTLED_V2);
        assert!(RNS_NATIVE_CLAIMED_QPCS_SOURCE_CARRIER_CONTRACT_IMPLEMENTED_V2);
        assert!(RNS_NATIVE_CLAIMED_QPCS_SOURCE_PREFLIGHT_ORDER_IMPLEMENTED_V2);
        assert!(RNS_NATIVE_CLAIMED_QPCS_NUMERIC_TAIL_IMPLEMENTED_V2);
        assert!(RNS_NATIVE_CLAIMED_QPCS_CARRIER_LEDGER_IS_ADDITIVE_V2);
        assert!(!RNS_NATIVE_CLAIMED_QPCS_SOURCE_CARRIER_LIVE_INTEGRATED_V2);
        assert!(!RNS_NATIVE_CLAIMED_QPCS_PRE_QPCS_Q_MASK_INTEGRATED_V2);
        assert!(!RNS_NATIVE_CLAIMED_QPCS_PRE_DIRECT_AXES_INTEGRATED_V2);
        assert!(!RNS_NATIVE_CLAIMED_QPCS_DIRECT_RELATION_INTEGRATED_V2);
        assert!(!RNS_NATIVE_CLAIMED_QPCS_INVENTORY_MEMBERSHIP_INTEGRATED_V2);
        assert!(!RNS_NATIVE_CLAIMED_QPCS_GLOBAL_ROOT_DISCHARGED_V2);
        assert!(!RNS_NATIVE_CLAIMED_QPCS_ZERO_ROOT_DISCHARGED_V2);
        assert!(!RNS_NATIVE_CLAIMED_QPCS_DIRECT_OPENINGS_AVAILABLE_V2);
        assert!(!RNS_NATIVE_CLAIMED_QPCS_RESOURCE_EVIDENCE_QUALIFIED_V2);
        assert!(!RNS_NATIVE_CLAIMED_QPCS_READINESS_V2);
        assert!(!RNS_NATIVE_CLAIMED_QPCS_RELEASE_AUTHORIZED_V2);
    }
}

#[test]
fn local_ledger_explicitly_excludes_existing_subtransition_costs() {
    let source = include_str!("claimed_qpcs_source_carrier_v2.rs");
    let ledger_docs = source
        .split_once("/// Exact additive accounting for the carrier-local work")
        .expect("additive ledger scope")
        .1
        .split_once("pub(super) struct RnsNativeClaimedQpcsSourceCarrierLocalResourceLedgerV2")
        .expect("ledger declaration boundary")
        .0;
    assert!(ledger_docs.contains("not end-to-end accounting"));
    assert!(ledger_docs.contains("3,520-object/200-evaluation public read"));
    assert!(ledger_docs.contains("complete qPCS/FRI work"));
    assert!(ledger_docs.contains("both confidential-source passes"));
    assert!(ledger_docs.contains("later numeric-cursor destination"));
    assert!(ledger_docs.contains("zero additive carrier-local"));
    assert!(ledger_docs.contains("4,800-byte"));
    assert!(ledger_docs.contains("retained inline"));
    assert!(ledger_docs.contains("28,144 before"));
    assert!(ledger_docs.contains("6,962 binding bytes"));
    assert!(ledger_docs.contains("35,106 total"));
    assert!(ledger_docs.contains("per canonical check, modular operation"));
    assert!(ledger_docs.contains("not an instruction count"));
    assert!(ledger_docs.contains("excludes control-flow comparisons"));
}

#[test]
fn consuming_transition_preserves_the_audited_one_shot_order() {
    let source = include_str!("claimed_qpcs_source_carrier_v2.rs");
    let transition = source
        .split_once("pub(super) fn authenticate_claimed_qpcs_source_carrier_v2")
        .expect("sealed consuming transition")
        .1
        .split_once("fn map_public_read_error_v2")
        .expect("transition boundary")
        .0;
    let begin = transition
        .find("RnsNativeSingleQpcsScheduleBatchV2::begin_v2")
        .unwrap();
    let read = transition.find("take_next_evaluation_v2").unwrap();
    let finish = transition.find("batch.finish_v2").unwrap();
    let prepare = transition
        .find("prepare_rns_native_qpcs_pre_auth_claimed_v1")
        .unwrap();
    let authenticate = transition
        .find("authenticate_rns_native_qpcs_pre_auth_claimed_v1")
        .unwrap();
    let preflight = transition
        .find("preflight_rns_native_qpcs_authenticated_claimed_source_v1")
        .unwrap();
    let materialize = transition
        .find("materialize_numeric_and_take_schedule_v1")
        .unwrap();
    assert!(begin < read);
    assert!(read < finish);
    assert!(finish < prepare);
    assert!(prepare < authenticate);
    assert!(authenticate < preflight);
    assert!(preflight < materialize);
    assert_eq!(transition.matches("take_next_evaluation_v2").count(), 1);
    assert_eq!(transition.matches("batch.finish_v2").count(), 1);
    assert_eq!(
        transition
            .matches("materialize_numeric_and_take_schedule_v1")
            .count(),
        1
    );
}

#[test]
fn source_preflight_precedes_numeric_materialization_and_schedule_take() {
    let qpcs_source = include_str!("../../../rns_native_qpcs_fri_complete.rs");
    let preflight = qpcs_source
        .split_once("pub(super) fn preflight_rns_native_qpcs_authenticated_claimed_source_v1")
        .expect("claimed source preflight")
        .1
        .split_once("fn map_claimed_source_preflight_error_v1")
        .expect("preflight boundary")
        .0;
    let schedule_before = preflight
        .find("if !qpcs.has_relation_schedule_v1()")
        .unwrap();
    let source_call = preflight
        .find("preflight_rns_native_rlwe_source_statement_v1")
        .unwrap();
    let schedule_after = preflight
        .find("if !source.qpcs().has_relation_schedule_v1()")
        .unwrap();
    assert!(schedule_before < source_call);
    assert!(source_call < schedule_after);

    let materialize = qpcs_source
        .split_once("pub(super) fn materialize_numeric_and_take_schedule_v1")
        .expect("numeric materialization")
        .1
        .split_once("fn claimed_source_numeric_binding_digest_v1")
        .expect("materialization boundary")
        .0;
    let validate = materialize
        .find("validate_claimed_source_numeric_tail_v1")
        .unwrap();
    let take = materialize.find("take_qpcs_relation_schedule_v1").unwrap();
    let scheduleless_check = materialize
        .find("if self.source.qpcs().has_relation_schedule_v1()")
        .unwrap();
    assert!(validate < take);
    assert!(take < scheduleless_check);
    assert_eq!(
        materialize
            .matches("take_qpcs_relation_schedule_v1")
            .count(),
        1
    );
}

#[test]
fn retained_owners_are_move_only_opaque_and_do_not_downgrade_chronology() {
    let carrier_source = include_str!("claimed_qpcs_source_carrier_v2.rs");
    let qpcs_source = include_str!("../../../rns_native_qpcs_fri_complete.rs");

    let numeric_tail = qpcs_source
        .split_once("pub(super) struct RnsNativeQpcsAuthenticatedNumericTailV1")
        .expect("authenticated numeric tail")
        .1
        .split_once("impl RnsNativeQpcsAuthenticatedNumericTailV1")
        .expect("numeric tail boundary")
        .0;
    assert!(numeric_tail.contains("a: u64"));
    assert!(numeric_tail.contains("product: u64"));
    assert!(numeric_tail.contains("opening_quotient: u64"));
    assert!(!numeric_tail.contains("point: u64"));

    let retained = carrier_source
        .split_once("struct RnsNativeClaimedQpcsRetainedPublicationV2")
        .expect("retained publication")
        .1
        .split_once("pub(in crate::vega::zk_ams::mkhe) struct RnsNativeClaimedQpcsOwnedStageV2")
        .expect("retained publication boundary")
        .0;
    assert!(retained.contains("owners: RnsNativeWholePublicationOwnersV2"));
    assert!(retained.contains("read_receipt: RnsNativePublicPolynomialReadReceiptV1"));
    assert!(retained.contains("facts: RnsNativePreTranscriptPublicStatementFactsV2"));
    assert!(retained.contains("equation_commitment_digests:"));
    assert!(retained.contains("limb_commitment_digests:"));
    assert!(retained.contains("carrier_binding_digest:"));

    let exact_stage = carrier_source
        .split_once("pub(in crate::vega::zk_ams::mkhe) struct RnsNativeClaimedQpcsOwnedStageV2")
        .expect("exact stage")
        .1
        .split_once("struct RnsNativeClaimedQpcsSourceStageV2")
        .expect("exact-stage boundary")
        .0;
    assert!(exact_stage.contains("retained: RnsNativeClaimedQpcsRetainedPublicationV2"));
    assert!(exact_stage.contains("stage: Stage"));
    assert!(!exact_stage.contains("pub retained:"));
    assert!(!exact_stage.contains("pub stage:"));

    let carrier = carrier_source
        .split_once("pub(super) struct RnsNativeClaimedQpcsSourceCarrierV2")
        .expect("top carrier")
        .1
        .split_once("impl<'qpcs, S:")
        .expect("top carrier boundary")
        .0;
    assert!(carrier.contains("owned: RnsNativeClaimedQpcsOwnedStageV2"));
    assert!(!carrier.contains("relation_schedule:"));
    assert!(!carrier.contains("terminal_chronology:"));
    assert!(!carrier.contains("derive(Clone"));
    assert!(!carrier.contains("derive(Copy"));

    let scheduleless = qpcs_source
        .split_once("pub(super) struct RnsNativeQpcsSchedulelessClaimedSourceV1")
        .expect("opaque scheduleless source")
        .1
        .split_once("impl<S: ZkAmsMkheRnsNativeSourceSnapshotV1>")
        .expect("scheduleless source boundary")
        .0;
    assert_eq!(scheduleless.matches("relation_schedule:").count(), 1);
    assert_eq!(scheduleless.matches("terminal_chronology:").count(), 1);
    assert_eq!(scheduleless.matches("numeric_tails:").count(), 1);
    assert!(!scheduleless.contains("RnsNativeQpcsCompletedLineageV1"));
    assert!(!scheduleless.contains("ZkAmsMkheRnsNativeChallengeSeedsV1"));
    assert!(!scheduleless.contains("ZkAmsMkheRnsNativeQpcsBoundTranscriptV1"));

    let input = carrier_source
        .split_once("pub(super) struct RnsNativeClaimedQpcsAuthenticationInputV2")
        .expect("fixed authentication input")
        .1
        .split_once("impl<'digests, 'proof> RnsNativeClaimedQpcsAuthenticationInputV2")
        .expect("authentication input boundary")
        .0;
    assert!(!input.contains("transcript:"));
    assert!(!carrier_source.contains("RnsNativeCrossFieldRlweClaimedRelationV1"));
    assert!(!carrier_source.contains("fn into_parts"));
    assert!(!carrier_source.contains("fn relation_schedule_v2"));
    assert!(!carrier_source.contains("fn terminal_chronology_v2"));
    assert!(!carrier_source.contains("fn roots_v2"));
}

#[test]
fn numeric_authority_moves_once_and_only_the_parent_sidecar_implements_the_cursor() {
    let carrier = include_str!("claimed_qpcs_source_carrier_v2.rs");
    let direct = include_str!("../../../rns_native_cross_field_rlwe_direct.rs");

    assert!(!carrier.contains("RnsNativeCrossFieldNumericCursorV1\n    for RnsNativeClaimed"));
    let origin = carrier
        .split_once("pub(super) fn into_claimed_successor_stage_v2")
        .expect("sole origin transition")
        .1
        .split_once("impl<K, P, S>")
        .expect("origin boundary")
        .0;
    let mint = origin
        .find("RnsNativeClaimedDirectNumericOriginV2::mint_v2(")
        .unwrap();
    let move_into_parent = origin.find(".into_claimed_successor_stage_v2(").unwrap();
    assert!(mint < move_into_parent);

    let token_mint = carrier
        .split_once("fn mint_v2(")
        .expect("private origin mint")
        .1
        .split_once("pub(in crate::vega::zk_ams::mkhe) fn is_fresh_v2")
        .expect("private origin mint boundary")
        .0;
    let fresh = token_mint.find("cursor.next_relation != 0").unwrap();
    let exact_array = token_mint.find(".try_into()").unwrap();
    let retained_binding = token_mint
        .find("RnsNativeClaimedQpcsRetainedPublicationOriginBindingV2")
        .unwrap();
    assert!(fresh < exact_array && exact_array < retained_binding);

    let token_cursor = carrier
        .split_once("fn take_public_evaluation_v2(")
        .expect("numeric origin cursor")
        .1
        .split_once("fn is_complete_v2")
        .expect("numeric origin cursor boundary")
        .0;
    let poison = token_cursor.find("self.poisoned = true").unwrap();
    let order = token_cursor
        .find("limb != relation / REPETITIONS_V2")
        .unwrap();
    let public = token_cursor.find("self.public_evaluations").unwrap();
    let commit = token_cursor.find("self.poisoned = false").unwrap();
    assert!(poison < order && order < public && public < commit);

    let sidecar = direct
        .split_once(
            "impl RnsNativeCrossFieldNumericCursorV1 for RnsNativeCrossFieldRlweNumericSidecarV2",
        )
        .expect("numeric sidecar cursor")
        .1
        .split_once("/// Authenticated public-point source")
        .expect("numeric sidecar boundary")
        .0;
    let clear = sidecar
        .find("*destination = RnsNativeCrossFieldNumericEvaluationV1::default()")
        .unwrap();
    let consume_origin = sidecar
        .find("self.origin.take_public_evaluation_v2(limb, repetition)")
        .unwrap();
    assert!(clear < consume_origin);
    assert!(direct.contains("if !numeric_sidecar.is_complete_v2()"));
    let direct_origin = direct
        .split_once("pub(super) fn into_claimed_successor_stage_v2(")
        .expect("direct origin consumer")
        .1
        .split_once("/// Opaque core")
        .expect("direct origin consumer boundary")
        .0;
    assert!(direct_origin.contains("numeric_origin: RnsNativeClaimedDirectNumericOriginV2"));
    assert!(!direct_origin.contains("Box<[RnsNativePublicPolynomialEvaluationV1"));
    assert!(!direct_origin.contains("next_relation:"));
    assert!(!direct_origin.contains("poisoned:"));
}

#[test]
fn exact_stage_wrapper_has_only_purpose_specific_forward_transitions() {
    let source = include_str!("claimed_qpcs_source_carrier_v2.rs");
    for transition in [
        "verify_comparator_product_v2",
        "verify_comparator_range_carry_v2",
        "verify_small_sign_disjointness_v2",
        "verify_q_mask_linear_relations_v2",
        "authenticate_existing_radix_v2",
        "verify_radix_complement_v2",
        "verify_centering_subtraction_v2",
        "derive_global_lookup_pre_z_v2",
        "authenticate_global_lookup_post_z_v2",
        "verify_global_inverse_product_v2",
        "verify_global_membership_v2",
        "verify_direct_global_membership_handoff_v2",
    ] {
        assert!(source.contains(transition), "missing {transition}");
    }
    assert!(!source.contains("fn map_stage"));
    assert!(!source.contains("fn into_parts"));
    assert!(!source.contains("fn retained_publication"));
}
