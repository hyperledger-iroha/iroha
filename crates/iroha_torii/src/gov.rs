//! App-facing governance API.
#![allow(unexpected_cfgs)]
//!
//! This module hosts minimal DTOs and handlers for governance endpoints
//! described in `gov.md` and `docs/source/contract_deployment.md`.
//! Handlers validate inputs, build instruction skeletons, and optionally submit
//! signed transactions when `authority`/`private_key` are provided.
//!
//! Notes
//! - JSON parsing uses Norito's serde wrappers via the `NoritoJson` extractor.
//! - Keep responses stable and explicit; map input errors to 400.

use core::str::FromStr;

use base64::Engine as _;
use iroha_core::{
    governance::{
        draw,
        parliament::{self, CandidateRef, CandidateVariant},
    },
    smartcontracts::Execute as _,
    state::{StateReadOnly, WorldReadOnly},
};
use iroha_crypto::blake2::{Blake2b512, digest::Digest};
use iroha_data_model::{
    governance::types::AtWindow, isi::governance::CouncilDerivationKind,
    smart_contract::manifest::ManifestProvenance,
};
use mv::storage::StorageReadOnly;
use norito::{
    codec::Encode as _,
    derive::{NoritoDeserialize, NoritoSerialize},
    json,
};

use crate::{
    JsonBody, NoritoJson, NoritoQuery,
    json_macros::{JsonDeserialize, JsonSerialize},
    routing::{MaybeTelemetry, parse_account_literal},
};

const CONTEXT_GOV_PROPOSE_DEPLOY_AUTHORITY: &str = "/v1/gov/proposals/deploy-contract#authority";
const CONTEXT_GOV_BALLOT_ZK_AUTHORITY: &str = "/v1/gov/ballots/zk#authority";
const CONTEXT_GOV_BALLOT_ZK_V1_AUTHORITY: &str = "/v1/gov/ballots/zk-v1#authority";
const CONTEXT_GOV_BALLOT_ZK_V1_BALLOT_PROOF_AUTHORITY: &str =
    "/v1/gov/ballots/zk-v1/ballot-proof#authority";
const CONTEXT_GOV_BALLOT_PLAIN_AUTHORITY: &str = "/v1/gov/ballots/plain#authority";
const CONTEXT_GOV_BALLOT_PLAIN_OWNER: &str = "/v1/gov/ballots/plain#owner";
const CONTEXT_GOV_FINALIZE_AUTHORITY: &str = "/v1/gov/finalize#authority";
const CONTEXT_GOV_ENACT_AUTHORITY: &str = "/v1/gov/enact#authority";

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

use std::sync::Arc;

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
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
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
/// Request body for proposing deployment of IVM bytecode via governance.
///
/// All hashes are 32-byte hex, with or without `0x` prefix.
pub struct ProposeDeployContractDto {
    /// Contract identifier (namespace-qualified)
    pub contract_id: String,
    /// Namespace for governance gating
    pub namespace: String,
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
    /// Optional authority (AccountId string) to submit the proposal directly.
    #[norito(default)]
    pub authority: Option<String>,
    /// Optional private key (multihash string) to sign and submit the proposal.
    #[norito(default)]
    pub private_key: Option<String>,
}

impl norito::core::NoritoSerialize for ProposeDeployContractDto {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
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
    /// Content-addressed proposal id (placeholder hex)
    pub proposal_id: String,
    /// Optional transaction skeleton for clients to sign and submit
    pub tx_instructions: Vec<TxInstr>,
}

#[derive(Debug, JsonDeserialize, JsonSerialize)]
/// Request body for submitting a zero-knowledge ballot.
pub struct ZkBallotDto {
    /// Authority submitting the ballot (AccountId string)
    pub authority: String,
    /// Chain id to build the transaction skeleton for
    pub chain_id: String,
    pub election_id: String,
    /// Base64-encoded proof bytes
    pub proof_b64: String,
    /// Public inputs (opaque for now)
    #[norito(default)]
    pub public: Option<norito::json::Value>,
    /// Optional private key to submit the ballot directly.
    #[norito(default)]
    pub private_key: Option<String>,
}

