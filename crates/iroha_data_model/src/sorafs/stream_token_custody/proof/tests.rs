//! Genuine finality signatures over explicitly synthetic, admission-free custody World preimages.

use super::*;
use crate::sumeragi_finality::{
    WorldStateElementKindV1, WorldStateSnapshotEntryV1, test_fixtures::NativeFinalityFixture,
    world_state_value_hash_v1,
};
use iroha_crypto::{Algorithm, KeyPair};
use sorafs_manifest::signer::{
    custody::SignerCustodyAuthorityV1, custody_control::SignerCustodyPolicyV1,
    protocol::SignerKeyAlgorithmV1,
};

fn key(seed: u8) -> KeyPair {
    KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519)
}
fn row<K: norito::codec::Encode, V: norito::codec::Encode>(
    field: &str,
    key: &K,
    value: &V,
) -> WorldStateSnapshotEntryV1 {
    WorldStateSnapshotEntryV1 {
        field_id: field.into(),
        kind: WorldStateElementKindV1::Table,
        key_hash: Some(world_state_value_hash_v1(key).unwrap()),
        value_hash: world_state_value_hash_v1(value).unwrap(),
    }
}
fn rebuild(proof: &mut StreamTokenCustodyProofV1, provider: ProviderId) {
    let mut entries = vec![row("world.provider_owners", &provider, &proof.owner)];
    if let Some(current) = &proof.current {
        let index: StreamTokenCustodyControlIndexV1 =
            norito::decode_canonical(&current.head).unwrap();
        entries.push(row(
            "world.smart_contract_state",
            &head_key(provider),
            &current.head,
        ));
        entries.push(row(
            "world.smart_contract_state",
            &record_key(provider, index.revision),
            &current.record,
        ));
        entries.push(row(
            "world.smart_contract_state",
            &height_key(provider, index.height, index.ordinal),
            &current.head,
        ));
    }
    entries
        .sort_by(|a, b| (&a.field_id, a.kind, a.key_hash).cmp(&(&b.field_id, b.kind, b.key_hash)));
    proof.world.entries = entries;
}
fn certify(
    native: &mut NativeFinalityFixture,
    proof: &StreamTokenCustodyProofV1,
) -> VerifiedSumeragiBlock {
    let mut header = native.next_header();
    header.creation_time_ms = header.creation_time_ms.max(2_000);
    let block = native.block_with_submitted_work(header);
    let certificate = native.certify_with_world_root(block, proof.world.root().unwrap());
    native
        .verifier()
        .verify_retained_decision(&certificate)
        .unwrap()
}
fn fixture(
    present: bool,
) -> (
    NativeFinalityFixture,
    StreamTokenCustodyProofV1,
    ProviderId,
    SignerCustodyBindingV1,
    VerifiedSumeragiBlock,
) {
    let mut native = NativeFinalityFixture::start("native-custody-synthetic-world");
    let provider = ProviderId::new([0x36; 32]);
    let owner = AccountId::new(key(1).public_key().clone());
    let binding = SignerCustodyBindingV1 {
        chain_id: native.chain_id().into(),
        network_id: *native.network_id().as_bytes(),
        runtime_handle: "software://stream/runtime".into(),
        key_handle: "software://stream/key".into(),
        service_id: "token-service".into(),
        administrator_id: "token-admin".into(),
        role: SignerRoleV1::StreamToken,
        purpose: SignerPurposeBindingV1::StreamToken {
            provider_id: *provider.as_bytes(),
        },
        algorithm: SignerKeyAlgorithmV1::Ed25519,
        public_key: key(2).public_key().clone(),
        key_revision: 1,
        policy_revision: 1,
        policy_digest: [4; 32],
    };
    let control = SignerCustodyControlStateV1 {
        policy: SignerCustodyPolicyV1 {
            binding: binding.clone(),
            attester_authority: SignerCustodyAuthorityV1 {
                service_id: "custody-service".into(),
                administrator_id: "custody-admin".into(),
                key_revision: 1,
                policy_revision: 1,
                policy_digest: [5; 32],
            },
            attester_public_key: key(3).public_key().clone(),
            active_from_unix_ms: 1,
            active_until_unix_ms: 10_000,
            max_validity_ms: 3_000,
            max_anchor_age_ms: 1_000,
        },
        next_sequence: 1,
        predecessor_digest: [0; 32],
        active_head: None,
        signer_revoked: false,
        attester_revoked: false,
    };
    control.validate().unwrap();
    let record = StreamTokenCustodyControlRecordV1 {
        provider_id: provider,
        revision: 1,
        predecessor_digest: [0; 32],
        request_digest: [6; 32],
        execution_height: 2,
        ordinal: 0,
        recorded_at_unix_ms: 2_000,
        authority: owner.clone(),
        control_state: norito::encode_canonical(&control).unwrap(),
        active_enrollment: None,
    };
    let index = StreamTokenCustodyControlIndexV1 {
        revision: 1,
        digest: record.canonical_digest().unwrap(),
        height: 2,
        ordinal: 0,
    };
    let mut proof = StreamTokenCustodyProofV1 {
        world: WorldStateSnapshotV1 {
            schema_hash: Hash::new(b"independent native schema specimen"),
            entries: Vec::new(),
        },
        owner,
        current: present.then(|| StreamTokenCustodyRecordProofV1 {
            head: norito::encode_canonical(&index).unwrap(),
            record: norito::encode_canonical(&record).unwrap(),
        }),
    };
    rebuild(&mut proof, provider);
    let block = certify(&mut native, &proof);
    (native, proof, provider, binding, block)
}
fn verify(
    proof: &StreamTokenCustodyProofV1,
    provider: ProviderId,
    binding: &SignerCustodyBindingV1,
    block: &VerifiedSumeragiBlock,
) -> Result<VerifiedStreamTokenCustodyStateV1, FinalityError> {
    // The fixture selects this schema/owner before any response mutation.
    proof.verify(
        block.commitment().schedule.current.network_id,
        provider,
        &AccountId::new(key(1).public_key().clone()),
        binding,
        Hash::new(b"independent native schema specimen"),
        block,
    )
}

