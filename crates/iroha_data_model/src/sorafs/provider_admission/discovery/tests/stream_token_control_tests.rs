//! Authenticated control reads over explicitly synthetic native World preimages and real signatures.

use super::{
    account_read_tests::{account_fixture, install},
    *,
};
use crate::sorafs::stream_token_custody::{
    STREAM_TOKEN_CUSTODY_MAX_RECORD_BYTES_V1, STREAM_TOKEN_CUSTODY_MAX_REVISIONS_V1,
    StreamTokenCustodyControlRecordV1,
    history::{StreamTokenCustodyControlIndexV1, head_key, height_key, record_key},
};
use sorafs_manifest::signer::{
    custody::SignerCustodyBindingV1, custody_control::SignerCustodyControlStateV1,
    protocol::SignerPurposeBindingV1,
};

fn control_record(
    proof: &ProviderDiscoveryProofV1,
) -> (
    StreamTokenCustodyControlRecordV1,
    SignerCustodyControlStateV1,
) {
    let record: StreamTokenCustodyControlRecordV1 =
        norito::decode_canonical(&proof.stream_token.as_ref().unwrap().record).unwrap();
    let control = norito::decode_canonical(&record.control_state).unwrap();
    (record, control)
}

fn read_control(
    proof: &ProviderDiscoveryProofV1,
    provider: ProviderId,
    expected_binding: &SignerCustodyBindingV1,
    block: &VerifiedSumeragiBlock,
    now_unix_ms: u64,
) -> Result<stream_token_control::VerifiedStreamTokenCustodyControlV1, FinalityError> {
    proof.verify_stream_token_custody_control(
        block.commitment().schedule.current.network_id,
        provider,
        proof.world.schema_hash,
        expected_binding,
        block,
        now_unix_ms,
    )
}

#[test]
fn current_control_returns_exact_native_revision_binding_and_approval_anchor() {
    let (_, proof, provider, block) = account_fixture();
    let (record, control) = control_record(&proof);
    let verified = read_control(&proof, provider, &control.policy.binding, &block, 2_500).unwrap();
    assert_eq!(verified.control(), &control);
    assert_eq!(verified.revision(), record.revision);
    assert_eq!(verified.discovery().height(), block.height());
    assert_eq!(verified.discovery().owner(), &proof.owner);
    assert_eq!(verified.anchor().height, block.height());
    assert_eq!(
        verified.anchor().block_hash,
        *block.header().hash().as_ref()
    );
    assert_eq!(
        verified.anchor().state_digest,
        record.canonical_digest().unwrap()
    );
    assert_ne!(verified.anchor().state_digest, control.predecessor_digest);
}

#[test]
fn unenrolled_rotated_revoked_and_expired_controls_are_readable_without_signing_eligibility() {
    for mutation in 0..7 {
        let (mut native, mut proof, provider, _) = account_fixture();
        let (mut record, mut control) = control_record(&proof);
        let predecessor = control.predecessor_digest;
        match mutation {
            0 | 6 => {
                // Configuration is inspectable before enrollment or account-read capability.
                if mutation == 6 {
                    let (plain_native, plain_proof, plain_provider, _) = fixture(false);
                    assert_eq!(plain_provider, provider);
                    native = plain_native;
                    proof = plain_proof;
                }
                control.next_sequence = 1;
                control.predecessor_digest = [0; 32];
                control.active_head = None;
                record.active_enrollment = None;
            }
            1 => {
                // A policy/key rotation retains enrollment sequence and predecessor, but no head.
                control.policy.binding.key_revision += 1;
                control.policy.binding.public_key = key(0x63).public_key().clone();
                control.policy.binding.policy_revision += 1;
                control.policy.binding.policy_digest = [14; 32];
                control.policy.attester_public_key = key(0x64).public_key().clone();
                control.policy.attester_authority.key_revision += 1;
                control.active_head = None;
                record.active_enrollment = None;
                record.predecessor_digest = record.canonical_digest().unwrap();
                record.revision += 1;
            }
            2 => control.signer_revoked = true,
            3 => control.attester_revoked = true,
            // Expired enrollment and expired policy remain committed facts, not permission.
            4 | 5 => {}
            _ => unreachable!(),
        }
        record.control_state = norito::encode_canonical(&control).unwrap();
        install(&mut proof, &record);
        rebuild(&mut proof, provider);
        let block = certify(&mut native, &proof);
        let now = match mutation {
            4 => 5_000,
            5 => 10_000,
            _ => 2_500,
        };
        let verified =
            read_control(&proof, provider, &control.policy.binding, &block, now).unwrap();
        assert_eq!(verified.control(), &control, "committed state {mutation}");
        assert_eq!(verified.revision(), record.revision);
        if mutation == 0 {
            // The existing data-only frame carries unenrolled state without a new wire type.
            let decoded =
                ProviderDiscoveryProofV1::decode_frame(&norito::encode_canonical(&proof).unwrap())
                    .unwrap();
            assert_eq!(decoded, proof);
            assert_eq!(
                read_control(&decoded, provider, &control.policy.binding, &block, now)
                    .unwrap()
                    .control(),
                &control,
            );
            assert_eq!(
                norito::json::from_json::<ProviderDiscoveryProofV1>(
                    &norito::json::to_json(&proof).unwrap(),
                )
                .unwrap(),
                proof,
            );
        }
        if mutation == 1 {
            assert_eq!(verified.control().next_sequence, 2);
            assert_eq!(verified.control().predecessor_digest, predecessor);
        }
        assert!(
            proof
                .verify_account_read(
                    native.chain_id(),
                    native.network_id(),
                    provider,
                    proof.world.schema_hash,
                    &block,
                    now,
                )
                .is_err(),
            "control state {mutation} must not authorize account reads"
        );
    }
}

