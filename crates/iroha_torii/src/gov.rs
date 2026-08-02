//! App-facing governance API.
#![allow(unexpected_cfgs)]
//!
//! This module hosts minimal DTOs and handlers for governance endpoints
//! described in `gov.md` and `specs/contract_deployment.md`.
//! Handlers validate inputs and build instruction skeletons for callers to
//! submit through the locally signed transaction pipeline. Draft request
//! schemas that previously exposed server-side signing inputs are strict and
//! no longer admit private signing material.
//!
//! Notes
//! - JSON parsing uses Norito's serde wrappers via the `NoritoJson` extractor.
//! - Keep responses stable and explicit; map input errors to 400.

use core::str::FromStr;

use base64::Engine as _;
use iroha_core::{
    smartcontracts::Execute as _,
    state::{StateReadOnly, WorldReadOnly},
};
use iroha_crypto::blake2::{Blake2b512, digest::Digest};
use iroha_data_model::{
    governance::types::{AtWindow, ParliamentBody},
    isi::governance::{CouncilDerivationKind, ParliamentDecision},
    ministry::{AgendaProposalRecordV1, AgendaProposalV1},
    smart_contract::manifest::{EntryPointKind, ManifestProvenance},
    validation_fee::{
        VALIDATION_FEE_PLAIN_MAX_MEMBERS_V1, ValidationFeePlainElectorateEligibilityRuleV1,
        ValidationFeePlainElectorateRulesV1,
    },
};
use iroha_primitives::numeric::Quantity;
use mv::storage::StorageReadOnly;
use norito::{
    codec::Encode as _,
    derive::{NoritoDeserialize, NoritoSerialize},
    json,
};

use crate::{
    JsonBody, NoritoJson, NoritoJsonWithBytes, NoritoQuery,
    json_macros::{JsonDeserialize, JsonSerialize},
    routing::{MaybeTelemetry, parse_account_literal_with_state},
};

const CONTEXT_GOV_BALLOT_ZK_AUTHORITY: &str = "/v1/gov/ballots/zk#authority";
const CONTEXT_GOV_BALLOT_ZK_V1_AUTHORITY: &str = "/v1/gov/ballots/zk-v1#authority";
const CONTEXT_GOV_BALLOT_ZK_V1_BALLOT_PROOF_AUTHORITY: &str =
    "/v1/gov/ballots/zk-v1/ballot-proof#authority";
const CONTEXT_GOV_BALLOT_PLAIN_AUTHORITY: &str = "/v1/gov/ballots/plain#authority";
const CONTEXT_GOV_BALLOT_PLAIN_OWNER: &str = "/v1/gov/ballots/plain#owner";
const CONTEXT_GOV_PROTECTED_AUTHORITY: &str = "/v1/gov/protected-namespaces#authority";
const CONTEXT_MINISTRY_AGENDA_DRAFT_AUTHORITY: &str =
    "/v1/ministry/agenda/proposals/draft#authority";
const CONTEXT_GOV_PARLIAMENT_BALLOT_AUTHORITY: &str = "/v1/gov/parliament/ballots#authority";

fn decode_hex(s: &str) -> Result<Vec<u8>, crate::Error> {
    let s = s.trim_start_matches("0x");
    if !s.len().is_multiple_of(2) {
        return Err(crate::Error::Query(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "odd hex length".into(),
                ),
            ),
        ));
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let b = s.as_bytes();
    for i in (0..b.len()).step_by(2) {
        let h = from_hex_nibble(b[i]).ok_or_else(|| {
            crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion("bad hex".into()),
            ))
        })?;
        let l = from_hex_nibble(b[i + 1]).ok_or_else(|| {
            crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion("bad hex".into()),
            ))
        })?;
        out.push((h << 4) | l);
    }
    Ok(out)
}

fn from_hex_nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

use std::{collections::BTreeSet, sync::Arc};

#[derive(Debug, JsonDeserialize, JsonSerialize, Clone, Copy)]
/// Inclusive height window used for governance scheduling.
///
/// Both bounds are block heights; handlers treat missing windows as
/// implementation-defined defaults appropriate for the action.
pub struct AtWindowDto {
    /// Lower bound (inclusive) in blocks.
    pub lower: u64,
    /// Upper bound (inclusive) in blocks.
    pub upper: u64,
}

impl norito::core::NoritoSerialize for AtWindowDto {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let tuple = (self.lower, self.upper);
        <(u64, u64) as norito::core::NoritoSerialize>::serialize(&tuple, writer)
    }
}

impl<'de> norito::core::NoritoDeserialize<'de> for AtWindowDto {
    fn try_deserialize(
        archived: &'de norito::core::Archived<AtWindowDto>,
    ) -> Result<Self, norito::core::Error> {
        let archived_tuple: &norito::core::Archived<(u64, u64)> = archived.cast();
        let (lower, upper) =
            <(u64, u64) as norito::core::NoritoDeserialize>::try_deserialize(archived_tuple)?;
        Ok(Self { lower, upper })
    }

    fn deserialize(archived: &'de norito::core::Archived<AtWindowDto>) -> Self {
        Self::try_deserialize(archived)
            .expect("AtWindowDto should deserialize from (lower, upper) tuple")
    }
}

impl norito::core::DecodeFromSlice<'_> for AtWindowDto {
    fn decode_from_slice(bytes: &[u8]) -> Result<(Self, usize), norito::core::Error> {
        let ((lower, upper), used) =
            <(u64, u64) as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
        Ok((Self { lower, upper }, used))
    }
}

#[derive(Debug, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
/// Request body for proposing deployment of IVM bytecode via governance.
///
/// All hashes are 32-byte hex, with or without `0x` prefix.
pub struct ProposeDeployContractDto {
    /// Optional canonical contract address targeted by the proposal.
    #[norito(default)]
    pub contract_address: Option<iroha_data_model::smart_contract::ContractAddress>,
    /// Optional on-chain contract alias resolved to the canonical contract address.
    #[norito(default)]
    pub contract_alias: Option<iroha_data_model::smart_contract::ContractAlias>,
    /// ABI version (e.g., "1")
    pub abi_version: String,
    /// Deterministic code hash (blake2b-32; prefixed or raw hex)
    pub code_hash: String,
    /// Deterministic ABI hash (blake2b-32; prefixed or raw hex)
    pub abi_hash: String,
    /// Optional enactment window override (inclusive)
    pub window: Option<AtWindowDto>,
    /// Optional voting mode: "Zk" or "Plain" (default Zk)
    #[norito(default)]
    pub mode: Option<String>,
    /// Optional per-contract limits (opaque for now)
    #[norito(default)]
    pub limits: Option<norito::json::Value>,
    /// Optional manifest provenance (public key + signature over the manifest payload).
    #[norito(default)]
    pub manifest_provenance: Option<ManifestProvenance>,
}

impl norito::core::NoritoSerialize for ProposeDeployContractDto {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let value = norito::json::to_value(self)
            .map_err(|err| norito::core::Error::Message(err.to_string()))?;
        let json = norito::json::to_string(&value)
            .map_err(|err| norito::core::Error::Message(err.to_string()))?;
        <String as norito::core::NoritoSerialize>::serialize(&json, writer)
    }
}

impl<'de> norito::core::NoritoDeserialize<'de> for ProposeDeployContractDto {
    fn try_deserialize(
        archived: &'de norito::core::Archived<ProposeDeployContractDto>,
    ) -> Result<Self, norito::core::Error> {
        let archived_json: &norito::core::Archived<String> = archived.cast();
        let json = <String as norito::core::NoritoDeserialize>::try_deserialize(archived_json)?;
        norito::json::from_str(&json).map_err(|err| norito::core::Error::Message(err.to_string()))
    }

    fn deserialize(archived: &'de norito::core::Archived<ProposeDeployContractDto>) -> Self {
        Self::try_deserialize(archived)
            .expect("ProposeDeployContractDto should deserialize from JSON string")
    }
}

/// Response body for a deploy-contract proposal
#[derive(Debug, JsonSerialize)]
pub struct ProposeDeployContractResponse {
    pub ok: bool,
    /// Deterministic 32-byte BLAKE2b proposal id, encoded as lowercase hex.
    pub proposal_id: String,
    /// Optional transaction skeleton for clients to sign and submit
    pub tx_instructions: Vec<TxInstr>,
}

#[derive(Debug, JsonDeserialize, JsonSerialize)]
/// Request body for proposing one closed SCCP registry action via governance.
pub struct ProposeSccpRouteGovernanceDto {
    /// Atomic closed registry action to apply if governance enacts the proposal.
    pub action: iroha_data_model::isi::bridge::SccpRouteGovernanceActionV1,
    /// Optional enactment window override (inclusive).
    pub window: Option<AtWindowDto>,
    /// Optional exact voting mode (default `Zk`).
    #[norito(default)]
    pub mode: Option<iroha_data_model::isi::governance::VotingMode>,
}

impl norito::core::NoritoSerialize for ProposeSccpRouteGovernanceDto {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let value = norito::json::to_value(self)
            .map_err(|err| norito::core::Error::Message(err.to_string()))?;
        let json = norito::json::to_string(&value)
            .map_err(|err| norito::core::Error::Message(err.to_string()))?;
        <String as norito::core::NoritoSerialize>::serialize(&json, writer)
    }
}

impl<'de> norito::core::NoritoDeserialize<'de> for ProposeSccpRouteGovernanceDto {
    fn try_deserialize(
        archived: &'de norito::core::Archived<ProposeSccpRouteGovernanceDto>,
    ) -> Result<Self, norito::core::Error> {
        let archived_json: &norito::core::Archived<String> = archived.cast();
        let json = <String as norito::core::NoritoDeserialize>::try_deserialize(archived_json)?;
        norito::json::from_str(&json).map_err(|err| norito::core::Error::Message(err.to_string()))
    }

    fn deserialize(archived: &'de norito::core::Archived<ProposeSccpRouteGovernanceDto>) -> Self {
        Self::try_deserialize(archived)
            .expect("ProposeSccpRouteGovernanceDto should deserialize from JSON string")
    }
}

/// Response body for an SCCP route-governance proposal.
#[derive(Debug, JsonSerialize)]
pub struct ProposeSccpRouteGovernanceResponse {
    pub ok: bool,
    /// Deterministic 32-byte BLAKE2b proposal id, encoded as lowercase hex.
    pub proposal_id: String,
    /// Optional transaction skeleton for clients to sign and submit.
    pub tx_instructions: Vec<TxInstr>,
}

#[derive(Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
/// Request body for drafting a Ministry agenda proposal submission transaction.
pub struct MinistryAgendaProposalDraftDto {
    /// Agenda proposal payload that will be submitted on-chain.
    pub proposal: AgendaProposalV1,
    /// Canonical I105 account id that will sign the transaction.
    pub authority: String,
}

#[derive(Debug, JsonSerialize)]
/// Draft response for a Ministry agenda proposal submission.
pub struct MinistryAgendaProposalDraftResponse {
    /// Whether the draft generation succeeded.
    pub ok: bool,
    /// Stable agenda proposal identifier.
    pub agenda_proposal_id: String,
    /// Canonical I105 authority used for transaction construction.
    pub authority: String,
    /// Single-instruction transaction skeleton for wallets/clients that want an instruction preview.
    pub tx_instructions: Vec<TxInstr>,
    /// Base64-encoded canonical `TransactionPayload` bytes for Connect `SignRequestTx`.
    pub signable_transaction_b64: String,
}

#[derive(Debug, JsonSerialize)]
/// Lookup response for submitted Ministry agenda proposals.
pub struct MinistryAgendaProposalGetResponse {
    /// Whether the proposal record exists in committed state.
    pub found: bool,
    /// Persisted proposal record when found.
    pub record: Option<AgendaProposalRecordV1>,
}

#[derive(Debug)]
/// Result of drafting a Ministry agenda proposal transaction.
pub enum MinistryAgendaProposalDraftOutcome {
    /// Draft created successfully.
    Draft(MinistryAgendaProposalDraftResponse),
    /// Proposal id already exists in committed state.
    Duplicate(MinistryAgendaProposalGetResponse),
}

#[derive(Debug, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
/// Request body for submitting a zero-knowledge ballot.
pub struct ZkBallotDto {
    /// Authority as canonical I105 or on-chain account alias.
    pub authority: String,
    /// Chain id to build the transaction skeleton for
    pub chain_id: String,
    pub election_id: String,
    /// Base64-encoded proof bytes
    pub proof_b64: String,
    /// Public inputs (opaque for now)
    #[norito(default)]
    pub public: Option<norito::json::Value>,
}

impl norito::core::NoritoSerialize for ZkBallotDto {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let value = norito::json::to_value(self)
            .map_err(|err| norito::core::Error::Message(err.to_string()))?;
        let json = norito::json::to_string(&value)
            .map_err(|err| norito::core::Error::Message(err.to_string()))?;
        <String as norito::core::NoritoSerialize>::serialize(&json, writer)
    }
}

impl<'de> norito::core::NoritoDeserialize<'de> for ZkBallotDto {
    fn try_deserialize(
        archived: &'de norito::core::Archived<ZkBallotDto>,
    ) -> Result<Self, norito::core::Error> {
        let archived_json: &norito::core::Archived<String> = archived.cast();
        let json = <String as norito::core::NoritoDeserialize>::try_deserialize(archived_json)?;
        norito::json::from_str(&json).map_err(|err| norito::core::Error::Message(err.to_string()))
    }

    fn deserialize(archived: &'de norito::core::Archived<ZkBallotDto>) -> Self {
        Self::try_deserialize(archived).expect("ZkBallotDto should deserialize from JSON string")
    }
}

#[derive(Debug, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
/// Request body for submitting a plain (non-ZK) quadratic ballot.
pub struct PlainBallotDto {
    /// Authority as canonical I105 or on-chain account alias.
    pub authority: String,
    /// Chain id to build the transaction skeleton for
    pub chain_id: String,
    pub referendum_id: String,
    /// Owner as canonical I105 or on-chain account alias.
    pub owner: String,
    /// Exact non-negative token quantity.
    pub amount: Quantity,
    /// Canonical unsigned decimal block duration.
    pub duration_blocks: String,
    /// One of: "Aye" | "Nay" | "Abstain"
    pub direction: String,
}

#[derive(Debug, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
/// Request body for drafting an equal signed Parliament body ballot.
pub struct ParliamentBallotDto {
    /// Authority as canonical I105 or on-chain account alias.
    pub authority: String,
    /// Chain id to build the transaction skeleton for.
    pub chain_id: String,
    /// Proposal id as 32-byte hex, optionally prefixed with `0x` or `blake2b32:`.
    pub proposal_id: String,
    /// Parliament body casting the stage ballot.
    pub body: ParliamentBody,
    /// Equal citizen decision.
    pub decision: ParliamentDecision,
}

impl norito::core::NoritoSerialize for ParliamentBallotDto {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let value = norito::json::to_value(self)
            .map_err(|err| norito::core::Error::Message(err.to_string()))?;
        let json = norito::json::to_string(&value)
            .map_err(|err| norito::core::Error::Message(err.to_string()))?;
        <String as norito::core::NoritoSerialize>::serialize(&json, writer)
    }
}

impl<'de> norito::core::NoritoDeserialize<'de> for ParliamentBallotDto {
    fn try_deserialize(
        archived: &'de norito::core::Archived<ParliamentBallotDto>,
    ) -> Result<Self, norito::core::Error> {
        let archived_json: &norito::core::Archived<String> = archived.cast();
        let json = <String as norito::core::NoritoDeserialize>::try_deserialize(archived_json)?;
        norito::json::from_str(&json).map_err(|err| norito::core::Error::Message(err.to_string()))
    }

    fn deserialize(archived: &'de norito::core::Archived<ParliamentBallotDto>) -> Self {
        Self::try_deserialize(archived)
            .expect("ParliamentBallotDto should deserialize from JSON string")
    }
}

impl norito::core::NoritoSerialize for PlainBallotDto {
    fn serialize(&self, writer: &mut norito::core::Encoder<'_>) -> Result<(), norito::core::Error> {
        let value = norito::json::to_value(self)
            .map_err(|err| norito::core::Error::Message(err.to_string()))?;
        let json = norito::json::to_string(&value)
            .map_err(|err| norito::core::Error::Message(err.to_string()))?;
        <String as norito::core::NoritoSerialize>::serialize(&json, writer)
    }
}

impl<'de> norito::core::NoritoDeserialize<'de> for PlainBallotDto {
    fn try_deserialize(
        archived: &'de norito::core::Archived<PlainBallotDto>,
    ) -> Result<Self, norito::core::Error> {
        let archived_json: &norito::core::Archived<String> = archived.cast();
        let json = <String as norito::core::NoritoDeserialize>::try_deserialize(archived_json)?;
        norito::json::from_str(&json).map_err(|err| norito::core::Error::Message(err.to_string()))
    }

    fn deserialize(archived: &'de norito::core::Archived<PlainBallotDto>) -> Self {
        Self::try_deserialize(archived).expect("PlainBallotDto should deserialize from JSON string")
    }
}

/// Response to ballot submission (both zk/plain)
#[derive(Debug, JsonSerialize)]
pub struct BallotSubmitResponse {
    pub ok: bool,
    pub accepted: bool,
    pub reason: Option<String>,
    /// Optional transaction skeleton for clients to sign and submit
    pub tx_instructions: Vec<TxInstr>,
}

fn ballot_rejection(reason: &str) -> JsonBody<BallotSubmitResponse> {
    JsonBody(BallotSubmitResponse {
        ok: false,
        accepted: false,
        reason: Some(reason.to_string()),
        tx_instructions: Vec::new(),
    })
}

fn lock_hints_incomplete(owner: bool, amount: bool, duration: bool) -> bool {
    let any = owner || amount || duration;
    any && !(owner && amount && duration)
}

fn hint_present(map: &json::Map, key: &str) -> bool {
    map.get(key)
        .map(|value| !matches!(value, json::Value::Null))
        .unwrap_or(false)
}

fn normalize_zk_ballot_public_inputs(map: &mut json::Map) -> Result<(), String> {
    reject_zk_public_input_aliases(map)?;
    canonicalize_hex32_public_input(map, "root_hint", "root_hint")?;
    canonicalize_hex32_public_input(map, "nullifier", "nullifier")?;
    Ok(())
}

fn reject_zk_public_input_aliases(map: &json::Map) -> Result<(), String> {
    reject_zk_public_input_key(map, "durationBlocks", "duration_blocks")?;
    reject_zk_public_input_key(map, "root_hint_hex", "root_hint")?;
    reject_zk_public_input_key(map, "rootHintHex", "root_hint")?;
    reject_zk_public_input_key(map, "rootHint", "root_hint")?;
    reject_zk_public_input_key(map, "nullifier_hex", "nullifier")?;
    reject_zk_public_input_key(map, "nullifierHex", "nullifier")?;
    Ok(())
}

fn ensure_owner_canonical(owner: &str) -> Result<(), String> {
    let canonical = iroha_data_model::account::AccountId::canonicalize(owner)
        .map_err(|_| "owner must use canonical I105 account id form".to_string())?;
    if canonical != owner {
        return Err("owner must use canonical I105 account id form".to_string());
    }
    Ok(())
}

fn reject_zk_public_input_owner(map: &json::Map) -> Result<(), String> {
    let Some(value) = map.get("owner") else {
        return Ok(());
    };
    if matches!(value, json::Value::Null) {
        return Ok(());
    }
    let owner = value
        .as_str()
        .ok_or_else(|| "owner must be a canonical I105 account id".to_string())?;
    ensure_owner_canonical(owner)
}

fn reject_zk_public_input_key(map: &json::Map, key: &str, canonical: &str) -> Result<(), String> {
    if map.contains_key(key) {
        return Err(format!(
            "public inputs must use {canonical} (unsupported key {key})"
        ));
    }
    Ok(())
}

fn reject_zk_v1_aliases_from_raw(raw: &[u8]) -> Result<(), String> {
    let Ok(value) = json::from_slice::<json::Value>(raw) else {
        return Ok(());
    };
    let json::Value::Object(map) = value else {
        return Ok(());
    };
    reject_zk_public_input_aliases(&map)?;
    Ok(())
}

fn reject_zk_v1_ballotproof_aliases_from_raw(raw: &[u8]) -> Result<(), String> {
    let Ok(value) = json::from_slice::<json::Value>(raw) else {
        return Ok(());
    };
    let json::Value::Object(map) = value else {
        return Ok(());
    };
    let Some(json::Value::Object(ballot)) = map.get("ballot") else {
        return Ok(());
    };
    reject_zk_public_input_aliases(ballot)?;
    reject_zk_public_input_owner(ballot)?;
    Ok(())
}

fn canonicalize_hex32_public_input(
    map: &mut json::Map,
    key: &str,
    label: &str,
) -> Result<(), String> {
    let Some(value) = map.get_mut(key) else {
        return Ok(());
    };
    if matches!(value, json::Value::Null) {
        return Ok(());
    }
    let raw = value
        .as_str()
        .ok_or_else(|| format!("{label} must be 32-byte hex"))?;
    let canonical =
        canonicalize_hex32_value(raw).ok_or_else(|| format!("{label} must be 32-byte hex"))?;
    *value = json::Value::String(canonical);
    Ok(())
}

fn canonicalize_hex32_value(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let without_scheme = if let Some((scheme, rest)) = trimmed.split_once(':') {
        if scheme.is_empty() || scheme.eq_ignore_ascii_case("blake2b32") {
            rest
        } else {
            return None;
        }
    } else {
        trimmed
    };
    let body = without_scheme.trim();
    let body = body
        .strip_prefix("0x")
        .or_else(|| body.strip_prefix("0X"))
        .unwrap_or(body)
        .trim();
    if body.len() != 64 || !body.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(body.to_ascii_lowercase())
}

fn parse_canonical_u64_decimal(field: &str, value: &str) -> Result<u64, String> {
    if value.is_empty()
        || (value.len() > 1 && value.starts_with('0'))
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(format!(
            "{field} must be a canonical unsigned decimal integer"
        ));
    }
    value
        .parse::<u64>()
        .map_err(|_| format!("{field} is outside the unsigned 64-bit integer range"))
}

// -------- ZK Ballot V1 DTO --------
#[derive(Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
/// Request body for submitting a ZK ballot using BallotProof-style fields.
pub struct ZkBallotV1Dto {
    /// Authority submitting the ballot (AccountId string)
    pub authority: String,
    /// Chain id to build the transaction skeleton for
    pub chain_id: String,
    pub election_id: String,
    /// Backend tag for the proof (e.g., halo2/ipa)
    pub backend: String,
    /// Base64-encoded envelope bytes (ZK1 or H2* container)
    pub envelope_b64: String,
    /// Optional eligibility root hint (hex-32, 0x allowed)
    #[norito(default)]
    pub root_hint: Option<String>,
    /// Optional owner account id (for lock hints when the circuit commits owner)
    #[norito(default)]
    pub owner: Option<String>,
    /// Optional exact lock amount hint.
    #[norito(default)]
    pub amount: Option<Quantity>,
    /// Optional lock duration hint in blocks.
    #[norito(default)]
    pub duration_blocks: Option<u64>,
    /// Optional direction hint ("Aye" | "Nay" | "Abstain").
    #[norito(default)]
    pub direction: Option<String>,
    /// Optional nullifier hint (hex-32, 0x allowed)
    #[norito(default)]
    pub nullifier: Option<String>,
}

