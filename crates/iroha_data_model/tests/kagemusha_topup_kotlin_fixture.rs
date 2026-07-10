//! Cross-language regression: the Kotlin SDK's Norito encoder must produce a
//! `KagemushaRecursiveSpendTopUpRequestV1` archive that the Rust decoder accepts,
//! exactly as the Torii offline issuer does (`norito::decode_from_bytes`) when
//! validating a wallet top-up request.
//!
//! The fixture `fixtures/kagemusha_topup_request_kotlin.bin` is produced by the
//! Kotlin test `kotlin top-up archive matches the shared Rust fixture` in
//! `KagemushaRecursiveSpendRequestCodecsTest`; regenerate it there if the encoder
//! or its sample inputs change. The same test class produces the transfer and
//! unshield verifier-record fixtures asserted below.

use iroha_crypto::Hash;
use iroha_data_model::{
    confidential::ConfidentialStatus,
    offline::KagemushaRecursiveSpendTopUpRequestV1,
    proof::{VerifyingKeyBox, VerifyingKeyRecord},
    zk::BackendTag,
};
use sha2::{Digest, Sha256};

const TRANSFER_CIRCUIT_ID: &str = "halo2/pasta/ipa/anon-transfer-2x2-merkle16-poseidon-diversified";
const UNSHIELD_CIRCUIT_ID: &str =
    "halo2/pasta/ipa/anon-unshield-2in-1change-merkle16-poseidon-diversified";
const TRANSFER_SCHEMA: &[u8] = br#"{"schema":"confidential_transfer_v2","public_inputs":["input_commitment_0","input_commitment_1","nullifier_0","nullifier_1","output_commitment_0","output_commitment_1","root","asset_tag","chain_tag"]}"#;
const UNSHIELD_SCHEMA: &[u8] = br#"{"schema":"confidential_unshield_v3","public_inputs":["input_commitment_0","input_commitment_1","nullifier_0","nullifier_1","change_commitment_0","root","public_amount","asset_tag","chain_tag"]}"#;

#[test]
fn kotlin_topup_archive_decodes_in_rust() {
    let bytes = include_bytes!("fixtures/kagemusha_topup_request_kotlin.bin");
    let request: KagemushaRecursiveSpendTopUpRequestV1 = norito::decode_from_bytes(bytes)
        .unwrap_or_else(|err| {
            panic!("Kotlin-encoded top-up archive rejected by Rust decoder: {err:?}")
        });

    assert_eq!(request.amount.to_string(), "17");
    assert_eq!(request.init_request.current_note.amount.to_string(), "17");
    assert_eq!(
        request.init_request.current_note.note_commitment,
        [0x21; 32]
    );
    assert_eq!(
        request.init_request.current_note.spend_nullifier,
        [0x22; 32]
    );
    assert!(request.init_request.lineage_verifier_key.is_none());
    assert!(request.init_request.lineage_proving_key_archive.is_none());
    assert!(request.init_request.block_height.is_none());

    let record_bundle = &request.init_request.record_bundle;
    assert_eq!(
        record_bundle.bundle.chain_id.as_str(),
        "kagemusha-test-chain"
    );
    assert_eq!(record_bundle.bundle.steps.len(), 1);
    assert_eq!(record_bundle.verifier_records.len(), 1);
    assert_eq!(request.asset.definition(), &record_bundle.bundle.asset);
    let step = &record_bundle.bundle.steps[0];
    assert_eq!(step.root_before, [0x31; 32]);
    assert_eq!(step.root_after, [0x32; 32]);
    assert_eq!(step.input_nullifiers, vec![[0x43; 32]]);
    assert_eq!(step.output_commitments, vec![[0x44; 32]]);
    let verifier = &record_bundle.verifier_records[0];
    assert_eq!(verifier.id.backend.to_string(), "halo2/ipa");
    assert_eq!(
        verifier.id.name,
        "kagemusha-test-anon-transfer-2x2-merkle16-poseidon-diversified"
    );
    assert_eq!(verifier.record.version, 1);
    assert_eq!(verifier.record.circuit_id, TRANSFER_CIRCUIT_ID);
    assert_eq!(verifier.record.namespace, "offline_kagemusha");
    assert_eq!(verifier.record.backend, BackendTag::Halo2IpaPasta);
    assert_eq!(verifier.record.curve, "pallas");
    assert_eq!(verifier.record.status, ConfidentialStatus::Active);
    assert_eq!(verifier.record.max_proof_bytes, 192 * 1024);
    assert!(verifier.record.key.is_some());

    let reencoded = norito::to_bytes(&request).expect("re-encode Kotlin top-up fixture");
    assert_eq!(
        reencoded.as_slice(),
        bytes.as_slice(),
        "Rust canonical re-encoding drifted from Kotlin top-up bytes"
    );
}