#[test]
fn control_read_requires_the_complete_independent_binding_and_certified_scope() {
    let (native, proof, provider, block) = account_fixture();
    let (_, control) = control_record(&proof);
    for mutation in 0..12 {
        let mut expected = control.policy.binding.clone();
        match mutation {
            0 => expected.chain_id = "foreign-chain".into(),
            1 => expected.network_id = [21; 32],
            2 => expected.runtime_handle = "software://other/runtime".into(),
            3 => expected.key_handle = "software://other/key".into(),
            4 => expected.service_id = "other-service".into(),
            5 => expected.administrator_id = "other-administrator".into(),
            6 => {
                expected.purpose = SignerPurposeBindingV1::StreamToken {
                    provider_id: [22; 32],
                }
            }
            7 => expected.public_key = key(0x65).public_key().clone(),
            8 => expected.key_revision += 1,
            9 => expected.policy_revision += 1,
            10 => expected.policy_digest = [23; 32],
            11 => expected.runtime_handle.clear(),
            _ => unreachable!(),
        }
        assert!(
            read_control(&proof, provider, &expected, &block, 2_500).is_err(),
            "binding field {mutation}"
        );
    }
    assert!(
        read_control(
            &proof,
            ProviderId::new([24; 32]),
            &control.policy.binding,
            &block,
            2_500
        )
        .is_err()
    );
    assert!(
        proof
            .verify_stream_token_custody_control(
                native.network_id(),
                provider,
                Hash::new(b"foreign schema"),
                &control.policy.binding,
                &block,
                2_500,
            )
            .is_err()
    );
    let other_network = NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
        Hash::new(b"other genesis"),
    ));
    assert!(
        proof
            .verify_stream_token_custody_control(
                other_network,
                provider,
                proof.world.schema_hash,
                &control.policy.binding,
                &block,
                2_500,
            )
            .is_err()
    );
    let advert = decode_provider_advert_v1(&proof.advert).unwrap();
    // Advert validity includes its expiry second; the first expired second is one later.
    let expired_unix_ms = advert
        .expires_at
        .checked_add(1)
        .unwrap()
        .checked_mul(1000)
        .unwrap();
    assert!(
        read_control(
            &proof,
            provider,
            &control.policy.binding,
            &block,
            expired_unix_ms
        )
        .is_err(),
        "expired advert is not current provider admission"
    );
}

#[test]
fn missing_malformed_oversized_or_substituted_control_frames_are_rejected() {
    let (_, original, provider, block) = account_fixture();
    let (_, control) = control_record(&original);
    for mutation in 0..9 {
        let mut proof = original.clone();
        match mutation {
            0 => proof.stream_token = None,
            1 => proof.stream_token.as_mut().unwrap().head.clear(),
            2 => proof.stream_token.as_mut().unwrap().record.clear(),
            3 => proof.stream_token.as_mut().unwrap().head.push(0),
            4 => proof.stream_token.as_mut().unwrap().record.push(0),
            5 => {
                proof.stream_token.as_mut().unwrap().record =
                    vec![0; STREAM_TOKEN_CUSTODY_MAX_RECORD_BYTES_V1 + 1]
            }
            6 => {
                proof.stream_token.as_mut().unwrap().head =
                    vec![0; STREAM_TOKEN_CUSTODY_MAX_RECORD_BYTES_V1 + 1]
            }
            7 => {
                let (mut record, _) = control_record(&proof);
                record.control_state.push(0);
                install(&mut proof, &record);
            }
            8 => {
                // Canonical, internally consistent bytes still need the original World preimage.
                let (mut record, mut changed) = control_record(&proof);
                changed.signer_revoked = true;
                record.control_state = norito::encode_canonical(&changed).unwrap();
                install(&mut proof, &record);
            }
            _ => unreachable!(),
        }
        assert!(
            read_control(&proof, provider, &control.policy.binding, &block, 2_500).is_err(),
            "frame mutation {mutation}"
        );
    }
}

