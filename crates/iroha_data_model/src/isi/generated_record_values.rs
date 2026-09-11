//! Deterministic populated values for instruction records absent from the first capture.
//!
//! Opaque artifact fields exercise byte preservation. These codec fixtures do not
//! claim that an artifact is qualified or that an instruction passes ledger admission.

use iroha_crypto::{Algorithm, Hash, KeyPair, Signature};
use iroha_primitives::numeric::Quantity;
use norito::json::Value;

use super::capture;
use crate::{
    account::AccountId,
    asset::{AssetDefinitionId, AssetId},
    confidential::{ConfidentialParamsId, ConfidentialStatus, PoseidonParams},
    isi::{
        confidential, content, runtime_upgrade, smart_contract_code, soradns, sorafs, staking,
        transfer, transparent,
    },
    nexus::{PublicLaneRewardRole, PublicLaneRewardShare},
    runtime::{RuntimeUpgradeId, RuntimeUpgradeManifest},
    smart_contract::{ContractAddress, ContractLifecycleOwnerV1},
};
use iroha_model_base::metadata::Metadata;
use iroha_model_base::{topology::DataSpaceId, topology::LaneId};

fn keypair(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519).expect("deterministic fixture key")
}

fn account(seed: u8) -> AccountId {
    AccountId::new(keypair(seed).public_key().clone())
}

fn asset_definition() -> AssetDefinitionId {
    AssetDefinitionId::from_uuid_bytes([
        1, 2, 3, 4, 5, 6, 0x47, 8, 0x89, 10, 11, 12, 13, 14, 15, 16,
    ])
    .expect("fixture UUIDv4")
}

fn confidential_values() -> Vec<Value> {
    vec![
        capture(confidential::SetPedersenParamsLifecycle {
            params_id: ConfidentialParamsId::new(7),
            status: ConfidentialStatus::Active,
            activation_height: Some(10),
            withdraw_height: Some(20),
        }),
        capture(confidential::PublishPoseidonParams {
            params: PoseidonParams {
                params_id: ConfidentialParamsId::new(8),
                round_constants_hash: [0x21; 32],
                mds_matrix_hash: [0x22; 32],
                metadata_uri_cid: Some("ipfs://fixture-poseidon-metadata".into()),
                params_cid: Some("ipfs://fixture-poseidon-params".into()),
                activation_height: Some(11),
                withdraw_height: None,
                status: ConfidentialStatus::Proposed,
            },
        }),
        capture(confidential::SetPoseidonParamsLifecycle {
            params_id: ConfidentialParamsId::new(8),
            status: ConfidentialStatus::Withdrawn,
            activation_height: Some(11),
            withdraw_height: Some(21),
        }),
    ]
}

fn runtime_values() -> Vec<Value> {
    let manifest = RuntimeUpgradeManifest {
        name: "fixture-release".into(),
        description: "Codec fixture with the first-release ABI".into(),
        abi_version: 1,
        abi_hash: [0x31; 32],
        added_syscalls: Vec::new(),
        added_pointer_types: Vec::new(),
        start_height: 10,
        end_height: 20,
        sbom_digests: Vec::new(),
        slsa_attestation: b"synthetic-codec-artifact".to_vec(),
        provenance: Vec::new(),
    };
    let manifest_bytes = norito::to_bytes(&manifest).expect("typed manifest bytes");
    let id = RuntimeUpgradeId(*Hash::new(&manifest_bytes).as_ref());
    vec![
        capture(runtime_upgrade::ProposeRuntimeUpgrade { manifest_bytes }),
        capture(runtime_upgrade::ActivateRuntimeUpgrade { id }),
        capture(runtime_upgrade::CancelRuntimeUpgrade { id }),
    ]
}

fn contract_values() -> Vec<Value> {
    let contract_address = ContractAddress::derive(
        &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
            .parse()
            .expect("canonical fixture network"),
        &account(0x41),
        7,
        DataSpaceId::UNIVERSAL,
    )
    .expect("fixture contract address");
    vec![
        capture(smart_contract_code::SetContractParliamentDelegation {
            contract_address: contract_address.clone(),
            expected_revision: 2,
            delegated: true,
        }),
        capture(smart_contract_code::OfferContractOwnership {
            contract_address: contract_address.clone(),
            expected_revision: 3,
            new_owner: ContractLifecycleOwnerV1::Account(account(0x42)),
        }),
        capture(smart_contract_code::AcceptContractOwnership {
            contract_address: contract_address.clone(),
            expected_revision: 4,
        }),
        capture(smart_contract_code::CancelContractOwnershipOffer {
            contract_address,
            expected_revision: 5,
        }),
    ]
}

fn staking_values() -> Vec<Value> {
    let validator = account(0x51);
    let staker = account(0x52);
    let request_id = Hash::new(b"fixture-unbond-request");
    vec![
        capture(staking::BondPublicLaneStake {
            lane_id: LaneId::SINGLE,
            validator: validator.clone(),
            staker: staker.clone(),
            amount: Quantity::from(13_u64),
            metadata: Metadata::default(),
        }),
        capture(staking::SchedulePublicLaneUnbond {
            lane_id: LaneId::SINGLE,
            validator: validator.clone(),
            staker: staker.clone(),
            request_id,
            amount: Quantity::from(7_u64),
            release_at_ms: 1_234_567,
        }),
        capture(staking::FinalizePublicLaneUnbond {
            lane_id: LaneId::SINGLE,
            validator: validator.clone(),
            staker,
            request_id,
        }),
        capture(staking::SlashPublicLaneValidator {
            lane_id: LaneId::SINGLE,
            validator: validator.clone(),
            offence_height: 17,
            slash_id: Hash::new(b"fixture-slash"),
            amount: Quantity::from(3_u64),
            reason_code: "double_sign".into(),
            metadata: Metadata::default(),
        }),
        capture(staking::RecordPublicLaneRewards {
            lane_id: LaneId::SINGLE,
            epoch: 4,
            reward_asset: AssetId::new(asset_definition(), validator.clone()),
            total_reward: Quantity::from(5_u64),
            shares: vec![PublicLaneRewardShare {
                account: validator,
                role: PublicLaneRewardRole::Validator,
                amount: Quantity::from(5_u64),
            }],
            metadata: Metadata::default(),
        }),
    ]
}

