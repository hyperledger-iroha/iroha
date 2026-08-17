//! Independent semantic validation for the immutable four-wave rollout observation.
//!
//! The authority receives the plan and observation as signed subject data.  It
//! deliberately does not trust either document's self-declared hashes: the plan
//! is compared with the canonical checked-in policy bytes and both the candidate
//! binding and complete rollout identifier are re-derived here.

use super::service::TairaAuthorityErrorV1;
use norito::json::{Map, Value};
use sha2::{Digest as _, Sha256};
use std::collections::BTreeSet;

const IMMUTABLE_PLAN_V1: &[u8] =
    include_bytes!("../../../../../configs/soranexus/taira/privacy_rollout_plan_v1.json");

const AUTHORITY_SCHEMA_V1: &str = "iroha.taira.authenticated-rollout-observation-authority.v1";
const REPLAY_NAMESPACE_V1: &str = "iroha.taira.authenticated-rollout-observation-replay.v1";
const RESULT_SCHEMA_V1: &str = "iroha.taira.privacy_rollout_observation";
const CANDIDATE_ID_DOMAIN_V1: &[u8] = b"iroha.taira.privacy_rollout_candidate.v1\0";
const ROLLOUT_ID_DOMAIN_V1: &[u8] = b"iroha.taira.privacy_rollout_observation.v1\0";
const FROZEN_CARGO_LOCK_SHA256_V1: &str =
    "cd9e829e454171f17540abeb7fd1aa14129252082bd8b076a0199b0ffa4e3f79";
const EXACT12_MATRIX_SHA256_V1: &str =
    "7336d0221fddc51486ee53d4203f5a92d560d0ec9104a49de25896a8b10673d0";

const ENDPOINTS_V1: [&str; 5] = [
    "public-torii",
    "taira-validator-1",
    "taira-validator-2",
    "taira-validator-3",
    "taira-validator-4",
];
const VALIDATORS_V1: [&str; 4] = [
    "taira-validator-1",
    "taira-validator-2",
    "taira-validator-3",
    "taira-validator-4",
];
const PROTOCOLS_V1: [&str; 12] = [
    "zk-ace-pq-authorization-v0",
    "anonymous-pgc-k-out-of-n-v1",
    "verange-transparent-range-v1",
    "iroha-zk-ams-v1",
    "vega-existing-credential-zk-v0",
    "iroha-zk-x509-stark-p256-v0",
    "iroha-jindo-polynomial-commitment-v0",
    "iroha-bootle-lantern-anoncred-v1",
    "orchard-halo2-actions-v1",
    "monero-fcmp-plus-plus-v1",
    "iroha-ivm-private-note-stark-v1",
    "pq-masp-stark-v0",
];
const WAVE_1_V1: [&str; 4] = [
    "zk-ace-pq-authorization-v0",
    "anonymous-pgc-k-out-of-n-v1",
    "verange-transparent-range-v1",
    "iroha-bootle-lantern-anoncred-v1",
];
const WAVE_2_V1: [&str; 4] = [
    "orchard-halo2-actions-v1",
    "monero-fcmp-plus-plus-v1",
    "iroha-ivm-private-note-stark-v1",
    "pq-masp-stark-v0",
];
const WAVE_3_V1: [&str; 2] = [
    "vega-existing-credential-zk-v0",
    "iroha-jindo-polynomial-commitment-v0",
];
const WAVE_4_V1: [&str; 2] = ["iroha-zk-x509-stark-p256-v0", "iroha-zk-ams-v1"];
const WAVES_V1: [&[&str]; 4] = [&WAVE_1_V1, &WAVE_2_V1, &WAVE_3_V1, &WAVE_4_V1];

const NOTICE_INTERVAL_BLOCKS_V1: u64 = 300;
const OBSERVATION_INTERVAL_BLOCKS_V1: u64 = 300;
const MAX_ADDRESS_SPACE_BYTES_V1: u64 = 1024 * 1024 * 1024 * 1024;
const MAX_ELAPSED_MILLIS_V1: u64 = 60 * 60 * 1_000;
const MAX_PEAK_RSS_BYTES_V1: u64 = 64 * 1024 * 1024 * 1024;
const MAX_TRANSPORT_BYTES_V1: u64 = 128 * 1024 * 1024;
const MAX_WORK_UNITS_V1: u64 = 100_000_000_000;
const ZK_AMS_EVALUATED_KEY_BYTES_V1: u64 = 48_452_611_616;
const JINDO_ASSURANCE_V1: &str = "available-experimental";
const JINDO_MISSING_EVIDENCE_V1: [&str; 1] = ["MissingDistributionWideKnowledgeSoundnessEvidence"];

