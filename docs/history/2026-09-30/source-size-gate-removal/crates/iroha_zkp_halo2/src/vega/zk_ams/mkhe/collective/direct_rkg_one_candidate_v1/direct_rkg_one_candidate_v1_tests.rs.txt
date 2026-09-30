use super::super::direct_rkg_one_sealed_candidate_v1::SealedDirectRkgOneCandidateV1;
use super::super::tests::{Inject, Rng, begin, drops};
use super::*;
use crate::vega::zk_ams::mkhe::{
    active::ZkAmsMkheGovernedActiveRosterV1,
    active_exact_binding::{DirectRelationPublicObjectsV1, VerifiedPersistentWitnessBindingSetV1},
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

fn assert_owner_fields(source: &str, marker: &str, fields: &str) {
    let owner = struct_body(source, marker);
    let fields = fields.split('|');
    assert_eq!(owner.matches(": ").count(), fields.clone().count());
    for field in fields {
        assert_eq!(owner.matches(field).count(), 1, "owner field: {field}");
    }
}

fn assert_restricted_reexport(source: &str, item: &str) {
    assert_eq!(source.matches(item).count(), 1);
    let before = source.split_once(item).expect("restricted reexport").0;
    let route = before.rsplit_once("pub(").expect("restricted visibility").1;
    assert!(route.starts_with("in crate::vega::zk_ams::mkhe) use "));
    assert!(!route.contains(';'));
}

#[expect(dead_code, reason = "compile-only precise-capture check")]
fn sealed_semantic_owner_does_not_borrow_provider_or_objects<'a, P>(
    owner: SealedDirectRkgOneCandidateV1<'a>,
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
    let mut fixture = ReadyFixture::new(b"rkg1-post-take-error");
    let creation_mask = fixture.state.party_local_rkg_ephemeral_creation_mask;
    begin(Inject::None);
    let result = take_ready_direct_rkg_one_prover_session_v1(
        &mut fixture.state,
        fixture.wrapper,
        &fixture.roster,
        &fixture.bindings,
        fixture.context,
        &mut Rng::fail(0xaa, 0),
    );
    assert!(matches!(result, Err(ZkAmsMkheErrorV1::RandomUnavailable)));
    assert!(fixture.state.party_local_rkg_ephemeral_opening.is_none());
    let post_mask = fixture.state.party_local_rkg_ephemeral_creation_mask;
    assert_eq!(post_mask, creation_mask);
    assert_eq!(drops(), [1, 0, 1, 1]);
}

#[test]
fn post_take_unwind_burns_ephemeral_owner_and_keeps_creation_bit() {
    let mut fixture = ReadyFixture::new(b"rkg1-post-take-unwind");
    let creation_mask = fixture.state.party_local_rkg_ephemeral_creation_mask;
    begin(Inject::None);
    let mut random = Rng::panic(0xaa, 0);
    let unwind = catch_unwind(AssertUnwindSafe(|| {
        let _ = take_ready_direct_rkg_one_prover_session_v1(
            &mut fixture.state,
            fixture.wrapper,
            &fixture.roster,
            &fixture.bindings,
            fixture.context,
            &mut random,
        );
    }));
    assert!(unwind.is_err());
    assert!(fixture.state.party_local_rkg_ephemeral_opening.is_none());
    let post_mask = fixture.state.party_local_rkg_ephemeral_creation_mask;
    assert_eq!(post_mask, creation_mask);
    assert_eq!(drops(), [1, 0, 1, 1]);
}