fn transfer_and_metadata_values() -> Vec<Value> {
    let entry = transfer::TransferAssetBatchEntry::with_leg_id(
        "fixture-leg-7",
        account(0x61),
        account(0x62),
        asset_definition(),
        19_u64,
    );
    let asset = AssetId::new(asset_definition(), account(0x61));
    vec![
        capture(entry.clone()),
        capture(transfer::TransferAssetBatch::new(vec![entry])),
        capture(transparent::SetAssetKeyValue::new(
            asset.clone(),
            "fixture_key".parse().expect("metadata name"),
            37_u64,
        )),
        capture(transparent::RemoveAssetKeyValue::new(
            asset,
            "fixture_key".parse().expect("metadata name"),
        )),
    ]
}

fn soradns_values() -> Vec<Value> {
    use crate::soradns::{DirectoryRotationPolicyV1, RadRevokeReason, ResolverDirectoryRecordV1};
    let key = keypair(0x71);
    let signature = Signature::try_new(key.private_key(), b"synthetic-directory-fixture")
        .expect("sign deterministic codec fixture");
    let cid: crate::ipfs::IpfsPath =
        "/ipfs/bafybeigdyrzt5sfp7udm7hu76u4eckj5mtgmjgdxunrxhxmfdv4svvff3q"
            .parse()
            .expect("fixture CID");
    let record = ResolverDirectoryRecordV1 {
        root_hash: [0x72; 32],
        record_version: 1,
        created_at_ms: 1_000,
        rad_count: 3,
        directory_json_sha256: [0x73; 32],
        previous_root: Some([0x74; 32]),
        published_at_block: 0,
        published_at_unix: 0,
        proof_manifest_cid: cid.clone(),
        builder_public_key: key.public_key().clone(),
        builder_signature: signature.clone(),
    };
    vec![
        capture(soradns::SubmitDirectoryDraft {
            record,
            car_cid: cid,
            directory_json_sha256: [0x73; 32],
            builder_public_key: key.public_key().clone(),
            builder_signature: signature,
        }),
        capture(soradns::RevokeResolver {
            resolver_id: [0x75; 32],
            reason: RadRevokeReason::IntegrityViolation,
        }),
        capture(soradns::UnrevokeResolver {
            resolver_id: [0x75; 32],
        }),
        capture(soradns::AddReleaseSigner {
            public_key: key.public_key().clone(),
        }),
        capture(soradns::RemoveReleaseSigner {
            public_key: key.public_key().clone(),
        }),
        capture(soradns::SetDirectoryRotationPolicy {
            policy: DirectoryRotationPolicyV1::default(),
        }),
    ]
}

fn sorafs_values() -> Vec<Value> {
    use crate::sorafs::pop_registry::{POP_ISSUER_POLICY_VERSION_V1, PopIssuerPolicyV1};
    let key = keypair(0x81);
    let (_, public_key) = key.public_key().try_to_bytes().expect("public key bytes");
    let policy = PopIssuerPolicyV1 {
        version: POP_ISSUER_POLICY_VERSION_V1,
        revision: 1,
        predecessor_policy_digest: None,
        issuer_id: "pop-issuer-fixture".into(),
        issuer_account: AccountId::new(key.public_key().clone()),
        issuer_public_key: public_key.try_into().expect("Ed25519 key width"),
        max_credentials_per_batch: 16,
        max_revocations_per_publication: 32,
        max_credential_lifetime_secs: 86_400,
        max_future_clock_skew_secs: 30,
        paused: false,
    };
    policy.validate().expect("fixture issuer policy shape");
    let issuer_policy_digest = policy.digest().expect("fixture policy digest");
    vec![
        capture(sorafs::SetSorafsPopIssuerPolicy { policy }),
        capture(sorafs::CommitSorafsPopCredentialBatch {
            batch_payload: b"opaque-synthetic-codec-batch".to_vec(),
        }),
        capture(sorafs::PublishSorafsPopRevocationList {
            revocation_list_payload: b"opaque-synthetic-codec-revocations".to_vec(),
            issuer_policy_digest,
        }),
        capture(sorafs::RegisterSorafsModerationJurorEligibility {
            case_id: "fixture-case".into(),
            round_id: "fixture-round".into(),
            membership_proof_payload: b"opaque-synthetic-codec-membership".to_vec(),
        }),
    ]
}

pub(super) fn values() -> Vec<Value> {
    let mut records = confidential_values();
    records.push(capture(content::RetireContentBundle {
        bundle_id: Hash::new(b"fixture-content-bundle"),
    }));
    records.extend(runtime_values());
    records.extend(contract_values());
    records.extend(staking_values());
    records.extend(transfer_and_metadata_values());
    records.extend(soradns_values());
    records.extend(sorafs_values());
    assert_eq!(records.len(), 30);
    records
}