type ResultV1<T = ()> = Result<T, TairaAuthorityErrorV1>;

#[derive(Clone, Debug, PartialEq, Eq)]
struct SnapshotConvergenceV1 {
    height: u64,
    block_hash: String,
    manifest_sha256: String,
}

/// Validate the exact outer subject and every independently observable rollout semantic.
pub(super) fn validate_rollout_observation_subject_v1(subject: &Value) -> ResultV1 {
    let subject = exact_object(
        subject,
        &[
            "authority_schema",
            "observation",
            "plan",
            "replay_namespace",
        ],
    )?;
    require_str_eq(subject, "authority_schema", AUTHORITY_SCHEMA_V1)?;
    require_str_eq(subject, "replay_namespace", REPLAY_NAMESPACE_V1)?;

    let plan = subject.get("plan").ok_or(TairaAuthorityErrorV1::Rejected)?;
    let plan_sha256 = validate_immutable_plan_v1(plan)?;
    let observation = subject
        .get("observation")
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    validate_observation_v1(observation, plan_sha256)
}

fn validate_immutable_plan_v1(plan: &Value) -> ResultV1<[u8; 32]> {
    let core = IMMUTABLE_PLAN_V1
        .strip_suffix(b"\n")
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    if core.is_empty() || core.ends_with(b"\n") {
        return rejected();
    }
    let immutable: Value =
        norito::json::from_slice(core).map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    if &immutable != plan || canonical_json_core(&immutable)? != core {
        return rejected();
    }
    Ok(Sha256::digest(IMMUTABLE_PLAN_V1).into())
}

