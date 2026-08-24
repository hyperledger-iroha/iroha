//! App-facing governance API.
#![allow(unexpected_cfgs)]
//!
//! This module hosts minimal DTOs and handlers for governance endpoints described in `gov.md` and
//! `specs/contract_deployment.md`. Handlers validate inputs and build instruction skeletons for
//! callers to submit through the locally signed transaction pipeline. Draft request schemas that
//! previously exposed server-side signing inputs are strict and no longer admit private signing
//! material.
//!
//! Notes
//! - JSON parsing uses Norito's serde wrappers via the `NoritoJson` extractor.
//! - Keep responses stable and explicit; map input errors to 400.
use crate::{
    JsonBody, NoritoBody, NoritoJson, NoritoJsonWithBytes, NoritoQuery,
    json_macros::{JsonDeserialize, JsonSerialize},
    routing::{MaybeTelemetry, parse_account_literal_with_state},
};
use base64::Engine as _;
use core::str::FromStr;
use iroha_core::{
    governance::parliament::ParliamentDecisionModeV1,
    kura::Kura,
    smartcontracts::Execute as _,
    state::{StateReadOnly, WorldReadOnly},
};
use iroha_data_model::{
    governance::types::{
        AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal,
        ParliamentNoResultKindV1, ProposalContentId, ProposalKind, SccpRouteGovernanceProposal,
    },
    isi::governance::CouncilDerivationKind,
    ministry::{AgendaProposalRecordV1, AgendaProposalV1},
    smart_contract::manifest::{EntryPointKind, ManifestProvenance},
};
use iroha_primitives::numeric::Quantity;
use iroha_torii_shared::governance_proposal_api::{
    DeployContractProposalDraftRequestV1, DeployContractProposalDraftResponseV1,
    GovernanceProposalInstructionDraftV1, SccpRouteGovernanceProposalDraftRequestV1,
    SccpRouteGovernanceProposalDraftResponseV1,
};
use iroha_torii_shared::parliament_api::{
    PARLIAMENT_API_VERSION_V1, PARLIAMENT_ATTEMPT_READ_MAX_STATE_BYTES_V1,
    PARLIAMENT_TIMED_OVN_CASTING_PROOF_MAX_FINALITY_CHAIN_BYTES_V1,
    PARLIAMENT_TIMED_OVN_CASTING_PROOF_MAX_RESPONSE_BYTES_V1,
    PARLIAMENT_TIMED_OVN_CASTING_PROOF_VERSION_V1, ParliamentAttemptDraftRequestV1,
    ParliamentAttemptDraftResponseV1, ParliamentAttemptReadResponseV1,
    ParliamentBodyStateProjectionV1, ParliamentDecisionModeProjectionV1,
    ParliamentInstructionDraftV1, ParliamentTimedOvnCastingContextResponseV1,
    ParliamentTimedOvnCastingPhaseProjectionV1, ParliamentTimedOvnCastingProofRequestV1,
    ParliamentTimedOvnCastingProofResponseV1, ParliamentTimedOvnReleaseIdentityProjectionV1,
    ParliamentTimedOvnSessionProjectionV1, ParliamentTleAdaptiveDealerCommitmentV1,
    ParliamentTleAdaptivePublicShareV1, ParliamentTleKeySessionBindingV1,
    ParliamentTleReleaseContextResponseV1, ParliamentTransitionDraftRequestV1,
    ParliamentTransitionDraftResponseV1, RequiredParliamentBodyProjectionV1,
    parliament_timed_ovn_casting_proof_page_tip,
};
use mv::storage::StorageReadOnly;
use norito::{
    derive::{NoritoDeserialize, NoritoSerialize},
    json,
};
const CONTEXT_GOV_BALLOT_ZK_V1_AUTHORITY: &str = "/v1/gov/ballots/zk-v1#authority";
const CONTEXT_GOV_BALLOT_ZK_V1_BALLOT_PROOF_AUTHORITY: &str =
    "/v1/gov/ballots/zk-v1/ballot-proof#authority";
const CONTEXT_GOV_BALLOT_PLAIN_AUTHORITY: &str = "/v1/gov/ballots/plain#authority";
const CONTEXT_GOV_BALLOT_PLAIN_OWNER: &str = "/v1/gov/ballots/plain#owner";
const CONTEXT_GOV_PROTECTED_AUTHORITY: &str = "/v1/gov/protected-namespaces#authority";
const CONTEXT_MINISTRY_AGENDA_DRAFT_AUTHORITY: &str =
    "/v1/ministry/agenda/proposals/draft#authority";
use std::{collections::BTreeSet, sync::Arc};
#[derive(Debug, JsonDeserialize, NoritoDeserialize, NoritoSerialize)]
#[norito(deny_unknown_fields)]
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
/// Request body for submitting a plain (non-ZK) quadratic ballot.
pub struct PlainBallotDto {
    /// Authority as canonical I105 or on-chain account alias.
    pub authority: String,
    /// Exact genesis-derived network to build the transaction skeleton for.
    pub network_id: iroha_data_model::NetworkId,
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
fn validate_optional_ballot_direction(direction: Option<&str>) -> Result<(), String> {
    if direction.is_some_and(|value| !matches!(value, "Aye" | "Nay" | "Abstain")) {
        return Err("direction must be Aye, Nay, or Abstain".to_owned());
    }
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
fn canonicalize_hex32_value(raw: &str) -> Option<String> {
    let without_scheme = if let Some((scheme, rest)) = raw.split_once(':') {
        if scheme.eq_ignore_ascii_case("blake2b32") {
            rest
        } else {
            return None;
        }
    } else {
        raw
    };
    let body = without_scheme
        .strip_prefix("0x")
        .or_else(|| without_scheme.strip_prefix("0X"))
        .unwrap_or(without_scheme);
    if body.len() != 64 || !body.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(body.to_ascii_lowercase())
}
fn validate_exact_nonempty_token(field: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || value
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err(format!(
            "{field} must be a non-empty token without whitespace or control characters"
        ));
    }
    Ok(())
}
fn validate_governance_selector_v1(field: &str, value: &str) -> Result<(), String> {
    if !iroha_data_model::governance::is_valid_governance_selector_v1(value) {
        return Err(format!(
            "{field} must match {}",
            iroha_data_model::governance::GOVERNANCE_SELECTOR_V1_PATTERN
        ));
    }
    Ok(())
}

fn is_stored_typed_proposal_fingerprint(state: &iroha_core::state::State, selector: &str) -> bool {
    if selector.len() != 64
        || !selector
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return false;
    }
    let mut proposal_id = [0_u8; 32];
    if hex::decode_to_slice(selector, &mut proposal_id).is_err() {
        return false;
    }
    state
        .world_view()
        .governance_proposals()
        .get(&proposal_id)
        .is_some()
}

