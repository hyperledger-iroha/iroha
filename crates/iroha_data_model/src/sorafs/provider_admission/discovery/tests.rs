//! Genuine finality certificates over explicitly synthetic provider World preimages.

use super::*;
mod account_read_tests;
mod stream_token_control_tests;
use crate::{
    sorafs::provider_admission::history::{GenesisAdmissionOriginV1, encode_history_value},
    sumeragi_finality::{
        WorldStateElementKindV1, WorldStateSnapshotEntryV1, test_fixtures::NativeFinalityFixture,
        world_state_value_hash_v1,
    },
};
use iroha_crypto::{Algorithm, KeyPair, PrivateKey, Signature};
use sorafs_manifest::{
    CouncilSignature,
    provider_admission::{
        compute_advert_body_digest, compute_envelope_authorization_digest, compute_proposal_digest,
    },
};

fn key(seed: u8) -> KeyPair {
    KeyPair::from_private_key(PrivateKey::from_bytes(Algorithm::Ed25519, &[seed; 32]).unwrap())
        .unwrap()
}
fn raw(key: &KeyPair) -> [u8; 32] {
    key.public_key().to_bytes().1.try_into().unwrap()
}
fn sign(envelope: &mut ProviderAdmissionEnvelopeV1, policy: &ProviderAdmissionCouncilPolicyV1) {
    envelope.network_id = policy.network_id;
    envelope.policy_id = policy.policy_id;
    envelope.policy_revision = policy.revision;
    envelope.policy_digest = policy.canonical_digest().unwrap();
    envelope.proposal_digest = compute_proposal_digest(&envelope.proposal).unwrap();
    envelope.advert_body_digest = compute_advert_body_digest(&envelope.advert_body).unwrap();
    envelope.council_signatures.clear();
    let signer = key(0x44);
    let digest = compute_envelope_authorization_digest(envelope).unwrap();
    envelope.council_signatures.push(CouncilSignature {
        signer: raw(&signer),
        signature: Signature::new(signer.private_key(), &digest)
            .payload()
            .to_vec(),
    });
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
fn rebuild(proof: &mut ProviderDiscoveryProofV1, provider: ProviderId) {
    let mut entries = vec![row("world.provider_owners", &provider, &proof.owner)];
    for (subject, original) in [(None, &proof.council), (Some(provider), &proof.provider)] {
        let record = AdmissionHistoryRecordV1::decode_frame(&original.head).unwrap();
        entries.push(row(
            "world.smart_contract_state",
            &admission_history_path(subject, AdmissionHistoryPathV1::Head),
            &original.head,
        ));
        entries.push(row(
            "world.smart_contract_state",
            &admission_history_path(subject, AdmissionHistoryPathV1::Revision(record.revision)),
            &original.head,
        ));
        if let Some(previous) = &original.predecessor {
            entries.push(row(
                "world.smart_contract_state",
                &admission_history_path(
                    subject,
                    AdmissionHistoryPathV1::Revision(record.revision - 1),
                ),
                previous,
            ));
        }
    }
    if let Some(token) = &proof.stream_token {
        use crate::sorafs::stream_token_custody::history::{
            StreamTokenCustodyControlIndexV1, head_key, height_key, record_key,
        };
        let index: StreamTokenCustodyControlIndexV1 =
            norito::decode_canonical(&token.head).unwrap();
        entries.push(row(
            "world.smart_contract_state",
            &head_key(provider),
            &token.head,
        ));
        entries.push(row(
            "world.smart_contract_state",
            &record_key(provider, index.revision),
            &token.record,
        ));
        entries.push(row(
            "world.smart_contract_state",
            &height_key(provider, index.height, index.ordinal),
            &token.head,
        ));
    }
    entries
        .sort_by(|a, b| (&a.field_id, a.kind, a.key_hash).cmp(&(&b.field_id, b.kind, b.key_hash)));
    proof.world.entries = entries;
}
fn certify(
    native: &mut NativeFinalityFixture,
    proof: &ProviderDiscoveryProofV1,
) -> VerifiedSumeragiBlock {
    let mut header = native.next_header();
    header.creation_time_ms = header.creation_time_ms.max(2_000);
    let block = native.block_with_submitted_work(header);
    let cert = native.certify_with_world_root(block, proof.world.root().unwrap());
    native.verifier().verify_retained_decision(&cert).unwrap()
}
fn fixture(
    genesis: bool,
) -> (
    NativeFinalityFixture,
    ProviderDiscoveryProofV1,
    ProviderId,
    VerifiedSumeragiBlock,
) {
    let mut native = NativeFinalityFixture::start("provider-discovery-synthetic-world");
    let policy = ProviderAdmissionCouncilPolicyV1 {
        network_id: *native.network_id().as_bytes(),
        policy_id: [0xC1; 32],
        version: 1,
        revision: 1,
        predecessor_policy_digest: None,
        trusted_signers: vec![raw(&key(0x44))],
        signature_threshold: 1,
        paused: false,
    };
    let base: ProviderAdmissionEnvelopeV1 = norito::decode_from_bytes(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sorafs_manifest/provider_admission/envelope_v1.to"
    )))
    .unwrap();
    let mut envelope = ProviderAdmissionGenesisMaterialV1 {
        proposal: base.proposal,
        advert_body: base.advert_body,
        issued_at: 0,
        retention_epoch: 3600,
    }
    .project(
        policy.network_id,
        policy.policy_id,
        policy.canonical_digest().unwrap(),
    )
    .unwrap();
    if !genesis {
        sign(&mut envelope, &policy);
    }
    let provider = ProviderId::new(envelope.proposal.provider_id);
    let owner = AccountId::new(key(0x45).public_key().clone());
    let origin = genesis.then_some(GenesisAdmissionOriginV1 {
        entrypoint_index: 0,
        instruction_digest: [7; 32],
    });
    let head = |material, owner| AdmissionHistoryRecordV1 {
        network_id: policy.network_id,
        genesis_origin: origin,
        revision: 1,
        predecessor: None,
        height: if genesis { 1 } else { 2 },
        recorded_at_unix_ms: if genesis { 1 } else { 2_000 },
        owner,
        revoked: false,
        material,
    };
    let council = ProviderAdmissionHeadProofV1 {
        head: encode_history_value(&head(norito::encode_canonical(&policy).unwrap(), None))
            .unwrap(),
        predecessor: None,
    };
    let provider_head = ProviderAdmissionHeadProofV1 {
        head: encode_history_value(&head(
            norito::encode_canonical(&envelope).unwrap(),
            Some(owner.clone()),
        ))
        .unwrap(),
        predecessor: None,
    };
    let mut advert: ProviderAdvertV1 = norito::decode_from_bytes(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/sorafs_manifest/provider_admission/advert_v1.to"
    )))
    .unwrap();
    advert.network_id = policy.network_id;
    advert.body = envelope.advert_body;
    advert.issued_at = 0;
    advert.expires_at = 60;
    advert.signature_strict = true;
    advert.signature.public_key = raw(&key(0x21)).to_vec();
    advert.signature.signature = Signature::new(
        key(0x21).private_key(),
        &advert.signature_payload_bytes().unwrap(),
    )
    .payload()
    .to_vec();
    let mut proof = ProviderDiscoveryProofV1 {
        world: WorldStateSnapshotV1 {
            schema_hash: Hash::new(b"synthetic native schema"),
            entries: vec![],
        },
        council,
        provider: provider_head,
        owner,
        advert: norito::encode_canonical(&advert).unwrap(),
        stream_token: None,
    };
    rebuild(&mut proof, provider);
    let block = certify(&mut native, &proof);
    (native, proof, provider, block)
}
fn verify(
    proof: &ProviderDiscoveryProofV1,
    provider: ProviderId,
    block: &VerifiedSumeragiBlock,
) -> Result<VerifiedProviderDiscoveryV1, FinalityError> {
    proof.verify(
        block.commitment().schedule.current.network_id,
        provider,
        proof.world.schema_hash,
        block,
        2,
    )
}

