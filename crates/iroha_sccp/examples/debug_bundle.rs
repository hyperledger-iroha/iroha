//! Temporary decoder for live SCCP message bundles.

use std::io::{self, Read};

use iroha_sccp::{
    decode_nexus_bridge_finality_proof, sccp_message_source_domain, sccp_message_target_domain,
    sccp_message_transparent_public_inputs, verify_nexus_bridge_finality_proof_structure,
    verify_sccp_payload_structure, NexusSccpMessageProofV1,
};

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn main() {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .expect("read bundle json");
    let bundle: NexusSccpMessageProofV1 =
        norito::json::from_str(&input).expect("decode bundle json");
    println!("bundle.version={}", bundle.version);
    println!("commitment.version={}", bundle.commitment.version);
    println!("source_domain={}", sccp_message_source_domain(&bundle.payload));
    println!("target_domain={}", sccp_message_target_domain(&bundle.payload));
    println!(
        "payload_structure={}",
        verify_sccp_payload_structure(&bundle.payload)
    );
    println!("message_id={}", hex_encode(&bundle.commitment.message_id));
    println!("commitment_root={}", hex_encode(&bundle.commitment_root));
    println!(
        "public_inputs_some={}",
        sccp_message_transparent_public_inputs(&bundle).is_some()
    );
    let Some(finality) = decode_nexus_bridge_finality_proof(&bundle.finality_proof) else {
        println!("finality_decode=false");
        return;
    };
    println!("finality_decode=true");
    println!("finality.version={}", finality.version);
    println!("finality.chain_id={}", finality.chain_id);
    println!("finality.height={}", finality.height);
    println!("finality.block_hash={}", hex_encode(&finality.block_hash));
    println!(
        "finality.commitment_root={}",
        hex_encode(&finality.commitment_root)
    );
    println!(
        "finality.header_len={}",
        finality.block_header_bytes.len()
    );
    println!(
        "finality.structure={}",
        verify_nexus_bridge_finality_proof_structure(&finality)
    );
    let qc = &finality.commit_qc;
    println!("qc.version={}", qc.version);
    println!("qc.phase={:?}", qc.phase);
    println!("qc.height={}", qc.height);
    println!("qc.view={}", qc.view);
    println!("qc.epoch={}", qc.epoch);
    println!("qc.mode_tag={}", qc.mode_tag);
    println!(
        "qc.subject_block_hash={}",
        hex_encode(&qc.subject_block_hash)
    );
    println!(
        "qc.validator_set_hash_version={}",
        qc.validator_set_hash_version
    );
    println!("qc.validators={}", qc.validator_public_keys.len());
    println!("qc.pops={}", qc.validator_set_pops.len());
    println!("qc.signers_bitmap={}", hex_encode(&qc.signers_bitmap));
    println!("qc.bls_sig_len={}", qc.bls_aggregate_signature.len());
}
