//! Canonical local wallet proving; the Core owner selects and verifies the circuit.

use super::*;
use iroha_core::zk::confidential::{ConfidentialProof, ConfidentialProver, ConfidentialTree};
use zeroize::{Zeroize, Zeroizing};

fn prover(network: &[u8], asset: &str, spend_key: &[u8]) -> napi::Result<ConfidentialProver> {
    let network = parse_transaction_network_id_bytes(network)?;
    let asset: AssetDefinitionId = asset.parse().map_err(norito_to_napi)?;
    if spend_key.len() != 32 {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "confidential spend key must be 32 bytes",
        ));
    }
    let mut key = Zeroizing::new([0; 32]);
    key.copy_from_slice(spend_key);
    ConfidentialProver::new(network, &asset, key).map_err(norito_to_napi)
}

// N-API owns these temporary strings; parsing guards erase them on success,
// rejection and unwind. JavaScript's original immutable strings remain caller-owned.
impl Zeroize for JsConfidentialTransferInputV2 {
    fn zeroize(&mut self) {
        self.amount.zeroize();
        self.rho_hex.zeroize();
        self.diversifier_hex.zeroize();
        self.leaf_index.zeroize();
    }
}
impl Zeroize for JsConfidentialTransferOutputV2 {
    fn zeroize(&mut self) {
        self.amount.zeroize();
        self.rho_hex.zeroize();
        self.owner_tag_hex.zeroize();
    }
}
impl Zeroize for JsConfidentialUnshieldOutputV3 {
    fn zeroize(&mut self) {
        self.amount.zeroize();
        self.rho_hex.zeroize();
    }
}

fn validate_shape(leaves: usize, inputs: usize, outputs: usize) -> napi::Result<()> {
    if leaves > (1 << confidential_v2::CONFIDENTIAL_TREE_DEPTH_V2) {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "confidential tree exceeds its capacity",
        ));
    }
    if !(1..=2).contains(&inputs) || !(1..=2).contains(&outputs) {
        return Err(napi::Error::new(
            napi::Status::InvalidArg,
            "confidential proving requires one or two input notes and valid output counts",
        ));
    }
    Ok(())
}

fn envelope(result: ConfidentialProof) -> JsConfidentialTransferProofEnvelopeV2 {
    JsConfidentialTransferProofEnvelopeV2 {
        nullifiers: result
            .nullifiers
            .into_iter()
            .map(|v| Buffer::from(v.to_vec()))
            .collect(),
        output_commitments: result
            .output_commitments
            .into_iter()
            .map(|v| Buffer::from(v.to_vec()))
            .collect(),
        root: Buffer::from(result.root.to_vec()),
        proof: Buffer::from(result.proof.bytes),
    }
}

/// Prove a local transfer using the canonical circuit and internally selected key.
#[napi]
#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
pub fn prove_confidential_transfer(
    network_id: Uint8Array,
    asset_definition_id: String,
    spend_key: Uint8Array,
    tree_commitments_hex: Vec<String>,
    inputs: Vec<JsConfidentialTransferInputV2>,
    outputs: Vec<JsConfidentialTransferOutputV2>,
    root_hex: String,
) -> napi::Result<JsConfidentialTransferProofEnvelopeV2> {
    let mut inputs = Zeroizing::new(inputs);
    let mut outputs = Zeroizing::new(outputs);
    let prover = prover(
        network_id.as_ref(),
        &asset_definition_id,
        spend_key.as_ref(),
    )?;
    validate_shape(tree_commitments_hex.len(), inputs.len(), outputs.len())?;
    let leaves = parse_confidential_tree_commitments(tree_commitments_hex)?;
    let inputs = parse_confidential_transfer_inputs_v2(core::mem::take(&mut *inputs))?;
    let outputs = parse_confidential_transfer_outputs_v2(core::mem::take(&mut *outputs))?;
    let root = parse_fixed_32_hex("root", &root_hex)?;
    prover
        .prove_transfer(
            ConfidentialTree::Commitments {
                root,
                leaves: &leaves,
            },
            inputs,
            outputs,
        )
        .map(envelope)
        .map_err(norito_to_napi)
}

