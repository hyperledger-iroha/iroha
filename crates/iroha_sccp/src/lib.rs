#![cfg_attr(not(feature = "std"), no_std)]
#![allow(missing_docs)]
#![allow(missing_copy_implementations)]

extern crate alloc;

use alloc::{string::String, vec::Vec};

use blake2::{
    Blake2bVar,
    digest::{Update, VariableOutput},
};
use codec::{Decode, Encode};
use scale_info::TypeInfo;
use tiny_keccak::Hasher;

pub const SCCP_DOMAIN_SORA: u32 = 0;
pub const SCCP_DOMAIN_ETH: u32 = 1;
pub const SCCP_DOMAIN_BSC: u32 = 2;
pub const SCCP_DOMAIN_SOL: u32 = 3;
pub const SCCP_DOMAIN_TON: u32 = 4;
pub const SCCP_DOMAIN_TRON: u32 = 5;
pub const SCCP_DOMAIN_SORA_KUSAMA: u32 = 6;
pub const SCCP_DOMAIN_SORA_POLKADOT: u32 = 7;

pub const SCCP_CORE_REMOTE_DOMAINS: [u32; 7] = [
    SCCP_DOMAIN_ETH,
    SCCP_DOMAIN_BSC,
    SCCP_DOMAIN_SOL,
    SCCP_DOMAIN_TON,
    SCCP_DOMAIN_TRON,
    SCCP_DOMAIN_SORA_KUSAMA,
    SCCP_DOMAIN_SORA_POLKADOT,
];

pub const SCCP_MSG_PREFIX_BURN_V1: &[u8] = b"sccp:burn:v1";
pub const SCCP_MSG_PREFIX_TOKEN_ADD_V1: &[u8] = b"sccp:token:add:v1";
pub const SCCP_MSG_PREFIX_TOKEN_PAUSE_V1: &[u8] = b"sccp:token:pause:v1";
pub const SCCP_MSG_PREFIX_TOKEN_RESUME_V1: &[u8] = b"sccp:token:resume:v1";
pub const IROHA_CONSENSUS_PROTO_VERSION_V1: u32 = 1;

const SCCP_HUB_LEAF_PREFIX_V1: &[u8] = b"sccp:hub:leaf:v1";
const SCCP_HUB_NODE_PREFIX_V1: &[u8] = b"sccp:hub:node:v1";
const SCCP_PAYLOAD_HASH_PREFIX_V1: &[u8] = b"sccp:payload:v1";
const SCCP_PARLIAMENT_HASH_PREFIX_V1: &[u8] = b"sccp:parliament:v1";

