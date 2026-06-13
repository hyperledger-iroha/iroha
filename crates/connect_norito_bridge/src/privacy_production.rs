//! Real proving/verification dispatch for the two production privacy algorithms
//! (`confidential-transfer-v2`, `unshield`). This module exists to keep the witness
//! decoding and prover wiring out of the oversized crate root, and because the
//! decoded witness carries secret material (`spend_key`, note `rho`, amounts) that
//! must be zeroized the moment proving completes — a self-contained module makes that
//! lifetime auditable.
//!
//! The bridge request only carries an opaque `witness: Vec<u8>`. We define a
//! deterministic Norito wire format here (`PrivacyConfidentialWitnessV1`) that the
//! consuming SDK encodes; it mirrors the typed argument set the existing in-tree
//! prover wrappers take. Numeric amounts are little-endian `u128`, byte commitments
//! are length-checked `Vec<u8>` so the wire shape stays stable across the FFI boundary.

use iroha_core::zk::{
    ZK_BACKEND_HALO2_IPA,
    confidential_v2::{
        CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID, CONFIDENTIAL_TREE_CAPACITY_V2,
        CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID, ConfidentialTransferInputV2,
        ConfidentialTransferOutputV2, ConfidentialUnshieldInputV2, ConfidentialUnshieldOutputV3,
        build_confidential_transfer_proof_v2, build_confidential_unshield_proof_v3,
        confidential_transfer_v2_vk_box, confidential_unshield_v3_vk_box,
    },
    verify_backend,
};
use iroha_data_model::{ChainId, proof::ProofBox};
use zeroize::Zeroize;

use crate::{
    PRIVACY_FFI_ERROR_INVALID_REQUEST, PRIVACY_FFI_ERROR_PROVING_FAILED, PRIVACY_FFI_STATUS_OK,
    PRIVACY_FFI_VERSION_V1, PrivacyProofOperationV1, PrivacyProofRequestV1, PrivacyProofResultV1,
    privacy_failure_result, privacy_production_disabled_result,
};

const PRIVACY_CONFIDENTIAL_TRANSFER_V2_ALGORITHM_ID: &str = "confidential-transfer-v2";
const PRIVACY_CONFIDENTIAL_UNSHIELD_ALGORITHM_ID: &str = "unshield";
const PRIVACY_CONFIDENTIAL_BYTES_32: usize = 32;
const PRIVACY_CONFIDENTIAL_SPEND_KEY_BYTES: usize = 32;
const PRIVACY_CONFIDENTIAL_MAX_INPUTS_V2: usize = 2;
const PRIVACY_CONFIDENTIAL_MAX_TRANSFER_OUTPUTS_V2: usize = 2;
const PRIVACY_CONFIDENTIAL_MAX_UNSHIELD_CHANGE_OUTPUTS_V3: usize = 1;