fn validate_observation_v1(observation: &Value, plan_sha256: [u8; 32]) -> ResultV1 {
    let root = exact_object(
        observation,
        &[
            "baseline",
            "candidate",
            "completed_at_unix",
            "plan_sha256",
            "post_cutover",
            "rollback",
            "rollout_id",
            "schema",
            "schema_version",
            "started_at_unix",
            "terminal",
            "waves",
        ],
    )?;
    require_str_eq(root, "schema", RESULT_SCHEMA_V1)?;
    require_u64_eq(root, "schema_version", 1)?;
    if required_digest(root, "plan_sha256")? != plan_sha256 {
        return rejected();
    }
    let started = required_positive_u64(root, "started_at_unix")?;
    let completed = required_positive_u64(root, "completed_at_unix")?;
    if completed < started {
        return rejected();
    }

    let candidate = exact_object(
        required(root, "candidate")?,
        &[
            "archive_sha256",
            "candidate_binding_sha256",
            "candidate_oci_digest",
            "capability_schema_sha256",
            "cargo_lock_sha256",
            "dpn_validator_release_commit",
            "irohad_sha256",
            "protocol_matrix_sha256",
            "source_commit",
            "workspace_source_manifest_sha256",
        ],
    )?;
    let candidate_binding = validate_candidate_v1(candidate)?;
    let candidate_oci = required_str(candidate, "candidate_oci_digest")?.to_owned();

    let baseline = exact_object(required(root, "baseline")?, &["snapshots"])?;
    let baseline = validate_snapshots_v1(
        required(baseline, "snapshots")?,
        &candidate_binding,
        &[],
        None,
    )?;

    let waves = required_array(root, "waves")?;
    if waves.len() != WAVES_V1.len() {
        return rejected();
    }
    let mut prior_observation_height = baseline.height;
    let mut final_observation_manifest = None;
    let mut used_transactions = BTreeSet::new();
    for (offset, (wave_value, protocols)) in waves.iter().zip(WAVES_V1).enumerate() {
        let wave_index = offset + 1;
        let wave = exact_object(
            wave_value,
            &[
                "activate_at_height",
                "activation_transactions",
                "canaries",
                "candidate_binding_sha256",
                "index",
                "label",
                "observation_completed_at_height",
                "observation_snapshots",
                "post_activation_snapshots",
                "post_restart_snapshots",
                "pre_activation_snapshots",
                "proposed_at_height",
                "protocols",
                "resources",
                "restart",
            ],
        )?;
        if required_u64(wave, "index")? != wave_index as u64
            || required_str(wave, "label")? != format!("wave-{wave_index}")
            || !string_array_eq(required(wave, "protocols")?, protocols)
            || required_str(wave, "candidate_binding_sha256")? != candidate_binding
        {
            return rejected();
        }
        let proposed = required_min_u64(wave, "proposed_at_height", 2)?;
        let activation = required_min_u64(wave, "activate_at_height", 2)?;
        let observation_height = required_min_u64(wave, "observation_completed_at_height", 2)?;
        if proposed <= prior_observation_height
            || activation
                .checked_sub(proposed)
                .is_none_or(|delta| delta < NOTICE_INTERVAL_BLOCKS_V1)
            || observation_height
                .checked_sub(activation)
                .is_none_or(|delta| delta < OBSERVATION_INTERVAL_BLOCKS_V1)
        {
            return rejected();
        }

        validate_activation_transactions_v1(
            required(wave, "activation_transactions")?,
            protocols,
            &mut used_transactions,
        )?;
        let prior_active = expected_active_v1(offset);
        let active = expected_active_v1(wave_index);
        validate_snapshots_v1(
            required(wave, "pre_activation_snapshots")?,
            &candidate_binding,
            &prior_active,
            proposed.checked_sub(1),
        )?;
        validate_snapshots_v1(
            required(wave, "post_activation_snapshots")?,
            &candidate_binding,
            &active,
            Some(activation),
        )?;
        let maximum_canary_height = validate_canaries_v1(
            required(wave, "canaries")?,
            protocols,
            &candidate_binding,
            activation,
            observation_height,
            &mut used_transactions,
        )?;
        validate_resources_v1(required(wave, "resources")?, protocols)?;
        let successor = validate_restart_v1(
            required(wave, "restart")?,
            wave_index,
            maximum_canary_height,
            observation_height,
        )?;
        validate_snapshots_v1(
            required(wave, "post_restart_snapshots")?,
            &candidate_binding,
            &active,
            Some(successor),
        )?;
        let observed = validate_snapshots_v1(
            required(wave, "observation_snapshots")?,
            &candidate_binding,
            &active,
            Some(observation_height),
        )?;
        final_observation_manifest = Some(observed.manifest_sha256);
        prior_observation_height = observation_height;
    }

    let (post_height, post_manifest) = validate_post_cutover_v1(
        required(root, "post_cutover")?,
        &candidate_oci,
        &candidate_binding,
        prior_observation_height,
        &mut used_transactions,
    )?;
    validate_rollback_v1(required(root, "rollback")?, &candidate_oci, &post_manifest)?;
    validate_terminal_v1(
        required(root, "terminal")?,
        &candidate_oci,
        &post_manifest,
        post_height,
        prior_observation_height,
        final_observation_manifest.as_deref(),
    )?;

    let mut body = root.clone();
    body.remove("rollout_id")
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    let derived = digest_domain_value(ROLLOUT_ID_DOMAIN_V1, &Value::Object(body))?;
    if required_digest(root, "rollout_id")? != derived {
        return rejected();
    }
    Ok(())
}

fn validate_candidate_v1(candidate: &Map) -> ResultV1<String> {
    validate_commit(required_str(candidate, "source_commit")?)?;
    validate_commit(required_str(candidate, "dpn_validator_release_commit")?)?;
    require_str_eq(candidate, "cargo_lock_sha256", FROZEN_CARGO_LOCK_SHA256_V1)?;
    require_str_eq(
        candidate,
        "protocol_matrix_sha256",
        EXACT12_MATRIX_SHA256_V1,
    )?;
    for field in [
        "archive_sha256",
        "capability_schema_sha256",
        "irohad_sha256",
        "workspace_source_manifest_sha256",
    ] {
        required_digest(candidate, field)?;
    }
    validate_oci_digest(required_str(candidate, "candidate_oci_digest")?)?;

    let mut body = candidate.clone();
    body.remove("candidate_binding_sha256")
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    let derived = digest_domain_value(CANDIDATE_ID_DOMAIN_V1, &Value::Object(body))?;
    if required_digest(candidate, "candidate_binding_sha256")? != derived {
        return rejected();
    }
    Ok(hex::encode(derived))
}

