use super::*;

use std::collections::BTreeSet;

use halo2_proofs::halo2curves::{
    group::prime::PrimeCurveAffine as _,
    pasta::{EpAffine, EqAffine, Fp, Fq},
};
use sha2::{Digest as _, Sha256};

use super::super::{
    registered_platform_p256_circuit_source::assemble_unverified_registered_platform_p256_circuit_candidates_v2,
    registered_platform_p256_statement::{
        REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2,
        registered_platform_p256_source_pair_for_test_v2,
    },
    state_recursive_fold::{
        StateRecursiveFoldInputRoleV2, state_guard_inputs_from_verified_guard_bundle_v2,
    },
};
use crate::zk::offline_cash_v1::{
    OfflineCashHalo2CircuitRoleV1, OfflineCashHalo2ParityV1,
    offline_cash_halo2_protocol_identity_v1,
};

fn decode_hex<const N: usize>(encoded: &str) -> [u8; N] {
    hex::decode(encoded)
        .expect("fixture is hexadecimal")
        .try_into()
        .unwrap_or_else(|_| panic!("fixture has exactly {N} bytes"))
}

fn exact_p256_statement() -> [u8; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2] {
    let x = decode_hex::<32>("60FED4BA255A9D31C961EB74C6356D68C049B8923B61FA6CE669622E60F29FB6");
    let y = decode_hex::<32>("7903FE1008B8BC99A41AE9E95628BC64F2F1B20C2D7E9F5177A3C294D4462299");
    let prehash =
        decode_hex::<32>("90801AB8A0473D3800296DAAFC313EB49E469993CFDC3F3EE7644218B24E66AC");
    let r = decode_hex::<32>("EFD48B2AACB6A8FD1140DD9CD45E81D69D2C877B56AAF991C34D0EA84EAF3716");
    let low_s =
        decode_hex::<32>("0834E36AD29A83BF2BC9385E491D6099C8FDF9D1ED67AA7EA5F51F93782857A9");
    let mut statement = [0_u8; REGISTERED_PLATFORM_P256_STATEMENT_BYTES_V2];
    statement[0] = 4;
    statement[1..33].copy_from_slice(&x);
    statement[33..65].copy_from_slice(&y);
    statement[65..97].copy_from_slice(&prehash);
    statement[97..129].copy_from_slice(&r);
    statement[129..].copy_from_slice(&low_s);
    statement
}

fn p256_candidates() -> UnverifiedRegisteredPlatformP256CircuitCandidatesV2 {
    assemble_unverified_registered_platform_p256_circuit_candidates_v2(
        registered_platform_p256_source_pair_for_test_v2(exact_p256_statement()),
    )
    .expect("exact registered P-256 fixture")
}

fn statement(p256_present: bool) -> OfflineCashGuardBundleStatementV2 {
    let p256 = exact_p256_statement();
    let platform_key_digest: [u8; 32] = Sha256::digest(&p256[..65]).into();
    OfflineCashGuardBundleStatementV2 {
        operation: OfflineCashGuardBundleOperationV2::SendSplit,
        android_key_cert_present: false,
        p256_signature_present: p256_present,
        from_sequence: 7,
        to_sequence: 8,
        release_id: [0x11; 32],
        context_digest: [0x12; 32],
        current_head: [0x13; 32],
        current_lineage_digest: [0x14; 32],
        transition_digest: [0x15; 32],
        wallet_binding: [0x16; 32],
        hardware_policy_id: [0x17; 32],
        guard_device_id: [0x18; 32],
        current_guard_binding: [0x19; 32],
        next_guard_binding: [0x1a; 32],
        platform_key_digest,
        platform_message_digest: decode_hex::<32>(
            "90801AB8A0473D3800296DAAFC313EB49E469993CFDC3F3EE7644218B24E66AC",
        ),
        guard_use_claim_digest: [0x31; 32],
        platform_bind_claim_digest: [0x32; 32],
        android_certificate_digest: [0; 32],
        android_tbs_digest: [0; 32],
        android_issuer_key_digest: [0; 32],
        android_attestation_digest: [0; 32],
        android_key_cert_claim_digest: [0; 32],
        registration_receipt_commitment: if p256_present { [0x22; 32] } else { [0; 32] },
        guard_bundle_digest: [0x33; 32],
    }
}