#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
struct PrivacyConfidentialNoteWitnessV1 {
    amount: u128,
    rho: Vec<u8>,
    diversifier: Vec<u8>,
    leaf_index: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
struct PrivacyConfidentialTransferOutputWitnessV1 {
    amount: u128,
    rho: Vec<u8>,
    owner_tag: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
struct PrivacyConfidentialUnshieldChangeWitnessV1 {
    amount: u128,
    rho: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
struct PrivacyConfidentialWitnessV1 {
    chain_id: String,
    asset_definition_id: String,
    spend_key: Vec<u8>,
    tree_commitments: Vec<Vec<u8>>,
    inputs: Vec<PrivacyConfidentialNoteWitnessV1>,
    transfer_outputs: Vec<PrivacyConfidentialTransferOutputWitnessV1>,
    unshield_change: Vec<PrivacyConfidentialUnshieldChangeWitnessV1>,
    public_amount: u128,
    root_hint: Vec<u8>,
}

impl Zeroize for PrivacyConfidentialNoteWitnessV1 {
    fn zeroize(&mut self) {
        self.amount.zeroize();
        self.rho.zeroize();
        self.diversifier.zeroize();
        self.leaf_index.zeroize();
    }
}

impl Zeroize for PrivacyConfidentialTransferOutputWitnessV1 {
    fn zeroize(&mut self) {
        self.amount.zeroize();
        self.rho.zeroize();
        self.owner_tag.zeroize();
    }
}

impl Zeroize for PrivacyConfidentialUnshieldChangeWitnessV1 {
    fn zeroize(&mut self) {
        self.amount.zeroize();
        self.rho.zeroize();
    }
}

impl Zeroize for PrivacyConfidentialWitnessV1 {
    fn zeroize(&mut self) {
        self.chain_id.zeroize();
        self.asset_definition_id.zeroize();
        self.spend_key.zeroize();
        for commitment in &mut self.tree_commitments {
            commitment.zeroize();
        }
        for input in &mut self.inputs {
            input.zeroize();
        }
        for output in &mut self.transfer_outputs {
            output.zeroize();
        }
        for change in &mut self.unshield_change {
            change.zeroize();
        }
        self.public_amount.zeroize();
        self.root_hint.zeroize();
    }
}

fn privacy_fixed_32(
    field: &str,
    bytes: &[u8],
) -> Result<[u8; PRIVACY_CONFIDENTIAL_BYTES_32], String> {
    bytes
        .try_into()
        .map_err(|_| format!("{field} must be {PRIVACY_CONFIDENTIAL_BYTES_32} bytes"))
}

fn privacy_require_32(field: &str, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() != PRIVACY_CONFIDENTIAL_BYTES_32 {
        return Err(format!(
            "{field} must be {PRIVACY_CONFIDENTIAL_BYTES_32} bytes"
        ));
    }
    Ok(())
}

// Witness shapes are validated by the privacy_validate_* helpers before any
// secret-bearing copy exists, so the construction helpers below must stay
// infallible: an early `?` between copy construction and the post-prover wipe
// would drop secrets un-zeroized.
fn privacy_array_32(bytes: &[u8]) -> [u8; PRIVACY_CONFIDENTIAL_BYTES_32] {
    bytes.try_into().expect("length validated before copying")
}

fn privacy_validate_tree_commitments(raw: &[Vec<u8>]) -> Result<(), String> {
    if raw.len() > CONFIDENTIAL_TREE_CAPACITY_V2 {
        return Err(format!(
            "tree commitments exceed confidential v2 capacity of {CONFIDENTIAL_TREE_CAPACITY_V2}"
        ));
    }
    raw.iter()
        .try_for_each(|commitment| privacy_require_32("tree commitment", commitment))
}

fn privacy_validate_note_witnesses(
    raw: &[PrivacyConfidentialNoteWitnessV1],
    tree_len: usize,
) -> Result<(), String> {
    if raw.is_empty() || raw.len() > PRIVACY_CONFIDENTIAL_MAX_INPUTS_V2 {
        return Err("confidential witness must include one or two inputs".to_owned());
    }
    for (index, note) in raw.iter().enumerate() {
        privacy_require_32("input rho", &note.rho)?;
        privacy_require_32("input diversifier", &note.diversifier)?;
        let leaf_index = usize::try_from(note.leaf_index)
            .map_err(|_| "input leaf_index exceeds usize".to_owned())?;
        if leaf_index >= tree_len {
            return Err(format!(
                "inputs[{index}].leaf_index must reference tree_commitments"
            ));
        }
        for (previous_index, previous) in raw[..index].iter().enumerate() {
            if previous.leaf_index == note.leaf_index {
                return Err(format!(
                    "inputs[{index}].leaf_index duplicates inputs[{previous_index}]"
                ));
            }
            if previous.rho == note.rho {
                return Err(format!(
                    "inputs[{index}].rho duplicates inputs[{previous_index}]"
                ));
            }
        }
    }
    Ok(())
}

fn privacy_validate_transfer_outputs(
    raw: &[PrivacyConfidentialTransferOutputWitnessV1],
) -> Result<(), String> {
    if raw.is_empty() || raw.len() > PRIVACY_CONFIDENTIAL_MAX_TRANSFER_OUTPUTS_V2 {
        return Err("confidential transfer witness must include one or two outputs".to_owned());
    }
    for output in raw {
        privacy_require_32("output rho", &output.rho)?;
        privacy_require_32("output owner_tag", &output.owner_tag)?;
    }
    Ok(())
}

fn privacy_validate_unshield_change(
    raw: &[PrivacyConfidentialUnshieldChangeWitnessV1],
) -> Result<(), String> {
    if raw.len() > PRIVACY_CONFIDENTIAL_MAX_UNSHIELD_CHANGE_OUTPUTS_V3 {
        return Err(
            "confidential unshield v3 witness supports at most one private change output"
                .to_owned(),
        );
    }
    raw.iter()
        .try_for_each(|change| privacy_require_32("change rho", &change.rho))
}

fn privacy_tree_commitments(raw: &[Vec<u8>]) -> Vec<[u8; PRIVACY_CONFIDENTIAL_BYTES_32]> {
    raw.iter()
        .map(|commitment| privacy_array_32(commitment))
        .collect()
}

fn privacy_transfer_inputs(
    raw: &[PrivacyConfidentialNoteWitnessV1],
) -> Vec<ConfidentialTransferInputV2> {
    raw.iter()
        .map(|note| ConfidentialTransferInputV2 {
            amount: note.amount,
            rho: privacy_array_32(&note.rho),
            diversifier: privacy_array_32(&note.diversifier),
            leaf_index: usize::try_from(note.leaf_index).expect("validated before copying"),
        })
        .collect()
}

fn privacy_unshield_inputs(
    raw: &[PrivacyConfidentialNoteWitnessV1],
) -> Vec<ConfidentialUnshieldInputV2> {
    raw.iter()
        .map(|note| ConfidentialUnshieldInputV2 {
            amount: note.amount,
            rho: privacy_array_32(&note.rho),
            diversifier: privacy_array_32(&note.diversifier),
            leaf_index: usize::try_from(note.leaf_index).expect("validated before copying"),
        })
        .collect()
}

fn privacy_transfer_outputs(
    raw: &[PrivacyConfidentialTransferOutputWitnessV1],
) -> Vec<ConfidentialTransferOutputV2> {
    raw.iter()
        .map(|output| ConfidentialTransferOutputV2 {
            amount: output.amount,
            rho: privacy_array_32(&output.rho),
            owner_tag: privacy_array_32(&output.owner_tag),
        })
        .collect()
}

fn privacy_unshield_change(
    raw: &[PrivacyConfidentialUnshieldChangeWitnessV1],
) -> Vec<ConfidentialUnshieldOutputV3> {
    raw.iter()
        .map(|change| ConfidentialUnshieldOutputV3 {
            amount: change.amount,
            rho: privacy_array_32(&change.rho),
        })
        .collect()
}

fn privacy_decode_witness(witness: &[u8]) -> Result<PrivacyConfidentialWitnessV1, String> {
    norito::decode_from_bytes(witness)
        .map_err(|err| format!("privacy witness is not a valid v1 archive: {err}"))
}

fn privacy_common_witness_checks(witness: &PrivacyConfidentialWitnessV1) -> Result<(), String> {
    if witness.spend_key.len() != PRIVACY_CONFIDENTIAL_SPEND_KEY_BYTES {
        return Err(format!(
            "confidential spend key must be {PRIVACY_CONFIDENTIAL_SPEND_KEY_BYTES} bytes"
        ));
    }
    Ok(())
}

fn privacy_validate_transfer_witness_shape(
    witness: &PrivacyConfidentialWitnessV1,
) -> Result<(), String> {
    privacy_common_witness_checks(witness)?;
    if witness.public_amount != 0 {
        return Err("confidential transfer witness must not include public_amount".to_owned());
    }
    if !witness.unshield_change.is_empty() {
        return Err(
            "confidential transfer witness must not include unshield change outputs".to_owned(),
        );
    }
    privacy_validate_tree_commitments(&witness.tree_commitments)?;
    privacy_validate_note_witnesses(&witness.inputs, witness.tree_commitments.len())?;
    privacy_validate_transfer_outputs(&witness.transfer_outputs)?;
    privacy_fixed_32("root_hint", &witness.root_hint)?;
    Ok(())
}

fn privacy_validate_unshield_witness_shape(
    witness: &PrivacyConfidentialWitnessV1,
) -> Result<(), String> {
    privacy_common_witness_checks(witness)?;
    if !witness.transfer_outputs.is_empty() {
        return Err("confidential unshield witness must not include transfer outputs".to_owned());
    }
    privacy_validate_tree_commitments(&witness.tree_commitments)?;
    privacy_validate_note_witnesses(&witness.inputs, witness.tree_commitments.len())?;
    privacy_validate_unshield_change(&witness.unshield_change)?;
    privacy_fixed_32("root_hint", &witness.root_hint)?;
    Ok(())
}

fn privacy_parse_chain_id(witness: &PrivacyConfidentialWitnessV1) -> Result<ChainId, String> {
    witness
        .chain_id
        .parse()
        .map_err(|err| format!("invalid chain id: {err}"))
}

fn privacy_zeroize_transfer_inputs(inputs: &mut [ConfidentialTransferInputV2]) {
    for input in inputs {
        input.amount.zeroize();
        input.rho.zeroize();
        input.diversifier.zeroize();
        input.leaf_index.zeroize();
    }
}

fn privacy_zeroize_transfer_outputs(outputs: &mut [ConfidentialTransferOutputV2]) {
    for output in outputs {
        output.amount.zeroize();
        output.rho.zeroize();
        output.owner_tag.zeroize();
    }
}

fn privacy_zeroize_unshield_inputs(inputs: &mut [ConfidentialUnshieldInputV2]) {
    for input in inputs {
        input.amount.zeroize();
        input.rho.zeroize();
        input.diversifier.zeroize();
        input.leaf_index.zeroize();
    }
}

fn privacy_zeroize_unshield_change(change: &mut [ConfidentialUnshieldOutputV3]) {
    for output in change {
        output.amount.zeroize();
        output.rho.zeroize();
    }
}

fn privacy_build_transfer_proof(witness: &PrivacyConfidentialWitnessV1) -> Result<Vec<u8>, String> {
    privacy_validate_transfer_witness_shape(witness)?;
    let chain_id = privacy_parse_chain_id(witness)?;
    let root_hint = privacy_fixed_32("root_hint", &witness.root_hint)?;
    let vk_box = confidential_transfer_v2_vk_box()?;
    // All fallible steps are behind us: the typed copies below carry secret
    // rho/diversifier/amounts, so nothing may `?`-return until they are wiped.
    let mut tree_commitments = privacy_tree_commitments(&witness.tree_commitments);
    let mut inputs = privacy_transfer_inputs(&witness.inputs);
    let mut outputs = privacy_transfer_outputs(&witness.transfer_outputs);
    let outcome = build_confidential_transfer_proof_v2(
        &chain_id,
        &witness.asset_definition_id,
        &witness.spend_key,
        &tree_commitments,
        &inputs,
        &outputs,
        root_hint,
        CONFIDENTIAL_TRANSFER_V2_CIRCUIT_ID,
        &vk_box,
    );
    privacy_zeroize_transfer_inputs(&mut inputs);
    privacy_zeroize_transfer_outputs(&mut outputs);
    tree_commitments.zeroize();
    Ok(outcome?.proof.bytes)
}

fn privacy_build_unshield_proof(witness: &PrivacyConfidentialWitnessV1) -> Result<Vec<u8>, String> {
    privacy_validate_unshield_witness_shape(witness)?;
    let chain_id = privacy_parse_chain_id(witness)?;
    let root_hint = privacy_fixed_32("root_hint", &witness.root_hint)?;
    let vk_box = confidential_unshield_v3_vk_box()?;
    // All fallible steps are behind us: the typed copies below carry secret
    // rho/diversifier/amounts, so nothing may `?`-return until they are wiped.
    let mut tree_commitments = privacy_tree_commitments(&witness.tree_commitments);
    let mut inputs = privacy_unshield_inputs(&witness.inputs);
    let mut change = privacy_unshield_change(&witness.unshield_change);
    let outcome = build_confidential_unshield_proof_v3(
        &chain_id,
        &witness.asset_definition_id,
        &witness.spend_key,
        &tree_commitments,
        &inputs,
        &change,
        witness.public_amount,
        root_hint,
        CONFIDENTIAL_UNSHIELD_V3_CIRCUIT_ID,
        &vk_box,
    );
    privacy_zeroize_unshield_inputs(&mut inputs);
    privacy_zeroize_unshield_change(&mut change);
    tree_commitments.zeroize();
    Ok(outcome?.proof.bytes)
}

fn privacy_success_result(
    request: &PrivacyProofRequestV1,
    proof: Vec<u8>,
    verified: bool,
) -> PrivacyProofResultV1 {
    let result = PrivacyProofResultV1 {
        version: PRIVACY_FFI_VERSION_V1,
        status: PRIVACY_FFI_STATUS_OK,
        error_code: 0,
        message: String::new(),
        algorithm_id: request.algorithm_id.clone(),
        entrypoint: request.entrypoint.clone(),
        vk_ref: request.vk_ref.clone(),
        // The proof envelope is the authoritative carrier of the public
        // instances; the request field is unauthenticated and must not be
        // echoed as if verified.
        public_inputs: Vec::new(),
        proof,
        verified,
    };
    debug_assert!(privacy_success_result_invariants_hold(&result));
    result
}

fn privacy_success_result_invariants_hold(result: &PrivacyProofResultV1) -> bool {
    result.version == PRIVACY_FFI_VERSION_V1
        && result.status == PRIVACY_FFI_STATUS_OK
        && result.error_code == 0
        && result.message.is_empty()
}

fn privacy_dispatch_build(
    request: &PrivacyProofRequestV1,
    validator: fn(&PrivacyConfidentialWitnessV1) -> Result<(), String>,
    builder: fn(&PrivacyConfidentialWitnessV1) -> Result<Vec<u8>, String>,
) -> PrivacyProofResultV1 {
    let mut witness = match privacy_decode_witness(&request.witness) {
        Ok(witness) => witness,
        Err(message) => {
            return privacy_failure_result(
                PRIVACY_FFI_ERROR_INVALID_REQUEST,
                &message,
                Some(request),
            );
        }
    };
    if let Err(message) = validator(&witness) {
        witness.zeroize();
        return privacy_failure_result(PRIVACY_FFI_ERROR_INVALID_REQUEST, &message, Some(request));
    }
    let outcome = builder(&witness);
    witness.zeroize();
    match outcome {
        // Build does not assert chain admission, so it does not claim `verified`.
        Ok(proof) => privacy_success_result(request, proof, false),
        Err(message) => {
            privacy_failure_result(PRIVACY_FFI_ERROR_PROVING_FAILED, &message, Some(request))
        }
    }
}

fn privacy_dispatch_verify(
    request: &PrivacyProofRequestV1,
    vk_box: VkBoxResult,
) -> PrivacyProofResultV1 {
    let vk_box = match vk_box {
        Ok(vk_box) => vk_box,
        Err(message) => {
            return privacy_failure_result(
                PRIVACY_FFI_ERROR_PROVING_FAILED,
                &message,
                Some(request),
            );
        }
    };
    let proof = ProofBox::new(ZK_BACKEND_HALO2_IPA.to_owned(), request.proof.clone());
    let verified = verify_backend(ZK_BACKEND_HALO2_IPA, &proof, Some(&vk_box));
    if !verified {
        return privacy_failure_result(
            PRIVACY_FFI_ERROR_PROVING_FAILED,
            "privacy proof verification failed",
            Some(request),
        );
    }
    privacy_success_result(request, request.proof.clone(), true)
}

type VkBoxResult = Result<iroha_data_model::proof::VerifyingKeyBox, String>;

/// Entry from `privacy_result_for_request`: the request has already passed every
/// structural guard. We only need to route the two in-scope algorithms to their
/// real prover/verifier; anything else stays fail-closed.
pub(crate) fn privacy_production_dispatch(
    request: &PrivacyProofRequestV1,
    operation: PrivacyProofOperationV1,
) -> PrivacyProofResultV1 {
    match (request.algorithm_id.as_str(), operation) {
        (PRIVACY_CONFIDENTIAL_TRANSFER_V2_ALGORITHM_ID, PrivacyProofOperationV1::Build) => {
            privacy_dispatch_build(
                request,
                privacy_validate_transfer_witness_shape,
                privacy_build_transfer_proof,
            )
        }
        (PRIVACY_CONFIDENTIAL_UNSHIELD_ALGORITHM_ID, PrivacyProofOperationV1::Build) => {
            privacy_dispatch_build(
                request,
                privacy_validate_unshield_witness_shape,
                privacy_build_unshield_proof,
            )
        }
        (PRIVACY_CONFIDENTIAL_TRANSFER_V2_ALGORITHM_ID, PrivacyProofOperationV1::Verify) => {
            privacy_dispatch_verify(request, confidential_transfer_v2_vk_box())
        }
        (PRIVACY_CONFIDENTIAL_UNSHIELD_ALGORITHM_ID, PrivacyProofOperationV1::Verify) => {
            privacy_dispatch_verify(request, confidential_unshield_v3_vk_box())
        }
        _ => privacy_production_disabled_result(request),
    }
}

/// Test-only fixtures shared with the crate-root FFI-boundary tests. They live at
/// module scope (not inside the inner `tests` module) so the full-pipeline tests in
/// `lib.rs` can build a request archive that carries a real, decodable witness and
/// therefore exercises the status=OK path through the exported FFI entrypoints.
#[cfg(test)]
pub(crate) mod test_fixtures {
    use iroha_core::zk::confidential_v2::{
        compute_confidential_root_v2, derive_confidential_diversifier_v2,
        derive_confidential_note_v2, derive_confidential_owner_tag_v2_with_diversifier,
    };