fn validate_activation_transactions_v1(
    value: &Value,
    protocols: &[&str],
    used_transactions: &mut BTreeSet<String>,
) -> ResultV1 {
    let rows = value.as_array().ok_or(TairaAuthorityErrorV1::Rejected)?;
    if rows.len() != protocols.len() {
        return rejected();
    }
    for (row, protocol) in rows.iter().zip(protocols) {
        let row = exact_object(
            row,
            &["governance_outcome", "protocol", "transaction_sha256"],
        )?;
        require_str_eq(row, "protocol", protocol)?;
        require_str_eq(row, "governance_outcome", "committed")?;
        let transaction = required_digest_string(row, "transaction_sha256")?;
        if !used_transactions.insert(transaction) {
            return rejected();
        }
    }
    Ok(())
}

fn validate_snapshots_v1(
    value: &Value,
    candidate_binding: &str,
    active_protocols: &[&str],
    expected_height: Option<u64>,
) -> ResultV1<SnapshotConvergenceV1> {
    let rows = value.as_array().ok_or(TairaAuthorityErrorV1::Rejected)?;
    if rows.len() != ENDPOINTS_V1.len() {
        return rejected();
    }
    let mut common = None;
    for (row, endpoint) in rows.iter().zip(ENDPOINTS_V1) {
        let row = exact_object(
            row,
            &[
                "active_protocols",
                "available_protocols",
                "block_hash",
                "candidate_binding_sha256",
                "capability_manifest_sha256",
                "endpoint",
                "height",
                "jindo_assurance",
                "jindo_missing_evidence",
                "unavailable_protocols",
            ],
        )?;
        require_str_eq(row, "endpoint", endpoint)?;
        require_str_eq(row, "candidate_binding_sha256", candidate_binding)?;
        let height = required_positive_u64(row, "height")?;
        if expected_height.is_some_and(|expected| expected != height) {
            return rejected();
        }
        let block_hash = required_digest_string(row, "block_hash")?;
        let manifest_sha256 = required_digest_string(row, "capability_manifest_sha256")?;
        if !string_array_eq(required(row, "available_protocols")?, &PROTOCOLS_V1)
            || !required_array(row, "unavailable_protocols")?.is_empty()
            || !string_array_eq(required(row, "active_protocols")?, active_protocols)
            || required_str(row, "jindo_assurance")? != JINDO_ASSURANCE_V1
            || !string_array_eq(
                required(row, "jindo_missing_evidence")?,
                &JINDO_MISSING_EVIDENCE_V1,
            )
        {
            return rejected();
        }
        let observed = SnapshotConvergenceV1 {
            height,
            block_hash,
            manifest_sha256,
        };
        if common.as_ref().is_some_and(|prior| prior != &observed) {
            return rejected();
        }
        common = Some(observed);
    }
    common.ok_or(TairaAuthorityErrorV1::Rejected)
}

fn validate_queries_v1(value: &Value) -> ResultV1<(u64, String)> {
    let rows = value.as_array().ok_or(TairaAuthorityErrorV1::Rejected)?;
    if rows.len() != ENDPOINTS_V1.len() {
        return rejected();
    }
    let mut common = None;
    for (row, endpoint) in rows.iter().zip(ENDPOINTS_V1) {
        let row = exact_object(row, &["endpoint", "height", "state_sha256"])?;
        require_str_eq(row, "endpoint", endpoint)?;
        let observed = (
            required_positive_u64(row, "height")?,
            required_digest_string(row, "state_sha256")?,
        );
        if common.as_ref().is_some_and(|prior| prior != &observed) {
            return rejected();
        }
        common = Some(observed);
    }
    common.ok_or(TairaAuthorityErrorV1::Rejected)
}

fn validate_rejections_v1(
    value: &Value,
    failure_code: &str,
    minimum_height: u64,
    maximum_height: Option<u64>,
) -> ResultV1<u64> {
    let rows = value.as_array().ok_or(TairaAuthorityErrorV1::Rejected)?;
    if rows.len() != ENDPOINTS_V1.len() {
        return rejected();
    }
    let mut maximum_observed = minimum_height;
    for (row, endpoint) in rows.iter().zip(ENDPOINTS_V1) {
        let row = exact_object(row, &["endpoint", "failure_code", "observed_height"])?;
        require_str_eq(row, "endpoint", endpoint)?;
        require_str_eq(row, "failure_code", failure_code)?;
        let observed = required_positive_u64(row, "observed_height")?;
        if observed < minimum_height || maximum_height.is_some_and(|maximum| observed > maximum) {
            return rejected();
        }
        maximum_observed = maximum_observed.max(observed);
    }
    Ok(maximum_observed)
}

