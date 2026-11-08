//! Exact schema/provenance tests with synthetic proof/opening DATA and real P-256 receipts.
//! These tests never establish suffix proof validity; the genuine corpus owns that gate.
use super::*;
use crate::kagemusha_wallet_advance_v1::KagemushaWalletTransitionOwnerV1 as _;
use crate::kagemusha_wallet_state_v1::{
    custody::SuffixTransitionOwnerV1,
    tests::{field, fixture, signer},
};
use iroha_data_model::kagemusha::kagemusha_wallet_suffix_unload_v1::*;
use p256::ecdsa::{Signature, signature::Signer as _};

fn selected_fixture() -> ReleasedStep {
    let claim: KagemushaWalletUnloadClaimV1 = fixture("KagemushaWalletUnloadClaimV1");
    let mut statement = claim.package.statement;
    let adjusted = claim.package.lineage.lineage().unwrap().public;
    let mut successor = KagemushaWalletStateV1::bootstrap(&claim.credential, field(92)).unwrap();
    successor.core.sequence = statement.sequence;
    successor.core.lifecycle = statement.lifecycle;
    successor.core.enabled_controls = statement.enabled_controls;
    successor.core.next_load = statement.next_load;
    successor.core.balance = statement.lineage_burned_total + 100;
    successor.core.burned_total = statement.lineage_burned_total;
    successor.core.pending_outgoing_root = statement.lineage_pending_outgoing_root;
    let KagemushaWalletEffectV1::Unload { redeem_ordinal, .. } = statement.effect else {
        panic!("Unload fixture");
    };
    successor.core.next_redeem = redeem_ordinal + 1;
    statement.successor = successor.commitment().unwrap();
    let openings = vec![vec![0; 1124], vec![0; 1028]];
    let header = KagemushaWalletSuffixHeaderV1 {
        version: 1,
        method: KAGEMUSHA_WALLET_SUFFIX_METHOD_NATIVE_V1,
        network_id: [7; 32],
        scheme_id: statement.scheme_id,
        wallet_id: adjusted.wallet_id,
        verifier_definition: [8; 32],
        artifact_manifest: [9; 32],
        source_capsule: [10; 32],
        request_id: [11; 32],
        anchor_digest: [12; 32],
        anchor_sequence: statement.sequence - 4,
        step_count: 3,
        chunk_count: 2,
        stream_bytes: 300,
        chunk_digests: vec![[13; 32], [14; 32]],
        tip: statement.predecessor,
        tip_sequence: statement.sequence - 1,
        tip_adjusted: adjusted,
        statement,
        sigma: claim.package.step_proof,
        recovery_witness_digest: kagemusha_wallet_suffix_recovery_witness_digest_v1(&openings)
            .unwrap(),
        account: claim.account,
        charge: claim.charge,
    };
    let header_original = header.to_canonical_bytes().unwrap();
    let authority = kagemusha_wallet_suffix_authority_digest_v1(&header_original).unwrap();
    let body = SuffixRecoveryBodyV1 {
        version: 1,
        discriminator: 10,
        scheme_id: header.scheme_id,
        wallet_id: header.wallet_id,
        operation_id: statement.operation_id(&header.wallet_id).unwrap(),
        predecessor_capsule_digest: header.source_capsule,
        successor_state: successor,
        statement,
        step_proof: header.sigma,
        authority_header: header_original,
        authority_digest: authority,
        pinned_sources_root: [16; 32],
        map_openings: openings,
        output: KagemushaWalletOutputDescriptorV1::for_transition(&statement, &authority, &[0; 32])
            .unwrap(),
    };
    let frozen = SuffixFrozenTransitionV1 {
        credential: claim.credential,
        certificates: archive::encode(&claim.certificates).unwrap(),
        capsule: body.freeze().unwrap().seal(field(17)).unwrap(),
    };
    let owner = SuffixTransitionOwnerV1::new(frozen.credential, claim.certificates);
    let receipt_body = owner
        .receipt_body(&frozen.capsule, &frozen.capsule.custody_digest())
        .unwrap();
    let message =
        kagemusha_wallet_signing_message_v1(KagemushaWalletSigningDomainV1::Receipt, &receipt_body);
    let signature: Signature = signer(&frozen.credential).sign(&message);
    let signature = signature.normalize_s();
    let signature = KagemushaDeviceSignatureV1::from_raw_bytes(&signature.to_bytes()).unwrap();
    let record = owner
        .assemble(
            &frozen.capsule,
            &frozen.capsule.custody_digest(),
            &signature,
        )
        .unwrap();
    let frame = record.to_canonical_bytes().unwrap();
    ReleasedStep::selected_suffix(
        frozen,
        Retained {
            operation_id: record.operation_id,
            capsule_digest: record.capsule_digest,
            selected_generation: 7,
            completion_digest: SuffixCompletionRecordV1::frame_digest(&frame),
            frame,
            record,
        },
    )
    .unwrap()
}