    use super::*;

    const TEST_CHAIN_ID: &str = "809574f5-fee7-5e69-bfcf-52451e42d50f";
    const TEST_ASSET_ID: &str = "xor#universal";

    pub(super) fn valid_transfer_witness() -> PrivacyConfidentialWitnessV1 {
        let spend_key = [0x11_u8; 32];
        let input_rho = [0x22_u8; 32];
        let output_rho = [0x44_u8; 32];
        let input_diversifier = derive_confidential_diversifier_v2(b"input");
        let input_owner_tag =
            derive_confidential_owner_tag_v2_with_diversifier(&spend_key, input_diversifier)
                .expect("input owner tag");
        let input_commitment =
            derive_confidential_note_v2(TEST_ASSET_ID, 7, input_rho, input_owner_tag)
                .expect("input commitment");
        let tree_commitments = vec![input_commitment];
        let root = compute_confidential_root_v2(&tree_commitments).expect("root");

        let recipient_key = [0x33_u8; 32];
        let output_diversifier = derive_confidential_diversifier_v2(b"recipient");
        let output_owner_tag =
            derive_confidential_owner_tag_v2_with_diversifier(&recipient_key, output_diversifier)
                .expect("output owner tag");

        PrivacyConfidentialWitnessV1 {
            chain_id: TEST_CHAIN_ID.to_owned(),
            asset_definition_id: TEST_ASSET_ID.to_owned(),
            spend_key: spend_key.to_vec(),
            tree_commitments: tree_commitments.iter().map(|node| node.to_vec()).collect(),
            inputs: vec![PrivacyConfidentialNoteWitnessV1 {
                amount: 7,
                rho: input_rho.to_vec(),
                diversifier: input_diversifier.to_vec(),
                leaf_index: 0,
            }],
            transfer_outputs: vec![PrivacyConfidentialTransferOutputWitnessV1 {
                amount: 7,
                rho: output_rho.to_vec(),
                owner_tag: output_owner_tag.to_vec(),
            }],
            unshield_change: Vec::new(),
            public_amount: 0,
            root_hint: root.to_vec(),
        }
    }