#[derive(Debug, JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
/// Request body that carries a BallotProof directly along with transaction context.
pub struct ZkBallotV1BallotProofDto {
    pub authority: String,
    pub chain_id: String,
    pub election_id: String,
    pub ballot: iroha_data_model::isi::governance::BallotProof,
}

/// POST /v1/gov/ballots/zk-v1 — accept BallotProof-like DTO and build an instruction skeleton.
///
/// The request schema excludes private signing material; callers submit locally signed transactions.
///
/// # Errors
/// Returns `crate::Error::Query` for invalid chain id or authority. Invalid payloads are
/// reflected in the response body.
pub async fn handle_gov_ballot_zk_v1(
    chain_id: Arc<iroha_data_model::ChainId>,
    state: Arc<iroha_core::state::State>,
    telemetry: MaybeTelemetry,
    NoritoJsonWithBytes { value: body, raw }: NoritoJsonWithBytes<ZkBallotV1Dto>,
) -> Result<JsonBody<BallotSubmitResponse>, crate::Error> {
    if let Err(reason) = reject_zk_v1_aliases_from_raw(raw.as_ref()) {
        return Ok(ballot_rejection(&reason));
    }
    // Minimal size check for b64
    if base64::engine::general_purpose::STANDARD
        .decode(body.envelope_b64.as_bytes())
        .map(|bytes| bytes.len())
        .unwrap_or(0)
        == 0
    {
        return Ok(JsonBody(BallotSubmitResponse {
            ok: false,
            accepted: false,
            reason: Some("invalid proof envelope".to_string()),
            tx_instructions: Vec::new(),
        }));
    }
    ensure_chain_id_matches(chain_id.as_ref(), &body.chain_id)?;
    let _authority_id = parse_authority_literal(
        state.as_ref(),
        body.authority.as_str(),
        &telemetry,
        CONTEXT_GOV_BALLOT_ZK_V1_AUTHORITY,
    )?;
    let has_owner = body.owner.is_some();
    let has_amount = body.amount.is_some();
    let has_duration = body.duration_blocks.is_some();
    if lock_hints_incomplete(has_owner, has_amount, has_duration) {
        return Ok(ballot_rejection(
            "lock hints must include owner, amount, duration_blocks",
        ));
    }
    if let Some(owner) = &body.owner {
        if let Err(reason) = ensure_owner_canonical(owner) {
            return Ok(ballot_rejection(&reason));
        }
    }
    // Build public inputs JSON object with optional hints
    let mut pub_map = norito::json::Map::new();
    if let Some(rh) = &body.root_hint {
        let Some(canonical) = canonicalize_hex32_value(rh) else {
            return Ok(ballot_rejection("root_hint must be 32-byte hex"));
        };
        pub_map.insert("root_hint".into(), norito::json::Value::from(canonical));
    }
    if let Some(owner) = &body.owner {
        pub_map.insert("owner".into(), norito::json::Value::from(owner.clone()));
    }
    if let Some(amount) = &body.amount {
        pub_map.insert(
            "amount".into(),
            norito::json::Value::from(amount.to_string()),
        );
    }
    if let Some(duration_blocks) = body.duration_blocks {
        pub_map.insert(
            "duration_blocks".into(),
            norito::json::Value::from(duration_blocks),
        );
    }
    if let Some(direction) = &body.direction {
        pub_map.insert(
            "direction".into(),
            norito::json::Value::from(direction.clone()),
        );
    }
    if let Some(nullifier) = &body.nullifier {
        let Some(canonical) = canonicalize_hex32_value(nullifier) else {
            return Ok(ballot_rejection("nullifier must be 32-byte hex"));
        };
        pub_map.insert("nullifier".into(), norito::json::Value::from(canonical));
    }
    let public_inputs_json = norito::json::to_json(&norito::json::Value::Object(pub_map))
        .unwrap_or_else(|_| "{}".into());
    // Convert to CastZkBallot skeleton
    let instr = iroha_data_model::isi::governance::CastZkBallot {
        election_id: body.election_id,
        proof_b64: body.envelope_b64,
        public_inputs_json,
    };
    let tx_instructions = vec![tx_instr_from_box(instr.into())];
    Ok(JsonBody(BallotSubmitResponse {
        ok: true,
        accepted: true,
        reason: Some("build transaction skeleton".to_string()),
        tx_instructions,
    }))
}

/// POST /v1/gov/ballots/zk-v1/ballot-proof — accept BallotProof JSON and build instruction skeleton.
///
/// The request schema excludes private signing material; callers submit locally signed transactions.
///
/// # Errors
/// Returns `crate::Error::Query` for invalid chain id or authority. Malformed payloads are
/// reported via the response payload.
pub async fn handle_gov_ballot_zk_v1_ballotproof(
    chain_id: Arc<iroha_data_model::ChainId>,
    state: Arc<iroha_core::state::State>,
    telemetry: MaybeTelemetry,
    NoritoJsonWithBytes { value: body, raw }: NoritoJsonWithBytes<ZkBallotV1BallotProofDto>,
) -> Result<JsonBody<BallotSubmitResponse>, crate::Error> {
    if let Err(reason) = reject_zk_v1_ballotproof_aliases_from_raw(raw.as_ref()) {
        return Ok(ballot_rejection(&reason));
    }
    if body.ballot.envelope_bytes.is_empty() {
        return Ok(JsonBody(BallotSubmitResponse {
            ok: false,
            accepted: false,
            reason: Some("invalid proof envelope".to_string()),
            tx_instructions: Vec::new(),
        }));
    }
    ensure_chain_id_matches(chain_id.as_ref(), &body.chain_id)?;
    let _authority_id = parse_authority_literal(
        state.as_ref(),
        body.authority.as_str(),
        &telemetry,
        CONTEXT_GOV_BALLOT_ZK_V1_BALLOT_PROOF_AUTHORITY,
    )?;
    let has_owner = body.ballot.owner.is_some();
    let has_amount = body.ballot.amount.is_some();
    let has_duration = body.ballot.duration_blocks.is_some();
    if lock_hints_incomplete(has_owner, has_amount, has_duration) {
        return Ok(ballot_rejection(
            "lock hints must include owner, amount, duration_blocks",
        ));
    }
    // Build public inputs JSON from optional hints in BallotProof
    let mut pub_map = norito::json::Map::new();
    if let Some(rh) = &body.ballot.root_hint {
        pub_map.insert(
            "root_hint".into(),
            norito::json::Value::from(hex::encode(rh)),
        );
    }
    if let Some(owner) = &body.ballot.owner {
        pub_map.insert("owner".into(), norito::json::Value::from(owner.to_string()));
    }
    if let Some(amount) = &body.ballot.amount {
        pub_map.insert(
            "amount".into(),
            norito::json::Value::from(amount.to_string()),
        );
    }
    if let Some(duration_blocks) = body.ballot.duration_blocks {
        pub_map.insert(
            "duration_blocks".into(),
            norito::json::Value::from(duration_blocks),
        );
    }
    if let Some(direction) = &body.ballot.direction {
        pub_map.insert(
            "direction".into(),
            norito::json::Value::from(direction.clone()),
        );
    }
    if let Some(nullifier) = &body.ballot.nullifier {
        pub_map.insert(
            "nullifier".into(),
            norito::json::Value::from(hex::encode(nullifier)),
        );
    }
    let public_inputs_json = norito::json::to_json(&norito::json::Value::Object(pub_map))
        .unwrap_or_else(|_| "{}".into());
    // Re-encode envelope_bytes as base64 for CastZkBallot
    let proof_b64 = base64::engine::general_purpose::STANDARD.encode(&body.ballot.envelope_bytes);
    let instr = iroha_data_model::isi::governance::CastZkBallot {
        election_id: body.election_id,
        proof_b64,
        public_inputs_json,
    };
    let tx_instructions = vec![tx_instr_from_box(instr.into())];
    Ok(JsonBody(BallotSubmitResponse {
        ok: true,
        accepted: true,
        reason: Some("build transaction skeleton".to_string()),
        tx_instructions,
    }))
}

/// A single council member (account id string)
#[derive(Debug, JsonSerialize)]
pub struct CouncilMemberDto {
    pub account_id: String,
}

/// Current council response (epoch + members)
#[derive(Debug, JsonSerialize)]
pub struct CouncilCurrentResponse {
    pub epoch: u64,
    pub members: Vec<CouncilMemberDto>,
    pub alternates: Vec<CouncilMemberDto>,
    pub candidate_count: usize,
    pub derived_by: CouncilDerivationKind,
}

/// Citizenship status response for an account.
#[derive(Debug, JsonSerialize)]
pub struct CitizenStatusResponse {
    pub account_id: String,
    pub is_citizen: bool,
    pub amount: Option<String>,
    pub bonded_height: Option<String>,
    pub seats_in_epoch: Option<String>,
    pub last_epoch_seen: Option<String>,
    pub cooldown_until: Option<String>,
}

/// Exact citizen registry count response.
#[derive(Debug, JsonSerialize)]
pub struct CitizenCountResponse {
    pub total: String,
}

/// Stable schema identifier for the strict governance readiness projection.
pub const GOVERNANCE_CAPABILITIES_SCHEMA_V1: &str = "iroha.governance.capabilities.v1";
/// Current strict governance readiness projection version.
pub const GOVERNANCE_CAPABILITIES_VERSION_V1: u16 = 1;

/// Project the exact first-release validation-fee PLAIN electorate contract
/// from the active governance configuration.
#[must_use]
pub(crate) fn validation_fee_plain_electorate_rules(
    gov: &iroha_config::parameters::actual::Governance,
) -> ValidationFeePlainElectorateRulesV1 {
    ValidationFeePlainElectorateRulesV1 {
        voting_asset_id: gov.voting_asset_id.clone(),
        bond_escrow_account: gov.bond_escrow_account.clone(),
        slash_receiver_account: gov.slash_receiver_account.clone(),
        ballot_amount: gov.min_bond_amount.clone(),
        ballot_duration_blocks: gov.window_span,
        citizenship_amount: gov.citizenship_bond_amount.clone(),
        max_members: VALIDATION_FEE_PLAIN_MAX_MEMBERS_V1,
        conviction_step_blocks: gov.conviction_step_blocks,
        max_conviction: gov.max_conviction,
        min_turnout: gov.min_turnout,
        approval_threshold_numerator: gov.approval_threshold_q_num,
        approval_threshold_denominator: gov.approval_threshold_q_den,
        eligibility_rule:
            ValidationFeePlainElectorateEligibilityRuleV1::ProposalOperatorAtOrBeforeGateOthersAfterGate,
    }
}

/// Configured target sizes for all seven SORA Parliament bodies.
#[derive(Debug, Clone, JsonSerialize)]
pub struct GovernanceTargetBodySizesV1 {
    /// Rules Committee target seats.
    pub rules_committee: String,
    /// Agenda Council target seats.
    pub agenda_council: String,
    /// Interest Panel target seats.
    pub interest_panel: String,
    /// Review Panel target seats.
    pub review_panel: String,
    /// Policy Jury target seats.
    pub policy_jury: String,
    /// Oversight Committee target seats.
    pub oversight_committee: String,
    /// FMA Committee target seats.
    pub fma_committee: String,
}

/// Public fail-closed governance configuration and route projection.
#[derive(Debug, JsonSerialize)]
pub struct GovernanceCapabilitiesV1 {
    /// Stable projection schema identifier.
    pub schema: String,
    /// Projection layout version.
    pub version: u16,
    /// Exact chain identifier.
    pub chain_id: String,
    /// Lowercase committed genesis block hash.
    pub genesis_hash: String,
    /// Latest committed block height.
    pub current_height: String,
    /// I105 network prefix used by this chain.
    pub network_prefix: String,
    /// Active IVM ABI version.
    pub abi_version: String,
    /// Data-model compatibility version.
    pub data_model_version: String,
    /// Exact configured governance approval mode.
    pub approval_mode: String,
    /// Whether first-release PLAIN voting is enabled.
    pub plain_voting_enabled: bool,
    /// Whether PLAIN referenda deterministically finalize after their inclusive end height.
    pub auto_finalize_plain: bool,
    /// Exact scope in which deterministic PLAIN auto-finalization remains enabled.
    pub auto_finalize_plain_scope: String,
    /// Validation-fee PLAIN referenda require an explicit typed finalization instruction.
    pub validation_fee_plain_requires_explicit_finalization: bool,
    /// Citizenship bond asset.
    pub citizenship_asset_id: String,
    /// Exact citizenship bond as a decimal string.
    pub citizenship_bond_amount: String,
    /// Account that custodies citizenship bonds.
    pub citizenship_escrow_account: String,
    /// Citizen ballot bond asset.
    pub voting_asset_id: String,
    /// Exact minimum citizen ballot bond as a decimal string.
    pub min_bond_amount: String,
    /// Account that custodies citizen ballot bonds.
    pub bond_escrow_account: String,
    /// Exact immutable PLAIN electorate rules required by validation-fee proposals.
    pub validation_fee_plain_electorate_rules: ValidationFeePlainElectorateRulesV1,
    /// Conviction step in blocks.
    pub conviction_step_blocks: String,
    /// Maximum conviction multiplier.
    pub max_conviction: String,
    /// Proposal-to-voting minimum delay in blocks.
    pub min_enactment_delay: String,
    /// Inclusive referendum span in blocks.
    pub window_span: String,
    /// Exact minimum turnout as a decimal string.
    pub min_turnout: String,
    /// Approval fraction numerator.
    pub approval_threshold_numerator: String,
    /// Approval fraction denominator.
    pub approval_threshold_denominator: String,
    /// Parliament approval quorum in basis points.
    pub parliament_quorum_bps: String,
    /// Configured targets; actual proposal rosters are capped by eligible citizens.
    pub target_body_sizes: GovernanceTargetBodySizesV1,
    /// Typed proposal kinds supported by the first release.
    pub supported_proposal_kinds: Vec<String>,
    /// Canonical public governance routes supported by the node.
    pub supported_routes: Vec<String>,
}

fn governance_approval_mode(state: &iroha_core::state::State) -> String {
    let catalog = state.nexus_snapshot().governance;
    let configured = catalog
        .default_module
        .as_deref()
        .and_then(|name| catalog.modules.get(name))
        .and_then(|module| module.module_type.as_deref())
        .or(catalog.default_module.as_deref())
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .replace('-', "_");
    if configured.contains("parliament") || configured.contains("sortition") {
        "PARLIAMENT_SORTITION_JIT".to_owned()
    } else {
        "LEGACY_COUNCIL_EPOCH".to_owned()
    }
}

/// GET `/v1/gov/capabilities` — return strict public governance readiness.
///
/// # Errors
/// Returns an internal query error before a committed genesis block exists.
pub async fn handle_gov_capabilities(
    state: Arc<iroha_core::state::State>,
) -> Result<JsonBody<GovernanceCapabilitiesV1>, crate::Error> {
    let genesis_hash = state
        .committed_block_hashes_snapshot()
        .first()
        .copied()
        .ok_or_else(|| {
            crate::Error::Query(iroha_data_model::ValidationFail::InternalError(
                "governance capabilities are unavailable before committed genesis".into(),
            ))
        })?;
    let gov = state.governance_snapshot();
    let validation_fee_plain_electorate_rules = validation_fee_plain_electorate_rules(&gov);
    let world = state.world_view();
    Ok(JsonBody(GovernanceCapabilitiesV1 {
        schema: GOVERNANCE_CAPABILITIES_SCHEMA_V1.to_owned(),
        version: GOVERNANCE_CAPABILITIES_VERSION_V1,
        chain_id: state.chain_id_ref().to_string(),
        genesis_hash: hex::encode(genesis_hash.as_ref()),
        current_height: u64::try_from(state.committed_height())
            .unwrap_or(u64::MAX)
            .to_string(),
        network_prefix: iroha_data_model::account::address::chain_discriminant().to_string(),
        abi_version: world.abi_version().to_string(),
        data_model_version: iroha_data_model::DATA_MODEL_VERSION.to_string(),
        approval_mode: governance_approval_mode(state.as_ref()),
        plain_voting_enabled: gov.plain_voting_enabled,
        auto_finalize_plain: true,
        auto_finalize_plain_scope: "GENERIC_NON_VALIDATION_FEE_ONLY".to_owned(),
        validation_fee_plain_requires_explicit_finalization: true,
        citizenship_asset_id: gov.citizenship_asset_id.to_string(),
        citizenship_bond_amount: gov.citizenship_bond_amount.to_string(),
        citizenship_escrow_account: gov.citizenship_escrow_account.to_string(),
        voting_asset_id: gov.voting_asset_id.to_string(),
        min_bond_amount: gov.min_bond_amount.to_string(),
        bond_escrow_account: gov.bond_escrow_account.to_string(),
        validation_fee_plain_electorate_rules,
        conviction_step_blocks: gov.conviction_step_blocks.to_string(),
        max_conviction: gov.max_conviction.to_string(),
        min_enactment_delay: gov.min_enactment_delay.to_string(),
        window_span: gov.window_span.to_string(),
        min_turnout: gov.min_turnout.to_string(),
        approval_threshold_numerator: gov.approval_threshold_q_num.to_string(),
        approval_threshold_denominator: gov.approval_threshold_q_den.to_string(),
        parliament_quorum_bps: gov.parliament_quorum_bps.to_string(),
        target_body_sizes: GovernanceTargetBodySizesV1 {
            rules_committee: gov.rules_committee_size.to_string(),
            agenda_council: gov.agenda_council_size.to_string(),
            interest_panel: gov.interest_panel_size.to_string(),
            review_panel: gov.review_panel_size.to_string(),
            policy_jury: gov.policy_jury_size.to_string(),
            oversight_committee: gov.oversight_committee_size.to_string(),
            fma_committee: gov.fma_committee_size.to_string(),
        },
        supported_proposal_kinds: vec![
            "VALIDATION_FEE_PAYOUT_LIFECYCLE".to_owned(),
            "VALIDATION_FEE_POLICY".to_owned(),
        ],
        supported_routes: vec![
            "/v1/gov/capabilities".to_owned(),
            "/v1/gov/citizens/draft".to_owned(),
            "/v1/validation-fee/proposals".to_owned(),
            "/v1/validation-fee/proposals/{proposal_id}".to_owned(),
            "/v1/validation-fee/proposals/draft".to_owned(),
            "/v1/validation-fee/proposals/{proposal_id}/plain-ballot/draft".to_owned(),
            "/v1/gov/parliament/ballots".to_owned(),
            "/v1/gov/ballots/plain".to_owned(),
            "/v1/gov/enact".to_owned(),
        ],
    }))
}

/// Strict citizen registration draft request.
#[derive(Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
pub struct CitizenDraftRequestV1 {
    /// Request layout version.
    pub version: u16,
    /// Canonical citizen account that will sign the resulting instruction.
    pub owner: iroha_data_model::account::AccountId,
}

/// Exact native citizen registration draft.
#[derive(Debug, JsonSerialize)]
pub struct CitizenDraftResponseV1 {
    /// Response layout version.
    pub version: u16,
    /// Canonical citizen account.
    pub owner: iroha_data_model::account::AccountId,
    /// Exact configured bond as a decimal string.
    pub amount: String,
    /// Exactly one canonical `RegisterCitizen` instruction.
    pub tx_instructions: Vec<TxInstr>,
}

/// POST `/v1/gov/citizens/draft` — build the exact configured citizenship instruction.
///
/// # Errors
/// Returns a conversion error for unsupported request versions.
pub async fn handle_gov_citizen_draft(
    state: Arc<iroha_core::state::State>,
    NoritoJson(body): NoritoJson<CitizenDraftRequestV1>,
) -> Result<JsonBody<CitizenDraftResponseV1>, crate::Error> {
    if body.version != GOVERNANCE_CAPABILITIES_VERSION_V1 {
        return Err(crate::routing::conversion_error(
            "unsupported governance citizen draft version".into(),
        ));
    }
    let amount = state.governance_snapshot().citizenship_bond_amount;
    let instruction = iroha_data_model::isi::governance::RegisterCitizen {
        owner: body.owner.clone(),
        amount: amount.clone(),
    };
    Ok(JsonBody(CitizenDraftResponseV1 {
        version: GOVERNANCE_CAPABILITIES_VERSION_V1,
        owner: body.owner,
        amount: amount.to_string(),
        tx_instructions: vec![tx_instr_from_box(instruction.into())],
    }))
}

/// GET /v1/gov/citizens — return the exact citizenship registry count.
///
/// # Errors
/// This handler never returns an error; an empty registry is represented as `total = 0`.
pub async fn handle_gov_citizen_count(
    state: Arc<iroha_core::state::State>,
) -> Result<JsonBody<CitizenCountResponse>, crate::Error> {
    let world = state.world_view();
    Ok(JsonBody(CitizenCountResponse {
        total: world.citizens().iter().count().to_string(),
    }))
}

/// GET /v1/gov/citizens/{account_id} — read the citizenship registry entry for an account.
///
/// # Errors
/// Returns a conversion error when the account id path segment is invalid.
pub async fn handle_gov_citizen_status(
    state: Arc<iroha_core::state::State>,
    account_id: axum::extract::Path<String>,
    telemetry: MaybeTelemetry,
) -> Result<JsonBody<CitizenStatusResponse>, crate::Error> {
    let (account, canonical_account_id) = parse_account_literal_with_state(
        state.as_ref(),
        &account_id.0,
        &telemetry,
        "/v1/gov/citizens/{account_id}",
    )
    .map_err(|err| {
        crate::routing::conversion_error(format!("invalid account_id: {}", err.reason()))
    })?;
    let world = state.world_view();
    let record = world.citizens().get(&account).cloned();
    Ok(JsonBody(CitizenStatusResponse {
        account_id: canonical_account_id.to_string(),
        is_citizen: record.is_some(),
        amount: record.as_ref().map(|record| record.amount.to_string()),
        bonded_height: record
            .as_ref()
            .map(|record| record.bonded_height.to_string()),
        seats_in_epoch: record
            .as_ref()
            .map(|record| record.seats_in_epoch.to_string()),
        last_epoch_seen: record
            .as_ref()
            .map(|record| record.last_epoch_seen.to_string()),
        cooldown_until: record
            .as_ref()
            .map(|record| record.cooldown_until.to_string()),
    }))
}

// --- Unlock sweep stats (operator/audit) ---
/// Response with lock/unlock statistics.
#[derive(Copy, Clone, Debug, JsonSerialize)]
pub struct UnlockStatsResponse {
    /// Current height used for evaluation
    pub height_current: u64,
    /// Number of locks that would be expired at current height across all referenda
    pub expired_locks_now: u64,
    /// Number of referenda that have at least one expired lock
    pub referenda_with_expired: u64,
    /// Height at which expired locks were last swept and persisted
    pub last_sweep_height: u64,
}

