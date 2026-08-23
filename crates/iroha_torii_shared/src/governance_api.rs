//! Typed Torii DTOs for the locally signed governance deployment workflow.

use iroha_data_model::{
    NetworkId,
    account::AccountId,
    governance::types::{AtWindow, ProposalKind},
    isi::{
        Instruction, InstructionBox,
        governance::{BallotProof, VotingMode},
    },
    prelude::Quantity,
    smart_contract::{ContractAddress, ContractAlias, manifest::ManifestProvenance},
};
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

/// Maximum canonical framed bytes accepted for one governance instruction draft.
pub const GOVERNANCE_INSTRUCTION_DRAFT_MAX_BYTES_V1: usize = 1024 * 1024;

/// Inclusive block-height window required by the governed deployment draft.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    JsonDeserialize,
    JsonSerialize,
    NoritoDeserialize,
    NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct GovernanceAtWindowV1 {
    /// Inclusive lower block height.
    pub lower: u64,
    /// Inclusive upper block height.
    pub upper: u64,
}

impl GovernanceAtWindowV1 {
    /// Validate that the window is ordered.
    ///
    /// # Errors
    /// Returns an error when `upper` precedes `lower`.
    pub fn validate(self) -> Result<(), String> {
        if self.upper < self.lower {
            return Err("window.upper must be greater than or equal to window.lower".to_owned());
        }
        Ok(())
    }
}

impl From<GovernanceAtWindowV1> for AtWindow {
    fn from(value: GovernanceAtWindowV1) -> Self {
        Self {
            lower: value.lower,
            upper: value.upper,
        }
    }
}

impl From<AtWindow> for GovernanceAtWindowV1 {
    fn from(value: AtWindow) -> Self {
        Self {
            lower: value.lower,
            upper: value.upper,
        }
    }
}

/// One canonical framed native instruction returned for local signing.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct GovernanceInstructionDraftV1 {
    /// Registered instruction wire identifier.
    pub wire_id: String,
    /// Lowercase hexadecimal canonical framed instruction bytes.
    pub payload_hex: String,
}

impl GovernanceInstructionDraftV1 {
    /// Encode one exact instruction into its canonical typed draft.
    ///
    /// # Errors
    /// Returns an error if canonical instruction framing fails.
    pub fn from_instruction(instruction: &InstructionBox) -> Result<Self, String> {
        let wire_id = Instruction::id(&**instruction);
        let payload = Instruction::dyn_encode(&**instruction);
        let framed = iroha_data_model::isi::frame_instruction_payload(wire_id, &payload)
            .map_err(|error| format!("failed to frame governance instruction: {error}"))?;
        Ok(Self {
            wire_id: wire_id.to_owned(),
            payload_hex: hex::encode(framed),
        })
    }

    /// Decode and re-encode one exact canonical instruction draft.
    ///
    /// # Errors
    /// Returns an error for an unknown wire id, non-lowercase or oversized hex,
    /// malformed framing, trailing data, or a non-canonical re-encoding.
    pub fn decode_instruction(&self) -> Result<InstructionBox, String> {
        if self.wire_id.is_empty() || self.wire_id.trim() != self.wire_id {
            return Err("governance instruction draft wire_id must be an exact token".to_owned());
        }
        if self.payload_hex.is_empty()
            || self.payload_hex.len() % 2 != 0
            || self
                .payload_hex
                .bytes()
                .any(|byte| !(byte.is_ascii_digit() || matches!(byte, b'a'..=b'f')))
        {
            return Err(
                "governance instruction draft payload_hex must be exact lowercase hexadecimal"
                    .to_owned(),
            );
        }
        if self.payload_hex.len() / 2 > GOVERNANCE_INSTRUCTION_DRAFT_MAX_BYTES_V1 {
            return Err("governance instruction draft exceeds the byte limit".to_owned());
        }
        let payload = hex::decode(&self.payload_hex)
            .map_err(|error| format!("invalid governance instruction draft hex: {error}"))?;
        let instruction =
            iroha_data_model::isi::decode_instruction_from_pair(&self.wire_id, &payload)
                .map_err(|error| format!("invalid governance instruction draft: {error}"))?;
        let canonical = Self::from_instruction(&instruction)?;
        if canonical != *self {
            return Err("governance instruction draft is not byte-canonical".to_owned());
        }
        Ok(instruction)
    }
}