impl norito::core::NoritoSerialize for ZkBallotDto {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
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
/// Request body for submitting a plain (non-ZK) quadratic ballot.
pub struct PlainBallotDto {
    /// Authority casting the ballot (AccountId string)
    pub authority: String,
    /// Chain id to build the transaction skeleton for
    pub chain_id: String,
    pub referendum_id: String,
    pub owner: String,
    /// Token amount as decimal string to avoid JSON f64 issues
    pub amount: String,
    pub duration_blocks: u64,
    /// One of: "Aye" | "Nay" | "Abstain"
    pub direction: String,
    /// Optional private key to submit the ballot directly.
    #[norito(default)]
    pub private_key: Option<String>,
}

impl norito::core::NoritoSerialize for PlainBallotDto {
    fn serialize<W: std::io::Write>(&self, writer: W) -> Result<(), norito::core::Error> {
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
    normalize_zk_public_input_alias(map, "durationBlocks", "duration_blocks")?;
    normalize_zk_public_input_alias(map, "nullifierHex", "nullifier_hex")?;
    normalize_zk_public_input_alias(map, "rootHintHex", "root_hint")?;
    normalize_zk_public_input_alias(map, "rootHint", "root_hint")?;
    canonicalize_hex32_public_input(map, "root_hint", "root_hint")?;
    canonicalize_hex32_public_input(map, "nullifier_hex", "nullifier")?;
    Ok(())
}

fn normalize_zk_public_input_alias(
    map: &mut json::Map,
    alias: &str,
    canonical: &str,
) -> Result<(), String> {
    let Some(value) = map.remove(alias) else {
        return Ok(());
    };
    if map.contains_key(canonical) {
        return Err(format!(
            "public inputs cannot include both {alias} and {canonical}"
        ));
    }
    map.insert(canonical.to_string(), value);
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

// -------- ZK Ballot V1 DTO (feature-gated) --------
#[cfg(feature = "zk-ballot")]
#[derive(Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
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
    pub root_hint_hex: Option<String>,
    /// Optional owner account id (for lock hints when the circuit commits owner)
    #[norito(default)]
    pub owner: Option<String>,
    /// Optional lock amount hint (decimal string).
    #[norito(default)]
    pub amount: Option<String>,
    /// Optional lock duration hint in blocks.
    #[norito(default)]
    pub duration_blocks: Option<u64>,
    /// Optional direction hint ("Aye" | "Nay" | "Abstain").
    #[norito(default)]
    pub direction: Option<String>,
    /// Optional nullifier hint (hex-32, 0x allowed)
    #[norito(default)]
    pub nullifier_hex: Option<String>,
    /// Optional private key to submit the ballot directly.
    #[norito(default)]
    pub private_key: Option<String>,
}

#[cfg(feature = "zk-ballot")]
#[derive(Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
/// Request body that carries a BallotProof directly along with transaction context.
pub struct ZkBallotV1BallotProofDto {
    pub authority: String,
    pub chain_id: String,
    pub election_id: String,
    pub ballot: iroha_data_model::isi::governance::BallotProof,
    #[norito(default)]
    pub private_key: Option<String>,
}

#[cfg(feature = "zk-ballot")]
/// POST /v1/gov/ballots/zk-v1 — accept BallotProof-like DTO and build an instruction skeleton.
///
/// If `private_key` is provided, Torii signs and submits the ballot transaction.
///
/// # Errors
/// Returns `crate::Error::Query` for invalid chain id or authority. Invalid payloads are
/// reflected in the response body.
pub async fn handle_gov_ballot_zk_v1(
    chain_id: Arc<iroha_data_model::ChainId>,
    queue: Arc<iroha_core::queue::Queue>,
    state: Arc<iroha_core::state::State>,
    telemetry: MaybeTelemetry,
    strict_addresses: bool,
    NoritoJson(body): NoritoJson<ZkBallotV1Dto>,
) -> Result<JsonBody<BallotSubmitResponse>, crate::Error> {
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
    let authority_id = parse_authority_literal(
        body.authority.as_str(),
        &telemetry,
        CONTEXT_GOV_BALLOT_ZK_V1_AUTHORITY,
        strict_addresses,
    )?;
    let has_owner = body.owner.is_some();
    let has_amount = body.amount.is_some();
    let has_duration = body.duration_blocks.is_some();
    if lock_hints_incomplete(has_owner, has_amount, has_duration) {
        return Ok(ballot_rejection(
            "lock hints must include owner, amount, duration_blocks",
        ));
    }
    // Build public inputs JSON object with optional hints
    let mut pub_map = norito::json::Map::new();
    if let Some(rh) = &body.root_hint_hex {
        let Some(canonical) = canonicalize_hex32_value(rh) else {
            return Ok(ballot_rejection("root_hint must be 32-byte hex"));
        };
        pub_map.insert("root_hint".into(), norito::json::Value::from(canonical));
    }
    if let Some(owner) = &body.owner {
        pub_map.insert("owner".into(), norito::json::Value::from(owner.clone()));
    }
    if let Some(amount) = &body.amount {
        pub_map.insert("amount".into(), norito::json::Value::from(amount.clone()));
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
    if let Some(nullifier_hex) = &body.nullifier_hex {
        let Some(canonical) = canonicalize_hex32_value(nullifier_hex) else {
            return Ok(ballot_rejection("nullifier must be 32-byte hex"));
        };
        pub_map.insert("nullifier_hex".into(), norito::json::Value::from(canonical));
    }
    let public_inputs_json = norito::json::to_json(&norito::json::Value::Object(pub_map))
        .unwrap_or_else(|_| "{}".into());
    // Convert to CastZkBallot skeleton
    let instr = iroha_data_model::isi::governance::CastZkBallot {
        election_id: body.election_id,
        proof_b64: body.envelope_b64,
        public_inputs_json,
    };
    let boxed: iroha_data_model::isi::InstructionBox = instr.clone().into();
    let tx_instructions = vec![tx_instr_from_box(boxed)];
    let submitted = maybe_submit_with_authority(
        chain_id,
        queue,
        state,
        telemetry,
        &authority_id,
        body.private_key.as_deref(),
        core::iter::once(iroha_data_model::isi::InstructionBox::from(instr)),
        "/v1/gov/ballots/zk-v1",
    )
    .await?;
    Ok(JsonBody(BallotSubmitResponse {
        ok: true,
        accepted: true,
        reason: Some(
            if submitted {
                "submitted transaction"
            } else {
                "build transaction skeleton"
            }
            .to_string(),
        ),
        tx_instructions,
    }))
}

#[cfg(feature = "zk-ballot")]
/// POST /v1/gov/ballots/zk-v1/ballot-proof — accept BallotProof JSON and build instruction skeleton.
///
/// If `private_key` is provided, Torii signs and submits the ballot transaction.
///
/// # Errors
/// Returns `crate::Error::Query` for invalid chain id or authority. Malformed payloads are
/// reported via the response payload.
pub async fn handle_gov_ballot_zk_v1_ballotproof(
    chain_id: Arc<iroha_data_model::ChainId>,
    queue: Arc<iroha_core::queue::Queue>,
    state: Arc<iroha_core::state::State>,
    telemetry: MaybeTelemetry,
    strict_addresses: bool,
    NoritoJson(body): NoritoJson<ZkBallotV1BallotProofDto>,
) -> Result<JsonBody<BallotSubmitResponse>, crate::Error> {
    if body.ballot.envelope_bytes.is_empty() {
        return Ok(JsonBody(BallotSubmitResponse {
            ok: false,
            accepted: false,
            reason: Some("invalid proof envelope".to_string()),
            tx_instructions: Vec::new(),
        }));
    }
    ensure_chain_id_matches(chain_id.as_ref(), &body.chain_id)?;
    let authority_id = parse_authority_literal(
        body.authority.as_str(),
        &telemetry,
        CONTEXT_GOV_BALLOT_ZK_V1_BALLOT_PROOF_AUTHORITY,
        strict_addresses,
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
        pub_map.insert("amount".into(), norito::json::Value::from(amount.clone()));
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
            "nullifier_hex".into(),
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
    let boxed: iroha_data_model::isi::InstructionBox = instr.clone().into();
    let tx_instructions = vec![tx_instr_from_box(boxed)];
    let submitted = maybe_submit_with_authority(
        chain_id,
        queue,
        state,
        telemetry,
        &authority_id,
        body.private_key.as_deref(),
        core::iter::once(iroha_data_model::isi::InstructionBox::from(instr)),
        "/v1/gov/ballots/zk-v1/ballot-proof",
    )
    .await?;
    Ok(JsonBody(BallotSubmitResponse {
        ok: true,
        accepted: true,
        reason: Some(
            if submitted {
                "submitted transaction"
            } else {
                "build transaction skeleton"
            }
            .to_string(),
        ),
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
    pub verified: usize,
    pub derived_by: CouncilDerivationKind,
}

// --- VRF-backed council derivation (optional feature) ---
#[cfg(feature = "gov_vrf")]
#[derive(Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub struct VrfCandidateDto {
    pub account_id: String,
    /// Variant: "Normal" (pk in G1, sig in G2) or "Small" (pk in G2, sig in G1)
    pub variant: String,
    /// Public key bytes (base64, canonical compressed encoding)
    pub pk_b64: String,
    /// Proof/signature bytes (base64, canonical compressed encoding)
    pub proof_b64: String,
}

#[cfg(feature = "gov_vrf")]
#[derive(Debug)]
struct ParsedCandidate {
    account_id: iroha_data_model::account::AccountId,
    variant: CandidateVariant,
    pk: Vec<u8>,
    proof: Vec<u8>,
}

#[cfg(feature = "gov_vrf")]
fn parse_candidates(dtos: &[VrfCandidateDto]) -> Vec<ParsedCandidate> {
    let mut parsed = Vec::with_capacity(dtos.len());
    for dto in dtos {
        let Ok(account_id) = dto.account_id.parse() else {
            continue;
        };
        let Ok(variant) = CandidateVariant::from_str(dto.variant.as_str()) else {
            continue;
        };
        let Ok(pk) = base64::engine::general_purpose::STANDARD.decode(dto.pk_b64.as_bytes()) else {
            continue;
        };
        let Ok(proof) = base64::engine::general_purpose::STANDARD.decode(dto.proof_b64.as_bytes())
        else {
            continue;
        };
        parsed.push(ParsedCandidate {
            account_id,
            variant,
            pk,
            proof,
        });
    }
    parsed
}

#[cfg(feature = "gov_vrf")]
#[derive(Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
pub struct CouncilDeriveVrfRequest {
    /// Optional committee size override; defaults to governance config.
    #[norito(default)]
    pub committee_size: Option<usize>,
    /// Optional alternate pool size; defaults to governance config or committee size.
    #[norito(default)]
    pub alternate_size: Option<usize>,
    /// Optional epoch override; defaults to height/TERM_BLOCKS
    #[norito(default)]
    pub epoch: Option<u64>,
    pub candidates: Vec<VrfCandidateDto>,
}

#[cfg(feature = "gov_vrf")]
#[derive(Debug, JsonSerialize)]
pub struct CouncilDeriveVrfResponse {
    pub epoch: u64,
    pub members: Vec<CouncilMemberDto>,
    pub alternates: Vec<CouncilMemberDto>,
    pub total_candidates: usize,
    pub verified: usize,
    pub derived_by: CouncilDerivationKind,
}

#[cfg(not(feature = "gov_vrf"))]
#[derive(Debug, Copy, Clone, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
/// Placeholder request used when `gov_vrf` is disabled at compile time.
pub struct CouncilDeriveVrfRequest;

#[cfg(not(feature = "gov_vrf"))]
#[derive(Debug, Copy, Clone, JsonSerialize)]
/// Placeholder response used when `gov_vrf` is disabled at compile time.
pub struct CouncilDeriveVrfResponse;

#[cfg(feature = "gov_vrf")]
/// POST /v1/gov/council/derive-vrf — derive the council roster from VRF candidates.
///
/// # Errors
/// This handler never returns an error; invalid candidates are skipped from the roster.
pub async fn handle_gov_council_derive_vrf(
    state: Arc<iroha_core::state::State>,
    NoritoJson(body): NoritoJson<CouncilDeriveVrfRequest>,
) -> Result<JsonBody<CouncilDeriveVrfResponse>, crate::Error> {
    const TERM_BLOCKS: u64 = 43_200; // ~12h @1s
    let gov_cfg = state.gov.clone();
    let view = state.view();
    let height = view.height() as u64;
    let epoch = body.epoch.unwrap_or(height / TERM_BLOCKS);
    let chain_id = view.chain_id();
    let beacon_bytes: [u8; 32] = view
        .latest_block_hash()
        .map(|h| *h.as_ref())
        .unwrap_or([0u8; 32]);
    let committee_size = body
        .committee_size
        .unwrap_or(gov_cfg.parliament_committee_size);
    let alternate_size = body.alternate_size.unwrap_or_else(|| {
        gov_cfg
            .parliament_alternate_size
            .unwrap_or(committee_size)
            .max(committee_size)
    });

    let parsed = parse_candidates(&body.candidates);
    let candidate_refs = parsed.iter().map(|candidate| CandidateRef {
        account_id: &candidate.account_id,
        variant: candidate.variant,
        public_key: candidate.pk.as_slice(),
        proof: candidate.proof.as_slice(),
    });

    let draw = draw::run_draw(
        chain_id,
        epoch,
        &beacon_bytes,
        candidate_refs,
        committee_size,
        alternate_size,
    );

    let members = draw
        .members
        .iter()
        .map(|member| CouncilMemberDto {
            account_id: member.to_string(),
        })
        .collect();
    let alternates = draw
        .alternates
        .iter()
        .map(|member| CouncilMemberDto {
            account_id: member.to_string(),
        })
        .collect();
    let derived_by = if draw.verified > 0 {
        CouncilDerivationKind::Vrf
    } else {
        CouncilDerivationKind::Fallback
    };

    Ok(JsonBody(CouncilDeriveVrfResponse {
        epoch,
        members,
        alternates,
        total_candidates: parsed.len(),
        verified: draw.verified,
        derived_by,
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
    let v = state.view();
    let now_h = v.height() as u64;
    let mut expired_locks_now: u64 = 0;
    let mut refs_with_expired: u64 = 0;
    for (_rid, rec) in v.world().governance_locks().iter() {
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
    let last_sweep_height = *v.world().governance_last_unlock_sweep_height();
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

fn parse_authority_literal(
    raw: &str,
    telemetry: &MaybeTelemetry,
    context: &'static str,
    strict_addresses: bool,
) -> Result<iroha_data_model::account::AccountId, crate::Error> {
    parse_account_literal(raw, telemetry, context, strict_addresses)
        .map_err(|err| {
            crate::routing::conversion_error(format!("invalid authority: {}", err.reason()))
        })
        .map(iroha_data_model::account::ParsedAccountId::into_account_id)
}

fn parse_private_key_literal(raw: &str) -> Result<iroha_crypto::PrivateKey, crate::Error> {
    let trimmed = raw.trim();
    trimmed
        .parse::<iroha_crypto::PrivateKey>()
        .map_err(|err| crate::routing::conversion_error(format!("invalid private_key: {err}")))
}

#[allow(clippy::too_many_arguments)]
async fn submit_signed_instructions(
    chain_id: Arc<iroha_data_model::ChainId>,
    queue: Arc<iroha_core::queue::Queue>,
    state: Arc<iroha_core::state::State>,
    telemetry: MaybeTelemetry,
    authority: iroha_data_model::account::AccountId,
    private_key: iroha_crypto::PrivateKey,
    instructions: impl IntoIterator<Item = iroha_data_model::isi::InstructionBox>,
    endpoint: &'static str,
) -> Result<(), crate::Error> {
    use iroha_data_model::prelude as dm;

    let tx = dm::TransactionBuilder::new((*chain_id).clone(), authority)
        .with_instructions(instructions)
        .sign(&private_key);
    crate::routing::handle_transaction_with_metrics(
        chain_id, queue, state, tx, telemetry, endpoint,
    )
    .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn maybe_submit_with_authority(
    chain_id: Arc<iroha_data_model::ChainId>,
    queue: Arc<iroha_core::queue::Queue>,
    state: Arc<iroha_core::state::State>,
    telemetry: MaybeTelemetry,
    authority: &iroha_data_model::account::AccountId,
    private_key: Option<&str>,
    instructions: impl IntoIterator<Item = iroha_data_model::isi::InstructionBox>,
    endpoint: &'static str,
) -> Result<bool, crate::Error> {
    let Some(private_key) = private_key else {
        return Ok(false);
    };
    let private_key = parse_private_key_literal(private_key)?;
    submit_signed_instructions(
        chain_id,
        queue,
        state,
        telemetry,
        authority.clone(),
        private_key,
        instructions,
        endpoint,
    )
    .await?;
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
async fn maybe_submit_optional_signer(
    chain_id: Arc<iroha_data_model::ChainId>,
    queue: Arc<iroha_core::queue::Queue>,
    state: Arc<iroha_core::state::State>,
    telemetry: MaybeTelemetry,
    authority: Option<&str>,
    private_key: Option<&str>,
    authority_context: &'static str,
    strict_addresses: bool,
    instructions: impl IntoIterator<Item = iroha_data_model::isi::InstructionBox>,
    endpoint: &'static str,
) -> Result<bool, crate::Error> {
    match (authority, private_key) {
        (None, None) => Ok(false),
        (Some(_), None) | (None, Some(_)) => Err(crate::routing::conversion_error(
            "authority and private_key must be provided together".into(),
        )),
        (Some(authority), Some(private_key)) => {
            let authority_id = parse_authority_literal(
                authority,
                &telemetry,
                authority_context,
                strict_addresses,
            )?;
            let private_key = parse_private_key_literal(private_key)?;
            submit_signed_instructions(
                chain_id,
                queue,
                state,
                telemetry,
                authority_id,
                private_key,
                instructions,
                endpoint,
            )
            .await?;
            Ok(true)
        }
    }
}

fn instruction_skeleton_for_propose(
    instr: &iroha_data_model::isi::governance::ProposeDeployContract,
) -> Vec<TxInstr> {
    let boxed: iroha_data_model::isi::InstructionBox = instr.clone().into();
    vec![tx_instr_from_box(boxed)]
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
    namespace: &str,
    contract_id: &str,
    code_hash: &[u8; 32],
    abi_hash: &[u8; 32],
) -> [u8; 32] {
    use iroha_crypto::blake2::{Blake2b512, digest::Digest};

    let namespace_len = namespace.len() as u32;
    let contract_len = contract_id.len() as u32;
    let mut input = Vec::with_capacity(
        b"iroha:gov:proposal:v1|".len()
            + core::mem::size_of::<u32>() * 2
            + namespace.len()
            + contract_id.len()
            + code_hash.len()
            + abi_hash.len(),
    );
    input.extend_from_slice(b"iroha:gov:proposal:v1|");
    input.extend_from_slice(&namespace_len.to_le_bytes());
    input.extend_from_slice(namespace.as_bytes());
    input.extend_from_slice(&contract_len.to_le_bytes());
    input.extend_from_slice(contract_id.as_bytes());
    input.extend_from_slice(code_hash);
    input.extend_from_slice(abi_hash);
    let digest = Blake2b512::digest(&input);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    out
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
    let v = state.view();
    let found = v.world().governance_proposals().get(&id_arr).cloned();
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
    let v = state.view();
    let found = v.world().governance_locks().get(&ref_id).cloned();
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
    let v = state.view();
    let found = v.world().governance_referenda().get(&rid).copied();
    Ok(JsonBody(ReferendumGetResponse {
        found: found.is_some(),
        referendum: found,
    }))
}

/// Handler for computing a referendum tally summary.
///
/// # Errors
/// This handler never returns an error; missing records result in zeroed tallies.
pub async fn handle_gov_get_tally(
    state: Arc<iroha_core::state::State>,
    id: axum::extract::Path<String>,
) -> Result<JsonBody<TallyGetResponse>, crate::Error> {
    let rid = id.0;
    let v = state.view();
    // Mirror FinalizeReferendum tally logic without mutating state.
    let now_h = v.height() as u64;
    let mut approve: u128 = 0;
    let mut reject: u128 = 0;
    let mut abstain: u128 = 0;
    let mode = v
        .world()
        .governance_referenda()
        .get(&rid)
        .map(|rec| rec.mode)
        .unwrap_or(iroha_core::state::GovernanceReferendumMode::Plain);
    match mode {
        iroha_core::state::GovernanceReferendumMode::Plain => {
            if let Some(locks) = v.world().governance_locks().get(&rid) {
                let step = v.gov.conviction_step_blocks.max(1);
                let max_c = v.gov.max_conviction;
                for (_owner, rec) in locks.locks.iter() {
                    if rec.expiry_height < now_h {
                        continue;
                    }
                    let base = integer_sqrt_u128(rec.amount);
                    let mut f = 1u64 + (rec.duration_blocks / step);
                    if f > max_c {
                        f = max_c;
                    }
                    let w = base.saturating_mul(u128::from(f));
                    match rec.direction {
                        0 => approve = approve.saturating_add(w),
                        1 => reject = reject.saturating_add(w),
                        _ => abstain = abstain.saturating_add(w),
                    }
                }
            }
        }
        iroha_core::state::GovernanceReferendumMode::Zk => {
            if let Some(e) = v.world().elections().get(&rid) {
                if e.finalized && e.tally.len() >= 2 {
                    approve = e.tally[0] as u128;
                    reject = e.tally[1] as u128;
                }
            }
        }
    }
    Ok(JsonBody(TallyGetResponse {
        referendum_id: rid,
        approve,
        reject,
        abstain,
    }))
}

fn integer_sqrt_u128(n: u128) -> u128 {
    if n == 0 {
        return 0;
    }
    let mut x0 = n;
    let mut x1 = (x0 + n / x0) / 2;
    while x1 < x0 {
        x0 = x1;
        x1 = (x0 + n / x0) / 2;
    }
    x0
}

#[derive(Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
/// Request body for finalizing a referendum
pub struct FinalizeDto {
    /// Referendum identifier
    pub referendum_id: String,
    /// Proposal id (hex 64)
    pub proposal_id: String,
    /// Optional authority (AccountId string) to submit the finalize transaction.
    #[norito(default)]
    pub authority: Option<String>,
    /// Optional private key (multihash string) to sign and submit.
    #[norito(default)]
    pub private_key: Option<String>,
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
/// If `authority` and `private_key` are provided, Torii submits the transaction.
///
/// # Errors
/// Returns `crate::Error::Query` when `proposal_id` is not 32-byte hex.
pub async fn handle_gov_finalize(
    chain_id: Arc<iroha_data_model::ChainId>,
    queue: Arc<iroha_core::queue::Queue>,
    state: Arc<iroha_core::state::State>,
    telemetry: MaybeTelemetry,
    strict_addresses: bool,
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
    let _submitted = maybe_submit_optional_signer(
        chain_id,
        queue,
        state,
        telemetry,
        body.authority.as_deref(),
        body.private_key.as_deref(),
        CONTEXT_GOV_FINALIZE_AUTHORITY,
        strict_addresses,
        core::iter::once(iroha_data_model::isi::InstructionBox::from(instr)),
        iroha_torii_shared::uri::GOV_FINALIZE,
    )
    .await?;
    Ok(JsonBody(FinalizeResponse {
        ok: true,
        tx_instructions,
    }))
}

#[derive(Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
/// Request body for enacting an approved referendum.
pub struct EnactDto {
    /// Proposal id (hex 64)
    pub proposal_id: String,
    /// Optional preimage hash for auditability (hex 64); defaults to zeros
    #[norito(default)]
    pub preimage_hash: Option<String>,
    /// Optional enactment window to encode in the instruction
    #[norito(default)]
    pub window: Option<AtWindowDto>,
    /// Optional authority (AccountId string) to submit the enact transaction.
    #[norito(default)]
    pub authority: Option<String>,
    /// Optional private key (multihash string) to sign and submit.
    #[norito(default)]
    pub private_key: Option<String>,
}

#[derive(Debug, JsonSerialize)]
/// Response for enactment draft transaction.
pub struct EnactResponse {
    pub ok: bool,
    pub tx_instructions: Vec<TxInstr>,
}

/// Handler for building an enactment transaction (draft only).
///
/// If `authority` and `private_key` are provided, Torii submits the transaction.
///
/// # Errors
/// Returns `crate::Error::Query` when the proposal or preimage hashes are not 32-byte hex strings.
pub async fn handle_gov_enact(
    chain_id: Arc<iroha_data_model::ChainId>,
    queue: Arc<iroha_core::queue::Queue>,
    state: Arc<iroha_core::state::State>,
    telemetry: MaybeTelemetry,
    strict_addresses: bool,
    NoritoJson(body): NoritoJson<EnactDto>,
) -> Result<JsonBody<EnactResponse>, crate::Error> {
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
    let mut pid = [0u8; 32];
    pid.copy_from_slice(&bytes);
    // Parse preimage hash or default to zeros
    let mut preimage = [0u8; 32];
    if let Some(h) = body.preimage_hash.as_deref() {
        let b = hex::decode(h.trim_start_matches("0x")).map_err(|_| {
            crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "invalid preimage_hash".into(),
                ),
            ))
        })?;
        if b.len() != 32 {
            return Err(crate::Error::Query(
                iroha_data_model::ValidationFail::QueryFailed(
                    iroha_data_model::query::error::QueryExecutionFail::Conversion(
                        "invalid preimage_hash length".into(),
                    ),
                ),
            ));
        }
        preimage.copy_from_slice(&b);
    }
    // Build instruction
    let instr = iroha_data_model::isi::governance::EnactReferendum {
        referendum_id: pid,
        preimage_hash: preimage,
        at_window: body
            .window
            .map(|w| iroha_data_model::governance::types::AtWindow {
                lower: w.lower,
                upper: w.upper,
            })
            .unwrap_or(iroha_data_model::governance::types::AtWindow { lower: 0, upper: 0 }),
    };
    let boxed: iroha_data_model::isi::InstructionBox = instr.clone().into();
    let tx_instructions = vec![tx_instr_from_box(boxed)];
    let _submitted = maybe_submit_optional_signer(
        chain_id,
        queue,
        state,
        telemetry,
        body.authority.as_deref(),
        body.private_key.as_deref(),
        CONTEXT_GOV_ENACT_AUTHORITY,
        strict_addresses,
        core::iter::once(iroha_data_model::isi::InstructionBox::from(instr)),
        iroha_torii_shared::uri::GOV_ENACT,
    )
    .await?;
    Ok(JsonBody(EnactResponse {
        ok: true,
        tx_instructions,
    }))
}

#[derive(Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
/// Request body for applying protected namespaces parameter.
pub struct ProtectedNamespacesDto {
    /// Namespaces to protect (e.g., `["apps", "system"]`).
    pub namespaces: Vec<String>,
}

#[derive(Debug, JsonSerialize, Clone, Copy)]
/// Response to applying the protected namespaces parameter.
/// Contains a success flag and the number of namespaces applied.
pub struct ProtectedNamespacesApplyResponse {
    pub ok: bool,
    pub applied: usize,
}

/// POST /v1/gov/protected-namespaces — apply the custom parameter directly.
/// Requires API token (if configured) and rate-limit key.
///
/// # Errors
/// Returns `crate::Error::Query` when the namespaces cannot be serialized into the custom
/// parameter or when executing the synthetic `SetParameter` fails.
pub async fn handle_gov_protected_set(
    state: Arc<iroha_core::state::State>,
    NoritoJson(body): NoritoJson<ProtectedNamespacesDto>,
) -> Result<JsonBody<ProtectedNamespacesApplyResponse>, crate::Error> {
    // Build SetParameter(Custom) and execute inside a synthetic block
    // note: using fully qualified paths below; no prelude/nonzero macros needed
    use std::str::FromStr as _;

    use iroha_data_model::parameter::{CustomParameterId, Parameter, custom::CustomParameter};

    // Validate namespace strings are non-empty ASCII (basic check)
    let filtered: Vec<String> = body
        .namespaces
        .into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    let applied = filtered.len();
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

    // Create a minimal block header: height = current+1
    let curr_h = state.view().height() as u64;
    let header = iroha_data_model::block::BlockHeader::new(
        core::num::NonZeroU64::new(curr_h + 1).unwrap(),
        None,
        None,
        None,
        0,
        0,
    );
    let mut sblock = state.block(header);
    let mut stx = sblock.transaction();
    // Authority is not used by SetParameter currently
    let dummy_auth: iroha_data_model::account::AccountId =
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
            .parse()
            .unwrap_or_else(|_| {
                // Fallback: construct a synthetic account id with zero key
                use iroha_crypto::KeyPair;
                let kp = KeyPair::random();
                iroha_data_model::account::AccountId::of(
                    "wonderland".parse().unwrap(),
                    kp.public_key().clone(),
                )
            });
    isi.execute(&dummy_auth, &mut stx).map_err(|e| {
        crate::Error::Query(iroha_data_model::ValidationFail::InternalError(
            e.to_string(),
        ))
    })?;
    stx.apply();
    // We do not commit a block here; parameter is applied at transaction granularity.

    Ok(JsonBody(ProtectedNamespacesApplyResponse {
        ok: true,
        applied,
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
    let v = state.view();
    let params = v.world().parameters();
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

#[derive(Debug, JsonSerialize, Clone)]
/// Contract instance descriptor
pub struct InstanceDto {
    /// Contract id (namespace-qualified)
    pub contract_id: String,
    /// Code hash in hex
    pub code_hash_hex: String,
}

#[derive(Debug, JsonSerialize)]
/// Response for listing instances by namespace
pub struct InstancesByNamespaceResponse {
    /// The queried namespace
    pub namespace: String,
    /// Matching instances
    pub instances: Vec<InstanceDto>,
    /// Total number of matches
    pub total: usize,
    /// Page offset
    pub offset: u32,
    /// Page limit
    pub limit: u32,
}

/// GET /v1/gov/instances/{ns} — lists active contract instances for a namespace.
///
/// # Errors
/// This handler never returns an error; filters and pagination only affect the response content.
pub async fn handle_gov_instances_by_ns(
    state: Arc<iroha_core::state::State>,
    ns: axum::extract::Path<String>,
    NoritoQuery(q): NoritoQuery<InstancesQuery>,
) -> Result<JsonBody<InstancesByNamespaceResponse>, crate::Error> {
    let namespace = ns.0;
    let v = state.view();
    let mut out: Vec<InstanceDto> = v
        .world()
        .contract_instances()
        .iter()
        .filter_map(|((ns_key, cid), h)| {
            if ns_key == &namespace {
                let bytes: [u8; 32] = (*h).into();
                Some(InstanceDto {
                    contract_id: cid.clone(),
                    code_hash_hex: hex::encode(bytes),
                })
            } else {
                None
            }
        })
        .collect();

    // Filters
    if let Some(s) = q.contains.as_deref() {
        out.retain(|x| x.contract_id.contains(s));
    }
    if let Some(pref) = q.hash_prefix.as_deref() {
        let pref_l = pref.to_ascii_lowercase();
        out.retain(|x| x.code_hash_hex.starts_with(&pref_l));
    }

    // Sort
    match q.order.as_deref() {
        Some("cid_desc") => out.sort_by(|a, b| b.contract_id.cmp(&a.contract_id)),
        Some("hash_asc") => out.sort_by(|a, b| a.code_hash_hex.cmp(&b.code_hash_hex)),
        Some("hash_desc") => out.sort_by(|a, b| b.code_hash_hex.cmp(&a.code_hash_hex)),
        _ => out.sort_by(|a, b| a.contract_id.cmp(&b.contract_id)), // cid_asc default
    }

    // Pagination
    let total = out.len();
    let offset = q.offset.unwrap_or(0);
    let limit = q.limit.unwrap_or(100).min(10_000);
    let start = offset as usize;
    let end = start.saturating_add(limit as usize).min(total);
    let view = if start < end {
        out[start..end].to_vec()
    } else {
        Vec::new()
    };
    Ok(JsonBody(InstancesByNamespaceResponse {
        namespace,
        instances: view,
        total,
        offset,
        limit,
    }))
}

#[derive(Debug, Default, JsonDeserialize)]
/// Optional filters for listing contract instances by namespace.
///
/// All fields are optional; pagination defaults to `offset = 0`, `limit = 100`.
pub struct InstancesQuery {
    /// Filter: contract_id contains substring (case-sensitive)
    pub contains: Option<String>,
    /// Filter: code_hash hex prefix (lowercase)
    pub hash_prefix: Option<String>,
    /// Pagination offset (default 0)
    pub offset: Option<u32>,
    /// Pagination limit (default 100, max 10_000)
    pub limit: Option<u32>,
    /// Order: one of cid_asc (default), cid_desc, hash_asc, hash_desc
    pub order: Option<String>,
}

/// POST /v1/gov/propose-deploy — build a proposal id and instruction skeleton.
///
/// If `authority` and `private_key` are provided, Torii signs and submits the
/// transaction before returning the draft instructions.
///
/// # Errors
/// Returns `crate::Error::Query` when the namespace, contract id, hashes, ABI version, or request
/// options fail validation (e.g., malformed hex or unsupported voting mode).
pub async fn handle_gov_propose_deploy(
    chain_id: Arc<iroha_data_model::ChainId>,
    queue: Arc<iroha_core::queue::Queue>,
    state: Arc<iroha_core::state::State>,
    telemetry: MaybeTelemetry,
    strict_addresses: bool,
    NoritoJson(body): NoritoJson<ProposeDeployContractDto>,
) -> Result<JsonBody<ProposeDeployContractResponse>, crate::Error> {
    use iroha_data_model::isi::governance as gov;

    let namespace = body.namespace.trim();
    if namespace.is_empty() {
        return Err(crate::Error::Query(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "namespace must not be empty".into(),
                ),
            ),
        ));
    }
    if namespace.len() > u32::MAX as usize {
        return Err(crate::Error::Query(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "namespace length exceeds 2^32 bytes".into(),
                ),
            ),
        ));
    }
    let contract_id = body.contract_id.trim();
    if contract_id.is_empty() {
        return Err(crate::Error::Query(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "contract_id must not be empty".into(),
                ),
            ),
        ));
    }
    if contract_id.len() > u32::MAX as usize {
        return Err(crate::Error::Query(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "contract_id length exceeds 2^32 bytes".into(),
                ),
            ),
        ));
    }

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
        namespace: namespace.to_string(),
        contract_id: contract_id.to_string(),
        code_hash_hex,
        abi_hash_hex,
        abi_version: abi_version.to_string(),
        window,
        mode,
        manifest_provenance: body.manifest_provenance.clone(),
    };

    let proposal_id_bytes = compute_proposal_id(
        &instr.namespace,
        &instr.contract_id,
        &code_hash_bytes,
        &abi_hash_bytes,
    );
    let proposal_id = hex::encode(proposal_id_bytes);
    let _submitted = maybe_submit_optional_signer(
        chain_id,
        queue,
        state,
        telemetry,
        body.authority.as_deref(),
        body.private_key.as_deref(),
        CONTEXT_GOV_PROPOSE_DEPLOY_AUTHORITY,
        strict_addresses,
        core::iter::once(iroha_data_model::isi::InstructionBox::from(instr.clone())),
        iroha_torii_shared::uri::GOV_PROPOSE_DEPLOY,
    )
    .await?;

    Ok(JsonBody(ProposeDeployContractResponse {
        ok: true,
        proposal_id,
        tx_instructions: instruction_skeleton_for_propose(&instr),
    }))
}

/// POST /v1/gov/ballot/zk — accept a ZK ballot and build an instruction skeleton.
///
/// If `private_key` is provided, Torii signs and submits the ballot transaction.
///
/// # Errors
/// Returns `crate::Error::Query` for invalid chain id or authority. Invalid proofs result in an
/// `ok = false` response.
pub async fn handle_gov_ballot_zk(
    chain_id: Arc<iroha_data_model::ChainId>,
    queue: Arc<iroha_core::queue::Queue>,
    state: Arc<iroha_core::state::State>,
    telemetry: MaybeTelemetry,
    strict_addresses: bool,
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
        private_key,
    } = body;
    ensure_chain_id_matches(chain_id.as_ref(), &body_chain_id)?;
    let authority_id = parse_authority_literal(
        authority.as_str(),
        &telemetry,
        CONTEXT_GOV_BALLOT_ZK_AUTHORITY,
        strict_addresses,
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
    let boxed: iroha_data_model::isi::InstructionBox = instr.clone().into();
    let tx_instructions = vec![tx_instr_from_box(boxed)];
    let submitted = maybe_submit_with_authority(
        chain_id,
        queue,
        state,
        telemetry,
        &authority_id,
        private_key.as_deref(),
        core::iter::once(iroha_data_model::isi::InstructionBox::from(instr)),
        iroha_torii_shared::uri::GOV_BALLOT_ZK,
    )
    .await?;
    Ok(JsonBody(BallotSubmitResponse {
        ok: true,
        accepted: true,
        reason: Some(
            if submitted {
                "submitted transaction"
            } else {
                "build transaction skeleton"
            }
            .to_string(),
        ),
        tx_instructions,
    }))
}

/// POST /v1/gov/ballot/plain — accept a plain quadratic ballot and build an instruction skeleton.
///
/// If `private_key` is provided, Torii signs and submits the ballot transaction.
///
/// # Errors
/// Returns `crate::Error::Query` when the ballot fields fail validation (direction, authority,
/// owner, amount parsing, or chain id mismatch).
pub async fn handle_gov_ballot_plain(
    chain_id: Arc<iroha_data_model::ChainId>,
    queue: Arc<iroha_core::queue::Queue>,
    state: Arc<iroha_core::state::State>,
    NoritoJson(body): NoritoJson<PlainBallotDto>,
) -> Result<JsonBody<BallotSubmitResponse>, crate::Error> {
    handle_gov_ballot_plain_with_policy(
        chain_id,
        queue,
        state,
        NoritoJson(body),
        MaybeTelemetry::disabled(),
        false,
    )
    .await
}

/// Variant of [`handle_gov_ballot_plain`] that allows callers to inject telemetry/strict-address
/// policy, enabling roadmap ADDR-5 adoption across Torii and tests.
pub async fn handle_gov_ballot_plain_with_policy(
    chain_id: Arc<iroha_data_model::ChainId>,
    queue: Arc<iroha_core::queue::Queue>,
    state: Arc<iroha_core::state::State>,
    NoritoJson(body): NoritoJson<PlainBallotDto>,
    telemetry: MaybeTelemetry,
    strict_addresses: bool,
) -> Result<JsonBody<BallotSubmitResponse>, crate::Error> {
    ensure_chain_id_matches(chain_id.as_ref(), &body.chain_id)?;
    // Basic shape validations
    if !(body.direction == "Aye" || body.direction == "Nay" || body.direction == "Abstain") {
        return Err(crate::routing::conversion_error("invalid direction".into()));
    }
    // Parse authority and owner; require equality for plain ballots
    let authority_id = parse_account_literal(
        body.authority.as_str(),
        &telemetry,
        CONTEXT_GOV_BALLOT_PLAIN_AUTHORITY,
        strict_addresses,
    )
    .map_err(|err| {
        crate::routing::conversion_error(format!("invalid authority: {}", err.reason()))
    })?
    .into_account_id();
    let owner = parse_account_literal(
        body.owner.as_str(),
        &telemetry,
        CONTEXT_GOV_BALLOT_PLAIN_OWNER,
        strict_addresses,
    )
    .map_err(|err| crate::routing::conversion_error(format!("invalid owner: {}", err.reason())))?
    .into_account_id();
    if owner != authority_id {
        return Err(crate::routing::conversion_error(
            "authority must equal owner".into(),
        ));
    }
    let instr = iroha_data_model::isi::governance::CastPlainBallot {
        referendum_id: body.referendum_id,
        owner,
        amount: body
            .amount
            .parse::<u128>()
            .map_err(|_| crate::routing::conversion_error("invalid amount".into()))?,
        duration_blocks: body.duration_blocks,
        direction: match body.direction.as_str() {
            "Aye" => 0,
            "Nay" => 1,
            _ => 2,
        },
    };
    let boxed: iroha_data_model::isi::InstructionBox = instr.clone().into();
    let tx_instructions = vec![tx_instr_from_box(boxed)];
    let submitted = maybe_submit_with_authority(
        chain_id,
        queue,
        state,
        telemetry,
        &authority_id,
        body.private_key.as_deref(),
        core::iter::once(iroha_data_model::isi::InstructionBox::from(instr)),
        iroha_torii_shared::uri::GOV_BALLOT_PLAIN,
    )
    .await?;
    Ok(JsonBody(BallotSubmitResponse {
        ok: true,
        accepted: true,
        reason: Some(
            if submitted {
                "submitted transaction"
            } else {
                "build transaction skeleton"
            }
            .to_string(),
        ),
        tx_instructions,
    }))
}

/// GET /v1/gov/council/current — derive or fetch the current council membership.
///
/// # Errors
/// This handler never returns an error; empty councils are represented with an empty member list.
pub async fn handle_gov_council_current(
    state: Arc<iroha_core::state::State>,
) -> Result<JsonBody<CouncilCurrentResponse>, crate::Error> {
    // Return the latest persisted council when available; otherwise fall back to a
    // deterministic derivation that mirrors the VRF spec using configured defaults.
    let v = state.view();
    let gov_cfg = &state.gov;
    let mut last_epoch: Option<u64> = None;
    for (ep, _) in v.world().council().iter() {
        last_epoch = Some(last_epoch.map(|e| e.max(*ep)).unwrap_or(*ep));
    }
    if let Some(epoch) = last_epoch {
        if let Some(cs) = v.world().council().get(&epoch) {
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
                verified: cs.verified as usize,
                derived_by: cs.derived_by,
            }));
        }
    }
    // Fallback derivation (deterministic hash scores using configured parameters)
    let height = v.height() as u64;
    let term_blocks = gov_cfg.parliament_term_blocks.max(1);
    let epoch = height / term_blocks;
    let chain_id = v.chain_id().to_string();
    let beacon_bytes: [u8; 32] = v
        .latest_block_hash()
        .map(|h| *h.as_ref())
        .unwrap_or([0u8; 32]);
    use iroha_crypto::blake2::{Blake2b512, Digest as _};
    let mut hasher = Blake2b512::new();
    hasher.update(b"gov:parliament:seed:v1");
    hasher.update(chain_id.as_bytes());
    hasher.update(epoch.to_be_bytes());
    hasher.update(&beacon_bytes);
    let seed = hasher.finalize();

    // Eligibility: accounts with balance >= min_stake of the configured stake asset
    let stake_def = gov_cfg.parliament_eligibility_asset_id.clone();
    let min_stake = gov_cfg.parliament_min_stake;
    let min_stake_numeric = if min_stake == 0 {
        iroha_primitives::numeric::Numeric::zero()
    } else {
        iroha_primitives::numeric::Numeric::from_str(&min_stake.to_string())
            .unwrap_or_else(|_| iroha_primitives::numeric::Numeric::zero())
    };
    use std::collections::HashSet;
    let mut elig: HashSet<iroha_data_model::account::AccountId> = HashSet::new();
    for (asset_id, value) in v.world().assets().iter() {
        if asset_id.definition() == &stake_def {
            // Non-zero balance qualifies
            if **value >= min_stake_numeric {
                elig.insert(asset_id.account().clone());
            }
        }
    }
    // Score eligible accounts by hash(tag|seed|account_id_bytes)
    let mut scored: Vec<(Vec<u8>, iroha_data_model::account::AccountId)> = Vec::new();
    let mut encode_buf = Vec::new();
    for acct in elig.into_iter() {
        let mut h = Blake2b512::new();
        h.update(b"iroha:vrf:v1:parliament|");
        h.update(&seed);
        encode_buf.clear();
        // Hash the canonical Norito encoding so the score is stable across
        // representations and matches persisted council entries.
        acct.encode_to(&mut encode_buf);
        h.update(&encode_buf);
        let out = h.finalize();
        scored.push((out.to_vec(), acct));
    }
    // Sort by score desc, tie-break by account string asc
    scored.sort_by(|a, b| {
        use std::cmp::Ordering;
        let ord = b.0.cmp(&a.0);
        if ord != Ordering::Equal {
            ord
        } else {
            a.1.to_string().cmp(&b.1.to_string())
        }
    });
    let mut roster: Vec<CouncilMemberDto> = scored
        .into_iter()
        .map(|(_, a)| CouncilMemberDto {
            account_id: a.to_string(),
        })
        .collect::<Vec<_>>();
    let total_candidates = roster.len();
    let committee_size = gov_cfg.parliament_committee_size;
    let alternate_size = gov_cfg
        .parliament_alternate_size
        .unwrap_or(committee_size)
        .max(committee_size);
    let alternates = if roster.len() > committee_size {
        let rest = roster.split_off(committee_size.min(roster.len()));
        rest.into_iter().take(alternate_size).collect()
    } else {
        Vec::new()
    };
    let members = roster;
    Ok(JsonBody(CouncilCurrentResponse {
        epoch,
        members,
        alternates,
        candidate_count: total_candidates,
        verified: 0,
        derived_by: CouncilDerivationKind::Fallback,
    }))
}

#[cfg(feature = "gov_vrf")]
#[derive(Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
/// Request to persist a VRF-derived council for the given epoch.
pub struct CouncilPersistRequest {
    /// Optional committee size to select (top-k by VRF output); defaults to governance config.
    #[norito(default)]
    pub committee_size: Option<usize>,
    /// Optional number of alternates to keep; defaults to governance config.
    #[norito(default)]
    pub alternate_size: Option<usize>,
    /// Optional epoch override; defaults to height/TERM_BLOCKS
    #[norito(default)]
    pub epoch: Option<u64>,
    /// Candidate set with VRF proofs
    pub candidates: Vec<VrfCandidateDto>,
    /// Optional authority to submit an on-chain transaction (preferred)
    #[norito(default)]
    pub authority: Option<String>,
    /// Optional private key (hex) for signing the on-chain transaction
    #[norito(default)]
    pub private_key: Option<String>,
}

#[cfg(feature = "gov_vrf")]
#[derive(Debug, JsonSerialize)]
/// Response for council persistence, mirroring derive-vrf summary with epoch.
pub struct CouncilPersistResponse {
    pub epoch: u64,
    pub members: Vec<CouncilMemberDto>,
    pub alternates: Vec<CouncilMemberDto>,
    pub total_candidates: usize,
    pub verified: usize,
    pub derived_by: CouncilDerivationKind,
}

#[cfg(feature = "gov_vrf")]
#[derive(Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
/// Request to replace a council member with the next available alternate.
pub struct CouncilReplaceRequest {
    /// Account id of the member to replace.
    pub missing: String,
    /// Optional epoch override; defaults to the latest persisted epoch.
    #[norito(default)]
    pub epoch: Option<u64>,
    /// Optional authority to submit an on-chain transaction (preferred)
    #[norito(default)]
    pub authority: Option<String>,
    /// Optional private key (hex) for signing the on-chain transaction
    #[norito(default)]
    pub private_key: Option<String>,
}

#[cfg(feature = "gov_vrf")]
#[derive(Debug, JsonSerialize)]
/// Response emitted after a council replacement attempt.
pub struct CouncilReplaceResponse {
    pub epoch: u64,
    pub members: Vec<CouncilMemberDto>,
    pub alternates: Vec<CouncilMemberDto>,
    pub replaced: bool,
}

/// Persist a VRF-derived council for the current epoch.
///
/// # Errors
/// Returns `crate::Error::Query` when requester credentials are invalid or missing, or when routing
/// the signed transaction fails.
#[cfg(feature = "gov_vrf")]
pub async fn handle_gov_council_persist(
    chain_id: Arc<iroha_data_model::ChainId>,
    queue: Arc<iroha_core::queue::Queue>,
    state: Arc<iroha_core::state::State>,
    telemetry: crate::routing::MaybeTelemetry,
    NoritoJson(body): NoritoJson<CouncilPersistRequest>,
) -> Result<JsonBody<CouncilPersistResponse>, crate::Error> {
    const TERM_BLOCKS: u64 = 43_200; // ~12h @1s
    let gov_cfg = state.gov.clone();
    let view = state.view();
    let height = view.height() as u64;
    let epoch = body.epoch.unwrap_or(height / TERM_BLOCKS);
    let chain_id_ref = view.chain_id();
    let beacon_bytes: [u8; 32] = view
        .latest_block_hash()
        .map(|h| *h.as_ref())
        .unwrap_or([0u8; 32]);
    let committee_size = body
        .committee_size
        .unwrap_or(gov_cfg.parliament_committee_size);
    let alternate_size = body.alternate_size.unwrap_or_else(|| {
        gov_cfg
            .parliament_alternate_size
            .unwrap_or(committee_size)
            .max(committee_size)
    });

    let parsed = parse_candidates(&body.candidates);
    let candidate_refs = parsed.iter().map(|candidate| CandidateRef {
        account_id: &candidate.account_id,
        variant: candidate.variant,
        public_key: candidate.pk.as_slice(),
        proof: candidate.proof.as_slice(),
    });

    let draw = draw::run_draw(
        chain_id_ref,
        epoch,
        &beacon_bytes,
        candidate_refs,
        committee_size,
        alternate_size,
    );
    let members: Vec<iroha_data_model::account::AccountId> = draw.members.clone();
    let alternates: Vec<iroha_data_model::account::AccountId> = draw.alternates.clone();
    let derived_by = if draw.verified > 0 {
        CouncilDerivationKind::Vrf
    } else {
        CouncilDerivationKind::Fallback
    };

    let instr = iroha_data_model::isi::governance::PersistCouncilForEpoch {
        epoch,
        members: members.clone(),
        alternates: alternates.clone(),
        verified: u32::try_from(draw.verified).unwrap_or(u32::MAX),
        candidates_count: u32::try_from(parsed.len()).unwrap_or(u32::MAX),
        derived_by,
    };

    if let (Some(authority), Some(private_key)) =
        (body.authority.as_deref(), body.private_key.as_deref())
    {
        // Preferred path: submit signed transaction
        use iroha_data_model::prelude as dm;
        let authority: iroha_data_model::account::AccountId = authority.parse().map_err(|_| {
            crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "invalid authority".into(),
                ),
            ))
        })?;
        let pk: iroha_crypto::PrivateKey = private_key.parse().map_err(|_| {
            crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "invalid private_key".into(),
                ),
            ))
        })?;
        let tx = dm::TransactionBuilder::new((*chain_id).clone(), authority)
            .with_instructions(core::iter::once(dm::InstructionBox::from(instr)))
            .sign(&pk);
        crate::routing::handle_transaction_with_metrics(
            chain_id,
            queue,
            state,
            tx,
            telemetry,
            "/v1/gov/council/persist",
        )
        .await?;
    } else {
        // Fallback: direct WSV mutation (admin/testing only)
        #[cfg(feature = "council_direct_wsv")]
        {
            let mut block = state.block_and_revert(iroha_core::block::BlockHeader::new(
                nonzero_ext::nonzero!(v.height().max(1) as u64),
                None,
                None,
                None,
                0,
                0,
            ));
            let mut stx = block.transaction();
            stx.world.council.insert(
                epoch,
                iroha_core::state::CouncilState {
                    epoch,
                    members: members.clone(),
                    alternates: alternates.clone(),
                    verified: instr.verified,
                    candidate_count: instr.candidates_count,
                    derived_by: instr.derived_by,
                },
            );
            stx.apply();
        }
        #[cfg(not(feature = "council_direct_wsv"))]
        {
            return Err(crate::Error::Query(
                iroha_data_model::ValidationFail::QueryFailed(
                    iroha_data_model::query::error::QueryExecutionFail::Conversion(
                        "authority/private_key required for on-chain persistence".into(),
                    ),
                ),
            ));
        }
    }

    Ok(JsonBody(CouncilPersistResponse {
        epoch,
        members: members
            .into_iter()
            .map(|a| CouncilMemberDto {
                account_id: a.to_string(),
            })
            .collect(),
        alternates: alternates
            .into_iter()
            .map(|a| CouncilMemberDto {
                account_id: a.to_string(),
            })
            .collect(),
        total_candidates: parsed.len(),
        verified: draw.verified,
        derived_by,
    }))
}