#[test]
fn current_council_and_genesis_heads_verify_and_roundtrip_without_response_selected_trust() {
    for genesis in [false, true] {
        let (native, proof, provider, block) = fixture(genesis);
        let verified = verify(&proof, provider, &block).unwrap();
        let borrowed = ProviderDiscoveryProofRefV1::new(
            &proof.world,
            &proof.council.head,
            proof.council.predecessor.as_ref(),
            &proof.provider.head,
            proof.provider.predecessor.as_ref(),
            &proof.owner,
            &proof.advert,
            proof.stream_token.as_ref().map(|p| (&p.head, &p.record)),
        );
        assert_eq!(
            norito::encode_canonical(&borrowed).unwrap(),
            norito::encode_canonical(&proof).unwrap()
        );
        assert_eq!(
            norito::json::to_json(&borrowed).unwrap(),
            norito::json::to_json(&proof).unwrap()
        );
        assert_eq!(verified.network_id(), native.network_id());
        assert_eq!(verified.height(), block.height());
        assert_eq!(verified.context_id(), block.context_id());
        assert_eq!(verified.owner(), &proof.owner);
        assert_eq!(verified.admission().provider_id(), provider.as_bytes());
        verified.advert().verify_signature().unwrap();
        assert_eq!(verified.admission().is_genesis_material(), genesis);
        assert_eq!(
            ProviderDiscoveryProofV1::decode_frame(&norito::encode_canonical(&proof).unwrap())
                .unwrap(),
            proof
        );
        assert_eq!(
            norito::json::from_json::<ProviderDiscoveryProofV1>(
                &norito::json::to_json(&proof).unwrap()
            )
            .unwrap(),
            proof
        );
        let record = AdmissionHistoryRecordV1::decode_frame(&proof.provider.head).unwrap();
        assert_eq!(
            norito::json::from_json::<AdmissionHistoryRecordV1>(
                &norito::json::to_json(&record).unwrap()
            )
            .unwrap(),
            record
        );
    }
}