#[test]
fn sealed_candidate_remains_opaque_and_unreachable() {
    let candidate = include_str!("../direct_rkg_one_candidate_v1.rs");
    let sealed = include_str!("../direct_rkg_one_sealed_candidate_v1.rs");
    let creator = include_str!("../direct_rkg_one_creator_v2.rs");
    let lifecycle = include_str!("../direct_rkg_one_publication_v1/direct_rkg_one_lifecycle_v2.rs");
    let parent = include_str!("../party_local_rkg_ephemeral_v1.rs");
    let collective = include_str!("../../collective.rs");
    let active = include_str!("../../active_exact_binding.rs");
    let direct_wire = include_str!("../../active_exact_binding/direct_relation_wire_v1.rs");
    let adapter = include_str!("../../active_exact_binding/direct_rkg_one_creator_adapter_v1.rs");
    let publication = include_str!("../direct_rkg_one_publication_v1.rs");
    let prover = include_str!(
        "../../active_exact_binding/direct_relation_wire_v1/rkg_one_creator_prover_v1.rs"
    );
    let permit = "DirectRkgOneProofDurabilityPermitV2";
    let permit_literal = "DirectRkgOneProofDurabilityPermitV2 { _private: () }";
    let permit_declaration = bounded_source_section(
        lifecycle,
        "/// Proof durability witness; only this lifecycle module can construct it.",
        "/// Sole live permit accepted before H0 staging; never returned by recovery.",
    );

    assert!(!sealed.contains("fn proof_bytes"));
    assert!(!prover.contains("fn proof_bytes"));
    assert_eq!(
        creator
            .matches("fn create_direct_rkg_one_sealed_candidate_v2")
            .count(),
        1
    );
    assert!(sealed.contains("pub(super) const fn from_durable_parts_v2"));
    assert!(permit_declaration.contains(
        "pub(in crate::vega::zk_ams::mkhe) struct DirectRkgOneProofDurabilityPermitV2 {\n    _private: (),\n}"
    ));
    for forbidden in ["derive", "impl ", "fn ", "const ", "static "] {
        assert!(!permit_declaration.contains(forbidden));
    }
    assert_eq!(lifecycle.matches(permit).count(), 2);
    assert_eq!(lifecycle.matches(permit_literal).count(), 1);
    assert_eq!(prover.matches(permit).count(), 2);
    for source in [publication, parent, collective] {
        assert_restricted_reexport(source, permit);
    }
    let no_route = format!("{candidate}{sealed}{creator}{adapter}{active}{direct_wire}");
    assert!(!no_route.contains(permit));
    let no_mint = format!("{prover}{creator}{sealed}{publication}{parent}{collective}{active}");
    assert!(!no_mint.contains(permit_literal));
    for source in [parent, collective, active] {
        assert!(!source.contains("create_direct_rkg_one_sealed_candidate_v2"));
        assert!(!source.contains("verify_finalized_direct_rkg_one_semantic_candidate_v1"));
        assert!(!source.contains("fn verify_semantic_candidate_v1"));
    }
    let authority_sources =
        format!("{candidate}{sealed}{creator}{lifecycle}{adapter}{publication}{prover}");
    for forbidden in [
        "ReadyRkg2",
        "VerifiedPersistentWitnessBindingV1",
        "VerifiedDirectRelationProofReceiptV1",
        "AdmissionV1",
        "ReleaseGate",
        "verify_and_consume",
    ] {
        assert!(!authority_sources.contains(forbidden));
    }
    assert!(!adapter.contains("bind_direct_relation_use("));
    assert!(!adapter.contains("mint_rkg_round_one_selector_v1"));
    assert!(!candidate.contains("witness_coefficient_v1"));
    assert!(candidate.contains("original_wrapper"));
    assert!(candidate.contains("persistent_guard"));
}