    pub(super) fn valid_unshield_witness() -> PrivacyConfidentialWitnessV1 {
        let spend_key = [0xA1_u8; 32];
        let input_rho = [0xA2_u8; 32];
        let change_rho = [0xA3_u8; 32];
        let input_diversifier = derive_confidential_diversifier_v2(b"unshield-v3-input");
        let input_owner_tag =
            derive_confidential_owner_tag_v2_with_diversifier(&spend_key, input_diversifier)
                .expect("input owner tag");
        let input_commitment =
            derive_confidential_note_v2(TEST_ASSET_ID, 9, input_rho, input_owner_tag)
                .expect("input commitment");
        let tree_commitments = vec![input_commitment];
        let root = compute_confidential_root_v2(&tree_commitments).expect("root");

        PrivacyConfidentialWitnessV1 {
            chain_id: TEST_CHAIN_ID.to_owned(),
            asset_definition_id: TEST_ASSET_ID.to_owned(),
            spend_key: spend_key.to_vec(),
            tree_commitments: tree_commitments.iter().map(|node| node.to_vec()).collect(),
            inputs: vec![PrivacyConfidentialNoteWitnessV1 {
                amount: 9,
                rho: input_rho.to_vec(),
                diversifier: input_diversifier.to_vec(),
                leaf_index: 0,
            }],
            transfer_outputs: Vec::new(),
            // input 9 = public 5 + private change 4
            unshield_change: vec![PrivacyConfidentialUnshieldChangeWitnessV1 {
                amount: 4,
                rho: change_rho.to_vec(),
            }],
            public_amount: 5,
            root_hint: root.to_vec(),
        }
    }