#[test]
fn suffix_frozen_schema_roundtrips_exact_original_and_refuses_other_scheme_or_trailing_bytes() {
    let step = selected_fixture();
    let (frozen, _) = step.suffix().unwrap();
    let original = frozen.to_canonical_bytes().unwrap();
    let reopened =
        SuffixFrozenTransitionV1::decode_canonical(&original, &frozen.credential.body.scheme_id)
            .unwrap();
    assert_eq!(&reopened, frozen);
    assert_eq!(reopened.to_canonical_bytes().unwrap(), original);
    assert_ne!(
        SuffixFrozenTransitionV1::frame_schema(),
        norito::schema::identity::frame_hash::<FrozenTransition>()
    );
    assert!(SuffixFrozenTransitionV1::decode_canonical(&original, &[93; 32]).is_err());
    let mut trailing = original;
    trailing.push(0);
    assert!(
        SuffixFrozenTransitionV1::decode_canonical(&trailing, &frozen.credential.body.scheme_id)
            .is_err()
    );
    assert!(step.ordinary_frozen().is_err());
    assert!(step.ordinary_capsule().is_err());
    assert_eq!(
        step.capsule().proof_digest().unwrap(),
        frozen.capsule.body().authority_digest
    );
    assert_eq!(
        step.receipt().capsule_commitment,
        frozen.capsule.receipt_commitment()
    );
}

#[test]
fn selected_suffix_rejects_altered_generation_original_and_valid_alternate_capsule() {
    let original = selected_fixture();
    original.verify_completion().unwrap();
    for mutation in 0..4 {
        let mut changed = original.clone();
        let ReleasedStep::Suffix {
            frozen, retained, ..
        } = &mut changed
        else {
            unreachable!()
        };
        match mutation {
            0 => retained.selected_generation += 1,
            1 => retained.frame.push(0),
            2 => {
                frozen.capsule = frozen
                    .capsule
                    .body()
                    .clone()
                    .freeze()
                    .unwrap()
                    .seal(field(18))
                    .unwrap();
                frozen.validate().unwrap();
            }
            3 => frozen.certificates.push(0),
            _ => unreachable!(),
        }
        assert!(
            changed.verify_completion().is_err(),
            "accepted mutation {mutation}"
        );
        original.verify_completion().unwrap();
    }
}

#[test]
fn cloned_selection_cannot_authorize_another_valid_receipt_for_the_same_statement() {
    use p256::ecdsa::signature::RandomizedSigner as _;
    let original = selected_fixture();
    let mut changed = original.clone();
    let ReleasedStep::Suffix {
        frozen, retained, ..
    } = &mut changed
    else {
        unreachable!()
    };
    let owner = SuffixTransitionOwnerV1::new(
        frozen.credential,
        archive::decode(frozen.certificates()).unwrap(),
    );
    let receipt_body = owner
        .receipt_body(&frozen.capsule, &frozen.capsule.custody_digest())
        .unwrap();
    let message =
        kagemusha_wallet_signing_message_v1(KagemushaWalletSigningDomainV1::Receipt, &receipt_body);
    let signature: Signature = signer(&frozen.credential)
        .try_sign_with_rng(&mut rand::rngs::SysRng, &message)
        .unwrap();
    let signature = signature.normalize_s();
    let signature = KagemushaDeviceSignatureV1::from_raw_bytes(&signature.to_bytes()).unwrap();
    retained.record = owner
        .assemble(
            &frozen.capsule,
            &frozen.capsule.custody_digest(),
            &signature,
        )
        .unwrap();
    retained
        .record
        .verify(&frozen.credential, &frozen.capsule)
        .unwrap();
    retained.frame = retained.record.to_canonical_bytes().unwrap();
    retained.completion_digest = SuffixCompletionRecordV1::frame_digest(&retained.frame);
    assert_ne!(changed.completion_frame(), original.completion_frame());
    assert!(matches!(
        changed.verify_completion(),
        Err(Error::WitnessLost("suffix selected release provenance"))
    ));
}

#[test]
fn enclosing_decode_refusal_is_retryable_without_changing_selected_originals() {
    let step = selected_fixture();
    let before = step.completion_frame().to_vec();
    let refused = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, 0, usize::MAX, usize::MAX, 64),
        || step.verify_completion(),
    );
    assert!(matches!(refused, Err(Error::DecodeResource(_))));
    assert_eq!(step.completion_frame(), before);
    step.verify_completion().unwrap();
}

#[test]
fn suffix_owner_preserves_actual_decode_refusal_before_signing_and_completion_assembly() {
    let step = selected_fixture();
    let (frozen, retained) = step.suffix().unwrap();
    let owner = SuffixTransitionOwnerV1::new(
        frozen.credential,
        archive::decode(frozen.certificates()).unwrap(),
    );
    let original = frozen.capsule.to_canonical_bytes();
    let refused = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, 0, usize::MAX, usize::MAX, 64),
        || owner.receipt_body(&frozen.capsule, &frozen.capsule.custody_digest()),
    );
    assert!(matches!(refused, Err(ProviderError::DecodeResource(_))));
    owner
        .receipt_body(&frozen.capsule, &frozen.capsule.custody_digest())
        .unwrap();
    let refused = norito::with_decode_limits_scope(
        norito::DecodeLimits::new(usize::MAX, 0, usize::MAX, usize::MAX, 64),
        || owner.require_completion(&frozen.capsule, &retained.record),
    );
    assert!(matches!(refused, Err(ProviderError::DecodeResource(_))));
    assert_eq!(frozen.capsule.to_canonical_bytes(), original);
    owner
        .require_completion(&frozen.capsule, &retained.record)
        .unwrap();
}