#[allow(clippy::too_many_arguments)]
fn validate_canaries_v1(
    value: &Value,
    protocols: &[&str],
    candidate_binding: &str,
    activation_height: u64,
    observation_height: u64,
    used_transactions: &mut BTreeSet<String>,
) -> ResultV1<u64> {
    let rows = value.as_array().ok_or(TairaAuthorityErrorV1::Rejected)?;
    if rows.len() != protocols.len() {
        return rejected();
    }
    let mut maximum_height = activation_height;
    let mut statements = BTreeSet::new();
    for (row, protocol) in rows.iter().zip(protocols) {
        let row = exact_object(
            row,
            &[
                "candidate_binding_sha256",
                "negative",
                "positive",
                "protocol",
                "replay",
            ],
        )?;
        require_str_eq(row, "protocol", protocol)?;
        require_str_eq(row, "candidate_binding_sha256", candidate_binding)?;

        let positive = exact_object(
            required(row, "positive")?,
            &[
                "accepted_height",
                "peer_queries",
                "statement_sha256",
                "submitted_via",
                "transaction_sha256",
            ],
        )?;
        require_str_eq(positive, "submitted_via", "public-torii")?;
        let accepted = required_positive_u64(positive, "accepted_height")?;
        if accepted < activation_height || accepted > observation_height {
            return rejected();
        }
        let transaction = required_digest_string(positive, "transaction_sha256")?;
        let statement = required_digest_string(positive, "statement_sha256")?;
        if !statements.insert(statement) || !used_transactions.insert(transaction.clone()) {
            return rejected();
        }
        let (query_height, _) = validate_queries_v1(required(positive, "peer_queries")?)?;
        if query_height < accepted || query_height > observation_height {
            return rejected();
        }

        let negative = exact_object(
            required(row, "negative")?,
            &["rejections", "transaction_sha256"],
        )?;
        let negative_transaction = required_digest_string(negative, "transaction_sha256")?;
        if !used_transactions.insert(negative_transaction) {
            return rejected();
        }
        let negative_height = validate_rejections_v1(
            required(negative, "rejections")?,
            "privacy-malformed-proof",
            activation_height,
            Some(observation_height),
        )?;

        let replay = exact_object(
            required(row, "replay")?,
            &["rejections", "transaction_sha256"],
        )?;
        if required_digest_string(replay, "transaction_sha256")? != transaction {
            return rejected();
        }
        let replay_height = validate_rejections_v1(
            required(replay, "rejections")?,
            "privacy-replay",
            accepted,
            Some(observation_height),
        )?;
        maximum_height = maximum_height
            .max(accepted)
            .max(query_height)
            .max(negative_height)
            .max(replay_height);
    }
    Ok(maximum_height)
}

fn validate_resources_v1(value: &Value, protocols: &[&str]) -> ResultV1 {
    let rows = value.as_array().ok_or(TairaAuthorityErrorV1::Rejected)?;
    if rows.len() != protocols.len() {
        return rejected();
    }
    for (row, protocol) in rows.iter().zip(protocols) {
        let row = exact_object(
            row,
            &[
                "address_space_bytes",
                "elapsed_millis",
                "evaluated_key_publication_bytes",
                "evaluated_key_retrieval_bytes",
                "peak_rss_bytes",
                "protocol",
                "transport_bytes",
                "work_units",
            ],
        )?;
        require_str_eq(row, "protocol", protocol)?;
        let elapsed = required_positive_u64(row, "elapsed_millis")?;
        let rss = required_positive_u64(row, "peak_rss_bytes")?;
        let address_space = required_positive_u64(row, "address_space_bytes")?;
        let transport = required_positive_u64(row, "transport_bytes")?;
        let work = required_positive_u64(row, "work_units")?;
        if elapsed > MAX_ELAPSED_MILLIS_V1
            || rss > MAX_PEAK_RSS_BYTES_V1
            || address_space < rss
            || address_space > MAX_ADDRESS_SPACE_BYTES_V1
            || transport > MAX_TRANSPORT_BYTES_V1
            || work > MAX_WORK_UNITS_V1
        {
            return rejected();
        }
        let expected_evaluated = if *protocol == "iroha-zk-ams-v1" {
            ZK_AMS_EVALUATED_KEY_BYTES_V1
        } else {
            0
        };
        if required_u64(row, "evaluated_key_publication_bytes")? != expected_evaluated
            || required_u64(row, "evaluated_key_retrieval_bytes")? != expected_evaluated
        {
            return rejected();
        }
    }
    Ok(())
}