    pub(super) fn overflowing_unshield_witness() -> PrivacyConfidentialWitnessV1 {
        let spend_key = [0xA1_u8; 32];
        let input_0_rho = [0xB1_u8; 32];
        let input_1_rho = [0xB2_u8; 32];
        let input_0_diversifier =
            derive_confidential_diversifier_v2(b"unshield-v3-overflow-input-0");
        let input_1_diversifier =
            derive_confidential_diversifier_v2(b"unshield-v3-overflow-input-1");
        let input_0_owner_tag =
            derive_confidential_owner_tag_v2_with_diversifier(&spend_key, input_0_diversifier)
                .expect("input 0 owner tag");
        let input_1_owner_tag =
            derive_confidential_owner_tag_v2_with_diversifier(&spend_key, input_1_diversifier)
                .expect("input 1 owner tag");
        let tree_commitments = vec![
            derive_confidential_note_v2(TEST_ASSET_ID, u128::MAX, input_0_rho, input_0_owner_tag)
                .expect("input 0 commitment"),
            derive_confidential_note_v2(TEST_ASSET_ID, 1, input_1_rho, input_1_owner_tag)
                .expect("input 1 commitment"),
        ];
        let root = compute_confidential_root_v2(&tree_commitments).expect("root");

        PrivacyConfidentialWitnessV1 {
            chain_id: TEST_CHAIN_ID.to_owned(),
            asset_definition_id: TEST_ASSET_ID.to_owned(),
            spend_key: spend_key.to_vec(),
            tree_commitments: tree_commitments.iter().map(|node| node.to_vec()).collect(),
            inputs: vec![
                PrivacyConfidentialNoteWitnessV1 {
                    amount: u128::MAX,
                    rho: input_0_rho.to_vec(),
                    diversifier: input_0_diversifier.to_vec(),
                    leaf_index: 0,
                },
                PrivacyConfidentialNoteWitnessV1 {
                    amount: 1,
                    rho: input_1_rho.to_vec(),
                    diversifier: input_1_diversifier.to_vec(),
                    leaf_index: 1,
                },
            ],
            transfer_outputs: Vec::new(),
            unshield_change: Vec::new(),
            public_amount: 0,
            root_hint: root.to_vec(),
        }
    }