/// Compute and return aggregate unlock statistics for governance locks.
///
/// # Errors
/// This handler never returns an error; the response always reflects the current view snapshot.
pub async fn handle_gov_unlock_stats(
    state: Arc<iroha_core::state::State>,
) -> Result<JsonBody<UnlockStatsResponse>, crate::Error> {
    let world = state.world_view();
    let now_h = state.committed_height() as u64;
    let mut expired_locks_now: u64 = 0;
    let mut refs_with_expired: u64 = 0;
    for (_rid, rec) in world.governance_locks().iter() {
        let mut any = false;
        for (_owner, l) in rec.locks.iter() {
            if l.expiry_height <= now_h {
                expired_locks_now += 1;
                any = true;
            }
        }
        if any {
            refs_with_expired += 1;
        }
    }
    let last_sweep_height = *world.governance_last_unlock_sweep_height();
    Ok(JsonBody(UnlockStatsResponse {
        height_current: now_h,
        expired_locks_now,
        referenda_with_expired: refs_with_expired,
        last_sweep_height,
    }))
}

#[derive(Debug, JsonSerialize)]
/// Instruction skeleton item for client-side signing.
///
/// `wire_id` identifies the instruction on the wire; `payload_hex` carries
/// the Norito-encoded payload as lowercase hex without `0x`.
pub struct TxInstr {
    pub wire_id: String,
    pub payload_hex: String,
}

fn tx_instr_from_box(boxed: iroha_data_model::isi::InstructionBox) -> TxInstr {
    use iroha_data_model::isi::Instruction;

    let type_name = Instruction::id(&*boxed);
    let wire_id = type_name.to_string();
    let payload = Instruction::dyn_encode(&*boxed);
    let framed = iroha_data_model::isi::frame_instruction_payload(type_name, &payload)
        .expect("instruction payload must use canonical Norito framing");
    TxInstr {
        wire_id,
        payload_hex: hex::encode(framed),
    }
}

fn ensure_chain_id_matches(
    chain_id: &iroha_data_model::ChainId,
    provided: &str,
) -> Result<(), crate::Error> {
    let provided = provided.trim();
    if provided.is_empty() {
        return Err(crate::routing::conversion_error(
            "chain_id must not be empty".into(),
        ));
    }
    if chain_id.as_str() != provided {
        return Err(crate::routing::conversion_error(format!(
            "chain_id mismatch: expected {}, got {}",
            chain_id.as_str(),
            provided
        )));
    }
    Ok(())
}

fn parse_account_literal_from_state(
    state: &iroha_core::state::State,
    raw: &str,
    telemetry: &MaybeTelemetry,
    context: &'static str,
) -> Result<iroha_data_model::account::AccountId, iroha_data_model::error::ParseError> {
    parse_account_literal_with_state(state, raw, telemetry, context)
        .map(|(account_id, _)| account_id)
}

fn parse_authority_literal(
    state: &iroha_core::state::State,
    raw: &str,
    telemetry: &MaybeTelemetry,
    context: &'static str,
) -> Result<iroha_data_model::account::AccountId, crate::Error> {
    parse_account_literal_from_state(state, raw, telemetry, context).map_err(|err| {
        crate::routing::conversion_error(format!("invalid authority: {}", err.reason()))
    })
}

fn parse_canonical_authority_literal(
    state: &iroha_core::state::State,
    raw: &str,
    telemetry: &MaybeTelemetry,
    context: &'static str,
) -> Result<iroha_data_model::account::AccountId, crate::Error> {
    let trimmed = raw.trim();
    let canonical = iroha_data_model::account::AccountId::canonicalize(trimmed).map_err(|_| {
        crate::routing::conversion_error("authority must use canonical I105 account id form".into())
    })?;
    if canonical != trimmed {
        return Err(crate::routing::conversion_error(
            "authority must use canonical I105 account id form".into(),
        ));
    }
    parse_authority_literal(state, trimmed, telemetry, context)
}

fn instruction_skeleton_for_propose(
    instr: &iroha_data_model::isi::governance::ProposeDeployContract,
) -> Vec<TxInstr> {
    let boxed: iroha_data_model::isi::InstructionBox = instr.clone().into();
    vec![tx_instr_from_box(boxed)]
}

fn instruction_skeleton_for_sccp_route_governance_propose(
    instr: &iroha_data_model::isi::governance::ProposeSccpRouteGovernance,
) -> Vec<TxInstr> {
    let boxed: iroha_data_model::isi::InstructionBox = instr.clone().into();
    vec![tx_instr_from_box(boxed)]
}

fn build_signable_transaction_b64(
    chain_id: &iroha_data_model::ChainId,
    authority: &iroha_data_model::account::AccountId,
    instructions: Vec<iroha_data_model::isi::InstructionBox>,
) -> String {
    let builder = iroha_data_model::transaction::signed::TransactionBuilder::new(
        chain_id.clone(),
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(instructions);
    base64::engine::general_purpose::STANDARD.encode(builder.encode_payload())
}

fn canonical_hex32(value: &str, field: &str) -> Result<(String, [u8; 32]), crate::Error> {
    let trimmed = value.trim();
    let without_scheme = if let Some((scheme, rest)) = trimmed.split_once(':') {
        if scheme.is_empty() || scheme.eq_ignore_ascii_case("blake2b32") {
            rest
        } else {
            return Err(crate::Error::Query(
                iroha_data_model::ValidationFail::QueryFailed(
                    iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                        "unsupported {field} scheme"
                    )),
                ),
            ));
        }
    } else {
        trimmed
    };
    let body = without_scheme.trim();
    let body = body
        .strip_prefix("0x")
        .or_else(|| body.strip_prefix("0X"))
        .unwrap_or(body)
        .trim();
    if body.len() != 64 || !body.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(crate::Error::Query(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                    "{field} must be 32-byte hex"
                )),
            ),
        ));
    }
    let canonical = body.to_ascii_lowercase();
    let mut out = [0u8; 32];
    if let Err(e) = hex::decode_to_slice(&canonical, &mut out) {
        return Err(crate::Error::Query(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                    "failed to decode {field}: {e}"
                )),
            ),
        ));
    }
    Ok((canonical, out))
}

fn compute_proposal_id(
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
    code_hash: &[u8; 32],
    abi_hash: &[u8; 32],
) -> [u8; 32] {
    use iroha_crypto::blake2::{Blake2b512, digest::Digest};

    let contract_address_literal = contract_address.as_ref();
    let contract_address_len = contract_address_literal.len() as u32;
    let mut input = Vec::with_capacity(
        b"iroha:gov:proposal:v1|".len()
            + core::mem::size_of::<u32>()
            + contract_address_literal.len()
            + code_hash.len()
            + abi_hash.len(),
    );
    input.extend_from_slice(b"iroha:gov:proposal:v1|");
    input.extend_from_slice(&contract_address_len.to_le_bytes());
    input.extend_from_slice(contract_address_literal.as_bytes());
    input.extend_from_slice(code_hash);
    input.extend_from_slice(abi_hash);
    let digest = Blake2b512::digest(&input);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
}

fn compute_sccp_route_governance_proposal_id(
    action: &iroha_data_model::isi::bridge::SccpRouteGovernanceActionV1,
) -> Result<[u8; 32], crate::Error> {
    let canonical = action.encode();
    let action_len: u32 = canonical.len().try_into().map_err(|_| {
        crate::routing::conversion_error(
            "SCCP route governance action length exceeds 2^32 bytes".into(),
        )
    })?;
    let mut input = Vec::with_capacity(
        b"iroha:gov:sccp-route-governance:proposal:v1|".len()
            + core::mem::size_of::<u32>()
            + canonical.len(),
    );
    input.extend_from_slice(b"iroha:gov:sccp-route-governance:proposal:v1|");
    input.extend_from_slice(&action_len.to_le_bytes());
    input.extend_from_slice(&canonical);
    let digest = Blake2b512::digest(&input);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    Ok(out)
}

fn resolve_governance_contract_target(
    state: &iroha_core::state::State,
    contract_address: Option<&iroha_data_model::smart_contract::ContractAddress>,
    contract_alias: Option<&iroha_data_model::smart_contract::ContractAlias>,
) -> Result<iroha_data_model::smart_contract::ContractAddress, crate::Error> {
    match (contract_address, contract_alias) {
        (Some(_), Some(_)) => Err(crate::Error::Query(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "exactly one of contract_address or contract_alias must be provided".into(),
                ),
            ),
        )),
        (Some(contract_address), None) => Ok(contract_address.clone()),
        (None, Some(contract_alias)) => {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            state
                .world_view()
                .contract_address_by_alias_at(contract_alias, now_ms)
                .ok_or_else(|| {
                    crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                        iroha_data_model::query::error::QueryExecutionFail::NotFound,
                    ))
                })
        }
        (None, None) => Err(crate::Error::Query(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "provide exactly one contract target via contract_address or contract_alias"
                        .into(),
                ),
            ),
        )),
    }
}

#[derive(Debug, JsonSerialize)]
/// Response payload for GET /v1/gov/proposals/{id}
pub struct ProposalGetResponse {
    /// Whether the proposal exists.
    pub found: bool,
    #[norito(skip_serializing_if = "Option::is_none")]
    /// Proposal record if found.
    pub proposal: Option<iroha_core::state::GovernanceProposalRecord>,
}

#[derive(Debug, JsonSerialize)]
/// Response payload for GET /v1/gov/locks/{rid}
pub struct LocksGetResponse {
    /// Whether locks exist for the given referendum id.
    pub found: bool,
    /// Referendum id echoed.
    pub referendum_id: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    /// Locks record, when present.
    pub locks: Option<iroha_core::state::GovernanceLocksForReferendum>,
}

/// Response payload for GET /v1/gov/referenda/{id}
/// Response payload for referendum lookup by id.
#[derive(Copy, Clone, Debug, JsonSerialize)]
pub struct ReferendumGetResponse {
    /// Whether the referendum exists.
    pub found: bool,
    #[norito(skip_serializing_if = "Option::is_none")]
    /// Referendum record if found.
    pub referendum: Option<iroha_core::state::GovernanceReferendumRecord>,
}

#[derive(Debug, JsonSerialize)]
/// Response payload for GET /v1/gov/tally/{id}
pub struct TallyGetResponse {
    /// Referendum id.
    pub referendum_id: String,
    /// Committed block height whose state was used for the tally.
    pub evaluated_block_height: u64,
    /// Committed block hash whose state was used for the tally.
    pub evaluated_block_hash: String,
    /// Approve votes.
    pub approve: u128,
    /// Reject votes.
    pub reject: u128,
    /// Abstain votes.
    pub abstain: u128,
}

/// Handler for fetching a proposal record by hex id.
///
/// # Errors
/// Returns `crate::Error::Query` when the provided identifier is not valid 32-byte hex.
pub async fn handle_gov_get_proposal(
    state: Arc<iroha_core::state::State>,
    id: axum::extract::Path<String>,
) -> Result<JsonBody<ProposalGetResponse>, crate::Error> {
    let hex = id.0;
    let bytes = decode_hex(&hex).map_err(|_| {
        crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion("invalid id".into()),
        ))
    })?;
    if bytes.len() != 32 {
        return Err(crate::Error::Query(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "invalid id length".into(),
                ),
            ),
        ));
    }
    let mut id_arr = [0u8; 32];
    id_arr.copy_from_slice(&bytes);
    let world = state.world_view();
    let found = world.governance_proposals().get(&id_arr).cloned();
    Ok(JsonBody(ProposalGetResponse {
        found: found.is_some(),
        proposal: found,
    }))
}

/// Handler for fetching governance locks by referendum id.
///
/// # Errors
/// This handler never returns an error; missing locks are reported with `found = false`.
pub async fn handle_gov_get_locks(
    state: Arc<iroha_core::state::State>,
    rid: axum::extract::Path<String>,
) -> Result<JsonBody<LocksGetResponse>, crate::Error> {
    let ref_id = rid.0;
    let world = state.world_view();
    let found = world.governance_locks().get(&ref_id).cloned();
    Ok(JsonBody(LocksGetResponse {
        found: found.is_some(),
        referendum_id: ref_id,
        locks: found,
    }))
}

/// Handler for fetching a referendum by id.
///
/// # Errors
/// This handler never returns an error; missing referenda are returned with `found = false`.
pub async fn handle_gov_get_referendum(
    state: Arc<iroha_core::state::State>,
    id: axum::extract::Path<String>,
) -> Result<JsonBody<ReferendumGetResponse>, crate::Error> {
    let rid = id.0;
    let world = state.world_view();
    let found = world.governance_referenda().get(&rid).copied();
    Ok(JsonBody(ReferendumGetResponse {
        found: found.is_some(),
        referendum: found,
    }))
}

/// Handler for computing a referendum tally summary.
///
/// # Errors
/// Returns a conversion error if an exact quadratic weight or tally exceeds
/// the fixed consensus tally domain, a PLAIN lock carries an invalid direction,
/// or the referendum belongs to the validation-fee governance flow. Validation-fee
/// callers must use the typed proposal-detail endpoint, which validates the frozen
/// electorate and retained ballot rules. Missing referenda return `NotFound`.
pub async fn handle_gov_get_tally(
    state: Arc<iroha_core::state::State>,
    id: axum::extract::Path<String>,
) -> Result<JsonBody<TallyGetResponse>, crate::Error> {
    let rid = id.0;
    let world = state.world_view();
    let mut proposal_id = [0_u8; 32];
    let is_validation_fee_referendum = rid.len() == 64
        && hex::decode_to_slice(&rid, &mut proposal_id).is_ok()
        && world
            .governance_proposals()
            .get(&proposal_id)
            .is_some_and(|proposal| {
                matches!(
                    &proposal.kind,
                    iroha_data_model::governance::types::ProposalKind::ValidationFeePolicy(_)
                        | iroha_data_model::governance::types::ProposalKind::ValidationFeePayoutLifecycle(_)
                )
            });
    if is_validation_fee_referendum {
        let typed_proposal_id = hex::encode(proposal_id);
        return Err(crate::routing::conversion_error(format!(
            "validation-fee referendum tally requires the typed \
             /v1/validation-fee/proposals/{typed_proposal_id} endpoint"
        )));
    }
    let referendum = world
        .governance_referenda()
        .get(&rid)
        .copied()
        .ok_or_else(|| {
            crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::NotFound,
            ))
        })?;
    let evaluated_block_height = state.committed_height() as u64;
    let evaluated_block_hash = match state.latest_block_hash_fast() {
        Some(hash) => hex::encode(hash.as_ref()),
        None if evaluated_block_height == 0 => hex::encode([0_u8; 32]),
        None => {
            return Err(crate::Error::Query(
                iroha_data_model::ValidationFail::InternalError(
                    "governance snapshot height has no committed block hash".to_owned(),
                ),
            ));
        }
    };
    let gov_cfg = state.gov.clone();
    // Mirror FinalizeReferendum tally logic without mutating state.
    let now_h = state.committed_height() as u64;
    let mut approve: u128 = 0;
    let mut reject: u128 = 0;
    let mut abstain: u128 = 0;
    match referendum.mode {
        iroha_core::state::GovernanceReferendumMode::Plain => {
            if let Some(locks) = world.governance_locks().get(&rid) {
                let step = gov_cfg.conviction_step_blocks.max(1);
                let max_c = gov_cfg.max_conviction;
                for (_owner, rec) in locks.locks.iter() {
                    if rec.expiry_height < now_h {
                        continue;
                    }
                    if rec.amount.scale() != 0 {
                        return Err(crate::routing::conversion_error(
                            "plain ballot lock amount must have scale zero".into(),
                        ));
                    }
                    let units = rec.amount.as_numeric().try_mantissa_u128().ok_or_else(|| {
                        crate::routing::conversion_error(
                            "plain ballot lock amount exceeds u128 voting range".into(),
                        )
                    })?;
                    let w = checked_plain_tally_weight(units, rec.duration_blocks, step, max_c)?;
                    match rec.direction {
                        0 => {
                            approve = approve.checked_add(w).ok_or_else(tally_overflow_error)?;
                        }
                        1 => {
                            reject = reject.checked_add(w).ok_or_else(tally_overflow_error)?;
                        }
                        2 => {
                            abstain = abstain.checked_add(w).ok_or_else(tally_overflow_error)?;
                        }
                        direction => {
                            return Err(crate::routing::conversion_error(format!(
                                "plain ballot lock has invalid direction {direction}; \
                                 expected 0, 1, or 2"
                            )));
                        }
                    }
                }
            }
        }
        iroha_core::state::GovernanceReferendumMode::Zk => {
            if let Some(e) = world.elections().get(&rid) {
                if e.finalized && e.tally.len() >= 2 {
                    approve = e.tally[0] as u128;
                    reject = e.tally[1] as u128;
                }
            }
        }
    }
    Ok(JsonBody(TallyGetResponse {
        referendum_id: rid,
        evaluated_block_height,
        evaluated_block_hash,
        approve,
        reject,
        abstain,
    }))
}

fn checked_plain_tally_weight(
    units: u128,
    duration_blocks: u64,
    conviction_step_blocks: u64,
    max_conviction: u64,
) -> Result<u128, crate::Error> {
    let base = integer_sqrt_u128(units);
    let step = conviction_step_blocks.max(1);
    let factor = (u128::from(duration_blocks / step) + 1).min(u128::from(max_conviction));
    base.checked_mul(factor).ok_or_else(tally_overflow_error)
}

fn tally_overflow_error() -> crate::Error {
    crate::routing::conversion_error("governance tally arithmetic overflow".into())
}

fn integer_sqrt_u128(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    let mut x0 = n;
    let mut x1 = u128::midpoint(x0, n / x0);
    while x1 < x0 {
        x0 = x1;
        x1 = u128::midpoint(x0, n / x0);
    }
    x0
}

#[derive(Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
/// Request body for finalizing a referendum
pub struct FinalizeDto {
    /// Referendum identifier
    pub referendum_id: String,
    /// Proposal id (hex 64)
    pub proposal_id: String,
}

#[derive(Debug, JsonSerialize)]
/// Response for finalize referendum draft transaction
pub struct FinalizeResponse {
    /// Whether the operation succeeded.
    pub ok: bool,
    /// Suggested transaction instructions for clients to sign.
    pub tx_instructions: Vec<TxInstr>,
}

/// Handler for finalizing a referendum (draft transaction).
///
/// The request schema excludes private signing material; callers submit locally signed transactions.
///
/// # Errors
/// Returns `crate::Error::Query` when `proposal_id` is not 32-byte hex.
pub async fn handle_gov_finalize(
    NoritoJson(body): NoritoJson<FinalizeDto>,
) -> Result<JsonBody<FinalizeResponse>, crate::Error> {
    // Parse proposal id hex
    let bytes = hex::decode(body.proposal_id.trim_start_matches("0x")).map_err(|_| {
        crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(
                "invalid proposal_id".into(),
            ),
        ))
    })?;
    if bytes.len() != 32 {
        return Err(crate::Error::Query(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "invalid proposal_id length".into(),
                ),
            ),
        ));
    }
    let mut id_arr = [0u8; 32];
    id_arr.copy_from_slice(&bytes);
    let instr = iroha_data_model::isi::governance::FinalizeReferendum {
        referendum_id: body.referendum_id,
        proposal_id: id_arr,
    };
    let boxed: iroha_data_model::isi::InstructionBox = instr.clone().into();
    let tx_instructions = vec![tx_instr_from_box(boxed)];
    Ok(JsonBody(FinalizeResponse {
        ok: true,
        tx_instructions,
    }))
}

#[derive(Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
/// Request body for enacting an approved referendum.
pub struct EnactDto {
    /// Proposal id as exactly 64 lowercase hexadecimal digits.
    pub proposal_id: String,
}

#[derive(Debug, JsonSerialize)]
/// Response for enactment draft transaction.
pub struct EnactResponse {
    pub ok: bool,
    /// Exact lowercase proposal fingerprint.
    pub proposal_id: String,
    /// Exact stored proposal kind whose fingerprint is bound into the instruction.
    pub proposal_kind: iroha_data_model::governance::types::ProposalKind,
    /// Exact retained referendum window.
    pub referendum_window: iroha_data_model::governance::types::AtWindow,
    pub tx_instructions: Vec<TxInstr>,
}

fn validate_validation_fee_policy_enactment_draft_height(
    effective_from_height: u64,
    current_tip: u64,
) -> Result<u64, String> {
    let target = effective_from_height
        .checked_sub(
            iroha_data_model::validation_fee::VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS,
        )
        .ok_or_else(|| {
            "validation-fee policy effective height cannot encode the exact activation delay"
                .to_owned()
        })?;
    let next_height = current_tip.checked_add(1).ok_or_else(|| {
        "validation-fee policy enactment target exceeds the block-height domain".to_owned()
    })?;
    match next_height.cmp(&target) {
        core::cmp::Ordering::Less => Err(format!(
            "validation-fee policy enactment is not ready; submit for exact block height {target}"
        )),
        core::cmp::Ordering::Greater => Err(format!(
            "validation-fee policy exact enactment height {target} was missed"
        )),
        core::cmp::Ordering::Equal => Ok(target),
    }
}

/// Handler for building an enactment transaction (draft only).
///
/// # Errors
/// Returns `crate::Error::Query` when the proposal is missing, not approved, or inconsistent.
pub async fn handle_gov_enact(
    state: Arc<iroha_core::state::State>,
    NoritoJson(body): NoritoJson<EnactDto>,
) -> Result<JsonBody<EnactResponse>, crate::Error> {
    if body.proposal_id.len() != 64
        || !body
            .proposal_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(crate::routing::conversion_error(
            "proposal_id must be exactly 64 lowercase hexadecimal digits".into(),
        ));
    }
    let bytes = hex::decode(&body.proposal_id).map_err(|_| {
        crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(
                "invalid proposal_id".into(),
            ),
        ))
    })?;
    if bytes.len() != 32 {
        return Err(crate::Error::Query(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "invalid proposal_id length".into(),
                ),
            ),
        ));
    }
    let mut pid = [0u8; 32];
    pid.copy_from_slice(&bytes);
    let current_tip = u64::try_from(state.committed_height()).map_err(|_| {
        crate::routing::conversion_error(
            "ledger height does not fit validation-fee enactment timing".into(),
        )
    })?;
    let world = state.world_view();
    let proposal = world.governance_proposals().get(&pid).ok_or_else(|| {
        crate::routing::conversion_error("approved governance proposal was not found".into())
    })?;
    if !matches!(
        proposal.status,
        iroha_core::state::GovernanceProposalStatus::Approved
            | iroha_core::state::GovernanceProposalStatus::Enacted
    ) || !proposal
        .finalization_evidence
        .as_ref()
        .is_some_and(|evidence| evidence.approved)
    {
        return Err(crate::routing::conversion_error(
            "governance proposal has no approved finalization evidence".into(),
        ));
    }
    let referendum = world
        .governance_referenda()
        .get(&body.proposal_id)
        .copied()
        .ok_or_else(|| {
            crate::routing::conversion_error(
                "governance proposal has no exact retained referendum".into(),
            )
        })?;
    if referendum.status != iroha_core::state::GovernanceReferendumStatus::Closed {
        return Err(crate::routing::conversion_error(
            "governance referendum is not closed".into(),
        ));
    }
    if let iroha_data_model::governance::types::ProposalKind::ValidationFeePolicy(payload) =
        &proposal.kind
    {
        validate_validation_fee_policy_enactment_draft_height(
            payload.policy.effective_from_height,
            current_tip,
        )
        .map_err(|message| crate::routing::conversion_error(message.into()))?;
    }
    let proposal_kind = proposal.kind.clone();
    let referendum_window = iroha_data_model::governance::types::AtWindow {
        lower: referendum.h_start,
        upper: referendum.h_end,
    };
    let instr = iroha_data_model::isi::governance::EnactReferendum {
        referendum_id: pid,
        preimage_hash: proposal_kind.fingerprint(),
        at_window: referendum_window,
    };
    let boxed: iroha_data_model::isi::InstructionBox = instr.clone().into();
    let tx_instructions = vec![tx_instr_from_box(boxed)];
    Ok(JsonBody(EnactResponse {
        ok: true,
        proposal_id: body.proposal_id,
        proposal_kind,
        referendum_window,
        tx_instructions,
    }))
}