/// Strict request for one governed IVM contract deployment proposal.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct GovernanceDeployContractDraftRequestV1 {
    /// Optional canonical contract address. Exactly one target field is required.
    #[norito(default)]
    pub contract_address: Option<ContractAddress>,
    /// Optional canonical on-chain alias. Exactly one target field is required.
    #[norito(default)]
    pub contract_alias: Option<ContractAlias>,
    /// Exact first-release Torii ABI value (`"1"`).
    pub abi_version: String,
    /// Exact lowercase Blake2b-32 code hash.
    pub code_hash: String,
    /// Exact lowercase canonical ABI hash.
    pub abi_hash: String,
    /// Complete, ordered governance window.
    pub window: GovernanceAtWindowV1,
    /// Optional exact referendum mode (`None` selects the governed default).
    #[norito(default)]
    pub mode: Option<VotingMode>,
    /// Required public signer and signature for the exact compiled manifest.
    pub manifest_provenance: ManifestProvenance,
}

impl GovernanceDeployContractDraftRequestV1 {
    /// Validate the closed request shape before network I/O.
    ///
    /// # Errors
    /// Returns an error for an ambiguous target, non-V1 ABI, noncanonical
    /// hashes, an unordered window, or a malformed provenance signature.
    pub fn validate(&self) -> Result<(), String> {
        match (&self.contract_address, &self.contract_alias) {
            (Some(_), None) | (None, Some(_)) => {}
            _ => {
                return Err(
                    "exactly one of contract_address or contract_alias must be provided".to_owned(),
                );
            }
        }
        if self.abi_version != "1" {
            return Err("abi_version must be the exact string `1`".to_owned());
        }
        exact_lower_hex_32("code_hash", &self.code_hash)?;
        exact_lower_hex_32("abi_hash", &self.abi_hash)?;
        self.window.validate()?;
        iroha_crypto::Signature::try_from_bytes(self.manifest_provenance.signature.payload())
            .map_err(|_| {
                "manifest_provenance.signature must be a non-empty non-zero signature".to_owned()
            })?;
        Ok(())
    }

    /// Compute the stable proposal id for a resolved contract address.
    ///
    /// # Errors
    /// Returns an error when request hashes are noncanonical or the address
    /// length exceeds the stable preimage width.
    pub fn proposal_id_for(&self, contract_address: &ContractAddress) -> Result<String, String> {
        use iroha_crypto::blake2::{Blake2b512, digest::Digest as _};
        let code_hash = exact_lower_hex_32("code_hash", &self.code_hash)?;
        let abi_hash = exact_lower_hex_32("abi_hash", &self.abi_hash)?;
        let address = contract_address.as_ref();
        let address_len: u32 = address
            .len()
            .try_into()
            .map_err(|_| "contract_address length exceeds 2^32 bytes".to_owned())?;
        let mut input = Vec::with_capacity(
            b"iroha:gov:proposal:v1|".len()
                + core::mem::size_of::<u32>()
                + address.len()
                + code_hash.len()
                + abi_hash.len(),
        );
        input.extend_from_slice(b"iroha:gov:proposal:v1|");
        input.extend_from_slice(&address_len.to_le_bytes());
        input.extend_from_slice(address.as_bytes());
        input.extend_from_slice(&code_hash);
        input.extend_from_slice(&abi_hash);
        let digest = Blake2b512::digest(input);
        Ok(hex::encode(&digest[..32]))
    }
}

/// Strict proposal draft response bound to one native instruction.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct GovernanceDeployContractDraftResponseV1 {
    /// Whether draft generation succeeded.
    pub ok: bool,
    /// Stable lowercase proposal id.
    pub proposal_id: String,
    /// Exactly one canonical `ProposeDeployContract` instruction.
    pub tx_instructions: Vec<GovernanceInstructionDraftV1>,
}

/// Strict request for one plain governance ballot draft.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct GovernancePlainBallotDraftRequestV1 {
    /// Canonical client authority.
    pub authority: String,
    /// Exact genesis-derived network.
    pub network_id: NetworkId,
    /// Canonical referendum selector.
    pub referendum_id: String,
    /// Canonical I105 owner; must equal `authority`.
    pub owner: String,
    /// Exact non-negative locked amount.
    pub amount: Quantity,
    /// Canonical unsigned decimal lock duration.
    pub duration_blocks: String,
    /// Exact `Aye`, `Nay`, or `Abstain` direction.
    pub direction: String,
}

/// Strict request for one ZK governance ballot draft.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct GovernanceZkBallotDraftRequestV1 {
    /// Canonical client authority.
    pub authority: String,
    /// Exact genesis-derived network.
    pub network_id: NetworkId,
    /// Canonical election/referendum selector.
    pub election_id: String,
    /// Exact proof backend label.
    pub backend: String,
    /// Canonical padded standard-base64 proof envelope.
    pub envelope_b64: String,
    /// Optional 32-byte eligibility root hint.
    #[norito(default)]
    pub root_hint: Option<String>,
    /// Optional canonical I105 owner hint.
    #[norito(default)]
    pub owner: Option<String>,
    /// Optional exact lock amount hint.
    #[norito(default)]
    pub amount: Option<Quantity>,
    /// Optional lock duration hint.
    #[norito(default)]
    pub duration_blocks: Option<u64>,
    /// Optional exact ballot direction hint.
    #[norito(default)]
    pub direction: Option<String>,
    /// Optional 32-byte nullifier hint.
    #[norito(default)]
    pub nullifier: Option<String>,
}

