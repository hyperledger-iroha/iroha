//! Regression tests for mapping IVM core host APIs to AXT bindings.
#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#![allow(clippy::similar_names, clippy::too_many_lines)]
use iroha_config::parameters::actual::NexusAxt as ActualAxtTiming;
#[cfg(feature = "app_api")]
use iroha_core::block::BlockBuilder;
#[cfg(feature = "app_api")]
use iroha_core::nexus::space_directory::{SpaceDirectoryManifestRecord, SpaceDirectoryManifestSet};
use iroha_core::{
    kura::Kura,
    query::store::LiveQueryStore,
    smartcontracts::ivm::host::CoreHost,
    state::{State, StateReadOnly, World, WorldReadOnly},
};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
#[cfg(feature = "app_api")]
#[allow(unused_imports)]
use iroha_data_model::nexus::{
    AssetPermissionManifest, LaneCatalog, LaneConfig, ManifestVersion, UniversalAccountId,
};
#[allow(unused_imports)]
use iroha_data_model::{
    DataSpaceId,
    block::BlockHeader,
    fastpq::{
        TRANSFER_TRANSCRIPTS_METADATA_KEY, TransferDeltaTranscript, TransferSmtWitness,
        TransferTranscript,
    },
    nexus::{
        AxtBinding, AxtDescriptor, AxtEffectBinding, AxtEnvelopeRecord, AxtHandleBudgetKey,
        AxtHandleFragment, AxtHandleReplayKey, AxtPolicyBinding, AxtPolicyEntry, AxtPolicySnapshot,
        AxtPolicySnapshotValidationError, AxtRejectReason, AxtRemoteSpendClaimV1, AxtReplayRecord,
        AxtTouchSpec, LaneId,
    },
    prelude::*,
};
use iroha_primitives::{Quantity, time::TimeSource};
use iroha_test_samples::ALICE_ID;
use ivm::{
    IVM, IVMHost, PointerType, ProgramMetadata, VMError,
    analysis::{AmxLimits, MemoryAccesses, ProgramAnalysis, RegisterUsage},
    axt::{
        self, AssetHandle, GroupBinding, HandleBudget, HandleSubject, RemoteSpendIntent, SpendOp,
        TouchManifest,
    },
};
use mv::storage::StorageReadOnly;
use nonzero_ext::nonzero;
#[cfg(feature = "app_api")]
use std::collections::BTreeMap;
use std::{num::NonZeroU64, sync::Arc, time::Duration};

const FIXTURE_AUTHORITY_PUBLIC_KEY: &str =
    "ed012059C8A4DA1EBB5380F74ABA51F502714652FDCCE9611FAFB9904E4A3C4D382774";
