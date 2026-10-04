//! Real attestation and finality signatures over explicitly synthetic native custody preimages.
use super::*;
use crate::sorafs::stream_token_custody::{
    StreamTokenCustodyControlRecordV1, history::StreamTokenCustodyControlIndexV1,
};
use sorafs_manifest::{
    provider_advert::account_read::RegisteredAccountReadV1,
    signer::{custody::*, custody_control::*, protocol::*},
};

pub(super) fn account_fixture() -> (
    NativeFinalityFixture,
    ProviderDiscoveryProofV1,
    ProviderId,
    VerifiedSumeragiBlock,
) {
    let (mut native, mut proof, provider, approval) = fixture(false);
    let council = AdmissionHistoryRecordV1::decode_frame(&proof.council.head).unwrap();
    let council: ProviderAdmissionCouncilPolicyV1 = decode_frame(&council.material).unwrap();
    let mut head = AdmissionHistoryRecordV1::decode_frame(&proof.provider.head).unwrap();
    let mut envelope: ProviderAdmissionEnvelopeV1 = decode_frame(&head.material).unwrap();
    let policy = RegisteredAccountReadV1 {
        https_host: "storage.example.com".into(),
        https_port: 443,
        ttl_secs: 60,
        max_streams: 1,
        rate_limit_bytes: 16 * 1024 * 1024,
        requests_per_minute: 30,
    };
    let capability = policy.to_capability().unwrap();
    envelope.proposal.capabilities.push(capability.clone());
    envelope.advert_body.capabilities.push(capability);
    sign(&mut envelope, &council);
    head.material = norito::encode_canonical(&envelope).unwrap();
    proof.provider.head = encode_history_value(&head).unwrap();
    let mut advert = decode_provider_advert_v1(&proof.advert).unwrap();
    advert.body = envelope.advert_body;
    advert.signature.signature = Signature::new(
        key(0x21).private_key(),
        &advert.signature_payload_bytes().unwrap(),
    )
    .payload()
    .to_vec();
    proof.advert = norito::encode_canonical(&advert).unwrap();
    let signer = key(0x61);
    let attester = key(0x62);
    let binding = SignerCustodyBindingV1 {
        chain_id: native.chain_id().into(),
        network_id: *native.network_id().as_bytes(),
        runtime_handle: "software://storage/token".into(),
        key_handle: "software://storage/token/key".into(),
        service_id: "storage-token-signer".into(),
        administrator_id: "storage-token-admin".into(),
        role: SignerRoleV1::StreamToken,
        purpose: SignerPurposeBindingV1::StreamToken {
            provider_id: *provider.as_bytes(),
        },
        algorithm: SignerKeyAlgorithmV1::Ed25519,
        public_key: signer.public_key().clone(),
        key_revision: 7,
        policy_revision: 1,
        policy_digest: [9; 32],
    };
    let authority = SignerCustodyAuthorityV1 {
        service_id: "independent-attester".into(),
        administrator_id: "independent-admin".into(),
        key_revision: 1,
        policy_revision: 1,
        policy_digest: [10; 32],
    };
    let approved_anchor = SignerCustodyAnchorV1 {
        height: approval.height(),
        block_hash: *approval.header().hash().as_ref(),
        state_digest: [11; 32],
    };
    let statement = SignerCustodyStatementV1 {
        magic: SIGNER_CUSTODY_MAGIC_V1,
        version: SIGNER_CUSTODY_VERSION_V1,
        binding: binding.clone(),
        authority: authority.clone(),
        anchor: approved_anchor,
        sequence: 1,
        predecessor_digest: [0; 32],
        issued_at_unix_ms: 2_000,
        expires_at_unix_ms: 5_000,
        evidence_digest: [12; 32],
        revoked: false,
    };
    let signature = Signature::new(
        attester.private_key(),
        &statement.signing_payload().unwrap(),
    );
    let enrollment = SignerCustodyRecordV1 {
        statement,
        attestation: signature.payload().try_into().unwrap(),
    };
    let digest = enrollment.canonical_digest().unwrap();
    let control = SignerCustodyControlStateV1 {
        policy: SignerCustodyPolicyV1 {
            binding,
            attester_authority: authority,
            attester_public_key: attester.public_key().clone(),
            active_from_unix_ms: 1,
            active_until_unix_ms: 10_000,
            max_validity_ms: 3_000,
            max_anchor_age_ms: 1_000,
        },
        next_sequence: 2,
        predecessor_digest: digest,
        active_head: Some(SignerCustodyActiveHeadV1 {
            record_digest: digest,
            sequence: 1,
            approved_anchor,
            key_revision: 7,
            policy_revision: 1,
            policy_digest: [9; 32],
        }),
        signer_revoked: false,
        attester_revoked: false,
    };
    let record = StreamTokenCustodyControlRecordV1 {
        provider_id: provider,
        revision: 1,
        predecessor_digest: [0; 32],
        request_digest: [13; 32],
        execution_height: 3,
        ordinal: 0,
        recorded_at_unix_ms: 2_000,
        authority: proof.owner.clone(),
        control_state: norito::encode_canonical(&control).unwrap(),
        active_enrollment: Some(norito::encode_canonical(&enrollment).unwrap()),
    };
    install(&mut proof, &record);
    rebuild(&mut proof, provider);
    let block = certify(&mut native, &proof);
    (native, proof, provider, block)
}
pub(super) fn install(
    proof: &mut ProviderDiscoveryProofV1,
    record: &StreamTokenCustodyControlRecordV1,
) {
    let index = StreamTokenCustodyControlIndexV1 {
        revision: record.revision,
        digest: record.canonical_digest().unwrap(),
        height: record.execution_height,
        ordinal: record.ordinal,
    };
    proof.stream_token = Some(
        crate::sorafs::stream_token_custody::proof::StreamTokenCustodyRecordProofV1 {
            head: norito::encode_canonical(&index).unwrap(),
            record: norito::encode_canonical(record).unwrap(),
        },
    );
}
#[test]
fn signed_current_custody_and_capability_are_bound_to_same_native_cut() {
    let (native, proof, provider, block) = account_fixture();
    let verified = proof
        .verify_account_read(
            native.chain_id(),
            native.network_id(),
            provider,
            proof.world.schema_hash,
            &block,
            2_500,
        )
        .unwrap();
    assert_eq!(
        verified.policy().https_origin().unwrap(),
        "https://storage.example.com"
    );
    assert_eq!(verified.token_public_key(), key(0x61).public_key());
    assert_eq!(verified.token_key_revision(), 7);
    assert_eq!(verified.enrollment_expires_at_unix_ms(), 5_000);
    assert!(
        proof
            .verify_account_read(
                "foreign-chain",
                native.network_id(),
                provider,
                proof.world.schema_hash,
                &block,
                2_500
            )
            .is_err()
    );
    let borrowed = ProviderDiscoveryProofRefV1::new(
        &proof.world,
        (&proof.council.head, None),
        (&proof.provider.head, None),
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
}
#[test]
fn expired_revoked_missing_and_substituted_enrollment_never_authorize_account_reads() {
    for mutation in 0..5 {
        let (mut native, mut proof, provider, _) = account_fixture();
        let mut record: StreamTokenCustodyControlRecordV1 =
            norito::decode_canonical(&proof.stream_token.as_ref().unwrap().record).unwrap();
        let mut control: SignerCustodyControlStateV1 =
            norito::decode_canonical(&record.control_state).unwrap();
        match mutation {
            0 => control.signer_revoked = true,
            1 => control.attester_revoked = true,
            2 => record.active_enrollment = None,
            3 => record.active_enrollment.as_mut().unwrap()[0] ^= 1,
            _ => {}
        }
        record.control_state = norito::encode_canonical(&control).unwrap();
        install(&mut proof, &record);
        rebuild(&mut proof, provider);
        let block = certify(&mut native, &proof);
        assert!(
            proof
                .verify_account_read(
                    native.chain_id(),
                    native.network_id(),
                    provider,
                    proof.world.schema_hash,
                    &block,
                    if mutation == 4 { 5_000 } else { 2_500 }
                )
                .is_err()
        );
    }
    let (native, proof, provider, block) = fixture(false);
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
            .is_err()
    );
}