    pub(super) fn encode_witness(witness: &PrivacyConfidentialWitnessV1) -> Vec<u8> {
        norito::to_bytes(witness).expect("encode confidential witness")
    }

    pub(crate) fn valid_transfer_witness_bytes() -> Vec<u8> {
        encode_witness(&valid_transfer_witness())
    }

    pub(crate) fn valid_unshield_witness_bytes() -> Vec<u8> {
        encode_witness(&valid_unshield_witness())
    }
}

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};

    use super::test_fixtures::{
        encode_witness, overflowing_unshield_witness, valid_transfer_witness,
        valid_unshield_witness,
    };
    use super::*;
    use crate::{
        PRIVACY_FFI_ERROR_INVALID_REQUEST, PRIVACY_FFI_ERROR_PRODUCTION_DISABLED,
        PRIVACY_FFI_STATUS_ERROR, PRIVACY_FFI_VERSION_V1,
    };

    fn build_request(algorithm_id: &str, witness: Vec<u8>) -> PrivacyProofRequestV1 {
        PrivacyProofRequestV1 {
            algorithm_id: algorithm_id.to_owned(),
            entrypoint: "buildConfidentialProof".to_owned(),
            vk_ref: "halo2-ipa-pasta:vk_ref".to_owned(),
            public_inputs: b"public-inputs".to_vec(),
            witness,
            proof: Vec::new(),
        }
    }

    fn verify_request(algorithm_id: &str, proof: Vec<u8>) -> PrivacyProofRequestV1 {
        PrivacyProofRequestV1 {
            algorithm_id: algorithm_id.to_owned(),
            entrypoint: "buildConfidentialProof".to_owned(),
            vk_ref: "halo2-ipa-pasta:vk_ref".to_owned(),
            public_inputs: b"public-inputs".to_vec(),
            witness: Vec::new(),
            proof,
        }
    }

    fn assert_invalid_witness(
        algorithm_id: &str,
        witness: PrivacyConfidentialWitnessV1,
        expected_message: &str,
    ) {
        let result = privacy_production_dispatch(
            &build_request(algorithm_id, encode_witness(&witness)),
            PrivacyProofOperationV1::Build,
        );
        assert_eq!(result.status, PRIVACY_FFI_STATUS_ERROR);
        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert!(
            result.message.contains(expected_message),
            "expected `{expected_message}` in `{}`",
            result.message
        );
        assert!(result.proof.is_empty());
        assert!(!result.verified);
    }

    fn sdk_transfer_witness_contract_fixture() -> PrivacyConfidentialWitnessV1 {
        PrivacyConfidentialWitnessV1 {
            chain_id: "809574f5-fee7-5e69-bfcf-52451e42d50f".to_owned(),
            asset_definition_id: "xor#universal".to_owned(),
            spend_key: vec![0x11; 32],
            tree_commitments: vec![vec![0x10; 32]],
            inputs: vec![PrivacyConfidentialNoteWitnessV1 {
                amount: 7,
                rho: vec![0x22; 32],
                diversifier: vec![0x33; 32],
                leaf_index: 0,
            }],
            transfer_outputs: vec![PrivacyConfidentialTransferOutputWitnessV1 {
                amount: 7,
                rho: vec![0x44; 32],
                owner_tag: vec![0x55; 32],
            }],
            unshield_change: Vec::new(),
            public_amount: 0,
            root_hint: vec![0x66; 32],
        }
    }

    fn sdk_transfer_witness_contract_archive() -> Vec<u8> {
        let sdk_archive_base64 = "TlJUMAAAfsqLqoiuWPS/Oqqw1+q/rAC2AQAAAAAAAB8kbLx8YYiUAgAAAAAAAAAAJSQ4MDk1NzRmNS1mZWU3LTVlNjktYmZjZi01MjQ1MWU0MmQ1MGYODXhvciN1bml2ZXJzYWwoIAAAAAAAAAARERERERERERERERERERERERERERERERERERERERERETEBAAAAAAAAACggAAAAAAAAABAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQdQEAAAAAAAAAbBAHAAAAAAAAAAAAAAAAAAAAKCAAAAAAAAAAIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIiIoIAAAAAAAAAAzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMzMwgAAAAAAAAAAGwBAAAAAAAAAGMQBwAAAAAAAAAAAAAAAAAAACggAAAAAAAAAEREREREREREREREREREREREREREREREREREREREREREKCAAAAAAAAAAVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVUIAAAAAAAAAAAQAAAAAAAAAAAAAAAAAAAAACggAAAAAAAAAGZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZmZm";
        BASE64_STANDARD
            .decode(sdk_archive_base64)
            .expect("SDK golden witness archive base64 decodes")
    }