/// Prove a local redemption, selecting full redemption or private change internally.
#[napi]
#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
pub fn prove_confidential_redemption(
    network_id: Uint8Array,
    asset_definition_id: String,
    spend_key: Uint8Array,
    tree_commitments_hex: Vec<String>,
    inputs: Vec<JsConfidentialTransferInputV2>,
    public_amount: String,
    root_hex: String,
    change: Option<JsConfidentialUnshieldOutputV3>,
) -> napi::Result<JsConfidentialTransferProofEnvelopeV2> {
    let mut inputs = Zeroizing::new(inputs);
    let mut change = Zeroizing::new(change);
    let prover = prover(
        network_id.as_ref(),
        &asset_definition_id,
        spend_key.as_ref(),
    )?;
    validate_shape(tree_commitments_hex.len(), inputs.len(), 1)?;
    let leaves = parse_confidential_tree_commitments(tree_commitments_hex)?;
    let inputs = parse_confidential_unshield_inputs_v2(core::mem::take(&mut *inputs))?;
    let amount = parse_confidential_amount_u128("publicAmount", &public_amount)?;
    let mut outputs = parse_confidential_unshield_outputs_v3(
        core::mem::take(&mut *change).into_iter().collect(),
    )?;
    let root = parse_fixed_32_hex("root", &root_hex)?;
    prover
        .prove_unshield(
            ConfidentialTree::Commitments {
                root,
                leaves: &leaves,
            },
            inputs,
            amount,
            outputs.pop(),
        )
        .map(envelope)
        .map_err(norito_to_napi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wallet_rejects_network_key_and_empty_spend_before_proof_work() {
        assert!(prover(&[0; 31], "", &[1; 32]).is_err());
        let asset = AssetDefinitionId::from_uuid_bytes([
            1, 2, 3, 4, 5, 6, 0x47, 8, 0x89, 10, 11, 12, 13, 14, 15, 16,
        ])
        .unwrap()
        .to_string();
        assert!(prover(&[1; 32], &asset, &[0; 32]).is_err());
        assert!(prover(&[1; 32], &asset, &[1; 31]).is_err());
        let transfer = prove_confidential_transfer(
            Uint8Array::from(vec![1; 32]),
            asset.clone(),
            Uint8Array::from(vec![1; 32]),
            vec![],
            vec![],
            vec![],
            "00".repeat(32),
        );
        assert!(
            transfer
                .err()
                .unwrap()
                .reason
                .contains("one or two input notes")
        );
        let redemption = prove_confidential_redemption(
            Uint8Array::from(vec![1; 32]),
            asset,
            Uint8Array::from(vec![1; 32]),
            vec![],
            vec![],
            "1".into(),
            "00".repeat(32),
            None,
        );
        assert!(
            redemption
                .err()
                .unwrap()
                .reason
                .contains("one or two input notes")
        );
    }

    #[test]
    fn ffi_private_strings_and_shape_controls_clear_all_owned_fields() {
        let mut input = JsConfidentialTransferInputV2 {
            amount: "7".into(),
            rho_hex: "11".repeat(32),
            diversifier_hex: Some("22".repeat(32)),
            leaf_index: 17,
        };
        input.zeroize();
        assert!(input.amount.is_empty() && input.rho_hex.is_empty());
        assert!(input.diversifier_hex.as_ref().is_none_or(String::is_empty));
        assert_eq!(input.leaf_index, 0);
        let mut output = JsConfidentialTransferOutputV2 {
            amount: "7".into(),
            rho_hex: "11".repeat(32),
            owner_tag_hex: "33".repeat(32),
        };
        output.zeroize();
        assert!(
            output.amount.is_empty()
                && output.rho_hex.is_empty()
                && output.owner_tag_hex.is_empty()
        );
        let mut change = JsConfidentialUnshieldOutputV3 {
            amount: "2".into(),
            rho_hex: "44".repeat(32),
        };
        change.zeroize();
        assert!(change.amount.is_empty() && change.rho_hex.is_empty());
        assert!(validate_shape(65536, 1, 1).is_ok());
        for counts in [(65537, 1, 1), (1, 0, 1), (1, 3, 1), (1, 1, 0), (1, 1, 3)] {
            assert!(validate_shape(counts.0, counts.1, counts.2).is_err());
        }
    }

    #[test]
    fn native_note_hex_and_amounts_reject_aliases_before_decoding() {
        assert_eq!(
            parse_fixed_32_hex("rho", &"ab".repeat(32)).unwrap(),
            [0xab; 32]
        );
        for invalid in [
            "AB".repeat(32),
            format!("0x{}", "ab".repeat(32)),
            format!(" {}", "ab".repeat(32)),
            "a".repeat(63),
            "x".repeat(64),
            "a".repeat(65),
        ] {
            assert!(parse_fixed_32_hex("rho", &invalid).is_err());
        }
        assert_eq!(parse_confidential_amount_u128("amount", "7").unwrap(), 7);
        for invalid in ["", " 7", "7 ", "+7", "-7", "7.0"] {
            assert!(parse_confidential_amount_u128("amount", invalid).is_err());
        }
    }

    #[test]
    fn note_parsers_preserve_fields_and_reject_late_private_field_errors() {
        let input = || JsConfidentialTransferInputV2 {
            amount: "17".into(),
            rho_hex: "11".repeat(32),
            diversifier_hex: Some("22".repeat(32)),
            leaf_index: 31,
        };
        let transfer = parse_confidential_transfer_inputs_v2(vec![input()]).unwrap();
        let redemption = parse_confidential_unshield_inputs_v2(vec![input()]).unwrap();
        assert_eq!(transfer[0].amount, 17);
        assert_eq!(transfer[0].rho, [0x11; 32]);
        assert_eq!(transfer[0].diversifier, [0x22; 32]);
        assert_eq!(transfer[0].leaf_index, 31);
        assert_eq!(redemption[0].amount, transfer[0].amount);
        assert_eq!(redemption[0].rho, transfer[0].rho);
        assert_eq!(redemption[0].diversifier, transfer[0].diversifier);
        assert_eq!(redemption[0].leaf_index, transfer[0].leaf_index);
        let invalid_input = || {
            let mut note = input();
            note.diversifier_hex = None;
            note
        };
        for error in [
            parse_confidential_transfer_inputs_v2(vec![input(), invalid_input()]).unwrap_err(),
            parse_confidential_unshield_inputs_v2(vec![input(), invalid_input()]).unwrap_err(),
        ] {
            assert_eq!(error.status, napi::Status::InvalidArg);
            assert_eq!(error.reason, "inputs[1].diversifier_hex is required");
        }
        let output = || JsConfidentialTransferOutputV2 {
            amount: "17".into(),
            rho_hex: "33".repeat(32),
            owner_tag_hex: "44".repeat(32),
        };
        let parsed = parse_confidential_transfer_outputs_v2(vec![output()]).unwrap();
        assert_eq!(parsed[0].amount, 17);
        assert_eq!(parsed[0].rho, [0x33; 32]);
        assert_eq!(parsed[0].owner_tag, [0x44; 32]);
        let mut invalid_output = output();
        invalid_output.owner_tag_hex = "private-invalid-tag".into();
        let error =
            parse_confidential_transfer_outputs_v2(vec![output(), invalid_output]).unwrap_err();
        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("outputs[1].owner_tag_hex"));
        assert!(!error.reason.contains("private-invalid-tag"));

        let change = || JsConfidentialUnshieldOutputV3 {
            amount: "3".into(),
            rho_hex: "55".repeat(32),
        };
        let parsed = parse_confidential_unshield_outputs_v3(vec![change()]).unwrap();
        assert_eq!(parsed[0].amount, 3);
        assert_eq!(parsed[0].rho, [0x55; 32]);
        let mut invalid_change = change();
        invalid_change.rho_hex = "private-invalid-rho".into();
        let error =
            parse_confidential_unshield_outputs_v3(vec![change(), invalid_change]).unwrap_err();
        assert_eq!(error.status, napi::Status::InvalidArg);
        assert!(error.reason.contains("outputs[1].rho_hex"));
        assert!(!error.reason.contains("private-invalid-rho"));
    }

    #[test]
    fn public_envelope_preserves_native_proof_and_output_order() {
        use iroha_core::zk::ProofRelation;
        let actual = envelope(ConfidentialProof {
            relation: ProofRelation::ConfidentialTransfer,
            proof: iroha_data_model::proof::ProofBox::new("halo2/ipa".into(), vec![7, 8]),
            root: [3; 32],
            nullifiers: vec![[4; 32], [5; 32]],
            output_commitments: vec![[6; 32]],
        });
        assert_eq!(actual.proof.as_ref(), &[7, 8]);
        assert_eq!(actual.root.as_ref(), &[3; 32]);
        assert_eq!(actual.nullifiers[1].as_ref(), &[5; 32]);
        assert_eq!(actual.output_commitments[0].as_ref(), &[6; 32]);
    }

    #[test]
    fn native_note_derivation_matches_core_and_redacts_late_decode_failures() {
        let asset = AssetDefinitionId::from_uuid_bytes([
            1, 2, 3, 4, 5, 6, 0x47, 8, 0x89, 10, 11, 12, 13, 14, 15, 16,
        ])
        .unwrap()
        .to_string();
        let key = [7; 32];
        let rho = [0x11; 32];
        let diversifier = [0x22; 32];
        let owner =
            confidential_v2::derive_confidential_owner_tag_v2_with_diversifier(&key, diversifier)
                .unwrap();
        let derived_owner = derive_confidential_owner_tag_v2(
            Uint8Array::from(key.to_vec()),
            Some(hex::encode(diversifier)),
        )
        .unwrap();
        assert_eq!(derived_owner.as_ref(), &owner);
        let note = derive_confidential_note_v2(
            asset.clone(),
            "17".into(),
            hex::encode(rho),
            hex::encode(owner),
        )
        .unwrap();
        assert_eq!(
            note.as_ref(),
            &confidential_v2::derive_confidential_note_v2(&asset, 17, rho, owner).unwrap()
        );
        let network = parse_transaction_network_id_bytes(&[1; 32]).unwrap();
        let nullifier = derive_confidential_nullifier_v2(
            Uint8Array::from(vec![1; 32]),
            asset.clone(),
            Uint8Array::from(key.to_vec()),
            hex::encode(rho),
        )
        .unwrap();
        assert_eq!(
            nullifier.as_ref(),
            &confidential_v2::derive_confidential_nullifier_v3(
                &key,
                rho,
                confidential_v2::derive_confidential_asset_tag_v3(&asset).unwrap(),
                confidential_v2::derive_confidential_network_tag_v3(&network).unwrap(),
            )
            .unwrap()
        );
        for error in [
            derive_confidential_owner_tag_v2(Uint8Array::from(key.to_vec()), None)
                .err()
                .expect("invalid input is rejected"),
            derive_confidential_note_v2(
                asset.clone(),
                "17".into(),
                hex::encode(rho),
                "private-invalid-owner".into(),
            )
            .err()
            .expect("invalid input is rejected"),
            derive_confidential_nullifier_v2(
                Uint8Array::from(vec![1; 32]),
                asset,
                Uint8Array::from(key.to_vec()),
                "private-invalid-rho".into(),
            )
            .err()
            .expect("invalid input is rejected"),
        ] {
            assert_eq!(error.status, napi::Status::InvalidArg);
            assert!(!error.reason.contains("private-invalid"));
        }
    }
}