#[derive(Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
/// Request body for applying protected namespaces parameter.
pub struct ProtectedNamespacesDto {
    /// Namespaces to protect (e.g., `["apps", "system"]`).
    pub namespaces: Vec<String>,
    /// Optional canonical I105 account id used to build a signable transaction payload.
    #[norito(default)]
    pub authority: Option<String>,
}

#[derive(Debug, JsonSerialize)]
/// Response to drafting a protected namespaces parameter transaction.
pub struct ProtectedNamespacesApplyResponse {
    pub ok: bool,
    pub namespace_count: usize,
    pub submitted: bool,
    pub tx_instructions: Vec<TxInstr>,
    pub signable_transaction_b64: Option<String>,
}

/// POST /v1/gov/protected-namespaces — draft a custom-parameter transaction.
///
/// # Errors
/// Returns `crate::Error::Query` when the namespaces cannot be serialized into
/// the custom parameter or the optional authority cannot be resolved.
pub async fn handle_gov_protected_set(
    chain_id: Arc<iroha_data_model::ChainId>,
    state: Arc<iroha_core::state::State>,
    telemetry: MaybeTelemetry,
    NoritoJson(body): NoritoJson<ProtectedNamespacesDto>,
) -> Result<JsonBody<ProtectedNamespacesApplyResponse>, crate::Error> {
    use std::str::FromStr as _;

    use iroha_data_model::parameter::{CustomParameterId, Parameter, custom::CustomParameter};

    // Validate namespace strings are non-empty ASCII (basic check)
    let filtered: Vec<String> = body
        .namespaces
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let namespace_count = filtered.len();
    let name = iroha_data_model::name::Name::from_str("gov_protected_namespaces").map_err(|e| {
        crate::Error::Query(iroha_data_model::ValidationFail::InternalError(
            e.to_string(),
        ))
    })?;
    let id = CustomParameterId(name);
    // Convert Vec<String> -> Vec<&str> to satisfy Json's From<Vec<T>> bound
    let json_array = norito::json::native::Value::Array(
        filtered
            .into_iter()
            .map(norito::json::native::Value::from)
            .collect(),
    );
    let json = iroha_primitives::json::Json::from(json_array);
    let custom = CustomParameter::new(id, json);
    let isi = iroha_data_model::isi::SetParameter::new(Parameter::Custom(custom));
    let instruction: iroha_data_model::isi::InstructionBox = isi.into();
    let tx_instructions = vec![tx_instr_from_box(instruction.clone())];
    let signable_transaction_b64 = if let Some(authority) = body.authority.as_deref() {
        let authority_id = parse_canonical_authority_literal(
            state.as_ref(),
            authority,
            &telemetry,
            CONTEXT_GOV_PROTECTED_AUTHORITY,
        )?;
        Some(build_signable_transaction_b64(
            chain_id.as_ref(),
            &authority_id,
            vec![instruction],
        ))
    } else {
        None
    };

    Ok(JsonBody(ProtectedNamespacesApplyResponse {
        ok: true,
        namespace_count,
        submitted: false,
        tx_instructions,
        signable_transaction_b64,
    }))
}

#[derive(Debug, JsonSerialize)]
/// Response for reading protected namespaces parameter
pub struct ProtectedNamespacesGetResponse {
    /// Whether the parameter is set.
    pub found: bool,
    /// List of protected namespaces.
    pub namespaces: Vec<String>,
}

/// GET /v1/gov/protected-namespaces — read current setting from custom parameters.
///
/// # Errors
/// This handler never returns an error; absent parameters yield `found = false`.
pub async fn handle_gov_protected_get(
    state: Arc<iroha_core::state::State>,
) -> Result<JsonBody<ProtectedNamespacesGetResponse>, crate::Error> {
    use std::str::FromStr as _;

    use iroha_data_model::{name::Name, parameter::CustomParameterId};
    let world = state.world_view();
    let params = world.parameters();
    let mut namespaces: Vec<String> = Vec::new();
    let mut found = false;
    if let Ok(name) = Name::from_str("gov_protected_namespaces") {
        let id = CustomParameterId(name);
        if let Some(custom) = params.custom().get(&id) {
            found = true;
            if let Ok(v) = custom.payload().try_into_any_norito::<Vec<String>>() {
                namespaces = v;
            }
        }
    }
    Ok(JsonBody(ProtectedNamespacesGetResponse {
        found,
        namespaces,
    }))
}

#[derive(Debug, JsonSerialize)]
/// Response for reading governance-managed contract binding state by canonical address.
pub struct GovernedContractResponse {
    /// Whether the contract is currently bound in state.
    pub found: bool,
    /// Canonical public contract address queried.
    pub contract_address: iroha_data_model::smart_contract::ContractAddress,
    /// Consensus-persisted non-signing account authority for the active contract.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub contract_subject_account: Option<String>,
    /// Dataspace alias derived from the contract address, when known.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub dataspace: Option<String>,
    /// Active code hash bound to the contract address, when present.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub code_hash_hex: Option<String>,
    /// Authenticated ABI hash embedded in the exact active artifact.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub abi_hash_hex: Option<String>,
    /// Sorted, unique transaction and read-only entrypoints exposed to applications.
    #[norito(skip_serializing_if = "Option::is_none")]
    pub public_entrypoints: Option<Vec<String>>,
}

fn governed_contract_invariant(message: impl Into<String>) -> crate::Error {
    crate::Error::Query(iroha_data_model::ValidationFail::InternalError(
        message.into(),
    ))
}

fn is_canonical_public_entrypoint_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    (1..=128).contains(&bytes.len())
        && bytes[0].is_ascii_lowercase()
        && bytes[1..]
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_')
}

/// GET /v1/gov/contracts/{contract_address} — read the active governance binding for a contract.
///
/// # Errors
/// Returns `crate::Error::Query` when the contract address is malformed or the dataspace alias
/// encoded in the address is unknown to the current node.
pub async fn handle_gov_contract_get(
    state: Arc<iroha_core::state::State>,
    contract_address: axum::extract::Path<String>,
) -> Result<JsonBody<GovernedContractResponse>, crate::Error> {
    let contract_address: iroha_data_model::smart_contract::ContractAddress =
        contract_address.0.parse().map_err(|err| {
            crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                    "invalid contract_address: {err}"
                )),
            ))
        })?;
    let dataspace_id = contract_address.dataspace_id().map_err(|err| {
        crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                "invalid contract_address dataspace: {err}"
            )),
        ))
    })?;
    let dataspace = state
        .nexus_snapshot()
        .dataspace_catalog
        .by_id(dataspace_id)
        .map(|entry| entry.alias.clone())
        .ok_or_else(|| {
            crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::NotFound,
            ))
        })?;
    let view = state.view();
    let active_code_hash = view
        .world()
        .contract_instances()
        .get(&contract_address)
        .copied();
    let Some(active_code_hash) = active_code_hash else {
        return Ok(JsonBody(GovernedContractResponse {
            found: false,
            contract_address,
            contract_subject_account: None,
            dataspace: Some(dataspace),
            code_hash_hex: None,
            abi_hash_hex: None,
            public_entrypoints: None,
        }));
    };

    let record =
        iroha_core::smartcontracts::code::fetch_bound_contract_record(&view, &contract_address)
            .ok_or_else(|| {
                governed_contract_invariant(
                    "active contract has incomplete code, manifest, alias, or subject bindings",
                )
            })?;
    if record.contract_address != contract_address || record.code_hash != active_code_hash {
        return Err(governed_contract_invariant(
            "active contract record disagrees with its world-state binding",
        ));
    }

    let verified = ivm::verify_contract_artifact(&record.code_bytes).map_err(|error| {
        governed_contract_invariant(format!(
            "active contract artifact failed independent verification: {error}"
        ))
    })?;
    let manifest_code_hash = record
        .manifest
        .code_hash
        .ok_or_else(|| governed_contract_invariant("active contract manifest has no code_hash"))?;
    let manifest_abi_hash = record
        .manifest
        .abi_hash
        .ok_or_else(|| governed_contract_invariant("active contract manifest has no abi_hash"))?;
    if verified.code_hash != active_code_hash || manifest_code_hash != active_code_hash {
        return Err(governed_contract_invariant(
            "active contract code hash does not match its stored artifact and manifest",
        ));
    }
    if verified.abi_hash != manifest_abi_hash
        || record.manifest.signature_payload() != verified.manifest.signature_payload()
    {
        return Err(governed_contract_invariant(
            "active contract manifest does not match its authenticated artifact metadata",
        ));
    }
    let provenance = record.manifest.provenance.as_ref().ok_or_else(|| {
        governed_contract_invariant("active contract manifest has no signed provenance")
    })?;
    provenance
        .signature
        .verify(
            &provenance.signer,
            &record.manifest.signature_payload_bytes(),
        )
        .map_err(|_| {
            governed_contract_invariant("active contract manifest provenance is invalid")
        })?;

    let code_hash_bytes: [u8; 32] = active_code_hash.into();
    let abi_hash_bytes: [u8; 32] = manifest_abi_hash.into();
    if code_hash_bytes.iter().all(|byte| *byte == 0) || abi_hash_bytes.iter().all(|byte| *byte == 0)
    {
        return Err(governed_contract_invariant(
            "active contract exposes an invalid all-zero code or ABI hash",
        ));
    }

    let mut public_entrypoints = verified
        .manifest
        .entrypoints
        .as_ref()
        .ok_or_else(|| governed_contract_invariant("active contract has no entrypoint manifest"))?
        .iter()
        .filter(|entrypoint| {
            matches!(
                entrypoint.kind,
                EntryPointKind::Kotoage | EntryPointKind::View
            )
        })
        .map(|entrypoint| entrypoint.name.clone())
        .collect::<Vec<_>>();
    if public_entrypoints.is_empty()
        || public_entrypoints
            .iter()
            .any(|name| !is_canonical_public_entrypoint_name(name))
    {
        return Err(governed_contract_invariant(
            "active contract has no canonical public entrypoints",
        ));
    }
    public_entrypoints.sort();
    if public_entrypoints.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(governed_contract_invariant(
            "active contract advertises duplicate public entrypoints",
        ));
    }

    Ok(JsonBody(GovernedContractResponse {
        found: true,
        contract_address,
        contract_subject_account: Some(record.contract_subject.to_string()),
        dataspace: Some(dataspace),
        code_hash_hex: Some(hex::encode(code_hash_bytes)),
        abi_hash_hex: Some(hex::encode(abi_hash_bytes)),
        public_entrypoints: Some(public_entrypoints),
    }))
}

/// POST /v1/gov/propose-deploy — build a proposal id and instruction skeleton.
///
/// Callers submit the returned instruction in a locally signed transaction.
///
/// # Errors
/// Returns `crate::Error::Query` when the contract target, hashes, ABI version, or request
/// options fail validation (e.g., malformed hex or unsupported voting mode).
pub async fn handle_gov_propose_deploy(
    state: Arc<iroha_core::state::State>,
    NoritoJson(body): NoritoJson<ProposeDeployContractDto>,
) -> Result<JsonBody<ProposeDeployContractResponse>, crate::Error> {
    use iroha_data_model::isi::governance as gov;

    let contract_address = resolve_governance_contract_target(
        &state,
        body.contract_address.as_ref(),
        body.contract_alias.as_ref(),
    )?;

    let (code_hash_hex, code_hash_bytes) = canonical_hex32(&body.code_hash, "code_hash")?;
    let (abi_hash_hex, abi_hash_bytes) = canonical_hex32(&body.abi_hash, "abi_hash")?;

    let abi_version = body.abi_version.trim();
    let expected_abi_hash = match abi_version {
        "1" => ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
        other => {
            return Err(crate::Error::Query(
                iroha_data_model::ValidationFail::QueryFailed(
                    iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                        "unsupported abi_version: {other}"
                    )),
                ),
            ));
        }
    };
    if abi_hash_bytes != expected_abi_hash {
        return Err(crate::Error::Query(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                    "abi_hash does not match canonical hash for abi_version {abi_version}"
                )),
            ),
        ));
    }

    let mode = match body.mode.as_deref() {
        Some(m) if m.eq_ignore_ascii_case("plain") => Some(gov::VotingMode::Plain),
        Some(m) if m.eq_ignore_ascii_case("zk") => Some(gov::VotingMode::Zk),
        Some(other) => {
            return Err(crate::Error::Query(
                iroha_data_model::ValidationFail::QueryFailed(
                    iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                        "unsupported voting mode: {other}"
                    )),
                ),
            ));
        }
        None => None,
    };

    let window = body.window.map(|w| AtWindow {
        lower: w.lower,
        upper: w.upper,
    });

    if let Some(ref win) = window {
        if win.upper < win.lower {
            return Err(crate::Error::Query(
                iroha_data_model::ValidationFail::QueryFailed(
                    iroha_data_model::query::error::QueryExecutionFail::Conversion(
                        "window.upper must be >= window.lower".into(),
                    ),
                ),
            ));
        }
    }

    let instr = gov::ProposeDeployContract {
        contract_address: contract_address.clone(),
        code_hash_hex,
        abi_hash_hex,
        abi_version: abi_version.to_string(),
        window,
        mode,
        manifest_provenance: body.manifest_provenance.clone(),
    };

    let proposal_id_bytes =
        compute_proposal_id(&instr.contract_address, &code_hash_bytes, &abi_hash_bytes);
    let proposal_id = hex::encode(proposal_id_bytes);
    Ok(JsonBody(ProposeDeployContractResponse {
        ok: true,
        proposal_id,
        tx_instructions: instruction_skeleton_for_propose(&instr),
    }))
}

/// POST /v1/gov/proposals/sccp-route-governance — build a proposal id and instruction skeleton.
///
/// The request schema excludes private signing material; callers submit locally
/// signed transactions after building the draft instructions.
///
/// # Errors
/// Returns `crate::Error::Query` when the request options fail validation.
pub async fn handle_gov_propose_sccp_route_governance(
    NoritoJson(body): NoritoJson<ProposeSccpRouteGovernanceDto>,
) -> Result<JsonBody<ProposeSccpRouteGovernanceResponse>, crate::Error> {
    use iroha_data_model::isi::governance as gov;

    let mode = body.mode;
    let window = body.window.map(|w| AtWindow {
        lower: w.lower,
        upper: w.upper,
    });

    if let Some(ref win) = window {
        if win.upper < win.lower {
            return Err(crate::Error::Query(
                iroha_data_model::ValidationFail::QueryFailed(
                    iroha_data_model::query::error::QueryExecutionFail::Conversion(
                        "window.upper must be >= window.lower".into(),
                    ),
                ),
            ));
        }
    }

    body.action.validate_static().map_err(|error| {
        crate::routing::conversion_error(format!("invalid SCCP route governance action: {error}"))
    })?;

    let instr = gov::ProposeSccpRouteGovernance {
        action: body.action,
        window,
        mode,
    };

    let proposal_id = hex::encode(compute_sccp_route_governance_proposal_id(&instr.action)?);

    Ok(JsonBody(ProposeSccpRouteGovernanceResponse {
        ok: true,
        proposal_id,
        tx_instructions: instruction_skeleton_for_sccp_route_governance_propose(&instr),
    }))
}

/// POST /v1/ministry/agenda/proposals/draft — build a detached-signature-ready Ministry submission transaction.
///
/// Returns a duplicate summary with HTTP 409 semantics when the proposal id already exists in committed
/// state; callers must submit the resulting signed transaction through the normal Torii `/v1/pipeline/transactions`
/// route.
pub async fn handle_ministry_agenda_proposal_draft(
    chain_id: Arc<iroha_data_model::ChainId>,
    state: Arc<iroha_core::state::State>,
    telemetry: MaybeTelemetry,
    NoritoJson(body): NoritoJson<MinistryAgendaProposalDraftDto>,
) -> Result<MinistryAgendaProposalDraftOutcome, crate::Error> {
    body.proposal.validate().map_err(|err| {
        crate::routing::conversion_error(format!("invalid agenda proposal: {err}"))
    })?;
    let authority_id = parse_canonical_authority_literal(
        state.as_ref(),
        body.authority.as_str(),
        &telemetry,
        CONTEXT_MINISTRY_AGENDA_DRAFT_AUTHORITY,
    )?;
    if let Some(existing) = state
        .world_view()
        .ministry_agenda_proposals()
        .get(&body.proposal.proposal_id)
        .cloned()
    {
        return Ok(MinistryAgendaProposalDraftOutcome::Duplicate(
            MinistryAgendaProposalGetResponse {
                found: true,
                record: Some(existing),
            },
        ));
    }

    let instr = iroha_data_model::isi::ministry::SubmitAgendaProposal {
        proposal: body.proposal.clone(),
    };
    let tx_instructions = vec![tx_instr_from_box(instr.clone().into())];
    let signable_transaction_b64 = build_signable_transaction_b64(
        chain_id.as_ref(),
        &authority_id,
        vec![iroha_data_model::isi::InstructionBox::from(instr)],
    );

    Ok(MinistryAgendaProposalDraftOutcome::Draft(
        MinistryAgendaProposalDraftResponse {
            ok: true,
            agenda_proposal_id: body.proposal.proposal_id,
            authority: authority_id.to_string(),
            tx_instructions,
            signable_transaction_b64,
        },
    ))
}

/// GET /v1/ministry/agenda/proposals/{proposal_id} — fetch a submitted Ministry agenda proposal record.
pub async fn handle_ministry_agenda_proposal_get(
    state: Arc<iroha_core::state::State>,
    proposal_id: axum::extract::Path<String>,
) -> Result<JsonBody<MinistryAgendaProposalGetResponse>, crate::Error> {
    let proposal_id = proposal_id.0.trim().to_string();
    if proposal_id.is_empty() {
        return Err(crate::routing::conversion_error(
            "proposal_id must not be empty".into(),
        ));
    }
    let record = state
        .world_view()
        .ministry_agenda_proposals()
        .get(&proposal_id)
        .cloned();
    Ok(JsonBody(MinistryAgendaProposalGetResponse {
        found: record.is_some(),
        record,
    }))
}

/// POST /v1/gov/ballot/zk — accept a ZK ballot and build an instruction skeleton.
///
/// The request schema excludes private signing material; callers submit locally signed transactions.
///
/// # Errors
/// Returns `crate::Error::Query` for invalid chain id or authority. Invalid proofs result in an
/// `ok = false` response.
pub async fn handle_gov_ballot_zk(
    chain_id: Arc<iroha_data_model::ChainId>,
    state: Arc<iroha_core::state::State>,
    telemetry: MaybeTelemetry,
    NoritoJson(body): NoritoJson<ZkBallotDto>,
) -> Result<JsonBody<BallotSubmitResponse>, crate::Error> {
    // Minimal size check for b64
    if base64::engine::general_purpose::STANDARD
        .decode(body.proof_b64.as_bytes())
        .map(|bytes| bytes.len())
        .unwrap_or(0)
        == 0
    {
        return Ok(JsonBody(BallotSubmitResponse {
            ok: false,
            accepted: false,
            reason: Some("invalid proof".to_string()),
            tx_instructions: Vec::new(),
        }));
    }
    let ZkBallotDto {
        authority,
        chain_id: body_chain_id,
        election_id,
        proof_b64,
        public,
    } = body;
    ensure_chain_id_matches(chain_id.as_ref(), &body_chain_id)?;
    let _authority_id = parse_authority_literal(
        state.as_ref(),
        authority.as_str(),
        &telemetry,
        CONTEXT_GOV_BALLOT_ZK_AUTHORITY,
    )?;
    let public_inputs = match public {
        None => norito::json::Value::Object(norito::json::Map::new()),
        Some(norito::json::Value::Object(mut map)) => {
            if let Err(reason) = normalize_zk_ballot_public_inputs(&mut map) {
                return Ok(ballot_rejection(&reason));
            }
            let has_owner = hint_present(&map, "owner");
            let has_amount = hint_present(&map, "amount");
            let has_duration = hint_present(&map, "duration_blocks");
            if lock_hints_incomplete(has_owner, has_amount, has_duration) {
                return Ok(ballot_rejection(
                    "lock hints must include owner, amount, duration_blocks",
                ));
            }
            if let Err(reason) = reject_zk_public_input_owner(&map) {
                return Ok(ballot_rejection(&reason));
            }
            norito::json::Value::Object(map)
        }
        Some(_) => {
            return Ok(ballot_rejection("public inputs must be a JSON object"));
        }
    };
    // Build instruction skeleton
    let instr = iroha_data_model::isi::governance::CastZkBallot {
        election_id,
        proof_b64,
        public_inputs_json: norito::json::to_json(&public_inputs).unwrap_or_else(|_| "{}".into()),
    };
    let tx_instructions = vec![tx_instr_from_box(instr.into())];
    Ok(JsonBody(BallotSubmitResponse {
        ok: true,
        accepted: true,
        reason: Some("build transaction skeleton".to_string()),
        tx_instructions,
    }))
}

/// POST /v1/gov/ballot/plain — accept a plain quadratic ballot and build an instruction skeleton.
///
/// The request schema excludes private signing material; callers submit locally signed transactions.
///
/// # Errors
/// Returns `crate::Error::Query` when the ballot fields fail validation (direction, authority,
/// owner, amount parsing, or chain id mismatch).
pub async fn handle_gov_ballot_plain(
    chain_id: Arc<iroha_data_model::ChainId>,
    state: Arc<iroha_core::state::State>,
    NoritoJson(body): NoritoJson<PlainBallotDto>,
) -> Result<JsonBody<BallotSubmitResponse>, crate::Error> {
    handle_gov_ballot_plain_with_policy(
        chain_id,
        state,
        NoritoJson(body),
        MaybeTelemetry::disabled(),
    )
    .await
}