fn helper_owner(
    p256_present: bool,
) -> Result<AuthenticatedOfflineCashCurrentHelperOwnerV2, OfflineCashGuardBundleProvenanceErrorV2> {
    AuthenticatedOfflineCashCurrentHelperOwnerV2::from_test_statement_v2(statement(p256_present))
}

fn eq_lineage(seed: u64) -> OfflineCashEqParentLineageV2 {
    OfflineCashEqParentLineageV2::live(
        std::array::from_fn(|index| Fp::from(seed + index as u64)),
        EqAffine::generator(),
    )
    .expect("live Eq lineage")
}

fn ep_lineage(seed: u64) -> OfflineCashEpParentLineageV2 {
    OfflineCashEpParentLineageV2::live(
        std::array::from_fn(|index| Fq::from(seed + index as u64)),
        EpAffine::generator(),
    )
    .expect("live Ep lineage")
}

fn assemble_present()
-> Result<UnverifiedOfflineCashGuardBundleProvenanceV2, OfflineCashGuardBundleProvenanceErrorV2> {
    assemble_unverified_offline_cash_guard_bundle_provenance_v2(
        helper_owner(true)?,
        OfflineCashRegisteredP256ChildProvenanceV2::Present(p256_candidates()),
        &eq_lineage(101),
        &ep_lineage(201),
    )
}

fn assemble_absent()
-> Result<UnverifiedOfflineCashGuardBundleProvenanceV2, OfflineCashGuardBundleProvenanceErrorV2> {
    assemble_unverified_offline_cash_guard_bundle_provenance_v2(
        helper_owner(false)?,
        OfflineCashRegisteredP256ChildProvenanceV2::CanonicallyAbsent,
        &eq_lineage(301),
        &ep_lineage(401),
    )
}

#[test]
fn finite_v2_roles_and_source_protocol_identities_are_exact_and_not_v1() {
    assert_eq!(
        OfflineCashHalo2CircuitRoleV2::ALL,
        [
            OfflineCashHalo2CircuitRoleV2::State,
            OfflineCashHalo2CircuitRoleV2::GuardUse,
            OfflineCashHalo2CircuitRoleV2::PlatformBind,
            OfflineCashHalo2CircuitRoleV2::AndroidKeyCert,
            OfflineCashHalo2CircuitRoleV2::GuardBundle,
            OfflineCashHalo2CircuitRoleV2::P256Signature,
        ]
    );
    assert_eq!(
        OfflineCashHalo2CircuitRoleV2::ALL.map(|role| role as u8),
        [1, 2, 3, 4, 5, 6]
    );

    let mut digests = BTreeSet::new();
    for parity in [OfflineCashHalo2ParityV2::Eq, OfflineCashHalo2ParityV2::Ep] {
        for role in OfflineCashHalo2CircuitRoleV2::ALL {
            let identity = offline_cash_halo2_protocol_source_identity_v2(parity, role);
            assert_eq!(identity.parity(), parity);
            assert_eq!(identity.role(), role);
            assert_ne!(identity.digest(), [0; 32]);
            assert!(digests.insert(identity.digest()));
        }
    }
    assert_eq!(digests.len(), 12);

    let v1 = offline_cash_halo2_protocol_identity_v1(
        OfflineCashHalo2ParityV1::Eq,
        OfflineCashHalo2CircuitRoleV1::GuardBundle,
    );
    assert_ne!(
        offline_cash_halo2_protocol_source_identity_v2(
            OfflineCashHalo2ParityV2::Eq,
            OfflineCashHalo2CircuitRoleV2::GuardBundle,
        )
        .digest(),
        v1.digest(),
    );
    assert_eq!(iroha_data_model::offline::OFFLINE_CASH_HALO2_K_V1, 16);
    assert_eq!(
        iroha_data_model::offline::OFFLINE_CASH_HISTORY_ACCUMULATOR_BYTES_V1,
        544
    );
    assert_eq!(OFFLINE_CASH_HALO2_K_V2, 17);
    assert_eq!(OFFLINE_CASH_PARENT_LINEAGE_ACCUMULATOR_BYTES_V2, 576);
}

