//! Regenerate the canonical AXT fixtures used across SDKs and guard scripts.
//!
//! Run with `cargo run -p iroha_data_model --features test-fixtures --bin axt_fixtures`
//! to refresh `tests/fixtures/*.json`. Use `--check` to verify the checked-in
//! fixtures are up to date without rewriting them.

use std::{env, error::Error, fs, path::Path};

use hex::{decode, encode};
use iroha_crypto::Hash;
use iroha_data_model::{
    nexus::{
        AssetHandle, AxtBinding, AxtDescriptorBuilder, AxtHandleFragment, AxtProofEnvelope,
        AxtProofFragment, AxtTouchFragment, DataSpaceId, GroupBinding, HandleBudget, HandleSubject,
        LaneId, ProofBlob, RemoteSpendIntent, SpendOp, TouchManifest, compute_descriptor_binding,
    },
    testing::axt::{
        DescriptorFixture, EnvelopeFixture, HandleFixtures, PoseidonConstantsFixture,
        PoseidonParamsFixture,
    },
};
use iroha_zkp_halo2::poseidon::{poseidon2_params_width3, poseidon2_params_width6};
use norito::{json, to_bytes};

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

fn encoded_account(public_key_hex: &str) -> String {
    iroha_data_model::account::AccountId::new(public_key_hex.parse().expect("public key"))
        .to_string()
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

fn build_envelope_fixture(
    descriptor: &DescriptorFixture,
) -> Result<EnvelopeFixture, Box<dyn Error>> {
    let binding_bytes = decode(&descriptor.binding_hex)?;
    let mut binding_array = [0u8; 32];
    binding_array.copy_from_slice(&binding_bytes);
    let binding = AxtBinding::new(binding_array);

    let manifest_root = |manifest: &TouchManifest| -> Result<[u8; 32], Box<dyn Error>> {
        let root = Hash::new(&to_bytes(manifest)?);
        let mut out = [0u8; 32];
        out.copy_from_slice(root.as_ref());
        Ok(out)
    };

    let dsid_one = DataSpaceId::new(1);
    let dsid_seven = DataSpaceId::new(7);
    let manifest_one = TouchManifest::from_read_write(["orders/root"], ["ledger/settlement"]);
    let manifest_seven = TouchManifest::from_read_write(
        ["reports/monthly"],
        ["aggregates/monthly", "audits/summary"],
    );
    let manifest_root_one = manifest_root(&manifest_one)?;
    let manifest_root_seven = manifest_root(&manifest_seven)?;

    let proof_one_payload = to_bytes(&AxtProofEnvelope {
        dsid: dsid_one,
        manifest_root: manifest_root_one,
        da_commitment: Some([0x11; 32]),
        proof: vec![0xAA, 0xBB, 0xCC, 0xDD],
        committed_amount: None,
        amount_commitment: None,
    })?;
    let proof_seven_payload = to_bytes(&AxtProofEnvelope {
        dsid: dsid_seven,
        manifest_root: manifest_root_seven,
        da_commitment: None,
        proof: vec![0xFE, 0xED, 0xFA, 0xCE],
        committed_amount: None,
        amount_commitment: None,
    })?;

    let proof_one = ProofBlob {
        payload: proof_one_payload.clone(),
        expiry_slot: Some(120),
    };
    let proof_seven = ProofBlob {
        payload: proof_seven_payload.clone(),
        expiry_slot: Some(98),
    };

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

    let alice =
        encoded_account("ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03");
    let bob =
        encoded_account("ed012004FF5B81046DDCCF19E2E451C45DFB6F53759D4EB30FA2EFA807284D1CC33016");
    let carol =
        encoded_account("ed0120ED77765E503B45FF9C059A1C19BF1DDE82C60432B7C2D01F7FCD75F5F9F3C07C");

    let happy_handles = vec![
        AxtHandleFragment {
            handle: AssetHandle {
                scope: vec!["transfer".to_string()],
                subject: HandleSubject {
                    account: alice.clone(),
                    origin_dsid: Some(dsid_one),
                },
                budget: HandleBudget {
                    remaining: 2,
                    per_use: Some(1),
                },
                handle_era: 5,
                sub_nonce: 3,
                group_binding: GroupBinding {
                    composability_group_id: b"ds:reports".to_vec(),
                    epoch_id: 42,
                },
                target_lane: LaneId::new(4),
                axt_binding: binding,
                manifest_view_root: manifest_root_one,
                expiry_slot: 200,
                max_clock_skew_ms: Some(5_000),
            },
            intent: RemoteSpendIntent {
                asset_dsid: dsid_one,
                op: SpendOp {
                    kind: "transfer".to_string(),
                    from: alice,
                    to: bob.clone(),
                    amount: "2500".to_string(),
                },
            },
            proof: Some(proof_one),
            amount: 2_500,
            amount_commitment: None,
        },
        AxtHandleFragment {
            handle: AssetHandle {
                scope: vec!["lock".to_string()],
                subject: HandleSubject {
                    account: bob.clone(),
                    origin_dsid: Some(dsid_seven),
                },
                budget: HandleBudget {
                    remaining: 5,
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
                manifest_view_root: manifest_root_seven,
                expiry_slot: 160,
                max_clock_skew_ms: Some(2_000),
            },
            intent: RemoteSpendIntent {
                asset_dsid: dsid_seven,
                op: SpendOp {
                    kind: "lock".to_string(),
                    from: bob,
                    to: carol,
                    amount: "9001".to_string(),
                },
            },
            proof: Some(proof_seven),
            amount: 9_001,
            amount_commitment: None,
        },
    ];

    let mut mismatched_binding = happy_handles[0].clone();
    mismatched_binding.handle.axt_binding = AxtBinding::new([0u8; 32]);

    let mut stale_manifest = happy_handles[1].clone();
    stale_manifest.handle.manifest_view_root = [0u8; 32];

    let rejects = vec![mismatched_binding, stale_manifest];

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
                "fixture {} is stale; run cargo run -p iroha_data_model --features test-fixtures --bin axt_fixtures",
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