fn validate_restart_v1(
    value: &Value,
    wave_index: usize,
    minimum_stopped_height: u64,
    observation_height: u64,
) -> ResultV1<u64> {
    let restart = exact_object(
        value,
        &[
            "peer_finality",
            "recovered_hash",
            "recovered_height",
            "sentinel_hash",
            "sentinel_height",
            "stopped_height",
            "successor_hash",
            "successor_height",
            "validator",
        ],
    )?;
    require_str_eq(restart, "validator", VALIDATORS_V1[wave_index - 1])?;
    let stopped = required_positive_u64(restart, "stopped_height")?;
    let sentinel = required_positive_u64(restart, "sentinel_height")?;
    let recovered = required_positive_u64(restart, "recovered_height")?;
    let successor = required_positive_u64(restart, "successor_height")?;
    let sentinel_hash = required_digest_string(restart, "sentinel_hash")?;
    let recovered_hash = required_digest_string(restart, "recovered_hash")?;
    let successor_hash = required_digest_string(restart, "successor_hash")?;
    if stopped < minimum_stopped_height
        || stopped >= sentinel
        || sentinel
            < minimum_stopped_height
                .checked_add(1)
                .ok_or(TairaAuthorityErrorV1::Rejected)?
        || recovered != sentinel
        || recovered_hash != sentinel_hash
        || successor
            != sentinel
                .checked_add(1)
                .ok_or(TairaAuthorityErrorV1::Rejected)?
        || successor > observation_height
    {
        return rejected();
    }
    let peers = required_array(restart, "peer_finality")?;
    if peers.len() != ENDPOINTS_V1.len() {
        return rejected();
    }
    for (row, endpoint) in peers.iter().zip(ENDPOINTS_V1) {
        let row = exact_object(row, &["block_hash", "endpoint", "height"])?;
        if required_str(row, "endpoint")? != endpoint
            || required_u64(row, "height")? != successor
            || required_str(row, "block_hash")? != successor_hash
        {
            return rejected();
        }
    }
    Ok(successor)
}

