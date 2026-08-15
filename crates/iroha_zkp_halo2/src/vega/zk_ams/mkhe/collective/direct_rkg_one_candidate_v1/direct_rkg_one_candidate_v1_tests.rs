use super::super::tests::{Inject, Rng, begin, drops};
use super::*;
use crate::vega::zk_ams::mkhe::{
    active::ZkAmsMkheGovernedActiveRosterV1,
    active_exact_binding::{
        DirectRelationPublicObjectsV1, SealedDirectRkgOneProofOwnerV1,
        VerifiedPersistentWitnessBindingSetV1,
    },
    direct_collective_eval_ceremony::ZkAmsMkheDirectEvaluatedKeyTargetV1,
    direct_object_transport::ZkAmsMkheDirectObjectReadAtProviderV1,
    direct_rkg_ephemeral_membership::tests::creator_state_fixture,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn bounded_source_section<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
    assert_eq!(source.matches(start).count(), 1, "non-unique start anchor");
    source
        .split_once(start)
        .and_then(|(_, tail)| tail.split_once(end).map(|(section, _)| section))
        .expect("bounded source section")
}

fn struct_body<'a>(source: &'a str, marker: &str) -> &'a str {
    source
        .split_once(marker)
        .map(|(_, tail)| tail)
        .expect("owned structure")
        .split_once("\n}")
        .map(|(body, _)| body)
        .expect("owned structure boundary")
}

fn assert_ordered(section: &str, snippets: &[&str]) {
    let mut previous = 0;
    for snippet in snippets {
        let offset = section[previous..]
            .find(snippet)
            .map(|offset| previous + offset)
            .unwrap_or_else(|| panic!("missing ordered snippet: {snippet}"));
        previous = offset + snippet.len();
    }
}

#[expect(dead_code, reason = "compile-only precise-capture check")]
fn semantic_owner_does_not_borrow_provider_or_objects<'a, P>(
    owner: SealedDirectRkgOneProofOwnerV1<'a>,
    context: ZkAmsMkheDirectCeremonyContextV1,
    objects: DirectRelationPublicObjectsV1,
    provider: &mut P,
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    let semantic = owner.verify_semantic_candidate_v1(context, objects, provider)?;
    let (_reused_objects, _reused_provider) = (objects, &mut *provider);
    drop(semantic);
    Ok(())
}

struct ReadyFixture {
    roster: ZkAmsMkheGovernedActiveRosterV1,
    bindings: VerifiedPersistentWitnessBindingSetV1,
    state: super::super::super::ZkAmsMkheCollectivePartyStateV1,
    wrapper: StateOwnedDirectRkgEphemeralMembershipPrecursorV1,
    context: ZkAmsMkheDirectCeremonyContextV1,
}

impl ReadyFixture {
    fn new(label: &[u8]) -> Self {
        let (roster, bindings, mut state) = creator_state_fixture(label);
        begin(Inject::Good);
        let wrapper = state
            .prepare_state_owned_direct_rkg_ephemeral_membership_v1(
                &roster,
                &bindings,
                0,
                &mut Rng::new(0xaa),
            )
            .expect("accepted creator precursor");
        let context = ZkAmsMkheDirectCeremonyContextV1::from_verified_binding_set(
            &roster,
            &bindings,
            ZkAmsMkheDirectEvaluatedKeyTargetV1::Relinearization,
            0,
        )
        .expect("accepted RKG1 context");
        Self {
            roster,
            bindings,
            state,
            wrapper,
            context,
        }
    }
}

#[test]
fn post_take_error_burns_ephemeral_owner_and_keeps_creation_bit() {
    let ReadyFixture {
        roster,
        bindings,
        mut state,
        wrapper,
        context,
    } = ReadyFixture::new(b"rkg1-post-take-error");
    let creation_mask = state.party_local_rkg_ephemeral_creation_mask;
    begin(Inject::None);
    let result = take_ready_direct_rkg_one_prover_session_v1(
        &mut state,
        wrapper,
        &roster,
        &bindings,
        context,
        &mut Rng::fail(0xaa, 0),
    );
    assert!(matches!(result, Err(ZkAmsMkheErrorV1::RandomUnavailable)));
    assert!(state.party_local_rkg_ephemeral_opening.is_none());
    assert_eq!(state.party_local_rkg_ephemeral_creation_mask, creation_mask);
    assert_eq!(drops(), [1, 0, 1, 1]);
}