/// Replace a council member using the next available alternate and persist the updated roster.
#[cfg(feature = "gov_vrf")]
pub async fn handle_gov_council_replace(
    chain_id: Arc<iroha_data_model::ChainId>,
    queue: Arc<iroha_core::queue::Queue>,
    state: Arc<iroha_core::state::State>,
    telemetry: crate::routing::MaybeTelemetry,
    NoritoJson(body): NoritoJson<CouncilReplaceRequest>,
) -> Result<JsonBody<CouncilReplaceResponse>, crate::Error> {
    let v = state.view();
    let target_epoch = if let Some(epoch) = body.epoch {
        epoch
    } else {
        v.world()
            .council()
            .iter()
            .map(|(ep, _)| *ep)
            .max()
            .unwrap_or(0)
    };
    let mut term = v
        .world()
        .council()
        .get(&target_epoch)
        .cloned()
        .ok_or_else(|| {
            crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "no persisted council for epoch".into(),
                ),
            ))
        })?;

    let missing: iroha_data_model::account::AccountId = body.missing.parse().map_err(|_| {
        crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
            iroha_data_model::query::error::QueryExecutionFail::Conversion(
                "invalid missing account id".into(),
            ),
        ))
    })?;

    if !term.replace_member(&missing) {
        return Err(crate::Error::Query(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "member not found or no alternates available".into(),
                ),
            ),
        ));
    }

    let instr = iroha_data_model::isi::governance::PersistCouncilForEpoch {
        epoch: term.epoch,
        members: term.members.clone(),
        alternates: term.alternates.clone(),
        verified: term.verified,
        candidates_count: term.candidate_count,
        derived_by: term.derived_by,
    };

    if let (Some(authority), Some(private_key)) =
        (body.authority.as_deref(), body.private_key.as_deref())
    {
        use iroha_data_model::prelude as dm;
        let authority: iroha_data_model::account::AccountId = authority.parse().map_err(|_| {
            crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "invalid authority".into(),
                ),
            ))
        })?;
        let pk: iroha_crypto::PrivateKey = private_key.parse().map_err(|_| {
            crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(
                    "invalid private_key".into(),
                ),
            ))
        })?;
        let tx = dm::TransactionBuilder::new((*chain_id).clone(), authority)
            .with_instructions(core::iter::once(dm::InstructionBox::from(instr)))
            .sign(&pk);
        crate::routing::handle_transaction_with_metrics(
            chain_id,
            queue,
            state,
            tx,
            telemetry,
            "/v1/gov/council/replace",
        )
        .await?;
    } else {
        #[cfg(feature = "council_direct_wsv")]
        {
            let mut block = state.block_and_revert(iroha_core::block::BlockHeader::new(
                nonzero_ext::nonzero!(v.height().max(1) as u64),
                None,
                None,
                None,
                0,
                0,
            ));
            let mut stx = block.transaction();
            stx.world.council.insert(target_epoch, term.clone());
            stx.apply();
        }
        #[cfg(not(feature = "council_direct_wsv"))]
        {
            return Err(crate::Error::Query(
                iroha_data_model::ValidationFail::QueryFailed(
                    iroha_data_model::query::error::QueryExecutionFail::Conversion(
                        "authority/private_key required for on-chain persistence".into(),
                    ),
                ),
            ));
        }
    }

    Ok(JsonBody(CouncilReplaceResponse {
        epoch: term.epoch,
        members: term
            .members
            .into_iter()
            .map(|a| CouncilMemberDto {
                account_id: a.to_string(),
            })
            .collect(),
        alternates: term
            .alternates
            .into_iter()
            .map(|a| CouncilMemberDto {
                account_id: a.to_string(),
            })
            .collect(),
        replaced: true,
    }))
}