fn validate_post_cutover_v1(
    value: &Value,
    candidate_oci: &str,
    candidate_binding: &str,
    minimum_height: u64,
    used_transactions: &mut BTreeSet<String>,
) -> ResultV1<(u64, String)> {
    let post = exact_object(
        value,
        &[
            "canary",
            "deployed_candidate_oci_digest",
            "readmitted_candidate_oci_digest",
            "snapshots",
        ],
    )?;
    require_str_eq(post, "deployed_candidate_oci_digest", candidate_oci)?;
    require_str_eq(post, "readmitted_candidate_oci_digest", candidate_oci)?;
    let canary = exact_object(
        required(post, "canary")?,
        &[
            "authority_scope",
            "bootstrap_authority",
            "governance_permission_present",
            "mode",
            "privacy",
            "replay",
            "skipped",
            "write",
        ],
    )?;
    require_str_eq(canary, "mode", "signed-write-and-privacy")?;
    require_str_eq(canary, "authority_scope", "dedicated-no-governance-canary")?;
    require_bool_eq(canary, "skipped", false)?;
    require_bool_eq(canary, "bootstrap_authority", false)?;
    require_bool_eq(canary, "governance_permission_present", false)?;

    let write = exact_object(
        required(canary, "write")?,
        &[
            "accepted_height",
            "peer_queries",
            "submitted_via",
            "transaction_sha256",
        ],
    )?;
    require_str_eq(write, "submitted_via", "public-torii")?;
    let write_height = required_min_u64(
        write,
        "accepted_height",
        minimum_height
            .checked_add(1)
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
    )?;
    let write_transaction = required_digest_string(write, "transaction_sha256")?;
    if !used_transactions.insert(write_transaction) {
        return rejected();
    }
    let (write_query_height, _) = validate_queries_v1(required(write, "peer_queries")?)?;
    if write_query_height < write_height {
        return rejected();
    }

    let privacy = exact_object(
        required(canary, "privacy")?,
        &[
            "accepted_height",
            "peer_queries",
            "protocol",
            "statement_sha256",
            "submitted_via",
            "transaction_sha256",
        ],
    )?;
    require_str_eq(privacy, "protocol", "verange-transparent-range-v1")?;
    require_str_eq(privacy, "submitted_via", "public-torii")?;
    let privacy_height = required_min_u64(
        privacy,
        "accepted_height",
        write_height
            .checked_add(1)
            .ok_or(TairaAuthorityErrorV1::Rejected)?,
    )?;
    let privacy_transaction = required_digest_string(privacy, "transaction_sha256")?;
    if !used_transactions.insert(privacy_transaction.clone()) {
        return rejected();
    }
    required_digest(privacy, "statement_sha256")?;
    let (privacy_query_height, _) = validate_queries_v1(required(privacy, "peer_queries")?)?;
    if privacy_query_height < privacy_height {
        return rejected();
    }

    let replay = exact_object(
        required(canary, "replay")?,
        &["rejections", "transaction_sha256"],
    )?;
    if required_str(replay, "transaction_sha256")? != privacy_transaction {
        return rejected();
    }
    let replay_height = validate_rejections_v1(
        required(replay, "rejections")?,
        "privacy-replay",
        privacy_height,
        None,
    )?;
    let snapshots = validate_snapshots_v1(
        required(post, "snapshots")?,
        candidate_binding,
        &expected_active_v1(WAVES_V1.len()),
        None,
    )?;
    let minimum_snapshot = write_query_height
        .max(privacy_query_height)
        .max(privacy_height)
        .max(replay_height);
    if snapshots.height < minimum_snapshot {
        return rejected();
    }
    Ok((snapshots.height, snapshots.manifest_sha256))
}

fn validate_rollback_v1(value: &Value, candidate_oci: &str, post_manifest: &str) -> ResultV1 {
    let rollback = exact_object(
        value,
        &[
            "armed",
            "invoked",
            "legacy_fallback_used",
            "previous_candidate_oci_digest",
            "previous_capability_manifest_sha256",
            "restore_mode",
        ],
    )?;
    require_bool_eq(rollback, "armed", true)?;
    require_bool_eq(rollback, "invoked", false)?;
    require_bool_eq(rollback, "legacy_fallback_used", false)?;
    require_str_eq(
        rollback,
        "restore_mode",
        "immutable-candidate-and-capability-set",
    )?;
    let previous_oci = required_str(rollback, "previous_candidate_oci_digest")?;
    validate_oci_digest(previous_oci)?;
    let previous_manifest =
        required_digest_string(rollback, "previous_capability_manifest_sha256")?;
    if previous_oci == candidate_oci || previous_manifest == post_manifest {
        return rejected();
    }
    Ok(())
}

fn validate_terminal_v1(
    value: &Value,
    candidate_oci: &str,
    post_manifest: &str,
    post_height: u64,
    prior_observation_height: u64,
    final_observation_manifest: Option<&str>,
) -> ResultV1 {
    let terminal = exact_object(
        value,
        &[
            "final_candidate_oci_digest",
            "final_capability_manifest_sha256",
            "halt_reason",
            "halted",
            "publication_authorized",
            "status",
        ],
    )?;
    if required_str(terminal, "status")? != "passed"
        || required_bool(terminal, "halted")?
        || !required(terminal, "halt_reason")?.is_null()
        || !required_bool(terminal, "publication_authorized")?
        || required_str(terminal, "final_candidate_oci_digest")? != candidate_oci
        || required_str(terminal, "final_capability_manifest_sha256")? != post_manifest
        || post_height <= prior_observation_height
        || final_observation_manifest.is_none_or(str::is_empty)
    {
        return rejected();
    }
    required_digest(terminal, "final_capability_manifest_sha256")?;
    Ok(())
}

fn expected_active_v1(completed_waves: usize) -> Vec<&'static str> {
    PROTOCOLS_V1
        .iter()
        .copied()
        .filter(|protocol| {
            WAVES_V1
                .iter()
                .take(completed_waves)
                .any(|wave| wave.contains(protocol))
        })
        .collect()
}