#[test]
fn post_take_unwind_burns_ephemeral_owner_and_keeps_creation_bit() {
    let ReadyFixture {
        roster,
        bindings,
        mut state,
        wrapper,
        context,
    } = ReadyFixture::new(b"rkg1-post-take-unwind");
    let creation_mask = state.party_local_rkg_ephemeral_creation_mask;
    begin(Inject::None);
    let mut random = Rng::panic(0xaa, 0);
    let unwind = catch_unwind(AssertUnwindSafe(|| {
        let _ = take_ready_direct_rkg_one_prover_session_v1(
            &mut state,
            wrapper,
            &roster,
            &bindings,
            context,
            &mut random,
        );
    }));
    assert!(unwind.is_err());
    assert!(state.party_local_rkg_ephemeral_opening.is_none());
    assert_eq!(state.party_local_rkg_ephemeral_creation_mask, creation_mask);
    assert_eq!(drops(), [1, 0, 1, 1]);
}

#[test]
fn sealed_candidate_remains_opaque_and_unreachable() {
    let candidate = include_str!("../direct_rkg_one_candidate_v1.rs");
    let sealed = include_str!("../direct_rkg_one_sealed_candidate_v1.rs");
    let parent = include_str!("../party_local_rkg_ephemeral_v1.rs");
    let collective = include_str!("../../collective.rs");
    let active = include_str!("../../active_exact_binding.rs");
    let adapter = include_str!("../../active_exact_binding/direct_rkg_one_creator_adapter_v1.rs");
    let publication = include_str!("../direct_rkg_one_publication_v1.rs");
    let prover = include_str!(
        "../../active_exact_binding/direct_relation_wire_v1/rkg_one_creator_prover_v1.rs"
    );

    assert!(candidate.contains("not creator provenance, extractor evidence, receipt,"));
    assert!(candidate.contains("binding, admission, or verifier authority."));
    assert!(sealed.contains("Unverified authority-neutral candidate"));
    assert!(!sealed.contains("fn proof_bytes"));
    assert!(!prover.contains("fn proof_bytes"));
    assert!(!sealed.contains("into_bytes"));
    assert_eq!(
        sealed
            .matches("fn create_direct_rkg_one_sealed_candidate_v1")
            .count(),
        1
    );
    for source in [parent, collective, active] {
        assert!(!source.contains("create_direct_rkg_one_sealed_candidate_v1"));
        assert!(!source.contains("verify_finalized_direct_rkg_one_semantic_candidate_v1"));
        assert!(!source.contains("fn verify_semantic_candidate_v1"));
    }
    for source in [candidate, sealed, adapter, publication, prover] {
        for forbidden in [
            "ReadyRkg2",
            "VerifiedPersistentWitnessBindingV1",
            "VerifiedDirectRelationProofReceiptV1",
            "AdmissionV1",
            "ReleaseGate",
            "verify_and_consume",
        ] {
            assert!(!source.contains(forbidden), "authority escape: {forbidden}");
        }
    }
    assert!(!adapter.contains("bind_direct_relation_use("));
    assert!(!adapter.contains("mint_rkg_round_one_selector_v1"));
    assert!(!candidate.contains("witness_coefficient_v1"));
    assert!(candidate.contains("original_wrapper"));
    assert!(candidate.contains("persistent_guard"));
}