/// Variant of [`handle_gov_ballot_plain`] that allows callers to inject telemetry
/// policy, enabling address parsing coverage across Torii and tests.
pub async fn handle_gov_ballot_plain_with_policy(
    chain_id: Arc<iroha_data_model::ChainId>,
    state: Arc<iroha_core::state::State>,
    NoritoJson(body): NoritoJson<PlainBallotDto>,
    telemetry: MaybeTelemetry,
) -> Result<JsonBody<BallotSubmitResponse>, crate::Error> {
    ensure_chain_id_matches(chain_id.as_ref(), &body.chain_id)?;
    // Basic shape validations
    if !(body.direction == "Aye" || body.direction == "Nay" || body.direction == "Abstain") {
        return Err(crate::routing::conversion_error("invalid direction".into()));
    }
    // Parse authority and owner; require equality for plain ballots
    let authority_id = parse_account_literal_from_state(
        state.as_ref(),
        body.authority.as_str(),
        &telemetry,
        CONTEXT_GOV_BALLOT_PLAIN_AUTHORITY,
    )
    .map_err(|err| {
        crate::routing::conversion_error(format!("invalid authority: {}", err.reason()))
    })?;
    let owner = parse_account_literal_from_state(
        state.as_ref(),
        body.owner.as_str(),
        &telemetry,
        CONTEXT_GOV_BALLOT_PLAIN_OWNER,
    )
    .map_err(|err| crate::routing::conversion_error(format!("invalid owner: {}", err.reason())))?;
    if owner != authority_id {
        return Err(crate::routing::conversion_error(
            "authority must equal owner".into(),
        ));
    }
    let duration_blocks = parse_canonical_u64_decimal("duration_blocks", &body.duration_blocks)
        .map_err(crate::routing::conversion_error)?;
    let instr = iroha_data_model::isi::governance::CastPlainBallot {
        referendum_id: body.referendum_id,
        owner,
        amount: body.amount,
        duration_blocks,
        direction: match body.direction.as_str() {
            "Aye" => 0,
            "Nay" => 1,
            _ => 2,
        },
    };
    let tx_instructions = vec![tx_instr_from_box(instr.into())];
    Ok(JsonBody(BallotSubmitResponse {
        ok: true,
        accepted: true,
        reason: Some("build transaction skeleton".to_string()),
        tx_instructions,
    }))
}

/// POST /v1/gov/parliament/ballots — draft an equal Parliament stage ballot.
///
/// # Errors
/// Returns `crate::Error::Query` when the authority, chain id, or proposal id is invalid.
pub async fn handle_gov_parliament_ballot(
    chain_id: Arc<iroha_data_model::ChainId>,
    state: Arc<iroha_core::state::State>,
    telemetry: MaybeTelemetry,
    NoritoJson(body): NoritoJson<ParliamentBallotDto>,
) -> Result<JsonBody<BallotSubmitResponse>, crate::Error> {
    ensure_chain_id_matches(chain_id.as_ref(), &body.chain_id)?;
    let _authority_id = parse_account_literal_from_state(
        state.as_ref(),
        body.authority.as_str(),
        &telemetry,
        CONTEXT_GOV_PARLIAMENT_BALLOT_AUTHORITY,
    )
    .map_err(|err| {
        crate::routing::conversion_error(format!("invalid authority: {}", err.reason()))
    })?;
    let (_proposal_hex, proposal_id) = canonical_hex32(&body.proposal_id, "proposal_id")?;
    let instr = iroha_data_model::isi::governance::CastParliamentBallot {
        body: body.body,
        proposal_id,
        decision: body.decision,
    };
    let tx_instructions = vec![tx_instr_from_box(instr.into())];
    Ok(JsonBody(BallotSubmitResponse {
        ok: true,
        accepted: true,
        reason: Some("build transaction skeleton".to_string()),
        tx_instructions,
    }))
}