#[test]
fn authenticated_cut_rejects_wrong_network_schema_provider_and_changed_preimages() {
    let (_, proof, provider, block) = fixture(false);
    let other = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
        Hash::new(b"independently selected foreign genesis"),
    ));
    assert!(
        proof
            .verify(other, provider, proof.world.schema_hash, &block, 2)
            .is_err()
    );
    assert!(
        proof
            .verify(
                block.commitment().schedule.current.network_id,
                provider,
                Hash::new(b"foreign schema"),
                &block,
                2
            )
            .is_err()
    );
    assert!(verify(&proof, ProviderId::new([9; 32]), &block).is_err());
    for mutation in 0..5 {
        let mut changed = proof.clone();
        match mutation {
            0 => changed.owner = AccountId::new(key(9).public_key().clone()),
            1 => changed.provider.head[60] ^= 1,
            2 => changed.council.head[60] ^= 1,
            3 => changed.world.entries.pop().map(|_| ()).unwrap(),
            _ => changed.advert[80] ^= 1,
        }
        assert!(
            verify(&changed, provider, &block).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn genuinely_certified_revocation_pause_owner_change_and_expiry_still_refuse_discovery() {
    for mutation in 0..4 {
        let (mut native, mut proof, provider, _) = fixture(false);
        match mutation {
            0 => {
                let mut head =
                    AdmissionHistoryRecordV1::decode_frame(&proof.provider.head).unwrap();
                head.revoked = true;
                proof.provider.head = encode_history_value(&head).unwrap();
            }
            1 => {
                let mut head = AdmissionHistoryRecordV1::decode_frame(&proof.council.head).unwrap();
                let mut policy: ProviderAdmissionCouncilPolicyV1 =
                    decode_frame(&head.material).unwrap();
                policy.paused = true;
                head.material = norito::encode_canonical(&policy).unwrap();
                proof.council.head = encode_history_value(&head).unwrap();
            }
            2 => proof.owner = AccountId::new(key(9).public_key().clone()),
            _ => {}
        }
        rebuild(&mut proof, provider);
        let block = certify(&mut native, &proof);
        let now = if mutation == 3 { 3600 } else { 2 };
        assert!(
            proof
                .verify(
                    native.network_id(),
                    provider,
                    proof.world.schema_hash,
                    &block,
                    now
                )
                .is_err()
        );
    }
}

#[test]
fn missing_predecessor_and_certified_future_history_cannot_be_presented_as_current_head() {
    let (mut native, mut proof, provider, _) = fixture(false);
    let mut head = AdmissionHistoryRecordV1::decode_frame(&proof.provider.head).unwrap();
    head.revision = 2;
    head.predecessor = Some(head.canonical_digest().unwrap());
    proof.provider.head = encode_history_value(&head).unwrap();
    rebuild(&mut proof, provider);
    let block = certify(&mut native, &proof);
    assert!(verify(&proof, provider, &block).is_err());
    let (mut native, mut proof, provider, _) = fixture(false);
    proof.world.entries.push(row(
        "world.smart_contract_state",
        &admission_history_path(Some(provider), AdmissionHistoryPathV1::Revision(2)),
        &vec![1_u8],
    ));
    proof
        .world
        .entries
        .sort_by(|a, b| (&a.field_id, a.kind, a.key_hash).cmp(&(&b.field_id, b.kind, b.key_hash)));
    let block = certify(&mut native, &proof);
    assert!(verify(&proof, provider, &block).is_err());
}

#[test]
fn exact_native_renewal_predecessor_and_borrowed_optional_frame_are_authenticated() {
    let (mut native, mut proof, provider, _) = fixture(false);
    let previous = AdmissionHistoryRecordV1::decode_frame(&proof.provider.head).unwrap();
    let previous_envelope: ProviderAdmissionEnvelopeV1 = decode_frame(&previous.material).unwrap();
    let council = AdmissionHistoryRecordV1::decode_frame(&proof.council.head).unwrap();
    let policy: ProviderAdmissionCouncilPolicyV1 = decode_frame(&council.material).unwrap();
    let mut envelope = previous_envelope.clone();
    envelope.admission_revision = 2;
    envelope.expected_current_event_digest =
        Some(compute_envelope_digest(&previous_envelope).unwrap());
    sign(&mut envelope, &policy);
    let mut next = previous.clone();
    next.revision = 2;
    next.height = 3;
    next.predecessor = Some(previous.canonical_digest().unwrap());
    next.material = norito::encode_canonical(&envelope).unwrap();
    proof.provider.predecessor = Some(std::mem::replace(
        &mut proof.provider.head,
        encode_history_value(&next).unwrap(),
    ));
    rebuild(&mut proof, provider);
    let block = certify(&mut native, &proof);
    assert_eq!(
        verify(&proof, provider, &block)
            .unwrap()
            .admission()
            .envelope()
            .admission_revision,
        2
    );
    let borrowed = ProviderDiscoveryProofRefV1::new(
        &proof.world,
        &proof.council.head,
        proof.council.predecessor.as_ref(),
        &proof.provider.head,
        proof.provider.predecessor.as_ref(),
        &proof.owner,
        &proof.advert,
        proof.stream_token.as_ref().map(|p| (&p.head, &p.record)),
    );
    assert_eq!(
        norito::encode_canonical(&borrowed).unwrap(),
        norito::encode_canonical(&proof).unwrap()
    );
    let mut changed = proof.clone();
    changed.provider.predecessor.as_mut().unwrap()[70] ^= 1;
    assert!(verify(&changed, provider, &block).is_err());
}

#[test]
fn history_paths_and_finite_encoder_preserve_native_ownership_and_full_width_revisions() {
    let provider = ProviderId::new([0xab; 32]);
    assert_eq!(
        admission_history_path(Some(provider), AdmissionHistoryPathV1::Revision(u64::MAX))
            .to_string(),
        format!(
            "sorafs/provider_admission/{}/history/{}",
            hex::encode(provider.as_bytes()),
            u64::MAX
        )
    );
    let suffixes = [
        AdmissionHistoryPathV1::Head,
        AdmissionHistoryPathV1::Revision(1),
        AdmissionHistoryPathV1::ProviderCount,
        AdmissionHistoryPathV1::HistoryBytes,
        AdmissionHistoryPathV1::RevocationBytes,
    ];
    let paths: std::collections::BTreeSet<_> = suffixes
        .into_iter()
        .map(|suffix| admission_history_path(None, suffix))
        .collect();
    assert_eq!(paths.len(), 5);
    assert!(
        encode_history_value(&vec![
            0_u8;
            super::super::history::MAX_ADMISSION_HISTORY_BYTES_V1
        ])
        .is_err()
    );
}

#[test]
fn canonical_readers_reject_trailing_frames_and_oversized_inputs() {
    let (_, proof, _, _) = fixture(false);
    let mut wire = norito::encode_canonical(&proof).unwrap();
    wire.push(0);
    assert!(ProviderDiscoveryProofV1::decode_frame(&wire).is_err());
    assert!(
        ProviderDiscoveryProofV1::decode_frame(&vec![0; MAX_PROVIDER_DISCOVERY_BYTES_V1 + 1])
            .is_err()
    );
    assert!(
        AdmissionHistoryRecordV1::decode_frame(&vec![
            0;
            super::super::history::MAX_ADMISSION_HISTORY_BYTES_V1
                + 1
        ])
        .is_err()
    );
}