#[test]
fn kotlin_transfer_verifier_record_decodes_in_rust() {
    assert_verifier_record_fixture(
        include_bytes!("fixtures/kagemusha_transfer_v2_verifier_record_kotlin.bin"),
        3,
        TRANSFER_CIRCUIT_ID,
        TRANSFER_SCHEMA,
    );
}

#[test]
fn kotlin_unshield_verifier_record_decodes_in_rust() {
    assert_verifier_record_fixture(
        include_bytes!("fixtures/kagemusha_unshield_v3_verifier_record_kotlin.bin"),
        1,
        UNSHIELD_CIRCUIT_ID,
        UNSHIELD_SCHEMA,
    );
}

fn assert_verifier_record_fixture(
    bytes: &[u8],
    version: u32,
    circuit_id: &str,
    public_inputs_schema: &[u8],
) {
    let decoded: VerifyingKeyRecord = norito::decode_from_bytes(bytes)
        .unwrap_or_else(|err| panic!("Kotlin verifier record rejected by Rust decoder: {err:?}"));
    let verifier_key = zk1_verifier_key(circuit_id);
    let expected = VerifyingKeyRecord {
        version,
        circuit_id: circuit_id.to_owned(),
        owner_manifest_id: None,
        namespace: "offline_kagemusha".to_owned(),
        backend: BackendTag::Halo2IpaPasta,
        curve: "pallas".to_owned(),
        public_inputs_schema_hash: Hash::new(public_inputs_schema).into(),
        commitment: verifier_key_commitment(&verifier_key),
        vk_len: verifier_key
            .len()
            .try_into()
            .expect("fixture key length fits u32"),
        max_proof_bytes: 192 * 1024,
        gas_schedule_id: Some("halo2_default".to_owned()),
        metadata_uri_cid: None,
        vk_bytes_cid: None,
        activation_height: None,
        withdraw_height: None,
        key: Some(VerifyingKeyBox::new(
            "halo2/ipa".parse().expect("valid verifier backend"),
            verifier_key,
        )),
        status: ConfidentialStatus::Active,
    };
    assert_eq!(decoded, expected);
    let reencoded = norito::to_bytes(&decoded).expect("re-encode Kotlin verifier record");
    assert_eq!(
        reencoded.as_slice(),
        bytes,
        "Rust canonical re-encoding drifted from Kotlin verifier-record bytes"
    );
}

fn zk1_verifier_key(circuit_id: &str) -> Vec<u8> {
    let mut key = b"ZK1\0".to_vec();
    append_tlv(&mut key, b"CID1", circuit_id.as_bytes());
    append_tlv(&mut key, b"IPAK", &[7, 0, 0, 0]);
    append_tlv(&mut key, b"H2VK", &(1_u8..=32).collect::<Vec<_>>());
    key
}

fn append_tlv(output: &mut Vec<u8>, tag: &[u8; 4], payload: &[u8]) {
    output.extend_from_slice(tag);
    output.extend_from_slice(
        &u32::try_from(payload.len())
            .expect("fixture TLV length fits u32")
            .to_le_bytes(),
    );
    output.extend_from_slice(payload);
}

fn verifier_key_commitment(verifier_key: &[u8]) -> [u8; 32] {
    let backend = b"halo2/ipa";
    let mut digest = Sha256::new();
    digest.update(b"iroha:zk:v1:vk");
    digest.update((backend.len() as u64).to_be_bytes());
    digest.update(backend);
    digest.update((verifier_key.len() as u64).to_be_bytes());
    digest.update(verifier_key);
    digest.finalize().into()
}