pub type H256 = [u8; 32];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct BurnPayloadV1 {
    pub version: u8,
    pub source_domain: u32,
    pub dest_domain: u32,
    pub nonce: u64,
    pub sora_asset_id: H256,
    pub amount: u128,
    pub recipient: H256,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TokenAddPayloadV1 {
    pub version: u8,
    pub target_domain: u32,
    pub nonce: u64,
    pub sora_asset_id: H256,
    pub decimals: u8,
    pub name: [u8; 32],
    pub symbol: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct TokenControlPayloadV1 {
    pub version: u8,
    pub target_domain: u32,
    pub nonce: u64,
    pub sora_asset_id: H256,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum GovernancePayloadV1 {
    Add(TokenAddPayloadV1),
    Pause(TokenControlPayloadV1),
    Resume(TokenControlPayloadV1),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum SccpHubMessageKind {
    Burn,
    TokenAdd,
    TokenPause,
    TokenResume,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SccpHubCommitmentV1 {
    pub version: u8,
    pub kind: SccpHubMessageKind,
    pub target_domain: u32,
    pub message_id: H256,
    pub payload_hash: H256,
    pub parliament_certificate_hash: Option<H256>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SccpMerkleStepV1 {
    pub sibling_hash: H256,
    pub sibling_is_left: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SccpMerkleProofV1 {
    pub steps: Vec<SccpMerkleStepV1>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum NexusConsensusPhaseV1 {
    Prepare = 1,
    Commit = 2,
    NewView = 3,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NexusCommitQcV1 {
    pub version: u8,
    pub phase: NexusConsensusPhaseV1,
    pub height: u64,
    pub view: u64,
    pub epoch: u64,
    pub mode_tag: String,
    pub subject_block_hash: H256,
    pub validator_set_hash_version: u16,
    pub validator_public_keys: Vec<String>,
    pub validator_set_pops: Vec<Vec<u8>>,
    pub signers_bitmap: Vec<u8>,
    pub bls_aggregate_signature: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NexusBridgeFinalityProofV1 {
    pub version: u8,
    pub chain_id: String,
    pub height: u64,
    pub block_hash: H256,
    pub commitment_root: H256,
    pub commit_qc: NexusCommitQcV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NexusParliamentSignatureV1 {
    pub public_key: String,
    pub signature: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NexusParliamentCertificateV1 {
    pub version: u8,
    pub preimage_hash: H256,
    pub enactment_window_start: u64,
    pub enactment_window_end: u64,
    pub payload_bytes: Vec<u8>,
    pub signatures: Vec<NexusParliamentSignatureV1>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NexusSccpBurnProofV1 {
    pub version: u8,
    pub commitment_root: H256,
    pub commitment: SccpHubCommitmentV1,
    pub merkle_proof: SccpMerkleProofV1,
    pub payload: BurnPayloadV1,
    pub finality_proof: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, TypeInfo)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NexusSccpGovernanceProofV1 {
    pub version: u8,
    pub commitment_root: H256,
    pub commitment: SccpHubCommitmentV1,
    pub merkle_proof: SccpMerkleProofV1,
    pub payload: GovernancePayloadV1,
    pub parliament_certificate: Vec<u8>,
    pub finality_proof: Vec<u8>,
}

pub fn is_supported_domain(domain_id: u32) -> bool {
    matches!(
        domain_id,
        SCCP_DOMAIN_SORA
            | SCCP_DOMAIN_ETH
            | SCCP_DOMAIN_BSC
            | SCCP_DOMAIN_SOL
            | SCCP_DOMAIN_TON
            | SCCP_DOMAIN_TRON
            | SCCP_DOMAIN_SORA_KUSAMA
            | SCCP_DOMAIN_SORA_POLKADOT
    )
}

pub fn burn_message_id(payload: &BurnPayloadV1) -> H256 {
    prefixed_keccak(SCCP_MSG_PREFIX_BURN_V1, &payload.encode())
}

pub fn token_add_message_id(payload: &TokenAddPayloadV1) -> H256 {
    prefixed_keccak(SCCP_MSG_PREFIX_TOKEN_ADD_V1, &payload.encode())
}

pub fn token_pause_message_id(payload: &TokenControlPayloadV1) -> H256 {
    prefixed_keccak(SCCP_MSG_PREFIX_TOKEN_PAUSE_V1, &payload.encode())
}

pub fn token_resume_message_id(payload: &TokenControlPayloadV1) -> H256 {
    prefixed_keccak(SCCP_MSG_PREFIX_TOKEN_RESUME_V1, &payload.encode())
}

pub fn governance_message_id(payload: &GovernancePayloadV1) -> H256 {
    match payload {
        GovernancePayloadV1::Add(payload) => token_add_message_id(payload),
        GovernancePayloadV1::Pause(payload) => token_pause_message_id(payload),
        GovernancePayloadV1::Resume(payload) => token_resume_message_id(payload),
    }
}

pub fn governance_target_domain(payload: &GovernancePayloadV1) -> u32 {
    match payload {
        GovernancePayloadV1::Add(payload) => payload.target_domain,
        GovernancePayloadV1::Pause(payload) => payload.target_domain,
        GovernancePayloadV1::Resume(payload) => payload.target_domain,
    }
}

pub fn payload_hash(payload: &[u8]) -> H256 {
    prefixed_blake2b(SCCP_PAYLOAD_HASH_PREFIX_V1, payload)
}

pub fn parliament_certificate_hash(certificate: &[u8]) -> H256 {
    prefixed_blake2b(SCCP_PARLIAMENT_HASH_PREFIX_V1, certificate)
}

pub fn commitment_leaf_hash(commitment: &SccpHubCommitmentV1) -> H256 {
    prefixed_blake2b(SCCP_HUB_LEAF_PREFIX_V1, &commitment.encode())
}

pub fn merkle_root_from_commitment(
    commitment: &SccpHubCommitmentV1,
    proof: &SccpMerkleProofV1,
) -> H256 {
    let mut current = commitment_leaf_hash(commitment);
    for step in &proof.steps {
        current = if step.sibling_is_left {
            hash_merkle_node(&step.sibling_hash, &current)
        } else {
            hash_merkle_node(&current, &step.sibling_hash)
        };
    }
    current
}

pub fn decode_nexus_bridge_finality_proof(
    proof_bytes: &[u8],
) -> Option<NexusBridgeFinalityProofV1> {
    NexusBridgeFinalityProofV1::decode(&mut &proof_bytes[..]).ok()
}

pub fn decode_nexus_parliament_certificate(
    certificate_bytes: &[u8],
) -> Option<NexusParliamentCertificateV1> {
    NexusParliamentCertificateV1::decode(&mut &certificate_bytes[..]).ok()
}

pub fn verify_nexus_bridge_finality_proof_structure(
    proof: &NexusBridgeFinalityProofV1,
) -> bool {
    if proof.version != 1 || proof.chain_id.is_empty() || proof.height == 0 {
        return false;
    }
    let qc = &proof.commit_qc;
    if qc.version != 1
        || qc.phase != NexusConsensusPhaseV1::Commit
        || qc.height != proof.height
        || qc.subject_block_hash != proof.block_hash
        || qc.mode_tag.is_empty()
        || qc.validator_set_hash_version != 1
        || qc.validator_public_keys.is_empty()
        || qc.validator_set_pops.len() != qc.validator_public_keys.len()
        || qc.bls_aggregate_signature.is_empty()
    {
        return false;
    }

    for public_key in &qc.validator_public_keys {
        if public_key.is_empty() {
            return false;
        }
    }
    for pop in &qc.validator_set_pops {
        if pop.is_empty() {
            return false;
        }
    }

    let roster_len = qc.validator_public_keys.len();
    if qc.signers_bitmap.len() != roster_len.div_ceil(8) {
        return false;
    }
    signer_indices_from_bitmap(&qc.signers_bitmap, roster_len).is_some()
}

pub fn verify_nexus_parliament_certificate_structure(
    certificate: &NexusParliamentCertificateV1,
    governance_payload_encoded: &[u8],
    proof_height: u64,
) -> bool {
    if certificate.version != 1
        || certificate.payload_bytes.is_empty()
        || certificate.signatures.is_empty()
        || certificate.enactment_window_start > certificate.enactment_window_end
        || proof_height < certificate.enactment_window_start
        || proof_height > certificate.enactment_window_end
        || certificate.preimage_hash != payload_hash(governance_payload_encoded)
    {
        return false;
    }

    for signature in &certificate.signatures {
        if signature.public_key.is_empty() || signature.signature.is_empty() {
            return false;
        }
    }
    true
}

pub fn nexus_commit_vote_preimage(chain_id: &str, certificate: &NexusCommitQcV1) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + 32 + 8 * 3 + 1);
    let domain = iroha_consensus_domain(chain_id, "Vote", b"v1", &certificate.mode_tag);
    out.extend_from_slice(&domain);
    out.extend_from_slice(&certificate.subject_block_hash);
    out.extend_from_slice(&certificate.height.to_be_bytes());
    out.extend_from_slice(&certificate.view.to_be_bytes());
    out.extend_from_slice(&certificate.epoch.to_be_bytes());
    out.push(certificate.phase as u8);
    out
}

pub fn verify_burn_bundle_structure(bundle: &NexusSccpBurnProofV1) -> bool {
    if bundle.version != 1 {
        return false;
    }
    if bundle.payload.version != 1 || !is_supported_domain(bundle.payload.source_domain) {
        return false;
    }
    let Some(finality_proof) = decode_nexus_bridge_finality_proof(&bundle.finality_proof) else {
        return false;
    };
    if !verify_nexus_bridge_finality_proof_structure(&finality_proof)
        || finality_proof.commitment_root != bundle.commitment_root
    {
        return false;
    }
    if !is_supported_domain(bundle.payload.dest_domain)
        || bundle.payload.dest_domain == bundle.payload.source_domain
        || bundle.payload.dest_domain == 0 && bundle.payload.source_domain == 0
    {
        return false;
    }
    if bundle.commitment.version != 1
        || bundle.commitment.kind != SccpHubMessageKind::Burn
        || bundle.commitment.target_domain != bundle.payload.dest_domain
        || bundle.commitment.message_id != burn_message_id(&bundle.payload)
        || bundle.commitment.payload_hash != payload_hash(&bundle.payload.encode())
        || bundle.commitment.parliament_certificate_hash.is_some()
    {
        return false;
    }
    merkle_root_from_commitment(&bundle.commitment, &bundle.merkle_proof) == bundle.commitment_root
}

pub fn verify_governance_bundle_structure(bundle: &NexusSccpGovernanceProofV1) -> bool {
    if bundle.version != 1 || bundle.commitment.version != 1 {
        return false;
    }
    let Some(finality_proof) = decode_nexus_bridge_finality_proof(&bundle.finality_proof) else {
        return false;
    };
    if !verify_nexus_bridge_finality_proof_structure(&finality_proof)
        || finality_proof.commitment_root != bundle.commitment_root
    {
        return false;
    }
    let Some(certificate) = decode_nexus_parliament_certificate(&bundle.parliament_certificate)
    else {
        return false;
    };
    if !verify_nexus_parliament_certificate_structure(
        &certificate,
        &bundle.payload.encode(),
        finality_proof.height,
    ) {
        return false;
    }

    let expected_kind = match bundle.payload {
        GovernancePayloadV1::Add(_) => SccpHubMessageKind::TokenAdd,
        GovernancePayloadV1::Pause(_) => SccpHubMessageKind::TokenPause,
        GovernancePayloadV1::Resume(_) => SccpHubMessageKind::TokenResume,
    };
    let target_domain = governance_target_domain(&bundle.payload);
    if !is_supported_domain(target_domain)
        || bundle.commitment.kind != expected_kind
        || bundle.commitment.target_domain != target_domain
        || bundle.commitment.message_id != governance_message_id(&bundle.payload)
        || bundle.commitment.payload_hash != payload_hash(&bundle.payload.encode())
        || bundle.commitment.parliament_certificate_hash
            != Some(parliament_certificate_hash(&bundle.parliament_certificate))
    {
        return false;
    }

    merkle_root_from_commitment(&bundle.commitment, &bundle.merkle_proof) == bundle.commitment_root
}

fn prefixed_keccak(prefix: &[u8], payload: &[u8]) -> H256 {
    let mut keccak = tiny_keccak::Keccak::v256();
    keccak.update(prefix);
    keccak.update(payload);
    let mut out = [0u8; 32];
    keccak.finalize(&mut out);
    out
}

fn prefixed_blake2b(prefix: &[u8], payload: &[u8]) -> H256 {
    let mut hasher = Blake2bVar::new(32).expect("fixed hash length");
    hasher.update(prefix);
    hasher.update(payload);
    let mut out = [0u8; 32];
    hasher
        .finalize_variable(&mut out)
        .expect("fixed hash length");
    out
}

fn hash_merkle_node(left: &H256, right: &H256) -> H256 {
    let mut hasher = Blake2bVar::new(32).expect("fixed hash length");
    hasher.update(SCCP_HUB_NODE_PREFIX_V1);
    hasher.update(left);
    hasher.update(right);
    let mut out = [0u8; 32];
    hasher
        .finalize_variable(&mut out)
        .expect("fixed hash length");
    out
}

fn iroha_consensus_domain(
    chain_id: &str,
    message_type_tag: &str,
    extra: &[u8],
    mode_tag: &str,
) -> H256 {
    let mut hasher = Blake2bVar::new(32).expect("fixed hash length");
    hasher.update(b"iroha2-consensus/v2");
    hasher.update(chain_id.as_bytes());
    hasher.update(mode_tag.as_bytes());
    hasher.update(&IROHA_CONSENSUS_PROTO_VERSION_V1.to_be_bytes());
    hasher.update(message_type_tag.as_bytes());
    hasher.update(extra);
    let mut out = [0u8; 32];
    hasher
        .finalize_variable(&mut out)
        .expect("fixed hash length");
    out
}

fn signer_indices_from_bitmap(bitmap: &[u8], roster_len: usize) -> Option<Vec<usize>> {
    if bitmap.len() != roster_len.div_ceil(8) {
        return None;
    }

    let mut indices = Vec::new();
    for (byte_idx, byte) in bitmap.iter().enumerate() {
        if *byte == 0 {
            continue;
        }
        for bit in 0..8 {
            if (byte >> bit) & 1 == 0 {
                continue;
            }
            let idx = byte_idx * 8 + bit;
            if idx >= roster_len {
                return None;
            }
            indices.push(idx);
        }
    }
    Some(indices)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_finality_proof(commitment_root: H256) -> Vec<u8> {
        NexusBridgeFinalityProofV1 {
            version: 1,
            chain_id: "00000000-0000-0000-0000-000000000753".to_owned(),
            height: 7,
            block_hash: [7u8; 32],
            commitment_root,
            commit_qc: NexusCommitQcV1 {
                version: 1,
                phase: NexusConsensusPhaseV1::Commit,
                height: 7,
                view: 0,
                epoch: 0,
                mode_tag: "iroha2-consensus::permissioned-sumeragi@v1".to_owned(),
                subject_block_hash: [7u8; 32],
                validator_set_hash_version: 1,
                validator_public_keys: vec![
                    "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
                        .to_owned(),
                ],
                validator_set_pops: vec![vec![1u8; 48]],
                signers_bitmap: vec![0b0000_0001],
                bls_aggregate_signature: vec![2u8; 96],
            },
        }
        .encode()
    }

    fn sample_parliament_certificate(payload: &GovernancePayloadV1) -> Vec<u8> {
        NexusParliamentCertificateV1 {
            version: 1,
            preimage_hash: payload_hash(&payload.encode()),
            enactment_window_start: 1,
            enactment_window_end: 10,
            payload_bytes: vec![9u8; 16],
            signatures: vec![NexusParliamentSignatureV1 {
                public_key:
                    "ea01309060D021340617E9554CCBC2CF3CC3DB922A9BA323ABDF7C271FCC6EF69BE7A8DEBCA7D9E96C0F0089ABA22CDAADE4A2"
                        .to_owned(),
                signature: vec![3u8; 64],
            }],
        }
        .encode()
    }

    #[test]
    fn burn_bundle_roundtrip_structure_verifies() {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 7,
            sora_asset_id: [1u8; 32],
            amount: 42,
            recipient: [2u8; 32],
        };
        let commitment = SccpHubCommitmentV1 {
            version: 1,
            kind: SccpHubMessageKind::Burn,
            target_domain: SCCP_DOMAIN_SORA,
            message_id: burn_message_id(&payload),
            payload_hash: payload_hash(&payload.encode()),
            parliament_certificate_hash: None,
        };
        let commitment_root = commitment_leaf_hash(&commitment);
        let bundle = NexusSccpBurnProofV1 {
            version: 1,
            commitment_root,
            commitment,
            merkle_proof: SccpMerkleProofV1 { steps: Vec::new() },
            payload,
            finality_proof: sample_finality_proof(commitment_root),
        };
        assert!(verify_burn_bundle_structure(&bundle));
    }

    #[test]
    fn governance_bundle_rejects_wrong_certificate_hash() {
        let payload = GovernancePayloadV1::Pause(TokenControlPayloadV1 {
            version: 1,
            target_domain: SCCP_DOMAIN_SORA,
            nonce: 3,
            sora_asset_id: [7u8; 32],
        });
        let commitment = SccpHubCommitmentV1 {
            version: 1,
            kind: SccpHubMessageKind::TokenPause,
            target_domain: SCCP_DOMAIN_SORA,
            message_id: governance_message_id(&payload),
            payload_hash: payload_hash(&payload.encode()),
            parliament_certificate_hash: Some([9u8; 32]),
        };
        let commitment_root = commitment_leaf_hash(&commitment);
        let parliament_certificate = sample_parliament_certificate(&payload);
        let bundle = NexusSccpGovernanceProofV1 {
            version: 1,
            commitment_root,
            commitment,
            merkle_proof: SccpMerkleProofV1 { steps: Vec::new() },
            payload,
            parliament_certificate,
            finality_proof: sample_finality_proof(commitment_root),
        };
        assert!(!verify_governance_bundle_structure(&bundle));
    }

    #[test]
    fn burn_bundle_rejects_mismatched_finality_root() {
        let payload = BurnPayloadV1 {
            version: 1,
            source_domain: SCCP_DOMAIN_ETH,
            dest_domain: SCCP_DOMAIN_SORA,
            nonce: 7,
            sora_asset_id: [1u8; 32],
            amount: 42,
            recipient: [2u8; 32],
        };
        let commitment = SccpHubCommitmentV1 {
            version: 1,
            kind: SccpHubMessageKind::Burn,
            target_domain: SCCP_DOMAIN_SORA,
            message_id: burn_message_id(&payload),
            payload_hash: payload_hash(&payload.encode()),
            parliament_certificate_hash: None,
        };
        let commitment_root = commitment_leaf_hash(&commitment);
        let bundle = NexusSccpBurnProofV1 {
            version: 1,
            commitment_root,
            commitment,
            merkle_proof: SccpMerkleProofV1 { steps: Vec::new() },
            payload,
            finality_proof: sample_finality_proof([0xabu8; 32]),
        };
        assert!(!verify_burn_bundle_structure(&bundle));
    }
}