#[test]
fn semantic_handoff_is_ordered_move_only_and_has_no_bypass() {
    let persistent = include_str!("../persistent_direct_opening_v1.rs");
    let candidate = include_str!("../direct_rkg_one_candidate_v1.rs");
    let adapter = include_str!("../../active_exact_binding/direct_rkg_one_creator_adapter_v1.rs");
    let prover = include_str!(
        "../../active_exact_binding/direct_relation_wire_v1/rkg_one_creator_prover_v1.rs"
    );
    let sealed = include_str!("../direct_rkg_one_sealed_candidate_v1.rs");
    let adapter_handoff = bounded_source_section(
        adapter,
        "fn verify_finalized_direct_rkg_one_semantic_candidate_v1",
        "fn prospective_ephemeral_identity_v1",
    );
    let prover_handoff = bounded_source_section(
        prover,
        "fn verify_semantic_candidate_v1",
        "fn seal_direct_rkg_one_proof_owner_v1",
    );
    let sealed_handoff = bounded_source_section(
        sealed,
        "fn verify_semantic_candidate_v1",
        "/// Private construction corridor",
    );

    assert_ordered(
        adapter_handoff,
        &[
            "let FinalizedDirectRkgOneCapabilityV1 {",
            "let semantic =",
            "verify_semantic_candidate_v1(",
            ")?;",
            "_prover_session.into_compacted_post_seal_v1()",
        ],
    );
    assert_ordered(
        prover_handoff,
        &[
            "let Self {",
            "let semantic_owner = verify_finalized_direct_rkg_one_semantic_candidate_v1(",
            "proof.as_bytes()",
            ")?;",
            "_proof: proof",
        ],
    );
    assert_ordered(
        sealed_handoff,
        &[
            "statement_objects_v1()? != objects",
            "return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);",
            "let proof_owner = self.proof_owner;",
            "let publication_owner = self._publication_owner;",
            "proof_owner.verify_semantic_candidate_v1(context, objects, provider)?",
            "Ok(PostSemanticDirectRkgOneCandidateV1 {",
            "_publication_owner: publication_owner,",
        ],
    );
    for handoff in [adapter_handoff, prover_handoff, sealed_handoff] {
        assert!(handoff.contains("impl Sized + use<'a, P>"));
        for forbidden in [
            "callback",
            "into_parts",
            ".clone()",
            "unsafe",
            "ManuallyDrop",
            "MaybeUninit",
            "mem::forget",
            "catch_unwind",
            "Receipt",
            "Binding",
            "Admission",
            "ReadyRkg2",
            "ReleaseGate",
        ] {
            assert!(!handoff.contains(forbidden), "handoff escape: {forbidden}");
        }
    }

    assert_eq!(adapter.matches("fn into_compacted_post_seal_v1").count(), 0);
    assert_eq!(prover.matches("fn into_compacted_post_seal_v1").count(), 0);
    assert!(!sealed.contains("into_compacted_sealed_candidate_v1"));
    assert!(!sealed.contains("CompactedSealedDirectRkgOneCandidateV1"));
    assert_eq!(
        candidate.matches("fn into_compacted_post_seal_v1").count(),
        1
    );
    assert_eq!(
        persistent.matches("fn into_compacted_post_seal_v1").count(),
        1
    );
    for (source, marker, fields) in [
        (
            adapter,
            "struct FinalizedDirectRkgOneCapabilityV1<'a> {",
            ["_prover_session:", "capability:"],
        ),
        (
            prover,
            "struct PostSemanticDirectRkgOneProofOwnerV1<S> {",
            ["_semantic_owner:", "_proof:"],
        ),
        (
            sealed,
            "struct PostSemanticDirectRkgOneCandidateV1<S> {",
            ["_proof_owner:", "_publication_owner:"],
        ),
    ] {
        let owner = struct_body(source, marker);
        assert_eq!(owner.matches(": ").count(), 2);
        for field in fields {
            assert_eq!(owner.matches(field).count(), 1, "owner field: {field}");
        }
    }
    for source in [prover, sealed] {
        for forbidden in [
            "impl Clone for PostSemantic",
            "impl Copy for PostSemantic",
            "Norito",
        ] {
            assert!(!source.contains(forbidden), "owner escape: {forbidden}");
        }
    }
    let top_line = sealed
        .lines()
        .find(|line| line.contains("fn verify_semantic_candidate_v1"))
        .expect("private top semantic handoff");
    assert!(top_line.trim_start().starts_with("fn "));
}

#[test]
fn semantic_error_or_unwind_cannot_restore_the_burn_or_open_a_gate() {
    let candidate = include_str!("../direct_rkg_one_candidate_v1.rs");
    let take = candidate
        .split_once("fn take_ready_direct_rkg_one_prover_session_v1")
        .expect("prover-session take corridor")
        .1;
    assert_eq!(take.matches(".take()").count(), 1);
    assert_ordered(
        take,
        &[
            ".take()",
            "let digit_bit = 1_u64",
            ".checked_shl(",
            "creation_mask & digit_bit == 0",
            "(&state.party_local_rkg_ephemeral_creation_mask, digit_bit)",
        ],
    );
    assert!(take.contains("&state.public_error"));
    for forbidden in [
        "consumer_mask",
        "party_local_rkg_ephemeral_opening = Some",
        "party_local_rkg_ephemeral_creation_mask &=",
        "party_local_rkg_ephemeral_creation_mask ^=",
    ] {
        assert!(!take.contains(forbidden), "burn escape: {forbidden}");
    }

    let active = include_str!("../../active_exact_binding.rs");
    for gate in [
        "let external_commitment_provenance_certified = false;",
        "let full_basis_mrep_crs_certified = false;",
        "let membership_argument_of_knowledge_certified = false;",
        "let membership_zero_knowledge_certified = false;",
        "let composite_rom_forking_certified = false;",
        "let full_ceremony_10_336_instance_composition_certified = false;",
        "let canonical_complete_wire_certified = false;",
        "let chunked_workspace_certified = false;",
        "let sampler_wired_to_runtime = false;",
        "let persistent_graph_wired_to_runtime = false;",
        "let split_decryption_wide_relation_certified = false;",
        "let release_kat_pinned = false;",
    ] {
        assert!(active.contains(gate), "closed gate: {gate}");
    }
    assert!(include_str!("../../resource.rs").contains("release_peak_memory_measured: false"));
}