#[derive(Debug, Default, JsonDeserialize)]
/// Optional query for council audit endpoint.
pub struct CouncilAuditQuery {
    /// Optional epoch override; defaults to height/TERM_BLOCKS
    #[norito(default)]
    pub epoch: Option<u64>,
}

#[derive(Debug, JsonSerialize)]
/// Council audit response (seed and epoch metadata).
pub struct CouncilAuditResponse {
    pub epoch: u64,
    pub seed_hex: String,
    /// Latest block hash used as VRF beacon (hex)
    pub beacon_hex: String,
    pub members_count: usize,
    /// Persisted candidate roster size (0 when not stored)
    pub candidate_count: usize,
    /// Persisted alternates count (0 when not stored)
    pub alternates_count: usize,
    /// Verified VRF proofs counted for this epoch (0 when using fallback)
    pub verified: usize,
    /// Derivation method for the persisted council (VRF vs fallback)
    pub derived_by: CouncilDerivationKind,
    /// Chain id for domain separation
    pub chain_id: String,
}

/// GET /v1/gov/council/audit — expose the seed/epoch used for council derivation and members count.
///
/// # Errors
/// This handler never returns an error; absent council data yields zero counts.
pub async fn handle_gov_council_audit(
    state: Arc<iroha_core::state::State>,
    NoritoQuery(q): NoritoQuery<CouncilAuditQuery>,
) -> Result<JsonBody<CouncilAuditResponse>, crate::Error> {
    const TERM_BLOCKS: u64 = 43_200; // ~12h @1s
    let v = state.view();
    let height = v.height() as u64;
    let epoch = q.epoch.unwrap_or(height / TERM_BLOCKS);
    let chain_id_ref = v.chain_id();
    let beacon_bytes: [u8; 32] = v
        .latest_block_hash()
        .map(|h| *h.as_ref())
        .unwrap_or([0u8; 32]);
    let seed = parliament::compute_seed(chain_id_ref, epoch, &beacon_bytes);
    let seed_hex = hex::encode(seed);
    let beacon_hex = hex::encode(beacon_bytes);
    let (members_count, candidate_count, alternates_count, verified, derived_by) = v
        .world()
        .council()
        .get(&epoch)
        .map(|c| {
            (
                c.members.len(),
                c.candidate_count as usize,
                c.alternates.len(),
                c.verified as usize,
                c.derived_by,
            )
        })
        .unwrap_or((0, 0, 0, 0, CouncilDerivationKind::Fallback));
    Ok(JsonBody(CouncilAuditResponse {
        epoch,
        seed_hex,
        beacon_hex,
        members_count,
        candidate_count,
        alternates_count,
        verified,
        derived_by,
        chain_id: chain_id_ref.to_string(),
    }))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use iroha_config::parameters::actual::LaneConfig;
    use iroha_core::{
        block::BlockBuilder,
        kura::Kura,
        query::store::LiveQueryStore,
        queue::{Queue, TransactionGuard},
        state::{
            GovernanceLockRecord, GovernanceLocksForReferendum, GovernanceProposalStatus,
            GovernanceReferendumMode, GovernanceReferendumRecord, GovernanceReferendumStatus,
            State, World,
        },
    };
    use iroha_crypto::{ExposedPrivateKey, KeyPair};
    use iroha_data_model::{
        ChainId, Registrable,
        account::{Account, AccountId},
        asset::{Asset, AssetDefinition, AssetDefinitionId, AssetId},
        block::BlockHeader,
        domain::{Domain, DomainId},
        permission::Permission,
        smart_contract::manifest::ContractManifest,
    };
    use iroha_primitives::numeric::Numeric;
    use iroha_test_samples::ALICE_ID;

    use super::*;
    use crate::routing::MaybeTelemetry;

    const ACCOUNT_AUTHORITY: &str =
        "ed0120AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA@wonderland";
    const ACCOUNT_OWNER_ALT: &str =
        "ed0120BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB@wonderland";

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

    fn canonical_literal(raw: &str) -> String {
        iroha_data_model::account::AccountId::parse(raw)
            .expect("literal parses")
            .canonical()
            .to_string()
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

    struct GovHarness {
        state: Arc<State>,
        queue: Arc<Queue>,
        chain_id: Arc<ChainId>,
        authority: AccountId,
        authority_keypair: KeyPair,
        asset_def_id: AssetDefinitionId,
        escrow: AccountId,
    }

    fn mk_governance_harness(with_permissions: bool) -> GovHarness {
        let authority_keypair = KeyPair::random();
        let domain_id: DomainId = "wonderland".parse().expect("domain id");
        let authority = AccountId::of(domain_id.clone(), authority_keypair.public_key().clone());
        let escrow: AccountId =
            iroha_config::parameters::defaults::governance::bond_escrow_account()
                .parse()
                .expect("escrow account id");
        let domain = Domain::new(domain_id.clone()).build(&authority);
        let authority_account = Account::new(authority.clone()).build(&authority);
        let escrow_account = Account::new(escrow.clone()).build(&escrow);
        let asset_def_id: AssetDefinitionId =
            "vote#wonderland".parse().expect("asset definition id");
        let asset_def = AssetDefinition::numeric(asset_def_id.clone()).build(&authority);
        let asset = Asset::new(
            AssetId::new(asset_def_id.clone(), authority.clone()),
            Numeric::from(1_000u32),
        );
        let escrow_asset = Asset::new(
            AssetId::new(asset_def_id.clone(), escrow.clone()),
            Numeric::from(0u32),
        );
        let world = World::with_assets(
            [domain],
            [authority_account, escrow_account],
            [asset_def],
            [asset, escrow_asset],
            [],
        );
        if with_permissions {
            let propose = Permission::new(
                "CanProposeContractDeployment".to_string(),
                norito::json!({ "contract_id": "apps.demo" }),
            );
            let ballot = Permission::new(
                "CanSubmitGovernanceBallot".to_string(),
                norito::json!({ "referendum_id": "any" }),
            );
            let enact = Permission::new("CanEnactGovernance".to_string(), norito::json!({}));
            let mut world_block = world.block();
            #[cfg(feature = "telemetry")]
            let mut world_tx = world_block.trasaction(None, LaneConfig::default(), 0);
            #[cfg(not(feature = "telemetry"))]
            let mut world_tx = world_block.trasaction(LaneConfig::default(), 0);
            let _ = world_tx.add_account_permission(&authority, propose);
            let _ = world_tx.add_account_permission(&authority, ballot);
            let _ = world_tx.add_account_permission(&authority, enact);
            world_tx.apply();
            world_block.commit();
        }

        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut state = State::new_for_testing(world, kura, query);
        let mut gov_cfg = state.gov.clone();
        gov_cfg.voting_asset_id = asset_def_id.clone();
        gov_cfg.citizenship_asset_id = asset_def_id.clone();
        gov_cfg.bond_escrow_account = escrow.clone();
        gov_cfg.citizenship_escrow_account = escrow.clone();
        gov_cfg.slash_receiver_account = escrow.clone();
        gov_cfg.min_bond_amount = 0;
        gov_cfg.citizenship_bond_amount = 0;
        gov_cfg.plain_voting_enabled = true;
        gov_cfg.conviction_step_blocks = 1;
        gov_cfg.max_conviction = 1;
        gov_cfg.window_span = 10;
        gov_cfg.min_enactment_delay = 0;
        gov_cfg.approval_threshold_q_num = 1;
        gov_cfg.approval_threshold_q_den = 1;
        gov_cfg.min_turnout = 1;
        state.set_gov(gov_cfg);

        let events = tokio::sync::broadcast::channel(1).0;
        let queue = Arc::new(Queue::from_config(
            iroha_config::parameters::actual::Queue::default(),
            events,
        ));
        let chain_id: ChainId = "chain".parse().expect("chain id");

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
            code_hash: Some(iroha_crypto::Hash::prehashed(code_hash)),
            abi_hash: Some(iroha_crypto::Hash::prehashed(abi_hash)),
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: None,
            provenance: None,
        }
        .signed(keypair);
        manifest
            .provenance
            .expect("signed manifest should carry provenance")
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
        let leader =
            iroha_crypto::KeyPair::random_with_algorithm(iroha_crypto::Algorithm::BlsNormal);
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
            .map(|(idx, _)| block_ref.error(idx).is_some())
            .collect::<Vec<_>>();
        crate::test_utils::finalize_committed_block(state, state_block, committed_block);
        errors
    }
    #[test]
    fn serde_shapes_compile() {
        let canonical_abi = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        let req = ProposeDeployContractDto {
            contract_id: "my.contract.v1".to_string(),
            namespace: "apps".to_string(),
            abi_version: "1".to_string(),
            code_hash: "0x".to_string() + &"aa".repeat(32),
            abi_hash: format!("0x{}", hex::encode(canonical_abi)),
            window: Some(AtWindowDto { lower: 1, upper: 2 }),
            mode: None,
            limits: Some(crate::json_object(vec![("max_pages", 64u64)])),
            manifest_provenance: None,
            authority: None,
            private_key: None,
        };
        let s = norito::json::to_json(&req).unwrap();
        let _: ProposeDeployContractDto = norito::json::from_str(&s).unwrap();
    }

    #[tokio::test]
    async fn finalize_builds_instruction_skeleton() {
        let (state, queue, chain_id) = mk_basic_context();
        let dto = FinalizeDto {
            referendum_id: "ref-xyz".to_string(),
            proposal_id: format!("0x{}", "aa".repeat(32)),
            authority: None,
            private_key: None,
        };
        let res = handle_gov_finalize(
            chain_id,
            queue,
            state,
            MaybeTelemetry::disabled(),
            false,
            NoritoJson(dto),
        )
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
        let (state, queue, chain_id) = mk_basic_context();
        let code_hash_input = format!("blake2b32:0X{}", "11".repeat(32));
        let canonical_abi = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        let abi_hash_input = format!("0x{}", hex::encode(canonical_abi));
        let dto = ProposeDeployContractDto {
            contract_id: "my.contract.v1".to_string(),
            namespace: "apps".to_string(),
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
            authority: None,
            private_key: None,
        };
        let (code_hex, code_bytes) = super::canonical_hex32(&code_hash_input, "code_hash").unwrap();
        let (abi_hex, abi_bytes) = super::canonical_hex32(&abi_hash_input, "abi_hash").unwrap();
        let res = handle_gov_propose_deploy(
            chain_id,
            queue,
            state,
            MaybeTelemetry::disabled(),
            false,
            NoritoJson(dto),
        )
        .await
        .expect("handler ok");
        let body = res.0;
        assert!(body.ok);
        assert_eq!(body.tx_instructions.len(), 1);

        // Canonical hashing matches core logic
        let expected_id =
            super::compute_proposal_id("apps", "my.contract.v1", &code_bytes, &abi_bytes);
        assert_eq!(body.proposal_id, hex::encode(expected_id));

        // Payload decodes to sanitized ProposeDeployContract
        let tx = &body.tx_instructions[0];
        let payload = hex::decode(&tx.payload_hex).expect("payload hex");
        let decoded: iroha_data_model::isi::governance::ProposeDeployContract =
            norito::decode_from_bytes(&payload).expect("decode payload");
        assert_eq!(decoded.namespace, "apps");
        assert_eq!(decoded.contract_id, "my.contract.v1");
        assert_eq!(decoded.code_hash_hex, code_hex);
        assert_eq!(decoded.abi_hash_hex, abi_hex);
        assert_eq!(decoded.abi_version, "1");
    }

    #[tokio::test]
    async fn propose_deploy_rejects_unknown_mode() {
        let (state, queue, chain_id) = mk_basic_context();
        let canonical_abi = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        let dto = ProposeDeployContractDto {
            contract_id: "my.contract.v1".to_string(),
            namespace: "apps".to_string(),
            abi_version: "1".to_string(),
            code_hash: format!("{}", "11".repeat(32)),
            abi_hash: format!("{}", hex::encode(canonical_abi)),
            window: None,
            mode: Some("quadratic".to_string()),
            limits: Some(norito::json::Value::Object(norito::json::Map::new())),
            manifest_provenance: None,
            authority: None,
            private_key: None,
        };
        let err = handle_gov_propose_deploy(
            chain_id,
            queue,
            state,
            MaybeTelemetry::disabled(),
            false,
            NoritoJson(dto),
        )
        .await
        .unwrap_err();
        assert!(format!("{err:?}").contains("unsupported voting mode"));
    }

    #[tokio::test]
    async fn propose_deploy_rejects_mismatched_abi_hash() {
        let (state, queue, chain_id) = mk_basic_context();
        let dto = ProposeDeployContractDto {
            contract_id: "my.contract.v1".to_string(),
            namespace: "apps".to_string(),
            abi_version: "1".to_string(),
            code_hash: format!("{}", "11".repeat(32)),
            abi_hash: format!("{}", "22".repeat(32)),
            window: None,
            mode: None,
            limits: Some(norito::json::Value::Object(norito::json::Map::new())),
            manifest_provenance: None,
            authority: None,
            private_key: None,
        };
        let err = handle_gov_propose_deploy(
            chain_id,
            queue,
            state,
            MaybeTelemetry::disabled(),
            false,
            NoritoJson(dto),
        )
        .await
        .unwrap_err();
        assert!(format!("{err:?}").contains("abi_hash does not match canonical hash"));
    }

    #[tokio::test]
    async fn ballot_plain_builds_instruction_skeleton() {
        let (state, queue, chain_id) = mk_basic_context();
        let canonical = canonical_literal(ACCOUNT_AUTHORITY);
        let chain_id_str = chain_id.as_str().to_string();
        // Build DTO via JSON to ensure serde shape is satisfied
        let body = crate::json_object(vec![
            crate::json_entry("authority", canonical.clone()),
            crate::json_entry("chain_id", chain_id_str),
            crate::json_entry("referendum_id", "r1"),
            crate::json_entry("owner", canonical.clone()),
            crate::json_entry("amount", "100"),
            crate::json_entry("duration_blocks", 600u64),
            crate::json_entry("direction", "Aye"),
        ]);
        let parsed: PlainBallotDto =
            norito::json::from_str(&norito::json::to_json(&body).unwrap()).unwrap();
        let res = handle_gov_ballot_plain_with_policy(
            chain_id,
            queue,
            state,
            NoritoJson(parsed),
            MaybeTelemetry::for_tests(),
            true,
        )
        .await
        .expect("handler ok");
        let body = res.0;
        assert!(body.ok);
        assert!(body.accepted);
        assert!(body.tx_instructions.len() == 1);
    }

    #[tokio::test]
    async fn ballot_plain_rejects_authority_mismatch() {
        let (state, queue, chain_id) = mk_basic_context();
        let canonical_authority = canonical_literal(ACCOUNT_AUTHORITY);
        let canonical_owner = canonical_literal(ACCOUNT_OWNER_ALT);
        let chain_id_str = chain_id.as_str().to_string();
        let body = crate::json_object(vec![
            crate::json_entry("authority", canonical_authority),
            crate::json_entry("chain_id", chain_id_str),
            crate::json_entry("referendum_id", "r1"),
            crate::json_entry("owner", canonical_owner),
            crate::json_entry("amount", "100"),
            crate::json_entry("duration_blocks", 600u64),
            crate::json_entry("direction", "Aye"),
        ]);
        let parsed: PlainBallotDto =
            norito::json::from_str(&norito::json::to_json(&body).unwrap()).unwrap();
        let err = handle_gov_ballot_plain_with_policy(
            chain_id,
            queue,
            state,
            NoritoJson(parsed),
            MaybeTelemetry::for_tests(),
            true,
        )
        .await
        .unwrap_err();
        let s = format!("{err:?}");
        assert!(s.contains("authority must equal owner"));
    }

    #[tokio::test]
    async fn ballot_plain_strict_mode_rejects_raw_public_key_literals() {
        let (state, queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        let body = crate::json_object(vec![
            crate::json_entry("authority", ACCOUNT_AUTHORITY),
            crate::json_entry("chain_id", chain_id_str),
            crate::json_entry("referendum_id", "r1"),
            crate::json_entry("owner", ACCOUNT_AUTHORITY),
            crate::json_entry("amount", "100"),
            crate::json_entry("duration_blocks", 600u64),
            crate::json_entry("direction", "Aye"),
        ]);
        let parsed: PlainBallotDto =
            norito::json::from_str(&norito::json::to_json(&body).unwrap()).unwrap();
        let err = handle_gov_ballot_plain_with_policy(
            chain_id,
            queue,
            state,
            NoritoJson(parsed),
            MaybeTelemetry::for_tests(),
            true,
        )
        .await
        .unwrap_err();
        let s = format!("{err:?}");
        assert!(
            s.contains("ERR_STRICT_ADDRESS_REQUIRED"),
            "strict mode must reject raw literals; got {s}"
        );
    }

    #[tokio::test]
    async fn ballot_zk_builds_instruction_skeleton() {
        let (state, queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        // minimal non-empty proof bytes
        let proof_b64 = base64::engine::general_purpose::STANDARD.encode(b"proof");
        let dto = ZkBallotDto {
            authority:
                "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
                    .to_string(),
            chain_id: chain_id_str,
            election_id: "e1".to_string(),
            proof_b64,
            public: Some(norito::json::Value::Object(norito::json::Map::new())),
            private_key: None,
        };
        let res = handle_gov_ballot_zk(
            chain_id,
            queue,
            state,
            MaybeTelemetry::disabled(),
            false,
            NoritoJson(dto),
        )
        .await
        .expect("handler ok");
        let body = res.0;
        assert!(body.ok);
        assert!(body.accepted);
        assert_eq!(body.tx_instructions.len(), 1);
    }

    #[tokio::test]
    async fn ballot_zk_rejects_non_object_public_inputs() {
        let (state, queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        let proof_b64 = base64::engine::general_purpose::STANDARD.encode(b"proof");
        let dto = ZkBallotDto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            chain_id: chain_id_str,
            election_id: "e1".to_string(),
            proof_b64,
            public: Some(norito::json::Value::String("oops".to_string())),
            private_key: None,
        };
        let res = handle_gov_ballot_zk(
            chain_id,
            queue,
            state,
            MaybeTelemetry::disabled(),
            false,
            NoritoJson(dto),
        )
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
        let (state, queue, chain_id) = mk_basic_context();
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
            private_key: None,
        };
        let res = handle_gov_ballot_zk(
            chain_id,
            queue,
            state,
            MaybeTelemetry::disabled(),
            false,
            NoritoJson(dto),
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
    async fn ballot_zk_rejects_alias_conflicts() {
        let (state, queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        let proof_b64 = base64::engine::general_purpose::STANDARD.encode(b"proof");
        let mut map = norito::json::Map::new();
        map.insert(
            "rootHint".to_string(),
            norito::json::Value::from("aa".repeat(32)),
        );
        map.insert(
            "root_hint".to_string(),
            norito::json::Value::from("bb".repeat(32)),
        );
        let dto = ZkBallotDto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            chain_id: chain_id_str,
            election_id: "e1".to_string(),
            proof_b64,
            public: Some(norito::json::Value::Object(map)),
            private_key: None,
        };
        let res = handle_gov_ballot_zk(
            chain_id,
            queue,
            state,
            MaybeTelemetry::disabled(),
            false,
            NoritoJson(dto),
        )
        .await
        .expect("handler ok");
        let body = res.0;
        assert!(!body.ok);
        assert!(!body.accepted);
        assert_eq!(
            body.reason.as_deref(),
            Some("public inputs cannot include both rootHint and root_hint")
        );
    }

    #[test]
    fn normalize_zk_ballot_public_inputs_maps_aliases_and_canonicalizes_hex() {
        let mut map = norito::json::Map::new();
        map.insert(
            "durationBlocks".to_string(),
            norito::json::Value::from(10u64),
        );
        let root_raw = format!("0x{}", "Aa".repeat(32));
        map.insert(
            "rootHintHex".to_string(),
            norito::json::Value::from(root_raw),
        );
        let nullifier_raw = format!("blake2b32:{}", "BB".repeat(32));
        map.insert(
            "nullifierHex".to_string(),
            norito::json::Value::from(nullifier_raw),
        );
        normalize_zk_ballot_public_inputs(&mut map).expect("normalize");
        let root_expected = "aa".repeat(32);
        let nullifier_expected = "bb".repeat(32);
        assert!(map.contains_key("duration_blocks"));
        assert!(map.contains_key("root_hint"));
        assert!(map.contains_key("nullifier_hex"));
        assert!(!map.contains_key("durationBlocks"));
        assert!(!map.contains_key("rootHintHex"));
        assert!(!map.contains_key("nullifierHex"));
        assert_eq!(
            map.get("root_hint").and_then(norito::json::Value::as_str),
            Some(root_expected.as_str())
        );
        assert_eq!(
            map.get("nullifier_hex")
                .and_then(norito::json::Value::as_str),
            Some(nullifier_expected.as_str())
        );
    }

    #[test]
    fn normalize_zk_ballot_public_inputs_rejects_alias_conflicts() {
        let mut map = norito::json::Map::new();
        map.insert(
            "rootHint".to_string(),
            norito::json::Value::from("aa".repeat(32)),
        );
        map.insert(
            "root_hint".to_string(),
            norito::json::Value::from("bb".repeat(32)),
        );
        let err = normalize_zk_ballot_public_inputs(&mut map).expect_err("conflict");
        assert!(err.contains("rootHint"));
        assert!(err.contains("root_hint"));
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
                amount: 9,
                slashed: 0,
                expiry_height: 100,
                direction: 0,
                duration_blocks: 4,
            },
        );
        stx.world.governance_locks_mut().insert(rid.clone(), locks);
        stx.apply();

        let res = handle_gov_get_tally(Arc::new(state), axum::extract::Path(rid))
            .await
            .expect("handler ok");
        let body = res.0;
        assert_eq!(body.approve, 9);
        assert_eq!(body.reject, 0);
    }

    #[tokio::test]
    async fn enact_builds_instruction_skeleton() {
        let (state, queue, chain_id) = mk_basic_context();
        let dto = EnactDto {
            proposal_id: format!("0x{}", "ab".repeat(32)),
            preimage_hash: Some(format!("0x{}", "cd".repeat(32))),
            window: Some(AtWindowDto {
                lower: 100,
                upper: 200,
            }),
            authority: None,
            private_key: None,
        };
        let res = handle_gov_enact(
            chain_id,
            queue,
            state,
            MaybeTelemetry::disabled(),
            false,
            NoritoJson(dto),
        )
        .await
        .expect("handler ok");
        let body = res.0;
        assert!(body.ok);
        assert_eq!(body.tx_instructions.len(), 1);
    }

    #[tokio::test]
    async fn gov_flow_submits_and_applies() {
        let harness = mk_governance_harness(true);
        let authority_str = harness.authority.to_string();
        let private_key =
            ExposedPrivateKey(harness.authority_keypair.private_key().clone()).to_string();
        let chain_id_str = harness.chain_id.as_str().to_string();

        let code_hash_bytes = [0x11u8; 32];
        let abi_hash_bytes = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        let manifest_provenance =
            mk_manifest_provenance(&harness.authority_keypair, code_hash_bytes, abi_hash_bytes);

        let propose = ProposeDeployContractDto {
            contract_id: "demo.contract".to_string(),
            namespace: "apps".to_string(),
            abi_version: "1".to_string(),
            code_hash: format!("0x{}", hex::encode(code_hash_bytes)),
            abi_hash: format!("0x{}", hex::encode(abi_hash_bytes)),
            window: None,
            mode: Some("Plain".to_string()),
            limits: None,
            manifest_provenance: Some(manifest_provenance),
            authority: Some(authority_str.clone()),
            private_key: Some(private_key.clone()),
        };
        let res = handle_gov_propose_deploy(
            harness.chain_id.clone(),
            harness.queue.clone(),
            harness.state.clone(),
            MaybeTelemetry::disabled(),
            false,
            NoritoJson(propose),
        )
        .await
        .expect("propose ok");
        let proposal_id = res.0.proposal_id;
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

        let ballot = PlainBallotDto {
            authority: authority_str.clone(),
            chain_id: chain_id_str.clone(),
            referendum_id: proposal_id.clone(),
            owner: authority_str.clone(),
            amount: "100".to_string(),
            duration_blocks: 1,
            direction: "Aye".to_string(),
            private_key: Some(private_key.clone()),
        };
        handle_gov_ballot_plain_with_policy(
            harness.chain_id.clone(),
            harness.queue.clone(),
            harness.state.clone(),
            NoritoJson(ballot),
            MaybeTelemetry::disabled(),
            false,
        )
        .await
        .expect("ballot ok");
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
        assert_eq!(lock.amount, 100);
        assert_eq!(lock.direction, 0);

        let finalize = FinalizeDto {
            referendum_id: proposal_id.clone(),
            proposal_id: format!("0x{}", proposal_id),
            authority: Some(authority_str.clone()),
            private_key: Some(private_key.clone()),
        };
        handle_gov_finalize(
            harness.chain_id.clone(),
            harness.queue.clone(),
            harness.state.clone(),
            MaybeTelemetry::disabled(),
            false,
            NoritoJson(finalize),
        )
        .await
        .expect("finalize ok");
        let applied = crate::test_utils::apply_queued_in_one_block(
            &harness.state,
            &harness.queue,
            harness.chain_id.as_ref(),
            height,
        );
        assert_eq!(applied, 1);
        height += 1;

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
            proposal_id: format!("0x{}", proposal_id),
            preimage_hash: None,
            window: None,
            authority: Some(authority_str),
            private_key: Some(private_key),
        };
        handle_gov_enact(
            harness.chain_id.clone(),
            harness.queue.clone(),
            harness.state.clone(),
            MaybeTelemetry::disabled(),
            false,
            NoritoJson(enact),
        )
        .await
        .expect("enact ok");
        let applied = crate::test_utils::apply_queued_in_one_block(
            &harness.state,
            &harness.queue,
            harness.chain_id.as_ref(),
            height,
        );
        assert_eq!(applied, 1);

        let view = harness.state.view();
        let code_hash = iroha_crypto::Hash::prehashed(code_hash_bytes);
        let instance_key = ("apps".to_string(), "demo.contract".to_string());
        let bound_hash = view
            .world()
            .contract_instances()
            .get(&instance_key)
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
        let authority_str = harness.authority.to_string();
        let private_key =
            ExposedPrivateKey(harness.authority_keypair.private_key().clone()).to_string();
        let code_hash_bytes = [0x22u8; 32];
        let abi_hash_bytes = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        let manifest_provenance =
            mk_manifest_provenance(&harness.authority_keypair, code_hash_bytes, abi_hash_bytes);
        let propose = ProposeDeployContractDto {
            contract_id: "demo.contract".to_string(),
            namespace: "apps".to_string(),
            abi_version: "1".to_string(),
            code_hash: format!("0x{}", hex::encode(code_hash_bytes)),
            abi_hash: format!("0x{}", hex::encode(abi_hash_bytes)),
            window: None,
            mode: Some("Plain".to_string()),
            limits: None,
            manifest_provenance: Some(manifest_provenance),
            authority: Some(authority_str),
            private_key: Some(private_key),
        };
        let res = handle_gov_propose_deploy(
            harness.chain_id.clone(),
            harness.queue.clone(),
            harness.state.clone(),
            MaybeTelemetry::disabled(),
            false,
            NoritoJson(propose),
        )
        .await
        .expect("handler ok");
        let proposal_id = res.0.proposal_id;
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
    async fn council_audit_surfaces_candidate_count_and_beacon() {
        // Build a minimal test state
        let kura = iroha_core::kura::Kura::blank_kura_for_testing();
        let query = iroha_core::query::store::LiveQueryStore::start_test();
        let state = Arc::new(iroha_core::state::State::new_for_testing(
            iroha_core::state::World::default(),
            kura,
            query,
        ));

        // Call audit handler for a specific epoch and check fields (no persisted council)
        let epoch = 7u64;
        let res = handle_gov_council_audit(
            state.clone(),
            NoritoQuery(CouncilAuditQuery { epoch: Some(epoch) }),
        )
        .await
        .expect("audit ok");
        let body = res.0;
        assert_eq!(body.epoch, epoch);
        assert_eq!(body.candidate_count, 0usize);
        assert_eq!(body.members_count, 0usize);
        assert_eq!(body.alternates_count, 0usize);
        assert_eq!(body.verified, 0usize);
        assert_eq!(body.derived_by, CouncilDerivationKind::Fallback);
        assert_eq!(body.seed_hex.len(), 128); // blake2b-512 hex
        assert_eq!(body.beacon_hex.len(), 64); // 32-byte hex
        assert!(!body.chain_id.is_empty());
    }

    #[cfg(feature = "zk-ballot")]
    #[tokio::test]
    async fn ballot_zk_v1_builds_instruction_skeleton() {
        use axum::{Router, routing::post};
        use http_body_util::BodyExt as _;
        use tower::ServiceExt as _;

        let (state, queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        // Route for zk-v1
        let app = Router::new().route(
            "/v1/gov/ballots/zk-v1",
            post({
                let state = state.clone();
                let queue = queue.clone();
                let chain_id = chain_id.clone();
                move |body: crate::NoritoJson<super::ZkBallotV1Dto>| {
                    let telemetry = MaybeTelemetry::disabled();
                    async move {
                        super::handle_gov_ballot_zk_v1(
                            chain_id, queue, state, telemetry, false, body,
                        )
                        .await
                    }
                }
            }),
        );

        // Build DTO
        let dto = super::ZkBallotV1Dto {
            authority:
                "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
                    .to_string(),
            chain_id: chain_id_str,
            election_id: "ref-1".to_string(),
            backend: "halo2/ipa".to_string(),
            envelope_b64: base64::engine::general_purpose::STANDARD.encode(&[1u8, 2, 3, 4]),
            root_hint_hex: Some(hex::encode([0u8; 32])),
            owner: Some(
                "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
                    .to_string(),
            ),
            amount: Some("100".to_string()),
            duration_blocks: Some(200),
            direction: Some("Aye".to_string()),
            nullifier_hex: Some(hex::encode([0x11u8; 32])),
            private_key: None,
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

    #[cfg(feature = "zk-ballot")]
    #[tokio::test]
    async fn ballot_zk_v1_rejects_invalid_root_hint_hex() {
        let (state, queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        let dto = super::ZkBallotV1Dto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            chain_id: chain_id_str,
            election_id: "ref-1".to_string(),
            backend: "halo2/ipa".to_string(),
            envelope_b64: base64::engine::general_purpose::STANDARD.encode(&[1u8, 2, 3, 4]),
            root_hint_hex: Some("invalid".to_string()),
            owner: None,
            amount: None,
            duration_blocks: None,
            direction: None,
            nullifier_hex: None,
            private_key: None,
        };
        let res = super::handle_gov_ballot_zk_v1(
            chain_id,
            queue,
            state,
            MaybeTelemetry::disabled(),
            false,
            crate::NoritoJson(dto),
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

    #[cfg(feature = "zk-ballot")]
    #[tokio::test]
    async fn ballot_zk_v1_rejects_partial_lock_hints() {
        let (state, queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        let dto = super::ZkBallotV1Dto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            chain_id: chain_id_str,
            election_id: "ref-1".to_string(),
            backend: "halo2/ipa".to_string(),
            envelope_b64: base64::engine::general_purpose::STANDARD.encode(&[1u8, 2, 3, 4]),
            root_hint_hex: None,
            owner: Some(ACCOUNT_AUTHORITY.to_string()),
            amount: None,
            duration_blocks: None,
            direction: None,
            nullifier_hex: None,
            private_key: None,
        };
        let res = super::handle_gov_ballot_zk_v1(
            chain_id,
            queue,
            state,
            MaybeTelemetry::disabled(),
            false,
            crate::NoritoJson(dto),
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

    #[cfg(feature = "zk-ballot")]
    #[tokio::test]
    async fn ballot_zk_v1_ballotproof_builds_instruction_skeleton() {
        use axum::{Router, routing::post};
        use http_body_util::BodyExt as _;
        use iroha_data_model::isi::governance::BallotProof;
        use tower::ServiceExt as _;

        let (state, queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        // Route for zk-v1/ballot-proof
        let app = Router::new().route(
            "/v1/gov/ballots/zk-v1/ballot-proof",
            post({
                let state = state.clone();
                let queue = queue.clone();
                let chain_id = chain_id.clone();
                move |body: crate::NoritoJson<super::ZkBallotV1BallotProofDto>| {
                    let telemetry = MaybeTelemetry::disabled();
                    async move {
                        super::handle_gov_ballot_zk_v1_ballotproof(
                            chain_id, queue, state, telemetry, false, body,
                        )
                        .await
                    }
                }
            }),
        );

        // Build DTO
        let ballot = BallotProof {
            backend: "halo2/ipa".into(),
            envelope_bytes: vec![1u8, 2, 3, 4],
            root_hint: Some([0xAA; 32]),
            owner: None,
            nullifier: Some([0x11; 32]),
            amount: Some("200".to_string()),
            duration_blocks: Some(256),
            direction: Some("Nay".to_string()),
        };
        let dto = super::ZkBallotV1BallotProofDto {
            authority:
                "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
                    .to_string(),
            chain_id: chain_id_str,
            election_id: "ref-1".to_string(),
            ballot,
            private_key: None,
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

    #[cfg(feature = "zk-ballot")]
    #[tokio::test]
    async fn ballot_zk_v1_ballotproof_rejects_partial_lock_hints() {
        use iroha_data_model::isi::governance::BallotProof;

        let (state, queue, chain_id) = mk_basic_context();
        let chain_id_str = chain_id.as_str().to_string();
        let ballot = BallotProof {
            backend: "halo2/ipa".into(),
            envelope_bytes: vec![1u8, 2, 3, 4],
            root_hint: None,
            owner: Some(ACCOUNT_AUTHORITY.parse().expect("valid account id")),
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
            private_key: None,
        };
        let res = super::handle_gov_ballot_zk_v1_ballotproof(
            chain_id,
            queue,
            state,
            MaybeTelemetry::disabled(),
            false,
            crate::NoritoJson(dto),
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