    #[test]
    fn confidential_witness_schema_path_matches_sdk_contract() {
        assert_eq!(
            <PrivacyConfidentialWitnessV1 as norito::NoritoSerialize>::schema_hash(),
            norito::core::schema_hash_for_name(
                "connect_norito_bridge::privacy_production::PrivacyConfidentialWitnessV1",
            ),
        );
    }

    #[test]
    fn sdk_confidential_witness_archive_decodes_to_native_contract() {
        let sdk_archive = sdk_transfer_witness_contract_archive();
        let native_archive = encode_witness(&sdk_transfer_witness_contract_fixture());
        assert_eq!(sdk_archive, native_archive);

        let witness = privacy_decode_witness(&sdk_archive).expect("SDK witness decodes natively");

        assert_eq!(witness.chain_id, "809574f5-fee7-5e69-bfcf-52451e42d50f");
        assert_eq!(witness.asset_definition_id, "xor#universal");
        assert_eq!(witness.spend_key, vec![0x11; 32]);
        assert_eq!(witness.tree_commitments, vec![vec![0x10; 32]]);
        assert_eq!(witness.inputs.len(), 1);
        assert_eq!(witness.inputs[0].amount, 7);
        assert_eq!(witness.inputs[0].rho, vec![0x22; 32]);
        assert_eq!(witness.inputs[0].diversifier, vec![0x33; 32]);
        assert_eq!(witness.inputs[0].leaf_index, 0);
        assert_eq!(witness.transfer_outputs.len(), 1);
        assert_eq!(witness.transfer_outputs[0].amount, 7);
        assert_eq!(witness.transfer_outputs[0].rho, vec![0x44; 32]);
        assert_eq!(witness.transfer_outputs[0].owner_tag, vec![0x55; 32]);
        assert!(witness.unshield_change.is_empty());
        assert_eq!(witness.public_amount, 0);
        assert_eq!(witness.root_hint, vec![0x66; 32]);
        privacy_validate_transfer_witness_shape(&witness).expect("SDK witness shape is accepted");
    }

    #[test]
    fn transfer_build_then_verify_round_trips() {
        let witness = encode_witness(&valid_transfer_witness());
        let build = privacy_production_dispatch(
            &build_request(PRIVACY_CONFIDENTIAL_TRANSFER_V2_ALGORITHM_ID, witness),
            PrivacyProofOperationV1::Build,
        );
        assert_eq!(build.status, PRIVACY_FFI_STATUS_OK);
        assert_eq!(build.error_code, 0);
        assert!(!build.proof.is_empty());
        assert!(!build.verified);
        assert!(build.message.is_empty());
        assert!(build.public_inputs.is_empty());

        let verify = privacy_production_dispatch(
            &verify_request(
                PRIVACY_CONFIDENTIAL_TRANSFER_V2_ALGORITHM_ID,
                build.proof.clone(),
            ),
            PrivacyProofOperationV1::Verify,
        );
        assert_eq!(verify.status, PRIVACY_FFI_STATUS_OK);
        assert_eq!(verify.error_code, 0);
        assert!(verify.verified);
        assert!(verify.public_inputs.is_empty());
    }

    #[test]
    fn unshield_build_then_verify_round_trips() {
        let witness = encode_witness(&valid_unshield_witness());
        let build = privacy_production_dispatch(
            &build_request(PRIVACY_CONFIDENTIAL_UNSHIELD_ALGORITHM_ID, witness),
            PrivacyProofOperationV1::Build,
        );
        assert_eq!(build.status, PRIVACY_FFI_STATUS_OK);
        assert_eq!(build.error_code, 0);
        assert!(!build.proof.is_empty());
        assert!(!build.verified);
        assert!(build.public_inputs.is_empty());

        let verify = privacy_production_dispatch(
            &verify_request(
                PRIVACY_CONFIDENTIAL_UNSHIELD_ALGORITHM_ID,
                build.proof.clone(),
            ),
            PrivacyProofOperationV1::Verify,
        );
        assert_eq!(verify.status, PRIVACY_FFI_STATUS_OK);
        assert!(verify.verified);
        assert!(verify.public_inputs.is_empty());
    }

    #[test]
    fn cross_algorithm_proof_does_not_verify() {
        let transfer_witness = encode_witness(&valid_transfer_witness());
        let transfer = privacy_production_dispatch(
            &build_request(
                PRIVACY_CONFIDENTIAL_TRANSFER_V2_ALGORITHM_ID,
                transfer_witness,
            ),
            PrivacyProofOperationV1::Build,
        );
        assert!(!transfer.proof.is_empty());

        // A transfer proof must not verify under the unshield verifying key.
        let verify = privacy_production_dispatch(
            &verify_request(PRIVACY_CONFIDENTIAL_UNSHIELD_ALGORITHM_ID, transfer.proof),
            PrivacyProofOperationV1::Verify,
        );
        assert_eq!(verify.status, PRIVACY_FFI_STATUS_ERROR);
        assert_eq!(verify.error_code, PRIVACY_FFI_ERROR_PROVING_FAILED);
        assert!(!verify.verified);
    }

    #[test]
    fn undecodable_witness_fails_closed_without_leaking_witness() {
        let secret = b"native-witness-never-echo-1a2b3c";
        let build = privacy_production_dispatch(
            &build_request(
                PRIVACY_CONFIDENTIAL_TRANSFER_V2_ALGORITHM_ID,
                secret.to_vec(),
            ),
            PrivacyProofOperationV1::Build,
        );
        assert_eq!(build.status, PRIVACY_FFI_STATUS_ERROR);
        assert_eq!(build.error_code, PRIVACY_FFI_ERROR_INVALID_REQUEST);
        assert!(build.proof.is_empty());
        assert!(!build.verified);
        let serialized = norito::to_bytes(&build).expect("encode result");
        assert!(
            !serialized
                .windows(secret.len())
                .any(|window| window == secret),
            "invalid-witness result must not echo witness bytes"
        );
    }