const AXT_TEST_CHAIN_ID: &[u8] = b"iroha-corehost-axt-test-chain";
macro_rules! assert_ok_gas {
    ($expr:expr $(,)?) => {{
        let gas = $expr.expect("syscall should succeed");
        assert!(gas > 0, "syscall should bill positive gas, got {gas}");
        gas
    }};
}
fn fixture_authority() -> AccountId {
    let public_key = FIXTURE_AUTHORITY_PUBLIC_KEY
        .parse()
        .expect("authority public key");
    AccountId::new(public_key)
}
fn checked_keypair() -> KeyPair {
    KeyPair::try_random().expect("IVM Corehost AXT fixture key generation should succeed")
}
fn axt_test_issuer() -> KeyPair {
    KeyPair::from_seed(vec![0xA5; 32], Algorithm::Ed25519)
}
fn axt_test_network_id() -> iroha_data_model::NetworkId {
    iroha_data_model::NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::new(b"iroha-corehost-axt-test-network"),
    ))
}
fn axt_test_issuer_id() -> iroha_data_model::nexus::UniversalAccountId {
    iroha_data_model::nexus::UniversalAccountId::from_hash(Hash::new(
        b"iroha-corehost-axt-test-issuer",
    ))
}
fn axt_test_asset_definition_id() -> iroha_data_model::asset::AssetDefinitionId {
    iroha_data_model::asset::AssetDefinitionId::from_uuid_bytes([
        0, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 1,
    ])
    .expect("valid AXT fixture asset id")
}
fn axt_test_asset_incarnation() -> iroha_data_model::nexus::AxtAssetIncarnationV1 {
    iroha_data_model::nexus::AxtAssetIncarnationV1::derive(
        &axt_test_network_id(),
        &axt_test_asset_definition_id(),
        &HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(
            b"iroha-corehost-axt-test-registration-header",
        )),
        &Hash::new(b"iroha-corehost-axt-test-registration-execution"),
        0,
    )
}
fn axt_test_issuer_context(
    dataspace: DataSpaceId,
    manifest_root: [u8; 32],
) -> iroha_data_model::nexus::AxtHandleIssuerContextV1 {
    iroha_data_model::nexus::AxtHandleIssuerContextV1 {
        network_id: axt_test_network_id(),
        asset_dsid: dataspace,
        asset_definition_incarnation: axt_test_asset_incarnation(),
        issuer: axt_test_issuer_id(),
        issuer_manifest_root: manifest_root,
        code_root: [0; 32],
        abi_version: 1,
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
    }
}
fn signed_model_handle(
    handle: iroha_data_model::nexus::AssetHandleDraft,
    dataspace: DataSpaceId,
) -> iroha_data_model::nexus::AssetHandle {
    let context = axt_test_issuer_context(dataspace, handle.manifest_view_root);
    handle
        .sign_by_issuer_v1(context, axt_test_issuer().private_key())
        .expect("sign AXT issuer fixture")
}
fn signed_abi_handle(handle: AssetHandle, dataspace: DataSpaceId) -> AssetHandle {
    signed_abi_handle_with_incarnation(handle, dataspace, axt_test_asset_incarnation())
}
fn abi_asset_handle_from_signed_model(handle: iroha_data_model::nexus::AssetHandle) -> AssetHandle {
    AssetHandle {
        asset_definition_id: handle.asset_definition_id,
        scope: handle.scope,
        subject: HandleSubject {
            account: handle.subject.account,
            origin_dsid: handle.subject.origin_dsid,
        },
        budget: HandleBudget {
            remaining: handle.budget.remaining,
            per_use: handle.budget.per_use,
        },
        handle_era: handle.handle_era,
        sub_nonce: handle.sub_nonce,
        group_binding: GroupBinding {
            composability_group_id: handle.group_binding.composability_group_id,
            epoch_id: handle.group_binding.epoch_id,
        },
        target_lane: handle.target_lane,
        axt_binding: handle.axt_binding.into_array().to_vec(),
        manifest_view_root: handle.manifest_view_root.to_vec(),
        expiry_slot: handle.expiry_slot,
        max_clock_skew_ms: handle.max_clock_skew_ms,
        issuer_context: handle.issuer_context,
        issuer_signature: handle.issuer_signature,
    }
}
fn configure_axt_test_host(
    host: &mut CoreHost,
    policies: impl IntoIterator<Item = (DataSpaceId, [u8; 32])>,
) {
    configure_axt_test_host_without_asset(host, policies);
    host.set_axt_asset_policy_for_tests(
        axt_test_asset_definition_id(),
        iroha_data_model::asset::AssetBalancePolicy::DataspaceRestricted,
    );
    host.set_axt_asset_incarnation_for_tests(
        axt_test_asset_definition_id(),
        axt_test_asset_incarnation(),
    );
}
fn configure_axt_test_host_without_asset(
    host: &mut CoreHost,
    policies: impl IntoIterator<Item = (DataSpaceId, [u8; 32])>,
) {
    host.set_chain_id_bytes(AXT_TEST_CHAIN_ID.to_vec());
    host.set_network_id(axt_test_network_id());
    let public_key = axt_test_issuer().public_key().clone();
    for (dataspace, manifest_root) in policies {
        host.set_axt_issuer_key_for_tests(
            dataspace,
            manifest_root,
            axt_test_issuer_id(),
            public_key.clone(),
        );
    }
}
#[test]
fn checked_keypair_preserves_default_algorithm() {
    assert_eq!(checked_keypair().algorithm(), Algorithm::default());
}
fn make_tlv(ty: PointerType, payload: &[u8]) -> Vec<u8> {
    let mut tlv = Vec::with_capacity(7 + payload.len() + 32);
    tlv.extend_from_slice(&(ty as u16).to_be_bytes());
    tlv.push(1);
    let length = u32::try_from(payload.len()).expect("payload length exceeds u32");
    tlv.extend_from_slice(&length.to_be_bytes());
    tlv.extend_from_slice(payload);
    let hash: [u8; 32] = iroha_crypto::Hash::new(payload).into();
    tlv.extend_from_slice(&hash);
    tlv
}
fn store_tlv_bytes(vm: &mut IVM, ty: PointerType, payload: &[u8]) -> u64 {
    let tlv = make_tlv(ty, payload);
    vm.alloc_host_tlv(&tlv).expect("allocate public TLV")
}
fn store_tlv_codec<T: norito::NoritoSerialize>(vm: &mut IVM, ty: PointerType, value: &T) -> u64 {
    let payload = norito::to_bytes(value).expect("serialize Norito payload");
    store_tlv_bytes(vm, ty, &payload)
}
fn store_tlv_norito<T: norito::NoritoSerialize>(vm: &mut IVM, ty: PointerType, value: &T) -> u64 {
    let payload = norito::to_bytes(value).expect("serialize Norito payload");
    store_tlv_bytes(vm, ty, &payload)
}
fn make_policy_snapshot(
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
    target_lane: LaneId,
    active_handle_era: u64,
    next_handle_counter: u64,
    current_slot: u64,
) -> AxtPolicySnapshot {
    let entry = AxtPolicyBinding {
        dsid,
        policy: AxtPolicyEntry {
            manifest_root,
            target_lane,
            active_handle_era,
            next_handle_counter,
            current_slot,
        },
    };
    AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&[entry]),
        entries: vec![entry],
    }
}
fn install_replay_probe_policy(
    host: &mut CoreHost,
    mut snapshot: AxtPolicySnapshot,
    dsid: DataSpaceId,
    handle_era: u64,
    sub_nonce: u64,
) {
    let binding = snapshot
        .entries
        .iter_mut()
        .find(|binding| binding.dsid == dsid)
        .expect("replay probe dataspace policy");
    binding.policy.active_handle_era = handle_era;
    binding.policy.next_handle_counter = sub_nonce;
    snapshot.version = AxtPolicySnapshot::compute_version(&snapshot.entries);
    host.refresh_axt_policy_snapshot(&snapshot)
        .expect("replay probe policy should remain canonical");
}
fn anchor_axt_test_header(state: &mut State, header: BlockHeader) {
    state.push_block_hash_for_testing(header.hash());
    state.update_latest_block_header_cache_for_tests(header);
}
#[test]
fn core_host_rejects_noncanonical_policy_snapshot_without_panicking() {
    let mut snapshot =
        make_policy_snapshot(DataSpaceId::new(91), [0x91; 32], LaneId::new(1), 1, 1, 1);
    let expected = snapshot.version;
    snapshot.version = snapshot.version.wrapping_add(1);
    let actual = snapshot.version;
    let result = CoreHost::new(fixture_authority()).with_axt_policy_snapshot(&snapshot);
    assert!(matches!(
        result,
        Err(AxtPolicySnapshotValidationError::VersionMismatch {
            expected: computed,
            actual: advertised,
        }) if computed == expected && advertised == actual
    ));
}
fn proof_blob_for(
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
    proof_seed: Vec<u8>,
    expiry_slot: u64,
) -> axt::ProofBlob {
    proof_blob_for_with_remote_spend_claims(
        dsid,
        manifest_root,
        proof_seed,
        expiry_slot,
        Vec::new(),
        None,
    )
}
fn proof_blob_for_with_remote_spend_claims(
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
    proof_seed: Vec<u8>,
    expiry_slot: u64,
    remote_spend_claims: Vec<AxtRemoteSpendClaimV1>,
    committed_amount: Option<u128>,
) -> axt::ProofBlob {
    proof_blob_for_profile(
        dsid,
        manifest_root,
        proof_seed,
        expiry_slot,
        remote_spend_claims,
        committed_amount,
        false,
    )
}
#[allow(clippy::too_many_arguments)]
fn proof_blob_for_profile(
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
    proof_seed: Vec<u8>,
    expiry_slot: u64,
    mut remote_spend_claims: Vec<AxtRemoteSpendClaimV1>,
    committed_amount: Option<u128>,
    opaque: bool,
) -> axt::ProofBlob {
    assert!(
        !opaque || remote_spend_claims.is_empty(),
        "opaque proof fixture cannot carry remote-spend claims"
    );
    let source_tx_commitment = test_digest(b"axt-test:source-tx", &[&proof_seed]);
    let claim_digest = test_digest(b"axt-test:claim", &[&proof_seed]);
    let witness_commitment = test_digest(b"axt-test:witness", &[&proof_seed]);
    let policy_commitment = test_digest(b"axt-test:policy", &[&manifest_root]);
    remote_spend_claims
        .sort_by_key(iroha_data_model::nexus::compute_remote_spend_claim_commitment_v1);
    let remote_spend_intent_commitments = remote_spend_claims
        .iter()
        .map(iroha_data_model::nexus::compute_remote_spend_claim_commitment_v1)
        .collect::<Vec<_>>();
    let source_asset = remote_spend_claims
        .first()
        .map(|claim| claim.asset_definition_id.clone());
    assert!(
        remote_spend_claims
            .iter()
            .all(|claim| Some(&claim.asset_definition_id) == source_asset.as_ref()),
        "one FASTPQ proof fixture must use one exact asset definition"
    );
    let binding = iroha_data_model::nexus::AxtFastpqBinding {
        parameter: fastpq_prover::AXT_DEFAULT_PARAMETER.to_string(),
        source_dsid: dsid.as_u64(),
        source_dataspace: "test-dataspace".to_string(),
        source_receipt_id: format!("receipt-{}", hex::encode(source_tx_commitment.as_ref())),
        source_tx_commitment: hex::encode(source_tx_commitment.as_ref()),
        claim_type: if opaque {
            "authorization".to_string()
        } else {
            "tx_predicate".to_string()
        },
        claim_digest: hex::encode(claim_digest.as_ref()),
        witness_commitment: hex::encode(witness_commitment.as_ref()),
        policy_commitment: hex::encode(policy_commitment.as_ref()),
        verified_effect_type: "test_effect".to_string(),
        corridor: "test-corridor".to_string(),
        verifier_id: "fastpq".to_string(),
        verifier_version: "v1".to_string(),
        target_dsids: vec![dsid.as_u64()],
        effect_binding: source_asset.as_ref().map(|asset| AxtEffectBinding {
            destination_domain: None,
            destination_account_id: None,
            vault_account_id: None,
            issuance_account_id: None,
            source_asset_definition_id: Some(asset.to_string()),
            destination_asset_definition_id: None,
            source_amount_i64: None,
            destination_amount_i64: None,
        }),
        remote_spend_intent_commitments,
    };
    let mut dsid_bytes = [0_u8; 16];
    dsid_bytes[..8].copy_from_slice(&dsid.as_u64().to_le_bytes());
    let mut batch = fastpq_prover::TransitionBatch::new(
        fastpq_prover::AXT_DEFAULT_PARAMETER,
        fastpq_prover::PublicInputs {
            dsid: dsid_bytes,
            slot: expiry_slot,
            old_root: test_digest(b"axt-test:old-root", &[&proof_seed]).into(),
            new_root: manifest_root,
            perm_root: test_digest(b"axt-test:perm-root", &[&proof_seed]).into(),
            tx_set_hash: test_digest(b"axt-test:tx-set", &[&proof_seed]).into(),
        },
    );
    if opaque {
        batch.push(fastpq_prover::StateTransition::new(
            b"axt/test/proof".to_vec(),
            proof_seed,
            manifest_root.to_vec(),
            fastpq_prover::OperationKind::MetaSet,
        ));
    } else {
        let batch_hash = source_tx_commitment;
        let mut transcripts = if remote_spend_claims.is_empty() {
            let from = ALICE_ID.clone();
            let to = AccountId::parse_encoded(FIXTURE_MERCHANT_ACCOUNT_LITERAL)
                .expect("canonical fixture receiver")
                .into_account_id();
            vec![TransferTranscript {
                batch_hash,
                deltas: vec![TransferDeltaTranscript {
                    from_account: from,
                    to_account: to,
                    asset_definition: axt_test_asset_definition_id(),
                    amount: Quantity::from(1_u64),
                    from_balance_before: Quantity::from(1_000_000_u64),
                    from_balance_after: Quantity::from(999_999_u64),
                    to_balance_before: Quantity::from(100_u64),
                    to_balance_after: Quantity::from(101_u64),
                    from_smt_witness: TransferSmtWitness::default(),
                    to_smt_witness: TransferSmtWitness::default(),
                }],
                authority_digest: Hash::new(b"axt-corehost-transfer-authority"),
                poseidon_preimage_digest: None,
            }]
        } else {
            remote_spend_claims
                .iter()
                .map(|claim| {
                    let from = AccountId::parse_encoded(&claim.from)
                        .expect("canonical sender")
                        .into_account_id();
                    let to = AccountId::parse_encoded(&claim.to)
                        .expect("canonical receiver")
                        .into_account_id();
                    let amount = iroha_data_model::fastpq::normalized_numeric_to_u64(
                        claim.effective_amount.as_numeric(),
                        0,
                    )
                    .expect("fixture transfer amount is an exact scale-zero u64");
                    TransferTranscript {
                        batch_hash,
                        deltas: vec![TransferDeltaTranscript {
                            from_account: from,
                            to_account: to,
                            asset_definition: claim.asset_definition_id.clone(),
                            amount: claim.effective_amount.clone(),
                            from_balance_before: Quantity::from(1_000_000_u64),
                            from_balance_after: Quantity::from(1_000_000_u64 - amount),
                            to_balance_before: Quantity::from(100_u64),
                            to_balance_after: Quantity::from(100_u64 + amount),
                            from_smt_witness: TransferSmtWitness::default(),
                            to_smt_witness: TransferSmtWitness::default(),
                        }],
                        authority_digest: Hash::new(b"axt-corehost-transfer-authority"),
                        poseidon_preimage_digest: None,
                    }
                })
                .collect::<Vec<_>>()
        };
        let (old_root, new_root) =
            fastpq_prover::gadgets::transfer::attach_transfer_smt_witnesses(&mut transcripts)
                .expect("attach transfer SMT witnesses");
        batch.public_inputs.old_root = old_root;
        batch.public_inputs.new_root = new_root;
        for transcript in &mut transcripts {
            transcript.poseidon_preimage_digest =
                Some(fastpq_prover::gadgets::transfer::compute_poseidon_digest(
                    &transcript.deltas[0],
                    &transcript.batch_hash,
                ));
            for delta in &transcript.deltas {
                let balance_bytes = |value: &Quantity| {
                    iroha_data_model::fastpq::normalized_numeric_to_u64(value.as_numeric(), 0)
                        .expect("fixture balance is an exact u64")
                        .to_le_bytes()
                        .to_vec()
                };
                batch.push(fastpq_prover::StateTransition::new(
                    format!("asset/{}/{}", delta.asset_definition, delta.from_account).into_bytes(),
                    balance_bytes(&delta.from_balance_before),
                    balance_bytes(&delta.from_balance_after),
                    fastpq_prover::OperationKind::Transfer,
                ));
                batch.push(fastpq_prover::StateTransition::new(
                    format!("asset/{}/{}", delta.asset_definition, delta.to_account).into_bytes(),
                    balance_bytes(&delta.to_balance_before),
                    balance_bytes(&delta.to_balance_after),
                    fastpq_prover::OperationKind::Transfer,
                ));
            }
        }
        batch.metadata.insert(
            TRANSFER_TRANSCRIPTS_METADATA_KEY.to_owned(),
            norito::to_bytes(&transcripts).expect("encode transfer transcripts"),
        );
        fastpq_prover::set_axt_remote_spend_claims(&mut batch, &binding, &remote_spend_claims)
            .expect("attach remote-spend claims");
    }
    batch.sort();
    batch.metadata.insert(
        "entry_hash".to_string(),
        source_tx_commitment.as_ref().to_vec(),
    );
    fastpq_prover::bind_axt_batch_with_proof_metadata(
        &mut batch,
        &binding,
        manifest_root,
        None,
        committed_amount,
        Some(expiry_slot),
    )
    .expect("bind AXT test batch");
    let proof = fastpq_prover::Prover::canonical(fastpq_prover::AXT_DEFAULT_PARAMETER)
        .expect("FASTPQ prover")
        .prove_axt_bound(&batch, &binding)
        .expect("FASTPQ proof");
    let fastpq_payload =
        fastpq_prover::encode_axt_fastpq_payload(&batch, proof).expect("AXT FASTPQ payload");
    let envelope = axt::AxtProofEnvelope {
        dsid,
        manifest_root,
        da_commitment: None,
        proof: fastpq_payload,
        fastpq_binding: Some(binding),
        committed_amount,
        amount_commitment: None,
    };
    axt::ProofBlob {
        payload: norito::to_bytes(&envelope).expect("encode proof envelope"),
        expiry_slot: Some(expiry_slot),
    }
}
fn proof_blob_for_remote_spend(
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
    proof_seed: Vec<u8>,
    expiry_slot: u64,
    handle: &AssetHandle,
    intent: &RemoteSpendIntent,
    effective_amount: &Quantity,
) -> axt::ProofBlob {
    proof_blob_for_remote_spends(
        dsid,
        manifest_root,
        proof_seed,
        expiry_slot,
        &[(handle, intent, effective_amount)],
    )
}
fn proof_blob_for_remote_spends(
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
    proof_seed: Vec<u8>,
    expiry_slot: u64,
    spends: &[(&AssetHandle, &RemoteSpendIntent, &Quantity)],
) -> axt::ProofBlob {
    proof_blob_for_remote_spends_with_committed_amount(
        dsid,
        manifest_root,
        proof_seed,
        expiry_slot,
        spends,
        None,
    )
}
fn proof_blob_for_remote_spends_with_committed_amount(
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
    proof_seed: Vec<u8>,
    expiry_slot: u64,
    spends: &[(&AssetHandle, &RemoteSpendIntent, &Quantity)],
    committed_amount: Option<u128>,
) -> axt::ProofBlob {
    let claims = spends
        .iter()
        .map(|(handle, intent, effective_amount)| {
            AxtRemoteSpendClaimV1::new(
                AxtHandleReplayKey::from_parts(
                    intent.asset_dsid,
                    handle.issuer_context.asset_definition_incarnation,
                    handle.binding_array().expect("fixture handle binding"),
                    handle.handle_era,
                    handle.sub_nonce,
                    handle.target_lane,
                ),
                intent.op.asset_definition_id.clone(),
                intent.op.kind.clone(),
                intent.op.from.clone(),
                intent.op.to.clone(),
                (*effective_amount).clone(),
            )
        })
        .collect::<Vec<_>>();
    proof_blob_for_with_remote_spend_claims(
        dsid,
        manifest_root,
        proof_seed,
        expiry_slot,
        claims,
        committed_amount,
    )
}
fn model_proof_blob_for(
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
    proof_seed: &[u8],
    expiry_slot: u64,
) -> iroha_data_model::nexus::ProofBlob {
    let proof = proof_blob_for(dsid, manifest_root, proof_seed.to_vec(), expiry_slot);
    iroha_data_model::nexus::ProofBlob {
        payload: proof.payload,
        expiry_slot: proof.expiry_slot,
    }
}
fn test_digest(domain: &[u8], parts: &[&[u8]]) -> iroha_crypto::Hash {
    let mut payload = Vec::new();
    payload.extend_from_slice(domain);
    for part in parts {
        payload.extend_from_slice(part);
    }
    iroha_crypto::Hash::new(payload)
}
fn host_with_policy(
    authority: AccountId,
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
    target_lane: LaneId,
    current_slot: u64,
) -> CoreHost {
    let snapshot = make_policy_snapshot(dsid, manifest_root, target_lane, 1, 1, current_slot);
    let mut host = CoreHost::new(authority)
        .with_axt_policy_snapshot(&snapshot)
        .expect("fixture AXT policy snapshot should be canonical");
    configure_axt_test_host(&mut host, [(dsid, manifest_root)]);
    host
}
fn single_remote_spend(
    authority: &AccountId,
    binding: [u8; 32],
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
    lane: LaneId,
    sub_nonce: u64,
    origin_dsid: Option<DataSpaceId>,
    recipient: &str,
) -> (AssetHandle, RemoteSpendIntent, Quantity) {
    let amount = Quantity::from(5_u64);
    let handle = signed_abi_handle(
        AssetHandle {
            asset_definition_id: axt_test_asset_definition_id(),
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid,
            },
            budget: HandleBudget {
                remaining: Quantity::from(10_u64),
                per_use: Some(Quantity::from(10_u64)),
            },
            handle_era: 1,
            sub_nonce,
            group_binding: GroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane: lane,
            axt_binding: binding.to_vec(),
            manifest_view_root: manifest_root.to_vec(),
            expiry_slot: 20,
            max_clock_skew_ms: Some(0),
            issuer_context: Default::default(),
            issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
        },
        dsid,
    );
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            asset_definition_id: axt_test_asset_definition_id(),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: recipient.to_owned(),
            amount: Some(amount.clone()),
        },
    };
    (handle, intent, amount)
}
fn use_single_handle_envelope(
    host: &mut CoreHost,
    descriptor: &axt::AxtDescriptor,
    proof: &axt::ProofBlob,
    handle: &AssetHandle,
    intent: &RemoteSpendIntent,
) -> Result<u64, VMError> {
    let mut vm = IVM::new(1_000_000);
    let descriptor_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, descriptor);
    vm.set_register(10, descriptor_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)?;
    let dsid_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &intent.asset_dsid);
    let touch = TouchManifest {
        read: Vec::new(),
        write: Vec::new(),
    };
    let touch_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &touch);
    vm.set_register(10, dsid_ptr);
    vm.set_register(11, touch_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)?;
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, proof);
    vm.set_register(10, dsid_ptr);
    vm.set_register(11, proof_ptr);
    host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm)?;
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, handle);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
}
fn commit_single_handle_envelope(
    host: &mut CoreHost,
    descriptor: &axt::AxtDescriptor,
    proof: &axt::ProofBlob,
    handle: &AssetHandle,
    intent: &RemoteSpendIntent,
) -> Result<u64, VMError> {
    use_single_handle_envelope(host, descriptor, proof, handle, intent)?;
    let mut vm = IVM::new(1_000_000);
    host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm)
}
fn abi_asset_handle_from_model(handle: &iroha_data_model::nexus::AssetHandle) -> AssetHandle {
    AssetHandle {
        asset_definition_id: handle.asset_definition_id.clone(),
        scope: handle.scope.clone(),
        subject: HandleSubject {
            account: handle.subject.account.clone(),
            origin_dsid: handle.subject.origin_dsid,
        },
        budget: HandleBudget {
            remaining: handle.budget.remaining.clone(),
            per_use: handle.budget.per_use.clone(),
        },
        handle_era: handle.handle_era,
        sub_nonce: handle.sub_nonce,
        group_binding: GroupBinding {
            composability_group_id: handle.group_binding.composability_group_id.clone(),
            epoch_id: handle.group_binding.epoch_id,
        },
        target_lane: handle.target_lane,
        axt_binding: handle.axt_binding.as_bytes().to_vec(),
        manifest_view_root: handle.manifest_view_root.to_vec(),
        expiry_slot: handle.expiry_slot,
        max_clock_skew_ms: handle.max_clock_skew_ms,
        issuer_context: handle.issuer_context,
        issuer_signature: handle.issuer_signature.clone(),
    }
}
fn nexus_with_lane_catalog(
    lane_catalog: iroha_data_model::nexus::LaneCatalog,
) -> iroha_config::parameters::actual::Nexus {
    use iroha_config::parameters::actual::LaneRoutingPolicy;
    use iroha_data_model::nexus::{DataSpaceCatalog, DataSpaceMetadata};
    use std::collections::BTreeSet;
    let mut dataspace_ids: BTreeSet<DataSpaceId> = lane_catalog
        .lanes()
        .iter()
        .map(|lane| lane.dataspace_id)
        .collect();
    dataspace_ids.insert(DataSpaceId::UNIVERSAL);
    let dataspace_catalog = DataSpaceCatalog::new(
        dataspace_ids
            .into_iter()
            .map(|id| DataSpaceMetadata {
                id,
                alias: if id == DataSpaceId::UNIVERSAL {
                    "universal".to_owned()
                } else {
                    format!("dataspace_{}", id.as_u64())
                },
                description: None,
                fault_tolerance: 1,
            })
            .collect(),
    )
    .expect("dataspace catalog derived from lane catalog");
    let default_lane = lane_catalog
        .lanes()
        .first()
        .expect("lane catalog contains at least one lane");
    let routing_policy = LaneRoutingPolicy {
        default_lane: default_lane.id,
        default_dataspace: default_lane.dataspace_id,
        rules: Vec::new(),
    };
    let lane_config = iroha_config::parameters::actual::LaneConfig::from_catalog(&lane_catalog);
    iroha_config::parameters::actual::Nexus {
        lane_config,
        lane_catalog,
        dataspace_catalog,
        routing_policy,
        ..iroha_config::parameters::actual::Nexus::default()
    }
}
#[test]
fn axt_policy_snapshot_refreshes_current_slot() {
    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::new(), kura, query_handle);
    let dsid = DataSpaceId::new(9);
    let target_lane = LaneId::new(3);
    let lane_catalog = iroha_data_model::nexus::LaneCatalog::new(
        nonzero!(4_u32),
        vec![iroha_data_model::nexus::LaneConfig {
            id: target_lane,
            dataspace_id: dsid,
            alias: "slot-refresh".into(),
            ..iroha_data_model::nexus::LaneConfig::default()
        }],
    )
    .expect("slot refresh lane catalog");
    *state.nexus.get_mut() = nexus_with_lane_catalog(lane_catalog);
    // Seed a matching hash/header pair so the synthetic state exposes an
    // authenticated, non-zero AXT slot.
    let slot_length_ms = state.view().nexus().axt.slot_length_ms.get();
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, slot_length_ms, 0);
    anchor_axt_test_header(&mut state, header);
    let entry = AxtPolicyEntry {
        manifest_root: [0x11; 32],
        target_lane,
        active_handle_era: 2,
        next_handle_counter: 5,
        current_slot: 0,
    };
    state.set_axt_policy(dsid, entry);
    let snapshot = state.axt_policy_snapshot();
    assert_eq!(snapshot.entries.len(), 1);
    let policy = snapshot.entries.first().expect("entry present").policy;
    assert_eq!(policy.current_slot, state.block_hashes.view().len() as u64);
}
#[test]
fn core_host_handles_axt_flow() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(7);
    let manifest_root = [0x21; 32];
    let mut vm = IVM::new(1_000_000);
    let mut host = host_with_policy(authority.clone(), dsid, manifest_root, LaneId::new(0), 5);
    // Prepare descriptor and begin envelope
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm));
    // Provide manifest to match descriptor prefixes
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/0".into()],
        write: vec!["ledger/0".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm));
    // Prepare two consecutive same-dataspace handles and reuse one verified
    // proof whose canonical intent set authorizes both exact statements.
    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle_a = signed_abi_handle(
        AssetHandle {
            asset_definition_id: axt_test_asset_definition_id(),
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget {
                remaining: Quantity::from(500_u64),
                per_use: Some(Quantity::from(300_u64)),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane: LaneId::new(0),
            axt_binding: binding.to_vec(),
            manifest_view_root: manifest_root.to_vec(),
            expiry_slot: 10,
            max_clock_skew_ms: Some(0),
            issuer_context: Default::default(),
            issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
        },
        dsid,
    );
    let handle_b = signed_abi_handle(
        AssetHandle {
            sub_nonce: 2,
            ..handle_a.clone()
        },
        dsid,
    );
    let intent_a = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            asset_definition_id: axt_test_asset_definition_id(),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: Some(Quantity::from(200_u64)),
        },
    };
    let intent_b = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            asset_definition_id: axt_test_asset_definition_id(),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_VENDOR_ACCOUNT_LITERAL.into(),
            amount: Some(Quantity::from(100_u64)),
        },
    };
    let amount_a = Quantity::from(200_u64);
    let amount_b = Quantity::from(100_u64);
    // A generic proof remains valid for standalone verification, but a proof
    // reused by a handle must bind the exact remote-spend statement.
    let proof = proof_blob_for_remote_spends(
        dsid,
        manifest_root,
        vec![0xA5, 0x5A],
        25,
        &[
            (&handle_a, &intent_a, &amount_a),
            (&handle_b, &intent_b, &amount_b),
        ],
    );
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm));
    let handle_a_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle_a);
    let intent_a_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent_a);
    vm.set_register(10, handle_a_ptr);
    vm.set_register(11, intent_a_ptr);
    vm.set_register(12, 0);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm));
    let handle_b_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle_b);
    let intent_b_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent_b);
    vm.set_register(10, handle_b_ptr);
    vm.set_register(11, intent_b_ptr);
    vm.set_register(12, 0);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm));
    // Commit the envelope
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm));
    // Subsequent operations without an active envelope must fail
    vm.set_register(10, ds_ptr);
    vm.set_register(11, 0);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}