#[test]
fn certified_control_rejects_impossible_coordinates_and_inconsistent_enrollment() {
    let (mut native, original, provider, _) = account_fixture();
    let (original_record, original_control) = control_record(&original);
    for mutation in 0..12 {
        let mut proof = original.clone();
        let mut record = original_record.clone();
        let mut control = original_control.clone();
        match mutation {
            0 => record.provider_id = ProviderId::new([25; 32]),
            1 => record.revision = 0,
            2 => record.revision = STREAM_TOKEN_CUSTODY_MAX_REVISIONS_V1 + 1,
            3 => record.execution_height = 0,
            4 => record.execution_height = u64::MAX,
            5 => record.recorded_at_unix_ms = 0,
            6 => record.recorded_at_unix_ms = u64::MAX,
            7 => record.active_enrollment = None,
            8 => control.next_sequence += 1,
            9 => control.policy.binding.network_id = [26; 32],
            10 => control.policy.binding.chain_id = "foreign-chain".into(),
            11 => {
                control.policy.binding.purpose = SignerPurposeBindingV1::StreamToken {
                    provider_id: [27; 32],
                }
            }
            _ => unreachable!(),
        }
        record.control_state = norito::encode_canonical(&control).unwrap();
        install(&mut proof, &record);
        rebuild(&mut proof, provider);
        let block = certify(&mut native, &proof);
        assert!(
            read_control(
                &proof,
                provider,
                &original_control.policy.binding,
                &block,
                2_500
            )
            .is_err(),
            "certified inconsistent control {mutation}"
        );
        assert!(
            proof
                .verify_account_read(
                    native.chain_id(),
                    native.network_id(),
                    provider,
                    proof.world.schema_hash,
                    &block,
                    2_500
                )
                .is_err(),
            "account-read consistency {mutation}"
        );
    }
}

#[test]
fn control_requires_all_native_indices_and_absence_of_a_later_revision() {
    let (mut native, original, provider, _) = account_fixture();
    let (record, control) = control_record(&original);
    for mutation in 0..9 {
        let mut proof = original.clone();
        match mutation {
            0..=2 => {
                let key = match mutation {
                    0 => head_key(provider),
                    1 => record_key(provider, record.revision),
                    _ => height_key(provider, record.execution_height, record.ordinal),
                };
                let key_hash = world_state_value_hash_v1(&key).unwrap();
                proof
                    .world
                    .entries
                    .retain(|entry| entry.key_hash != Some(key_hash));
            }
            3 => {
                proof.world.entries.push(row(
                    "world.smart_contract_state",
                    &record_key(provider, record.revision + 1),
                    &vec![1_u8],
                ));
                proof.world.entries.sort_by(|a, b| {
                    (&a.field_id, a.kind, a.key_hash).cmp(&(&b.field_id, b.kind, b.key_hash))
                });
            }
            _ => {
                let token = proof.stream_token.as_mut().unwrap();
                let mut head: StreamTokenCustodyControlIndexV1 =
                    norito::decode_canonical(&token.head).unwrap();
                match mutation {
                    4 => head.revision += 1,
                    5 => head.height += 1,
                    6 => head.ordinal += 1,
                    7 => head.digest = [28; 32],
                    8 => {
                        // A different value at the expected native height index is not the head.
                        let key_hash = world_state_value_hash_v1(&height_key(
                            provider,
                            record.execution_height,
                            record.ordinal,
                        ))
                        .unwrap();
                        proof
                            .world
                            .entries
                            .iter_mut()
                            .find(|entry| entry.key_hash == Some(key_hash))
                            .unwrap()
                            .value_hash = Hash::new(b"substituted index");
                    }
                    _ => unreachable!(),
                }
                token.head = norito::encode_canonical(&head).unwrap();
                if mutation != 8 {
                    rebuild(&mut proof, provider);
                }
            }
        }
        let block = certify(&mut native, &proof);
        assert!(
            read_control(&proof, provider, &control.policy.binding, &block, 2_500).is_err(),
            "native index mutation {mutation}"
        );
        assert!(
            proof
                .verify_account_read(
                    native.chain_id(),
                    native.network_id(),
                    provider,
                    proof.world.schema_hash,
                    &block,
                    2_500
                )
                .is_err(),
            "account-read native index mutation {mutation}"
        );
    }
}