#[test]
fn absence_and_exact_unenrolled_record_authenticate_without_provider_admission() {
    for present in [false, true] {
        let (_, proof, provider, binding, block) = fixture(present);
        let verified = verify(&proof, provider, &binding, &block).unwrap();
        assert_eq!(
            verified.network_id(),
            block.commitment().schedule.current.network_id
        );
        assert_eq!(verified.provider_id(), provider);
        assert_eq!(verified.owner(), &proof.owner);
        assert_eq!(verified.height(), block.height());
        assert_eq!(verified.context_id(), block.context_id());
        assert_eq!(verified.current().is_some(), present);
        if let Some(current) = verified.current() {
            let original: StreamTokenCustodyControlRecordV1 =
                norito::decode_canonical(&proof.current.as_ref().unwrap().record).unwrap();
            assert_eq!(current.record(), &original);
            assert_eq!(current.control().policy.binding, binding);
            assert!(current.control().active_head.is_none());
            assert_eq!(
                current.anchor().state_digest,
                original.canonical_digest().unwrap()
            );
            assert_eq!(current.anchor().block_hash, *block.header().hash().as_ref());
            assert_eq!(current.anchor().height, block.height());
        }
        let borrowed = StreamTokenCustodyProofRefV1::new(
            &proof.world,
            &proof.owner,
            proof.current.as_ref().map(|p| (&p.head, &p.record)),
        );
        let encoded = norito::encode_canonical(&proof).unwrap();
        assert_eq!(norito::encode_canonical(&borrowed).unwrap(), encoded);
        assert_eq!(
            StreamTokenCustodyProofV1::decode_frame(&encoded).unwrap(),
            proof
        );
        assert_eq!(
            norito::json::to_json(&borrowed).unwrap(),
            norito::json::to_json(&proof).unwrap()
        );
        assert_eq!(
            norito::json::from_json::<StreamTokenCustodyProofV1>(
                &norito::json::to_json(&proof).unwrap()
            )
            .unwrap(),
            proof
        );
    }
}