fn exact_object<'a>(value: &'a Value, fields: &[&str]) -> ResultV1<&'a Map> {
    let object = value.as_object().ok_or(TairaAuthorityErrorV1::Rejected)?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return rejected();
    }
    Ok(object)
}

fn required<'a>(object: &'a Map, field: &str) -> ResultV1<&'a Value> {
    object.get(field).ok_or(TairaAuthorityErrorV1::Rejected)
}

fn required_array<'a>(object: &'a Map, field: &str) -> ResultV1<&'a Vec<Value>> {
    required(object, field)?
        .as_array()
        .ok_or(TairaAuthorityErrorV1::Rejected)
}

fn required_str<'a>(object: &'a Map, field: &str) -> ResultV1<&'a str> {
    required(object, field)?
        .as_str()
        .ok_or(TairaAuthorityErrorV1::Rejected)
}

fn require_str_eq(object: &Map, field: &str, expected: &str) -> ResultV1 {
    if required_str(object, field)? != expected {
        return rejected();
    }
    Ok(())
}

fn required_bool(object: &Map, field: &str) -> ResultV1<bool> {
    required(object, field)?
        .as_bool()
        .ok_or(TairaAuthorityErrorV1::Rejected)
}

fn require_bool_eq(object: &Map, field: &str, expected: bool) -> ResultV1 {
    if required_bool(object, field)? != expected {
        return rejected();
    }
    Ok(())
}

fn required_u64(object: &Map, field: &str) -> ResultV1<u64> {
    required(object, field)?
        .as_u64()
        .ok_or(TairaAuthorityErrorV1::Rejected)
}

fn required_positive_u64(object: &Map, field: &str) -> ResultV1<u64> {
    required_min_u64(object, field, 1)
}

fn required_min_u64(object: &Map, field: &str, minimum: u64) -> ResultV1<u64> {
    required_u64(object, field).and_then(|value| {
        if value < minimum {
            rejected()
        } else {
            Ok(value)
        }
    })
}

fn require_u64_eq(object: &Map, field: &str, expected: u64) -> ResultV1 {
    if required_u64(object, field)? != expected {
        return rejected();
    }
    Ok(())
}

fn required_digest(object: &Map, field: &str) -> ResultV1<[u8; 32]> {
    parse_digest(required_str(object, field)?)
}

fn required_digest_string(object: &Map, field: &str) -> ResultV1<String> {
    let value = required_str(object, field)?;
    parse_digest(value)?;
    Ok(value.to_owned())
}

fn parse_digest(value: &str) -> ResultV1<[u8; 32]> {
    if value.len() != 64
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return rejected();
    }
    let digest: [u8; 32] = hex::decode(value)
        .map_err(|_| TairaAuthorityErrorV1::Rejected)?
        .try_into()
        .map_err(|_| TairaAuthorityErrorV1::Rejected)?;
    if digest == [0; 32] {
        return rejected();
    }
    Ok(digest)
}

fn validate_commit(value: &str) -> ResultV1 {
    if value.len() != 40
        || value.bytes().all(|byte| byte == b'0')
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return rejected();
    }
    Ok(())
}

fn validate_oci_digest(value: &str) -> ResultV1 {
    let digest = value
        .strip_prefix("sha256:")
        .ok_or(TairaAuthorityErrorV1::Rejected)?;
    parse_digest(digest).map(|_| ())
}

fn string_array_eq(value: &Value, expected: &[&str]) -> bool {
    value.as_array().is_some_and(|rows| {
        rows.len() == expected.len()
            && rows
                .iter()
                .zip(expected)
                .all(|(row, expected)| row.as_str() == Some(*expected))
    })
}

fn canonical_json_core(value: &Value) -> ResultV1<Vec<u8>> {
    norito::json::to_vec(value).map_err(|_| TairaAuthorityErrorV1::Rejected)
}

fn digest_domain_value(domain: &[u8], value: &Value) -> ResultV1<[u8; 32]> {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(canonical_json_core(value)?);
    Ok(hasher.finalize().into())
}

fn rejected<T>() -> ResultV1<T> {
    Err(TairaAuthorityErrorV1::Rejected)
}

#[cfg(test)]
#[path = "rollout_observation_tests.rs"]
pub(super) mod tests;