#[test]
fn prover_session_derives_persistent_points_from_installed_owner() {
    let persistent = include_str!("../persistent_direct_opening_v1.rs");
    let candidate = include_str!("../direct_rkg_one_candidate_v1.rs");
    let constructor = persistent
        .split_once("fn from_installed_binding_v1")
        .expect("installed-binding guard constructor")
        .1
        .split_once("fn checked_commitments_v1")
        .expect("installed-binding guard constructor boundary")
        .0;
    for required in [
        "owner: &'a mut PersistentDirectOpeningOwnerV1",
        "public_error: &'a SecretPolynomial",
        "creation_mask_digit_burn: (&'a u64, u64)",
        "binding.commitments() != &commitments",
        "encode_persistent_opening_commitments_v1(&commitments)?",
    ] {
        assert!(constructor.contains(required), "{required}");
    }
    for forbidden in ["expected_", "Vec<", "try_reserve"] {
        assert!(!constructor.contains(forbidden), "{forbidden}");
    }
    let accessor = persistent.split("checked_commitments_v1").nth(1).unwrap();
    assert!(accessor.contains(".verified_binding"));
    assert!(accessor.contains(".commitments()"));
    let take = candidate
        .split_once("fn take_ready_direct_rkg_one_prover_session_v1")
        .expect("prover-session take corridor")
        .1;
    assert!(take.contains("persistent_secret_binding_for("));
    assert!(take.contains("binding_identity != bindings.identity_digests()[party_index]"));
    assert!(take.contains("*persistent_guard.checked_commitments_v1()?"));
    assert!(!take.contains("binding_commitments"));
}

#[test]
fn semantic_overlap_accounting_is_only_a_logical_lower_bound() {
    const COMMON_A: usize = 39_845_888;
    const RKG_ONE_ERRORS: usize = 2_097_152;
    const PERSISTENT_NARROWING: usize = 131_072;
    const PROOF: usize = 25_248_766;
    const EPHEMERAL_U: usize = 1_048_576;
    const ORIGINAL_WRAPPER: usize = 11_576;
    const PERSISTENT_SECRET: usize = 1_048_576;
    const PUBLIC_ERROR: usize = 1_048_576;
    const GENERATORS: usize = 12_584_544;
    const RNS_LEDGER: usize = 87_031_808;
    let released_after_success = COMMON_A + RKG_ONE_ERRORS + PERSISTENT_NARROWING;
    let compact_candidate = PROOF + EPHEMERAL_U + ORIGINAL_WRAPPER;
    // Membership payloads are already inside `PROOF`; adding 71_568 would double-count them.
    let post_success =
        compact_candidate + PERSISTENT_SECRET + PUBLIC_ERROR + GENERATORS + RNS_LEDGER;
    let verification_overlap = post_success + released_after_success;
    assert_eq!(released_after_success, 42_074_112);
    assert_eq!(compact_candidate, 26_308_918);
    assert_eq!(post_success, 128_022_422);
    assert_eq!(verification_overlap, 170_096_534);
    let sealed = include_str!("../direct_rkg_one_sealed_candidate_v1.rs");
    for disclaimer in [
        "logical payload lower bounds",
        "not heap/RSS",
        "headroom",
        "certification",
        "170_096_534",
        "128_022_422",
    ] {
        assert!(sealed.contains(disclaimer));
    }
}

#[test]
fn candidate_and_handoff_support_areas_stay_within_review_caps() {
    let adapter = include_str!("../../active_exact_binding/direct_rkg_one_creator_adapter_v1.rs");
    let prover = include_str!(
        "../../active_exact_binding/direct_relation_wire_v1/rkg_one_creator_prover_v1.rs"
    );
    let candidate = include_str!("../direct_rkg_one_candidate_v1.rs");
    let sealed = include_str!("../direct_rkg_one_sealed_candidate_v1.rs");
    assert!(adapter.lines().count() <= 500 && adapter.len() <= 24 * 1024);
    assert!(prover.lines().count() <= 500 && prover.len() <= 24 * 1024);
    assert!(candidate.lines().count() + sealed.lines().count() <= 500);
    assert!(candidate.len() + sealed.len() <= 24 * 1024);
    for source in [
        include_str!("../persistent_direct_opening_v1.rs"),
        include_str!("../borrowed_product.rs"),
    ] {
        assert!(source.lines().count() <= 500 && source.len() <= 24 * 1024);
    }
    let tests = include_str!("direct_rkg_one_candidate_v1_tests.rs");
    assert!(tests.lines().count() <= 500 && tests.len() <= 24 * 1024);
}