#[test]
fn guard_bundle_abi_is_exact_and_prior_lineage_starts_at_word_192() {
    let statement = statement(true);
    let lineage = eq_lineage(501);
    let instances = OfflineCashGuardBundlePublicInstancesV2::eq(&statement, &lineage)
        .expect("exact Eq GuardBundle ABI");
    let words = instances.words();

    assert_eq!(OFFLINE_CASH_GUARD_BUNDLE_ABI_WORDS_V2, 336);
    assert_eq!(OFFLINE_CASH_GUARD_BUNDLE_INSTANCE_CELLS_V2, 48);
    assert_eq!(OFFLINE_CASH_GUARD_BUNDLE_PRIOR_LINEAGE_WORD_START_V2, 192);
    assert_eq!(OFFLINE_CASH_GUARD_BUNDLE_PRIOR_LINEAGE_WORDS_V2, 144);
    assert_eq!(
        OFFLINE_CASH_GUARD_BUNDLE_FINAL_CELL_ZERO_PADDING_WORDS_V2,
        0
    );
    assert_eq!(
        &words[..16],
        &[2, 2, 17, 1, 5, 1, 0, 1, 7, 0, 8, 0, 8, 22, 7, 48,]
    );
    assert_eq!(
        &words[OFFLINE_CASH_GUARD_BUNDLE_REGISTRATION_RECEIPT_WORD_START_V2
            ..OFFLINE_CASH_GUARD_BUNDLE_REGISTRATION_RECEIPT_WORD_START_V2 + 8],
        &[u32::from_le_bytes([0x22; 4]); 8]
    );
    let expected_lineage_words: Vec<u32> = lineage
        .encode()
        .chunks_exact(4)
        .map(|chunk| u32::from_le_bytes(chunk.try_into().expect("four-byte lineage word")))
        .collect();
    assert_eq!(
        &words[OFFLINE_CASH_GUARD_BUNDLE_PRIOR_LINEAGE_WORD_START_V2..],
        expected_lineage_words.as_slice()
    );
    assert_eq!(instances.eq_prior_lineage().expect("Eq prior"), lineage);
    assert_eq!(
        instances.ep_prior_lineage(),
        Err(OfflineCashGuardBundleProvenanceErrorV2::ParityMismatch)
    );

    let cells = instances.packed_cell_bytes();
    assert_eq!(cells.len(), 48);
    assert_eq!(
        OfflineCashGuardBundlePublicInstancesV2::unpack_cell_bytes(
            OfflineCashHalo2ParityV2::Eq,
            &cells,
        )
        .expect("canonical exact cells"),
        *words
    );
    assert_eq!(instances.field_instances::<Fp>().len(), 48);
    assert_eq!(
        OfflineCashGuardBundlePublicInstancesV2::unpack_cell_bytes(
            OfflineCashHalo2ParityV2::Eq,
            &cells[..47],
        ),
        Err(OfflineCashGuardBundleProvenanceErrorV2::NonCanonicalPacking)
    );
}

#[test]
fn optional_slots_never_reorder_and_absence_is_canonical() {
    assert_eq!(
        OFFLINE_CASH_GUARD_BUNDLE_LINEAGE_CHILD_ORDER_V2,
        [
            OfflineCashGuardBundleLineageChildRoleV2::GuardUse,
            OfflineCashGuardBundleLineageChildRoleV2::PlatformBind,
            OfflineCashGuardBundleLineageChildRoleV2::AndroidKeyCert,
            OfflineCashGuardBundleLineageChildRoleV2::P256Signature,
        ]
    );
    let present = statement(true).child_plan();
    let absent = statement(false).child_plan();
    assert_eq!(
        present.map(|slot| slot.lineage_role),
        OFFLINE_CASH_GUARD_BUNDLE_LINEAGE_CHILD_ORDER_V2
    );
    assert_eq!(
        present.map(|slot| slot.protocol_role),
        [
            OfflineCashHalo2CircuitRoleV2::GuardUse,
            OfflineCashHalo2CircuitRoleV2::PlatformBind,
            OfflineCashHalo2CircuitRoleV2::AndroidKeyCert,
            OfflineCashHalo2CircuitRoleV2::P256Signature,
        ]
    );
    assert_eq!(
        present.map(|slot| slot.presence),
        [
            OfflineCashGuardBundleChildPresenceV2::Required,
            OfflineCashGuardBundleChildPresenceV2::Required,
            OfflineCashGuardBundleChildPresenceV2::CanonicallyAbsent,
            OfflineCashGuardBundleChildPresenceV2::Present,
        ]
    );
    assert_eq!(
        absent[3].presence,
        OfflineCashGuardBundleChildPresenceV2::CanonicallyAbsent
    );
    assert!(
        assemble_present()
            .expect("present provenance")
            .has_registered_p256()
    );
    assert!(
        !assemble_absent()
            .expect("absent provenance")
            .has_registered_p256()
    );
}

