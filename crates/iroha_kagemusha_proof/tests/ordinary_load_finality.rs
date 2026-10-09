//! Direct BLS admission of a captured receipt followed by genuine monetary Load.
//! Synthetic ledger execution identities remain component-only; the certificate
//! and every native Load stage are independently verified.

#![allow(clippy::duplicate_mod)] // The shared Bootstrap chain retains its exact source types.

#[path = "common/bootstrap.rs"]
mod bootstrap;
#[path = "common/bootstrap_objects.rs"]
#[allow(dead_code)]
mod bootstrap_objects;
mod common;
/// The same native Load four-A/three-W producer used by subsequent chain fixtures.
#[path = "a_load_recursive.rs"]
pub mod load_chain;
#[path = "common/ordinary_load_fixture.rs"]
mod load_fixture;
#[path = "common/load_objects.rs"]
#[allow(dead_code)]
mod load_objects;

use ff::Field;
use iroha_data_model::sumeragi_finality::{
    FinalityValidator, SumeragiCommitCertificateV1, SumeragiCommitVerifierV1,
    SumeragiFinalityVerifier, genesis_epoch,
};
use iroha_pasta::Fp;
use std::{fs, path::PathBuf, sync::Arc};

const CAPTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/kagemusha/ordinary_first_load_receipt_v1.json"
);
fn first_capture() -> String {
    let metadata =
        fs::symlink_metadata(CAPTURE).expect("exact separately generated first-Load capture");
    assert!(metadata.file_type().is_file() && metadata.len() <= 1 << 20);
    fs::read_to_string(CAPTURE).unwrap()
}

#[test]
fn exact_first_load_capture_matches_bootstrap_identity_without_relabeling() {
    load_fixture::check_first_capture(&first_capture());
}

fn verified_receipt(capture: &str) -> [u8; 282] {
    let json: norito::json::Value = norito::json::from_str(capture).unwrap();
    let raw = |name: &str| {
        let text = json[name].as_str().unwrap();
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>()
    };
    let genesis =
        iroha_data_model::block::decode_framed_signed_block(&raw("signed_genesis_wire_hex"))
            .unwrap();
    let epoch = genesis_epoch(&genesis).unwrap();
    let roster = epoch
        .committee
        .iter()
        .map(|member| FinalityValidator {
            public_key: member.validator.public_key().clone(),
            proof_of_possession: member.proof_of_possession.clone(),
        })
        .collect();
    let native =
        SumeragiFinalityVerifier::new(&genesis, json["chain_id"].as_str().unwrap(), roster)
            .unwrap();
    let certificate = SumeragiCommitCertificateV1 {
        consensus_header: raw("consensus_header_frame_hex"),
        commit_qc: raw("commit_qc_frame_hex"),
        result_preimage: raw("result_preimage_hex"),
    };
    let mut verifier = SumeragiCommitVerifierV1::new(&native).unwrap();
    let verified = verifier.verify(&certificate).unwrap();
    assert_eq!(verified.height(), 2);
    let receipt =
        iroha_data_model::isi::kagemusha_wallet::KagemushaWalletLoadReceiptV1::decode_canonical(
            &raw("receipt_frame_hex"),
        )
        .unwrap();
    let event =
        iroha_data_model::events::data::kagemusha::KagemushaLoadCommittedV1::from_receipt(&receipt)
            .unwrap();
    let boxed = iroha_data_model::events::EventBox::Data(
        iroha_data_model::events::data::DataEvent::KagemushaLoadCommitted(event).into(),
    );
    let path = iroha_crypto::MerkleProof::from_audit_path(0, vec![]);
    assert!(path.verify(
        &iroha_crypto::HashOf::new(&boxed),
        verified.execution().event_commitment.as_ref().unwrap()
    ));
    let transcript = receipt.transcript().unwrap();
    assert_eq!(transcript.as_slice(), raw("receipt_transcript_hex"));
    assert_eq!(receipt.block_height, verified.height());
    transcript
}

#[test]
fn first_load_certificate_and_receipt_verify_without_proving_keys() {
    let receipt = verified_receipt(&first_capture());
    assert_eq!(receipt.len(), 282);
}

#[test]
#[ignore = "explicit genuine four-A/three-W Load proof construction after direct BLS admission"]
fn exact_first_load_native_finality_then_native_load() {
    let output = PathBuf::from(
        std::env::var_os("KAGEMUSHA_LOAD_OUTPUT")
            .expect("exclusive native Load artifact output directory"),
    );
    let receipt = verified_receipt(&first_capture());
    let rooted = load_chain::bootstrap_outer::compact_bootstrap::rooted_compact_bootstrap();
    let fixture: load_chain::LoadFixture =
        Arc::new(move |rooted| load_fixture::build(rooted, &receipt, &output));
    let terminal = load_chain::authenticated_load(&rooted, &fixture);
    assert_eq!(
        terminal.state.core[iroha_kagemusha_proof::witness::core_index::BALANCE],
        Fp::from(100)
    );
    assert_eq!(
        terminal.state.core[iroha_kagemusha_proof::witness::core_index::NEXT_LOAD],
        Fp::ONE
    );
}