#[test]
fn semantic_handoff_is_ordered_move_only_and_has_no_bypass() {
    let adapter = include_str!("../../active_exact_binding/direct_rkg_one_creator_adapter_v1.rs");
    let prover = include_str!(
        "../../active_exact_binding/direct_relation_wire_v1/rkg_one_creator_prover_v1.rs"
    );
    let sealed = include_str!("../direct_rkg_one_sealed_candidate_v1.rs");
    let lifecycle = include_str!("../direct_rkg_one_publication_v1/direct_rkg_one_lifecycle_v2.rs");
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
    let lifecycle_handoff = bounded_source_section(
        lifecycle,
        "fn verify_semantic_candidate_v2",
        "pub(in super::super) enum DirectRkgOneFreshReservationOutcomeV2",
    );
    let sealed_handoff = sealed
        .split_once("fn verify_semantic_candidate_v1")
        .expect("sealed semantic handoff")
        .1;

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
            "_durability_permit: DirectRkgOneProofDurabilityPermitV2,",
            "let Self {",
            "sealed,",
            "publication,",
            "let semantic_owner = verify_finalized_direct_rkg_one_semantic_candidate_v1(",
            "proof.as_bytes()",
            ")?;",
            "_proof: proof,",
            "_publication: publication,",
        ],
    );
    assert_ordered(
        lifecycle_handoff,
        &[
            "let Self {",
            "proof_owner,",
            "publication_owner,",
            "proof_owner.verify_semantic_candidate_v1(",
            "DirectRkgOneProofDurabilityPermitV2 { _private: () },",
            "context,",
            "objects,",
            "provider,",
            ")?;",
            "_publication_owner: publication_owner,",
        ],
    );
    assert_ordered(
        sealed_handoff,
        &[
            "statement_objects_v2()? != objects",
            "return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);",
            "let lifecycle_owner = self",
            ".lifecycle_owner",
            ".verify_semantic_candidate_v2(context, objects, provider)?",
            "Ok(PostSemanticDirectRkgOneCandidateV1 {",
            "_lifecycle_owner: lifecycle_owner,",
        ],
    );
    assert!(adapter_handoff.contains("impl Sized + use<'a, P>"));
    assert!(prover_handoff.contains("impl Sized + use<'a, P>"));
    assert!(lifecycle_handoff.contains("impl Sized + use<'a, P>"));
    assert!(sealed_handoff.contains("impl Sized + use<'a, P>"));
    let handoffs = format!("{adapter_handoff}{prover_handoff}{lifecycle_handoff}{sealed_handoff}");
    for forbidden in [
        "callback",
        "into_parts",
        ".clone()",
        "unsafe",
        "ManuallyDrop",
        "MaybeUninit",
        "mem::forget",
        "catch_unwind",
        "Binding",
        "Admission",
        "ReadyRkg2",
        "ReleaseGate",
    ] {
        assert!(!handoffs.contains(forbidden), "handoff escape: {forbidden}");
    }

    assert!(!sealed.contains("into_compacted_sealed_candidate_v1"));
    assert!(!sealed.contains("CompactedSealedDirectRkgOneCandidateV1"));
    assert_owner_fields(
        adapter,
        "struct FinalizedDirectRkgOneCapabilityV1<'a> {",
        "_prover_session:|capability:",
    );
    assert_owner_fields(
        prover,
        "struct PostSemanticDirectRkgOneProofOwnerV1<S> {",
        "_semantic_owner:|_proof:|_publication:",
    );
    assert_owner_fields(
        lifecycle,
        "struct PostSemanticDirectRkgOneLifecycleOwnerV2<S> {",
        "_proof_owner:|_publication_owner:|_scope:|_storage_key:|_record:",
    );
    assert_owner_fields(
        sealed,
        "struct PostSemanticDirectRkgOneCandidateV1<S> {",
        "_lifecycle_owner:",
    );
    let owners = format!("{prover}{lifecycle}{sealed}");
    assert!(!owners.contains("impl Clone for PostSemantic"));
    assert!(!owners.contains("impl Copy for PostSemantic"));
    assert!(!owners.contains("Norito"));
    let top_line = sealed
        .lines()
        .find(|line| line.contains("fn verify_semantic_candidate_v1"))
        .expect("private top semantic handoff");
    assert!(top_line.trim_start().starts_with("pub(super) fn "));
    let unpublished = bounded_source_section(
        prover,
        "impl<'a> SealedDirectRkgOneProofOwnerV1<'a>",
        "impl<'a> PublishedDirectRkgOneProofOwnerV2<'a>",
    );
    assert!(!unpublished.contains("fn verify_semantic_candidate_v1"));
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
    let released_after_success = 39_845_888 + 2_097_152 + 131_072;
    let compact_candidate = 25_248_766 + 1_048_576 + 11_576;
    // Membership payloads are already inside `PROOF`; adding 71_568 would double-count them.
    let post_success = compact_candidate + 1_048_576 + 1_048_576 + 12_584_544 + 87_031_808;
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
