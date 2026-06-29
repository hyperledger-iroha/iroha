//! Temporary decoder for live SCCP message bundles.

use std::io::{self, Read};

use iroha_sccp::{
    NexusSccpMessageProofV1, SCCP_DOMAIN_SORA, SccpSourceAdapterProofV1,
    build_nexus_sccp_message_transparent_proof_with_source_verifier_material_allow_unready,
    decode_nexus_bridge_finality_proof, decode_sccp_source_chain_proof_envelope,
    decode_sccp_source_consensus_proof, sccp_evm_family_mainnet_source_verifier_material_v1,
    sccp_message_source_domain, sccp_message_target_domain, sccp_message_transparent_public_inputs,
    sccp_message_transparent_public_inputs_with_source_verifier_material,
    sccp_source_adapter_ready_with_material_for_domain,
    sccp_source_verifier_material_from_evidence,
    sccp_source_verifier_material_from_message_bundle_evidence, sccp_source_verifier_material_hash,
    sccp_source_verifier_material_is_production_ready,
    sccp_source_verifier_material_uses_builtin_placeholder_components,
    verified_sccp_message_source_chain_proof_envelope_for_production_with_material,
    verify_message_bundle_structure, verify_message_bundle_structure_with_source_verifier_material,
    verify_nexus_bridge_finality_proof_structure, verify_sccp_payload_structure,
    verify_sccp_source_chain_proof_envelope_production_with_material,
    verify_sccp_source_chain_proof_envelope_structure,
    verify_sccp_source_chain_proof_envelope_structure_with_material,
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
    println!(
        "source_domain={}",
        sccp_message_source_domain(&bundle.payload)
    );
    println!(
        "target_domain={}",
        sccp_message_target_domain(&bundle.payload)
    );
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
    if sccp_message_source_domain(&bundle.payload) != SCCP_DOMAIN_SORA {
        let Some(source_proof) = decode_sccp_source_chain_proof_envelope(&bundle.finality_proof)
        else {
            println!("source_proof_decode=false");
            return;
        };
        println!("source_proof_decode=true");
        println!("source_proof.version={}", source_proof.version);
        println!("source_proof.source_domain={}", source_proof.source_domain);
        println!("source_proof.target_domain={}", source_proof.target_domain);
        println!("source_proof.chain={}", source_proof.source_chain);
        println!(
            "source_proof.source_proof_plan={:?}",
            source_proof.source_proof_plan
        );
        println!(
            "source_proof.finality_model={:?}",
            source_proof.finality_model
        );
        println!(
            "source_proof.message_id={}",
            hex_encode(&source_proof.message_id)
        );
        println!(
            "source_proof.payload_hash={}",
            hex_encode(&source_proof.payload_hash)
        );
        println!(
            "source_proof.source_event_digest={}",
            hex_encode(&source_proof.source_event_digest)
        );
        println!(
            "source_proof.commitment_root={}",
            hex_encode(&source_proof.commitment_root)
        );
        println!("source_proof.height={}", source_proof.finality_height);
        println!(
            "source_proof.block_hash={}",
            hex_encode(&source_proof.finality_block_hash)
        );
        println!(
            "source_proof.finalized_header_hash={}",
            hex_encode(&source_proof.finalized_header_hash)
        );
        println!(
            "source_proof.receipt_or_message_root={}",
            hex_encode(&source_proof.receipt_or_message_root)
        );
        println!(
            "source_proof.consensus_proof_len={}",
            source_proof.consensus_proof.len()
        );
        println!(
            "source_proof.message_inclusion_proof_len={}",
            source_proof.message_inclusion_proof.len()
        );
        println!(
            "source_proof.inclusion_branch_len={}",
            source_proof.inclusion_branch.len()
        );
        println!(
            "source_proof.structure={}",
            verify_sccp_source_chain_proof_envelope_structure(&source_proof)
        );
        println!(
            "bundle.structure={}",
            verify_message_bundle_structure(&bundle)
        );
        let material_from_bundle =
            sccp_source_verifier_material_from_message_bundle_evidence(&bundle);
        println!("material.from_bundle={}", material_from_bundle.is_some());
        let consensus = decode_sccp_source_consensus_proof(&source_proof.consensus_proof);
        println!("consensus.decode={}", consensus.is_some());
        if let Some(consensus) = consensus {
            if let SccpSourceAdapterProofV1::BscValidatorSetReceipt(adapter) =
                &consensus.adapter_proof
            {
                println!(
                    "bsc.validator_set_hash={}",
                    hex_encode(&adapter.validator_set_hash)
                );
                println!(
                    "bsc.commit_seal_hash={}",
                    hex_encode(&adapter.commit_seal_hash)
                );
                println!(
                    "bsc.receipt_trie_proof_hash={}",
                    hex_encode(&adapter.receipt_trie_proof_hash)
                );
                println!("bsc.receipts_root={}", hex_encode(&adapter.receipts_root));
            }
            let material =
                sccp_source_verifier_material_from_evidence(&consensus.verifier_evidence);
            println!("material.from_evidence={}", material.is_some());
            if let Some(material) = material {
                println!("material.source_domain={}", material.source_domain);
                println!("material.placeholder={}", material.placeholder_material);
                println!(
                    "material.hash={}",
                    hex_encode(&sccp_source_verifier_material_hash(&material))
                );
                println!(
                    "material.uses_builtin={}",
                    sccp_source_verifier_material_uses_builtin_placeholder_components(&material)
                );
                println!(
                    "material.production_ready={}",
                    sccp_source_verifier_material_is_production_ready(&material)
                );
                println!(
                    "material.ready_with_material={}",
                    sccp_source_adapter_ready_with_material_for_domain(
                        material.source_domain,
                        &material
                    )
                );
                println!(
                    "material.source_trust_anchor_id={}",
                    material.source_trust_anchor_id
                );
                println!(
                    "material.source_trust_anchor_hash={}",
                    hex_encode(&material.source_trust_anchor_hash)
                );
                println!(
                    "material.consensus_verifier_id={}",
                    material.consensus_verifier_id
                );
                println!(
                    "material.consensus_verifier_hash={}",
                    hex_encode(&material.consensus_verifier_hash)
                );
                println!(
                    "material.message_inclusion_verifier_id={}",
                    material.message_inclusion_verifier_id
                );
                println!(
                    "material.message_inclusion_verifier_hash={}",
                    hex_encode(&material.message_inclusion_verifier_hash)
                );
                println!(
                    "material.finality_policy_id={}",
                    material.finality_policy_id
                );
                println!(
                    "material.finality_policy_hash={}",
                    hex_encode(&material.finality_policy_hash)
                );
                println!(
                    "material.source_bridge_emitter_id={}",
                    material.source_bridge_emitter_id
                );
                println!(
                    "material.source_bridge_emitter_address_len={}",
                    material.source_bridge_emitter_address.len()
                );
                println!(
                    "material.source_bridge_emitter_code_hash={}",
                    hex_encode(&material.source_bridge_emitter_code_hash)
                );
                println!(
                    "material.source_bridge_network_id={}",
                    hex_encode(&material.source_bridge_network_id)
                );
                println!(
                    "material.source_bridge_owner_address={}",
                    hex_encode(&material.source_bridge_owner_address)
                );
                println!(
                    "material.source_bridge_config_hash={}",
                    hex_encode(&material.source_bridge_config_hash)
                );
                println!(
                    "evidence.source_bridge_network_id={}",
                    hex_encode(&consensus.verifier_evidence.source_bridge_network_id)
                );
                println!(
                    "evidence.source_bridge_owner_address={}",
                    hex_encode(&consensus.verifier_evidence.source_bridge_owner_address)
                );
                println!(
                    "evidence.source_bridge_config_hash={}",
                    hex_encode(&consensus.verifier_evidence.source_bridge_config_hash)
                );
                println!(
                    "evidence.source_adapter_deployment_hash={}",
                    hex_encode(&consensus.verifier_evidence.source_adapter_deployment_hash)
                );
                println!(
                    "evidence.source_adapter_deployment_receipt_hash={}",
                    hex_encode(
                        &consensus
                            .verifier_evidence
                            .source_adapter_deployment_receipt_hash
                    )
                );
                if let Some(mut expected) =
                    sccp_evm_family_mainnet_source_verifier_material_v1(material.source_domain)
                {
                    expected.source_bridge_emitter_address =
                        material.source_bridge_emitter_address.clone();
                    expected.source_bridge_emitter_code_hash =
                        material.source_bridge_emitter_code_hash;
                    println!(
                        "expected.source_trust_anchor_hash={}",
                        hex_encode(&expected.source_trust_anchor_hash)
                    );
                    println!(
                        "expected.consensus_verifier_hash={}",
                        hex_encode(&expected.consensus_verifier_hash)
                    );
                    println!(
                        "expected.message_inclusion_verifier_hash={}",
                        hex_encode(&expected.message_inclusion_verifier_hash)
                    );
                    println!(
                        "expected.finality_policy_hash={}",
                        hex_encode(&expected.finality_policy_hash)
                    );
                    println!(
                        "expected.hash={}",
                        hex_encode(&sccp_source_verifier_material_hash(&expected))
                    );
                    println!(
                        "expected.production_ready={}",
                        sccp_source_verifier_material_is_production_ready(&expected)
                    );
                }
                println!(
                    "source_proof.structure.material={}",
                    verify_sccp_source_chain_proof_envelope_structure_with_material(
                        &source_proof,
                        &material
                    )
                );
                println!(
                    "source_proof.production.material={}",
                    verify_sccp_source_chain_proof_envelope_production_with_material(
                        &source_proof,
                        &material
                    )
                );
                println!(
                    "bundle.structure.material={}",
                    verify_message_bundle_structure_with_source_verifier_material(
                        &bundle, &material
                    )
                );
                println!(
                    "public_inputs.material_some={}",
                    sccp_message_transparent_public_inputs_with_source_verifier_material(
                        &bundle, &material
                    )
                    .is_some()
                );
                println!(
                    "production.with_material={}",
                    verified_sccp_message_source_chain_proof_envelope_for_production_with_material(
                        &bundle, &material
                    )
                    .is_some()
                );
                let artifact =
                    build_nexus_sccp_message_transparent_proof_with_source_verifier_material_allow_unready(
                        &bundle,
                        &material,
                        true,
                    );
                println!("artifact.material={}", artifact.is_some());
                if let Some(artifact) = artifact {
                    println!(
                        "artifact.submission_kind={}",
                        artifact.submission_package.submission_kind
                    );
                }
            }
        }
        return;
    }
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
    println!("finality.header_len={}", finality.block_header_bytes.len());
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