    #[test]
    fn transfer_witness_shape_rejects_ignored_or_ambiguous_fields() {
        let mut with_public_amount = valid_transfer_witness();
        with_public_amount.public_amount = 1;
        assert_invalid_witness(
            PRIVACY_CONFIDENTIAL_TRANSFER_V2_ALGORITHM_ID,
            with_public_amount,
            "public_amount",
        );

        let mut with_unshield_change = valid_transfer_witness();
        with_unshield_change
            .unshield_change
            .push(PrivacyConfidentialUnshieldChangeWitnessV1 {
                amount: 1,
                rho: vec![0x7A; 32],
            });
        assert_invalid_witness(
            PRIVACY_CONFIDENTIAL_TRANSFER_V2_ALGORITHM_ID,
            with_unshield_change,
            "unshield change",
        );

        let mut without_outputs = valid_transfer_witness();
        without_outputs.transfer_outputs.clear();
        assert_invalid_witness(
            PRIVACY_CONFIDENTIAL_TRANSFER_V2_ALGORITHM_ID,
            without_outputs,
            "one or two outputs",
        );

        let mut duplicate_input = valid_transfer_witness();
        duplicate_input
            .inputs
            .push(duplicate_input.inputs[0].clone());
        assert_invalid_witness(
            PRIVACY_CONFIDENTIAL_TRANSFER_V2_ALGORITHM_ID,
            duplicate_input,
            "duplicates inputs[0]",
        );
    }

    #[test]
    fn unshield_witness_shape_rejects_ignored_or_ambiguous_fields() {
        let mut with_transfer_outputs = valid_unshield_witness();
        with_transfer_outputs.transfer_outputs = valid_transfer_witness().transfer_outputs;
        assert_invalid_witness(
            PRIVACY_CONFIDENTIAL_UNSHIELD_ALGORITHM_ID,
            with_transfer_outputs,
            "transfer outputs",
        );

        let mut too_many_change_outputs = valid_unshield_witness();
        too_many_change_outputs
            .unshield_change
            .push(too_many_change_outputs.unshield_change[0].clone());
        assert_invalid_witness(
            PRIVACY_CONFIDENTIAL_UNSHIELD_ALGORITHM_ID,
            too_many_change_outputs,
            "at most one",
        );

        let mut out_of_range_leaf = valid_unshield_witness();
        out_of_range_leaf.inputs[0].leaf_index = 1;
        assert_invalid_witness(
            PRIVACY_CONFIDENTIAL_UNSHIELD_ALGORITHM_ID,
            out_of_range_leaf,
            "must reference tree_commitments",
        );

        let mut duplicate_rho = valid_unshield_witness();
        let mut second_input = duplicate_rho.inputs[0].clone();
        second_input.leaf_index = 1;
        duplicate_rho.tree_commitments.push(vec![0; 32]);
        duplicate_rho.inputs.push(second_input);
        assert_invalid_witness(
            PRIVACY_CONFIDENTIAL_UNSHIELD_ALGORITHM_ID,
            duplicate_rho,
            "rho duplicates",
        );
    }

    #[test]
    fn decodable_but_inconsistent_witness_returns_proving_failed() {
        // A well-formed witness archive whose tree commitments do not match the
        // root_hint passes decoding but the prover rejects it.
        let mut witness = valid_transfer_witness();
        witness.root_hint = [0x55_u8; 32].to_vec();
        let build = privacy_production_dispatch(
            &build_request(
                PRIVACY_CONFIDENTIAL_TRANSFER_V2_ALGORITHM_ID,
                encode_witness(&witness),
            ),
            PrivacyProofOperationV1::Build,
        );
        assert_eq!(build.status, PRIVACY_FFI_STATUS_ERROR);
        assert_eq!(build.error_code, PRIVACY_FFI_ERROR_PROVING_FAILED);
        assert!(build.proof.is_empty());
    }

    #[test]
    fn overflowing_unshield_input_sum_returns_proving_failed() {
        let build = privacy_production_dispatch(
            &build_request(
                PRIVACY_CONFIDENTIAL_UNSHIELD_ALGORITHM_ID,
                encode_witness(&overflowing_unshield_witness()),
            ),
            PrivacyProofOperationV1::Build,
        );
        assert_eq!(build.status, PRIVACY_FFI_STATUS_ERROR);
        assert_eq!(build.error_code, PRIVACY_FFI_ERROR_PROVING_FAILED);
        assert!(
            build.message.contains("input amount sum overflows u128"),
            "unexpected overflow bridge error: {}",
            build.message
        );
        assert!(build.proof.is_empty());
    }

    #[test]
    fn out_of_scope_algorithm_stays_fail_closed() {
        let result = privacy_production_dispatch(
            &build_request("orchard-halo2-actions-v1", b"witness".to_vec()),
            PrivacyProofOperationV1::Build,
        );
        assert_eq!(result.status, PRIVACY_FFI_STATUS_ERROR);
        assert_eq!(result.error_code, PRIVACY_FFI_ERROR_PRODUCTION_DISABLED);
        assert!(result.proof.is_empty());
        assert_eq!(result.version, PRIVACY_FFI_VERSION_V1);
    }
}