#[test]
fn registered_p256_is_joined_by_every_current_helper_field_and_receipt() {
    let provenance = assemble_present().expect("exact field-for-field join");
    assert!(provenance.has_registered_p256());
    assert_eq!(
        provenance.eq_instances().parity(),
        OfflineCashHalo2ParityV2::Eq
    );
    assert_eq!(
        provenance.ep_instances().parity(),
        OfflineCashHalo2ParityV2::Ep
    );

    let mut wrong_context = statement(true);
    wrong_context.context_digest[0] ^= 1;
    let wrong_context =
        AuthenticatedOfflineCashCurrentHelperOwnerV2::from_test_statement_v2(wrong_context)
            .expect("structurally valid mismatched helper");
    assert!(matches!(
        assemble_unverified_offline_cash_guard_bundle_provenance_v2(
            wrong_context,
            OfflineCashRegisteredP256ChildProvenanceV2::Present(p256_candidates()),
            &eq_lineage(601),
            &ep_lineage(701),
        ),
        Err(OfflineCashGuardBundleProvenanceErrorV2::RegisteredP256ContextMismatch)
    ));

    let mut wrong_receipt = statement(true);
    wrong_receipt.registration_receipt_commitment[0] ^= 1;
    let wrong_receipt =
        AuthenticatedOfflineCashCurrentHelperOwnerV2::from_test_statement_v2(wrong_receipt)
            .expect("structurally valid mismatched receipt");
    assert!(matches!(
        assemble_unverified_offline_cash_guard_bundle_provenance_v2(
            wrong_receipt,
            OfflineCashRegisteredP256ChildProvenanceV2::Present(p256_candidates()),
            &eq_lineage(801),
            &ep_lineage(901),
        ),
        Err(OfflineCashGuardBundleProvenanceErrorV2::RegisteredP256ReceiptMismatch)
    ));

    assert!(matches!(
        assemble_unverified_offline_cash_guard_bundle_provenance_v2(
            helper_owner(true).expect("helper"),
            OfflineCashRegisteredP256ChildProvenanceV2::CanonicallyAbsent,
            &eq_lineage(1001),
            &ep_lineage(1101),
        ),
        Err(OfflineCashGuardBundleProvenanceErrorV2::MissingRegisteredP256Source)
    ));
    assert!(matches!(
        assemble_unverified_offline_cash_guard_bundle_provenance_v2(
            helper_owner(false).expect("helper"),
            OfflineCashRegisteredP256ChildProvenanceV2::Present(p256_candidates()),
            &eq_lineage(1201),
            &ep_lineage(1301),
        ),
        Err(OfflineCashGuardBundleProvenanceErrorV2::UnexpectedRegisteredP256Source)
    ));
}

#[test]
fn only_the_verified_move_only_handoff_can_make_state_guard_inputs() {
    let provenance = assemble_present().expect("exact provenance");
    let eq_current = CanonicalStateAccumulatorV2::decode(
        StateRecursiveFoldParityV2::Eq,
        &eq_lineage(1401).encode(),
    )
    .expect("Eq current accumulator fixture");
    let ep_current = CanonicalStateAccumulatorV2::decode(
        StateRecursiveFoldParityV2::Ep,
        &ep_lineage(1501).encode(),
    )
    .expect("Ep current accumulator fixture");
    let handoff = VerifiedOfflineCashGuardBundleStateHandoffV2::from_test_verified_parts_v2(
        provenance, eq_current, ep_current,
    )
    .expect("test-only verified handoff");
    let inputs = state_guard_inputs_from_verified_guard_bundle_v2(handoff);

    assert_eq!(
        inputs.eq_inputs().each_ref().map(|input| input.role()),
        [
            StateRecursiveFoldInputRoleV2::GuardCurrent,
            StateRecursiveFoldInputRoleV2::GuardPrior,
        ]
    );
    assert_eq!(
        inputs.ep_inputs().each_ref().map(|input| input.role()),
        [
            StateRecursiveFoldInputRoleV2::GuardCurrent,
            StateRecursiveFoldInputRoleV2::GuardPrior,
        ]
    );
    assert!(
        inputs
            .eq_inputs()
            .iter()
            .all(|input| input.accumulator().parity() == StateRecursiveFoldParityV2::Eq)
    );
    assert!(
        inputs
            .ep_inputs()
            .iter()
            .all(|input| input.accumulator().parity() == StateRecursiveFoldParityV2::Ep)
    );
    assert!(inputs.provenance_seal().provenance().has_registered_p256());
}