/// GET /v1/gov/council/current — fetch the latest persisted council membership.
///
/// # Errors
/// This handler never returns an error; empty councils are represented with an empty member list.
pub async fn handle_gov_council_current(
    state: Arc<iroha_core::state::State>,
) -> Result<JsonBody<CouncilCurrentResponse>, crate::Error> {
    let world = state.world_view();
    let mut last_epoch: Option<u64> = None;
    for (ep, _) in world.council().iter() {
        last_epoch = Some(last_epoch.map(|e| e.max(*ep)).unwrap_or(*ep));
    }
    if let Some(epoch) = last_epoch {
        if let Some(cs) = world.council().get(&epoch) {
            return Ok(JsonBody(CouncilCurrentResponse {
                epoch,
                members: cs
                    .members
                    .iter()
                    .map(|a| CouncilMemberDto {
                        account_id: a.to_string(),
                    })
                    .collect(),
                alternates: cs
                    .alternates
                    .iter()
                    .map(|a| CouncilMemberDto {
                        account_id: a.to_string(),
                    })
                    .collect(),
                candidate_count: cs.candidate_count as usize,
                derived_by: cs.derived_by,
            }));
        }
    }
    let height = state.committed_height() as u64;
    let term_blocks = state.gov.parliament_term_blocks.max(1);
    let epoch = height / term_blocks;
    Ok(JsonBody(CouncilCurrentResponse {
        epoch,
        members: Vec::new(),
        alternates: Vec::new(),
        candidate_count: 0,
        derived_by: CouncilDerivationKind::Manual,
    }))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::body::Bytes;
    use iroha_config::parameters::actual::LaneConfig;
    use iroha_core::{
        block::BlockBuilder,
        kura::Kura,
        query::store::LiveQueryStore,
        queue::{Queue, TransactionGuard},
        smartcontracts::code::{activate_instance, register_code_bytes, register_manifest},
        state::{
            GovernanceLockRecord, GovernanceLocksForReferendum, GovernancePipeline,
            GovernanceProposalRecord, GovernanceProposalStatus, GovernanceReferendumMode,
            GovernanceReferendumRecord, GovernanceReferendumStatus, GovernanceStageApprovals,
            State, World,
        },
    };
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{
        ChainId, Registrable,
        account::{Account, AccountId},
        asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId},
        block::BlockHeader,
        domain::{Domain, DomainId},
        isi::{InstructionBox, governance::RegisterCitizen},
        name::Name,
        permission::Permission,
        smart_contract::manifest::ContractManifest,
    };
    use iroha_primitives::numeric::Quantity;
    use iroha_test_samples::ALICE_ID;
    use nonzero_ext::nonzero;

    use super::*;
    use crate::routing::MaybeTelemetry;

    const ACCOUNT_AUTHORITY: &str = "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE";
    const ACCOUNT_OWNER_ALT: &str = "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D";

    #[test]
    fn hint_present_handles_nulls() {
        let mut map = json::Map::new();
        assert!(!hint_present(&map, "owner"));
        map.insert("owner".to_string(), json::Value::Null);
        assert!(!hint_present(&map, "owner"));
        map.insert(
            "owner".to_string(),
            json::Value::String("alice".to_string()),
        );
        assert!(hint_present(&map, "owner"));
    }

    fn conversion_message(err: crate::Error) -> String {
        match err {
            crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(message),
            )) => message,
            other => panic!("expected conversion query error, got {other:?}"),
        }
    }

    fn canonical_literal(raw: &str) -> String {
        iroha_data_model::account::AccountId::parse_encoded(raw)
            .expect("literal parses")
            .canonical()
            .to_string()
    }

    fn noncanonical_literal(raw: &str) -> String {
        AccountId::parse_encoded(raw)
            .expect("literal parses")
            .canonical()
            .replacen("sora", "ｓｏｒａ", 1)
    }

    fn mk_basic_context() -> (Arc<State>, Arc<Queue>, Arc<ChainId>) {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = Arc::new(State::new_for_testing(World::default(), kura, query));
        let events = tokio::sync::broadcast::channel(1).0;
        let queue = Arc::new(Queue::from_config(
            iroha_config::parameters::actual::Queue::default(),
            events,
        ));
        let chain_id: ChainId = "chain".parse().expect("chain id");
        (state, queue, Arc::new(chain_id))
    }

    fn bind_account_alias_for_test(state: &Arc<State>, account_id: &AccountId, alias: &str) {
        let label = iroha_data_model::account::rekey::AccountAlias::from_literal(
            alias,
            &state.nexus_snapshot().dataspace_catalog,
        )
        .expect("valid account alias");
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut tx = block.transaction();
        let world = tx.world_mut_for_testing();
        world
            .account_aliases_mut_for_testing()
            .insert(label.clone(), account_id.clone());
        let mut labels = world
            .account_aliases_by_account_mut_for_testing()
            .get(account_id)
            .cloned()
            .unwrap_or_default();
        labels.insert(label.clone());
        world
            .account_aliases_by_account_mut_for_testing()
            .insert(account_id.clone(), labels);
        world.account_rekey_records_mut_for_testing().insert(
            label.clone(),
            iroha_data_model::account::rekey::AccountRekeyRecord::new(label, account_id.clone()),
        );
        tx.apply();
        block.commit().expect("commit account alias for test");
    }

    struct GovHarness {
        state: Arc<State>,
        queue: Arc<Queue>,
        chain_id: Arc<ChainId>,
        authority: AccountId,
        authority_keypair: KeyPair,
        asset_def_id: AssetDefinitionId,
        escrow: AccountId,
    }

    fn checked_governance_keypair(seed: u8, algorithm: Algorithm) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], algorithm)
            .expect("test governance fixture key derivation should succeed")
    }

    fn checked_governance_ed25519_keypair(seed: u8) -> KeyPair {
        checked_governance_keypair(seed, Algorithm::Ed25519)
    }

    fn checked_governance_bls_keypair(seed: u8) -> KeyPair {
        checked_governance_keypair(seed, Algorithm::BlsNormal)
    }

    #[test]
    fn checked_governance_keypairs_use_fallible_seed_derivation() {
        let ed25519 = checked_governance_ed25519_keypair(0x90);
        let bls = checked_governance_bls_keypair(0x91);
        let bls_repeat = checked_governance_bls_keypair(0x91);
        let bls_other = checked_governance_bls_keypair(0x92);

        assert_eq!(ed25519.algorithm(), Algorithm::Ed25519);
        assert_eq!(bls.algorithm(), Algorithm::BlsNormal);
        assert_eq!(bls.public_key(), bls_repeat.public_key());
        assert_ne!(bls.public_key(), bls_other.public_key());
        assert!(
            KeyPair::try_from_seed(vec![0; 32], Algorithm::Ed25519).is_err(),
            "checked Ed25519 seed derivation must reject weak all-zero fixture seeds"
        );
    }

    fn mk_governance_harness(with_permissions: bool) -> GovHarness {
        let authority_keypair = checked_governance_ed25519_keypair(0x93);
        let domain_id: DomainId = DomainId::try_new("wonderland", "universal").expect("domain id");
        let authority = AccountId::of(authority_keypair.public_key().clone());
        let escrow: AccountId =
            iroha_config::parameters::defaults::governance::bond_escrow_account_id();
        let domain = Domain::new(domain_id.clone()).build(&authority);
        let authority_account = Account::new(authority.clone()).build(&authority);
        let escrow_account = Account::new(escrow.clone()).build(&escrow);
        let asset_def_id: AssetDefinitionId = AssetDefinitionId::new(
            domain_id.clone(),
            Name::from_str("vote").expect("asset definition name"),
        );
        let asset_def = {
            let __asset_definition_id = asset_def_id.clone();
            AssetDefinition::numeric(__asset_definition_id.clone())
                .with_name(__asset_definition_id.name().to_string())
        }
        .build(&authority);
        let asset = Asset::new(
            AssetId::new(asset_def_id.clone(), authority.clone()),
            Quantity::from(1_000u32),
        );
        let escrow_asset = Asset::new(
            AssetId::new(asset_def_id.clone(), escrow.clone()),
            Quantity::from(0u32),
        );
        let world = World::with_assets(
            [domain],
            [authority_account, escrow_account],
            [asset_def],
            [asset, escrow_asset],
            [],
        );
        if with_permissions {
            let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
                0,
                &authority,
                0,
                iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
            )
            .expect("contract address");
            let contract_address_literal = contract_address.to_string();
            let propose = Permission::new(
                "CanProposeContractDeployment".to_string(),
                norito::json!({ "contract_address": contract_address_literal }),
            );
            let ballot = Permission::new(
                "CanSubmitGovernanceBallot".to_string(),
                norito::json!({ "referendum_id": "any" }),
            );
            let enact = Permission::new("CanEnactGovernance".to_string(), norito::json!({}));
            let register_contract: Permission =
                iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode
                    .into();
            let mut world_block = world.block();
            let mut world_tx = world_block.transaction_without_telemetry(LaneConfig::default(), 0);
            let _ = world_tx.add_account_permission(&authority, propose);
            let _ = world_tx.add_account_permission(&authority, ballot);
            let _ = world_tx.add_account_permission(&authority, enact);
            let _ = world_tx.add_account_permission(&authority, register_contract);
            world_tx.apply();
            world_block.commit();
        }

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let chain_id: ChainId = "chain".parse().expect("chain id");
        let mut state = State::new_with_chain_for_testing(world, kura, query, chain_id.clone());
        let mut gov_cfg = state.gov.clone();
        gov_cfg.voting_asset_id = asset_def_id.clone();
        gov_cfg.citizenship_asset_id = asset_def_id.clone();
        gov_cfg.bond_escrow_account = escrow.clone();
        gov_cfg.citizenship_escrow_account = escrow.clone();
        gov_cfg.slash_receiver_account = escrow.clone();
        gov_cfg.min_bond_amount = 0_u64.into();
        gov_cfg.citizenship_bond_amount = 0_u64.into();
        gov_cfg.plain_voting_enabled = true;
        gov_cfg.conviction_step_blocks = 1;
        gov_cfg.max_conviction = 1;
        gov_cfg.window_span = 10;
        gov_cfg.min_enactment_delay = 0;
        gov_cfg.approval_threshold_q_num = 1;
        gov_cfg.approval_threshold_q_den = 1;
        gov_cfg.min_turnout = 1;
        state.set_gov(gov_cfg);

        let nexus = state.nexus_snapshot();
        let lane_manifests = Arc::new(
            iroha_core::governance::manifest::LaneManifestRegistry::from_config(
                &nexus.lane_catalog,
                &iroha_config::parameters::actual::GovernanceCatalog::default(),
                &iroha_config::parameters::actual::LaneRegistry::default(),
            ),
        );
        state.install_lane_manifests(&lane_manifests);

        let events = tokio::sync::broadcast::channel(1).0;
        let queue = Arc::new(Queue::from_config(
            iroha_config::parameters::actual::Queue::default(),
            events,
        ));
        GovHarness {
            state: Arc::new(state),
            queue,
            chain_id: Arc::new(chain_id),
            authority,
            authority_keypair,
            asset_def_id,
            escrow,
        }
    }

    fn mk_manifest_provenance(
        keypair: &KeyPair,
        code_hash: [u8; 32],
        abi_hash: [u8; 32],
    ) -> ManifestProvenance {
        let manifest = ContractManifest {
            seiyaku_name: None,
            code_hash: Some(iroha_crypto::Hash::prehashed(code_hash)),
            abi_hash: Some(iroha_crypto::Hash::prehashed(abi_hash)),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            states: None,
            kotoba: None,
            error_codes: None,
            provenance: None,
        }
        .signed(keypair);
        manifest
            .provenance
            .expect("signed manifest should carry provenance")
    }

    fn sample_contract_address() -> iroha_data_model::smart_contract::ContractAddress {
        "tairac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9ggff82m7"
            .parse()
            .expect("contract address")
    }

    fn install_governed_contract_for_test(
        harness: &GovHarness,
    ) -> (
        iroha_data_model::smart_contract::ContractAddress,
        iroha_crypto::Hash,
    ) {
        let (artifact, manifest) = ivm::KotodamaCompiler::new()
            .compile_source_with_manifest(
                r#"
seiyaku GovernedReadFixture {
    view fn balance() -> bool { return true; }
    kotoage fn transfer() authorize("CanTransferGovernedFixture") {}
}
"#,
            )
            .expect("compile governed contract fixture");
        let verified =
            ivm::verify_contract_artifact(&artifact).expect("verify governed contract fixture");
        assert_eq!(
            manifest.signature_payload(),
            verified.manifest.signature_payload()
        );
        let signed_manifest = manifest.signed(&harness.authority_keypair);
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &harness.authority,
            91,
            iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
        )
        .expect("governed contract address");
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = harness.state.block(header);
        let mut transaction = block.transaction();
        let code_hash = register_code_bytes(&harness.authority, artifact, &mut transaction)
            .expect("register governed contract bytes");
        assert_eq!(code_hash, verified.code_hash);
        register_manifest(&harness.authority, signed_manifest, &mut transaction)
            .expect("register governed contract manifest");
        activate_instance(
            &harness.authority,
            contract_address.clone(),
            code_hash,
            &mut transaction,
        )
        .expect("activate governed contract");
        transaction.apply();
        block.commit().expect("commit governed contract fixture");
        (contract_address, code_hash)
    }

    fn sample_sccp_route_governance_action()
    -> iroha_data_model::isi::bridge::SccpRouteGovernanceActionV1 {
        iroha_data_model::isi::bridge::SccpRouteGovernanceActionV1::Remove(
            iroha_data_model::bridge::SccpRouteKeyV1 {
                lane_id: iroha_data_model::bridge::SccpLaneIdV1 {
                    source: iroha_data_model::bridge::SccpNetworkV1::EthereumMainnet,
                    target: iroha_data_model::bridge::SccpNetworkV1::SoraTaira,
                },
                route_id: iroha_sccp::SCCP_TAIRA_ETH_XOR_ROUTE_ID_V1.to_owned(),
                asset_key: iroha_sccp::SCCP_TAIRA_XOR_ASSET_KEY_V1.to_owned(),
                revision: 1,
            },
        )
    }

    fn sample_agenda_proposal(proposal_id: &str) -> AgendaProposalV1 {
        AgendaProposalV1 {
            version: iroha_data_model::ministry::AGENDA_PROPOSAL_VERSION_V1,
            proposal_id: proposal_id.to_string(),
            submitted_at_unix_ms: 1_775_000_000_000,
            language: "en".to_string(),
            action: iroha_data_model::ministry::AgendaProposalAction::AddToDenylist,
            summary: iroha_data_model::ministry::AgendaProposalSummary {
                title: "Blacklist SoraFS CID bafy-test".to_string(),
                motivation: "Evidence review recommends blocking the published SoraFS root CID."
                    .to_string(),
                expected_impact:
                    "Participating gateways would deny delivery while the evidence is reviewed."
                        .to_string(),
            },
            tags: vec!["fraud".to_string()],
            targets: vec![iroha_data_model::ministry::AgendaProposalTarget {
                label: "bafy-test".to_string(),
                hash_family: "sorafs-root-cid".to_string(),
                hash_hex: "11".repeat(32),
                reason: "Fraud review evidence for the selected SoraFS CID.".to_string(),
            }],
            evidence: vec![iroha_data_model::ministry::AgendaEvidenceAttachment {
                kind: iroha_data_model::ministry::AgendaEvidenceKind::Url,
                uri: "https://example.org/evidence/case-42".to_string(),
                digest_blake3_hex: None,
                description: Some("Public incident report".to_string()),
            }],
            submitter: iroha_data_model::ministry::AgendaProposalSubmitter {
                name: "Review Council".to_string(),
                contact: "review@example.org".to_string(),
                organization: Some("SoraFS Moderation".to_string()),
                pgp_fingerprint: None,
            },
            duplicates: Vec::new(),
        }
    }

    fn decode_tx_instruction(instr: &TxInstr) -> iroha_data_model::isi::InstructionBox {
        let bytes = hex::decode(&instr.payload_hex).expect("instruction payload hex");
        iroha_data_model::isi::decode_instruction_from_pair(&instr.wire_id, &bytes)
            .expect("instruction payload decode")
    }

    fn queue_instruction_skeleton(harness: &GovHarness, tx_instructions: &[TxInstr]) {
        let instructions = tx_instructions
            .iter()
            .map(decode_tx_instruction)
            .collect::<Vec<_>>();
        let tx = iroha_data_model::transaction::signed::TransactionBuilder::new(
            (*harness.chain_id).clone(),
            harness.authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(instructions)
        .sign(harness.authority_keypair.private_key());
        let params = harness.state.view().world().parameters().clone();
        let accepted = iroha_core::tx::AcceptedTransaction::accept(
            tx,
            harness.chain_id.as_ref(),
            params.sumeragi().max_clock_drift(),
            params.transaction(),
            harness.state.crypto().as_ref(),
        )
        .expect("accepted governance instruction skeleton");
        harness
            .queue
            .push(accepted, harness.state.view())
            .expect("push governance instruction skeleton");
    }

    fn apply_queued_block_allow_errors(
        state: &Arc<State>,
        queue: &Arc<Queue>,
        expected_height: u64,
    ) -> Vec<bool> {
        let max_txs_in_block = core::num::NonZeroUsize::new(1024).expect("nonzero");
        let mut guards = Vec::new();
        queue.get_transactions_for_block(&state.view(), max_txs_in_block, &mut guards);
        if guards.is_empty() {
            return Vec::new();
        }
        let accepted: Vec<_> = guards
            .iter()
            .map(TransactionGuard::clone_accepted)
            .collect();
        let latest_block = state.view().latest_block();
        let leader = checked_governance_bls_keypair(0x94);
        let new_block = BlockBuilder::new(accepted)
            .chain(0, latest_block.as_deref())
            .sign(leader.private_key())
            .unpack(|_| {});
        assert_eq!(
            new_block.header().height().get(),
            expected_height,
            "unexpected block height"
        );
        let mut state_block = state.block(new_block.header());
        let valid_block = new_block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        let committed_block = valid_block.commit_unchecked().unpack(|_| {});
        let block_ref = committed_block.as_ref();
        let errors = block_ref
            .external_transactions()
            .enumerate()
            .map(|(idx, _)| {
                let error = block_ref.error(idx);
                if let Some(error) = error {
                    eprintln!("governance fixture transaction {idx} failed: {error:?}");
                }
                error.is_some()
            })
            .collect::<Vec<_>>();
        crate::test_utils::finalize_committed_block(state, state_block, committed_block);
        errors
    }

    #[tokio::test]
    async fn citizen_status_reports_registered_record() {
        let harness = mk_governance_harness(false);
        let instruction = InstructionBox::from(RegisterCitizen {
            owner: harness.authority.clone(),
            amount: Quantity::zero(),
        });
        let tx = iroha_data_model::transaction::signed::TransactionBuilder::new(
            (*harness.chain_id).clone(),
            harness.authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction])
        .sign(harness.authority_keypair.private_key());
        let params = harness.state.view().world().parameters().clone();
        let accepted = iroha_core::tx::AcceptedTransaction::accept(
            tx,
            harness.chain_id.as_ref(),
            params.sumeragi().max_clock_drift(),
            params.transaction(),
            harness.state.crypto().as_ref(),
        )
        .expect("accepted register citizen transaction");
        harness
            .queue
            .push(accepted, harness.state.view())
            .expect("push register citizen transaction");
        assert_eq!(
            apply_queued_block_allow_errors(&harness.state, &harness.queue, 1),
            vec![false]
        );

        let response = handle_gov_citizen_status(
            harness.state.clone(),
            axum::extract::Path(harness.authority.to_string()),
            MaybeTelemetry::disabled(),
        )
        .await
        .expect("citizen status response")
        .0;

        assert!(response.is_citizen);
        assert_eq!(response.account_id, harness.authority.to_string());
        assert_eq!(response.amount.as_deref(), Some("0"));
        assert_eq!(response.bonded_height.as_deref(), Some("1"));
    }

    #[tokio::test]
    async fn citizen_count_reports_exact_registry_total() {
        let harness = mk_governance_harness(false);
        assert_eq!(
            handle_gov_citizen_count(harness.state.clone())
                .await
                .expect("empty citizen count")
                .0
                .total,
            "0"
        );

        let instruction = InstructionBox::from(RegisterCitizen {
            owner: harness.authority.clone(),
            amount: Quantity::zero(),
        });
        let tx = iroha_data_model::transaction::signed::TransactionBuilder::new(
            (*harness.chain_id).clone(),
            harness.authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction])
        .sign(harness.authority_keypair.private_key());
        let params = harness.state.view().world().parameters().clone();
        let accepted = iroha_core::tx::AcceptedTransaction::accept(
            tx,
            harness.chain_id.as_ref(),
            params.sumeragi().max_clock_drift(),
            params.transaction(),
            harness.state.crypto().as_ref(),
        )
        .expect("accepted register citizen transaction");
        harness
            .queue
            .push(accepted, harness.state.view())
            .expect("push register citizen transaction");
        assert_eq!(
            apply_queued_block_allow_errors(&harness.state, &harness.queue, 1),
            vec![false]
        );

        let response = handle_gov_citizen_count(harness.state.clone())
            .await
            .expect("citizen count response")
            .0;

        assert_eq!(response.total, "1");
    }

    #[test]
    fn serde_shapes_compile() {
        let canonical_abi = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        let req = ProposeDeployContractDto {
            contract_address: Some(sample_contract_address()),
            contract_alias: None,
            abi_version: "1".to_string(),
            code_hash: "0x".to_string() + &"aa".repeat(32),
            abi_hash: format!("0x{}", hex::encode(canonical_abi)),
            window: Some(AtWindowDto { lower: 1, upper: 2 }),
            mode: None,
            limits: Some(crate::json_object(vec![("max_pages", 64u64)])),
            manifest_provenance: None,
        };
        let s = norito::json::to_json(&req).unwrap();
        let _: ProposeDeployContractDto = norito::json::from_str(&s).unwrap();

        let sccp = ProposeSccpRouteGovernanceDto {
            action: sample_sccp_route_governance_action(),
            window: Some(AtWindowDto { lower: 3, upper: 4 }),
            mode: Some(iroha_data_model::isi::governance::VotingMode::Plain),
        };
        let json = norito::json::to_json(&sccp).expect("encode SCCP governance DTO");
        let decoded: ProposeSccpRouteGovernanceDto =
            norito::json::from_str(&json).expect("decode SCCP governance DTO");
        assert_eq!(decoded.action, sccp.action);
    }

    #[tokio::test]
    async fn protected_namespaces_set_drafts_transaction_without_mutating_state() {
        let (state, _queue, chain_id) = mk_basic_context();

        let before = handle_gov_protected_get(state.clone())
            .await
            .expect("protected namespaces get")
            .0;
        assert!(!before.found);
        assert!(before.namespaces.is_empty());

        let response = handle_gov_protected_set(
            chain_id,
            state.clone(),
            MaybeTelemetry::disabled(),
            NoritoJson(ProtectedNamespacesDto {
                namespaces: vec!["apps".to_owned(), " system ".to_owned(), String::new()],
                authority: None,
            }),
        )
        .await
        .expect("protected namespaces draft")
        .0;

        assert!(response.ok);
        assert!(!response.submitted);
        assert_eq!(response.namespace_count, 2);
        assert_eq!(response.tx_instructions.len(), 1);
        assert!(response.signable_transaction_b64.is_none());

        let after = handle_gov_protected_get(state)
            .await
            .expect("protected namespaces get")
            .0;
        assert!(!after.found);
        assert!(after.namespaces.is_empty());
    }

    #[tokio::test]
    async fn protected_namespaces_set_returns_checked_signable_payload_for_authority() {
        let harness = mk_governance_harness(false);
        let response = handle_gov_protected_set(
            harness.chain_id.clone(),
            harness.state.clone(),
            MaybeTelemetry::disabled(),
            NoritoJson(ProtectedNamespacesDto {
                namespaces: vec!["apps".to_owned(), "system".to_owned()],
                authority: Some(harness.authority.to_string()),
            }),
        )
        .await
        .expect("protected namespaces signable draft")
        .0;

        assert!(response.ok);
        assert!(!response.submitted);
        assert_eq!(response.namespace_count, 2);
        assert_eq!(response.tx_instructions.len(), 1);
        let signable_payload = response
            .signable_transaction_b64
            .expect("authority should produce a signable transaction payload");
        let tx_bytes = base64::engine::general_purpose::STANDARD
            .decode(signable_payload.as_bytes())
            .expect("decode signable payload");
        let payload: iroha_data_model::transaction::signed::TransactionPayload = {
            let _guard = norito::core::PayloadCtxGuard::enter(&tx_bytes);
            let mut cursor = std::io::Cursor::new(tx_bytes.as_slice());
            norito::codec::Decode::decode(&mut cursor).expect("decode transaction payload")
        };
        assert_eq!(payload.authority, harness.authority);
        assert_eq!(payload.instructions.instruction_count(), 1);
    }

    #[tokio::test]
    async fn finalize_builds_instruction_skeleton() {
        let (_state, _queue, _chain_id) = mk_basic_context();
        let dto = FinalizeDto {
            referendum_id: "ref-xyz".to_string(),
            proposal_id: format!("0x{}", "aa".repeat(32)),
        };
        let res = handle_gov_finalize(NoritoJson(dto))
            .await
            .expect("handler ok");
        let body = res.0;
        assert!(body.ok);
        assert_eq!(body.tx_instructions.len(), 1);
        assert!(!body.tx_instructions[0].wire_id.is_empty());
        assert!(!body.tx_instructions[0].payload_hex.is_empty());
    }

    #[tokio::test]
    async fn propose_deploy_builds_instruction_skeleton() {
        let (state, _queue, _chain_id) = mk_basic_context();
        let code_hash_input = format!("blake2b32:0X{}", "11".repeat(32));
        let canonical_abi = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        let abi_hash_input = format!("0x{}", hex::encode(canonical_abi));
        let dto = ProposeDeployContractDto {
            contract_address: Some(sample_contract_address()),
            contract_alias: None,
            abi_version: "1".to_string(),
            code_hash: code_hash_input.clone(),
            abi_hash: abi_hash_input.clone(),
            window: Some(AtWindowDto {
                lower: 10,
                upper: 20,
            }),
            mode: Some("Zk".to_string()),
            limits: Some(norito::json::Value::Object(norito::json::Map::new())),
            manifest_provenance: None,
        };
        let (code_hex, code_bytes) = super::canonical_hex32(&code_hash_input, "code_hash").unwrap();
        let (abi_hex, abi_bytes) = super::canonical_hex32(&abi_hash_input, "abi_hash").unwrap();
        let res = handle_gov_propose_deploy(state, NoritoJson(dto))
            .await
            .expect("handler ok");
        let body = res.0;
        assert!(body.ok);
        assert_eq!(body.tx_instructions.len(), 1);

        // Canonical hashing matches core logic
        let expected_id =
            super::compute_proposal_id(&sample_contract_address(), &code_bytes, &abi_bytes);
        assert_eq!(body.proposal_id, hex::encode(expected_id));
        assert_eq!(body.proposal_id.len(), 64);
        assert!(body.proposal_id.chars().all(|ch| ch.is_ascii_hexdigit()));

        // Payload decodes to sanitized ProposeDeployContract
        let tx = &body.tx_instructions[0];
        let payload = hex::decode(&tx.payload_hex).expect("payload hex");
        let decoded: iroha_data_model::isi::governance::ProposeDeployContract =
            norito::decode_from_bytes(&payload).expect("decode payload");
        assert_eq!(decoded.contract_address, sample_contract_address());
        assert_eq!(decoded.code_hash_hex, code_hex);
        assert_eq!(decoded.abi_hash_hex, abi_hex);
        assert_eq!(decoded.abi_version, "1");
    }

    #[tokio::test]
    async fn propose_sccp_route_governance_builds_exact_instruction_and_proposal_id() {
        let action = sample_sccp_route_governance_action();
        let expected_id = compute_sccp_route_governance_proposal_id(&action).expect("proposal id");
        let response =
            handle_gov_propose_sccp_route_governance(NoritoJson(ProposeSccpRouteGovernanceDto {
                action: action.clone(),
                window: Some(AtWindowDto {
                    lower: 10,
                    upper: 20,
                }),
                mode: Some(iroha_data_model::isi::governance::VotingMode::Zk),
            }))
            .await
            .expect("valid SCCP governance draft")
            .0;

        assert!(response.ok);
        assert_eq!(response.proposal_id, hex::encode(expected_id));
        assert_eq!(response.tx_instructions.len(), 1);
        let instruction = decode_tx_instruction(&response.tx_instructions[0]);
        let decoded = instruction
            .as_any()
            .downcast_ref::<iroha_data_model::isi::governance::ProposeSccpRouteGovernance>()
            .expect("exact SCCP governance instruction");
        assert_eq!(decoded.action, action);
        assert_eq!(
            decoded.window,
            Some(AtWindow {
                lower: 10,
                upper: 20
            })
        );
        assert_eq!(
            decoded.mode,
            Some(iroha_data_model::isi::governance::VotingMode::Zk)
        );
    }

    #[tokio::test]
    async fn propose_sccp_route_governance_rejects_invalid_action_before_drafting() {
        let mut action = sample_sccp_route_governance_action();
        let iroha_data_model::isi::bridge::SccpRouteGovernanceActionV1::Remove(key) = &mut action
        else {
            unreachable!("fixture is a remove action");
        };
        key.revision = 0;

        let error =
            handle_gov_propose_sccp_route_governance(NoritoJson(ProposeSccpRouteGovernanceDto {
                action,
                window: None,
                mode: None,
            }))
            .await
            .expect_err("invalid SCCP action must fail before returning a skeleton");
        assert!(
            format!("{error:?}").contains("invalid SCCP route governance action"),
            "unexpected error: {error:?}"
        );
    }

    #[tokio::test]
    async fn propose_sccp_route_governance_rejects_mode_aliases_and_reversed_window() {
        let action = sample_sccp_route_governance_action();
        let canonical = norito::json::to_json(&ProposeSccpRouteGovernanceDto {
            action: action.clone(),
            window: None,
            mode: Some(iroha_data_model::isi::governance::VotingMode::Zk),
        })
        .expect("canonical SCCP governance DTO");
        assert!(
            canonical.contains("\"mode\":\"Zk\""),
            "unexpected canonical DTO: {canonical}"
        );
        for mode in ["zk", "plain", "PLAIN", " Zk", "Zk ", "Quadratic"] {
            let aliased = canonical.replace("\"mode\":\"Zk\"", &format!("\"mode\":\"{mode}\""));
            let error = norito::json::from_str::<ProposeSccpRouteGovernanceDto>(&aliased)
                .expect_err("noncanonical SCCP voting mode must reject during typed decoding");
            assert!(
                error.to_string().contains(mode.trim())
                    || error.to_string().contains("VotingMode")
                    || error.to_string().contains("unknown variant"),
                "{mode}: {error}"
            );
        }

        let error =
            handle_gov_propose_sccp_route_governance(NoritoJson(ProposeSccpRouteGovernanceDto {
                action,
                window: Some(AtWindowDto {
                    lower: 21,
                    upper: 20,
                }),
                mode: None,
            }))
            .await
            .expect_err("reversed SCCP governance window must reject");
        assert!(format!("{error:?}").contains("window.upper"));
    }

    #[test]
    fn sccp_route_governance_dto_rejects_retired_signing_and_unknown_fields() {
        let dto = ProposeSccpRouteGovernanceDto {
            action: sample_sccp_route_governance_action(),
            window: None,
            mode: None,
        };
        let canonical = norito::json::to_json(&dto).expect("canonical SCCP governance DTO");
        let body = canonical.strip_suffix('}').expect("DTO JSON is an object");
        for (field, value) in [
            ("authority", "\"sorau...\""),
            ("private_key", "\"secret\""),
            ("manifest", "null"),
            ("future_action_policy", "null"),
        ] {
            let injected = format!("{body},\"{field}\":{value}}}");
            let error = norito::json::from_str::<ProposeSccpRouteGovernanceDto>(&injected)
                .expect_err("retired or unknown SCCP draft field must reject");
            assert!(
                error.to_string().contains(field) || error.to_string().contains("unknown field"),
                "{field}: {error}"
            );
        }
    }

    #[test]
    fn governance_mutation_dtos_reject_retired_signing_fields_during_decode() {
        macro_rules! assert_rejects_field {
            ($field:literal; $($request:ty),+ $(,)?) => {
                $(
                    let error = norito::json::from_str::<$request>(
                        concat!(r#"{"#, $field, r#"": "must-not-cross-torii"}"#),
                    )
                    .expect_err("retired signing field must fail JSON admission");
                    let message = error.to_string();
                    assert!(
                        message.contains("unknown field") && message.contains($field),
                        "{} admitted retired field `{}`: {message}",
                        stringify!($request),
                        $field,
                    );
                )+
            };
        }

        assert_rejects_field!(
            "private_key";
            ProposeDeployContractDto,
            ZkBallotDto,
            PlainBallotDto,
            ParliamentBallotDto,
            ZkBallotV1Dto,
            ZkBallotV1BallotProofDto,
            FinalizeDto,
        );
        assert_rejects_field!(
            "authority";
            ProposeDeployContractDto,
            FinalizeDto,
        );
    }

    #[tokio::test]
    async fn propose_deploy_rejects_unknown_mode() {
        let (state, _queue, _chain_id) = mk_basic_context();
        let canonical_abi = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        let dto = ProposeDeployContractDto {
            contract_address: Some(sample_contract_address()),
            contract_alias: None,
            abi_version: "1".to_string(),
            code_hash: format!("{}", "11".repeat(32)),
            abi_hash: format!("{}", hex::encode(canonical_abi)),
            window: None,
            mode: Some("quadratic".to_string()),
            limits: Some(norito::json::Value::Object(norito::json::Map::new())),
            manifest_provenance: None,
        };
        let err = handle_gov_propose_deploy(state, NoritoJson(dto))
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("unsupported voting mode"));
    }

    #[tokio::test]
    async fn propose_deploy_rejects_mismatched_abi_hash() {
        let (state, _queue, _chain_id) = mk_basic_context();
        let dto = ProposeDeployContractDto {
            contract_address: Some(sample_contract_address()),
            contract_alias: None,
            abi_version: "1".to_string(),
            code_hash: format!("{}", "11".repeat(32)),
            abi_hash: format!("{}", "22".repeat(32)),
            window: None,
            mode: None,
            limits: Some(norito::json::Value::Object(norito::json::Map::new())),
            manifest_provenance: None,
        };
        let err = handle_gov_propose_deploy(state, NoritoJson(dto))
            .await
            .unwrap_err();
        assert!(format!("{err:?}").contains("abi_hash does not match canonical hash"));
    }

    #[tokio::test]
    async fn ministry_agenda_draft_returns_instruction_skeleton_and_signable_payload() {
        let harness = mk_governance_harness(true);
        let proposal = sample_agenda_proposal("AC-2026-241");
        let response = handle_ministry_agenda_proposal_draft(
            harness.chain_id.clone(),
            harness.state.clone(),
            MaybeTelemetry::disabled(),
            NoritoJson(MinistryAgendaProposalDraftDto {
                proposal: proposal.clone(),
                authority: harness.authority.to_string(),
            }),
        )
        .await
        .expect("draft ok");

        let MinistryAgendaProposalDraftOutcome::Draft(body) = response else {
            panic!("expected successful draft");
        };
        assert!(body.ok);
        assert_eq!(body.agenda_proposal_id, proposal.proposal_id);
        assert_eq!(body.authority, harness.authority.to_string());
        assert_eq!(body.tx_instructions.len(), 1);

        let tx_bytes = base64::engine::general_purpose::STANDARD
            .decode(body.signable_transaction_b64.as_bytes())
            .expect("decode signable payload");
        let payload: iroha_data_model::transaction::signed::TransactionPayload = {
            let _guard = norito::core::PayloadCtxGuard::enter(&tx_bytes);
            let mut cursor = std::io::Cursor::new(tx_bytes.as_slice());
            norito::codec::Decode::decode(&mut cursor).expect("decode transaction payload")
        };
        assert_eq!(payload.authority, harness.authority);
        assert_eq!(payload.instructions.instruction_count(), 1);
    }

    #[tokio::test]
    async fn ministry_agenda_get_returns_missing_then_persisted_record() {
        let harness = mk_governance_harness(true);
        let proposal = sample_agenda_proposal("AC-2026-242");

        let missing = handle_ministry_agenda_proposal_get(
            harness.state.clone(),
            axum::extract::Path(proposal.proposal_id.clone()),
        )
        .await
        .expect("lookup ok")
        .0;
        assert!(!missing.found);
        assert!(missing.record.is_none());

        let draft = handle_ministry_agenda_proposal_draft(
            harness.chain_id.clone(),
            harness.state.clone(),
            MaybeTelemetry::disabled(),
            NoritoJson(MinistryAgendaProposalDraftDto {
                proposal: proposal.clone(),
                authority: harness.authority.to_string(),
            }),
        )
        .await
        .expect("draft ok");
        let MinistryAgendaProposalDraftOutcome::Draft(body) = draft else {
            panic!("expected successful draft");
        };
        queue_instruction_skeleton(&harness, &body.tx_instructions);
        let applied = crate::test_utils::apply_queued_in_one_block(
            &harness.state,
            &harness.queue,
            harness.chain_id.as_ref(),
            1,
        );
        assert_eq!(applied, 1);

        let persisted = handle_ministry_agenda_proposal_get(
            harness.state.clone(),
            axum::extract::Path(proposal.proposal_id.clone()),
        )
        .await
        .expect("lookup ok")
        .0;
        assert!(persisted.found);
        let record = persisted.record.expect("record");
        assert_eq!(record.proposal, proposal);
        assert_eq!(record.authority, harness.authority);
        assert!(!record.submitted_tx_hash_hex.is_empty());
        assert_eq!(record.submitted_height, 1);
    }

    #[tokio::test]
    async fn ministry_agenda_draft_preflights_duplicate_proposal_ids() {
        let harness = mk_governance_harness(true);
        let proposal = sample_agenda_proposal("AC-2026-243");
        let draft = handle_ministry_agenda_proposal_draft(
            harness.chain_id.clone(),
            harness.state.clone(),
            MaybeTelemetry::disabled(),
            NoritoJson(MinistryAgendaProposalDraftDto {
                proposal: proposal.clone(),
                authority: harness.authority.to_string(),
            }),
        )
        .await
        .expect("draft ok");
        let MinistryAgendaProposalDraftOutcome::Draft(body) = draft else {
            panic!("expected successful draft");
        };
        queue_instruction_skeleton(&harness, &body.tx_instructions);
        let applied = crate::test_utils::apply_queued_in_one_block(
            &harness.state,
            &harness.queue,
            harness.chain_id.as_ref(),
            1,
        );
        assert_eq!(applied, 1);

        let duplicate = handle_ministry_agenda_proposal_draft(
            harness.chain_id.clone(),
            harness.state.clone(),
            MaybeTelemetry::disabled(),
            NoritoJson(MinistryAgendaProposalDraftDto {
                proposal,
                authority: harness.authority.to_string(),
            }),
        )
        .await
        .expect("duplicate preflight ok");
        let MinistryAgendaProposalDraftOutcome::Duplicate(body) = duplicate else {
            panic!("expected duplicate summary");
        };
        assert!(body.found);
        assert_eq!(
            body.record
                .as_ref()
                .map(|record| record.proposal.proposal_id.as_str()),
            Some("AC-2026-243")
        );
    }

    #[tokio::test]
    async fn ballot_plain_builds_instruction_skeleton() {
        let (state, _queue, chain_id) = mk_basic_context();
        let canonical = canonical_literal(ACCOUNT_AUTHORITY);
        let chain_id_str = chain_id.as_str().to_string();
        // Build DTO via JSON to ensure serde shape is satisfied
        let body = crate::json_object(vec![
            crate::json_entry("authority", canonical.clone()),
            crate::json_entry("chain_id", chain_id_str),
            crate::json_entry("referendum_id", "r1"),
            crate::json_entry("owner", canonical.clone()),
            crate::json_entry("amount", "100"),
            crate::json_entry("duration_blocks", "600"),
            crate::json_entry("direction", "Aye"),
        ]);
        let parsed: PlainBallotDto =
            norito::json::from_str(&norito::json::to_json(&body).unwrap()).unwrap();
        let res = handle_gov_ballot_plain_with_policy(
            chain_id,
            state,
            NoritoJson(parsed),
            MaybeTelemetry::for_tests(),
        )
        .await
        .expect("handler ok");
        let body = res.0;
        assert!(body.ok);
        assert!(body.accepted);
        assert!(body.tx_instructions.len() == 1);
    }

    #[tokio::test]
    async fn ballot_plain_accepts_account_aliases() {
        let (state, _queue, chain_id) = mk_basic_context();
        let authority = AccountId::parse_encoded(ACCOUNT_AUTHORITY)
            .expect("account parses")
            .into_account_id();
        bind_account_alias_for_test(&state, &authority, "ballot@universal");
        let chain_id_str = chain_id.as_str().to_string();
        let body = crate::json_object(vec![
            crate::json_entry("authority", "ballot@universal"),
            crate::json_entry("chain_id", chain_id_str),
            crate::json_entry("referendum_id", "r1"),
            crate::json_entry("owner", "ballot@universal"),
            crate::json_entry("amount", "100"),
            crate::json_entry("duration_blocks", "600"),
            crate::json_entry("direction", "Aye"),
        ]);
        let parsed: PlainBallotDto =
            norito::json::from_str(&norito::json::to_json(&body).unwrap()).unwrap();
        let res = handle_gov_ballot_plain_with_policy(
            chain_id,
            state,
            NoritoJson(parsed),
            MaybeTelemetry::for_tests(),
        )
        .await
        .expect("handler ok");
        assert!(res.0.ok);
        assert!(res.0.accepted);
        assert_eq!(res.0.tx_instructions.len(), 1);
    }

    #[tokio::test]
    async fn ballot_plain_rejects_authority_mismatch() {
        let (state, _queue, chain_id) = mk_basic_context();
        let canonical_authority = canonical_literal(ACCOUNT_AUTHORITY);
        let canonical_owner = canonical_literal(ACCOUNT_OWNER_ALT);
        let chain_id_str = chain_id.as_str().to_string();
        let body = crate::json_object(vec![
            crate::json_entry("authority", canonical_authority),
            crate::json_entry("chain_id", chain_id_str),
            crate::json_entry("referendum_id", "r1"),
            crate::json_entry("owner", canonical_owner),
            crate::json_entry("amount", "100"),
            crate::json_entry("duration_blocks", "600"),
            crate::json_entry("direction", "Aye"),
        ]);
        let parsed: PlainBallotDto =
            norito::json::from_str(&norito::json::to_json(&body).unwrap()).unwrap();
        let err = handle_gov_ballot_plain_with_policy(
            chain_id,
            state,
            NoritoJson(parsed),
            MaybeTelemetry::for_tests(),
        )
        .await
        .unwrap_err();
        let s = format!("{err:?}");
        assert!(s.contains("authority must equal owner"));
    }

    #[tokio::test]
    async fn ballot_plain_accepts_raw_public_key_literals() {
        let (state, _queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        let body = crate::json_object(vec![
            crate::json_entry("authority", ACCOUNT_AUTHORITY),
            crate::json_entry("chain_id", chain_id_str),
            crate::json_entry("referendum_id", "r1"),
            crate::json_entry("owner", ACCOUNT_AUTHORITY),
            crate::json_entry("amount", "100"),
            crate::json_entry("duration_blocks", "600"),
            crate::json_entry("direction", "Aye"),
        ]);
        let parsed: PlainBallotDto =
            norito::json::from_str(&norito::json::to_json(&body).unwrap()).unwrap();
        handle_gov_ballot_plain_with_policy(
            chain_id,
            state,
            NoritoJson(parsed),
            MaybeTelemetry::for_tests(),
        )
        .await
        .expect("raw public key literals should be accepted");
    }

    #[tokio::test]
    async fn ballot_zk_builds_instruction_skeleton() {
        let (state, _queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        // minimal non-empty proof bytes
        let proof_b64 = base64::engine::general_purpose::STANDARD.encode(b"proof");
        let dto = ZkBallotDto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            chain_id: chain_id_str,
            election_id: "e1".to_string(),
            proof_b64,
            public: Some(norito::json::Value::Object(norito::json::Map::new())),
        };
        let res =
            handle_gov_ballot_zk(chain_id, state, MaybeTelemetry::disabled(), NoritoJson(dto))
                .await
                .expect("handler ok");
        let body = res.0;
        assert!(body.ok);
        assert!(body.accepted);
        assert_eq!(body.tx_instructions.len(), 1);
    }

    #[tokio::test]
    async fn ballot_zk_rejects_non_object_public_inputs() {
        let (state, _queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        let proof_b64 = base64::engine::general_purpose::STANDARD.encode(b"proof");
        let dto = ZkBallotDto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            chain_id: chain_id_str,
            election_id: "e1".to_string(),
            proof_b64,
            public: Some(norito::json::Value::String("oops".to_string())),
        };
        let res =
            handle_gov_ballot_zk(chain_id, state, MaybeTelemetry::disabled(), NoritoJson(dto))
                .await
                .expect("handler ok");
        let body = res.0;
        assert!(!body.ok);
        assert!(!body.accepted);
        assert_eq!(
            body.reason.as_deref(),
            Some("public inputs must be a JSON object")
        );
    }

    #[tokio::test]
    async fn ballot_zk_rejects_partial_lock_hints() {
        let (state, _queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        let proof_b64 = base64::engine::general_purpose::STANDARD.encode(b"proof");
        let mut map = norito::json::Map::new();
        map.insert(
            "owner".to_string(),
            norito::json::Value::from(ACCOUNT_AUTHORITY.to_string()),
        );
        let dto = ZkBallotDto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            chain_id: chain_id_str,
            election_id: "e1".to_string(),
            proof_b64,
            public: Some(norito::json::Value::Object(map)),
        };
        let res =
            handle_gov_ballot_zk(chain_id, state, MaybeTelemetry::disabled(), NoritoJson(dto))
                .await
                .expect("handler ok");
        let body = res.0;
        assert!(!body.ok);
        assert!(!body.accepted);
        assert_eq!(
            body.reason.as_deref(),
            Some("lock hints must include owner, amount, duration_blocks")
        );
    }

    #[tokio::test]
    async fn ballot_zk_rejects_noncanonical_owner_hint() {
        let (state, _queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        let proof_b64 = base64::engine::general_purpose::STANDARD.encode(b"proof");
        let owner = noncanonical_literal(ACCOUNT_AUTHORITY);
        let mut map = norito::json::Map::new();
        map.insert("owner".to_string(), norito::json::Value::from(owner));
        map.insert("amount".to_string(), norito::json::Value::from("100"));
        map.insert(
            "duration_blocks".to_string(),
            norito::json::Value::from(64u64),
        );
        let dto = ZkBallotDto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            chain_id: chain_id_str,
            election_id: "e1".to_string(),
            proof_b64,
            public: Some(norito::json::Value::Object(map)),
        };
        let res =
            handle_gov_ballot_zk(chain_id, state, MaybeTelemetry::disabled(), NoritoJson(dto))
                .await
                .expect("handler ok");
        let body = res.0;
        assert!(!body.ok);
        assert!(!body.accepted);
        assert_eq!(
            body.reason.as_deref(),
            Some("owner must use canonical I105 account id form")
        );
    }

    #[tokio::test]
    async fn ballot_zk_rejects_deprecated_public_inputs() {
        let (state, _queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        let proof_b64 = base64::engine::general_purpose::STANDARD.encode(b"proof");
        let mut map = norito::json::Map::new();
        map.insert(
            "rootHint".to_string(),
            norito::json::Value::from("aa".repeat(32)),
        );
        let dto = ZkBallotDto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            chain_id: chain_id_str,
            election_id: "e1".to_string(),
            proof_b64,
            public: Some(norito::json::Value::Object(map)),
        };
        let res =
            handle_gov_ballot_zk(chain_id, state, MaybeTelemetry::disabled(), NoritoJson(dto))
                .await
                .expect("handler ok");
        let body = res.0;
        assert!(!body.ok);
        assert!(!body.accepted);
        assert_eq!(
            body.reason.as_deref(),
            Some("public inputs must use root_hint (unsupported key rootHint)")
        );
    }

    #[tokio::test]
    async fn parliament_ballot_builds_instruction_skeleton() {
        let (state, _queue, chain_id) = mk_basic_context();
        let dto = ParliamentBallotDto {
            authority: canonical_literal(ACCOUNT_AUTHORITY),
            chain_id: chain_id.as_str().to_owned(),
            proposal_id: "11".repeat(32),
            body: ParliamentBody::PolicyJury,
            decision: ParliamentDecision::Approve,
        };

        let response = handle_gov_parliament_ballot(
            chain_id,
            state,
            MaybeTelemetry::disabled(),
            NoritoJson(dto),
        )
        .await
        .expect("valid Parliament ballot must draft")
        .0;

        assert!(response.ok);
        assert!(response.accepted);
        assert_eq!(
            response.reason.as_deref(),
            Some("build transaction skeleton")
        );
        assert_eq!(response.tx_instructions.len(), 1);
        let instruction = decode_tx_instruction(&response.tx_instructions[0]);
        let ballot = instruction
            .as_any()
            .downcast_ref::<iroha_data_model::isi::governance::CastParliamentBallot>()
            .expect("exact Parliament ballot instruction");
        assert_eq!(ballot.body, ParliamentBody::PolicyJury);
        assert_eq!(ballot.proposal_id, [0x11; 32]);
        assert_eq!(ballot.decision, ParliamentDecision::Approve);
    }

    #[test]
    fn normalize_zk_ballot_public_inputs_canonicalizes_hex() {
        let mut map = norito::json::Map::new();
        let root_raw = format!("0x{}", "Aa".repeat(32));
        map.insert("root_hint".to_string(), norito::json::Value::from(root_raw));
        let nullifier_raw = format!("blake2b32:{}", "BB".repeat(32));
        map.insert(
            "nullifier".to_string(),
            norito::json::Value::from(nullifier_raw),
        );
        normalize_zk_ballot_public_inputs(&mut map).expect("normalize");
        let root_expected = "aa".repeat(32);
        let nullifier_expected = "bb".repeat(32);
        assert!(map.contains_key("root_hint"));
        assert!(map.contains_key("nullifier"));
        assert_eq!(
            map.get("root_hint").and_then(norito::json::Value::as_str),
            Some(root_expected.as_str())
        );
        assert_eq!(
            map.get("nullifier").and_then(norito::json::Value::as_str),
            Some(nullifier_expected.as_str())
        );
    }

    #[test]
    fn normalize_zk_ballot_public_inputs_rejects_deprecated_keys() {
        let mut map = norito::json::Map::new();
        map.insert(
            "nullifier_hex".to_string(),
            norito::json::Value::from("aa".repeat(32)),
        );
        let err = normalize_zk_ballot_public_inputs(&mut map).expect_err("deprecated");
        assert!(err.contains("nullifier_hex"));
    }

    #[test]
    fn normalize_zk_ballot_public_inputs_rejects_invalid_hex() {
        let mut map = norito::json::Map::new();
        map.insert(
            "root_hint".to_string(),
            norito::json::Value::from("not-hex"),
        );
        let err = normalize_zk_ballot_public_inputs(&mut map).expect_err("invalid hex");
        assert_eq!(err, "root_hint must be 32-byte hex");
    }

    #[tokio::test]
    async fn gov_get_tally_applies_conviction_factor() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut state = State::new_for_testing(World::default(), kura, query);
        let mut cfg = state.gov.clone();
        cfg.conviction_step_blocks = 2;
        cfg.max_conviction = 4;
        state.set_gov(cfg);

        let rid = "rid-tally-conviction".to_string();
        let header = BlockHeader::new(
            core::num::NonZeroU64::new(1).unwrap(),
            None,
            None,
            None,
            0,
            0,
        );
        {
            let mut sblock = state.block(header);
            let mut stx = sblock.transaction();
            stx.world.governance_referenda_mut().insert(
                rid.clone(),
                GovernanceReferendumRecord {
                    h_start: 1,
                    h_end: 10,
                    status: GovernanceReferendumStatus::Open,
                    mode: GovernanceReferendumMode::Plain,
                },
            );
            let mut locks = GovernanceLocksForReferendum::default();
            locks.locks.insert(
                ALICE_ID.clone(),
                GovernanceLockRecord {
                    owner: ALICE_ID.clone(),
                    amount: 9_u64.into(),
                    slashed: Quantity::zero(),
                    expiry_height: 100,
                    direction: 0,
                    duration_blocks: 4,
                    custody: None,
                },
            );
            stx.world.governance_locks_mut().insert(rid.clone(), locks);
            stx.apply();
            let iroha_core::state::StateBlock { world, .. } = sblock;
            world.commit();
        }

        let res = handle_gov_get_tally(Arc::new(state), axum::extract::Path(rid))
            .await
            .expect("handler ok");
        let body = res.0;
        assert_eq!(body.approve, 9);
        assert_eq!(body.reject, 0);
        assert_eq!(body.evaluated_block_hash.len(), 64);
        assert!(
            body.evaluated_block_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        );
    }

    #[tokio::test]
    async fn gov_get_tally_rejects_missing_referendum() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), kura, query);

        let err = handle_gov_get_tally(
            Arc::new(state),
            axum::extract::Path("missing-referendum".to_owned()),
        )
        .await
        .expect_err("a nonexistent referendum must not look like a zero tally");

        assert!(matches!(
            err,
            crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::NotFound
            ))
        ));
    }

    #[tokio::test]
    async fn gov_get_tally_rejects_invalid_plain_direction() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), kura, query);
        let rid = "rid-tally-invalid-direction".to_string();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        {
            let mut block = state.block(header);
            let mut tx = block.transaction();
            tx.world.governance_referenda_mut().insert(
                rid.clone(),
                GovernanceReferendumRecord {
                    h_start: 1,
                    h_end: 10,
                    status: GovernanceReferendumStatus::Open,
                    mode: GovernanceReferendumMode::Plain,
                },
            );
            let mut locks = GovernanceLocksForReferendum::default();
            locks.locks.insert(
                ALICE_ID.clone(),
                GovernanceLockRecord {
                    owner: ALICE_ID.clone(),
                    amount: 9_u64.into(),
                    slashed: Quantity::zero(),
                    expiry_height: 100,
                    direction: 3,
                    duration_blocks: 4,
                    custody: None,
                },
            );
            tx.world.governance_locks_mut().insert(rid.clone(), locks);
            tx.apply();
            let iroha_core::state::StateBlock { world, .. } = block;
            world.commit();
        }

        let err = handle_gov_get_tally(Arc::new(state), axum::extract::Path(rid))
            .await
            .expect_err("an invalid direction must fail closed");
        let message = conversion_message(err);
        assert!(
            message.contains("invalid direction 3"),
            "unexpected tally error: {message}"
        );
    }

    #[tokio::test]
    async fn gov_get_tally_directs_validation_fee_referenda_to_typed_endpoint() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), kura, query);
        let plain_electorate_rules = validation_fee_plain_electorate_rules(&state.gov);
        let kind = iroha_data_model::governance::types::ProposalKind::ValidationFeePolicy(
            iroha_data_model::governance::types::ValidationFeePolicyProposal {
                policy: iroha_data_model::validation_fee::ValidationFeePolicyV1 {
                    schema_version:
                        iroha_data_model::validation_fee::VALIDATION_FEE_POLICY_SCHEMA_VERSION,
                    chain_id: ChainId::from("chain"),
                    genesis_hash: [7; 32],
                    policy_version: 1,
                    previous_policy_hash: None,
                    ds_asset_id: plain_electorate_rules.voting_asset_id.clone(),
                    ds_scale: iroha_data_model::validation_fee::VALIDATION_FEE_DS_SCALE,
                    fee: Quantity::zero(),
                    treasury_account_id: ALICE_ID.clone(),
                    charging_mode:
                        iroha_data_model::validation_fee::ValidationFeeChargingMode::Disabled,
                    effective_from_height: 1,
                    expires_after_height: None,
                    exemption_classes: Vec::new(),
                    treasury_payout_binding: None,
                },
                payout_lifecycle_proposal_id: None,
                plain_electorate_rules,
            },
        );
        let proposal_id = kind.fingerprint();
        let rid = hex::encode(proposal_id);
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        {
            let mut block = state.block(header);
            let mut tx = block.transaction();
            tx.world.governance_proposals_mut().insert(
                proposal_id,
                GovernanceProposalRecord {
                    proposer: ALICE_ID.clone(),
                    kind,
                    created_height: 1,
                    status: GovernanceProposalStatus::Proposed,
                    pipeline: GovernancePipeline::default(),
                    parliament_snapshot: None,
                    finalization_evidence: None,
                    enacted_at_height: None,
                },
            );
            tx.world.governance_referenda_mut().insert(
                rid.clone(),
                GovernanceReferendumRecord {
                    h_start: 1,
                    h_end: 10,
                    status: GovernanceReferendumStatus::Open,
                    mode: GovernanceReferendumMode::Plain,
                },
            );
            tx.apply();
            let iroha_core::state::StateBlock { world, .. } = block;
            world.commit();
        }

        let err = handle_gov_get_tally(Arc::new(state), axum::extract::Path(rid.clone()))
            .await
            .expect_err("generic tally must reject a validation-fee referendum");
        let message = conversion_message(err);
        assert!(
            message.contains(&format!("/v1/validation-fee/proposals/{rid}")),
            "unexpected tally error: {message}"
        );
    }

    #[tokio::test]
    async fn gov_get_tally_rejects_accumulator_overflow() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut state = State::new_for_testing(World::default(), kura, query);
        let mut cfg = state.gov.clone();
        cfg.conviction_step_blocks = 1;
        cfg.max_conviction = u64::MAX;
        state.set_gov(cfg);

        let rid = "rid-tally-overflow".to_string();
        let other = AccountId::parse_encoded(ACCOUNT_OWNER_ALT)
            .expect("alternate account id")
            .into_account_id();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        {
            let mut block = state.block(header);
            let mut tx = block.transaction();
            tx.world.governance_referenda_mut().insert(
                rid.clone(),
                GovernanceReferendumRecord {
                    h_start: 1,
                    h_end: u64::MAX,
                    status: GovernanceReferendumStatus::Open,
                    mode: GovernanceReferendumMode::Plain,
                },
            );
            let mut locks = GovernanceLocksForReferendum::default();
            for owner in [ALICE_ID.clone(), other] {
                locks.locks.insert(
                    owner.clone(),
                    GovernanceLockRecord {
                        owner,
                        amount: Quantity::from(u128::MAX),
                        slashed: Quantity::zero(),
                        expiry_height: u64::MAX,
                        direction: 0,
                        duration_blocks: u64::MAX - 1,
                        custody: None,
                    },
                );
            }
            tx.world.governance_locks_mut().insert(rid.clone(), locks);
            tx.apply();
            let iroha_core::state::StateBlock { world, .. } = block;
            world.commit();
        }

        let err = handle_gov_get_tally(Arc::new(state), axum::extract::Path(rid))
            .await
            .expect_err("overflowing tally must fail");
        let message = conversion_message(err);
        assert!(
            message.contains("governance tally arithmetic overflow"),
            "unexpected tally error: {message}"
        );
    }

    #[tokio::test]
    async fn enact_rejects_noncanonical_proposal_id() {
        let (state, _queue, _chain_id) = mk_basic_context();
        let dto = EnactDto {
            proposal_id: format!("0x{}", "ab".repeat(32)),
        };
        let error = handle_gov_enact(state, NoritoJson(dto))
            .await
            .expect_err("0x-prefixed proposal id must be rejected");
        assert!(error.to_string().contains("64 lowercase hexadecimal"));
    }

    #[test]
    fn validation_fee_enactment_draft_is_available_for_one_exact_next_block() {
        let activation_delay =
            iroha_data_model::validation_fee::VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS;
        let target = 3_604;
        let effective = target + activation_delay;

        let early = validate_validation_fee_policy_enactment_draft_height(effective, target - 2)
            .expect_err("next block is still earlier than the exact target");
        assert!(early.contains("not ready"));
        assert!(early.contains("3604"));

        assert_eq!(
            validate_validation_fee_policy_enactment_draft_height(effective, target - 1)
                .expect("the next committed block is the exact target"),
            target
        );

        let missed = validate_validation_fee_policy_enactment_draft_height(effective, target)
            .expect_err("the exact target cannot be recovered after it is missed");
        assert!(missed.contains("was missed"));
        assert!(missed.contains("3604"));
    }

    #[test]
    fn validation_fee_enactment_draft_height_fails_closed_on_overflow() {
        let activation_delay =
            iroha_data_model::validation_fee::VALIDATION_FEE_POLICY_ACTIVATION_DELAY_BLOCKS;
        assert!(
            validate_validation_fee_policy_enactment_draft_height(
                activation_delay.saturating_sub(1),
                1,
            )
            .expect_err("effective height below activation delay is invalid")
            .contains("cannot encode")
        );
        assert!(
            validate_validation_fee_policy_enactment_draft_height(u64::MAX, u64::MAX)
                .expect_err("next block height overflow must fail closed")
                .contains("exceeds")
        );
    }

    #[test]
    fn governed_contract_entrypoint_names_are_closed_ascii_identifiers() {
        for name in ["a", "balance", "transfer_2"] {
            assert!(is_canonical_public_entrypoint_name(name), "{name}");
        }
        for name in [
            "",
            "Balance",
            "2transfer",
            "transfer-funds",
            "transfer funds",
            "tránsfer",
            &"a".repeat(129),
        ] {
            assert!(!is_canonical_public_entrypoint_name(name), "{name}");
        }
    }

    #[tokio::test]
    async fn governed_contract_read_serializes_exact_inactive_shape() {
        let harness = mk_governance_harness(true);
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &harness.authority,
            92,
            iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
        )
        .expect("inactive contract address");
        let response = handle_gov_contract_get(
            harness.state,
            axum::extract::Path(contract_address.to_string()),
        )
        .await
        .expect("inactive governed contract read");
        let value = norito::json::to_value(&response.0).expect("serialize inactive response");
        let object = value.as_object().expect("inactive response object");
        assert_eq!(
            object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            ["found", "contract_address", "dataspace"]
                .into_iter()
                .collect()
        );
        assert_eq!(object.get("found"), Some(&norito::json::Value::Bool(false)));
        assert_eq!(
            object
                .get("contract_address")
                .and_then(norito::json::Value::as_str),
            Some(contract_address.as_ref())
        );
        assert_eq!(
            object
                .get("dataspace")
                .and_then(norito::json::Value::as_str),
            Some("universal")
        );
    }

    #[tokio::test]
    async fn governed_contract_read_verifies_real_artifact_and_exact_active_shape() {
        let harness = mk_governance_harness(true);
        let (contract_address, expected_code_hash) = install_governed_contract_for_test(&harness);
        let response = handle_gov_contract_get(
            harness.state,
            axum::extract::Path(contract_address.to_string()),
        )
        .await
        .expect("active governed contract read");
        let value = norito::json::to_value(&response.0).expect("serialize active response");
        let object = value.as_object().expect("active response object");
        assert_eq!(
            object.keys().map(String::as_str).collect::<BTreeSet<_>>(),
            [
                "found",
                "contract_address",
                "contract_subject_account",
                "dataspace",
                "code_hash_hex",
                "abi_hash_hex",
                "public_entrypoints",
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(object.get("found"), Some(&norito::json::Value::Bool(true)));
        assert_eq!(
            object
                .get("contract_address")
                .and_then(norito::json::Value::as_str),
            Some(contract_address.as_ref())
        );
        assert_eq!(
            object
                .get("contract_subject_account")
                .and_then(norito::json::Value::as_str),
            Some(contract_address.subject_id().to_string().as_str())
        );
        assert_eq!(
            object
                .get("code_hash_hex")
                .and_then(norito::json::Value::as_str),
            Some(hex::encode(<[u8; 32]>::from(expected_code_hash)).as_str())
        );
        assert_eq!(
            object.get("public_entrypoints"),
            Some(&norito::json!(["balance", "transfer"]))
        );
    }

    #[tokio::test]
    async fn governed_contract_read_rejects_incomplete_active_state() {
        let harness = mk_governance_harness(true);
        let contract_address = iroha_data_model::smart_contract::ContractAddress::derive(
            iroha_data_model::account::address::chain_discriminant(),
            &harness.authority,
            93,
            iroha_data_model::nexus::DataSpaceId::UNIVERSAL,
        )
        .expect("incomplete contract address");
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = harness.state.block(header);
        let mut transaction = block.transaction();
        transaction
            .world_mut_for_testing()
            .bind_active_contract_subject_for_testing(
                contract_address.clone(),
                iroha_crypto::Hash::prehashed([0x44; 32]),
            );
        transaction.apply();
        block.commit().expect("commit incomplete fixture");

        let error = handle_gov_contract_get(
            harness.state,
            axum::extract::Path(contract_address.to_string()),
        )
        .await
        .expect_err("incomplete active state must fail closed");
        assert!(error.to_string().contains("incomplete code"));
    }

    #[tokio::test]
    async fn governed_contract_read_rejects_removed_manifest_provenance() {
        let harness = mk_governance_harness(true);
        let (contract_address, code_hash) = install_governed_contract_for_test(&harness);
        let mut manifest = harness
            .state
            .view()
            .world()
            .contract_manifests()
            .get(&code_hash)
            .cloned()
            .expect("registered manifest");
        manifest.provenance = None;
        let header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut block = harness.state.block(header);
        let mut transaction = block.transaction();
        transaction
            .world_mut_for_testing()
            .contract_manifests_mut_for_testing()
            .insert(code_hash, manifest);
        transaction.apply();
        block.commit().expect("commit corrupted manifest fixture");

        let error = handle_gov_contract_get(
            harness.state,
            axum::extract::Path(contract_address.to_string()),
        )
        .await
        .expect_err("unsigned active manifest must fail closed");
        assert!(error.to_string().contains("signed provenance"));
    }

    #[tokio::test]
    async fn gov_flow_submits_and_applies() {
        let harness = mk_governance_harness(true);
        let authority_str = harness.authority.to_string();
        let chain_id_str = harness.chain_id.as_str().to_string();

        let (artifact, manifest) = ivm::KotodamaCompiler::new()
            .compile_source_with_manifest(
                r#"
seiyaku GovernanceFlowFixture {
    view fn ready() -> bool { return true; }
}
"#,
            )
            .expect("compile governance flow contract fixture");
        let verified =
            ivm::verify_contract_artifact(&artifact).expect("verify governance flow contract");
        let code_hash_bytes: [u8; 32] = verified.code_hash.into();
        let abi_hash_bytes: [u8; 32] = verified.abi_hash.into();
        let signed_manifest = manifest.signed(&harness.authority_keypair);
        let manifest_provenance = signed_manifest
            .provenance
            .clone()
            .expect("signed governance flow manifest provenance");
        {
            let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
            let mut block = harness.state.block(header);
            let mut transaction = block.transaction();
            let registered_hash =
                register_code_bytes(&harness.authority, artifact, &mut transaction)
                    .expect("register governance flow contract bytes");
            assert_eq!(registered_hash, verified.code_hash);
            register_manifest(&harness.authority, signed_manifest, &mut transaction)
                .expect("register signed governance flow contract manifest");
            transaction.apply();
            block
                .commit()
                .expect("commit governance flow contract artifacts");
        }
        let contract_address = sample_contract_address();

        let propose = ProposeDeployContractDto {
            contract_address: Some(contract_address.clone()),
            contract_alias: None,
            abi_version: "1".to_string(),
            code_hash: format!("0x{}", hex::encode(code_hash_bytes)),
            abi_hash: format!("0x{}", hex::encode(abi_hash_bytes)),
            window: None,
            mode: Some("Plain".to_string()),
            limits: None,
            manifest_provenance: Some(manifest_provenance),
        };
        let res = handle_gov_propose_deploy(harness.state.clone(), NoritoJson(propose))
            .await
            .expect("propose ok");
        let proposal_id = res.0.proposal_id.clone();
        queue_instruction_skeleton(&harness, &res.0.tx_instructions);
        let mut height = 1_u64;
        let applied = crate::test_utils::apply_queued_in_one_block(
            &harness.state,
            &harness.queue,
            harness.chain_id.as_ref(),
            height,
        );
        assert_eq!(applied, 1);
        height += 1;

        let pid_bytes = hex::decode(&proposal_id).expect("proposal id hex");
        let mut pid_arr = [0u8; 32];
        pid_arr.copy_from_slice(&pid_bytes);
        {
            let view = harness.state.view();
            let proposal = view
                .world()
                .governance_proposals()
                .get(&pid_arr)
                .cloned()
                .expect("proposal stored");
            assert!(matches!(
                proposal.status,
                GovernanceProposalStatus::Proposed
            ));
            let referendum = view
                .world()
                .governance_referenda()
                .get(&proposal_id)
                .copied()
                .expect("referendum stored");
            assert!(matches!(referendum.mode, GovernanceReferendumMode::Plain));
        }

        let approvals_header = BlockHeader::new(nonzero!(2_u64), None, None, None, 0, 0);
        let mut approvals_block = harness.state.block(approvals_header);
        let mut approvals_tx = approvals_block.transaction();
        let mut approvals = GovernanceStageApprovals::default();
        for body in [
            ParliamentBody::RulesCommittee,
            ParliamentBody::AgendaCouncil,
            ParliamentBody::InterestPanel,
            ParliamentBody::ReviewPanel,
            ParliamentBody::PolicyJury,
            ParliamentBody::OversightCommittee,
        ] {
            approvals
                .ensure_stage(body, 0, 1, approvals_tx.gov.parliament_quorum_bps)
                .record(harness.authority.clone());
        }
        approvals_tx
            .world
            .governance_stage_approvals_mut()
            .insert(proposal_id.clone(), approvals);
        approvals_tx.apply();
        approvals_block.commit().expect("commit approvals");

        let ballot = PlainBallotDto {
            authority: authority_str.clone(),
            chain_id: chain_id_str.clone(),
            referendum_id: proposal_id.clone(),
            owner: authority_str.clone(),
            amount: 100_u64.into(),
            duration_blocks: "10".to_owned(),
            direction: "Aye".to_string(),
        };
        let ballot = handle_gov_ballot_plain_with_policy(
            harness.chain_id.clone(),
            harness.state.clone(),
            NoritoJson(ballot),
            MaybeTelemetry::disabled(),
        )
        .await
        .expect("ballot ok");
        queue_instruction_skeleton(&harness, &ballot.0.tx_instructions);
        let applied = crate::test_utils::apply_queued_in_one_block(
            &harness.state,
            &harness.queue,
            harness.chain_id.as_ref(),
            height,
        );
        assert_eq!(applied, 1);
        height += 1;

        let locks = harness
            .state
            .view()
            .world()
            .governance_locks()
            .get(&proposal_id)
            .cloned()
            .expect("locks stored");
        let lock = locks.locks.get(&harness.authority).expect("authority lock");
        assert_eq!(lock.amount, Quantity::from(100_u64));
        assert_eq!(lock.direction, 0);

        let mut lifecycle_height = height;
        while lifecycle_height <= 11 {
            let latest_block = harness.state.view().latest_block();
            let leader = checked_governance_bls_keypair(
                u8::try_from(lifecycle_height).expect("small fixture height"),
            );
            let new_block = BlockBuilder::new(Vec::new())
                .chain(0, latest_block.as_deref())
                .sign(leader.private_key())
                .unpack(|_| {});
            assert_eq!(
                new_block.header().height().get(),
                lifecycle_height,
                "referendum lifecycle fixture must advance the canonical block chain"
            );
            let mut state_block = harness.state.block(new_block.header());
            state_block.chain_id = harness.chain_id.as_ref().clone();
            let valid_block = new_block
                .validate_and_record_transactions(&mut state_block)
                .unpack(|_| {});
            let committed_block = valid_block.commit_unchecked().unpack(|_| {});
            crate::test_utils::finalize_committed_block(
                &harness.state,
                state_block,
                committed_block,
            );
            lifecycle_height += 1;
        }
        height = lifecycle_height;

        let proposal = harness
            .state
            .view()
            .world()
            .governance_proposals()
            .get(&pid_arr)
            .cloned()
            .expect("proposal present");
        assert!(matches!(
            proposal.status,
            GovernanceProposalStatus::Approved
        ));

        let enact = EnactDto {
            proposal_id: proposal_id.clone(),
        };
        let enact = handle_gov_enact(harness.state.clone(), NoritoJson(enact))
            .await
            .expect("enact ok");
        queue_instruction_skeleton(&harness, &enact.0.tx_instructions);
        let applied = crate::test_utils::apply_queued_in_one_block(
            &harness.state,
            &harness.queue,
            harness.chain_id.as_ref(),
            height,
        );
        assert_eq!(applied, 1);

        let view = harness.state.view();
        let code_hash = iroha_crypto::Hash::prehashed(code_hash_bytes);
        let bound_hash = view
            .world()
            .contract_instances()
            .get(&contract_address)
            .copied()
            .expect("instance bound");
        assert_eq!(bound_hash, code_hash);
        assert!(view.world().contract_manifests().get(&code_hash).is_some());
        let proposal = view
            .world()
            .governance_proposals()
            .get(&pid_arr)
            .cloned()
            .expect("proposal present");
        assert!(matches!(proposal.status, GovernanceProposalStatus::Enacted));
    }

    #[tokio::test]
    async fn propose_deploy_rejected_without_permission() {
        let harness = mk_governance_harness(false);
        let code_hash_bytes = [0x22u8; 32];
        let abi_hash_bytes = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        let manifest_provenance =
            mk_manifest_provenance(&harness.authority_keypair, code_hash_bytes, abi_hash_bytes);
        let propose = ProposeDeployContractDto {
            contract_address: Some(sample_contract_address()),
            contract_alias: None,
            abi_version: "1".to_string(),
            code_hash: format!("0x{}", hex::encode(code_hash_bytes)),
            abi_hash: format!("0x{}", hex::encode(abi_hash_bytes)),
            window: None,
            mode: Some("Plain".to_string()),
            limits: None,
            manifest_provenance: Some(manifest_provenance),
        };
        let res = handle_gov_propose_deploy(harness.state.clone(), NoritoJson(propose))
            .await
            .expect("handler ok");
        let proposal_id = res.0.proposal_id.clone();
        queue_instruction_skeleton(&harness, &res.0.tx_instructions);
        let errors = apply_queued_block_allow_errors(&harness.state, &harness.queue, 1);
        assert_eq!(errors, vec![true]);
        let pid_bytes = hex::decode(&proposal_id).expect("proposal id hex");
        let mut pid_arr = [0u8; 32];
        pid_arr.copy_from_slice(&pid_bytes);
        assert!(
            harness
                .state
                .view()
                .world()
                .governance_proposals()
                .get(&pid_arr)
                .is_none(),
            "proposal should not be persisted without permission"
        );
    }

    #[tokio::test]
    async fn ballot_zk_v1_builds_instruction_skeleton() {
        use axum::{Router, routing::post};
        use http_body_util::BodyExt as _;
        use tower::ServiceExt as _;

        let (state, _queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        // Route for zk-v1
        let app =
            Router::new().route(
                "/v1/gov/ballots/zk-v1",
                post({
                    let state = state.clone();
                    let chain_id = chain_id.clone();
                    move |body: crate::NoritoJsonWithBytes<super::ZkBallotV1Dto>| {
                        let telemetry = MaybeTelemetry::disabled();
                        async move {
                            super::handle_gov_ballot_zk_v1(chain_id, state, telemetry, body).await
                        }
                    }
                }),
            );

        // Build DTO
        let owner = canonical_literal(ACCOUNT_AUTHORITY);
        let dto = super::ZkBallotV1Dto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            chain_id: chain_id_str,
            election_id: "ref-1".to_string(),
            backend: "halo2/ipa".to_string(),
            envelope_b64: base64::engine::general_purpose::STANDARD.encode(&[1u8, 2, 3, 4]),
            root_hint: Some(hex::encode([0u8; 32])),
            owner: Some(owner),
            amount: Some(100_u64.into()),
            duration_blocks: Some(200),
            direction: Some("Aye".to_string()),
            nullifier: Some(hex::encode([0x11u8; 32])),
        };
        let req = http::Request::builder()
            .method("POST")
            .uri("/v1/gov/ballots/zk-v1")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(
                norito::json::to_vec(&norito::json::to_value(&dto).unwrap()).unwrap(),
            ))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);
        let b = resp.into_body().collect().await.unwrap().to_bytes();
        let v: norito::json::Value = norito::json::from_slice(&b).unwrap();
        assert_eq!(
            v.get("ok").and_then(norito::json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            v.get("accepted").and_then(norito::json::Value::as_bool),
            Some(true)
        );
        assert!(
            v.get("tx_instructions")
                .and_then(|x| x.as_array())
                .is_some()
        );
    }

    #[tokio::test]
    async fn ballot_zk_v1_rejects_invalid_root_hint() {
        let (state, _queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        let dto = super::ZkBallotV1Dto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            chain_id: chain_id_str,
            election_id: "ref-1".to_string(),
            backend: "halo2/ipa".to_string(),
            envelope_b64: base64::engine::general_purpose::STANDARD.encode(&[1u8, 2, 3, 4]),
            root_hint: Some("invalid".to_string()),
            owner: None,
            amount: None,
            duration_blocks: None,
            direction: None,
            nullifier: None,
        };
        let raw =
            Bytes::from(norito::json::to_vec(&norito::json::to_value(&dto).unwrap()).unwrap());
        let res = super::handle_gov_ballot_zk_v1(
            chain_id,
            state,
            MaybeTelemetry::disabled(),
            crate::NoritoJsonWithBytes { value: dto, raw },
        )
        .await
        .expect("handler ok");
        let body = res.0;
        assert!(!body.ok);
        assert!(!body.accepted);
        assert_eq!(
            body.reason.as_deref(),
            Some("root_hint must be 32-byte hex")
        );
    }

    #[tokio::test]
    async fn ballot_zk_v1_rejects_partial_lock_hints() {
        let (state, _queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        let dto = super::ZkBallotV1Dto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            chain_id: chain_id_str,
            election_id: "ref-1".to_string(),
            backend: "halo2/ipa".to_string(),
            envelope_b64: base64::engine::general_purpose::STANDARD.encode(&[1u8, 2, 3, 4]),
            root_hint: None,
            owner: Some(ACCOUNT_AUTHORITY.to_string()),
            amount: None,
            duration_blocks: None,
            direction: None,
            nullifier: None,
        };
        let raw =
            Bytes::from(norito::json::to_vec(&norito::json::to_value(&dto).unwrap()).unwrap());
        let res = super::handle_gov_ballot_zk_v1(
            chain_id,
            state,
            MaybeTelemetry::disabled(),
            crate::NoritoJsonWithBytes { value: dto, raw },
        )
        .await
        .expect("handler ok");
        let body = res.0;
        assert!(!body.ok);
        assert!(!body.accepted);
        assert_eq!(
            body.reason.as_deref(),
            Some("lock hints must include owner, amount, duration_blocks")
        );
    }

    #[tokio::test]
    async fn ballot_zk_v1_rejects_alias_keys_in_raw_json() {
        let (state, _queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        let envelope_b64 = base64::engine::general_purpose::STANDARD.encode(&[1u8, 2, 3, 4]);
        let root_hint = hex::encode([0u8; 32]);
        let dto = super::ZkBallotV1Dto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            chain_id: chain_id_str.clone(),
            election_id: "ref-1".to_string(),
            backend: "halo2/ipa".to_string(),
            envelope_b64: envelope_b64.clone(),
            root_hint: Some(root_hint.clone()),
            owner: None,
            amount: None,
            duration_blocks: None,
            direction: None,
            nullifier: None,
        };
        let root_hint_alias = root_hint.clone();
        let raw = Bytes::from(
            norito::json::to_vec(&norito::json!({
                "authority": ACCOUNT_AUTHORITY,
                "chain_id": chain_id_str,
                "election_id": "ref-1",
                "backend": "halo2/ipa",
                "envelope_b64": envelope_b64,
                "root_hint": root_hint_alias,
                "rootHintHex": root_hint,
            }))
            .unwrap(),
        );
        let res = super::handle_gov_ballot_zk_v1(
            chain_id,
            state,
            MaybeTelemetry::disabled(),
            crate::NoritoJsonWithBytes { value: dto, raw },
        )
        .await
        .expect("handler ok");
        let body = res.0;
        assert!(!body.ok);
        assert!(!body.accepted);
        assert_eq!(
            body.reason.as_deref(),
            Some("public inputs must use root_hint (unsupported key rootHintHex)")
        );
    }

    #[tokio::test]
    async fn ballot_zk_v1_rejects_noncanonical_owner_hint() {
        let (state, _queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        let owner = noncanonical_literal(ACCOUNT_AUTHORITY);
        let dto = super::ZkBallotV1Dto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            chain_id: chain_id_str,
            election_id: "ref-1".to_string(),
            backend: "halo2/ipa".to_string(),
            envelope_b64: base64::engine::general_purpose::STANDARD.encode(&[1u8, 2, 3, 4]),
            root_hint: None,
            owner: Some(owner),
            amount: Some(100_u64.into()),
            duration_blocks: Some(200),
            direction: None,
            nullifier: None,
        };
        let raw =
            Bytes::from(norito::json::to_vec(&norito::json::to_value(&dto).unwrap()).unwrap());
        let res = super::handle_gov_ballot_zk_v1(
            chain_id,
            state,
            MaybeTelemetry::disabled(),
            crate::NoritoJsonWithBytes { value: dto, raw },
        )
        .await
        .expect("handler ok");
        let body = res.0;
        assert!(!body.ok);
        assert!(!body.accepted);
        assert_eq!(
            body.reason.as_deref(),
            Some("owner must use canonical I105 account id form")
        );
    }

    #[tokio::test]
    async fn ballot_zk_v1_ballotproof_builds_instruction_skeleton() {
        use axum::{Router, routing::post};
        use http_body_util::BodyExt as _;
        use iroha_data_model::isi::governance::BallotProof;
        use tower::ServiceExt as _;

        let (state, _queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        // Route for zk-v1/ballot-proof
        let app = Router::new().route(
            "/v1/gov/ballots/zk-v1/ballot-proof",
            post({
                let state = state.clone();
                let chain_id = chain_id.clone();
                move |body: crate::NoritoJsonWithBytes<super::ZkBallotV1BallotProofDto>| {
                    let telemetry = MaybeTelemetry::disabled();
                    async move {
                        super::handle_gov_ballot_zk_v1_ballotproof(chain_id, state, telemetry, body)
                            .await
                    }
                }
            }),
        );

        // Build DTO
        let owner = canonical_literal(ACCOUNT_AUTHORITY);
        let ballot = BallotProof {
            backend: "halo2/ipa".into(),
            envelope_bytes: vec![1u8, 2, 3, 4],
            root_hint: Some([0xAA; 32]),
            owner: Some(
                AccountId::parse_encoded(&owner)
                    .expect("valid account id")
                    .into_account_id(),
            ),
            nullifier: Some([0x11; 32]),
            amount: Some(200_u64.into()),
            duration_blocks: Some(256),
            direction: Some("Nay".to_string()),
        };
        let dto = super::ZkBallotV1BallotProofDto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            chain_id: chain_id_str,
            election_id: "ref-1".to_string(),
            ballot,
        };
        let req = http::Request::builder()
            .method("POST")
            .uri("/v1/gov/ballots/zk-v1/ballot-proof")
            .header(http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(
                norito::json::to_vec(&norito::json::to_value(&dto).unwrap()).unwrap(),
            ))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), http::StatusCode::OK);
        let b = resp.into_body().collect().await.unwrap().to_bytes();
        let v: norito::json::Value = norito::json::from_slice(&b).unwrap();
        assert_eq!(
            v.get("ok").and_then(norito::json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            v.get("accepted").and_then(norito::json::Value::as_bool),
            Some(true)
        );
        assert!(
            v.get("tx_instructions")
                .and_then(|x| x.as_array())
                .is_some()
        );
    }

    #[tokio::test]
    async fn ballot_zk_v1_ballotproof_rejects_alias_keys_in_raw_json() {
        use iroha_data_model::isi::governance::BallotProof;

        let (state, _queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        let envelope_b64 = base64::engine::general_purpose::STANDARD.encode(&[1u8, 2, 3, 4]);
        let root_hint = hex::encode([0xAAu8; 32]);
        let ballot = BallotProof {
            backend: "halo2/ipa".into(),
            envelope_bytes: vec![1u8, 2, 3, 4],
            root_hint: Some([0xAAu8; 32]),
            owner: None,
            nullifier: None,
            amount: None,
            duration_blocks: None,
            direction: None,
        };
        let dto = super::ZkBallotV1BallotProofDto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            chain_id: chain_id_str.clone(),
            election_id: "ref-1".to_string(),
            ballot,
        };
        let root_hint_alias = root_hint.clone();
        let raw = Bytes::from(
            norito::json::to_vec(&norito::json!({
                "authority": ACCOUNT_AUTHORITY,
                "chain_id": chain_id_str,
                "election_id": "ref-1",
                "ballot": {
                    "backend": "halo2/ipa",
                    "envelope_bytes": envelope_b64,
                    "root_hint": root_hint_alias,
                    "rootHintHex": root_hint,
                },
            }))
            .unwrap(),
        );
        let res = super::handle_gov_ballot_zk_v1_ballotproof(
            chain_id,
            state,
            MaybeTelemetry::disabled(),
            crate::NoritoJsonWithBytes { value: dto, raw },
        )
        .await
        .expect("handler ok");
        let body = res.0;
        assert!(!body.ok);
        assert!(!body.accepted);
        assert_eq!(
            body.reason.as_deref(),
            Some("public inputs must use root_hint (unsupported key rootHintHex)")
        );
    }

    #[tokio::test]
    async fn ballot_zk_v1_ballotproof_rejects_noncanonical_owner_hint_in_raw_json() {
        use iroha_data_model::isi::governance::BallotProof;

        let (state, _queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        let envelope_b64 = base64::engine::general_purpose::STANDARD.encode(&[1u8, 2, 3, 4]);
        let owner_canonical = canonical_literal(ACCOUNT_AUTHORITY);
        let owner_noncanonical = noncanonical_literal(ACCOUNT_AUTHORITY);
        let ballot = BallotProof {
            backend: "halo2/ipa".into(),
            envelope_bytes: vec![1u8, 2, 3, 4],
            root_hint: None,
            owner: Some(
                AccountId::parse_encoded(&owner_canonical)
                    .expect("valid account id")
                    .into_account_id(),
            ),
            nullifier: None,
            amount: Some(200_u64.into()),
            duration_blocks: Some(256),
            direction: None,
        };
        let dto = super::ZkBallotV1BallotProofDto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            chain_id: chain_id_str.clone(),
            election_id: "ref-1".to_string(),
            ballot,
        };
        let raw = Bytes::from(
            norito::json::to_vec(&norito::json!({
                "authority": ACCOUNT_AUTHORITY,
                "chain_id": chain_id_str,
                "election_id": "ref-1",
                "ballot": {
                    "backend": "halo2/ipa",
                    "envelope_bytes": envelope_b64,
                    "owner": owner_noncanonical,
                    "amount": "200",
                    "duration_blocks": 256,
                },
            }))
            .unwrap(),
        );
        let res = super::handle_gov_ballot_zk_v1_ballotproof(
            chain_id,
            state,
            MaybeTelemetry::disabled(),
            crate::NoritoJsonWithBytes { value: dto, raw },
        )
        .await
        .expect("handler ok");
        let body = res.0;
        assert!(!body.ok);
        assert!(!body.accepted);
        assert_eq!(
            body.reason.as_deref(),
            Some("owner must use canonical I105 account id form")
        );
    }

    #[tokio::test]
    async fn ballot_zk_v1_ballotproof_rejects_partial_lock_hints() {
        use iroha_data_model::isi::governance::BallotProof;

        let (state, _queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        let ballot = BallotProof {
            backend: "halo2/ipa".into(),
            envelope_bytes: vec![1u8, 2, 3, 4],
            root_hint: None,
            owner: Some(
                AccountId::parse_encoded(ACCOUNT_AUTHORITY)
                    .expect("valid account id")
                    .into_account_id(),
            ),
            nullifier: None,
            amount: None,
            duration_blocks: None,
            direction: None,
        };
        let dto = super::ZkBallotV1BallotProofDto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            chain_id: chain_id_str,
            election_id: "ref-1".to_string(),
            ballot,
        };
        let raw =
            Bytes::from(norito::json::to_vec(&norito::json::to_value(&dto).unwrap()).unwrap());
        let res = super::handle_gov_ballot_zk_v1_ballotproof(
            chain_id,
            state,
            MaybeTelemetry::disabled(),
            crate::NoritoJsonWithBytes { value: dto, raw },
        )
        .await
        .expect("handler ok");
        let body = res.0;
        assert!(!body.ok);
        assert!(!body.accepted);
        assert_eq!(
            body.reason.as_deref(),
            Some("lock hints must include owner, amount, duration_blocks")
        );
    }
}