// Keep the remote-spend proof matrix in the parent's shared fixture namespace.
include!("ivm_corehost_axt/remote_spend_proof_tests.rs");

#[test]
fn axt_policy_reject_exposes_context() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(13);
    let lane = LaneId::new(0);
    let manifest_root = [0x33; 32];
    let snapshot = make_policy_snapshot(dsid, manifest_root, lane, 1, 1, 4);
    let mut vm = IVM::new(50_000);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let mut host = CoreHost::new(authority.clone())
        .with_axt_policy_snapshot(&snapshot)
        .expect("fixture AXT policy snapshot should be canonical");
    configure_axt_test_host(&mut host, [(dsid, manifest_root)]);
    // Begin envelope and record a touch for the dataspace.
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm));
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/0".into()],
        write: vec!["ledger/0".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm));
    // Build a handle with a mismatched manifest root to trigger a policy rejection.
    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = signed_abi_handle(
        AssetHandle {
            asset_definition_id: axt_test_asset_definition_id(),
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget {
                remaining: Quantity::from(10_u64),
                per_use: Some(Quantity::from(10_u64)),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0xAA],
                epoch_id: 1,
            },
            target_lane: lane,
            axt_binding: binding.to_vec(),
            manifest_view_root: vec![0xBA; 32],
            expiry_slot: 5,
            max_clock_skew_ms: None,
            issuer_context: Default::default(),
            issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
        },
        dsid,
    );
    let model_handle = handle.clone();
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &model_handle);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            asset_definition_id: axt_test_asset_definition_id(),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: authority.to_string(),
            amount: Some(Quantity::from(1_u64)),
        },
    };
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Err(VMError::PermissionDenied)
    );
    let context = host
        .take_axt_reject_for_tests()
        .expect("axt reject context recorded");
    assert_eq!(context.reason, AxtRejectReason::PolicyDenied);
    assert_eq!(context.dataspace, Some(dsid));
    assert_eq!(context.lane, Some(lane));
    assert_eq!(context.snapshot_version, Some(snapshot.version));
    assert!(
        context.detail.contains("signature") || context.detail.contains("issuer"),
        "detail should identify issuer authentication failure"
    );
}
#[test]
fn axt_handle_allows_configured_clock_skew_window() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(31);
    let manifest_root = [0x66; 32];
    let mut vm = IVM::new(10_000);
    let timing = ActualAxtTiming {
        slot_length_ms: NonZeroU64::new(10).expect("slot length"),
        max_clock_skew_ms: 5,
        proof_cache_ttl_slots: NonZeroU64::new(1).expect("ttl slots"),
        replay_retention_slots: NonZeroU64::new(1).expect("replay slots"),
    };
    let snapshot = make_policy_snapshot(dsid, manifest_root, LaneId::new(0), 1, 1, 11);
    let mut host = CoreHost::new(authority.clone())
        .with_axt_timing(timing)
        .expect("fixture AXT timing should be accepted")
        .with_axt_policy_snapshot(&snapshot)
        .expect("fixture AXT policy snapshot should be canonical");
    configure_axt_test_host(&mut host, [(dsid, manifest_root)]);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm));
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/".into()],
        write: vec!["ledger/".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm));
    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = signed_abi_handle(
        AssetHandle {
            asset_definition_id: axt_test_asset_definition_id(),
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget {
                remaining: Quantity::from(50_u64),
                per_use: Some(Quantity::from(50_u64)),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane: LaneId::new(0),
            axt_binding: binding.to_vec(),
            manifest_view_root: manifest_root.to_vec(),
            expiry_slot: 10,
            max_clock_skew_ms: Some(5),
            issuer_context: Default::default(),
            issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
        },
        dsid,
    );
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            asset_definition_id: axt_test_asset_definition_id(),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_VENDOR_ACCOUNT_LITERAL.into(),
            amount: Some(Quantity::from(25_u64)),
        },
    };
    let amount = Quantity::from(25_u64);
    let proof = proof_blob_for_remote_spend(
        dsid,
        manifest_root,
        vec![0xE5],
        10,
        &handle,
        &intent,
        &amount,
    );
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm));
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm));
}
#[test]
fn axt_handle_rejects_clock_skew_above_config() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(32);
    let manifest_root = [0x67; 32];
    let mut vm = IVM::new(10_000);
    let timing = ActualAxtTiming {
        slot_length_ms: NonZeroU64::new(10).expect("slot length"),
        max_clock_skew_ms: 5,
        proof_cache_ttl_slots: NonZeroU64::new(1).expect("ttl slots"),
        replay_retention_slots: NonZeroU64::new(1).expect("replay slots"),
    };
    let snapshot = make_policy_snapshot(dsid, manifest_root, LaneId::new(0), 1, 1, 2);
    let mut host = CoreHost::new(authority.clone())
        .with_axt_timing(timing)
        .expect("fixture AXT timing should be accepted")
        .with_axt_policy_snapshot(&snapshot)
        .expect("fixture AXT policy snapshot should be canonical");
    configure_axt_test_host(&mut host, [(dsid, manifest_root)]);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm));
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/".into()],
        write: vec!["ledger/".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm));
    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = signed_abi_handle(
        AssetHandle {
            asset_definition_id: axt_test_asset_definition_id(),
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget {
                remaining: Quantity::from(50_u64),
                per_use: Some(Quantity::from(50_u64)),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane: LaneId::new(0),
            axt_binding: binding.to_vec(),
            manifest_view_root: manifest_root.to_vec(),
            expiry_slot: 50,
            max_clock_skew_ms: Some(20),
            issuer_context: Default::default(),
            issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
        },
        dsid,
    );
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            asset_definition_id: axt_test_asset_definition_id(),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_VENDOR_ACCOUNT_LITERAL.into(),
            amount: Some(Quantity::from(25_u64)),
        },
    };
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}
#[test]
fn axt_replay_ledger_persists_through_kura_replay() {
    use iroha_core::block::{BlockBuilder, ValidBlock};
    use iroha_crypto::HashOf;
    use iroha_data_model::{
        nexus::{
            AssetHandleDraft as ModelAssetHandleDraft, AxtEnvelopeRecord as ModelAxtEnvelopeRecord,
            AxtHandleFragment as ModelAxtHandleFragment, AxtHandleReplayKey,
            AxtProofFragment as ModelAxtProofFragment, AxtTouchFragment as ModelAxtTouchFragment,
            GroupBinding as ModelGroupBinding, HandleBudget as ModelHandleBudget,
            HandleSubject as ModelHandleSubject, RemoteSpendIntent as ModelRemoteSpendIntent,
            SpendOp as ModelSpendOp, TouchManifest as ModelTouchManifest,
        },
        peer::PeerId,
        transaction::TransactionEntrypoint,
    };
    use iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR;
    use std::collections::BTreeMap;
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(99);
    let lane = LaneId::new(0);
    let manifest_root = [0x58; 32];
    let handle_era = 2;
    let handle_sub_nonce = 5;
    let genesis_account = iroha_data_model::account::AccountId::new(
        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.public_key().clone(),
    );
    let genesis_domain = Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone());
    let genesis_domain = genesis_domain.build(&genesis_account);
    let genesis_account_value = Account::new(genesis_account.clone()).build(&genesis_account);
    let world = World::with([genesis_domain], [genesis_account_value], []);
    let lane_meta = LaneConfig {
        id: lane,
        dataspace_id: dsid,
        alias: "primary".to_owned(),
        ..LaneConfig::default()
    };
    let lane_catalog = LaneCatalog::new(nonzero!(1_u32), vec![lane_meta]).expect("catalog");
    let nexus = nexus_with_lane_catalog(lane_catalog);
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, Arc::clone(&kura), query);
    *state.nexus.get_mut() = nexus.clone();
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            active_handle_era: handle_era,
            next_handle_counter: handle_sub_nonce,
            current_slot: 0,
        },
    );
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders/replay".into()],
            write: vec!["ledger/replay".into()],
        }],
    };
    let binding_bytes = axt::compute_binding(&descriptor).expect("binding");
    let binding = iroha_data_model::nexus::AxtBinding::new(binding_bytes);
    let envelope = ModelAxtEnvelopeRecord {
        binding,
        lane,
        descriptor: iroha_data_model::nexus::AxtDescriptor {
            dsids: descriptor.dsids.clone(),
            touches: descriptor
                .touches
                .iter()
                .map(|t| iroha_data_model::nexus::AxtTouchSpec {
                    dsid: t.dsid,
                    read: t.read.clone(),
                    write: t.write.clone(),
                })
                .collect(),
        },
        touches: vec![ModelAxtTouchFragment {
            dsid,
            manifest: ModelTouchManifest {
                read: vec!["orders/replay".into()],
                write: vec!["ledger/replay".into()],
            },
        }],
        proofs: vec![ModelAxtProofFragment {
            dsid,
            proof: model_proof_blob_for(dsid, manifest_root, b"kura-replay", 10_000),
        }],
        handles: vec![ModelAxtHandleFragment {
            handle: signed_model_handle(
                ModelAssetHandleDraft {
                    asset_definition_id: axt_test_asset_definition_id(),
                    scope: vec!["transfer".into()],
                    subject: ModelHandleSubject {
                        account: authority.to_string(),
                        origin_dsid: Some(dsid),
                    },
                    budget: ModelHandleBudget {
                        remaining: Quantity::from(50_u64),
                        per_use: Some(Quantity::from(50_u64)),
                    },
                    handle_era,
                    sub_nonce: handle_sub_nonce,
                    group_binding: ModelGroupBinding {
                        composability_group_id: vec![0; 32],
                        epoch_id: 2,
                    },
                    target_lane: lane,
                    axt_binding: binding,
                    manifest_view_root: manifest_root,
                    expiry_slot: 10_000,
                    max_clock_skew_ms: Some(0),
                },
                dsid,
            ),
            intent: ModelRemoteSpendIntent {
                asset_dsid: dsid,
                op: ModelSpendOp {
                    asset_definition_id: axt_test_asset_definition_id(),
                    kind: "transfer".into(),
                    from: authority.to_string(),
                    to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
                    amount: Some(Quantity::from(10_u64)),
                },
            },
            proof: None,
            amount: Some(Quantity::from(10_u64)),
            amount_commitment: None,
        }],
        commit_height: 1,
    };
    let entry_hashes: Vec<HashOf<TransactionEntrypoint>> = Vec::new();
    let signer = checked_keypair();
    let (_, time_source) = TimeSource::new_mock(Duration::ZERO);
    let mut base_block: iroha_data_model::block::SignedBlock =
        BlockBuilder::new_with_time_source(Vec::new(), time_source)
            .chain(0, None)
            .sign(signer.private_key())
            .unpack(|_| {})
            .into();
    let mut state_block = state.block(base_block.header());
    // A live State without an authenticated header cannot publish a policy
    // snapshot. The candidate block context supplies the exact consensus slot
    // and permanent counter projection used by deterministic validation.
    let deterministic_snapshot = state_block.axt_policy_snapshot();
    base_block
        .set_transaction_results_with_transcripts(
            Vec::new(),
            &entry_hashes,
            Vec::new(),
            BTreeMap::new(),
            Vec::new(),
            deterministic_snapshot,
        )
        .expect("empty validation block should advertise its deterministic AXT post-state");
    // Empty live validation still commits its deterministic internal block
    // phases, so advertise the exact count that validation will recompute.
    base_block.set_committed_fragment_count(2);
    let valid_block = ValidBlock::validate_unchecked(base_block, &mut state_block).unpack(|_| {});
    let mut committed = valid_block.commit_unchecked().unpack(|_| {});
    let mut replay_snapshot = committed
        .as_ref()
        .axt_policy_snapshot()
        .cloned()
        .expect("validated block should carry its deterministic AXT post-state");
    let replay_policy = replay_snapshot
        .entries
        .iter_mut()
        .find(|binding| binding.dsid == dsid)
        .expect("replay dataspace policy should remain in the validated snapshot");
    assert_eq!(replay_policy.policy.active_handle_era, handle_era);
    assert_eq!(replay_policy.policy.next_handle_counter, handle_sub_nonce);
    replay_policy.policy.next_handle_counter = handle_sub_nonce
        .checked_add(1)
        .expect("fixture handle counter should advance");
    replay_snapshot.version = AxtPolicySnapshot::compute_version(&replay_snapshot.entries);
    // `validate_unchecked` rebuilds live-execution results. Reattach the
    // historical envelope and its exact ratcheted post-state to this explicitly
    // synthetic replay fixture.
    committed
        .as_mut()
        .set_transaction_results_with_transcripts(
            Vec::new(),
            &entry_hashes,
            Vec::new(),
            BTreeMap::new(),
            vec![envelope.clone()],
            replay_snapshot,
        )
        .expect("replay fixture should retain its AXT envelope");
    committed.as_mut().set_committed_fragment_count(2);
    let peer_id = PeerId::new(signer.public_key().clone());
    let _ = state_block.apply_without_execution(&committed, vec![peer_id.clone()]);
    state_block
        .commit()
        .expect("commit state after AXT envelope");
    kura.store_block(Arc::new(committed.clone().into()))
        .expect("store block with AXT envelope");
    let replay_world = {
        let genesis_domain = Domain::new(iroha_genesis::GENESIS_DOMAIN_ID.clone());
        let genesis_domain = genesis_domain.build(&genesis_account);
        let genesis_account_value = Account::new(genesis_account.clone()).build(&genesis_account);
        World::with([genesis_domain], [genesis_account_value], [])
    };
    let replay_query = LiveQueryStore::start_test();
    let mut replay_state = State::new_for_testing(replay_world, Arc::clone(&kura), replay_query);
    *replay_state.nexus.get_mut() = nexus;
    replay_state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            active_handle_era: handle_era,
            next_handle_counter: handle_sub_nonce,
            current_slot: 0,
        },
    );
    let mut replay_block = replay_state.block(committed.as_ref().header());
    let _ = replay_block.apply_without_execution(&committed, vec![peer_id.clone()]);
    replay_block.commit().expect("commit replayed state");
    let replay_key = AxtHandleReplayKey::from_handle(dsid, &envelope.handles[0].handle);
    let replay_view = replay_state.view();
    let Some(ledger_entry) = replay_view.world().axt_replay_ledger().get(&replay_key) else {
        return;
    };
    assert_eq!(ledger_entry.dataspace, dsid);
    let updated_policy = replay_view
        .world()
        .axt_policies()
        .get(&dsid)
        .expect("policy persisted through replay");
    assert_eq!(
        updated_policy.active_handle_era,
        envelope.handles[0].handle.handle_era
    );
    assert_eq!(
        updated_policy.next_handle_counter,
        envelope.handles[0].handle.sub_nonce.saturating_add(1)
    );
    drop(replay_view);
    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::from_state(authority.clone(), &replay_state)
        .expect("replayed fixture state should produce a valid CoreHost");
    install_replay_probe_policy(
        &mut host,
        replay_state.axt_policy_snapshot(),
        dsid,
        envelope.handles[0].handle.handle_era,
        envelope.handles[0].handle.sub_nonce,
    );
    configure_axt_test_host(&mut host, [(dsid, manifest_root)]);
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/replay".into()],
        write: vec!["ledger/replay".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch");
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            asset_definition_id: axt_test_asset_definition_id(),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: Some(Quantity::from(5_u64)),
        },
    };
    let replay_handle = abi_asset_handle_from_model(&envelope.handles[0].handle);
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &replay_handle);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    let err = host
        .syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
        .expect_err("replay should be rejected after kura replay");
    assert!(matches!(err, VMError::PermissionDenied));
    let reject = host.take_axt_reject_for_tests().expect("reject context");
    assert!(
        matches!(
            reject.reason,
            AxtRejectReason::ReplayCache | AxtRejectReason::Descriptor
        ),
        "unexpected reject reason: {:?}",
        reject.reason
    );
}
#[test]
fn axt_replay_ledger_rejects_reuse_after_restart() {
    let authority: AccountId = ALICE_ID.clone();
    let dsid = DataSpaceId::new(48);
    let lane = LaneId::new(1);
    let manifest_root = [0x42; 32];
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::new(), kura, query_handle);
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            active_handle_era: 1,
            next_handle_counter: 1,
            current_slot: 0,
        },
    );
    let binding = AxtBinding::new([0xBE; 32]);
    let model_handle = signed_model_handle(
        iroha_data_model::nexus::AssetHandleDraft {
            asset_definition_id: axt_test_asset_definition_id(),
            scope: vec!["transfer".into()],
            subject: iroha_data_model::nexus::HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: iroha_data_model::nexus::HandleBudget {
                remaining: Quantity::from(10_u64),
                per_use: Some(Quantity::from(10_u64)),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: iroha_data_model::nexus::GroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane: lane,
            axt_binding: binding,
            manifest_view_root: manifest_root,
            expiry_slot: 50,
            max_clock_skew_ms: Some(0),
        },
        dsid,
    );
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 25, 0);
    let committed_header = header.clone();
    let mut block = state.block(header);
    {
        use iroha_data_model::nexus::{
            RemoteSpendIntent as ModelRemoteSpendIntent, SpendOp as ModelSpendOp,
        };
        let mut stx = block.transaction();
        let envelope = AxtEnvelopeRecord {
            binding,
            lane,
            descriptor: AxtDescriptor {
                dsids: vec![dsid],
                touches: vec![AxtTouchSpec {
                    dsid,
                    read: Vec::new(),
                    write: Vec::new(),
                }],
            },
            touches: Vec::new(),
            proofs: Vec::new(),
            handles: vec![AxtHandleFragment {
                handle: model_handle.clone(),
                intent: ModelRemoteSpendIntent {
                    asset_dsid: dsid,
                    op: ModelSpendOp {
                        asset_definition_id: axt_test_asset_definition_id(),
                        kind: "transfer".into(),
                        from: authority.to_string(),
                        to: FIXTURE_VENDOR_ACCOUNT_LITERAL.into(),
                        amount: Some(Quantity::from(5_u64)),
                    },
                },
                proof: None,
                amount: Some(Quantity::from(5_u64)),
                amount_commitment: None,
            }],
            commit_height: 1,
        };
        stx.record_axt_envelope(envelope)
            .expect("exact replay-ledger AXT sequence should stage");
        stx.apply();
    }
    block.commit().expect("commit replay ledger setup");
    state.push_block_hash_for_testing(committed_header.hash());
    state.update_latest_block_header_cache_for_tests(committed_header);
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            active_handle_era: 1,
            next_handle_counter: 1,
            current_slot: 0,
        },
    );
    let mut vm = IVM::new(10_000);
    let mut host = CoreHost::from_state(authority.clone(), &state)
        .expect("fixture state should produce a valid CoreHost");
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: Vec::new(),
            write: Vec::new(),
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm));
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: Vec::new(),
        write: Vec::new(),
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm));
    let handle = abi_asset_handle_from_signed_model(model_handle);
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            asset_definition_id: axt_test_asset_definition_id(),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_VENDOR_ACCOUNT_LITERAL.into(),
            amount: Some(Quantity::from(5_u64)),
        },
    };
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    let result = host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm);
    assert_eq!(result, Err(ivm::VMError::PermissionDenied));
    let reject = host
        .take_axt_reject_for_tests()
        .expect("replay rejection recorded");
    assert!(
        matches!(
            reject.reason,
            AxtRejectReason::ReplayCache | AxtRejectReason::Descriptor
        ),
        "unexpected reject reason: {:?}",
        reject.reason
    );
    assert_eq!(reject.dataspace.unwrap_or(dsid), dsid);
}
#[test]
fn axt_replay_ledger_prunes_expired_entries_on_slot_rollover() {
    let authority: AccountId = ALICE_ID.clone();
    let dsid = DataSpaceId::new(49);
    let lane = LaneId::new(2);
    let manifest_root = [0x24; 32];
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::new(), kura, query_handle);
    let lane_catalog = LaneCatalog::new(
        nonzero!(3_u32),
        vec![LaneConfig {
            id: lane,
            dataspace_id: dsid,
            alias: "replay-prune".to_owned(),
            ..LaneConfig::default()
        }],
    )
    .expect("replay prune lane catalog");
    *state.nexus.get_mut() = nexus_with_lane_catalog(lane_catalog);
    state.nexus.get_mut().axt.slot_length_ms = nonzero!(1_u64);
    state.nexus.get_mut().axt.replay_retention_slots =
        NonZeroU64::new(1).expect("non-zero retention");
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            active_handle_era: 1,
            next_handle_counter: 1,
            current_slot: 0,
        },
    );
    let binding = AxtBinding::new([0xCD; 32]);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 1, 0);
    let mut block = state.block(header);
    {
        use iroha_data_model::nexus::{
            AssetHandleDraft as ModelAssetHandleDraft, GroupBinding as ModelGroupBinding,
            HandleBudget as ModelHandleBudget, HandleSubject as ModelHandleSubject,
            RemoteSpendIntent as ModelRemoteSpendIntent, SpendOp as ModelSpendOp,
        };
        let mut stx = block.transaction();
        let handle = signed_model_handle(
            ModelAssetHandleDraft {
                asset_definition_id: axt_test_asset_definition_id(),
                scope: vec!["transfer".into()],
                subject: ModelHandleSubject {
                    account: authority.to_string(),
                    origin_dsid: Some(dsid),
                },
                budget: ModelHandleBudget {
                    remaining: Quantity::from(5_u64),
                    per_use: Some(Quantity::from(5_u64)),
                },
                handle_era: 1,
                sub_nonce: 1,
                group_binding: ModelGroupBinding {
                    composability_group_id: vec![0; 32],
                    epoch_id: 1,
                },
                target_lane: lane,
                axt_binding: binding,
                manifest_view_root: manifest_root,
                expiry_slot: 2,
                max_clock_skew_ms: Some(0),
            },
            dsid,
        );
        let envelope = AxtEnvelopeRecord {
            binding,
            lane,
            descriptor: AxtDescriptor {
                dsids: vec![dsid],
                touches: Vec::new(),
            },
            touches: Vec::new(),
            proofs: Vec::new(),
            handles: vec![AxtHandleFragment {
                handle,
                intent: ModelRemoteSpendIntent {
                    asset_dsid: dsid,
                    op: ModelSpendOp {
                        asset_definition_id: axt_test_asset_definition_id(),
                        kind: "transfer".into(),
                        from: authority.to_string(),
                        to: FIXTURE_VENDOR_ACCOUNT_LITERAL.into(),
                        amount: Some(Quantity::from(5_u64)),
                    },
                },
                proof: None,
                amount: Some(Quantity::from(5_u64)),
                amount_commitment: None,
            }],
            commit_height: 1,
        };
        stx.record_axt_envelope(envelope)
            .expect("exact replay-ledger AXT sequence should stage");
        stx.apply();
    }
    block.commit().expect("commit first replay block");
    assert_eq!(
        WorldReadOnly::axt_replay_ledger(state.view().world())
            .iter()
            .count(),
        1,
        "ledger entry should be present after recording"
    );
    let header2 = BlockHeader::new(nonzero!(2_u64), None, None, None, 10, 0);
    let mut block2 = state.block(header2);
    {
        let _stx = block2.transaction();
    }
    block2.commit().expect("commit second replay block");
    assert!(
        WorldReadOnly::axt_replay_ledger(state.view().world()).is_empty(),
        "expired replay entries should be pruned on slot rollover"
    );
}
#[test]
fn axt_replay_ledger_blocks_reuse_after_host_rebuild() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(17);
    let target_lane = LaneId::new(0);
    let manifest_root = [0xAB; 32];
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders/replay".into()],
            write: vec!["ledger/replay".into()],
        }],
    };
    let binding_bytes = axt::compute_binding(&descriptor).expect("binding");
    let model_handle = signed_model_handle(
        iroha_data_model::nexus::AssetHandleDraft {
            asset_definition_id: axt_test_asset_definition_id(),
            scope: vec!["transfer".into()],
            subject: iroha_data_model::nexus::HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: iroha_data_model::nexus::HandleBudget {
                remaining: Quantity::from(50_u64),
                per_use: Some(Quantity::from(50_u64)),
            },
            handle_era: 1,
            sub_nonce: 3,
            group_binding: iroha_data_model::nexus::GroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane,
            axt_binding: AxtBinding::new(binding_bytes),
            manifest_view_root: manifest_root,
            expiry_slot: 25,
            max_clock_skew_ms: Some(0),
        },
        dsid,
    );
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::new(), kura, query);
    let lane_catalog = iroha_data_model::nexus::LaneCatalog::new(
        nonzero!(1_u32),
        vec![iroha_data_model::nexus::LaneConfig {
            id: target_lane,
            dataspace_id: dsid,
            alias: "replay-host-rebuild".into(),
            ..iroha_data_model::nexus::LaneConfig::default()
        }],
    )
    .expect("replay host rebuild lane catalog");
    *state.nexus.get_mut() = nexus_with_lane_catalog(lane_catalog);
    state.nexus.get_mut().axt.replay_retention_slots =
        NonZeroU64::new(64).expect("retention slots");
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane,
            active_handle_era: 1,
            next_handle_counter: 3,
            current_slot: 0,
        },
    );
    let model_descriptor = AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![AxtTouchSpec {
            dsid,
            read: vec!["orders/replay".into()],
            write: vec!["ledger/replay".into()],
        }],
    };
    let binding = AxtBinding::new(binding_bytes);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 5, 0);
    let committed_header = header.clone();
    let mut block = state.block(header);
    {
        use iroha_data_model::nexus::{
            RemoteSpendIntent as ModelRemoteSpendIntent, SpendOp as ModelSpendOp,
        };
        let mut stx = block.transaction();
        stx.current_lane_id = Some(target_lane);
        stx.record_axt_envelope(AxtEnvelopeRecord {
            binding,
            lane: target_lane,
            descriptor: model_descriptor,
            touches: Vec::new(),
            proofs: Vec::new(),
            handles: vec![AxtHandleFragment {
                handle: model_handle.clone(),
                intent: ModelRemoteSpendIntent {
                    asset_dsid: dsid,
                    op: ModelSpendOp {
                        asset_definition_id: axt_test_asset_definition_id(),
                        kind: "transfer".into(),
                        from: authority.to_string(),
                        to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
                        amount: Some(Quantity::from(5_u64)),
                    },
                },
                proof: None,
                amount: Some(Quantity::from(5_u64)),
                amount_commitment: None,
            }],
            commit_height: 1,
        })
        .expect("exact replay-ledger AXT sequence should stage");
        stx.apply();
    }
    block.commit().expect("commit replay ledger envelope");
    anchor_axt_test_header(&mut state, committed_header);
    let mut host = CoreHost::from_state(authority.clone(), &state)
        .expect("fixture state should produce a valid CoreHost");
    install_replay_probe_policy(
        &mut host,
        state.axt_policy_snapshot(),
        dsid,
        model_handle.handle_era,
        model_handle.sub_nonce,
    );
    configure_axt_test_host(&mut host, [(dsid, manifest_root)]);
    let mut vm = IVM::new(1_000_000);
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm));
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/replay".into()],
        write: vec!["ledger/replay".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm));
    let handle = abi_asset_handle_from_model(&model_handle);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            asset_definition_id: axt_test_asset_definition_id(),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: Some(Quantity::from(5_u64)),
        },
    };
    let proof = proof_blob_for_remote_spend(
        dsid,
        manifest_root,
        vec![0xAA, 0x55],
        40,
        &handle,
        &intent,
        &Quantity::from(5_u64),
    );
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm));
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Err(VMError::PermissionDenied)
    );
    let context = host
        .take_axt_reject_for_tests()
        .expect("replay rejection should record context");
    assert_eq!(context.reason, AxtRejectReason::ReplayCache);
    assert_eq!(context.dataspace, Some(dsid));
    assert_eq!(context.lane, Some(target_lane));
    // Advance the ledger a few slots and rebuild the host to simulate a restart; the replay guard
    // should still block reuse until the retention window elapses.
    let slot_length_ms = state.view().nexus().axt.slot_length_ms.get();
    let mut previous_hash = state.view().latest_block_hash();
    for height in 2..=11_u64 {
        let header = BlockHeader::new(
            NonZeroU64::new(height).expect("non-zero synthetic height"),
            previous_hash,
            None,
            None,
            slot_length_ms.saturating_mul(height),
            0,
        );
        let hash = header.hash();
        state.push_block_hash_for_testing(hash);
        state.update_latest_block_header_cache_for_tests(header);
        previous_hash = Some(hash);
    }
    let mut host = CoreHost::from_state(authority.clone(), &state)
        .expect("fixture state should produce a valid CoreHost");
    install_replay_probe_policy(
        &mut host,
        state.axt_policy_snapshot(),
        dsid,
        model_handle.handle_era,
        model_handle.sub_nonce,
    );
    configure_axt_test_host(&mut host, [(dsid, manifest_root)]);
    let mut vm = IVM::new(1_000_000);
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm));
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/replay".into()],
        write: vec!["ledger/replay".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm));
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm));
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Err(VMError::PermissionDenied)
    );
    let context = host
        .take_axt_reject_for_tests()
        .expect("replay rejection should survive host rebuild");
    assert_eq!(context.reason, AxtRejectReason::ReplayCache);
    assert_eq!(context.dataspace, Some(dsid));
    assert_eq!(context.lane, Some(target_lane));
}
#[cfg(feature = "app_api")]
#[test]
fn axt_replay_ledger_blocks_reuse_after_policy_reset() {
    use iroha_data_model::nexus::{
        AssetHandleDraft as ModelAssetHandleDraft, AxtDescriptor as ModelAxtDescriptor,
        AxtEnvelopeRecord as ModelAxtEnvelopeRecord, AxtHandleFragment as ModelAxtHandleFragment,
        AxtProofFragment as ModelAxtProofFragment, AxtTouchFragment as ModelAxtTouchFragment,
        AxtTouchSpec as ModelAxtTouchSpec, GroupBinding as ModelGroupBinding,
        HandleBudget as ModelHandleBudget, HandleSubject as ModelHandleSubject,
        RemoteSpendIntent as ModelRemoteSpendIntent, SpendOp as ModelSpendOp,
        TouchManifest as ModelTouchManifest,
    };
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(45);
    let lane = LaneId::new(0);
    let manifest_root = [0x33; 32];
    let world = World::new();
    let lane_meta = iroha_data_model::nexus::LaneConfig {
        id: lane,
        dataspace_id: dsid,
        alias: "primary".to_owned(),
        ..LaneConfig::default()
    };
    let lane_catalog = LaneCatalog::new(nonzero!(1_u32), vec![lane_meta]).expect("catalog");
    let nexus = nexus_with_lane_catalog(lane_catalog);
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query);
    *state.nexus.get_mut() = nexus;
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            active_handle_era: 2,
            next_handle_counter: 5,
            current_slot: 0,
        },
    );
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders/replay".into()],
            write: vec!["ledger/replay".into()],
        }],
    };
    let binding_bytes = axt::compute_binding(&descriptor).expect("binding");
    let binding = AxtBinding::new(binding_bytes);
    let touch_fragment = ModelAxtTouchFragment {
        dsid,
        manifest: ModelTouchManifest {
            read: vec!["orders/replay".into()],
            write: vec!["ledger/replay".into()],
        },
    };
    let proof_fragment = ModelAxtProofFragment {
        dsid,
        proof: model_proof_blob_for(dsid, manifest_root, b"policy-reset", 50),
    };
    let handle_fragment = ModelAxtHandleFragment {
        handle: signed_model_handle(
            ModelAssetHandleDraft {
                asset_definition_id: axt_test_asset_definition_id(),
                scope: vec!["transfer".into()],
                subject: ModelHandleSubject {
                    account: authority.to_string(),
                    origin_dsid: Some(dsid),
                },
                budget: ModelHandleBudget {
                    remaining: Quantity::from(50_u64),
                    per_use: Some(Quantity::from(50_u64)),
                },
                handle_era: 2,
                sub_nonce: 5,
                group_binding: ModelGroupBinding {
                    composability_group_id: vec![0; 32],
                    epoch_id: 2,
                },
                target_lane: lane,
                axt_binding: binding,
                manifest_view_root: manifest_root,
                expiry_slot: 100,
                max_clock_skew_ms: Some(0),
            },
            dsid,
        ),
        intent: ModelRemoteSpendIntent {
            asset_dsid: dsid,
            op: ModelSpendOp {
                asset_definition_id: axt_test_asset_definition_id(),
                kind: "transfer".into(),
                from: authority.to_string(),
                to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
                amount: Some(Quantity::from(10_u64)),
            },
        },
        proof: None,
        amount: Some(Quantity::from(10_u64)),
        amount_commitment: None,
    };
    let envelope = ModelAxtEnvelopeRecord {
        binding,
        lane,
        descriptor: ModelAxtDescriptor {
            dsids: descriptor.dsids.clone(),
            touches: descriptor
                .touches
                .iter()
                .map(|t| ModelAxtTouchSpec {
                    dsid: t.dsid,
                    read: t.read.clone(),
                    write: t.write.clone(),
                })
                .collect(),
        },
        touches: vec![touch_fragment],
        proofs: vec![proof_fragment],
        handles: vec![handle_fragment.clone()],
        commit_height: 1,
    };
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let committed_header = header.clone();
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.current_lane_id = Some(lane);
    stx.record_axt_envelope(envelope)
        .expect("exact replay-ledger AXT sequence should stage");
    stx.apply();
    block.commit().expect("commit initial replay envelope");
    anchor_axt_test_header(&mut state, committed_header);
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            active_handle_era: 1,
            next_handle_counter: 1,
            current_slot: 0,
        },
    );
    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::from_state(authority.clone(), &state)
        .expect("fixture state should produce a valid CoreHost");
    install_replay_probe_policy(
        &mut host,
        state.axt_policy_snapshot(),
        dsid,
        handle_fragment.handle.handle_era,
        handle_fragment.handle.sub_nonce,
    );
    configure_axt_test_host(&mut host, [(dsid, manifest_root)]);
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/replay".into()],
        write: vec!["ledger/replay".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch");
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            asset_definition_id: axt_test_asset_definition_id(),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: Some(Quantity::from(5_u64)),
        },
    };
    let replayed_handle = abi_asset_handle_from_model(&handle_fragment.handle);
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &replayed_handle);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    let err = host
        .syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
        .expect_err("replay should be rejected");
    assert!(matches!(err, VMError::PermissionDenied));
    let reject = host.take_axt_reject_for_tests().expect("reject context");
    assert_eq!(reject.reason, AxtRejectReason::ReplayCache);
}
#[cfg(feature = "app_api")]
#[test]
fn axt_replay_ledger_persists_across_apply_without_execution() {
    use iroha_data_model::nexus::{
        AssetHandleDraft as ModelAssetHandleDraft, AxtDescriptor as ModelAxtDescriptor,
        AxtEnvelopeRecord as ModelAxtEnvelopeRecord, AxtHandleFragment as ModelAxtHandleFragment,
        AxtProofFragment as ModelAxtProofFragment, AxtTouchFragment as ModelAxtTouchFragment,
        AxtTouchSpec as ModelAxtTouchSpec, GroupBinding as ModelGroupBinding,
        HandleBudget as ModelHandleBudget, HandleSubject as ModelHandleSubject,
        RemoteSpendIntent as ModelRemoteSpendIntent, SpendOp as ModelSpendOp,
        TouchManifest as ModelTouchManifest,
    };
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(58);
    let lane = LaneId::new(1);
    let manifest_root = [0x44; 32];
    let handle_era = 2;
    let handle_sub_nonce = 5;
    let world = World::new();
    let lane_meta = iroha_data_model::nexus::LaneConfig {
        id: lane,
        dataspace_id: dsid,
        alias: "replayed".to_owned(),
        ..LaneConfig::default()
    };
    let lane_catalog = LaneCatalog::new(nonzero!(2_u32), vec![lane_meta]).expect("catalog");
    let nexus = nexus_with_lane_catalog(lane_catalog);
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query);
    *state.nexus.get_mut() = nexus;
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            active_handle_era: handle_era,
            next_handle_counter: handle_sub_nonce,
            current_slot: 0,
        },
    );
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders/replayed".into()],
            write: vec!["ledger/replayed".into()],
        }],
    };
    let binding_bytes = axt::compute_binding(&descriptor).expect("binding");
    let binding = AxtBinding::new(binding_bytes);
    let touch_fragment = ModelAxtTouchFragment {
        dsid,
        manifest: ModelTouchManifest {
            read: vec!["orders/replayed".into()],
            write: vec!["ledger/replayed".into()],
        },
    };
    let proof_fragment = ModelAxtProofFragment {
        dsid,
        proof: model_proof_blob_for(dsid, manifest_root, b"apply-without-execution", 200),
    };
    let handle_fragment = ModelAxtHandleFragment {
        handle: signed_model_handle(
            ModelAssetHandleDraft {
                asset_definition_id: axt_test_asset_definition_id(),
                scope: vec!["transfer".into()],
                subject: ModelHandleSubject {
                    account: authority.to_string(),
                    origin_dsid: Some(dsid),
                },
                budget: ModelHandleBudget {
                    remaining: Quantity::from(50_u64),
                    per_use: Some(Quantity::from(50_u64)),
                },
                handle_era,
                sub_nonce: handle_sub_nonce,
                group_binding: ModelGroupBinding {
                    composability_group_id: vec![0; 32],
                    epoch_id: 2,
                },
                target_lane: lane,
                axt_binding: binding,
                manifest_view_root: manifest_root,
                expiry_slot: 200,
                max_clock_skew_ms: Some(0),
            },
            dsid,
        ),
        intent: ModelRemoteSpendIntent {
            asset_dsid: dsid,
            op: ModelSpendOp {
                asset_definition_id: axt_test_asset_definition_id(),
                kind: "transfer".into(),
                from: authority.to_string(),
                to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
                amount: Some(Quantity::from(10_u64)),
            },
        },
        proof: None,
        amount: Some(Quantity::from(10_u64)),
        amount_commitment: None,
    };
    let envelope = ModelAxtEnvelopeRecord {
        binding,
        lane,
        descriptor: ModelAxtDescriptor {
            dsids: descriptor.dsids.clone(),
            touches: descriptor
                .touches
                .iter()
                .map(|touch| ModelAxtTouchSpec {
                    dsid: touch.dsid,
                    read: touch.read.clone(),
                    write: touch.write.clone(),
                })
                .collect(),
        },
        touches: vec![touch_fragment],
        proofs: vec![proof_fragment],
        handles: vec![handle_fragment.clone()],
        commit_height: 1,
    };
    let entry_hashes: Vec<HashOf<TransactionEntrypoint>> = Vec::new();
    let signer = checked_keypair();
    let (_, time_source) = TimeSource::new_mock(Duration::ZERO);
    let mut base_block: iroha_data_model::block::SignedBlock =
        BlockBuilder::new_with_time_source(Vec::new(), time_source)
            .chain(0, None)
            .sign(signer.private_key())
            .unpack(|_| {})
            .into();
    let envelopes = vec![envelope.clone()];
    let mut state_block = state.block(base_block.header());
    // Derive the advertised post-state from the authenticated candidate block
    // so its slot and permanent counter projection match live validation.
    let deterministic_snapshot = state_block.axt_policy_snapshot();
    base_block
        .set_transaction_results_with_transcripts(
            Vec::new(),
            &entry_hashes,
            Vec::new(),
            BTreeMap::new(),
            Vec::new(),
            deterministic_snapshot,
        )
        .expect("empty validation block should advertise its deterministic AXT post-state");
    // Empty live validation still commits its deterministic internal block
    // phases, so advertise the exact count that validation will recompute.
    base_block.set_committed_fragment_count(2);
    let valid = iroha_core::block::ValidBlock::validate_unchecked(base_block, &mut state_block)
        .unpack(|_| {});
    let mut committed = valid.commit_unchecked().unpack(|_| {});
    let mut replay_snapshot = committed
        .as_ref()
        .axt_policy_snapshot()
        .cloned()
        .expect("validated block should carry its deterministic AXT post-state");
    let replay_policy = replay_snapshot
        .entries
        .iter_mut()
        .find(|binding| binding.dsid == dsid)
        .expect("replay dataspace policy should remain in the validated snapshot");
    assert_eq!(replay_policy.policy.active_handle_era, handle_era);
    assert_eq!(replay_policy.policy.next_handle_counter, handle_sub_nonce);
    replay_policy.policy.next_handle_counter = handle_sub_nonce
        .checked_add(1)
        .expect("fixture handle counter should advance");
    replay_snapshot.version = AxtPolicySnapshot::compute_version(&replay_snapshot.entries);
    committed
        .as_mut()
        .set_transaction_results_with_transcripts(
            Vec::new(),
            &entry_hashes,
            Vec::new(),
            BTreeMap::new(),
            envelopes.clone(),
            replay_snapshot,
        )
        .expect("empty committed test block should attach AXT envelope results");
    committed.as_mut().set_committed_fragment_count(2);
    assert_eq!(
        committed
            .as_ref()
            .axt_envelopes()
            .map_or(0, <[ModelAxtEnvelopeRecord]>::len),
        envelopes.len(),
        "committed block should retain AXT envelopes for replay"
    );
    let _ = state_block.apply_without_execution(&committed, Vec::new());
    state_block.commit().expect("commit replay ledger");
    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::from_state(authority.clone(), &state)
        .expect("fixture state should produce a valid CoreHost");
    install_replay_probe_policy(
        &mut host,
        state.axt_policy_snapshot(),
        dsid,
        handle_fragment.handle.handle_era,
        handle_fragment.handle.sub_nonce,
    );
    configure_axt_test_host(&mut host, [(dsid, manifest_root)]);
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/replayed".into()],
        write: vec!["ledger/replayed".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch");
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            asset_definition_id: axt_test_asset_definition_id(),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: Some(Quantity::from(10_u64)),
        },
    };
    let replayed_handle = abi_asset_handle_from_model(&handle_fragment.handle);
    let proof = proof_blob_for_remote_spend(
        dsid,
        manifest_root,
        vec![0xCC],
        replayed_handle.expiry_slot,
        &replayed_handle,
        &intent,
        &Quantity::from(10_u64),
    );
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm)
        .expect("verify proof");
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &replayed_handle);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    let err = host
        .syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
        .expect_err("replay should be rejected after resync");
    assert!(matches!(err, VMError::PermissionDenied));
    let reject = host.take_axt_reject_for_tests().expect("reject context");
    assert_eq!(reject.reason, AxtRejectReason::ReplayCache);
}
#[cfg(feature = "app_api")]
#[test]
fn axt_replay_entries_expire_after_retention_window() {
    use iroha_core::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, World},
    };
    use iroha_data_model::nexus::{
        AssetHandleDraft as ModelAssetHandleDraft, AxtHandleReplayKey, AxtReplayRecord,
        GroupBinding as ModelGroupBinding, HandleBudget as ModelHandleBudget,
        HandleSubject as ModelHandleSubject,
    };
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(46);
    let lane = LaneId::new(0);
    let manifest_root = [0x55; 32];
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders/ttl".into()],
            write: vec!["ledger/ttl".into()],
        }],
    };
    let binding_bytes = axt::compute_binding(&descriptor).expect("binding");
    let binding = AxtBinding::new(binding_bytes);
    let world = World::new();
    let lane_meta = LaneConfig {
        id: lane,
        dataspace_id: dsid,
        alias: "primary".to_owned(),
        ..LaneConfig::default()
    };
    let lane_catalog = LaneCatalog::new(nonzero!(1_u32), vec![lane_meta]).expect("catalog");
    let mut nexus = nexus_with_lane_catalog(lane_catalog);
    nexus.axt = iroha_config::parameters::actual::NexusAxt {
        slot_length_ms: nonzero!(1_u64),
        max_clock_skew_ms: 0,
        proof_cache_ttl_slots: nonzero!(1_u64),
        replay_retention_slots: nonzero!(2_u64),
    };
    let retention_slots = nexus.axt.replay_retention_slots.get();
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query);
    *state.nexus.get_mut() = nexus;
    state.prune_axt_replay_ledger_for_tests(5, retention_slots);
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            active_handle_era: 1,
            next_handle_counter: 1,
            current_slot: 10,
        },
    );
    // Seed the replay ledger with a stale entry that should expire once the retention window elapses.
    let stale_handle = signed_model_handle(
        ModelAssetHandleDraft {
            asset_definition_id: axt_test_asset_definition_id(),
            scope: vec!["transfer".into()],
            subject: ModelHandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: ModelHandleBudget {
                remaining: Quantity::from(10_u64),
                per_use: Some(Quantity::from(10_u64)),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: ModelGroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane: lane,
            axt_binding: binding,
            manifest_view_root: manifest_root,
            expiry_slot: 50,
            max_clock_skew_ms: Some(0),
        },
        dsid,
    );
    state.insert_axt_replay_entry_for_tests(
        AxtHandleReplayKey::from_handle(dsid, &stale_handle),
        AxtReplayRecord {
            dataspace: dsid,
            budget_key: AxtHandleBudgetKey::from_handle(&stale_handle),
            used_slot: 1,
            retain_until_slot: 3,
        },
    );
    // Advance logical slot so the seeded entry expires before hydration.
    state.prune_axt_replay_ledger_for_tests(5, retention_slots);
    anchor_axt_test_header(
        &mut state,
        BlockHeader::new(nonzero!(1_u64), None, None, None, 5, 0),
    );
    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::from_state(authority.clone(), &state)
        .expect("fixture state should produce a valid CoreHost");
    configure_axt_test_host(&mut host, [(dsid, manifest_root)]);
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/ttl".into()],
        write: vec!["ledger/ttl".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch");
    let handle = signed_abi_handle(
        axt::AssetHandle {
            asset_definition_id: axt_test_asset_definition_id(),
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget {
                remaining: Quantity::from(10_u64),
                per_use: Some(Quantity::from(10_u64)),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane: lane,
            axt_binding: binding_bytes.to_vec(),
            manifest_view_root: manifest_root.to_vec(),
            expiry_slot: 50,
            max_clock_skew_ms: Some(0),
            issuer_context: Default::default(),
            issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
        },
        dsid,
    );
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            asset_definition_id: axt_test_asset_definition_id(),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: Some(Quantity::from(5_u64)),
        },
    };
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    let result = host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm);
    assert!(
        result.is_ok(),
        "handle should be accepted after replay ledger entry expires: {result:?}"
    );
    assert!(host.take_axt_reject_for_tests().is_none());
}
include!("ivm_corehost_axt/amx_budget_test.rs");
#[test]
fn core_host_requires_proof_for_all_dataspaces() {
    let authority = fixture_authority();
    let ds_a = DataSpaceId::new(101);
    let ds_b = DataSpaceId::new(102);
    let root_a = [0xA5; 32];
    let root_b = [0xB6; 32];
    let entries = vec![
        AxtPolicyBinding {
            dsid: ds_a,
            policy: AxtPolicyEntry {
                manifest_root: root_a,
                target_lane: LaneId::new(0),
                active_handle_era: 1,
                next_handle_counter: 1,
                current_slot: 3,
            },
        },
        AxtPolicyBinding {
            dsid: ds_b,
            policy: AxtPolicyEntry {
                manifest_root: root_b,
                target_lane: LaneId::new(0),
                active_handle_era: 1,
                next_handle_counter: 1,
                current_slot: 3,
            },
        },
    ];
    let snapshot = AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&entries),
        entries,
    };
    let descriptor = axt::AxtDescriptor {
        dsids: vec![ds_a, ds_b],
        touches: vec![
            axt::AxtTouchSpec {
                dsid: ds_a,
                read: vec!["orders/a".into()],
                write: vec!["ledger/a".into()],
            },
            axt::AxtTouchSpec {
                dsid: ds_b,
                read: vec!["orders/b".into()],
                write: vec!["ledger/b".into()],
            },
        ],
    };
    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::new(authority.clone())
        .with_axt_policy_snapshot(&snapshot)
        .expect("fixture AXT policy snapshot should be canonical");
    configure_axt_test_host(&mut host, [(ds_a, root_a), (ds_b, root_b)]);
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm));
    let touch_a = TouchManifest {
        read: vec!["orders/a/1".into()],
        write: vec!["ledger/a/1".into()],
    };
    let ds_a_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &ds_a);
    let touch_a_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &touch_a);
    vm.set_register(10, ds_a_ptr);
    vm.set_register(11, touch_a_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm));
    let touch_b = TouchManifest {
        read: vec!["orders/b/1".into()],
        write: vec!["ledger/b/1".into()],
    };
    let ds_b_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &ds_b);
    let touch_b_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &touch_b);
    vm.set_register(10, ds_b_ptr);
    vm.set_register(11, touch_b_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm));
    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle_a = signed_abi_handle(
        AssetHandle {
            asset_definition_id: axt_test_asset_definition_id(),
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(ds_a),
            },
            budget: HandleBudget {
                remaining: Quantity::from(10_u64),
                per_use: None,
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane: LaneId::new(0),
            axt_binding: binding.to_vec(),
            manifest_view_root: root_a.to_vec(),
            expiry_slot: 10,
            max_clock_skew_ms: Some(0),
            issuer_context: Default::default(),
            issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
        },
        ds_a,
    );
    let handle_b = signed_abi_handle(
        AssetHandle {
            subject: HandleSubject {
                origin_dsid: Some(ds_b),
                ..handle_a.subject.clone()
            },
            sub_nonce: 1,
            manifest_view_root: root_b.to_vec(),
            ..handle_a.clone()
        },
        ds_b,
    );
    let intent_a = RemoteSpendIntent {
        asset_dsid: ds_a,
        op: SpendOp {
            asset_definition_id: axt_test_asset_definition_id(),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: Some(Quantity::from(1_u64)),
        },
    };
    let proof_a = proof_blob_for_remote_spend(
        ds_a,
        root_a,
        vec![0xAA],
        12,
        &handle_a,
        &intent_a,
        &Quantity::from(1_u64),
    );
    let intent_b = RemoteSpendIntent {
        asset_dsid: ds_b,
        op: SpendOp {
            asset_definition_id: axt_test_asset_definition_id(),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: Some(Quantity::from(1_u64)),
        },
    };
    let proof_b = proof_blob_for_remote_spend(
        ds_b,
        root_b,
        vec![0xBB],
        12,
        &handle_b,
        &intent_b,
        &Quantity::from(1_u64),
    );
    let handle_a_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle_a);
    let intent_a_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent_a);
    let proof_a_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof_a);
    vm.set_register(10, handle_a_ptr);
    vm.set_register(11, intent_a_ptr);
    vm.set_register(12, proof_a_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm));
    let handle_b_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle_b);
    let intent_b_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent_b);
    let proof_b_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof_b);
    vm.set_register(10, handle_b_ptr);
    vm.set_register(11, intent_b_ptr);
    vm.set_register(12, proof_b_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm));
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm));
    // Omit proof for ds_b to confirm rejection
    let mut vm_fail = IVM::new(1_000_000);
    let mut host_fail = CoreHost::new(authority)
        .with_axt_policy_snapshot(&snapshot)
        .expect("fixture AXT policy snapshot should be canonical");
    configure_axt_test_host(&mut host_fail, [(ds_a, root_a), (ds_b, root_b)]);
    let desc_ptr_fail = store_tlv_codec(&mut vm_fail, PointerType::AxtDescriptor, &descriptor);
    vm_fail.set_register(10, desc_ptr_fail);
    assert_ok_gas!(host_fail.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm_fail));
    let ds_a_ptr_fail = store_tlv_codec(&mut vm_fail, PointerType::DataSpaceId, &ds_a);
    let touch_a_ptr_fail = store_tlv_norito(&mut vm_fail, PointerType::NoritoBytes, &touch_a);
    vm_fail.set_register(10, ds_a_ptr_fail);
    vm_fail.set_register(11, touch_a_ptr_fail);
    assert_ok_gas!(host_fail.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm_fail));
    let ds_b_ptr_fail = store_tlv_codec(&mut vm_fail, PointerType::DataSpaceId, &ds_b);
    let touch_b_ptr_fail = store_tlv_norito(&mut vm_fail, PointerType::NoritoBytes, &touch_b);
    vm_fail.set_register(10, ds_b_ptr_fail);
    vm_fail.set_register(11, touch_b_ptr_fail);
    assert_ok_gas!(host_fail.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm_fail));
    let handle_a_ptr_fail = store_tlv_norito(&mut vm_fail, PointerType::AssetHandle, &handle_a);
    let intent_a_ptr_fail = store_tlv_norito(&mut vm_fail, PointerType::NoritoBytes, &intent_a);
    vm_fail.set_register(10, handle_a_ptr_fail);
    vm_fail.set_register(11, intent_a_ptr_fail);
    let proof_a_ptr_fail = store_tlv_norito(&mut vm_fail, PointerType::ProofBlob, &proof_a);
    vm_fail.set_register(12, proof_a_ptr_fail);
    assert_ok_gas!(host_fail.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm_fail));
    assert!(matches!(
        host_fail.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm_fail),
        Err(VMError::PermissionDenied)
    ));
}
#[test]
fn core_host_rejects_invalid_descriptor() {
    let authority = fixture_authority();
    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::new(authority);
    let dup_descriptor = axt::AxtDescriptor {
        dsids: vec![DataSpaceId::new(7), DataSpaceId::new(7)],
        touches: Vec::new(),
    };
    let dup_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &dup_descriptor);
    vm.set_register(10, dup_ptr);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Err(VMError::PermissionDenied)
    ));
    let ctx = host
        .take_axt_reject_for_tests()
        .expect("descriptor reject recorded");
    assert_eq!(
        ctx.reason,
        iroha_data_model::nexus::AxtRejectReason::Descriptor
    );
    assert!(
        !ctx.detail.is_empty(),
        "descriptor rejection should provide detail"
    );
    let bad_touch_descriptor = axt::AxtDescriptor {
        dsids: vec![DataSpaceId::new(8)],
        touches: vec![axt::AxtTouchSpec {
            dsid: DataSpaceId::new(99),
            read: vec![],
            write: vec![],
        }],
    };
    let bad_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &bad_touch_descriptor);
    vm.set_register(10, bad_ptr);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Err(VMError::PermissionDenied)
    ));
    let ctx = host
        .take_axt_reject_for_tests()
        .expect("descriptor reject recorded");
    assert_eq!(
        ctx.reason,
        iroha_data_model::nexus::AxtRejectReason::Descriptor
    );
    assert!(
        !ctx.detail.is_empty(),
        "descriptor rejection should include detail"
    );
    let dup_touch_descriptor = axt::AxtDescriptor {
        dsids: vec![DataSpaceId::new(9)],
        touches: vec![
            axt::AxtTouchSpec {
                dsid: DataSpaceId::new(9),
                read: vec![],
                write: vec![],
            },
            axt::AxtTouchSpec {
                dsid: DataSpaceId::new(9),
                read: vec![],
                write: vec![],
            },
        ],
    };
    let dup_touch_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &dup_touch_descriptor);
    vm.set_register(10, dup_touch_ptr);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm),
        Err(VMError::PermissionDenied)
    ));
    let ctx = host
        .take_axt_reject_for_tests()
        .expect("descriptor reject recorded");
    assert_eq!(
        ctx.reason,
        iroha_data_model::nexus::AxtRejectReason::Descriptor
    );
    assert!(
        ctx.detail.contains("descriptor failed validation"),
        "descriptor rejection should surface validation failure detail"
    );
}
struct DenyTouchPolicy {
    denied: DataSpaceId,
}
impl axt::AxtPolicy for DenyTouchPolicy {
    fn allow_touch(
        &self,
        dsid: DataSpaceId,
        _manifest: &axt::TouchManifest,
    ) -> Result<(), VMError> {
        if dsid == self.denied {
            Err(VMError::PermissionDenied)
        } else {
            Ok(())
        }
    }
    fn allow_handle(&self, _usage: &axt::HandleUsage) -> Result<(), VMError> {
        Ok(())
    }
}
struct DenyHandlePolicy;
impl axt::AxtPolicy for DenyHandlePolicy {
    fn allow_touch(
        &self,
        _dsid: DataSpaceId,
        _manifest: &axt::TouchManifest,
    ) -> Result<(), VMError> {
        Ok(())
    }
    fn allow_handle(&self, _usage: &axt::HandleUsage) -> Result<(), VMError> {
        Err(VMError::PermissionDenied)
    }
}
#[test]
fn core_host_policy_rejects_touch() {
    let authority = fixture_authority();
    let mut vm = IVM::new(1_000_000);
    let dsid = DataSpaceId::new(50);
    let snapshot = make_policy_snapshot(dsid, [0x50; 32], LaneId::new(0), 1, 1, 0);
    let mut host = CoreHost::new(authority)
        .with_axt_policy(Arc::new(DenyTouchPolicy { denied: dsid }))
        .with_axt_policy_snapshot(&snapshot)
        .expect("fixture AXT policy snapshot should be canonical");
    host.refresh_axt_policy_snapshot(&snapshot)
        .expect("refresh should retain the supplemental policy hook");
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec![],
            write: vec![],
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm));
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &descriptor.dsids[0]);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, 0);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm),
        Err(VMError::PermissionDenied)
    ));
    let context = host
        .take_axt_reject_for_tests()
        .expect("custom touch policy rejection should be recorded");
    assert_eq!(context.reason, AxtRejectReason::PolicyDenied);
    assert_eq!(context.dataspace, Some(dsid));
    assert_eq!(context.lane, Some(LaneId::new(0)));
    assert_eq!(context.snapshot_version, Some(snapshot.version));
    assert_eq!(context.detail, "policy hook denied touch manifest");
}
#[test]
fn core_host_policy_rejects_handle() {
    let authority = fixture_authority();
    let mut vm = IVM::new(1_000_000);
    let dsid = DataSpaceId::new(51);
    let manifest_root = [0x51; 32];
    let snapshot = make_policy_snapshot(dsid, manifest_root, LaneId::new(0), 1, 1, 0);
    let mut host = CoreHost::new(authority.clone())
        .with_axt_policy_snapshot(&snapshot)
        .expect("fixture AXT policy snapshot should be canonical")
        .with_axt_policy(Arc::new(DenyHandlePolicy));
    configure_axt_test_host(&mut host, [(dsid, manifest_root)]);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm));
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, 0);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm));
    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = signed_abi_handle(
        AssetHandle {
            asset_definition_id: axt_test_asset_definition_id(),
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget {
                remaining: Quantity::from(10_u64),
                per_use: None,
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane: LaneId::new(0),
            axt_binding: binding.to_vec(),
            manifest_view_root: manifest_root.to_vec(),
            expiry_slot: 10,
            max_clock_skew_ms: Some(0),
            issuer_context: Default::default(),
            issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
        },
        dsid,
    );
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            asset_definition_id: axt_test_asset_definition_id(),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: Some(Quantity::from(1_u64)),
        },
    };
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Err(VMError::PermissionDenied)
    ));
    let context = host
        .take_axt_reject_for_tests()
        .expect("custom handle policy rejection should be recorded");
    assert_eq!(context.reason, AxtRejectReason::PolicyDenied);
    assert_eq!(context.dataspace, Some(dsid));
    assert_eq!(context.lane, Some(LaneId::new(0)));
    assert_eq!(context.snapshot_version, Some(snapshot.version));
    assert_eq!(context.active_handle_era, Some(1));
    assert_eq!(context.next_handle_counter, Some(1));
    assert_eq!(context.detail, "policy hook denied handle usage");
}
fn use_handle_with_snapshot(
    authority: &AccountId,
    dsid: DataSpaceId,
    snapshot: &AxtPolicySnapshot,
    mut handle: AssetHandle,
) -> Result<u64, VMError> {
    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::new(authority.clone())
        .with_axt_policy_snapshot(snapshot)
        .expect("fixture AXT policy snapshot should be canonical");
    configure_axt_test_host(
        &mut host,
        snapshot
            .entries
            .iter()
            .map(|entry| (entry.dsid, entry.policy.manifest_root)),
    );
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)?;
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, 0);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)?;
    let binding = axt::compute_binding(&descriptor).expect("descriptor binding");
    handle.axt_binding = binding.to_vec();
    let handle = signed_abi_handle(handle, dsid);
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            asset_definition_id: axt_test_asset_definition_id(),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: Some(Quantity::from(10_u64)),
        },
    };
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
}
#[test]
fn axt_snapshot_policy_enforces_lanes_and_counters() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(9);
    let manifest_root = [0xAAu8; 32];
    let entries = vec![AxtPolicyBinding {
        dsid,
        policy: AxtPolicyEntry {
            manifest_root,
            target_lane: LaneId::new(2),
            active_handle_era: 3,
            next_handle_counter: 2,
            current_slot: 50,
        },
    }];
    let snapshot = AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&entries),
        entries,
    };
    let base_handle = AssetHandle {
        asset_definition_id: axt_test_asset_definition_id(),
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: Quantity::from(500_u64),
            per_use: Some(Quantity::from(500_u64)),
        },
        handle_era: 3,
        sub_nonce: 2,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 1,
        },
        target_lane: LaneId::new(2),
        axt_binding: Vec::new(),
        manifest_view_root: manifest_root.to_vec(),
        expiry_slot: 60,
        max_clock_skew_ms: Some(0),
        issuer_context: Default::default(),
        issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
    };
    let mut wrong_lane = base_handle.clone();
    wrong_lane.target_lane = LaneId::new(1);
    assert_eq!(
        use_handle_with_snapshot(&authority, dsid, &snapshot, wrong_lane),
        Err(VMError::PermissionDenied)
    );
    let mut expired = base_handle.clone();
    expired.expiry_slot = 40;
    assert_eq!(
        use_handle_with_snapshot(&authority, dsid, &snapshot, expired),
        Err(VMError::PermissionDenied)
    );
    let mut wrong_root = base_handle.clone();
    wrong_root.manifest_view_root = vec![0xBB; 32];
    assert_eq!(
        use_handle_with_snapshot(&authority, dsid, &snapshot, wrong_root),
        Err(VMError::PermissionDenied)
    );
    let mut old_era = base_handle.clone();
    old_era.handle_era = 1;
    assert_eq!(
        use_handle_with_snapshot(&authority, dsid, &snapshot, old_era),
        Err(VMError::PermissionDenied)
    );
    let mut stale_nonce = base_handle.clone();
    stale_nonce.sub_nonce = 1;
    assert_eq!(
        use_handle_with_snapshot(&authority, dsid, &snapshot, stale_nonce),
        Err(VMError::PermissionDenied)
    );
    assert_ok_gas!(use_handle_with_snapshot(
        &authority,
        dsid,
        &snapshot,
        base_handle
    ));
}
#[test]
fn core_host_reports_amx_budget_timeout() {
    let authority = fixture_authority();
    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::new(authority);
    let analysis = ProgramAnalysis {
        metadata: ProgramMetadata {
            version_major: 1,
            version_minor: 0,
            mode: 0,
            vector_length: 0,
            max_cycles: 0,
            abi_version: 1,
        },
        instruction_count: 10_000,
        registers: RegisterUsage::default(),
        memory: MemoryAccesses::default(),
        syscalls: Vec::new(),
    };
    host.set_amx_analysis(analysis);
    host.set_amx_limits(AmxLimits {
        per_dataspace_budget_ms: 0,
        group_budget_ms: 0,
        ..AmxLimits::default()
    });
    let dsid = DataSpaceId::new(33);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["budget".into()],
            write: vec!["budget".into()],
        }],
    };
    let manifest = TouchManifest {
        read: vec!["budget/read".into()],
        write: vec!["budget/write".into()],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm));
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm));
    let result = host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm);
    match result {
        Err(VMError::AmxBudgetExceeded {
            dataspace,
            stage,
            elapsed_ms,
            budget_ms,
        }) => {
            assert_eq!(dataspace, dsid);
            assert_eq!(stage, iroha_data_model::errors::AmxStage::Commit);
            assert!(elapsed_ms >= budget_ms);
            assert!(elapsed_ms > 0);
        }
        other => panic!("expected AMX budget error, got {other:?}"),
    }
}
#[cfg(feature = "app_api")]
fn use_handle_with_host_policy(
    host: &mut CoreHost,
    authority: &AccountId,
    dsid: DataSpaceId,
    descriptor: &axt::AxtDescriptor,
    mut handle: AssetHandle,
) -> Result<u64, VMError> {
    let mut vm = IVM::new(1_000_000);
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)?;
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = descriptor
        .touches
        .iter()
        .find(|touch| touch.dsid == dsid)
        .map_or_else(
            || TouchManifest {
                read: Vec::new(),
                write: Vec::new(),
            },
            |touch| TouchManifest {
                read: touch.read.clone(),
                write: touch.write.clone(),
            },
        );
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)?;
    let binding = axt::compute_binding(descriptor).expect("descriptor binding");
    handle.axt_binding = binding.to_vec();
    let handle = signed_abi_handle(handle, dsid);
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            asset_definition_id: axt_test_asset_definition_id(),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: Some(Quantity::from(5_u64)),
        },
    };
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
}
#[cfg(feature = "app_api")]
fn use_handle_with_state_policy(
    state: &State,
    authority: &AccountId,
    dsid: DataSpaceId,
    descriptor: &axt::AxtDescriptor,
    handle: AssetHandle,
) -> Result<u64, VMError> {
    let mut host = CoreHost::from_state(authority.clone(), state)
        .expect("fixture state should produce a valid CoreHost");
    configure_axt_test_host(
        &mut host,
        state
            .view()
            .axt_policy_snapshot()
            .entries
            .iter()
            .map(|entry| (entry.dsid, entry.policy.manifest_root)),
    );
    use_handle_with_host_policy(&mut host, authority, dsid, descriptor, handle)
}
#[cfg(feature = "app_api")]
#[test]
fn core_host_rejects_placeholder_policy_with_zero_manifest_root() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(88);
    // Placeholder authorization is a host-policy input, not an active
    // committed Space Directory binding. Model it explicitly so the test
    // cannot accidentally promote an inactive manifest into live state.
    let snapshot = make_policy_snapshot(dsid, [0; 32], LaneId::new(0), 0, 0, 0);
    let policy = snapshot
        .entries
        .iter()
        .find(|entry| entry.dsid == dsid)
        .expect("dataspace binding present")
        .policy;
    assert!(
        policy.manifest_root.iter().all(|byte| *byte == 0),
        "placeholder policies must carry zeroed manifest roots"
    );
    let mut host = CoreHost::new(authority.clone())
        .with_axt_policy_snapshot(&snapshot)
        .expect("placeholder policy snapshot should be canonical");
    configure_axt_test_host(&mut host, [(dsid, policy.manifest_root)]);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let handle = AssetHandle {
        asset_definition_id: axt_test_asset_definition_id(),
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: Quantity::from(25_u64),
            per_use: Some(Quantity::from(25_u64)),
        },
        handle_era: 1,
        sub_nonce: 1,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 1,
        },
        target_lane: LaneId::new(1),
        axt_binding: Vec::new(),
        manifest_view_root: vec![0xCD; 32],
        expiry_slot: 5,
        max_clock_skew_ms: Some(0),
        issuer_context: Default::default(),
        issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
    };
    // A zero-root issuer context is itself non-canonical, so the public syscall
    // must fail during issuer authentication. The host unit test exercises the
    // downstream explicit zero-root policy guard directly.
    let result = use_handle_with_host_policy(&mut host, &authority, dsid, &descriptor, handle);
    assert_eq!(result, Err(VMError::PermissionDenied));
    let rejection = host
        .take_axt_reject_for_tests()
        .expect("placeholder policy should record its fail-closed rejection");
    assert_eq!(rejection.reason, AxtRejectReason::PolicyDenied);
    assert_eq!(rejection.dataspace, Some(dsid));
    assert_eq!(rejection.lane, Some(LaneId::new(0)));
    assert_eq!(rejection.snapshot_version, Some(snapshot.version));
    assert_eq!(rejection.detail, "AXT handle issuer signature is invalid");
}
include!("ivm_corehost_axt/state_envelope_tail.rs");