fn reject_typed_proposal_ballot_selector(
    state: &iroha_core::state::State,
    selector: &str,
) -> Result<(), String> {
    if is_stored_typed_proposal_fingerprint(state, selector) {
        return Err(
            "typed proposal fingerprints use the authenticated Parliament lifecycle, not standalone referendum ballots"
                .to_owned(),
        );
    }
    Ok(())
}
fn parse_exact_lower_hex32_path(field: &str, value: &str) -> Result<[u8; 32], crate::Error> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(crate::routing::conversion_error(format!(
            "{field} must be exact lowercase 32-byte hex"
        )));
    }
    let mut decoded = [0_u8; 32];
    hex::decode_to_slice(value, &mut decoded).map_err(|_| {
        crate::routing::conversion_error(format!("{field} must be exact lowercase 32-byte hex"))
    })?;
    Ok(decoded)
}
fn require_exact_governance_path_token(field: &str, value: &str) -> Result<(), crate::Error> {
    validate_governance_selector_v1(field, value)
        .map_err(|message| crate::routing::conversion_error(message.into()))
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
    /// Exact genesis-derived network to build the transaction skeleton for.
    pub network_id: iroha_data_model::NetworkId,
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
    /// Exact genesis-derived network to build the transaction skeleton for.
    pub network_id: iroha_data_model::NetworkId,
    pub election_id: String,
    pub ballot: iroha_data_model::isi::governance::BallotProof,
}
/// POST /v1/gov/ballots/zk-v1 — accept BallotProof-like DTO and build an instruction skeleton.
///
/// The request schema excludes private signing material; callers submit locally signed transactions.
///
/// # Errors
/// Returns `crate::Error::Query` for a foreign network or invalid authority. Invalid payloads are
/// reflected in the response body.
pub async fn handle_gov_ballot_zk_v1(
    state: Arc<iroha_core::state::State>,
    authenticated_account: &iroha_data_model::account::AccountId,
    telemetry: MaybeTelemetry,
    NoritoJsonWithBytes { value: body, raw }: NoritoJsonWithBytes<ZkBallotV1Dto>,
) -> Result<JsonBody<BallotSubmitResponse>, crate::Error> {
    ensure_network_id_matches(state.as_ref(), &body.network_id)?;
    let authority_id = parse_authority_literal(
        state.as_ref(),
        body.authority.as_str(),
        &telemetry,
        CONTEXT_GOV_BALLOT_ZK_V1_AUTHORITY,
    )?;
    ensure_authenticated_authority(authenticated_account, &authority_id)?;
    if let Err(reason) = reject_zk_v1_aliases_from_raw(raw.as_ref()) {
        return Ok(ballot_rejection(&reason));
    }
    if let Err(reason) = validate_exact_nonempty_token("backend", &body.backend) {
        return Ok(ballot_rejection(&reason));
    }
    if let Err(reason) = validate_governance_selector_v1("election_id", &body.election_id) {
        return Ok(ballot_rejection(&reason));
    }
    if let Err(reason) = reject_typed_proposal_ballot_selector(state.as_ref(), &body.election_id) {
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
        if owner != &authority_id.to_string() {
            return Ok(ballot_rejection("owner must equal authority"));
        }
    }
    if let Err(reason) = validate_optional_ballot_direction(body.direction.as_deref()) {
        return Ok(ballot_rejection(&reason));
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
/// Returns `crate::Error::Query` for a foreign network or invalid authority. Malformed payloads are
/// reported via the response payload.
pub async fn handle_gov_ballot_zk_v1_ballotproof(
    state: Arc<iroha_core::state::State>,
    authenticated_account: &iroha_data_model::account::AccountId,
    telemetry: MaybeTelemetry,
    NoritoJsonWithBytes { value: body, raw }: NoritoJsonWithBytes<ZkBallotV1BallotProofDto>,
) -> Result<JsonBody<BallotSubmitResponse>, crate::Error> {
    ensure_network_id_matches(state.as_ref(), &body.network_id)?;
    let authority_id = parse_authority_literal(
        state.as_ref(),
        body.authority.as_str(),
        &telemetry,
        CONTEXT_GOV_BALLOT_ZK_V1_BALLOT_PROOF_AUTHORITY,
    )?;
    ensure_authenticated_authority(authenticated_account, &authority_id)?;
    if let Err(reason) = reject_zk_v1_ballotproof_aliases_from_raw(raw.as_ref()) {
        return Ok(ballot_rejection(&reason));
    }
    if let Err(reason) = validate_exact_nonempty_token("backend", &body.ballot.backend) {
        return Ok(ballot_rejection(&reason));
    }
    if let Err(reason) = validate_governance_selector_v1("election_id", &body.election_id) {
        return Ok(ballot_rejection(&reason));
    }
    if let Err(reason) = reject_typed_proposal_ballot_selector(state.as_ref(), &body.election_id) {
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
    let has_owner = body.ballot.owner.is_some();
    let has_amount = body.ballot.amount.is_some();
    let has_duration = body.ballot.duration_blocks.is_some();
    if lock_hints_incomplete(has_owner, has_amount, has_duration) {
        return Ok(ballot_rejection(
            "lock hints must include owner, amount, duration_blocks",
        ));
    }
    if body
        .ballot
        .owner
        .as_ref()
        .is_some_and(|owner| owner != &authority_id)
    {
        return Ok(ballot_rejection("owner must equal authority"));
    }
    if let Err(reason) = validate_optional_ballot_direction(body.ballot.direction.as_deref()) {
        return Ok(ballot_rejection(&reason));
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
/// Configured target sizes for all ten SORA Parliament bodies.
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
    /// Coordination Council target seats.
    pub coordination_council: String,
    /// Monetary Policy Committee target seats.
    pub mpc_committee: String,
    /// Financial Markets Authority Committee target seats.
    pub fma_committee: String,
    /// Oversight Committee target seats.
    pub oversight_committee: String,
    /// Policy Jury target seats.
    pub policy_jury: String,
    /// Maximum Confirmation Jury target seats.
    pub confirmation_jury: String,
}
/// Public fail-closed governance configuration and route projection.
#[derive(Debug, JsonSerialize)]
pub struct GovernanceCapabilitiesV1 {
    /// Stable projection schema identifier.
    pub schema: String,
    /// Projection layout version.
    pub version: u16,
    /// Exact genesis-derived network identity.
    pub network_id: iroha_data_model::NetworkId,
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
    /// Mandatory private binding-ballot protocol.
    pub private_ballot_protocol: String,
    /// Whether every binding Parliament ballot is private and timed-opened.
    pub mandatory_private_ballots: bool,
    /// Whether proposal-backed referendum ballot routes may replace Parliament.
    pub proposal_backed_referendum_ballots_supported: bool,
    /// Whether explicitly standalone PLAIN referenda retain their separate route.
    pub standalone_plain_ballots_supported: bool,
    /// Whether explicitly standalone ZK referenda retain their separate route.
    pub standalone_zk_ballots_supported: bool,
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
    /// Certification-to-enactment delay in blocks.
    pub min_enactment_delay: String,
    /// Invitation-response window in blocks.
    pub invitation_phase_blocks: String,
    /// Timed-OVN proof-registration window in blocks.
    pub registration_phase_blocks: String,
    /// Timed-OVN authenticated-dropout/survivor window in blocks.
    pub survivor_freeze_phase_blocks: String,
    /// Timed-OVN masked-ballot commitment window in blocks.
    pub commitment_phase_blocks: String,
    /// Delay from commitment close to threshold release in blocks.
    pub release_delay_blocks: String,
    /// Timed-OVN aggregate-opening window in blocks.
    pub opening_phase_blocks: String,
    /// Retry attempts permitted after the initial private ballot.
    pub max_ballot_retries: String,
    /// Hard participant/corpus entry bound.
    pub max_corpus_entries: String,
    /// Configured targets; actual proposal rosters are capped by eligible citizens.
    pub target_body_sizes: GovernanceTargetBodySizesV1,
    /// Typed proposal kinds supported by the first release.
    pub supported_proposal_kinds: Vec<String>,
    /// Canonical public governance routes supported by the node.
    pub supported_routes: Vec<String>,
}
const GOVERNANCE_APPROVAL_MODE_V1: &str = "PARLIAMENT_ATTEMPT_TIMED_OVN_V1";
/// GET `/v1/gov/capabilities` — return strict public governance readiness.
///
/// # Errors
/// Returns an internal query error before a committed genesis block exists.
pub async fn handle_gov_capabilities(
    state: Arc<iroha_core::state::State>,
) -> Result<JsonBody<GovernanceCapabilitiesV1>, crate::Error> {
    if state.committed_height() == 0 {
        return Err(crate::Error::Query(
            iroha_data_model::ValidationFail::InternalError(
                "governance capabilities are unavailable before committed genesis".into(),
            ),
        ));
    }
    let gov = state.governance_snapshot();
    let world = state.world_view();
    Ok(JsonBody(GovernanceCapabilitiesV1 {
        schema: GOVERNANCE_CAPABILITIES_SCHEMA_V1.to_owned(),
        version: GOVERNANCE_CAPABILITIES_VERSION_V1,
        network_id: *state.network_id_ref(),
        current_height: u64::try_from(state.committed_height())
            .unwrap_or(u64::MAX)
            .to_string(),
        network_prefix: iroha_data_model::account::address::chain_discriminant().to_string(),
        abi_version: world.abi_version().to_string(),
        data_model_version: iroha_data_model::DATA_MODEL_VERSION.to_string(),
        approval_mode: GOVERNANCE_APPROVAL_MODE_V1.to_owned(),
        private_ballot_protocol: "TIMED_OVN_TLE_THRESHOLD_BLS_V1".to_owned(),
        mandatory_private_ballots: true,
        proposal_backed_referendum_ballots_supported: false,
        standalone_plain_ballots_supported: gov.plain_voting_enabled,
        standalone_zk_ballots_supported: true,
        citizenship_asset_id: gov.citizenship_asset_id.to_string(),
        citizenship_bond_amount: gov.citizenship_bond_amount.to_string(),
        citizenship_escrow_account: gov.citizenship_escrow_account.to_string(),
        voting_asset_id: gov.voting_asset_id.to_string(),
        min_bond_amount: gov.min_bond_amount.to_string(),
        bond_escrow_account: gov.bond_escrow_account.to_string(),
        min_enactment_delay: gov.min_enactment_delay.to_string(),
        invitation_phase_blocks: gov.parliament_invitation_phase_blocks.to_string(),
        registration_phase_blocks: gov
            .parliament_timed_ovn
            .registration_phase_blocks
            .to_string(),
        survivor_freeze_phase_blocks: gov
            .parliament_timed_ovn
            .survivor_freeze_phase_blocks
            .to_string(),
        commitment_phase_blocks: gov.parliament_timed_ovn.commitment_phase_blocks.to_string(),
        release_delay_blocks: gov.parliament_timed_ovn.release_delay_blocks.to_string(),
        opening_phase_blocks: gov.parliament_timed_ovn.opening_phase_blocks.to_string(),
        max_ballot_retries: gov.parliament_timed_ovn.max_ballot_retries.to_string(),
        max_corpus_entries: gov.parliament_timed_ovn.max_corpus_entries.to_string(),
        target_body_sizes: GovernanceTargetBodySizesV1 {
            rules_committee: gov.rules_committee_size.to_string(),
            agenda_council: gov.agenda_council_size.to_string(),
            interest_panel: gov.interest_panel_size.to_string(),
            review_panel: gov.review_panel_size.to_string(),
            coordination_council: gov.coordination_council_size.to_string(),
            mpc_committee: gov.mpc_committee_size.to_string(),
            fma_committee: gov.fma_committee_size.to_string(),
            oversight_committee: gov.oversight_committee_size.to_string(),
            policy_jury: gov.policy_jury_size.to_string(),
            confirmation_jury: gov.confirmation_jury_size.to_string(),
        },
        supported_proposal_kinds: vec![
            "DEPLOY_CONTRACT".to_owned(),
            "MUSUBI_REGISTRY_GOVERNANCE".to_owned(),
            "RUNTIME_UPGRADE".to_owned(),
            "SCCP_ROUTE_GOVERNANCE".to_owned(),
            "SORAFS_PROVIDER_GOVERNANCE".to_owned(),
            "VALIDATION_FEE_PAYOUT_LIFECYCLE".to_owned(),
            "VALIDATION_FEE_POLICY".to_owned(),
        ],
        supported_routes: vec![
            "/v1/gov/capabilities".to_owned(),
            "/v1/gov/citizens/draft".to_owned(),
            "/v1/gov/parliament/attempts/draft".to_owned(),
            "/v1/gov/parliament/attempts/{governance_attempt_id}".to_owned(),
            "/v1/gov/parliament/ballots/{ballot_attempt_id}/casting-proof".to_owned(),
            "/v1/gov/parliament/transitions/draft".to_owned(),
            "/v1/gov/ballots/plain".to_owned(),
            "/v1/gov/ballots/zk-v1".to_owned(),
            "/v1/gov/ballots/zk-v1/ballot-proof".to_owned(),
            "/v1/validation-fee/proposals".to_owned(),
            "/v1/validation-fee/proposals/{proposal_id}".to_owned(),
            "/v1/validation-fee/proposals/draft".to_owned(),
        ],
    }))
}

/// POST `/v1/gov/parliament/attempts/draft` — draft one canonical attempt creation.
///
/// Authentication is enforced by the route wrapper. The response is bound to
/// the exact proposal and retry sequence and contains no signing material.
///
/// # Errors
/// Returns a conversion error for an unsupported request version.
pub async fn handle_gov_parliament_attempt_draft(
    NoritoJson(body): NoritoJson<ParliamentAttemptDraftRequestV1>,
) -> Result<JsonBody<ParliamentAttemptDraftResponseV1>, crate::Error> {
    if body.version != PARLIAMENT_API_VERSION_V1 {
        return Err(crate::routing::conversion_error(format!(
            "unsupported Parliament attempt draft version {}; expected {}",
            body.version, PARLIAMENT_API_VERSION_V1
        )));
    }
    let instruction = iroha_data_model::isi::governance::CreateParliamentGovernanceAttemptV1 {
        proposal: body.proposal,
        attempt_sequence: body.attempt_sequence,
    };
    let proposal_content_id = instruction.proposal_content_id();
    let governance_attempt_id = instruction.governance_attempt_id();
    let draft = tx_instr_from_box(instruction.into());
    Ok(JsonBody(ParliamentAttemptDraftResponseV1 {
        version: PARLIAMENT_API_VERSION_V1,
        proposal_content_id,
        governance_attempt_id,
        tx_instructions: vec![ParliamentInstructionDraftV1 {
            wire_id: draft.wire_id,
            payload_hex: draft.payload_hex,
        }],
    }))
}

/// POST `/v1/gov/parliament/transitions/draft` — draft one closed transition.
///
/// The state-independent request bounds are checked before framing. Consensus
/// still rechecks authority, state, phase, height, proof, roster, and
/// certificate bindings when the locally signed instruction executes.
///
/// # Errors
/// Returns a conversion error for an unsupported version or invalid static
/// bound.
pub async fn handle_gov_parliament_transition_draft(
    NoritoJson(body): NoritoJson<ParliamentTransitionDraftRequestV1>,
) -> Result<JsonBody<ParliamentTransitionDraftResponseV1>, crate::Error> {
    body.validate_static()
        .map_err(|reason| crate::routing::conversion_error(reason.to_owned()))?;
    let transition_kind = body.transition.kind();
    let transition_digest = body.transition.digest_v1();
    let governance_attempt_id = body.governance_attempt_id;
    let instruction = iroha_data_model::isi::governance::SubmitParliamentLifecycleTransitionV1 {
        governance_attempt_id,
        transition: body.transition,
    };
    let draft = tx_instr_from_box(instruction.into());
    Ok(JsonBody(ParliamentTransitionDraftResponseV1 {
        version: PARLIAMENT_API_VERSION_V1,
        governance_attempt_id,
        transition_kind,
        transition_digest,
        tx_instructions: vec![ParliamentInstructionDraftV1 {
            wire_id: draft.wire_id,
            payload_hex: draft.payload_hex,
        }],
    }))
}

/// GET `/v1/gov/parliament/attempts/{governance_attempt_id}` — read one attempt.
///
/// The typed summary and complete reducer bytes come from one committed query
/// view. The payload is canonical Norito and never includes secret DKG shares,
/// plaintext ballots, or a recovery fallback.
///
/// # Errors
/// Returns a conversion error for a noncanonical identifier, a missing attempt,
/// or a reducer payload exceeding the defensive response bound.
pub async fn handle_gov_parliament_attempt_read(
    state: Arc<iroha_core::state::State>,
    governance_attempt_id: String,
) -> Result<JsonBody<ParliamentAttemptReadResponseV1>, crate::Error> {
    let governance_attempt_id = governance_attempt_id
        .parse::<iroha_data_model::governance::types::GovernanceAttemptId>()
        .map_err(|_| {
            crate::routing::conversion_error(
                "governance_attempt_id must be exactly 64 lowercase hexadecimal characters"
                    .to_owned(),
            )
        })?;
    if governance_attempt_id
        .as_bytes()
        .iter()
        .all(|byte| *byte == 0)
    {
        return Err(crate::routing::conversion_error(
            "governance_attempt_id must be non-zero".to_owned(),
        ));
    }
    let view = state.query_view();
    let attempt = view
        .world()
        .parliament_attempts()
        .get(&governance_attempt_id)
        .ok_or_else(|| {
            crate::routing::conversion_error("Parliament governance attempt was not found".into())
        })?;
    let state_payload = norito::core::to_bytes_bounded(
        attempt,
        PARLIAMENT_ATTEMPT_READ_MAX_STATE_BYTES_V1,
    )
    .map_err(|_| {
        crate::Error::Query(iroha_data_model::ValidationFail::InternalError(
            "Parliament attempt projection exceeds or violates the first-release framed Norito bound"
                .into(),
        ))
    })?;
    let required_bodies = attempt
        .required_bodies()
        .iter()
        .map(|entry| RequiredParliamentBodyProjectionV1 {
            body: entry.body,
            decision_mode: match entry.decision_mode {
                ParliamentDecisionModeV1::PublicFinding => {
                    ParliamentDecisionModeProjectionV1::PublicFinding
                }
                ParliamentDecisionModeV1::HiddenBindingBallot => {
                    ParliamentDecisionModeProjectionV1::HiddenBindingBallot
                }
            },
        })
        .collect::<Vec<_>>();
    let body_states = attempt
        .required_bodies()
        .iter()
        .map(|entry| {
            let state = attempt.sealed_body_for_role(entry.body);
            let ballot = state.and_then(|body| attempt.active_ballot_for_body(&body.instance().id));
            let no_result_kind = state
                .and_then(iroha_core::governance::parliament::ParliamentBodyStateV1::public_finding_no_result_kind)
                .or_else(|| {
                    ballot
                        .and_then(iroha_core::governance::parliament::ParliamentBallotStateV1::failure_kind)
                        .map(ParliamentNoResultKindV1::from)
                });
            let no_result_height = state
                .and_then(iroha_core::governance::parliament::ParliamentBodyStateV1::public_finding_no_result_height)
                .or_else(|| {
                    ballot.and_then(
                        iroha_core::governance::parliament::ParliamentBallotStateV1::failure_height,
                    )
                });
            ParliamentBodyStateProjectionV1 {
                body: entry.body,
                body_instance_id: state.map(|body| body.instance().id),
                status: state.map(|body| body.instance().status),
                public_finding_opened_at_height: state
                    .and_then(iroha_core::governance::parliament::ParliamentBodyStateV1::public_finding_opened_at_height),
                public_finding_phase_blocks: state
                    .and_then(iroha_core::governance::parliament::ParliamentBodyStateV1::public_finding_phase_blocks),
                public_finding_deadline_height: state
                    .and_then(iroha_core::governance::parliament::ParliamentBodyStateV1::public_finding_deadline_height),
                no_result_kind,
                no_result_height,
            }
        })
        .collect();
    Ok(JsonBody(ParliamentAttemptReadResponseV1 {
        version: PARLIAMENT_API_VERSION_V1,
        current_height: u64::try_from(view.height()).unwrap_or(u64::MAX),
        attempt: attempt.attempt().clone(),
        policy_version: attempt.policy_version(),
        required_bodies,
        body_states,
        certificate: attempt.certificate().cloned(),
        terminal_height: attempt.terminal_height(),
        execution_failure_root: attempt.execution_failure_root(),
        superseding_head: attempt.superseding_head(),
        state_payload_hex: hex::encode(state_payload),
    }))
}

fn project_parliament_tle_key_session_v1(
    session: &iroha_core::tle_release::TleKeySessionPublicStateV1,
) -> ParliamentTleKeySessionBindingV1 {
    ParliamentTleKeySessionBindingV1 {
        version: session.version,
        key_session_id: session.key_session_id,
        network_id: session.network_id,
        roster_hash: session.roster_hash,
        committee_size: session.committee_size,
        threshold: session.threshold,
        generator_h: session.generator_h,
        generator_v: session.generator_v,
        qualified_dealers: session.qualified_dealers.clone(),
        qualified_dealer_commitments: session
            .qualified_dealer_commitments
            .iter()
            .map(|dealer| ParliamentTleAdaptiveDealerCommitmentV1 {
                dealer_index: dealer.dealer_index,
                coefficient_commitments: dealer.coefficient_commitments.clone(),
                constant_pok_commitment: dealer.constant_pok_commitment,
                constant_pok_response: dealer.constant_pok_response,
            })
            .collect(),
        dkg_event_hash: session.dkg_event_hash,
        group_public_key: session.group_public_key,
        public_shares: session
            .public_shares
            .iter()
            .map(|share| ParliamentTleAdaptivePublicShareV1 {
                index: share.index,
                participant_hash: share.participant_hash,
                public_key_share: share.public_key_share,
            })
            .collect(),
        transcript_hash: session.transcript_hash,
    }
}

fn project_parliament_timed_ovn_release_identity_v1(
    identity: &iroha_core::governance::timed_ovn::TimedOvnReleaseIdentityPublicV1,
) -> ParliamentTimedOvnReleaseIdentityProjectionV1 {
    use iroha_data_model::governance::types::{
        BallotAttemptId, BodyInstanceId, GovernanceAttemptId,
    };

    ParliamentTimedOvnReleaseIdentityProjectionV1 {
        tle_key_session_id: identity.tle_key_session_id,
        governance_attempt_id: GovernanceAttemptId::new(identity.governance_attempt_id),
        body_instance_id: BodyInstanceId::new(identity.body_instance_id),
        ballot_attempt_id: BallotAttemptId::new(identity.ballot_attempt_id),
        survivor_corpus_root: identity.survivor_corpus_root,
        no_recovery_root: identity.no_recovery_root,
        target_finalized_height: identity.target_finalized_height,
        parameter_hash: identity.parameter_hash,
    }
}

/// GET `/v1/gov/parliament/ballots/{ballot_attempt_id}/casting-context`.
///
/// Core admits only the three pre-seal timed-OVN phases and replays the exact
/// lifecycle and complete public TLE transcript before this projection is
/// built. This node-local projection is diagnostic only and is never sufficient
/// native-wallet or seed-unsealing input. Wallets consume the finality-bound
/// casting-proof route, verify an externally pinned consensus chain, and replay
/// its archive before touching secret material.
///
/// # Errors
/// Returns a conversion error for a noncanonical identifier or unavailable,
/// terminal, cross-bound, or replay-invalid committed state.
pub fn handle_gov_parliament_timed_ovn_casting_context_read(
    state: Arc<iroha_core::state::State>,
    ballot_attempt_id: String,
) -> Result<JsonBody<ParliamentTimedOvnCastingContextResponseV1>, crate::Error> {
    use base64::Engine as _;
    use iroha_core::tle_release::ParliamentTimedOvnCastingPhaseV1;
    use iroha_data_model::governance::types::{
        BallotAttemptId, BodyInstanceId, GovernanceAttemptId, ProposalContentId,
    };

    let ballot_attempt_id = ballot_attempt_id.parse::<BallotAttemptId>().map_err(|_| {
        crate::routing::conversion_error(
            "ballot_attempt_id must be exactly 64 lowercase hexadecimal characters".to_owned(),
        )
    })?;
    if ballot_attempt_id.as_bytes().iter().all(|byte| *byte == 0) {
        return Err(crate::routing::conversion_error(
            "ballot_attempt_id must be non-zero".to_owned(),
        ));
    }

    let view = state.query_view();
    let context = iroha_core::tle_release::authorize_parliament_timed_ovn_casting_context_v1(
        &view,
        ballot_attempt_id,
    )
    .map_err(|error| {
        crate::routing::conversion_error(format!(
            "Parliament timed-OVN casting context is not authorized: {error}"
        ))
    })?;
    let archive = context.archive_v1();
    let archive_bytes = archive.to_canonical_bytes_v1().map_err(|error| {
        crate::Error::Query(iroha_data_model::ValidationFail::InternalError(format!(
            "authorized Parliament timed-OVN casting archive could not be framed: {error}"
        )))
    })?;
    let session = context.session();
    let phase = match context.phase() {
        ParliamentTimedOvnCastingPhaseV1::Registered => {
            ParliamentTimedOvnCastingPhaseProjectionV1::Registered
        }
        ParliamentTimedOvnCastingPhaseV1::RegistrationClosed => {
            ParliamentTimedOvnCastingPhaseProjectionV1::RegistrationClosed
        }
        ParliamentTimedOvnCastingPhaseV1::SurvivorsFrozen => {
            ParliamentTimedOvnCastingPhaseProjectionV1::SurvivorsFrozen
        }
    };
    let response = ParliamentTimedOvnCastingContextResponseV1 {
        version: PARLIAMENT_API_VERSION_V1,
        current_height: context.finalized_height(),
        phase,
        session: ParliamentTimedOvnSessionProjectionV1 {
            network_id: session.network_id,
            proposal_content_id: ProposalContentId::new(session.proposal_content_id),
            governance_attempt_id: GovernanceAttemptId::new(session.governance_attempt_id),
            body_instance_id: BodyInstanceId::new(session.body_instance_id),
            ballot_attempt_id: BallotAttemptId::new(session.ballot_attempt_id),
            parameter_hash: session.parameter_hash,
            tle_key_session_id: session.tle_key_session_id,
            tle_key_transcript_hash: session.tle_key_transcript_hash,
            tle_master_public_key: session.tle_master_public_key,
        },
        registration_opened_at_finalized_height: context.registration_opened_at_finalized_height(),
        target_finalized_height: context.target_finalized_height(),
        tle_key_session: project_parliament_tle_key_session_v1(
            context.tle_key_session().public_state(),
        ),
        registration_records_hex: context
            .registration_records()
            .iter()
            .map(hex::encode)
            .collect(),
        survivor_participant_hashes: context.survivor_participant_hashes().map(<[_]>::to_vec),
        release_identity: context
            .release_identity()
            .map(project_parliament_timed_ovn_release_identity_v1),
        archive_norito_base64: base64::engine::general_purpose::STANDARD.encode(archive_bytes),
    };
    response
        .validate_for_ballot(ballot_attempt_id)
        .map_err(|reason| {
            crate::Error::Query(iroha_data_model::ValidationFail::InternalError(
                reason.into(),
            ))
        })?;
    Ok(JsonBody(response))
}

/// POST `/v1/gov/parliament/ballots/{ballot_attempt_id}/casting-proof`.
///
/// A bounded intermediate page carries only consecutive finality proofs for
/// durable checkpoint promotion. The terminal page additionally carries the
/// replay-validated public archive, its exact compact binding, membership in
/// the per-block casting root, and the fixed ordinary-write witness.
///
/// # Errors
/// Returns a conversion error for an invalid request/identifier and a stable
/// service error when finalized proof material is unavailable or inconsistent.
pub fn handle_gov_parliament_timed_ovn_casting_proof(
    state: Arc<iroha_core::state::State>,
    kura: Arc<Kura>,
    ballot_attempt_id: String,
    request: ParliamentTimedOvnCastingProofRequestV1,
) -> Result<NoritoBody<ParliamentTimedOvnCastingProofResponseV1>, crate::Error> {
    use iroha_data_model::governance::types::BallotAttemptId;

    let inconsistent = |message: String| crate::Error::AppServiceUnavailable {
        code: "parliament_timed_ovn_casting_proof_inconsistent",
        message,
    };
    if request.version != PARLIAMENT_TIMED_OVN_CASTING_PROOF_VERSION_V1
        || request.trusted_checkpoint_height == 0
    {
        return Err(crate::routing::conversion_error(
            "Parliament casting proof version or checkpoint height is invalid".to_owned(),
        ));
    }
    let ballot_attempt_id = ballot_attempt_id.parse::<BallotAttemptId>().map_err(|_| {
        crate::routing::conversion_error(
            "ballot_attempt_id must be exactly 64 lowercase hexadecimal characters".to_owned(),
        )
    })?;
    if ballot_attempt_id.as_bytes().iter().all(|byte| *byte == 0) {
        return Err(crate::routing::conversion_error(
            "ballot_attempt_id must be non-zero".to_owned(),
        ));
    }

    let state_view = state.query_view();
    let observed_ledger_tip_height = u64::try_from(state_view.height()).map_err(|_| {
        inconsistent("ledger height does not fit the Parliament casting proof".to_owned())
    })?;
    if request.trusted_checkpoint_height > observed_ledger_tip_height {
        return Err(crate::routing::conversion_error(
            "trusted checkpoint is newer than the observed ledger tip".to_owned(),
        ));
    }
    let evaluated_height = parliament_timed_ovn_casting_proof_page_tip(
        request.trusted_checkpoint_height,
        observed_ledger_tip_height,
    )
    .ok_or_else(|| {
        crate::routing::conversion_error(
            "trusted checkpoint cannot begin a Parliament casting finality page".to_owned(),
        )
    })?;

    let terminal_archive = if evaluated_height == observed_ledger_tip_height {
        let authorized =
            iroha_core::tle_release::authorize_parliament_timed_ovn_casting_context_v1(
                &state_view,
                ballot_attempt_id,
            )
            .map_err(|error| {
                crate::routing::conversion_error(format!(
                    "Parliament timed-OVN casting context is not authorized: {error}"
                ))
            })?;
        let archive = authorized.archive_v1();
        let archive_bytes = archive.to_canonical_bytes_v1().map_err(|error| {
            inconsistent(format!(
                "authorized Parliament casting archive could not be framed: {error}"
            ))
        })?;
        Some((archive, archive_bytes))
    } else {
        None
    };
    drop(state_view);

    let terminal_fields = if let Some((archive, archive_bytes)) = terminal_archive {
        let proof = kura
            .parliament_timed_ovn_finalized_casting_proof_v1(evaluated_height, ballot_attempt_id)
            .map_err(|error| {
                inconsistent(format!(
                    "evaluated Parliament casting proof is invalid: {error}"
                ))
            })?
            .ok_or_else(|| {
                inconsistent(
                    "authorized ballot has no retained finalized casting membership proof"
                        .to_owned(),
                )
            })?;
        let validated_archive = archive.validate_v1().map_err(|error| {
            inconsistent(format!(
                "authorized Parliament casting archive failed replay: {error}"
            ))
        })?;
        if !validated_archive.matches_compact_binding_v1(&proof.binding) {
            return Err(inconsistent(
                "authorized casting archive differs from its finalized compact binding".to_owned(),
            ));
        }
        (
            Some(archive_bytes),
            Some(proof.binding),
            Some(proof.membership_proof),
            Some(proof.snapshot_witness),
        )
    } else {
        (None, None, None, None)
    };

    let proof_count = evaluated_height
        .checked_sub(request.trusted_checkpoint_height)
        .and_then(|gap| gap.checked_add(1))
        .and_then(|count| usize::try_from(count).ok())
        .ok_or_else(|| {
            crate::routing::conversion_error(
                "trusted checkpoint is newer than the evaluated casting block".to_owned(),
            )
        })?;
    let mut finality_chain = Vec::with_capacity(proof_count);
    for height in request.trusted_checkpoint_height..=evaluated_height {
        finality_chain.push(
            iroha_core::bridge::build_finality_proof(state.as_ref(), height).map_err(|error| {
                inconsistent(format!(
                    "Parliament casting finality proof at height {height} is unavailable: {error}"
                ))
            })?,
        );
    }
    let finality_encoded_bytes =
        norito::core::encoded_frame_len(&finality_chain).map_err(|error| {
            inconsistent(format!(
                "Parliament casting finality chain cannot be encoded: {error}"
            ))
        })?;
    if finality_encoded_bytes > PARLIAMENT_TIMED_OVN_CASTING_PROOF_MAX_FINALITY_CHAIN_BYTES_V1 {
        return Err(crate::Error::AppConflict {
            code: "parliament_timed_ovn_casting_finality_page_too_large",
            message: "The bounded Parliament casting finality page exceeds its byte budget."
                .to_owned(),
        });
    }
    let evaluated = finality_chain
        .last()
        .ok_or_else(|| inconsistent("Parliament casting finality chain is empty".to_owned()))?;
    let evaluated_context_id = evaluated.finality_artifact.context_id();
    let evaluated_block_hash = evaluated.finality_artifact.block_hash;
    let response = ParliamentTimedOvnCastingProofResponseV1 {
        version: PARLIAMENT_TIMED_OVN_CASTING_PROOF_VERSION_V1,
        casting_context_archive: terminal_fields.0,
        casting_context_binding: terminal_fields.1,
        context_membership_proof: terminal_fields.2,
        casting_witness: terminal_fields.3,
        finality_chain,
        evaluated_context_id,
        evaluated_block_height: evaluated_height,
        evaluated_block_hash: hex::encode(evaluated_block_hash.as_ref()),
        observed_ledger_tip_height,
        more_available: evaluated_height < observed_ledger_tip_height,
    };
    let trusted = response
        .finality_chain
        .first()
        .expect("constructed Parliament casting finality chain is non-empty");
    response
        .verify_consensus_page_against(
            trusted.finality_artifact.height_context.network_id,
            request.trusted_checkpoint_height,
            *trusted.finality_artifact.context_id().0.as_ref(),
            ballot_attempt_id,
        )
        .map_err(|error| inconsistent(format!("constructed casting proof failed: {error}")))?;
    let response_encoded_bytes = norito::core::encoded_frame_len(&response).map_err(|error| {
        inconsistent(format!(
            "Parliament casting proof response cannot be encoded: {error}"
        ))
    })?;
    if response_encoded_bytes > PARLIAMENT_TIMED_OVN_CASTING_PROOF_MAX_RESPONSE_BYTES_V1 {
        return Err(crate::Error::AppConflict {
            code: "parliament_timed_ovn_casting_proof_too_large",
            message: "The Parliament casting proof exceeds its response byte budget.".to_owned(),
        });
    }
    Ok(NoritoBody(response))
}

/// GET `/v1/gov/parliament/ballots/{ballot_attempt_id}/release-context`.
///
/// Core returns a context only for a replay-valid sealed timed-OVN corpus whose
/// committed ballot is already `Opening`, whose release height has arrived,
/// and whose inclusive opening deadline has not passed. The response contains
/// public transcript and identity bindings only; it excludes every corpus,
/// share, secret, and individual opening.
///
/// # Errors
/// Returns a conversion error for a noncanonical identifier or when Core does
/// not authorize release from the point-in-time committed query view.
pub fn handle_gov_parliament_tle_release_context_read(
    state: Arc<iroha_core::state::State>,
    ballot_attempt_id: String,
) -> Result<JsonBody<ParliamentTleReleaseContextResponseV1>, crate::Error> {
    use iroha_data_model::governance::types::{
        BallotAttemptId, BallotAttemptStatusV1, BodyInstanceId, GovernanceAttemptId,
    };
    use sha2::{Digest as _, Sha256};

    let ballot_attempt_id = ballot_attempt_id.parse::<BallotAttemptId>().map_err(|_| {
        crate::routing::conversion_error(
            "ballot_attempt_id must be exactly 64 lowercase hexadecimal characters".to_owned(),
        )
    })?;
    if ballot_attempt_id.as_bytes().iter().all(|byte| *byte == 0) {
        return Err(crate::routing::conversion_error(
            "ballot_attempt_id must be non-zero".to_owned(),
        ));
    }

    let view = state.query_view();
    let context =
        iroha_core::tle_release::authorize_parliament_tle_release_v1(&view, ballot_attempt_id)
            .map_err(|error| {
                crate::routing::conversion_error(format!(
                    "Parliament TLE release context is not authorized: {error}"
                ))
            })?;
    let release_identity = context.public_release_identity();
    let session = context.session().public_state();
    let identity_payload = context.identity().payload_bytes();
    let identity_digest: [u8; 32] =
        Sha256::digest(context.identity().release_message().map_err(|error| {
            crate::Error::Query(iroha_data_model::ValidationFail::InternalError(format!(
                "authorized Parliament TLE release message could not be framed: {error}"
            )))
        })?)
        .into();

    Ok(JsonBody(ParliamentTleReleaseContextResponseV1 {
        version: PARLIAMENT_API_VERSION_V1,
        current_height: context.finalized_height(),
        ballot_attempt_id,
        governance_attempt_id: GovernanceAttemptId::new(release_identity.governance_attempt_id),
        body_instance_id: BodyInstanceId::new(release_identity.body_instance_id),
        status: BallotAttemptStatusV1::Opening,
        release_height: release_identity.target_finalized_height,
        opening_deadline_height: context.opening_deadline_height(),
        tle_key_session: project_parliament_tle_key_session_v1(session),
        release_identity: project_parliament_timed_ovn_release_identity_v1(release_identity),
        identity_digest,
        identity_payload_hex: hex::encode(identity_payload),
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
        total: world.citizens().len().to_string(),
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
    /// Current committed State height.
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
    let view = state.query_view();
    let snapshot = *view.world().governance_unlock_stats();
    let last_sweep_height = *view.world().governance_last_unlock_sweep_height();
    Ok(JsonBody(UnlockStatsResponse {
        height_current: u64::try_from(view.height()).unwrap_or(u64::MAX),
        expired_locks_now: snapshot.expired_locks_now,
        referenda_with_expired: snapshot.referenda_with_expired,
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
fn ensure_network_id_matches(
    state: &iroha_core::state::State,
    provided: &iroha_data_model::NetworkId,
) -> Result<(), crate::Error> {
    if state.network_id_ref() != provided {
        return Err(crate::Error::Query(
            iroha_data_model::ValidationFail::NotPermitted(
                "governance ballot targets a different network".to_owned(),
            ),
        ));
    }
    Ok(())
}
fn ensure_authenticated_authority(
    authenticated_account: &iroha_data_model::account::AccountId,
    authority: &iroha_data_model::account::AccountId,
) -> Result<(), crate::Error> {
    if authenticated_account != authority {
        return Err(crate::Error::Query(
            iroha_data_model::ValidationFail::NotPermitted(
                "authenticated account must equal the governance ballot authority".to_owned(),
            ),
        ));
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
    let canonical = iroha_data_model::account::AccountId::canonicalize(raw).map_err(|_| {
        crate::routing::conversion_error("authority must use canonical I105 account id form".into())
    })?;
    if canonical != raw {
        return Err(crate::routing::conversion_error(
            "authority must use canonical I105 account id form".into(),
        ));
    }
    parse_authority_literal(state, raw, telemetry, context)
}
fn instruction_skeleton_for_propose(
    instr: &iroha_data_model::isi::governance::ProposeDeployContract,
) -> [GovernanceProposalInstructionDraftV1; 1] {
    let boxed: iroha_data_model::isi::InstructionBox = instr.clone().into();
    [governance_proposal_instruction_draft(boxed)]
}
fn instruction_skeleton_for_sccp_route_governance_propose(
    instr: &iroha_data_model::isi::governance::ProposeSccpRouteGovernance,
) -> [GovernanceProposalInstructionDraftV1; 1] {
    let boxed: iroha_data_model::isi::InstructionBox = instr.clone().into();
    [governance_proposal_instruction_draft(boxed)]
}
fn governance_proposal_instruction_draft(
    boxed: iroha_data_model::isi::InstructionBox,
) -> GovernanceProposalInstructionDraftV1 {
    let TxInstr {
        wire_id,
        payload_hex,
    } = tx_instr_from_box(boxed);
    GovernanceProposalInstructionDraftV1 {
        wire_id,
        payload_hex,
    }
}
fn build_signable_transaction_b64(
    network_id: &iroha_data_model::NetworkId,
    authority: &iroha_data_model::account::AccountId,
    instructions: Vec<iroha_data_model::isi::InstructionBox>,
) -> String {
    let builder = iroha_data_model::transaction::signed::TransactionBuilder::new(
        *network_id,
        authority.clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions(instructions);
    base64::engine::general_purpose::STANDARD.encode(builder.encode_payload())
}
fn deploy_contract_proposal_kind(
    contract_address: &iroha_data_model::smart_contract::ContractAddress,
    code_hash: &[u8; 32],
    abi_hash: &[u8; 32],
    manifest_provenance: Option<ManifestProvenance>,
) -> ProposalKind {
    ProposalKind::DeployContract(DeployContractProposal {
        contract_address: contract_address.clone(),
        code_hash: ContractCodeHash::new(*code_hash),
        abi_hash: ContractAbiHash::new(*abi_hash),
        abi_version: AbiVersion::new(1),
        manifest_provenance,
    })
}
fn sccp_route_governance_proposal_kind(
    anchor: &iroha_data_model::isi::bridge::SccpRouteGovernanceAnchorV1,
) -> ProposalKind {
    ProposalKind::SccpRouteGovernance(SccpRouteGovernanceProposal {
        anchor: Box::new(anchor.clone()),
    })
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
/// Response payload for GET /v1/gov/referenda/{id} Response payload for referendum lookup by id.
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
/// Returns `crate::Error::Query` unless the identifier is exact lowercase 32-byte hex.
pub async fn handle_gov_get_proposal(
    state: Arc<iroha_core::state::State>,
    id: axum::extract::Path<String>,
) -> Result<JsonBody<ProposalGetResponse>, crate::Error> {
    let id_arr = parse_exact_lower_hex32_path("proposal id", &id.0)?;
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
/// Returns a conversion error for a noncanonical referendum token. Missing locks are reported
/// with `found = false`.
pub async fn handle_gov_get_locks(
    state: Arc<iroha_core::state::State>,
    rid: axum::extract::Path<String>,
) -> Result<JsonBody<LocksGetResponse>, crate::Error> {
    let ref_id = rid.0;
    require_exact_governance_path_token("referendum id", &ref_id)?;
    reject_typed_proposal_ballot_selector(state.as_ref(), &ref_id)
        .map_err(crate::routing::conversion_error)?;
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
/// Returns a conversion error for a noncanonical referendum token. Missing referenda are returned
/// with `found = false`.
pub async fn handle_gov_get_referendum(
    state: Arc<iroha_core::state::State>,
    id: axum::extract::Path<String>,
) -> Result<JsonBody<ReferendumGetResponse>, crate::Error> {
    let rid = id.0;
    require_exact_governance_path_token("referendum id", &rid)?;
    reject_typed_proposal_ballot_selector(state.as_ref(), &rid)
        .map_err(crate::routing::conversion_error)?;
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
/// or the selector is the fingerprint of a stored typed proposal. Typed proposals
/// use the authenticated Parliament attempt-read API. Missing referenda return `NotFound`.
pub async fn handle_gov_get_tally(
    state: Arc<iroha_core::state::State>,
    id: axum::extract::Path<String>,
) -> Result<JsonBody<TallyGetResponse>, crate::Error> {
    let rid = id.0;
    require_exact_governance_path_token("referendum id", &rid)?;
    reject_typed_proposal_ballot_selector(state.as_ref(), &rid)
        .map_err(crate::routing::conversion_error)?;
    let world = state.world_view();
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
    // Project the current standalone referendum tally without mutating state.
    let now_h = state.committed_height() as u64;
    let mut approve: u128 = 0;
    let mut reject: u128 = 0;
    let mut abstain: u128 = 0;
    match referendum.mode {
        iroha_core::state::GovernanceReferendumMode::Plain => {
            let tally_height = if referendum.status
                == iroha_core::state::GovernanceReferendumStatus::Closed
                || now_h > referendum.h_end
            {
                referendum.h_end
            } else {
                now_h
            };
            if let Some(locks) = world.governance_locks().get(&rid) {
                let step = gov_cfg.conviction_step_blocks.max(1);
                let max_c = gov_cfg.max_conviction;
                for (_owner, rec) in locks.locks.iter() {
                    if rec.expiry_height < tally_height {
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
                    abstain = e.tally.get(2).copied().map_or(0, u128::from);
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
    state: Arc<iroha_core::state::State>,
    telemetry: MaybeTelemetry,
    NoritoJson(body): NoritoJson<ProtectedNamespacesDto>,
) -> Result<JsonBody<ProtectedNamespacesApplyResponse>, crate::Error> {
    use iroha_data_model::parameter::{CustomParameterId, Parameter, custom::CustomParameter};
    use std::str::FromStr as _;
    let namespaces: Vec<String> = body
        .namespaces
        .into_iter()
        .enumerate()
        .map(|(index, namespace)| {
            validate_exact_nonempty_token(&format!("namespaces[{index}]"), &namespace)?;
            if !namespace.is_ascii() {
                return Err(format!(
                    "namespaces[{index}] must contain only ASCII characters"
                ));
            }
            Ok(namespace)
        })
        .collect::<Result<_, String>>()
        .map_err(|message| crate::routing::conversion_error(message.into()))?;
    let namespace_count = namespaces.len();
    let name = iroha_data_model::name::Name::from_str("gov_protected_namespaces").map_err(|e| {
        crate::Error::Query(iroha_data_model::ValidationFail::InternalError(
            e.to_string(),
        ))
    })?;
    let id = CustomParameterId(name);
    // Convert Vec<String> -> Vec<&str> to satisfy Json's From<Vec<T>> bound
    let json_array = norito::json::native::Value::Array(
        namespaces
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
            state.network_id_ref(),
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
    use iroha_data_model::{name::Name, parameter::CustomParameterId};
    use std::str::FromStr as _;
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
/// POST /v1/gov/proposals/deploy-contract — build a proposal id and instruction skeleton.
///
/// Callers submit the returned instruction in a locally signed transaction.
///
/// # Errors
/// Returns `crate::Error::Query` when the contract target, hashes, or ABI version fails
/// validation.
pub async fn handle_gov_propose_deploy(
    state: Arc<iroha_core::state::State>,
    NoritoJson(body): NoritoJson<DeployContractProposalDraftRequestV1>,
) -> Result<JsonBody<DeployContractProposalDraftResponseV1>, crate::Error> {
    use iroha_data_model::isi::governance as gov;
    let contract_address = resolve_governance_contract_target(
        &state,
        body.contract_address.as_ref(),
        body.contract_alias.as_ref(),
    )?;
    if body.abi_version != AbiVersion::new(1) {
        return Err(crate::routing::conversion_error(format!(
            "unsupported abi_version: {}",
            body.abi_version
        )));
    }
    let code_hash_bytes = body.code_hash.into_bytes();
    let abi_hash_bytes = body.abi_hash.into_bytes();
    let expected_abi_hash = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
    if abi_hash_bytes != expected_abi_hash {
        return Err(crate::Error::Query(
            iroha_data_model::ValidationFail::QueryFailed(
                iroha_data_model::query::error::QueryExecutionFail::Conversion(format!(
                    "abi_hash does not match canonical hash for abi_version {}",
                    body.abi_version
                )),
            ),
        ));
    }
    let instr = gov::ProposeDeployContract {
        contract_address: contract_address.clone(),
        code_hash: body.code_hash,
        abi_hash: body.abi_hash,
        abi_version: body.abi_version,
        manifest_provenance: body.manifest_provenance.clone(),
    };
    let proposal_id = ProposalContentId::new(
        deploy_contract_proposal_kind(
            &instr.contract_address,
            &code_hash_bytes,
            &abi_hash_bytes,
            instr.manifest_provenance.clone(),
        )
        .fingerprint(),
    );
    Ok(JsonBody(DeployContractProposalDraftResponseV1 {
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
/// Returns `crate::Error::Query` when the action fails static validation.
pub async fn handle_gov_propose_sccp_route_governance(
    state: Arc<iroha_core::state::State>,
    NoritoJson(body): NoritoJson<SccpRouteGovernanceProposalDraftRequestV1>,
) -> Result<JsonBody<SccpRouteGovernanceProposalDraftResponseV1>, crate::Error> {
    use iroha_data_model::isi::governance as gov;
    body.action.validate_static().map_err(|error| {
        crate::routing::conversion_error(format!("invalid SCCP route governance action: {error}"))
    })?;
    let instr = gov::ProposeSccpRouteGovernance {
        anchor: iroha_data_model::isi::bridge::SccpRouteGovernanceAnchorV1 {
            network_id: *state.network_id_ref(),
            action: body.action,
        },
    };
    let proposal_kind = sccp_route_governance_proposal_kind(&instr.anchor);
    if let Some(reason) = proposal_kind.first_release_exact_json_u64_invariant_error() {
        return Err(crate::routing::conversion_error(reason.to_owned()));
    }
    let proposal_id = ProposalContentId::new(proposal_kind.fingerprint());
    Ok(JsonBody(SccpRouteGovernanceProposalDraftResponseV1 {
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
        state.network_id_ref(),
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
    let proposal_id = proposal_id.0;
    if !iroha_data_model::ministry::is_valid_agenda_proposal_id(&proposal_id) {
        return Err(crate::routing::conversion_error(
            "proposal_id must follow the exact AC-YYYY-### format".into(),
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
/// POST /v1/gov/ballot/plain — accept a plain quadratic ballot and build an instruction skeleton.
///
/// The request schema excludes private signing material; callers submit locally signed transactions.
///
/// # Errors
/// Returns `crate::Error::Query` when the ballot fields fail validation (direction, authority,
/// owner, amount parsing, or exact network mismatch).
pub async fn handle_gov_ballot_plain(
    state: Arc<iroha_core::state::State>,
    authenticated_account: &iroha_data_model::account::AccountId,
    NoritoJson(body): NoritoJson<PlainBallotDto>,
) -> Result<JsonBody<BallotSubmitResponse>, crate::Error> {
    handle_gov_ballot_plain_with_policy(
        state,
        authenticated_account,
        NoritoJson(body),
        MaybeTelemetry::disabled(),
    )
    .await
}
/// Variant of [`handle_gov_ballot_plain`] that allows callers to inject telemetry
/// policy, enabling address parsing coverage across Torii and tests.
pub async fn handle_gov_ballot_plain_with_policy(
    state: Arc<iroha_core::state::State>,
    authenticated_account: &iroha_data_model::account::AccountId,
    NoritoJson(body): NoritoJson<PlainBallotDto>,
    telemetry: MaybeTelemetry,
) -> Result<JsonBody<BallotSubmitResponse>, crate::Error> {
    ensure_network_id_matches(state.as_ref(), &body.network_id)?;
    let authority_id = parse_account_literal_from_state(
        state.as_ref(),
        body.authority.as_str(),
        &telemetry,
        CONTEXT_GOV_BALLOT_PLAIN_AUTHORITY,
    )
    .map_err(|err| {
        crate::routing::conversion_error(format!("invalid authority: {}", err.reason()))
    })?;
    ensure_authenticated_authority(authenticated_account, &authority_id)?;
    validate_governance_selector_v1("referendum_id", &body.referendum_id)
        .map_err(|message| crate::routing::conversion_error(message.into()))?;
    reject_typed_proposal_ballot_selector(state.as_ref(), &body.referendum_id)
        .map_err(|message| crate::routing::conversion_error(message.into()))?;
    // Basic shape validations
    if !(body.direction == "Aye" || body.direction == "Nay" || body.direction == "Abstain") {
        return Err(crate::routing::conversion_error("invalid direction".into()));
    }
    // Parse authority and owner; require equality for plain ballots
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
/// GET /v1/gov/council/current — fetch the latest persisted council membership.
///
/// # Errors
/// This handler never returns an error; empty councils are represented with an empty member list.
pub async fn handle_gov_council_current(
    state: Arc<iroha_core::state::State>,
) -> Result<JsonBody<CouncilCurrentResponse>, crate::Error> {
    let world = state.world_view();
    if let Some((epoch, council)) = world.council().last_key_value() {
        return Ok(JsonBody(CouncilCurrentResponse {
            epoch: *epoch,
            members: council
                .members
                .iter()
                .map(|account| CouncilMemberDto {
                    account_id: account.to_string(),
                })
                .collect(),
            alternates: council
                .alternates
                .iter()
                .map(|account| CouncilMemberDto {
                    account_id: account.to_string(),
                })
                .collect(),
            candidate_count: council.candidate_count as usize,
            derived_by: council.derived_by,
        }));
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
    use super::*;
    use crate::routing::MaybeTelemetry;
    use axum::body::Bytes;
    use iroha_config::parameters::actual::LaneConfig;
    use iroha_core::{
        block::BlockBuilder,
        kura::Kura,
        query::store::LiveQueryStore,
        queue::{Queue, TransactionGuard},
        smartcontracts::code::{activate_instance, register_code_bytes, register_manifest},
        state::{
            CouncilState, ElectionState, GovernanceLockCustody, GovernanceLockRecord,
            GovernanceLocksForReferendum, GovernanceProposalRecord, GovernanceProposalStatus,
            GovernanceReferendumMode, GovernanceReferendumRecord, GovernanceReferendumStatus,
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
    use std::sync::Arc;
    const ACCOUNT_AUTHORITY: &str = "sorauﾛ1NﾗhBUd2BﾂｦﾄiﾔﾆﾂﾇKSﾃaﾘﾒﾓQﾗrﾒoﾘﾅnｳﾘbQｳQJﾆLJ5HSE";
    const ACCOUNT_OWNER_ALT: &str = "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D";

    #[test]
    fn casting_context_route_holds_heavy_admission_through_blocking_replay() {
        let route_source = include_str!("lib.rs");
        let handler = route_source
            .split("async fn handler_gov_parliament_timed_ovn_casting_context_read")
            .nth(1)
            .and_then(|tail| {
                tail.split("async fn handler_gov_parliament_tle_release_context_read")
                    .next()
            })
            .expect("casting-context route source");
        let access = handler
            .find("check_access(")
            .expect("canonical access gate");
        let admission = handler
            .find("let replay_admission = acquire_query_admission(app.as_ref(), true).await?;")
            .expect("heavy admission gate");
        let blocking = handler
            .find("tokio::task::spawn_blocking(move ||")
            .expect("blocking replay isolation");
        let retained = handler
            .find("let _replay_admission = replay_admission;")
            .expect("permit retained by physical replay task");
        assert!(access < admission && admission < blocking && blocking < retained);
    }

    #[test]
    fn release_context_route_holds_heavy_admission_through_blocking_replay() {
        let route_source = include_str!("lib.rs");
        let handler = route_source
            .split("async fn handler_gov_parliament_tle_release_context_read")
            .nth(1)
            .and_then(|tail| {
                tail.split("async fn handler_gov_parliament_tle_partial_release")
                    .next()
            })
            .expect("release-context route source");
        let access = handler
            .find("check_access(")
            .expect("canonical access gate");
        let admission = handler
            .find("let replay_admission = acquire_query_admission(app.as_ref(), true).await?;")
            .expect("heavy admission gate");
        let blocking = handler
            .find("tokio::task::spawn_blocking(move ||")
            .expect("blocking replay isolation");
        let retained = handler
            .find("let _replay_admission = replay_admission;")
            .expect("permit retained by physical replay task");
        assert!(access < admission && admission < blocking && blocking < retained);
    }

    fn generic_lock_custody(state: &State) -> GovernanceLockCustody {
        GovernanceLockCustody {
            escrowed: !state.gov.min_bond_amount.is_zero(),
            asset_definition_id: state.gov.voting_asset_id.clone(),
            bond_escrow_account: state.gov.bond_escrow_account.clone(),
            slash_receiver_account: state.gov.slash_receiver_account.clone(),
        }
    }
    #[test]
    fn first_release_capabilities_expose_only_attempt_based_private_parliament() {
        assert_eq!(
            GOVERNANCE_APPROVAL_MODE_V1,
            "PARLIAMENT_ATTEMPT_TIMED_OVN_V1"
        );
        let source = include_str!("gov.rs");
        let retired_mode = ["LEGACY", "COUNCIL", "EPOCH"].join("_");
        let retired_resolver = ["fn governance", "approval", "mode"].join("_");
        assert!(!source.contains(&retired_mode));
        assert!(!source.contains(&retired_resolver));
        let capabilities_tail = &source[source
            .find("supported_routes: vec![")
            .expect("capability route projection")..];
        let capabilities = &capabilities_tail[..capabilities_tail
            .find("],\n    }))")
            .expect("capability route projection end")];
        assert!(capabilities.contains("/v1/gov/ballots/plain"));
        assert!(capabilities.contains("/v1/gov/ballots/zk-v1"));
        assert!(capabilities.contains(
            "/v1/gov/parliament/ballots/{ballot_attempt_id}/casting-proof"
        ));
        assert!(!capabilities.contains("\"/v1/gov/parliament/ballots\".to_owned()"));
        assert!(!capabilities.contains("/v1/gov/finalize"));
        assert!(!capabilities.contains("/v1/gov/enact"));
        assert!(capabilities.contains("/v1/gov/parliament/attempts/draft"));
        assert!(capabilities.contains("/v1/gov/parliament/transitions/draft"));
    }
    #[tokio::test]
    async fn parliament_draft_handlers_frame_exact_native_instructions() {
        use iroha_data_model::{
            governance::types::{
                AbiVersion, ContractAbiHash, ContractCodeHash, DeployContractProposal,
                GovernanceAttemptId, ProposalKind,
            },
            isi::{
                Instruction as _,
                governance::{
                    CreateParliamentGovernanceAttemptV1, ParliamentLifecycleTransitionV1,
                    SubmitParliamentLifecycleTransitionV1,
                },
            },
        };

        let attempt_request = ParliamentAttemptDraftRequestV1 {
            version: PARLIAMENT_API_VERSION_V1,
            proposal: ProposalKind::DeployContract(DeployContractProposal {
                contract_address: sample_contract_address(),
                code_hash: ContractCodeHash::new([0x11; 32]),
                abi_hash: ContractAbiHash::new([0x22; 32]),
                abi_version: AbiVersion::new(1),
                manifest_provenance: None,
            }),
            attempt_sequence: 4,
        };
        let attempt_response = handle_gov_parliament_attempt_draft(NoritoJson(attempt_request))
            .await
            .expect("draft exact Parliament attempt")
            .0;
        assert_eq!(attempt_response.tx_instructions.len(), 1);
        let attempt_draft = &attempt_response.tx_instructions[0];
        let attempt_instruction = iroha_data_model::isi::decode_instruction_from_pair(
            &attempt_draft.wire_id,
            &hex::decode(&attempt_draft.payload_hex).expect("attempt payload hex"),
        )
        .expect("decode exact Parliament attempt instruction");
        let attempt_instruction = attempt_instruction
            .as_any()
            .downcast_ref::<CreateParliamentGovernanceAttemptV1>()
            .expect("exact attempt instruction type");
        assert_eq!(
            attempt_response.governance_attempt_id,
            attempt_instruction.governance_attempt_id()
        );

        let transition_request = ParliamentTransitionDraftRequestV1 {
            version: PARLIAMENT_API_VERSION_V1,
            governance_attempt_id: GovernanceAttemptId::new([0x33; 32]),
            transition: ParliamentLifecycleTransitionV1::CompleteQualification,
        };
        let expected_digest = transition_request.transition.digest_v1();
        let transition_response =
            handle_gov_parliament_transition_draft(NoritoJson(transition_request))
                .await
                .expect("draft exact Parliament transition")
                .0;
        assert_eq!(transition_response.transition_digest, expected_digest);
        let transition_draft = &transition_response.tx_instructions[0];
        let transition_instruction = iroha_data_model::isi::decode_instruction_from_pair(
            &transition_draft.wire_id,
            &hex::decode(&transition_draft.payload_hex).expect("transition payload hex"),
        )
        .expect("decode exact Parliament transition instruction");
        let transition_instruction = transition_instruction
            .as_any()
            .downcast_ref::<SubmitParliamentLifecycleTransitionV1>()
            .expect("exact transition instruction type");
        assert_eq!(
            transition_instruction.transition.digest_v1(),
            expected_digest
        );
    }
    #[test]
    fn unlock_stats_handler_cannot_reintroduce_an_expiry_index_scan() {
        let source = include_str!("gov.rs");
        let start = source
            .find("pub async fn handle_gov_unlock_stats(")
            .expect("unlock stats handler");
        let tail = &source[start..];
        let end = tail
            .find("pub struct TxInstr")
            .expect("unlock stats handler terminator");
        let implementation = &tail[..end];
        assert!(implementation.contains("let view = state.query_view();"));
        assert!(implementation.contains("view.height()"));
        assert!(implementation.contains("governance_unlock_stats()"));
        assert!(!implementation.contains("governance_lock_expiry_index()"));
        assert!(!implementation.contains(".range("));
    }
    #[test]
    fn scalar_governance_handlers_cannot_reintroduce_history_scans() {
        let source = include_str!("gov.rs");
        let citizen_start = source
            .find("pub async fn handle_gov_citizen_count(")
            .expect("citizen count handler");
        let citizen_tail = &source[citizen_start..];
        let citizen_end = citizen_tail
            .find("/// GET /v1/gov/citizens/{account_id}")
            .expect("citizen count handler terminator");
        let citizen_handler = &citizen_tail[..citizen_end];
        assert!(citizen_handler.contains("world.citizens().len()"));
        assert!(!citizen_handler.contains("citizens().iter()"));
        let council_start = source
            .find("pub async fn handle_gov_council_current(")
            .expect("current council handler");
        let council_tail = &source[council_start..];
        let council_end = council_tail
            .find("#[cfg(test)]\nmod tests")
            .expect("current council handler terminator");
        let council_handler = &council_tail[..council_end];
        assert!(council_handler.contains("world.council().last_key_value()"));
        assert!(!council_handler.contains("world.council().iter()"));
        assert!(!council_handler.contains(".max_by"));
        assert!(!council_handler.contains(".max_by_key"));
    }
    #[test]
    fn optional_ballot_direction_is_closed() {
        for direction in [None, Some("Aye"), Some("Nay"), Some("Abstain")] {
            validate_optional_ballot_direction(direction).expect("canonical direction");
        }
        assert_eq!(
            validate_optional_ballot_direction(Some("aye")),
            Err("direction must be Aye, Nay, or Abstain".to_owned())
        );
        assert_eq!(
            validate_optional_ballot_direction(Some("Approve")),
            Err("direction must be Aye, Nay, or Abstain".to_owned())
        );
    }
    #[test]
    fn canonicalize_hex32_value_accepts_only_declared_wire_forms() {
        let uppercase = "AB".repeat(32);
        let expected = "ab".repeat(32);
        for literal in [
            uppercase.clone(),
            format!("0X{uppercase}"),
            format!("BlAkE2b32:{uppercase}"),
            format!("BLAKE2B32:0x{uppercase}"),
        ] {
            assert_eq!(canonicalize_hex32_value(&literal), Some(expected.clone()));
        }
        for literal in [
            format!(":{uppercase}"),
            format!(" {uppercase}"),
            format!("{uppercase} "),
            format!("sha256:{uppercase}"),
            "ab".repeat(31),
        ] {
            assert_eq!(canonicalize_hex32_value(&literal), None);
        }
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
    fn canonical_account(raw: &str) -> AccountId {
        AccountId::parse_encoded(raw)
            .expect("literal parses")
            .into_account_id()
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
    fn seed_typed_proposal_fingerprint_for_ballot_test(
        state: &Arc<State>,
        proposer: &AccountId,
    ) -> String {
        let kind = deploy_contract_proposal_kind(
            &sample_contract_address(),
            &[0x71; 32],
            &[0x72; 32],
            None,
        );
        let proposal_id = kind.fingerprint();
        let record = iroha_core::state::GovernanceProposalRecord {
            proposer: proposer.clone(),
            kind,
            created_height: 1,
            status: iroha_core::state::GovernanceProposalStatus::Proposed,
        };
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        transaction
            .world_mut_for_testing()
            .governance_proposals_mut()
            .insert(proposal_id, record);
        transaction.apply();
        block
            .commit()
            .expect("commit typed proposal ballot guard fixture");
        hex::encode(proposal_id)
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
        let asset_def_id: AssetDefinitionId = AssetDefinitionId::derive_from_components(
            domain_id.clone(),
            Name::from_str("vote").expect("asset definition name"),
        );
        let asset_def = {
            let __asset_definition_id = asset_def_id.clone();
            AssetDefinition::numeric(
                __asset_definition_id.clone(),
                "vote".to_owned(),
                iroha_data_model::asset::AssetBalancePolicy::Global,
                None,
            )
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
                &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                    .parse()
                    .expect("canonical test network id"),
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
            let register_contract: Permission =
                iroha_executor_data_model::permission::smart_contract::CanRegisterSmartContractCode
                    .into();
            let mut world_block = world.block();
            let mut world_tx = world_block.transaction_without_telemetry(LaneConfig::default(), 0);
            let _ = world_tx.add_account_permission(&authority, propose);
            let _ = world_tx.add_account_permission(&authority, ballot);
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
        "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw"
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
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
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
    fn decode_governance_proposal_instruction(
        instr: &GovernanceProposalInstructionDraftV1,
    ) -> iroha_data_model::isi::InstructionBox {
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
            *harness.state.network_id_ref(),
            harness.authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(instructions)
        .sign(harness.authority_keypair.private_key());
        let params = harness.state.view().world().parameters().clone();
        let accepted = iroha_core::tx::AcceptedTransaction::accept(
            tx,
            harness.state.network_id_ref(),
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
    fn queue_governance_proposal_instruction_skeleton(
        harness: &GovHarness,
        tx_instructions: &[GovernanceProposalInstructionDraftV1],
    ) {
        let instructions = tx_instructions
            .iter()
            .map(decode_governance_proposal_instruction)
            .collect::<Vec<_>>();
        let tx = iroha_data_model::transaction::signed::TransactionBuilder::new(
            *harness.state.network_id_ref(),
            harness.authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions(instructions)
        .sign(harness.authority_keypair.private_key());
        let params = harness.state.view().world().parameters().clone();
        let accepted = iroha_core::tx::AcceptedTransaction::accept(
            tx,
            harness.state.network_id_ref(),
            params.sumeragi().max_clock_drift(),
            params.transaction(),
            harness.state.crypto().as_ref(),
        )
        .expect("accepted governance proposal instruction skeleton");
        harness
            .queue
            .push(accepted, harness.state.view())
            .expect("push governance proposal instruction skeleton");
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
            *harness.state.network_id_ref(),
            harness.authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction])
        .sign(harness.authority_keypair.private_key());
        let params = harness.state.view().world().parameters().clone();
        let accepted = iroha_core::tx::AcceptedTransaction::accept(
            tx,
            harness.state.network_id_ref(),
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
            *harness.state.network_id_ref(),
            harness.authority.clone(),
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([instruction])
        .sign(harness.authority_keypair.private_key());
        let params = harness.state.view().world().parameters().clone();
        let accepted = iroha_core::tx::AcceptedTransaction::accept(
            tx,
            harness.state.network_id_ref(),
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
    #[tokio::test]
    async fn council_current_projects_latest_epoch_from_multi_epoch_history() {
        let (state, _queue, _chain_id) = mk_basic_context();
        let latest_member = AccountId::of(
            checked_governance_ed25519_keypair(0xA4)
                .public_key()
                .clone(),
        );
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let mut block = state.block(header);
        let mut transaction = block.transaction();
        for epoch in 0_u64..128 {
            transaction.world.council_mut().insert(
                epoch,
                CouncilState {
                    epoch,
                    members: vec![if epoch == 127 {
                        latest_member.clone()
                    } else {
                        ALICE_ID.clone()
                    }],
                    candidate_count: 1,
                    ..CouncilState::default()
                },
            );
        }
        transaction.apply();
        block.commit().expect("commit multi-epoch council history");
        let response = handle_gov_council_current(state)
            .await
            .expect("current council response")
            .0;
        assert_eq!(response.epoch, 127);
        assert_eq!(response.candidate_count, 1);
        assert_eq!(response.members.len(), 1);
        assert_eq!(response.members[0].account_id, latest_member.to_string());
    }
    #[test]
    fn serde_shapes_compile() {
        let canonical_abi = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        let req = DeployContractProposalDraftRequestV1 {
            contract_address: Some(sample_contract_address()),
            contract_alias: None,
            abi_version: AbiVersion::new(1),
            code_hash: ContractCodeHash::new([0xAA; 32]),
            abi_hash: ContractAbiHash::new(canonical_abi),
            manifest_provenance: None,
        };
        let s = norito::json::to_json(&req).unwrap();
        let _: DeployContractProposalDraftRequestV1 = norito::json::from_str(&s).unwrap();
        let sccp = SccpRouteGovernanceProposalDraftRequestV1 {
            action: sample_sccp_route_governance_action(),
        };
        let json = norito::json::to_json(&sccp).expect("encode SCCP governance DTO");
        let decoded: SccpRouteGovernanceProposalDraftRequestV1 =
            norito::json::from_str(&json).expect("decode SCCP governance DTO");
        assert_eq!(decoded.action, sccp.action);
    }
    #[tokio::test]
    async fn protected_namespaces_set_drafts_transaction_without_mutating_state() {
        let (state, _queue, _chain_id) = mk_basic_context();
        let before = handle_gov_protected_get(state.clone())
            .await
            .expect("protected namespaces get")
            .0;
        assert!(!before.found);
        assert!(before.namespaces.is_empty());
        let response = handle_gov_protected_set(
            state.clone(),
            MaybeTelemetry::disabled(),
            NoritoJson(ProtectedNamespacesDto {
                namespaces: vec!["apps".to_owned(), "system".to_owned()],
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
    async fn protected_namespaces_rejects_noncanonical_tokens_before_drafting() {
        let (state, _queue, _chain_id) = mk_basic_context();
        for namespace in ["", " system", "system ", "system namespace", "systèm"] {
            let error = handle_gov_protected_set(
                state.clone(),
                MaybeTelemetry::disabled(),
                NoritoJson(ProtectedNamespacesDto {
                    namespaces: vec![namespace.to_owned()],
                    authority: None,
                }),
            )
            .await
            .expect_err("noncanonical namespace must fail before drafting");
            assert!(error.to_string().contains("namespaces[0]"));
        }
    }
    #[tokio::test]
    async fn protected_namespaces_set_returns_checked_signable_payload_for_authority() {
        let harness = mk_governance_harness(false);
        let response = handle_gov_protected_set(
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
    async fn propose_deploy_builds_instruction_skeleton() {
        let (state, _queue, _chain_id) = mk_basic_context();
        let code_hash_bytes = [0x11; 32];
        let canonical_abi = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        let provenance_key =
            KeyPair::try_from_seed(b"proposal-id-provenance".to_vec(), Algorithm::Ed25519)
                .expect("derive proposal provenance fixture key");
        let provenance = mk_manifest_provenance(&provenance_key, [0x11; 32], canonical_abi);
        let dto = DeployContractProposalDraftRequestV1 {
            contract_address: Some(sample_contract_address()),
            contract_alias: None,
            abi_version: AbiVersion::new(1),
            code_hash: ContractCodeHash::new(code_hash_bytes),
            abi_hash: ContractAbiHash::new(canonical_abi),
            manifest_provenance: Some(provenance.clone()),
        };
        let res = handle_gov_propose_deploy(state, NoritoJson(dto))
            .await
            .expect("handler ok");
        let body = res.0;
        assert_eq!(body.tx_instructions.len(), 1);
        let expected_id = deploy_contract_proposal_kind(
            &sample_contract_address(),
            &code_hash_bytes,
            &canonical_abi,
            Some(provenance.clone()),
        )
        .fingerprint();
        assert_eq!(body.proposal_id, ProposalContentId::new(expected_id));
        assert_ne!(
            expected_id,
            deploy_contract_proposal_kind(
                &sample_contract_address(),
                &code_hash_bytes,
                &canonical_abi,
                None,
            )
            .fingerprint(),
            "proposal id must bind the complete manifest provenance"
        );
        // Payload decodes to the exact certificate-only proposal instruction.
        let instruction = decode_governance_proposal_instruction(&body.tx_instructions[0]);
        let decoded = instruction
            .as_any()
            .downcast_ref::<iroha_data_model::isi::governance::ProposeDeployContract>()
            .expect("exact deploy-contract proposal instruction");
        assert_eq!(decoded.contract_address, sample_contract_address());
        assert_eq!(decoded.code_hash, ContractCodeHash::new(code_hash_bytes));
        assert_eq!(decoded.abi_hash, ContractAbiHash::new(canonical_abi));
        assert_eq!(decoded.abi_version, AbiVersion::new(1));
        assert_eq!(decoded.manifest_provenance, Some(provenance));
    }
    #[tokio::test]
    async fn propose_sccp_route_governance_builds_exact_instruction_and_proposal_id() {
        let (state, _queue, _chain_id) = mk_basic_context();
        let action = sample_sccp_route_governance_action();
        let anchor = iroha_data_model::isi::bridge::SccpRouteGovernanceAnchorV1 {
            network_id: *state.network_id_ref(),
            action: action.clone(),
        };
        let expected_id = sccp_route_governance_proposal_kind(&anchor).fingerprint();
        let response = handle_gov_propose_sccp_route_governance(
            state,
            NoritoJson(SccpRouteGovernanceProposalDraftRequestV1 {
                action: action.clone(),
            }),
        )
        .await
        .expect("valid SCCP governance draft")
        .0;
        assert_eq!(response.proposal_id, ProposalContentId::new(expected_id));
        assert_eq!(response.tx_instructions.len(), 1);
        let instruction = decode_governance_proposal_instruction(&response.tx_instructions[0]);
        let decoded = instruction
            .as_any()
            .downcast_ref::<iroha_data_model::isi::governance::ProposeSccpRouteGovernance>()
            .expect("exact SCCP governance instruction");
        assert_eq!(decoded.anchor, anchor);
    }
    #[tokio::test]
    async fn propose_sccp_route_governance_rejects_invalid_action_before_drafting() {
        let (state, _queue, _chain_id) = mk_basic_context();
        let mut action = sample_sccp_route_governance_action();
        let iroha_data_model::isi::bridge::SccpRouteGovernanceActionV1::Remove(key) = &mut action
        else {
            unreachable!("fixture is a remove action");
        };
        key.revision = 0;
        let error = handle_gov_propose_sccp_route_governance(
            state,
            NoritoJson(SccpRouteGovernanceProposalDraftRequestV1 { action }),
        )
        .await
        .expect_err("invalid SCCP action must fail before returning a skeleton");
        assert!(
            format!("{error:?}").contains("invalid SCCP route governance action"),
            "unexpected error: {error:?}"
        );
    }
    #[tokio::test]
    async fn propose_sccp_route_governance_rejects_inexact_json_numbers_before_drafting() {
        let (state, _queue, _chain_id) = mk_basic_context();
        let iroha_data_model::isi::bridge::SccpRouteGovernanceActionV1::Remove(key) =
            sample_sccp_route_governance_action()
        else {
            unreachable!("fixture is a remove action")
        };
        let action = iroha_data_model::isi::bridge::SccpRouteGovernanceActionV1::SetActivation(
            iroha_data_model::isi::bridge::SccpSetRouteActivationV1 {
                key,
                expected_current: iroha_data_model::bridge::SccpRouteActivationV1::InboundOnly,
                next: iroha_data_model::bridge::SccpRouteActivationV1::Retired,
                inbound_finality_cutoff: Some(
                    iroha_data_model::bridge::SccpInboundFinalityCutoffV1 {
                        trust_anchor_hash: [0x91; 32],
                        max_anchor_interval_height:
                            iroha_data_model::parliament_types::FIRST_RELEASE_MAX_EXACT_JSON_U64 + 1,
                    },
                ),
            },
        );
        assert!(action.validate_static().is_ok());
        let error = handle_gov_propose_sccp_route_governance(
            state,
            NoritoJson(SccpRouteGovernanceProposalDraftRequestV1 { action }),
        )
        .await
        .expect_err("inexact SCCP JSON numbers must fail before returning a skeleton");
        assert!(
            format!("{error:?}").contains("exact JSON integer maximum"),
            "unexpected precision rejection: {error:?}"
        );
    }
    #[test]
    fn propose_sccp_route_governance_rejects_retired_lifecycle_controls() {
        let canonical = norito::json::to_json(&SccpRouteGovernanceProposalDraftRequestV1 {
            action: sample_sccp_route_governance_action(),
        })
        .expect("canonical SCCP governance DTO");
        let body = canonical.strip_suffix('}').expect("DTO JSON is an object");
        for (field, value) in [
            ("mode", "\"Zk\""),
            ("window", "{\"lower\":10,\"upper\":20}"),
        ] {
            let injected = format!("{body},\"{field}\":{value}}}");
            let error =
                norito::json::from_str::<SccpRouteGovernanceProposalDraftRequestV1>(&injected)
                    .expect_err("retired SCCP lifecycle control must reject");
            assert!(error.to_string().contains(field), "{field}: {error}");
        }
    }
    #[test]
    fn sccp_route_governance_dto_rejects_retired_signing_and_unknown_fields() {
        let dto = SccpRouteGovernanceProposalDraftRequestV1 {
            action: sample_sccp_route_governance_action(),
        };
        let canonical = norito::json::to_json(&dto).expect("canonical SCCP governance DTO");
        let body = canonical.strip_suffix('}').expect("DTO JSON is an object");
        for (field, value) in [
            ("authority", "\"sorau...\""),
            ("private_key", "\"secret\""),
            ("manifest", "null"),
            ("window", "null"),
            ("mode", "\"Zk\""),
            ("future_action_policy", "null"),
        ] {
            let injected = format!("{body},\"{field}\":{value}}}");
            let error =
                norito::json::from_str::<SccpRouteGovernanceProposalDraftRequestV1>(&injected)
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
            ($field:expr; $($request:ty),+ $(,)?) => {
                $(
                    let input = format!(r#"{{"{}":"must-not-cross-torii"}}"#, $field);
                    let error = norito::json::from_str::<$request>(&input)
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
        for field in [
            "private_key",
            "privateKey",
            "private_key_hex",
            "privateKeyHex",
            "private_key_bytes",
            "privateKeyBytes",
            "private_key_seed",
            "privateKeySeed",
            "private_key_multihash",
            "privateKeyMultihash",
            "private_key_algorithm",
            "privateKeyAlgorithm",
        ] {
            assert_rejects_field!(
                field;
                DeployContractProposalDraftRequestV1,
                SccpRouteGovernanceProposalDraftRequestV1,
                MinistryAgendaProposalDraftDto,
                PlainBallotDto,
                ZkBallotV1Dto,
                ZkBallotV1BallotProofDto,
                ProtectedNamespacesDto,
            );
        }
        assert_rejects_field!(
            "authority";
            DeployContractProposalDraftRequestV1,
        );
    }
    #[test]
    fn governance_nested_request_types_reject_unknown_fields() {
        let ballot = iroha_data_model::isi::governance::BallotProof {
            backend: "halo2/ipa".into(),
            envelope_bytes: vec![1, 2, 3, 4],
            root_hint: None,
            owner: None,
            nullifier: None,
            amount: None,
            duration_blocks: None,
            direction: None,
        };
        let canonical = norito::json::to_json(&ballot).expect("encode canonical ballot proof");
        let body = canonical
            .strip_suffix('}')
            .expect("ballot proof JSON is an object");
        let injected = format!(r#"{body},"privateKeySeed":"secret"}}"#);
        let error =
            norito::json::from_str::<iroha_data_model::isi::governance::BallotProof>(&injected)
                .expect_err("ballot proof must be closed");
        assert!(error.to_string().contains("privateKeySeed"));
        let keypair =
            KeyPair::try_from_seed(b"closed-manifest-provenance".to_vec(), Algorithm::Ed25519)
                .expect("derive manifest provenance fixture key");
        let provenance = mk_manifest_provenance(&keypair, [0x11; 32], [0x22; 32]);
        let canonical =
            norito::json::to_json(&provenance).expect("encode canonical manifest provenance");
        let body = canonical
            .strip_suffix('}')
            .expect("manifest provenance JSON is an object");
        let injected = format!(r#"{body},"privateKeyAlgorithm":"secret"}}"#);
        let error = norito::json::from_str::<ManifestProvenance>(&injected)
            .expect_err("manifest provenance must be closed");
        assert!(error.to_string().contains("privateKeyAlgorithm"));
    }
    #[test]
    fn propose_deploy_rejects_retired_lifecycle_controls_during_decode() {
        let canonical_abi = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        let dto = DeployContractProposalDraftRequestV1 {
            contract_address: Some(sample_contract_address()),
            contract_alias: None,
            abi_version: AbiVersion::new(1),
            code_hash: ContractCodeHash::new([0x11; 32]),
            abi_hash: ContractAbiHash::new(canonical_abi),
            manifest_provenance: None,
        };
        let canonical = norito::json::to_json(&dto).expect("canonical deploy request");
        let body = canonical.strip_suffix('}').expect("DTO JSON is an object");
        for (field, value) in [
            ("mode", "\"Zk\""),
            ("window", "{\"lower\":10,\"upper\":20}"),
        ] {
            let injected = format!("{body},\"{field}\":{value}}}");
            let error = norito::json::from_str::<DeployContractProposalDraftRequestV1>(&injected)
                .expect_err("retired deploy lifecycle control must fail typed decoding");
            assert!(error.to_string().contains(field), "{field}: {error}");
        }
    }
    #[tokio::test]
    async fn propose_deploy_accepts_only_exact_abi_v1_label() {
        let (state, _queue, _chain_id) = mk_basic_context();
        let canonical_abi = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        for abi_version in [0, 2, u16::MAX] {
            let dto = DeployContractProposalDraftRequestV1 {
                contract_address: Some(sample_contract_address()),
                contract_alias: None,
                abi_version: AbiVersion::new(abi_version),
                code_hash: ContractCodeHash::new([0x11; 32]),
                abi_hash: ContractAbiHash::new(canonical_abi),
                manifest_provenance: None,
            };
            let error = handle_gov_propose_deploy(state.clone(), NoritoJson(dto))
                .await
                .expect_err("only the exact first-release ABI label is accepted");
            assert!(
                format!("{error:?}").contains("unsupported abi_version"),
                "{abi_version}: {error:?}"
            );
        }
    }
    #[tokio::test]
    async fn propose_deploy_rejects_mismatched_abi_hash() {
        let (state, _queue, _chain_id) = mk_basic_context();
        let dto = DeployContractProposalDraftRequestV1 {
            contract_address: Some(sample_contract_address()),
            contract_alias: None,
            abi_version: AbiVersion::new(1),
            code_hash: ContractCodeHash::new([0x11; 32]),
            abi_hash: ContractAbiHash::new([0x22; 32]),
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
    async fn ministry_agenda_draft_rejects_noncanonical_authority_without_trimming() {
        let harness = mk_governance_harness(true);
        let authority = format!(" {}", harness.authority);
        let error = handle_ministry_agenda_proposal_draft(
            harness.state,
            MaybeTelemetry::disabled(),
            NoritoJson(MinistryAgendaProposalDraftDto {
                proposal: sample_agenda_proposal("AC-2026-240"),
                authority,
            }),
        )
        .await
        .expect_err("whitespace authority alias must fail before drafting");
        assert!(
            format!("{error:?}").contains("canonical I105"),
            "unexpected error: {error:?}"
        );
    }
    #[tokio::test]
    async fn ministry_agenda_get_returns_missing_then_persisted_record() {
        let harness = mk_governance_harness(true);
        let proposal = sample_agenda_proposal("AC-2026-242");
        for invalid in [" AC-2026-242", "AC-2026-242 ", "ac-2026-242", "AC-2026-42"] {
            let error = handle_ministry_agenda_proposal_get(
                harness.state.clone(),
                axum::extract::Path(invalid.to_owned()),
            )
            .await
            .expect_err("noncanonical proposal id must fail before lookup");
            assert!(format!("{error:?}").contains("AC-YYYY-###"));
        }
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
        let (state, _queue, _chain_id) = mk_basic_context();
        let authenticated = canonical_account(ACCOUNT_AUTHORITY);
        let canonical = canonical_literal(ACCOUNT_AUTHORITY);
        // Build DTO via JSON to ensure serde shape is satisfied
        let body = crate::json_object(vec![
            crate::json_entry("authority", canonical.clone()),
            crate::json_entry("network_id", *state.network_id_ref()),
            crate::json_entry("referendum_id", "r1"),
            crate::json_entry("owner", canonical.clone()),
            crate::json_entry("amount", "100"),
            crate::json_entry("duration_blocks", "600"),
            crate::json_entry("direction", "Aye"),
        ]);
        let parsed: PlainBallotDto =
            norito::json::from_str(&norito::json::to_json(&body).unwrap()).unwrap();
        let res = handle_gov_ballot_plain_with_policy(
            state,
            &authenticated,
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
    async fn standalone_plain_ballot_rejects_stored_typed_proposal_fingerprint() {
        let (state, _queue, _chain_id) = mk_basic_context();
        let authenticated = canonical_account(ACCOUNT_AUTHORITY);
        let canonical = canonical_literal(ACCOUNT_AUTHORITY);
        let proposal_id = seed_typed_proposal_fingerprint_for_ballot_test(&state, &authenticated);
        let dto = PlainBallotDto {
            authority: canonical.clone(),
            network_id: *state.network_id_ref(),
            referendum_id: proposal_id,
            owner: canonical,
            amount: 100_u64.into(),
            duration_blocks: "600".to_owned(),
            direction: "Aye".to_owned(),
        };
        let error = handle_gov_ballot_plain_with_policy(
            state,
            &authenticated,
            NoritoJson(dto),
            MaybeTelemetry::disabled(),
        )
        .await
        .expect_err("typed proposal must not enter the standalone plain ballot path");
        assert!(
            error
                .to_string()
                .contains("authenticated Parliament lifecycle"),
            "unexpected error: {error:?}"
        );
    }
    #[tokio::test]
    async fn ballot_plain_accepts_account_aliases() {
        let (state, _queue, _chain_id) = mk_basic_context();
        let authority = AccountId::parse_encoded(ACCOUNT_AUTHORITY)
            .expect("account parses")
            .into_account_id();
        bind_account_alias_for_test(&state, &authority, "ballot@universal");
        let body = crate::json_object(vec![
            crate::json_entry("authority", "ballot@universal"),
            crate::json_entry("network_id", *state.network_id_ref()),
            crate::json_entry("referendum_id", "r1"),
            crate::json_entry("owner", "ballot@universal"),
            crate::json_entry("amount", "100"),
            crate::json_entry("duration_blocks", "600"),
            crate::json_entry("direction", "Aye"),
        ]);
        let parsed: PlainBallotDto =
            norito::json::from_str(&norito::json::to_json(&body).unwrap()).unwrap();
        let res = handle_gov_ballot_plain_with_policy(
            state,
            &authority,
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
        let (state, _queue, _chain_id) = mk_basic_context();
        let authenticated = canonical_account(ACCOUNT_AUTHORITY);
        let canonical_authority = canonical_literal(ACCOUNT_AUTHORITY);
        let canonical_owner = canonical_literal(ACCOUNT_OWNER_ALT);
        let body = crate::json_object(vec![
            crate::json_entry("authority", canonical_authority),
            crate::json_entry("network_id", *state.network_id_ref()),
            crate::json_entry("referendum_id", "r1"),
            crate::json_entry("owner", canonical_owner),
            crate::json_entry("amount", "100"),
            crate::json_entry("duration_blocks", "600"),
            crate::json_entry("direction", "Aye"),
        ]);
        let parsed: PlainBallotDto =
            norito::json::from_str(&norito::json::to_json(&body).unwrap()).unwrap();
        let err = handle_gov_ballot_plain_with_policy(
            state,
            &authenticated,
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
        let (state, _queue, _chain_id) = mk_basic_context();
        let authenticated = canonical_account(ACCOUNT_AUTHORITY);
        let body = crate::json_object(vec![
            crate::json_entry("authority", ACCOUNT_AUTHORITY),
            crate::json_entry("network_id", *state.network_id_ref()),
            crate::json_entry("referendum_id", "r1"),
            crate::json_entry("owner", ACCOUNT_AUTHORITY),
            crate::json_entry("amount", "100"),
            crate::json_entry("duration_blocks", "600"),
            crate::json_entry("direction", "Aye"),
        ]);
        let parsed: PlainBallotDto =
            norito::json::from_str(&norito::json::to_json(&body).unwrap()).unwrap();
        handle_gov_ballot_plain_with_policy(
            state,
            &authenticated,
            NoritoJson(parsed),
            MaybeTelemetry::for_tests(),
        )
        .await
        .expect("raw public key literals should be accepted");
    }
    include!("gov/network_id_tests.rs");
    #[test]
    fn exact_governance_path_token_grammar_rejects_aliasing_characters() {
        for valid in ["referendum-1", "A9_selector~with.dots"] {
            validate_governance_selector_v1("referendum id", valid)
                .expect("a bounded RFC 3986 unreserved selector is valid");
        }
        for invalid in [
            "",
            ".",
            "..",
            ".hidden",
            "a/b",
            "a%2Fb",
            "投票",
            " referendum",
            "referendum ",
            "refer\nendum",
            "refer\u{7f}endum",
        ] {
            validate_governance_selector_v1("referendum id", invalid)
                .expect_err("noncanonical path selectors must fail closed");
        }
        validate_governance_selector_v1("referendum id", &"a".repeat(128))
            .expect("the exact length boundary is valid");
        validate_governance_selector_v1("referendum id", &"a".repeat(129))
            .expect_err("overlong selectors must fail closed");
    }
    #[tokio::test]
    async fn governance_get_handlers_reject_noncanonical_selectors_before_lookup() {
        fn assert_conversion(error: &crate::Error) {
            assert!(
                matches!(
                    error,
                    crate::Error::Query(iroha_data_model::ValidationFail::QueryFailed(
                        iroha_data_model::query::error::QueryExecutionFail::Conversion(_)
                    ))
                ),
                "expected query conversion error, got {error:?}"
            );
        }
        let state = Arc::new(State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        for invalid in ["AA".repeat(32), format!("0x{}", "aa".repeat(32))] {
            let error = handle_gov_get_proposal(state.clone(), axum::extract::Path(invalid))
                .await
                .expect_err("proposal aliases must fail before lookup");
            assert_conversion(&error);
        }
        let missing = handle_gov_get_proposal(state.clone(), axum::extract::Path("aa".repeat(32)))
            .await
            .expect("an exact lowercase proposal id reaches lookup");
        assert!(!missing.0.found);
        for invalid in [
            "a/b".to_owned(),
            ".".to_owned(),
            ".hidden".to_owned(),
            "a%2Fb".to_owned(),
            "投票".to_owned(),
            "a".repeat(129),
        ] {
            let error =
                handle_gov_get_referendum(state.clone(), axum::extract::Path(invalid.clone()))
                    .await
                    .expect_err("noncanonical selectors must fail before referendum lookup");
            assert_conversion(&error);
        }
        let error =
            handle_gov_get_locks(state.clone(), axum::extract::Path(" referendum".to_owned()))
                .await
                .expect_err("leading whitespace must fail before lock lookup");
        assert_conversion(&error);
        let error =
            handle_gov_get_referendum(state.clone(), axum::extract::Path("referendum ".to_owned()))
                .await
                .expect_err("trailing whitespace must fail before referendum lookup");
        assert_conversion(&error);
        let error = handle_gov_get_tally(state, axum::extract::Path("refer\nendum".to_owned()))
            .await
            .expect_err("control characters must fail before tally lookup");
        assert_conversion(&error);
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
        let custody = generic_lock_custody(&state);
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
                    custody,
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
    async fn gov_get_tally_uses_referendum_end_for_closed_plain_view() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let mut state = State::new_for_testing(World::default(), kura, query);
        let mut cfg = state.gov.clone();
        cfg.conviction_step_blocks = 1;
        cfg.max_conviction = 1;
        state.set_gov(cfg);
        let rid = "rid-tally-closed-lock".to_string();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        let block_hash = iroha_crypto::HashOf::new(&header);
        {
            let mut block = state.block(header);
            let mut tx = block.transaction();
            tx.world.governance_referenda_mut().insert(
                rid.clone(),
                GovernanceReferendumRecord {
                    h_start: 0,
                    h_end: 0,
                    status: GovernanceReferendumStatus::Closed,
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
                    expiry_height: 0,
                    direction: 0,
                    duration_blocks: 0,
                    custody: None,
                },
            );
            tx.world.governance_locks_mut().insert(rid.clone(), locks);
            tx.apply();
            let iroha_core::state::StateBlock { world, .. } = block;
            world.commit();
        }
        state.push_block_hash_for_testing(block_hash);

        let response = handle_gov_get_tally(Arc::new(state), axum::extract::Path(rid))
            .await
            .expect("closed PLAIN tally");
        assert_eq!(response.0.evaluated_block_height, 1);
        assert_eq!(response.0.approve, 3);
        assert_eq!(response.0.reject, 0);
        assert_eq!(response.0.abstain, 0);
    }
    #[tokio::test]
    async fn gov_get_tally_projects_zk_abstentions() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), kura, query);
        let rid = "rid-tally-zk-abstain".to_string();
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
        {
            let mut block = state.block(header);
            let mut tx = block.transaction();
            tx.world.governance_referenda_mut().insert(
                rid.clone(),
                GovernanceReferendumRecord {
                    h_start: 1,
                    h_end: 10,
                    status: GovernanceReferendumStatus::Closed,
                    mode: GovernanceReferendumMode::Zk,
                },
            );
            tx.world.elections_mut().insert(
                rid.clone(),
                ElectionState {
                    finalized: true,
                    tally: vec![7, 3, 5],
                    ..ElectionState::default()
                },
            );
            tx.apply();
            let iroha_core::state::StateBlock { world, .. } = block;
            world.commit();
        }

        let response = handle_gov_get_tally(Arc::new(state), axum::extract::Path(rid))
            .await
            .expect("finalized ZK tally");
        assert_eq!(response.0.approve, 7);
        assert_eq!(response.0.reject, 3);
        assert_eq!(response.0.abstain, 5);
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
        let custody = generic_lock_custody(&state);
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
                    custody,
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
    async fn legacy_referendum_reads_reject_stored_typed_proposal_fingerprints() {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = State::new_for_testing(World::default(), kura, query);
        let kind = iroha_data_model::governance::types::ProposalKind::ValidationFeePolicy(
            iroha_data_model::governance::types::ValidationFeePolicyProposal {
                proposal_operator: ALICE_ID.clone(),
                policy: iroha_data_model::validation_fee::ValidationFeePolicyV1 {
                    schema_version:
                        iroha_data_model::validation_fee::VALIDATION_FEE_POLICY_SCHEMA_VERSION,
                    network_id: *state.network_id_ref(),
                    policy_version: 1,
                    previous_policy_hash: None,
                    ds_asset_id: state.gov.voting_asset_id.clone(),
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
        let state = Arc::new(state);
        for err in [
            handle_gov_get_tally(state.clone(), axum::extract::Path(rid.clone()))
                .await
                .expect_err("generic tally must reject a typed proposal fingerprint"),
            handle_gov_get_referendum(state.clone(), axum::extract::Path(rid.clone()))
                .await
                .expect_err("generic referendum read must reject a typed proposal fingerprint"),
            handle_gov_get_locks(state, axum::extract::Path(rid))
                .await
                .expect_err("generic lock read must reject a typed proposal fingerprint"),
        ] {
            let message = conversion_message(err);
            assert!(
                message.contains("authenticated Parliament lifecycle"),
                "unexpected legacy read error: {message}"
            );
        }
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
        let custody = generic_lock_custody(&state);
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
                        custody: custody.clone(),
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
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
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
            &"hash:0000000000000000000000000000000000000000000000000000000000000001#C50E"
                .parse()
                .expect("canonical test network id"),
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
    async fn propose_deploy_rejected_without_permission() {
        let harness = mk_governance_harness(false);
        let code_hash_bytes = [0x22u8; 32];
        let abi_hash_bytes = ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1);
        let manifest_provenance =
            mk_manifest_provenance(&harness.authority_keypair, code_hash_bytes, abi_hash_bytes);
        let propose = DeployContractProposalDraftRequestV1 {
            contract_address: Some(sample_contract_address()),
            contract_alias: None,
            abi_version: AbiVersion::new(1),
            code_hash: ContractCodeHash::new(code_hash_bytes),
            abi_hash: ContractAbiHash::new(abi_hash_bytes),
            manifest_provenance: Some(manifest_provenance),
        };
        let res = handle_gov_propose_deploy(harness.state.clone(), NoritoJson(propose))
            .await
            .expect("handler ok");
        let proposal_id = res.0.proposal_id.clone();
        queue_governance_proposal_instruction_skeleton(&harness, &res.0.tx_instructions);
        let errors = apply_queued_block_allow_errors(&harness.state, &harness.queue, 1);
        assert_eq!(errors, vec![true]);
        let pid_arr = proposal_id.into_bytes();
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
        let (state, _queue, _chain_id) = mk_basic_context();
        let authenticated = canonical_account(ACCOUNT_AUTHORITY);
        // Route for zk-v1
        let app = Router::new().route(
            "/v1/gov/ballots/zk-v1",
            post({
                let state = state.clone();
                let authenticated = authenticated.clone();
                move |body: crate::NoritoJsonWithBytes<super::ZkBallotV1Dto>| {
                    let telemetry = MaybeTelemetry::disabled();
                    let authenticated = authenticated.clone();
                    async move {
                        super::handle_gov_ballot_zk_v1(state, &authenticated, telemetry, body).await
                    }
                }
            }),
        );
        // Build DTO
        let owner = canonical_literal(ACCOUNT_AUTHORITY);
        let dto = super::ZkBallotV1Dto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            network_id: *state.network_id_ref(),
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
    async fn standalone_zk_ballot_rejects_stored_typed_proposal_fingerprint() {
        let (state, _queue, _chain_id) = mk_basic_context();
        let authenticated = canonical_account(ACCOUNT_AUTHORITY);
        let proposal_id = seed_typed_proposal_fingerprint_for_ballot_test(&state, &authenticated);
        let dto = super::ZkBallotV1Dto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            network_id: *state.network_id_ref(),
            election_id: proposal_id,
            backend: "halo2/ipa".to_owned(),
            envelope_b64: base64::engine::general_purpose::STANDARD.encode([1_u8, 2, 3, 4]),
            root_hint: None,
            owner: None,
            amount: None,
            duration_blocks: None,
            direction: None,
            nullifier: None,
        };
        let raw = norito::json::to_vec(&dto).expect("encode exact ZK ballot DTO");
        let response = super::handle_gov_ballot_zk_v1(
            state,
            &authenticated,
            MaybeTelemetry::disabled(),
            crate::NoritoJsonWithBytes {
                value: dto,
                raw: raw.into(),
            },
        )
        .await
        .expect("typed-proposal collision is a deterministic ballot rejection")
        .0;
        assert!(!response.ok);
        assert!(!response.accepted);
        assert!(response.tx_instructions.is_empty());
        assert!(
            response
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("authenticated Parliament lifecycle"))
        );
    }
    #[tokio::test]
    async fn ballot_zk_v1_rejects_invalid_root_hint() {
        let (state, _queue, _chain_id) = mk_basic_context();
        let authenticated = canonical_account(ACCOUNT_AUTHORITY);
        let dto = super::ZkBallotV1Dto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            network_id: *state.network_id_ref(),
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
            state,
            &authenticated,
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
        let (state, _queue, _chain_id) = mk_basic_context();
        let authenticated = canonical_account(ACCOUNT_AUTHORITY);
        let dto = super::ZkBallotV1Dto {
            authority: ACCOUNT_AUTHORITY.to_string(),
            network_id: *state.network_id_ref(),
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
            state,
            &authenticated,
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
    include!("gov/ballot_v1_strictness_tests.rs");
    include!("gov/ballotproof_shape_tests.rs");
}