#[test]
fn all_activation_surfaces_remain_closed_and_source_is_private() {
    assert!(OFFLINE_CASH_GUARD_BUNDLE_PROVENANCE_CONTRACT_IMPLEMENTED_V2);
    assert!(OFFLINE_CASH_V2_PROTOCOL_SOURCE_IDENTITIES_FROZEN_V2);
    assert!(!OFFLINE_CASH_GUARD_USE_CIRCUIT_SOURCE_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_PLATFORM_BIND_CIRCUIT_SOURCE_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_ANDROID_KEY_CERT_CIRCUIT_SOURCE_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_GUARD_BUNDLE_COMPILER_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_GUARD_BUNDLE_CIRCUIT_IMPLEMENTED_V2);
    assert!(!OFFLINE_CASH_GUARD_BUNDLE_ECC_STRATEGY_GOVERNED_V2);
    assert!(!OFFLINE_CASH_GUARD_BUNDLE_ARTIFACTS_AUTHENTICATED_V2);
    assert!(!OFFLINE_CASH_GUARD_BUNDLE_BACKEND_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_GUARD_BUNDLE_WIRE_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_GUARD_BUNDLE_READINESS_AVAILABLE_V2);
    assert!(!OFFLINE_CASH_GUARD_BUNDLE_RELEASE_ELIGIBLE_V2);
    assert!(!OFFLINE_CASH_GUARD_BUNDLE_PRODUCTION_AVAILABLE_V2);
    assert_eq!(OFFLINE_CASH_GUARD_BUNDLE_PROVENANCE_WIRE_DELTA_BYTES_V2, 0);
    assert_eq!(OFFLINE_CASH_GUARD_BUNDLE_PROVENANCE_PROOF_DELTA_BYTES_V2, 0);
    assert_eq!(
        OFFLINE_CASH_GUARD_BUNDLE_PROVENANCE_ARTIFACT_DELTA_BYTES_V2,
        0
    );
    assert_eq!(OFFLINE_CASH_GUARD_BUNDLE_PROVENANCE_TRACE_ROW_DELTA_V2, 0);
    assert!(matches!(
        fail_closed_offline_cash_guard_bundle_boundary_v2(
            assemble_absent().expect("structural absent provenance")
        ),
        Err(OfflineCashGuardBundleProvenanceErrorV2::VerificationUnavailable)
    ));

    let source = include_str!("guard_bundle_provenance.rs");
    let parent = include_str!("../offline_cash_v2.rs");
    assert_eq!(
        parent
            .lines()
            .filter(|line| line.trim() == "mod guard_bundle_provenance;")
            .count(),
        1
    );
    assert!(!parent.contains("pub mod guard_bundle_provenance"));
    assert!(source.contains("enum OfflineCashCurrentHelperAuthenticationAuthorityV2 {}"));
    assert!(source.contains("enum OfflineCashCurrentHelperFreshnessAuthorityV2 {}"));
    assert!(source.contains("enum OfflineCashGuardBundleProofVerifierAuthorityV2 {}"));
    assert!(source.contains("match authority {}"));
    assert!(source.contains("CURRENT_ACCUMULATOR_IN_PUBLIC_INSTANCES_V2: bool = false"));
    assert!(!source.contains("impl Circuit for"));
    assert!(!source.contains("verify_proof("));
    assert!(!source.contains("create_proof("));
    assert!(!source.contains("VerifierIPA"));
    assert!(!source.contains("pub(crate)"));
    assert!(!source.contains("impl Clone for AuthenticatedOfflineCashCurrentHelperOwnerV2"));
    assert!(!source.contains("impl Clone for UnverifiedOfflineCashGuardBundleProvenanceV2"));
    assert!(!source.contains("OFFLINE_CASH_GUARD_BUNDLE_BACKEND_AVAILABLE_V2: bool = true"));
    assert!(!source.contains("OFFLINE_CASH_GUARD_BUNDLE_PRODUCTION_AVAILABLE_V2: bool = true"));
}
