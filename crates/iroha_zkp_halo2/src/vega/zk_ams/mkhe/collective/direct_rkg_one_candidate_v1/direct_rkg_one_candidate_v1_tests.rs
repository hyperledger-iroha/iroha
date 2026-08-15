use super::super::tests::{Inject, Rng, begin, drops};
use super::*;
use crate::vega::zk_ams::mkhe::{
    active::ZkAmsMkheGovernedActiveRosterV1,
    active_exact_binding::VerifiedPersistentWitnessBindingSetV1,
    direct_collective_eval_ceremony::ZkAmsMkheDirectEvaluatedKeyTargetV1,
    direct_rkg_ephemeral_membership::tests::creator_state_fixture,
};
use std::panic::{AssertUnwindSafe, catch_unwind};

fn compact_body(source: &str) -> &str {
    source
        .split("fn into_compacted_post_seal_v1")
        .nth(1)
        .expect("nested compacting method")
        .split("\n    }\n")
        .next()
        .expect("nested compacting boundary")
}

fn struct_body<'a>(source: &'a str, marker: &str) -> &'a str {
    source
        .split(marker)
        .nth(1)
        .expect("owned structure")
        .split("\n}")
        .next()
        .expect("owned structure boundary")
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
    let result = take_ready_direct_rkg_one_owner_v1(
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
        let _ = take_ready_direct_rkg_one_owner_v1(
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
fn sealed_candidate_remains_opaque_unverified_and_unreachable() {
    let candidate = include_str!("../direct_rkg_one_candidate_v1.rs");
    let sealed = include_str!("../direct_rkg_one_sealed_candidate_v1.rs");
    let parent = include_str!("../party_local_rkg_ephemeral_v1.rs");
    let collective = include_str!("../../collective.rs");
    let adapter = include_str!("../../active_exact_binding/direct_rkg_one_creator_adapter_v1.rs");
    let publication = include_str!("../direct_rkg_one_publication_v1.rs");
    let prover = include_str!(
        "../../active_exact_binding/direct_relation_wire_v1/rkg_one_creator_prover_v1.rs"
    );
    let common = include_str!("../../active_exact_binding/direct_common_a_v1/creator_replay_v1.rs");

    assert!(sealed.contains("Unverified authority-neutral candidate"));
    assert!(sealed.contains("fn proof_bytes(&self) -> &[u8]"));
    assert!(!sealed.contains("into_bytes"));
    assert_eq!(
        sealed
            .matches("fn create_direct_rkg_one_sealed_candidate_v1")
            .count(),
        1
    );
    for source in [parent, collective] {
        assert!(!source.contains("create_direct_rkg_one_sealed_candidate_v1"));
        assert!(!source.contains("SealedDirectRkgOneCandidateV1"));
    }
    for source in [candidate, sealed, adapter, publication, prover, common] {
        assert!(!source.contains("ReadyRkg2"));
        assert!(!source.contains("VerifiedPersistentWitnessBindingV1"));
        assert!(!source.contains("VerifiedDirectRelationProofReceiptV1"));
        assert!(!source.contains("AdmissionV1"));
        assert!(!source.contains("ReleaseGate"));
        assert!(!source.contains("verify_and_consume"));
    }
    assert!(!adapter.contains("bind_direct_relation_use("));
    assert!(!adapter.contains("mint_rkg_round_one_selector_v1"));
    assert!(!candidate.contains("witness_coefficient_v1"));
    assert!(candidate.contains("original_wrapper"));
    assert!(candidate.contains("persistent_guard"));
    assert!(adapter.contains("FinalizedDirectRkgOneCapabilityV1"));
}

#[test]
fn post_seal_compaction_moves_the_exact_private_owner_chain() {
    let persistent = include_str!("../persistent_direct_opening_v1.rs");
    let candidate = include_str!("../direct_rkg_one_candidate_v1.rs");
    let adapter = include_str!("../../active_exact_binding/direct_rkg_one_creator_adapter_v1.rs");
    let prover = include_str!(
        "../../active_exact_binding/direct_relation_wire_v1/rkg_one_creator_prover_v1.rs"
    );
    let sealed = include_str!("../direct_rkg_one_sealed_candidate_v1.rs");
    let provenance = struct_body(
        candidate,
        "struct DirectRkgOneOwnerDerivedProvenanceV1<'a> {",
    );
    for field in [
        "persistent_guard:",
        "ephemeral_owner:",
        "_original_wrapper:",
        "error_zero:",
        "error_one:",
        "bound_two_blindings:",
        "persistent_commitments:",
        "ephemeral_commitments:",
        "common_a_matrix:",
    ] {
        assert_eq!(provenance.matches(field).count(), 1, "field {field}");
    }
    assert_eq!(provenance.matches(": ").count(), 9);
    let guard = struct_body(
        persistent,
        "struct PostCpkPersistentDirectOpeningGuardV1<'a> {",
    );
    for field in [
        "owner:",
        "coefficients:",
        "public_error:",
        "creation_mask_digit_burn:",
    ] {
        assert_eq!(guard.matches(field).count(), 1, "guard field {field}");
    }
    assert_eq!(guard.matches(": ").count(), 4);
    for (source, marker, fields) in [
        (
            adapter,
            "struct FinalizedDirectRkgOneCapabilityV1<'a> {",
            ["_provenance:", "capability:"],
        ),
        (
            prover,
            "struct SealedDirectRkgOneProofOwnerV1<'a> {",
            ["_finalized_capability:", "proof:"],
        ),
        (
            sealed,
            "struct SealedDirectRkgOneCandidateV1<'a> {",
            ["proof_owner:", "_publication_owner:"],
        ),
    ] {
        let owner = struct_body(source, marker);
        assert_eq!(owner.matches(": ").count(), 2);
        for field in fields {
            assert_eq!(owner.matches(field).count(), 1, "owner field {field}");
        }
    }
    for (source, declaration) in [
        (
            persistent,
            "pub(in crate::vega::zk_ams::mkhe::collective) fn into_compacted_post_seal_v1",
        ),
        (
            candidate,
            "pub(in crate::vega::zk_ams::mkhe) fn into_compacted_post_seal_v1",
        ),
        (
            adapter,
            "pub(in crate::vega::zk_ams::mkhe::active_exact_binding) fn into_compacted_post_seal_v1",
        ),
        (
            prover,
            "pub(in crate::vega::zk_ams::mkhe) fn into_compacted_post_seal_v1",
        ),
    ] {
        assert!(source.contains(declaration));
        let compact = compact_body(source);
        assert!(compact.trim_start().starts_with('('));
        assert!(compact.contains("-> impl Sized + 'a"));
        for forbidden in [
            "Result<",
            "?",
            ".clone()",
            "validate(",
            "try_reserve",
            "provider",
            "unsafe",
            "ManuallyDrop",
            "MaybeUninit",
            "mem::forget",
            "Norito",
            "mint",
            "verify",
            "receipt",
        ] {
            assert!(
                !compact.contains(forbidden),
                "compaction operation: {forbidden}"
            );
        }
    }
    let persistent_body = compact_body(persistent);
    for retained in [
        "self.owner",
        "self.public_error",
        "self.creation_mask_digit_burn",
    ] {
        assert_eq!(persistent_body.matches(retained).count(), 1);
    }
    assert_eq!(
        persistent_body.matches("drop(self.coefficients)").count(),
        1
    );
    let candidate_body = compact_body(candidate);
    for retained in [
        "self.persistent_guard.into_compacted_post_seal_v1()",
        "self.ephemeral_owner",
        "self._original_wrapper",
        "self.bound_two_blindings",
        "self.persistent_commitments",
        "self.ephemeral_commitments",
    ] {
        assert_eq!(
            candidate_body.matches(retained).count(),
            1,
            "retained {retained}"
        );
    }
    for dead in ["self.error_zero", "self.error_one", "self.common_a_matrix"] {
        assert_eq!(candidate_body.matches(dead).count(), 1, "dead {dead}");
    }
    assert_eq!(candidate_body.matches("drop((").count(), 1);
    assert_eq!(
        compact_body(adapter)
            .matches("self._provenance.into_compacted_post_seal_v1()")
            .count(),
        1
    );
    assert_eq!(compact_body(adapter).matches("self.capability").count(), 1);
    assert_eq!(
        compact_body(prover)
            .matches("self._finalized_capability.into_compacted_post_seal_v1()")
            .count(),
        1
    );
    assert_eq!(compact_body(prover).matches("self.proof").count(), 1);

    let final_line = sealed
        .lines()
        .find(|line| line.contains("fn into_compacted_sealed_candidate_v1"))
        .expect("plain-private final compaction");
    assert!(final_line.trim_start().starts_with("fn "));
    assert_eq!(
        sealed.matches("into_compacted_sealed_candidate_v1").count(),
        1
    );
    assert_eq!(
        sealed
            .matches("CompactedSealedDirectRkgOneCandidateV1")
            .count(),
        2
    );
    let final_body = sealed
        .split("fn into_compacted_sealed_candidate_v1")
        .nth(1)
        .expect("final compacting method")
        .split("\n    }\n")
        .next()
        .expect("final compacting boundary");
    assert_eq!(final_body.matches("self.proof_owner").count(), 1);
    assert_eq!(final_body.matches("self._publication_owner").count(), 1);
    for forbidden in ["ReadyRkg2", "Receipt", "Binding", "Admission", "Result<"] {
        assert!(
            !final_body.contains(forbidden),
            "authority escape: {forbidden}"
        );
    }
}

#[test]
fn post_seal_compaction_retains_burn_and_has_no_escape_or_gate() {
    let candidate = include_str!("../direct_rkg_one_candidate_v1.rs");
    let sealed = include_str!("../direct_rkg_one_sealed_candidate_v1.rs");
    let take = candidate
        .split("fn take_ready_direct_rkg_one_owner_v1")
        .nth(1)
        .expect("candidate take corridor");
    assert_eq!(take.matches(".take()").count(), 1);
    let taken = take.find(".take()").expect("single ephemeral take");
    let one_hot = take.find("let digit_bit = 1_u64").expect("one-hot burn");
    let checked_shift = take.find(".checked_shl(").expect("checked digit shift");
    let burn_check = take
        .find("creation_mask & digit_bit == 0")
        .expect("permanent burn check");
    let lease = take
        .find("(&state.party_local_rkg_ephemeral_creation_mask, digit_bit)")
        .expect("permanent burn lease");
    assert!(
        taken < one_hot
            && one_hot < checked_shift
            && checked_shift < burn_check
            && burn_check < lease
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

    for source in [
        include_str!("../../../mkhe.rs"),
        include_str!("../../collective.rs"),
        include_str!("../../active_exact_binding.rs"),
        include_str!(
            "../../active_exact_binding/direct_relation_wire_v1/predecode_v1/rkg_one_semantic_verifier_v1.rs"
        ),
        include_str!("../direct_rkg_one_publication_v1.rs"),
        include_str!("../direct_rkg_one_publication_v1/direct_rkg_one_orphan_journal_v1.rs"),
    ] {
        assert!(!source.contains("into_compacted_sealed_candidate_v1"));
        assert!(!source.contains("CompactedSealedDirectRkgOneCandidateV1"));
    }
    assert!(!sealed.contains("pub(crate)"));
    assert!(!sealed.contains("impl Clone for Compacted"));
    assert!(!sealed.contains("impl Copy for Compacted"));
    assert!(!sealed.contains("impl From"));
    assert!(!sealed.contains("impl Default"));

    let active = include_str!("../../active_exact_binding.rs");
    for gate in [
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
fn post_seal_compaction_accounting_is_only_a_logical_lower_bound() {
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
    let saving = COMMON_A + RKG_ONE_ERRORS + PERSISTENT_NARROWING;
    let compact_candidate = PROOF + EPHEMERAL_U + ORIGINAL_WRAPPER;
    // The 48 borrowed membership-proof payloads remain inside `PROOF`; adding
    // their 71_568 bytes separately would double-count retained proof bytes.
    let lower_bound =
        compact_candidate + PERSISTENT_SECRET + PUBLIC_ERROR + GENERATORS + RNS_LEDGER;
    assert_eq!(saving, 42_074_112);
    assert_eq!(compact_candidate, 26_308_918);
    assert_eq!(lower_bound, 128_022_422);
    let sealed = include_str!("../direct_rkg_one_sealed_candidate_v1.rs");
    for disclaimer in [
        "logical payload lower bounds",
        "not heap, RSS",
        "headroom",
        "certification",
    ] {
        assert!(sealed.contains(disclaimer));
    }
}

#[test]
fn candidate_and_owner_support_areas_stay_within_review_caps() {
    let candidate = include_str!("../direct_rkg_one_candidate_v1.rs");
    let sealed = include_str!("../direct_rkg_one_sealed_candidate_v1.rs");
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