/// Strict request carrying a fully typed governance ballot proof.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct GovernanceZkBallotProofDraftRequestV1 {
    /// Canonical client authority.
    pub authority: String,
    /// Exact genesis-derived network.
    pub network_id: NetworkId,
    /// Canonical election/referendum selector.
    pub election_id: String,
    /// Exact native ballot proof.
    pub ballot: BallotProof,
}

/// Strict response for plain and ZK governance ballot drafts.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct GovernanceBallotDraftResponseV1 {
    /// Whether the endpoint processed the request.
    pub ok: bool,
    /// Whether the draft was accepted.
    pub accepted: bool,
    /// Exact rejection or draft status text.
    #[norito(default)]
    pub reason: Option<String>,
    /// Exactly one canonical ballot instruction on success.
    pub tx_instructions: Vec<GovernanceInstructionDraftV1>,
}

/// Strict request for one referendum finalization draft.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct GovernanceFinalizeDraftRequestV1 {
    /// Exact lowercase referendum id.
    pub referendum_id: String,
    /// Exact matching lowercase proposal id.
    pub proposal_id: String,
}

/// Strict finalization draft response.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct GovernanceFinalizeDraftResponseV1 {
    /// Whether draft generation succeeded.
    pub ok: bool,
    /// Exactly one canonical `FinalizeReferendum` instruction.
    pub tx_instructions: Vec<GovernanceInstructionDraftV1>,
}

/// Strict request for one approved referendum enactment draft.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct GovernanceEnactDraftRequestV1 {
    /// Exact lowercase proposal id.
    pub proposal_id: String,
}

/// Strict enactment draft response bound to the retained proposal state.
#[derive(
    Debug, Clone, PartialEq, Eq, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize,
)]
#[norito(deny_unknown_fields)]
pub struct GovernanceEnactDraftResponseV1 {
    /// Whether draft generation succeeded.
    pub ok: bool,
    /// Exact lowercase proposal id.
    pub proposal_id: String,
    /// Exact retained native proposal kind.
    pub proposal_kind: ProposalKind,
    /// Exact retained referendum window.
    pub referendum_window: AtWindow,
    /// Exactly one canonical `EnactReferendum` instruction.
    pub tx_instructions: Vec<GovernanceInstructionDraftV1>,
}

/// Parse one exact lowercase 32-byte identifier.
///
/// # Errors
/// Returns an error for prefixes, uppercase characters, whitespace, or wrong width.
pub fn exact_lower_hex_32(label: &str, value: &str) -> Result<[u8; 32], String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(format!(
            "{label} must be exactly 64 lowercase hexadecimal digits"
        ));
    }
    let mut bytes = [0_u8; 32];
    hex::decode_to_slice(value, &mut bytes).map_err(|error| format!("invalid {label}: {error}"))?;
    Ok(bytes)
}

/// Validate a finalization request's exact shared identifier.
///
/// # Errors
/// Returns an error for malformed or different referendum/proposal ids.
pub fn validate_finalize_request(
    request: &GovernanceFinalizeDraftRequestV1,
) -> Result<[u8; 32], String> {
    let referendum_id = exact_lower_hex_32("referendum_id", &request.referendum_id)?;
    let proposal_id = exact_lower_hex_32("proposal_id", &request.proposal_id)?;
    if referendum_id != proposal_id {
        return Err("referendum_id must equal proposal_id".to_owned());
    }
    Ok(proposal_id)
}

/// Validate the authenticated identity of one typed ballot request.
///
/// # Errors
/// Returns an error when the request targets another network or authority.
pub fn validate_ballot_identity(
    network_id: NetworkId,
    authority: &AccountId,
    request_network_id: NetworkId,
    request_authority: &str,
) -> Result<(), String> {
    if request_network_id != network_id {
        return Err("governance ballot targets a different network".to_owned());
    }
    let canonical = AccountId::canonicalize(request_authority)
        .map_err(|_| "governance ballot authority must use canonical I105 form".to_owned())?;
    if canonical != request_authority || canonical != authority.to_string() {
        return Err("governance ballot authority must equal the client account".to_owned());
    }
    Ok(())
}