#[test]
fn absent_response_cannot_hide_either_native_head_or_first_record() {
    for keep_head in [false, true] {
        let (mut native, mut proof, provider, binding, _) = fixture(true);
        let key = if keep_head {
            head_key(provider)
        } else {
            record_key(provider, 1)
        };
        let hash = world_state_value_hash_v1(&key).unwrap();
        proof.world.entries.retain(|entry| {
            entry.field_id != "world.smart_contract_state" || entry.key_hash == Some(hash)
        });
        proof.current = None;
        let block = certify(&mut native, &proof);
        assert!(verify(&proof, provider, &binding, &block).is_err());
    }
}

#[test]
fn current_and_absent_projections_reject_changed_independent_trust_inputs() {
    for present in [false, true] {
        let (_, proof, provider, binding, block) = fixture(present);
        let mut altered = proof.clone();
        altered.owner = AccountId::new(key(9).public_key().clone());
        assert!(verify(&altered, provider, &binding, &block).is_err());
        let mut altered = proof.clone();
        altered.world.schema_hash = Hash::new(b"foreign schema");
        assert!(verify(&altered, provider, &binding, &block).is_err());
        let mut changed = binding.clone();
        changed.network_id = [9; 32];
        assert!(verify(&proof, provider, &changed, &block).is_err());
        let mut changed = binding.clone();
        changed.purpose = SignerPurposeBindingV1::StreamToken {
            provider_id: [8; 32],
        };
        assert!(verify(&proof, provider, &changed, &block).is_err());
        assert!(verify(&proof, ProviderId::new([8; 32]), &binding, &block).is_err());
        let mut altered = proof.clone();
        altered.world.entries.clear();
        assert!(verify(&altered, provider, &binding, &block).is_err());
        if present {
            let mut changed = binding.clone();
            changed.key_handle.push_str("-substituted");
            assert!(verify(&proof, provider, &changed, &block).is_err());
            let mut changed = binding.clone();
            changed.public_key = key(9).public_key().clone();
            assert!(verify(&proof, provider, &changed, &block).is_err());
            let mut altered = proof.clone();
            altered.current.as_mut().unwrap().record[0] ^= 1;
            assert!(verify(&altered, provider, &binding, &block).is_err());
            let mut altered = proof.clone();
            altered.current = None;
            assert!(verify(&altered, provider, &binding, &block).is_err());
        }
    }
}

#[test]
fn projection_rejects_noncanonical_or_unbounded_frames_and_nested_records() {
    let (_, mut proof, provider, binding, block) = fixture(true);
    let mut frame = norito::encode_canonical(&proof).unwrap();
    frame.push(0);
    assert!(StreamTokenCustodyProofV1::decode_frame(&frame).is_err());
    assert!(
        StreamTokenCustodyProofV1::decode_frame(&vec![
            0;
            MAX_STREAM_TOKEN_CUSTODY_PROOF_BYTES_V1 + 1
        ])
        .is_err()
    );
    proof.current.as_mut().unwrap().record = vec![0; STREAM_TOKEN_CUSTODY_MAX_RECORD_BYTES_V1 + 1];
    assert!(verify(&proof, provider, &binding, &block).is_err());
}

#[test]
fn absent_custody_rejects_private_roots_and_wrong_chain_with_genuine_certificates() {
    let (native, proof, provider, binding, block) = fixture(false);
    let mut wrong_chain = binding.clone();
    wrong_chain.chain_id = "different-label".into();
    assert!(verify(&proof, provider, &wrong_chain, &block).is_err());
    let mut private = NativeFinalityFixture::start_with_scope(
        &binding.chain_id,
        crate::block::consensus::SumeragiRootScope::Dataspace {
            parent_network_id: native.network_id(),
            dataspace_id: iroha_model_base::topology::DataSpaceId::new(u64::MAX),
        },
    );
    let mut private_binding = binding;
    private_binding.network_id = *private.network_id().as_bytes();
    let private_block = certify(&mut private, &proof);
    assert!(verify(&proof, provider, &private_binding, &private_block).is_err());
}
