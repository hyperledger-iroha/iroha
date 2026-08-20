//! Regenerate the canonical AXT fixtures used across SDKs and guard scripts.
//!
//! Run with `cargo run -p iroha_data_model --features dev-tools,test-fixtures --bin axt_fixtures`
//! to refresh `tests/fixtures/*.json`. Use `--check` to verify the checked-in
//! fixtures are up to date without rewriting them.
use hex::{decode, encode};
use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
use iroha_data_model::{
    NetworkId,
    asset::id::AssetDefinitionId,
    block::BlockHeader,
    domain::DomainId,
    nexus::{
        AssetHandle, AssetHandleDraft, AxtBinding, AxtDescriptorBuilder, AxtEffectBinding,
        AxtFastpqBinding, AxtHandleFragment, AxtHandleIssuerContextV1, AxtHandleReplayKey,
        AxtProofEnvelope, AxtProofFragment, AxtTouchFragment, DataSpaceId, GroupBinding,
        HandleBudget, HandleSubject, LaneId, ProofBlob, RemoteSpendIntent, SpendOp, TouchManifest,
        UniversalAccountId, compute_descriptor_binding, compute_remote_spend_intent_commitment_v1,
    },
    testing::axt::{
        DescriptorFixture, EnvelopeFixture, HandleFixtures, PoseidonConstantsFixture,
        PoseidonParamsFixture,
    },
};
use iroha_primitives::numeric::Quantity;
use iroha_zkp_halo2::poseidon::{poseidon2_params_width3, poseidon2_params_width6};
use norito::{json, to_bytes};
use std::{env, error::Error, fs, path::Path};
const DESCRIPTOR_FIXTURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/axt_descriptor_multi_ds.json"
);
const ENVELOPE_FIXTURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/axt_envelope_multi_ds.json"
);
const POSEIDON_FIXTURE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/axt_poseidon_constants.json"
);
fn fixture_issuer() -> KeyPair {
    KeyPair::from_seed(vec![0xA5; 32], Algorithm::Ed25519)
}
fn signed_fixture_handle(draft: AssetHandleDraft, dsid: DataSpaceId) -> AssetHandle {
    let context = AxtHandleIssuerContextV1 {
        network_id: NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::new(b"axt-fixture-network"),
        )),
        asset_dsid: dsid,
        issuer: UniversalAccountId::from_hash(Hash::new(b"axt-fixture-issuer")),
        issuer_manifest_root: draft.manifest_view_root,
        code_root: Hash::new(b"axt-fixture-program").into(),
        abi_version: 1,
        abi_hash: [0xA1; 32],
    };
    draft
        .sign_by_issuer_v1(context, fixture_issuer().private_key())
        .expect("fixture issuer key must sign canonical AXT claims")
}
fn fixture_digest(label: &[u8], dsid: DataSpaceId) -> String {
    let mut payload = Vec::new();
    payload.extend_from_slice(label);
    payload.extend_from_slice(&dsid.as_u64().to_le_bytes());
    encode(Hash::new(payload).as_ref())
}
fn fixture_fastpq_binding(dsid: DataSpaceId) -> AxtFastpqBinding {
    AxtFastpqBinding {
        parameter: "fastpq-lane-balanced".to_string(),
        source_dsid: dsid.as_u64(),
        source_dataspace: format!("fixture-dataspace-{}", dsid.as_u64()),
        source_receipt_id: format!("receipt-{}", fixture_digest(b"receipt", dsid)),
        source_tx_commitment: fixture_digest(b"source-tx", dsid),
        claim_type: "authorization".to_string(),
        claim_digest: fixture_digest(b"claim", dsid),
        witness_commitment: fixture_digest(b"witness", dsid),
        policy_commitment: fixture_digest(b"policy", dsid),
        verified_effect_type: "fixture_effect".to_string(),
        corridor: "fixture-corridor".to_string(),
        verifier_id: "fastpq".to_string(),
        verifier_version: "v1".to_string(),
        target_dsids: vec![dsid.as_u64()],
        effect_binding: None,
        remote_spend_intent_commitments: Vec::new(),
    }
}
fn seeded_account(seed: u8) -> String {
    let key_pair = KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519);
    iroha_data_model::account::AccountId::new(key_pair.public_key().clone()).to_string()
}
fn build_descriptor_fixture() -> Result<DescriptorFixture, Box<dyn Error>> {
    let descriptor = AxtDescriptorBuilder::new()
        .dataspace(DataSpaceId::new(7))
        .dataspace(DataSpaceId::new(1))
        .touch(DataSpaceId::new(1), ["payments/", "orders/"], ["ledger/"])
        .touch(
            DataSpaceId::new(7),
            ["reports/"],
            ["aggregates/", "audits/"],
        )
        .build()?;
    let descriptor_bytes = to_bytes(&descriptor)?;
    let binding_hex = encode(compute_descriptor_binding(&descriptor)?);
    let touch_manifest = vec![
        AxtTouchFragment {
            dsid: DataSpaceId::new(1),
            manifest: TouchManifest::from_read_write(["orders/root"], ["ledger/settlement"]),
        },
        AxtTouchFragment {
            dsid: DataSpaceId::new(7),
            manifest: TouchManifest::from_read_write(
                ["reports/monthly"],
                ["aggregates/monthly", "audits/summary"],
            ),
        },
    ];
    Ok(DescriptorFixture {
        descriptor,
        descriptor_hex: encode(descriptor_bytes),
        touch_manifest,
        binding_hex,
    })
}
fn fixture_binding(descriptor: &DescriptorFixture) -> Result<AxtBinding, Box<dyn Error>> {
    let binding_bytes = decode(&descriptor.binding_hex)?;
    let mut binding = [0u8; 32];
    binding.copy_from_slice(&binding_bytes);
    Ok(AxtBinding::new(binding))
}
fn manifest_root(manifest: &TouchManifest) -> Result<[u8; 32], Box<dyn Error>> {
    let root = Hash::new(&to_bytes(manifest)?);
    let mut out = [0u8; 32];
    out.copy_from_slice(root.as_ref());
    Ok(out)
}
fn proof_blob_fixture(
    dsid: DataSpaceId,
    manifest_root: [u8; 32],
    da_commitment: Option<[u8; 32]>,
    proof: Vec<u8>,
    expiry_slot: u64,
    source_asset: Option<&AssetDefinitionId>,
    remote_spend_intent_commitments: Vec<[u8; 32]>,
) -> Result<ProofBlob, Box<dyn Error>> {
    let mut fastpq_binding = fixture_fastpq_binding(dsid);
    if let Some(asset) = source_asset {
        fastpq_binding.claim_type = "tx_predicate".to_owned();
        fastpq_binding.effect_binding = Some(AxtEffectBinding {
            destination_domain: None,
            destination_account_id: None,
            vault_account_id: None,
            issuance_account_id: None,
            source_asset_definition_id: Some(asset.to_string()),
            destination_asset_definition_id: None,
            source_amount_i64: None,
            destination_amount_i64: None,
        });
    }
    fastpq_binding.remote_spend_intent_commitments = remote_spend_intent_commitments;
    let payload = to_bytes(&AxtProofEnvelope {
        dsid,
        manifest_root,
        da_commitment,
        proof,
        fastpq_binding: Some(fastpq_binding),
        committed_amount: None,
        amount_commitment: None,
    })?;
    Ok(ProofBlob {
        payload,
        expiry_slot: Some(expiry_slot),
    })
}
fn transfer_handle_fixture(
    binding: AxtBinding,
    manifest_view_root: [u8; 32],
    proof: ProofBlob,
    asset_definition_id: AssetDefinitionId,
    alice: String,
    bob: String,
) -> AxtHandleFragment {
    let dsid = DataSpaceId::new(1);
    AxtHandleFragment {
        handle: signed_fixture_handle(
            AssetHandleDraft {
                asset_definition_id: asset_definition_id.clone(),
                scope: vec!["transfer".to_string()],
                subject: HandleSubject {
                    account: alice.clone(),
                    origin_dsid: Some(dsid),
                },
                budget: HandleBudget {
                    remaining: Quantity::from(2_u64),
                    per_use: Some(Quantity::from(1_u64)),
                },
                handle_era: 5,
                sub_nonce: 3,
                group_binding: GroupBinding {
                    composability_group_id: b"ds:reports".to_vec(),
                    epoch_id: 42,
                },
                target_lane: LaneId::new(4),
                axt_binding: binding,
                manifest_view_root,
                expiry_slot: 200,
                max_clock_skew_ms: Some(5_000),
            },
            dsid,
        ),
        intent: RemoteSpendIntent {
            asset_dsid: dsid,
            op: SpendOp {
                asset_definition_id,
                kind: "transfer".to_string(),
                from: alice,
                to: bob,
                amount: Some(Quantity::from(2_500_u64)),
            },
        },
        proof: Some(proof),
        amount: Some(Quantity::from(2_500_u64)),
        amount_commitment: None,
    }
}
fn lock_handle_fixture(
    binding: AxtBinding,
    manifest_view_root: [u8; 32],
    proof: ProofBlob,
    asset_definition_id: AssetDefinitionId,
    bob: String,
    carol: String,
) -> AxtHandleFragment {
    let dsid = DataSpaceId::new(7);
    AxtHandleFragment {
        handle: signed_fixture_handle(
            AssetHandleDraft {
                asset_definition_id: asset_definition_id.clone(),
                scope: vec!["transfer".to_string()],
                subject: HandleSubject {
                    account: bob.clone(),
                    origin_dsid: Some(dsid),
                },
                budget: HandleBudget {
                    remaining: Quantity::from(5_u64),
                    per_use: None,
                },
                handle_era: 9,
                sub_nonce: 1,
                group_binding: GroupBinding {
                    composability_group_id: b"ds:audits".to_vec(),
                    epoch_id: 7,
                },
                target_lane: LaneId::new(4),
                axt_binding: binding,
                manifest_view_root,
                expiry_slot: 160,
                max_clock_skew_ms: Some(2_000),
            },
            dsid,
        ),
        intent: RemoteSpendIntent {
            asset_dsid: dsid,
            op: SpendOp {
                asset_definition_id,
                kind: "transfer".to_string(),
                from: bob,
                to: carol,
                amount: Some(Quantity::from(9_001_u64)),
            },
        },
        proof: Some(proof),
        amount: Some(Quantity::from(9_001_u64)),
        amount_commitment: None,
    }
}
fn rejected_handle_fixtures(happy: &[AxtHandleFragment]) -> Vec<AxtHandleFragment> {
    let mut mismatched_binding = happy[0].clone();
    mismatched_binding.handle.axt_binding = AxtBinding::new([0u8; 32]);
    let mut stale_manifest = happy[1].clone();
    stale_manifest.handle.manifest_view_root = [0u8; 32];
    vec![mismatched_binding, stale_manifest]
}
fn build_envelope_fixture(
    descriptor: &DescriptorFixture,
) -> Result<EnvelopeFixture, Box<dyn Error>> {
    let binding = fixture_binding(descriptor)?;
    let dsid_one = DataSpaceId::new(1);
    let dsid_seven = DataSpaceId::new(7);
    let manifest_one = TouchManifest::from_read_write(["orders/root"], ["ledger/settlement"]);
    let manifest_seven = TouchManifest::from_read_write(
        ["reports/monthly"],
        ["aggregates/monthly", "audits/summary"],
    );
    let manifest_root_one = manifest_root(&manifest_one)?;
    let manifest_root_seven = manifest_root(&manifest_seven)?;
    let alice = seeded_account(0xA1);
    let bob = seeded_account(0xB2);
    let carol = seeded_account(0xC3);
    let transfer_asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("fixtures", "universal")?,
        "transfer".parse()?,
    );
    let lock_asset = AssetDefinitionId::derive_from_components(
        DomainId::try_new("fixtures", "universal")?,
        "lock".parse()?,
    );
    let transfer_amount = Quantity::from(2_500_u64);
    let transfer_commitment = compute_remote_spend_intent_commitment_v1(
        AxtHandleReplayKey::from_parts(dsid_one, binding.into_array(), 5, 3, LaneId::new(4)),
        &transfer_asset,
        "transfer",
        &alice,
        &bob,
        &transfer_amount,
    );
    let lock_amount = Quantity::from(9_001_u64);
    let lock_commitment = compute_remote_spend_intent_commitment_v1(
        AxtHandleReplayKey::from_parts(dsid_seven, binding.into_array(), 9, 1, LaneId::new(4)),
        &lock_asset,
        "transfer",
        &bob,
        &carol,
        &lock_amount,
    );
    let proof_one = proof_blob_fixture(
        dsid_one,
        manifest_root_one,
        Some([0x11; 32]),
        vec![0xAA, 0xBB, 0xCC, 0xDD],
        120,
        Some(&transfer_asset),
        vec![transfer_commitment],
    )?;
    let proof_seven = proof_blob_fixture(
        dsid_seven,
        manifest_root_seven,
        None,
        vec![0xFE, 0xED, 0xFA, 0xCE],
        98,
        Some(&lock_asset),
        vec![lock_commitment],
    )?;
    let proofs = vec![
        AxtProofFragment {
            dsid: dsid_one,
            proof: proof_one.clone(),
        },
        AxtProofFragment {
            dsid: dsid_seven,
            proof: proof_seven.clone(),
        },
    ];
    let happy_handles = vec![
        transfer_handle_fixture(
            binding,
            manifest_root_one,
            proof_one,
            transfer_asset,
            alice,
            bob.clone(),
        ),
        lock_handle_fixture(
            binding,
            manifest_root_seven,
            proof_seven,
            lock_asset,
            bob,
            carol,
        ),
    ];
    let rejects = rejected_handle_fixtures(&happy_handles);
    Ok(EnvelopeFixture {
        descriptor_hex: descriptor.descriptor_hex.clone(),
        binding_hex: descriptor.binding_hex.clone(),
        proofs,
        handles: HandleFixtures {
            happy: happy_handles,
            rejects,
        },
    })
}
fn build_poseidon_fixture() -> PoseidonConstantsFixture {
    let width3 = poseidon2_params_width3();
    let width6 = poseidon2_params_width6();
    let encode_rounds = |rounds: Vec<[[u8; 32]; 3]>| {
        rounds
            .into_iter()
            .map(|round| round.into_iter().map(encode).collect())
            .collect()
    };
    let encode_rounds6 = |rounds: Vec<[[u8; 32]; 6]>| {
        rounds
            .into_iter()
            .map(|round| round.into_iter().map(encode).collect())
            .collect()
    };
    let encode_mds = |mds: [[[u8; 32]; 3]; 3]| {
        mds.into_iter()
            .map(|row| row.into_iter().map(encode).collect())
            .collect()
    };
    let encode_mds6 = |mds: [[[u8; 32]; 6]; 6]| {
        mds.into_iter()
            .map(|row| row.into_iter().map(encode).collect())
            .collect()
    };
    PoseidonConstantsFixture {
        width3: PoseidonParamsFixture {
            round_constants: encode_rounds(width3.round_constants),
            mds: encode_mds(width3.mds),
        },
        width6: PoseidonParamsFixture {
            round_constants: encode_rounds6(width6.round_constants),
            mds: encode_mds6(width6.mds),
        },
    }
}
fn write_fixture<T: json::JsonSerialize>(
    path: &Path,
    value: &T,
    check_only: bool,
) -> Result<(), Box<dyn Error>> {
    let new_content = json::to_json_pretty(value)?;
    if check_only {
        let existing = fs::read_to_string(path)?;
        if existing.trim() != new_content.trim() {
            return Err(format!(
                "fixture {} is stale; run cargo run -p iroha_data_model --features dev-tools,test-fixtures --bin axt_fixtures",
                path.display()
            )
            .into());
        }
        return Ok(());
    }
    fs::write(path, new_content)?;
    Ok(())
}
fn main() -> Result<(), Box<dyn Error>> {
    let check_only = env::args().any(|arg| arg == "--check");
    let descriptor = build_descriptor_fixture()?;
    let envelope = build_envelope_fixture(&descriptor)?;
    let poseidon = build_poseidon_fixture();
    write_fixture(Path::new(DESCRIPTOR_FIXTURE_PATH), &descriptor, check_only)?;
    write_fixture(Path::new(ENVELOPE_FIXTURE_PATH), &envelope, check_only)?;
    write_fixture(Path::new(POSEIDON_FIXTURE_PATH), &poseidon, check_only)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::seeded_account;
    use iroha_data_model::account::AccountId;

    #[test]
    fn seeded_fixture_accounts_are_valid_and_distinct() {
        let alice = seeded_account(0xA1);
        let bob = seeded_account(0xB2);

        assert_ne!(alice, bob);
        AccountId::parse_encoded(&alice).expect("Alice account parses");
        AccountId::parse_encoded(&bob).expect("Bob account parses");
    }
}
