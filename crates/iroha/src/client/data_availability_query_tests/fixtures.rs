//! Shared deterministic DA query and proof fixtures.
use iroha_crypto::{Hash, HashOf, Signature};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    da::{
        commitment::{
            DaCommitmentBundle, DaCommitmentLocation, DaCommitmentProof, DaCommitmentRecord,
            DaCommitmentWithLocation, DaProofPolicyBundle, DaProofScheme,
        },
        ingest::{DaIngestAuthorizationV1, DaIngestSignatureV1, DaPinScopeV1},
        pin_intent::{DaPinIntent, DaPinIntentBundle, DaPinIntentProof, DaPinIntentWithLocation},
        types::{BlobDigest, RetentionPolicy, StorageTicketId},
    },
    sorafs::pin_registry::ManifestDigest,
};
use iroha_model_base::topology::LaneId;
pub(super) fn sample_da_proof_policy_bundle() -> DaProofPolicyBundle {
    DaProofPolicyBundle::new(Vec::new())
}
pub(super) fn sample_da_commitment_record() -> DaCommitmentRecord {
    DaCommitmentRecord {
        lane_id: LaneId::new(7),
        epoch: 2,
        sequence: 5,
        client_blob_id: BlobDigest::new([0x61; 32]),
        manifest_hash: ManifestDigest::new([0x62; 32]),
        proof_scheme: DaProofScheme::MerkleSha256,
        chunk_root: Hash::prehashed([0x63; Hash::LENGTH]),
        proof_digest: None,
        retention_class: RetentionPolicy::default(),
        storage_ticket: StorageTicketId::new([0x64; 32]),
        acknowledgement_sig: iroha_crypto::Signature::try_from_bytes(&[0x65; 64])
            .expect("checked iroha client DA commitment acknowledgement signature fixture"),
    }
}
pub(super) fn sample_da_commitment_with_location() -> DaCommitmentWithLocation {
    DaCommitmentWithLocation {
        commitment: sample_da_commitment_record(),
        location: DaCommitmentLocation {
            block_height: 9,
            index_in_bundle: 0,
        },
    }
}
pub(super) fn sample_da_commitment_proof() -> DaCommitmentProof {
    DaCommitmentProof {
        commitment: sample_da_commitment_record(),
        location: DaCommitmentLocation {
            block_height: 9,
            index_in_bundle: 0,
        },
        bundle_hash: HashOf::<DaCommitmentBundle>::from_untyped_unchecked(Hash::prehashed(
            [0x66; Hash::LENGTH],
        )),
        bundle_len: 1,
        root: Hash::prehashed([0x67; Hash::LENGTH]),
        path: Vec::new(),
    }
}
pub(super) fn sample_da_pin_intent_with_location(network: NetworkId) -> DaPinIntentWithLocation {
    let lane_id = LaneId::new(4);
    let key_pair =
        iroha_crypto::KeyPair::try_from_seed(vec![0xE5; 32], iroha_crypto::Algorithm::Ed25519)
            .expect("valid deterministic client DA proof key");
    let mut authorization = DaIngestAuthorizationV1 {
        network_id: network,
        owner: AccountId::new(key_pair.public_key().clone()),
        lane_id,
        epoch: 6,
        sequence: 8,
        payload_hash: BlobDigest::new([0xE7; 32]),
        payload_bytes: 1,
        request_content_hash: Hash::prehashed([0xE8; 32]),
        signatures: Vec::new(),
    };
    authorization.signatures.push(DaIngestSignatureV1 {
        signer: key_pair.public_key().clone(),
        signature: Signature::try_new(key_pair.private_key(), &authorization.signing_digest())
            .expect("sign deterministic client DA proof authorization"),
    });
    let scope = DaPinScopeV1::new(
        &authorization,
        StorageTicketId::new([0x70; 32]),
        ManifestDigest::new([0x71; 32]),
        None,
    );
    let scope_authorization =
        iroha_data_model::da::ingest::DaPinScopeAuthorizationV1::try_sign(scope, &key_pair)
            .expect("sign deterministic client DA pin scope");
    DaPinIntentWithLocation {
        intent: DaPinIntent::new(authorization, scope_authorization),
        location: DaCommitmentLocation {
            block_height: 10,
            index_in_bundle: 0,
        },
    }
}
pub(super) fn sample_da_pin_intent_proof(network: NetworkId) -> DaPinIntentProof {
    let located = sample_da_pin_intent_with_location(network);
    DaPinIntentProof {
        intent: located.intent,
        location: located.location,
        bundle_hash: HashOf::<DaPinIntentBundle>::from_untyped_unchecked(Hash::prehashed(
            [0x72; Hash::LENGTH],
        )),
        bundle_len: 1,
        root: Hash::prehashed([0x73; Hash::LENGTH]),
        path: Vec::new(),
    }
}
