#![cfg(feature = "app_api")]

//! Capacity registry helpers exposed via Torii.

use std::{
    collections::HashSet,
    str::FromStr,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STD};
use hex::ToHex;
use iroha_core::state::{WorldReadOnly, WorldView};
use iroha_data_model::{
    metadata::Metadata,
    name::Name,
    sorafs::{
        capacity::{
            CapacityDeclarationRecord, CapacityDisputeEvidence, CapacityDisputeId,
            CapacityDisputeRecord, CapacityDisputeStatus, CapacityFeeLedgerEntry, ProviderId,
        },
        pin_registry::{
            ManifestAliasBinding, ManifestAliasId, ManifestAliasRecord, ManifestDigest,
            PinManifestRecord, PinPolicy, PinStatus, ReplicationOrderId, ReplicationOrderRecord,
            ReplicationOrderStatus, StorageClass,
        },
        pricing::ProviderCreditRecord,
    },
};
use iroha_primitives::{json::Json, numeric::Quantity};
use mv::storage::StorageReadOnly;
use norito::{
    DecodeLimits,
    core::Error as NoritoDecodeError,
    decode_from_bytes_with_limits,
    json::{self, Map, Value},
    to_bytes,
};
use sorafs_manifest::{
    capacity::{
        CapacityDeclarationV1, CapacityMetadataEntry, ChunkerCommitmentV1, LaneCommitmentV1,
        PricingScheduleV1, ReplicationOrderV1,
    },
    provider_advert::{CapabilityType, StakePointer},
};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{routing::MaybeTelemetry, sorafs::capability_name};

const METADATA_STATUS_TIMESTAMP_KEY: &str = "sorafs_status_timestamp_unix";
const METADATA_GOVERNANCE_REFS_KEY: &str = "sorafs_governance_refs";
const REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1: usize = 256 * 1024;
const REPLICATION_ORDER_DECODE_LIMITS_V1: DecodeLimits = DecodeLimits::new(
    sorafs_manifest::capacity::MAX_CAPACITY_METADATA_VALUE_BYTES,
    REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1,
    131_072,
    REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1 * 4,
    32,
);

/// Collect a snapshot of provider declarations and fee ledger entries.
pub(crate) fn collect_snapshot(world: &WorldView<'_>) -> Result<CapacitySnapshot, RegistryError> {
    let declarations = build_declarations(world.capacity_declarations().iter())?;
    let fee_ledger = build_fee_ledger(world.capacity_fee_ledger().iter());
    let credit_ledger = build_credit_ledger(world.provider_credit_ledger().iter());
    let disputes = build_disputes(world.capacity_disputes().iter());
    Ok(CapacitySnapshot {
        declarations,
        fee_ledger,
        credit_ledger,
        disputes,
    })
}

/// Aggregated registry snapshot returned by both REST and gRPC facades.
#[derive(Debug, Clone)]
pub(crate) struct CapacitySnapshot {
    /// Provider capacity declarations.
    pub(crate) declarations: Vec<RegistryDeclaration>,
    /// Fee ledger entries per provider.
    pub(crate) fee_ledger: Vec<RegistryFeeLedgerEntry>,
    /// Credit ledger entries per provider.
    pub(crate) credit_ledger: Vec<RegistryCreditLedgerEntry>,
    /// Disputes filed against providers.
    pub(crate) disputes: Vec<RegistryDispute>,
}

/// Provider declaration projection ready for JSON serialization.
#[derive(Debug, Clone)]
pub(crate) struct RegistryDeclaration {
    pub(crate) provider_id_hex: String,
    pub(crate) committed_capacity_gib: u64,
    pub(crate) registered_epoch: u64,
    pub(crate) valid_from_epoch: u64,
    pub(crate) valid_until_epoch: u64,
    pub(crate) declaration_json: Value,
    pub(crate) metadata_json: Value,
}

impl RegistryDeclaration {
    /// Convert the declaration into a Norito JSON value.
    pub fn into_json(self) -> Result<Value, json::Error> {
        let mut map = Map::new();
        map.insert(
            "provider_id_hex".into(),
            Value::String(self.provider_id_hex),
        );
        map.insert(
            "committed_capacity_gib".into(),
            json::to_value(&self.committed_capacity_gib)?,
        );
        map.insert(
            "registered_epoch".into(),
            json::to_value(&self.registered_epoch)?,
        );
        map.insert(
            "valid_from_epoch".into(),
            json::to_value(&self.valid_from_epoch)?,
        );
        map.insert(
            "valid_until_epoch".into(),
            json::to_value(&self.valid_until_epoch)?,
        );
        map.insert("declaration".into(), self.declaration_json);
        map.insert("metadata".into(), self.metadata_json);
        Ok(Value::Object(map))
    }
}

/// Fee ledger projection ready for JSON serialization.
#[derive(Debug, Clone)]
pub(crate) struct RegistryFeeLedgerEntry {
    pub(crate) provider_id_hex: String,
    pub(crate) total_declared_gib: u128,
    pub(crate) total_utilised_gib: u128,
    pub(crate) storage_fee: Quantity,
    pub(crate) egress_fee: Quantity,
    pub(crate) accrued_fee: Quantity,
    pub(crate) expected_settlement: Quantity,
    pub(crate) penalty_slashed: Quantity,
    pub(crate) penalty_events: u32,
    pub(crate) last_updated_epoch: u64,
}

impl RegistryFeeLedgerEntry {
    /// Convert the ledger entry into a Norito JSON value.
    pub fn into_json(self) -> Result<Value, json::Error> {
        let mut map = Map::new();
        map.insert(
            "provider_id_hex".into(),
            Value::String(self.provider_id_hex),
        );
        map.insert(
            "total_declared_gib".into(),
            json::to_value(&self.total_declared_gib)?,
        );
        map.insert(
            "total_utilised_gib".into(),
            json::to_value(&self.total_utilised_gib)?,
        );
        map.insert("storage_fee".into(), json::to_value(&self.storage_fee)?);
        map.insert("egress_fee".into(), json::to_value(&self.egress_fee)?);
        map.insert("accrued_fee".into(), json::to_value(&self.accrued_fee)?);
        map.insert(
            "expected_settlement".into(),
            json::to_value(&self.expected_settlement)?,
        );
        map.insert(
            "penalty_slashed".into(),
            json::to_value(&self.penalty_slashed)?,
        );
        map.insert(
            "penalty_events".into(),
            json::to_value(&self.penalty_events)?,
        );
        map.insert(
            "last_updated_epoch".into(),
            json::to_value(&self.last_updated_epoch)?,
        );
        Ok(Value::Object(map))
    }
}

/// Credit ledger projection ready for JSON serialization.
#[derive(Debug, Clone)]
pub(crate) struct RegistryCreditLedgerEntry {
    pub(crate) provider_id_hex: String,
    pub(crate) available_credit: Quantity,
    pub(crate) bonded: Quantity,
    pub(crate) required_bond: Quantity,
    pub(crate) expected_settlement: Quantity,
    pub(crate) onboarding_epoch: u64,
    pub(crate) last_settlement_epoch: u64,
    pub(crate) low_balance_since_epoch: Option<u64>,
    pub(crate) slashed: Quantity,
    pub(crate) under_delivery_strikes: u32,
    pub(crate) last_penalty_epoch: Option<u64>,
    pub(crate) metadata_json: Value,
}

impl RegistryCreditLedgerEntry {
    /// Convert the credit ledger entry into a Norito JSON value.
    pub fn into_json(self) -> Result<Value, json::Error> {
        let mut map = Map::new();
        map.insert(
            "provider_id_hex".into(),
            Value::String(self.provider_id_hex),
        );
        map.insert(
            "available_credit".into(),
            json::to_value(&self.available_credit)?,
        );
        map.insert("bonded".into(), json::to_value(&self.bonded)?);
        map.insert("required_bond".into(), json::to_value(&self.required_bond)?);
        map.insert(
            "expected_settlement".into(),
            json::to_value(&self.expected_settlement)?,
        );
        map.insert(
            "onboarding_epoch".into(),
            json::to_value(&self.onboarding_epoch)?,
        );
        map.insert(
            "last_settlement_epoch".into(),
            json::to_value(&self.last_settlement_epoch)?,
        );
        map.insert(
            "low_balance_since_epoch".into(),
            json::to_value(&self.low_balance_since_epoch)?,
        );
        map.insert("slashed".into(), json::to_value(&self.slashed)?);
        map.insert(
            "under_delivery_strikes".into(),
            json::to_value(&self.under_delivery_strikes)?,
        );
        map.insert(
            "last_penalty_epoch".into(),
            json::to_value(&self.last_penalty_epoch)?,
        );
        map.insert("metadata".into(), self.metadata_json);
        Ok(Value::Object(map))
    }
}

/// Dispute projection ready for JSON serialization.
#[derive(Debug, Clone)]
pub(crate) struct RegistryDispute {
    pub(crate) dispute_id_hex: String,
    pub(crate) provider_id_hex: String,
    pub(crate) complainant_id_hex: String,
    pub(crate) replication_order_id_hex: Option<String>,
    pub(crate) kind: String,
    pub(crate) submitted_epoch: u64,
    pub(crate) description: String,
    pub(crate) requested_remedy: Option<String>,
    pub(crate) status: String,
    pub(crate) resolution_epoch: Option<u64>,
    pub(crate) resolution_outcome: Option<String>,
    pub(crate) resolution_notes: Option<String>,
    pub(crate) evidence_digest_hex: String,
    pub(crate) evidence_media_type: Option<String>,
    pub(crate) evidence_uri: Option<String>,
    pub(crate) evidence_size_bytes: Option<u64>,
    pub(crate) dispute_b64: String,
}

impl RegistryDispute {
    /// Convert the dispute record into a JSON value.
    pub fn into_json(self) -> Result<Value, json::Error> {
        let mut map = Map::new();
        map.insert("dispute_id_hex".into(), Value::String(self.dispute_id_hex));
        map.insert(
            "provider_id_hex".into(),
            Value::String(self.provider_id_hex),
        );
        map.insert(
            "complainant_id_hex".into(),
            Value::String(self.complainant_id_hex),
        );
        map.insert(
            "replication_order_id_hex".into(),
            self.replication_order_id_hex
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        map.insert("kind".into(), Value::String(self.kind));
        map.insert(
            "submitted_epoch".into(),
            json::to_value(&self.submitted_epoch)?,
        );
        map.insert("description".into(), Value::String(self.description));
        map.insert(
            "requested_remedy".into(),
            self.requested_remedy
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        map.insert("status".into(), Value::String(self.status));
        map.insert(
            "resolution_epoch".into(),
            self.resolution_epoch
                .map(|value| json::to_value(&value))
                .transpose()?
                .unwrap_or(Value::Null),
        );
        map.insert(
            "resolution_outcome".into(),
            self.resolution_outcome
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        map.insert(
            "resolution_notes".into(),
            self.resolution_notes
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        map.insert(
            "evidence_digest_hex".into(),
            Value::String(self.evidence_digest_hex),
        );
        map.insert(
            "evidence_media_type".into(),
            self.evidence_media_type
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        map.insert(
            "evidence_uri".into(),
            self.evidence_uri.map(Value::String).unwrap_or(Value::Null),
        );
        map.insert(
            "evidence_size_bytes".into(),
            self.evidence_size_bytes
                .map(|value| json::to_value(&value))
                .transpose()?
                .unwrap_or(Value::Null),
        );
        map.insert("dispute_b64".into(), Value::String(self.dispute_b64));
        Ok(Value::Object(map))
    }
}

/// Errors raised while preparing registry projections.
#[derive(Debug, Error)]
pub(crate) enum RegistryError {
    #[error("failed to decode capacity declaration for {provider_id_hex}: {source}")]
    DecodeDeclaration {
        provider_id_hex: String,
        #[source]
        source: NoritoDecodeError,
    },
    #[error("failed to serialize capacity declaration JSON for {provider_id_hex}: {source}")]
    SerializeDeclaration {
        provider_id_hex: String,
        #[source]
        source: json::Error,
    },
    #[error("failed to serialize metadata for {provider_id_hex}: {source}")]
    SerializeMetadata {
        provider_id_hex: String,
        #[source]
        source: json::Error,
    },
}

fn build_declarations<'a, I>(records: I) -> Result<Vec<RegistryDeclaration>, RegistryError>
where
    I: IntoIterator<Item = (&'a ProviderId, &'a CapacityDeclarationRecord)>,
{
    let mut out = Vec::new();
    for (provider_id, record) in records.into_iter() {
        let provider_id_hex = provider_id.as_bytes().encode_hex::<String>();
        let declaration = norito::decode_from_bytes::<CapacityDeclarationV1>(&record.declaration)
            .map_err(|source| RegistryError::DecodeDeclaration {
            provider_id_hex: provider_id_hex.clone(),
            source,
        })?;

        let declaration_json = capacity_declaration_to_json(&declaration, &record.declaration)
            .map_err(|source| RegistryError::SerializeDeclaration {
                provider_id_hex: provider_id_hex.clone(),
                source,
            })?;

        let metadata_json = metadata_to_json(&record.metadata).map_err(|source| {
            RegistryError::SerializeMetadata {
                provider_id_hex: provider_id_hex.clone(),
                source,
            }
        })?;

        out.push(RegistryDeclaration {
            provider_id_hex,
            committed_capacity_gib: record.committed_capacity_gib,
            registered_epoch: record.registered_epoch,
            valid_from_epoch: record.valid_from_epoch,
            valid_until_epoch: record.valid_until_epoch,
            declaration_json,
            metadata_json,
        });
    }
    Ok(out)
}

fn build_fee_ledger<'a, I>(entries: I) -> Vec<RegistryFeeLedgerEntry>
where
    I: IntoIterator<Item = (&'a ProviderId, &'a CapacityFeeLedgerEntry)>,
{
    entries
        .into_iter()
        .map(|(provider_id, entry)| RegistryFeeLedgerEntry {
            provider_id_hex: provider_id.as_bytes().encode_hex::<String>(),
            total_declared_gib: entry.total_declared_gib,
            total_utilised_gib: entry.total_utilised_gib,
            storage_fee: entry.storage_fee.clone(),
            egress_fee: entry.egress_fee.clone(),
            accrued_fee: entry.accrued_fee.clone(),
            expected_settlement: entry.expected_settlement.clone(),
            penalty_slashed: entry.penalty_slashed.clone(),
            penalty_events: entry.penalty_events,
            last_updated_epoch: entry.last_updated_epoch,
        })
        .collect()
}

fn build_credit_ledger<'a, I>(entries: I) -> Vec<RegistryCreditLedgerEntry>
where
    I: IntoIterator<Item = (&'a ProviderId, &'a ProviderCreditRecord)>,
{
    entries
        .into_iter()
        .map(|(provider_id, record)| {
            let metadata_json =
                metadata_to_json(&record.metadata).unwrap_or_else(|_| Value::Object(Map::new()));
            RegistryCreditLedgerEntry {
                provider_id_hex: provider_id.as_bytes().encode_hex::<String>(),
                available_credit: record.available_credit.clone(),
                bonded: record.bonded.clone(),
                required_bond: record.required_bond.clone(),
                expected_settlement: record.expected_settlement.clone(),
                onboarding_epoch: record.onboarding_epoch,
                last_settlement_epoch: record.last_settlement_epoch,
                low_balance_since_epoch: record.low_balance_since_epoch,
                slashed: record.slashed.clone(),
                under_delivery_strikes: record.under_delivery_strikes,
                last_penalty_epoch: record.last_penalty_epoch,
                metadata_json,
            }
        })
        .collect()
}

fn build_disputes<'a, I>(entries: I) -> Vec<RegistryDispute>
where
    I: IntoIterator<Item = (&'a CapacityDisputeId, &'a CapacityDisputeRecord)>,
{
    entries
        .into_iter()
        .map(|(dispute_id, record)| {
            let dispute_b64 = BASE64_STD.encode(&record.dispute_payload);
            let replication_order_id_hex = record
                .replication_order_id
                .map(|id| id.encode_hex::<String>());
            let status = match &record.status {
                CapacityDisputeStatus::Pending => "pending".to_owned(),
                CapacityDisputeStatus::Resolved(_) => "resolved".to_owned(),
            };
            let (resolution_epoch, resolution_outcome, resolution_notes) = match &record.status {
                CapacityDisputeStatus::Pending => (None, None, None),
                CapacityDisputeStatus::Resolved(resolution) => (
                    Some(resolution.resolved_epoch),
                    Some(match resolution.outcome {
                        iroha_data_model::sorafs::capacity::CapacityDisputeOutcome::Upheld => {
                            "upheld"
                        }
                        iroha_data_model::sorafs::capacity::CapacityDisputeOutcome::Dismissed => {
                            "dismissed"
                        }
                        iroha_data_model::sorafs::capacity::CapacityDisputeOutcome::Withdrawn => {
                            "withdrawn"
                        }
                    }
                    .to_owned()),
                    resolution.notes.clone(),
                ),
            };

            RegistryDispute {
                dispute_id_hex: dispute_id.as_bytes().encode_hex::<String>(),
                provider_id_hex: record.provider_id.as_bytes().encode_hex::<String>(),
                complainant_id_hex: record.complainant_id.encode_hex::<String>(),
                replication_order_id_hex,
                kind: dispute_kind_label(record.kind).to_owned(),
                submitted_epoch: record.submitted_epoch,
                description: record.description.clone(),
                requested_remedy: record.requested_remedy.clone(),
                status,
                resolution_epoch,
                resolution_outcome,
                resolution_notes,
                evidence_digest_hex: record.evidence.digest.encode_hex::<String>(),
                evidence_media_type: record.evidence.media_type.clone(),
                evidence_uri: record.evidence.uri.clone(),
                evidence_size_bytes: record.evidence.size_bytes,
                dispute_b64,
            }
        })
        .collect()
}

fn dispute_kind_label(kind: u8) -> &'static str {
    match kind {
        1 => "replication_shortfall",
        2 => "uptime_breach",
        3 => "proof_failure",
        4 => "fee_dispute",
        _ => "other",
    }
}

fn capacity_declaration_to_json(
    declaration: &CapacityDeclarationV1,
    canonical_bytes: &[u8],
) -> Result<Value, json::Error> {
    let mut map = Map::new();
    map.insert("version".into(), json::to_value(&declaration.version)?);
    map.insert(
        "provider_id_hex".into(),
        Value::String(declaration.provider_id.encode_hex::<String>()),
    );
    map.insert("stake".into(), stake_pointer_to_json(&declaration.stake)?);
    map.insert(
        "committed_capacity_gib".into(),
        json::to_value(&declaration.committed_capacity_gib)?,
    );
    map.insert(
        "chunker_commitments".into(),
        Value::Array(
            declaration
                .chunker_commitments
                .iter()
                .map(chunker_commitment_to_json)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    );
    map.insert(
        "lane_commitments".into(),
        Value::Array(
            declaration
                .lane_commitments
                .iter()
                .map(lane_commitment_to_json)
                .collect::<Result<Vec<_>, _>>()?,
        ),
    );
    map.insert(
        "pricing".into(),
        match declaration.pricing.as_ref() {
            Some(pricing) => pricing_to_json(pricing)?,
            None => Value::Null,
        },
    );
    map.insert(
        "valid_from".into(),
        json::to_value(&declaration.valid_from)?,
    );
    map.insert(
        "valid_until".into(),
        json::to_value(&declaration.valid_until)?,
    );
    map.insert(
        "metadata".into(),
        capacity_metadata_entries_to_json(&declaration.metadata),
    );
    map.insert(
        "norito_b64".into(),
        Value::String(BASE64_STD.encode(canonical_bytes)),
    );
    Ok(Value::Object(map))
}

fn stake_pointer_to_json(stake: &StakePointer) -> Result<Value, json::Error> {
    let mut map = Map::new();
    map.insert(
        "pool_id_hex".into(),
        Value::String(stake.pool_id.encode_hex::<String>()),
    );
    map.insert("stake_amount".into(), json::to_value(&stake.stake_amount)?);
    Ok(Value::Object(map))
}

fn chunker_commitment_to_json(commitment: &ChunkerCommitmentV1) -> Result<Value, json::Error> {
    let mut map = Map::new();
    map.insert("profile_id".into(), json::to_value(&commitment.profile_id)?);
    map.insert(
        "profile_aliases".into(),
        json::to_value(&commitment.profile_aliases)?,
    );
    map.insert(
        "committed_gib".into(),
        json::to_value(&commitment.committed_gib)?,
    );
    map.insert(
        "capability_refs".into(),
        Value::Array(
            commitment
                .capability_refs
                .iter()
                .map(|cap| capability_ref_to_json(*cap))
                .collect::<Result<Vec<_>, _>>()?,
        ),
    );
    Ok(Value::Object(map))
}

fn capability_ref_to_json(capability: CapabilityType) -> Result<Value, json::Error> {
    let mut map = Map::new();
    map.insert(
        "type".into(),
        Value::String(capability_name(capability).to_string()),
    );
    map.insert("type_id".into(), json::to_value(&(capability as u16))?);
    Ok(Value::Object(map))
}

fn lane_commitment_to_json(lane: &LaneCommitmentV1) -> Result<Value, json::Error> {
    let mut map = Map::new();
    map.insert("lane_id".into(), json::to_value(&lane.lane_id)?);
    map.insert("max_gib".into(), json::to_value(&lane.max_gib)?);
    Ok(Value::Object(map))
}

fn pricing_to_json(pricing: &PricingScheduleV1) -> Result<Value, json::Error> {
    let mut map = Map::new();
    map.insert("currency".into(), json::to_value(&pricing.currency)?);
    map.insert(
        "rate_per_gib_hour_milliu".into(),
        json::to_value(&pricing.rate_per_gib_hour_milliu)?,
    );
    map.insert(
        "min_commitment_hours".into(),
        json::to_value(&pricing.min_commitment_hours)?,
    );
    map.insert("notes".into(), json::to_value(&pricing.notes)?);
    Ok(Value::Object(map))
}

fn capacity_metadata_entries_to_json(entries: &[CapacityMetadataEntry]) -> Value {
    let mut metadata = Map::new();
    for entry in entries {
        metadata.insert(entry.key.clone(), Value::String(entry.value.clone()));
    }
    Value::Object(metadata)
}

fn metadata_to_json(metadata: &Metadata) -> Result<Value, json::Error> {
    json::to_value(metadata)
}

/// Pin registry snapshot containing manifests, aliases, and replication orders.
#[derive(Debug, Clone)]
pub(crate) struct PinRegistrySnapshot {
    pub(crate) manifests: Vec<RegistryManifest>,
    pub(crate) aliases: Vec<RegistryAlias>,
    pub(crate) replication_orders: Vec<RegistryReplicationOrder>,
}

/// Aggregated metrics extracted from a [`PinRegistrySnapshot`].
#[derive(Debug, Default, Clone)]
pub(crate) struct PinRegistryMetricsSummary {
    pub(crate) manifests_pending: u64,
    pub(crate) manifests_approved: u64,
    pub(crate) manifests_retired: u64,
    pub(crate) alias_total: u64,
    pub(crate) orders_pending: u64,
    pub(crate) orders_completed: u64,
    pub(crate) orders_expired: u64,
    pub(crate) sla_met: u64,
    pub(crate) sla_missed: u64,
    pub(crate) completion_latencies: Vec<f64>,
    pub(crate) deadline_slack_epochs: Vec<f64>,
}

impl PinRegistryMetricsSummary {
    /// Build a summary from the supplied snapshot.
    #[must_use]
    pub(crate) fn from_snapshot(snapshot: &PinRegistrySnapshot) -> Self {
        let mut summary = Self {
            alias_total: snapshot.aliases.len() as u64,
            ..Self::default()
        };

        for manifest in &snapshot.manifests {
            match manifest.status_label() {
                "pending" => {
                    summary.manifests_pending = summary.manifests_pending.saturating_add(1);
                }
                "approved" => {
                    summary.manifests_approved = summary.manifests_approved.saturating_add(1);
                }
                "retired" => {
                    summary.manifests_retired = summary.manifests_retired.saturating_add(1);
                }
                _ => {}
            }
        }

        for order in &snapshot.replication_orders {
            match order.status_label() {
                "pending" => {
                    summary.orders_pending = summary.orders_pending.saturating_add(1);
                    let slack = order.deadline_epoch().saturating_sub(order.issued_epoch()) as f64;
                    summary.deadline_slack_epochs.push(slack);
                }
                "completed" => {
                    summary.orders_completed = summary.orders_completed.saturating_add(1);
                    if let Some(completion_epoch) = order.completion_epoch() {
                        let latency = completion_epoch.saturating_sub(order.issued_epoch()) as f64;
                        summary.completion_latencies.push(latency);
                        if completion_epoch <= order.deadline_epoch() {
                            summary.sla_met = summary.sla_met.saturating_add(1);
                        } else {
                            summary.sla_missed = summary.sla_missed.saturating_add(1);
                        }
                    } else {
                        summary.sla_missed = summary.sla_missed.saturating_add(1);
                    }
                }
                "expired" => {
                    summary.orders_expired = summary.orders_expired.saturating_add(1);
                    summary.sla_missed = summary.sla_missed.saturating_add(1);
                }
                _ => {}
            }
        }

        summary
    }
}

impl PinRegistrySnapshot {
    pub(crate) fn manifest_by_digest(&self, digest_hex: &str) -> Option<&RegistryManifest> {
        self.manifests
            .iter()
            .find(|manifest| manifest.digest_hex == digest_hex)
    }

    pub(crate) fn lineage_for(&self, digest_hex: &str) -> ManifestLineageSummary {
        let Some(manifest) = self.manifest_by_digest(digest_hex) else {
            return ManifestLineageSummary {
                successor_of_hex: None,
                head_hex: digest_hex.to_owned(),
                depth_to_head: 0,
                approved_successor: None,
                immediate_successor: None,
                anomalies: vec!["ManifestMissing".to_string()],
            };
        };

        let mut anomalies = Vec::new();
        let mut visited = HashSet::new();
        visited.insert(digest_hex.to_owned());

        let selection = select_successor(&self.manifests, digest_hex);
        if selection.has_fork {
            anomalies.push("SuccessorForkResolved".to_string());
        }
        let immediate_successor = selection.best.map(lineage_successor_from);
        let mut approved_successor =
            immediate_successor
                .as_ref()
                .and_then(|succ| match succ.status {
                    ManifestStatusProjection::Approved { .. } => Some(succ.clone()),
                    _ => None,
                });

        let mut head = manifest;
        let mut depth: u32 = 0;
        let mut current = selection.best;
        let mut hops: usize = 0;

        while let Some(next) = current {
            hops = hops.saturating_add(1);
            if hops > MAX_LINEAGE_DEPTH {
                anomalies.push("LineageDepthExceeded".to_string());
                break;
            }

            if !visited.insert(next.digest_hex.clone()) {
                anomalies.push("LineageCycleDetected".to_string());
                break;
            }

            depth = depth.saturating_add(1);
            head = next;
            if approved_successor.is_none()
                && matches!(next.status, ManifestStatusProjection::Approved { .. })
            {
                approved_successor = Some(lineage_successor_from(next));
            }

            let next_selection = select_successor(&self.manifests, &next.digest_hex);
            if next_selection.has_fork {
                anomalies.push("SuccessorForkResolved".to_string());
            }
            current = next_selection.best;
        }

        ManifestLineageSummary {
            successor_of_hex: manifest.successor_of_hex.clone(),
            head_hex: head.digest_hex.clone(),
            depth_to_head: depth,
            approved_successor,
            immediate_successor,
            anomalies,
        }
    }
}

/// Record Prometheus metrics for the given registry snapshot.
pub(crate) fn record_pin_registry_metrics(
    telemetry: &MaybeTelemetry,
    snapshot: &PinRegistrySnapshot,
) {
    let summary = PinRegistryMetricsSummary::from_snapshot(snapshot);
    telemetry.with_metrics(|metrics| {
        metrics.record_sorafs_registry(
            summary.manifests_pending,
            summary.manifests_approved,
            summary.manifests_retired,
            summary.alias_total,
            summary.orders_pending,
            summary.orders_completed,
            summary.orders_expired,
            summary.sla_met,
            summary.sla_missed,
            &summary.completion_latencies,
            &summary.deadline_slack_epochs,
        );
    });
}

#[derive(Debug, Clone)]
pub(crate) struct RegistryManifest {
    digest: ManifestDigest,
    digest_hex: String,
    chunker: RegistryChunkerHandle,
    chunk_digest_hex: String,
    pin_policy: Value,
    submitted_by: String,
    submitted_epoch: u64,
    alias: Option<AliasBindingProjection>,
    metadata: Value,
    successor_of_hex: Option<String>,
    status: ManifestStatusProjection,
    council_envelope_digest_hex: Option<String>,
    status_timestamp_unix: Option<u64>,
    governance_refs: Vec<GovernanceReference>,
}

#[derive(Debug, Clone)]
struct RegistryChunkerHandle {
    profile_id: u32,
    namespace: String,
    name: String,
    semver: String,
    multihash_code: u64,
}

#[derive(Debug, Clone)]
struct AliasBindingProjection {
    namespace: String,
    name: String,
    proof_b64: String,
}

#[derive(Debug, Clone)]
pub(crate) enum ManifestStatusProjection {
    Pending,
    Approved { epoch: u64 },
    Retired { epoch: u64 },
}

#[derive(Debug, Clone)]
pub(crate) struct RegistryAlias {
    alias_label: String,
    namespace: String,
    name: String,
    manifest_digest_hex: String,
    bound_by: String,
    bound_epoch: u64,
    expiry_epoch: u64,
    proof_b64: String,
}

#[derive(Debug, Clone)]
pub(crate) struct RegistryReplicationOrder {
    order_id_hex: String,
    manifest_digest_hex: String,
    manifest_cid: Vec<u8>,
    issued_by: String,
    issued_epoch: u64,
    deadline_epoch: u64,
    status: ReplicationOrderStatusProjection,
    canonical_order_b64: String,
    order_json: Value,
    providers: Vec<String>,
    provider_completions: Vec<RegistryReplicationCompletion>,
}

#[derive(Debug, Clone)]
struct RegistryReplicationCompletion {
    provider_hex: String,
    completed_by: String,
    completion_epoch: u64,
}

#[derive(Debug, Clone)]
enum ReplicationOrderStatusProjection {
    Pending,
    Completed { epoch: u64 },
    Expired { epoch: u64 },
}

#[derive(Debug, Clone)]
pub(crate) struct ManifestLineageSummary {
    pub successor_of_hex: Option<String>,
    pub head_hex: String,
    pub depth_to_head: u32,
    pub approved_successor: Option<LineageSuccessor>,
    pub immediate_successor: Option<LineageSuccessor>,
    pub anomalies: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct LineageSuccessor {
    pub digest_hex: String,
    pub status: ManifestStatusProjection,
    pub approved_epoch: Option<u64>,
    pub status_timestamp_unix: Option<u64>,
}

#[derive(Debug, Clone)]
pub(crate) struct GovernanceSummary {
    pub references: Vec<GovernanceReference>,
    pub revoked: Option<GovernanceReference>,
    pub frozen: Option<GovernanceReference>,
    pub rotated: Option<GovernanceReference>,
}

#[derive(Debug, Clone)]
pub(crate) struct GovernanceReference {
    pub cid: Option<String>,
    pub kind: GovernanceRefKind,
    pub effective_at_unix: Option<u64>,
    pub alias_label: Option<String>,
    pub manifest_digest_hex: Option<String>,
    pub signers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GovernanceRefKind {
    AliasRotate,
    FreezeAlias,
    UnfreezeAlias,
    RevokeManifest,
    Other(String),
}

const MAX_LINEAGE_DEPTH: usize = 64;

fn unix_to_rfc3339_string(unix: u64) -> Option<String> {
    let seconds = i64::try_from(unix).ok()?;
    let timestamp = OffsetDateTime::from_unix_timestamp(seconds).ok()?;
    timestamp.format(&Rfc3339).ok()
}

pub(crate) fn optional_rfc3339(unix: Option<u64>) -> Option<String> {
    unix.and_then(unix_to_rfc3339_string)
}

struct SuccessorSelection<'a> {
    best: Option<&'a RegistryManifest>,
    has_fork: bool,
}

fn metadata_timestamp_hint(metadata: &Metadata, key: &str) -> Option<u64> {
    let name = Name::from_str(key).ok()?;
    metadata.get(&name).and_then(json_u64)
}

fn json_u64(value: &Json) -> Option<u64> {
    value
        .try_into_any::<Value>()
        .ok()
        .and_then(|json| norito::json::Value::as_u64(&json))
}

fn governance_refs_from_metadata(
    metadata: &Metadata,
    manifest_digest_hex: &str,
) -> Vec<GovernanceReference> {
    let name = match Name::from_str(METADATA_GOVERNANCE_REFS_KEY) {
        Ok(name) => name,
        Err(_) => return Vec::new(),
    };
    let value = match metadata.get(&name) {
        Some(value) => value,
        None => return Vec::new(),
    };
    let parsed: Value = match value.try_into_any::<Value>().ok() {
        Some(parsed) => parsed,
        None => return Vec::new(),
    };
    let Value::Array(entries) = parsed else {
        return Vec::new();
    };

    entries
        .into_iter()
        .filter_map(|entry| parse_governance_reference(entry, manifest_digest_hex))
        .collect()
}

fn parse_governance_reference(
    value: Value,
    manifest_digest_hex: &str,
) -> Option<GovernanceReference> {
    let object = value.as_object()?;

    let kind = object
        .get("kind")
        .and_then(norito::json::Value::as_str)
        .map(parse_governance_kind)?;
    let cid = object
        .get("cid")
        .and_then(norito::json::Value::as_str)
        .map(str::to_owned);
    let effective_at_unix = object
        .get("effective_at")
        .and_then(norito::json::Value::as_u64);

    let targets = object.get("targets").and_then(|raw| raw.as_object());
    let alias_label = targets
        .and_then(|map| map.get("alias"))
        .and_then(norito::json::Value::as_str)
        .map(str::to_owned);
    let manifest_digest_override = targets
        .and_then(|map| map.get("pin_digest_hex"))
        .and_then(norito::json::Value::as_str)
        .map(str::to_owned);

    let manifest_digest_hex = manifest_digest_override.or_else(|| match kind {
        GovernanceRefKind::RevokeManifest => Some(manifest_digest_hex.to_owned()),
        _ => None,
    });

    let signers = object
        .get("signers")
        .and_then(|raw| raw.as_array())
        .map(|array| {
            array
                .iter()
                .filter_map(|entry| entry.as_str().map(str::to_owned))
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();

    Some(GovernanceReference {
        cid,
        kind,
        effective_at_unix,
        alias_label,
        manifest_digest_hex,
        signers,
    })
}

fn parse_governance_kind(raw: &str) -> GovernanceRefKind {
    match raw {
        "AliasRotate" => GovernanceRefKind::AliasRotate,
        "FreezeAlias" => GovernanceRefKind::FreezeAlias,
        "UnfreezeAlias" => GovernanceRefKind::UnfreezeAlias,
        "RevokeManifest" => GovernanceRefKind::RevokeManifest,
        other => GovernanceRefKind::Other(other.to_owned()),
    }
}

impl GovernanceReference {
    fn to_json(&self) -> Value {
        let mut map = Map::new();
        map.insert(
            "cid".into(),
            self.cid
                .as_ref()
                .map(|cid| Value::String(cid.clone()))
                .unwrap_or(Value::Null),
        );
        map.insert("kind".into(), Value::String(self.kind.as_str().to_owned()));
        map.insert(
            "effective_at".into(),
            optional_rfc3339(self.effective_at_unix)
                .map(Value::String)
                .unwrap_or(Value::Null),
        );
        map.insert(
            "effective_at_unix".into(),
            json::to_value(&self.effective_at_unix).unwrap_or(Value::Null),
        );
        let mut targets = Map::new();
        if let Some(alias) = &self.alias_label {
            targets.insert("alias".into(), Value::String(alias.clone()));
        }
        if let Some(digest) = &self.manifest_digest_hex {
            targets.insert("pin_digest_hex".into(), Value::String(digest.clone()));
        }
        map.insert("targets".into(), Value::Object(targets));
        let signers = self.signers.iter().cloned().map(Value::String).collect();
        map.insert("signers".into(), Value::Array(signers));
        Value::Object(map)
    }
}

impl GovernanceRefKind {
    fn as_str(&self) -> &str {
        match self {
            GovernanceRefKind::AliasRotate => "AliasRotate",
            GovernanceRefKind::FreezeAlias => "FreezeAlias",
            GovernanceRefKind::UnfreezeAlias => "UnfreezeAlias",
            GovernanceRefKind::RevokeManifest => "RevokeManifest",
            GovernanceRefKind::Other(value) => value.as_str(),
        }
    }
}

fn lineage_successor_from(manifest: &RegistryManifest) -> LineageSuccessor {
    LineageSuccessor {
        digest_hex: manifest.digest_hex.clone(),
        status: manifest.status.clone(),
        approved_epoch: manifest.status.approved_epoch(),
        status_timestamp_unix: manifest.status_timestamp_unix,
    }
}

#[cfg(test)]
pub(crate) fn approved_successor_for_tests(
    digest_hex: impl Into<String>,
    approved_epoch: u64,
    status_timestamp_unix: Option<u64>,
) -> LineageSuccessor {
    LineageSuccessor {
        digest_hex: digest_hex.into(),
        status: ManifestStatusProjection::Approved {
            epoch: approved_epoch,
        },
        approved_epoch: Some(approved_epoch),
        status_timestamp_unix,
    }
}

fn lineage_successor_to_json(successor: &LineageSuccessor) -> Value {
    let mut map = Map::new();
    map.insert(
        "digest_hex".into(),
        Value::String(successor.digest_hex.clone()),
    );
    map.insert(
        "status".into(),
        successor
            .status
            .to_json()
            .unwrap_or_else(|_| Value::String("unknown".to_owned())),
    );
    map.insert(
        "approved_epoch".into(),
        json::to_value(&successor.approved_epoch).unwrap_or(Value::Null),
    );
    map.insert(
        "approved_at".into(),
        optional_rfc3339(successor.status_timestamp_unix)
            .map(Value::String)
            .unwrap_or(Value::Null),
    );
    map.insert(
        "status_timestamp_unix".into(),
        json::to_value(&successor.status_timestamp_unix).unwrap_or(Value::Null),
    );
    Value::Object(map)
}

pub(crate) fn lineage_to_json(lineage: &ManifestLineageSummary) -> Value {
    let mut map = Map::new();
    map.insert(
        "successor_of_hex".into(),
        lineage
            .successor_of_hex
            .as_ref()
            .map(|hex| Value::String(hex.clone()))
            .unwrap_or(Value::Null),
    );
    map.insert("head_hex".into(), Value::String(lineage.head_hex.clone()));
    map.insert(
        "depth_to_head".into(),
        json::to_value(&lineage.depth_to_head).unwrap_or(Value::Null),
    );
    let is_head = lineage.immediate_successor.is_none();
    map.insert("is_head".into(), Value::Bool(is_head));
    map.insert(
        "superseded_by".into(),
        lineage
            .approved_successor
            .as_ref()
            .map(lineage_successor_to_json)
            .unwrap_or(Value::Null),
    );
    map.insert(
        "immediate_successor".into(),
        lineage
            .immediate_successor
            .as_ref()
            .map(lineage_successor_to_json)
            .unwrap_or(Value::Null),
    );
    map.insert(
        "anomalies".into(),
        Value::Array(
            lineage
                .anomalies
                .iter()
                .cloned()
                .map(Value::String)
                .collect(),
        ),
    );
    Value::Object(map)
}

fn select_successor<'a>(
    manifests: &'a [RegistryManifest],
    predecessor_hex: &str,
) -> SuccessorSelection<'a> {
    let mut approved_best: Option<&RegistryManifest> = None;
    let mut approved_epoch: Option<u64> = None;
    let mut pending_best: Option<&RegistryManifest> = None;
    let mut pending_epoch: Option<u64> = None;
    let mut total = 0usize;

    for manifest in manifests
        .iter()
        .filter(|candidate| candidate.successor_of_hex.as_deref() == Some(predecessor_hex))
    {
        total = total.saturating_add(1);
        match manifest.status {
            ManifestStatusProjection::Approved { epoch } => {
                let better = match approved_epoch {
                    Some(best_epoch) if epoch > best_epoch => true,
                    Some(best_epoch) if epoch == best_epoch => {
                        manifest.digest_hex > approved_best.unwrap().digest_hex
                    }
                    Some(_) => false,
                    None => true,
                };
                if better {
                    approved_best = Some(manifest);
                    approved_epoch = Some(epoch);
                }
            }
            _ => {
                let submitted = manifest.submitted_epoch;
                let better = match pending_epoch {
                    Some(best_epoch) if submitted > best_epoch => true,
                    Some(best_epoch) if submitted == best_epoch => {
                        manifest.digest_hex > pending_best.unwrap().digest_hex
                    }
                    Some(_) => false,
                    None => true,
                };
                if better {
                    pending_best = Some(manifest);
                    pending_epoch = Some(submitted);
                }
            }
        }
    }

    let best = approved_best.or(pending_best);
    let has_fork = match best {
        Some(_) => total.saturating_sub(1) > 0,
        None => false,
    };

    SuccessorSelection { best, has_fork }
}

/// Errors raised while preparing pin registry projections.
#[derive(Debug, Error)]
pub(crate) enum PinRegistryError {
    #[error("failed to serialize manifest metadata for {digest_hex}: {source}")]
    SerializeManifestMetadata {
        digest_hex: String,
        #[source]
        source: json::Error,
    },
    #[error("failed to serialize manifest policy for {digest_hex}: {source}")]
    SerializeManifestPolicy {
        digest_hex: String,
        #[source]
        source: json::Error,
    },
    #[error("failed to decode replication order payload for {order_id_hex}: {source}")]
    DecodeReplicationOrder {
        order_id_hex: String,
        #[source]
        source: NoritoDecodeError,
    },
    #[error("failed to serialize replication order payload for {order_id_hex}: {source}")]
    SerializeReplicationOrder {
        order_id_hex: String,
        #[source]
        source: json::Error,
    },
    #[error("invalid canonical replication order payload for {order_id_hex}: {reason}")]
    InvalidReplicationOrder {
        order_id_hex: String,
        reason: String,
    },
}

/// Collect the current pin registry snapshot (manifests, aliases, and replication orders).
pub(crate) fn collect_pin_registry(
    world: &WorldView<'_>,
) -> Result<PinRegistrySnapshot, PinRegistryError> {
    let mut manifests = Vec::new();
    for (digest, record) in world.pin_manifests().iter() {
        manifests.push(RegistryManifest::from_store(digest, record)?);
    }
    manifests.sort_by(|a, b| a.digest_hex.cmp(&b.digest_hex));

    let mut aliases = Vec::new();
    for (alias_id, record) in world.manifest_aliases().iter() {
        aliases.push(RegistryAlias::from_store(alias_id, record));
    }
    aliases.sort_by(|a, b| a.alias_label.cmp(&b.alias_label));

    let mut replication_orders = Vec::new();
    for (order_id, record) in world.replication_orders().iter() {
        replication_orders.push(RegistryReplicationOrder::from_store(order_id, record)?);
    }
    replication_orders.sort_by(|a, b| a.order_id_hex.cmp(&b.order_id_hex));

    Ok(PinRegistrySnapshot {
        manifests,
        aliases,
        replication_orders,
    })
}

impl RegistryManifest {
    fn from_store(
        digest: &ManifestDigest,
        record: &PinManifestRecord,
    ) -> Result<Self, PinRegistryError> {
        let digest_hex = digest.as_bytes().encode_hex::<String>();
        let chunker = RegistryChunkerHandle {
            profile_id: record.chunker.profile_id,
            namespace: record.chunker.namespace.clone(),
            name: record.chunker.name.clone(),
            semver: record.chunker.semver.clone(),
            multihash_code: record.chunker.multihash_code,
        };

        let pin_policy = pin_policy_to_json(&record.policy).map_err(|source| {
            PinRegistryError::SerializeManifestPolicy {
                digest_hex: digest_hex.clone(),
                source,
            }
        })?;

        let metadata = metadata_to_json(&record.metadata).map_err(|source| {
            PinRegistryError::SerializeManifestMetadata {
                digest_hex: digest_hex.clone(),
                source,
            }
        })?;

        let alias = record.alias.as_ref().map(|binding| AliasBindingProjection {
            namespace: binding.namespace.clone(),
            name: binding.name.clone(),
            proof_b64: BASE64_STD.encode(&binding.proof),
        });

        let status = match record.status {
            PinStatus::Pending => ManifestStatusProjection::Pending,
            PinStatus::Approved(epoch) => ManifestStatusProjection::Approved { epoch },
            PinStatus::Retired(epoch) => ManifestStatusProjection::Retired { epoch },
        };
        let successor_of_hex = record
            .successor_of
            .as_ref()
            .map(|parent| parent.as_bytes().encode_hex::<String>());
        let status_timestamp_unix =
            metadata_timestamp_hint(&record.metadata, METADATA_STATUS_TIMESTAMP_KEY);
        let governance_refs = governance_refs_from_metadata(&record.metadata, &digest_hex);

        Ok(Self {
            digest: *digest,
            digest_hex,
            chunker,
            chunk_digest_hex: record.chunk_digest_sha3_256.encode_hex::<String>(),
            pin_policy,
            submitted_by: record.submitted_by.to_string(),
            submitted_epoch: record.submitted_epoch,
            alias,
            metadata,
            successor_of_hex,
            status,
            council_envelope_digest_hex: record
                .council_envelope_digest
                .map(|digest| digest.encode_hex::<String>()),
            status_timestamp_unix,
            governance_refs,
        })
    }

    pub(crate) fn successor_of_hex(&self) -> Option<&str> {
        self.successor_of_hex.as_deref()
    }

    pub(crate) fn status_timestamp_unix(&self) -> Option<u64> {
        self.status_timestamp_unix
    }

    pub(crate) fn governance_summary(&self) -> GovernanceSummary {
        let mut revoked = None;
        let mut frozen = None;
        let mut rotated = None;

        for reference in &self.governance_refs {
            match reference.kind {
                GovernanceRefKind::RevokeManifest => revoked = Some(reference.clone()),
                GovernanceRefKind::FreezeAlias => frozen = Some(reference.clone()),
                GovernanceRefKind::AliasRotate => rotated = Some(reference.clone()),
                GovernanceRefKind::UnfreezeAlias | GovernanceRefKind::Other(_) => {}
            }
        }

        GovernanceSummary {
            references: self.governance_refs.clone(),
            revoked,
            frozen,
            rotated,
        }
    }

    pub(crate) fn status_label(&self) -> &'static str {
        self.status.label()
    }

    pub(crate) fn digest_hex(&self) -> &str {
        &self.digest_hex
    }

    pub(crate) fn chunker_handle(&self) -> String {
        format!(
            "{}.{}@{}",
            self.chunker.namespace, self.chunker.name, self.chunker.semver
        )
    }

    pub(crate) fn to_json(&self) -> Result<Value, json::Error> {
        let mut map = Map::new();
        map.insert("digest_hex".into(), Value::String(self.digest_hex.clone()));
        map.insert("chunker".into(), self.chunker.to_json()?);
        map.insert(
            "chunk_digest_sha3_256_hex".into(),
            Value::String(self.chunk_digest_hex.clone()),
        );
        map.insert("pin_policy".into(), self.pin_policy.clone());
        map.insert(
            "submitted_by".into(),
            Value::String(self.submitted_by.clone()),
        );
        map.insert(
            "submitted_epoch".into(),
            json::to_value(&self.submitted_epoch)?,
        );
        map.insert("status".into(), self.status.to_json()?);
        map.insert("metadata".into(), self.metadata.clone());
        if let Some(alias) = &self.alias {
            map.insert("alias".into(), alias.to_json());
        } else {
            map.insert("alias".into(), Value::Null);
        }
        map.insert(
            "successor_of_hex".into(),
            self.successor_of_hex
                .as_ref()
                .map(|hex| Value::String(hex.clone()))
                .unwrap_or(Value::Null),
        );
        map.insert(
            "status_timestamp_unix".into(),
            json::to_value(&self.status_timestamp_unix)?,
        );
        let governance_values = self
            .governance_refs
            .iter()
            .map(GovernanceReference::to_json)
            .collect();
        map.insert("governance_refs".into(), Value::Array(governance_values));
        map.insert(
            "council_envelope_digest_hex".into(),
            self.council_envelope_digest_hex
                .as_ref()
                .map(|hex| Value::String(hex.clone()))
                .unwrap_or(Value::Null),
        );
        Ok(Value::Object(map))
    }
}

impl RegistryChunkerHandle {
    fn to_json(&self) -> Result<Value, json::Error> {
        let mut map = Map::new();
        map.insert("profile_id".into(), json::to_value(&self.profile_id)?);
        map.insert("namespace".into(), Value::String(self.namespace.clone()));
        map.insert("name".into(), Value::String(self.name.clone()));
        map.insert("semver".into(), Value::String(self.semver.clone()));
        map.insert(
            "multihash_code".into(),
            json::to_value(&self.multihash_code)?,
        );
        Ok(Value::Object(map))
    }
}

impl AliasBindingProjection {
    fn to_json(&self) -> Value {
        let mut map = Map::new();
        map.insert("namespace".into(), Value::String(self.namespace.clone()));
        map.insert("name".into(), Value::String(self.name.clone()));
        map.insert("proof_b64".into(), Value::String(self.proof_b64.clone()));
        Value::Object(map)
    }
}

impl ManifestStatusProjection {
    fn label(&self) -> &'static str {
        match self {
            ManifestStatusProjection::Pending => "pending",
            ManifestStatusProjection::Approved { .. } => "approved",
            ManifestStatusProjection::Retired { .. } => "retired",
        }
    }

    fn approved_epoch(&self) -> Option<u64> {
        if let Self::Approved { epoch } = self {
            Some(*epoch)
        } else {
            None
        }
    }

    fn to_json(&self) -> Result<Value, json::Error> {
        let mut map = Map::new();
        map.insert("state".into(), Value::String(self.label().to_owned()));
        match self {
            ManifestStatusProjection::Approved { epoch }
            | ManifestStatusProjection::Retired { epoch } => {
                map.insert("epoch".into(), json::to_value(epoch)?);
            }
            ManifestStatusProjection::Pending => {}
        }
        Ok(Value::Object(map))
    }
}

impl RegistryAlias {
    fn from_store(alias_id: &ManifestAliasId, record: &ManifestAliasRecord) -> Self {
        let alias_label = alias_id.as_label();
        Self {
            alias_label,
            namespace: alias_id.namespace.clone(),
            name: alias_id.name.clone(),
            manifest_digest_hex: record.manifest.as_bytes().encode_hex::<String>(),
            bound_by: record.bound_by.to_string(),
            bound_epoch: record.bound_epoch,
            expiry_epoch: record.expiry_epoch,
            proof_b64: BASE64_STD.encode(&record.binding.proof),
        }
    }

    pub(crate) fn alias_label(&self) -> &str {
        &self.alias_label
    }

    pub(crate) fn namespace(&self) -> &str {
        &self.namespace
    }

    pub(crate) fn manifest_digest_hex(&self) -> &str {
        &self.manifest_digest_hex
    }

    pub(crate) fn proof_b64(&self) -> &str {
        &self.proof_b64
    }

    pub(crate) fn to_json(&self) -> Result<Value, json::Error> {
        let mut map = Map::new();
        map.insert("alias".into(), Value::String(self.alias_label.clone()));
        map.insert("namespace".into(), Value::String(self.namespace.clone()));
        map.insert("name".into(), Value::String(self.name.clone()));
        map.insert(
            "manifest_digest_hex".into(),
            Value::String(self.manifest_digest_hex.clone()),
        );
        map.insert("bound_by".into(), Value::String(self.bound_by.clone()));
        map.insert("bound_epoch".into(), json::to_value(&self.bound_epoch)?);
        map.insert("expiry_epoch".into(), json::to_value(&self.expiry_epoch)?);
        map.insert("proof_b64".into(), Value::String(self.proof_b64.clone()));
        Ok(Value::Object(map))
    }
}

impl RegistryReplicationOrder {
    fn from_store(
        order_id: &ReplicationOrderId,
        record: &ReplicationOrderRecord,
    ) -> Result<Self, PinRegistryError> {
        let order_id_hex = order_id.as_bytes().encode_hex::<String>();
        if record.canonical_order.is_empty()
            || record.canonical_order.len() > REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1
        {
            return Err(PinRegistryError::InvalidReplicationOrder {
                order_id_hex,
                reason: format!(
                    "canonical payload length must be 1..={REPLICATION_ORDER_MAX_CANONICAL_BYTES_V1} bytes"
                ),
            });
        }
        let order: ReplicationOrderV1 = decode_from_bytes_with_limits(
            &record.canonical_order,
            REPLICATION_ORDER_DECODE_LIMITS_V1,
        )
        .map_err(|source| PinRegistryError::DecodeReplicationOrder {
            order_id_hex: order_id_hex.clone(),
            source,
        })?;
        order
            .validate()
            .map_err(|error| PinRegistryError::InvalidReplicationOrder {
                order_id_hex: order_id_hex.clone(),
                reason: error.to_string(),
            })?;
        let canonical =
            to_bytes(&order).map_err(|error| PinRegistryError::InvalidReplicationOrder {
                order_id_hex: order_id_hex.clone(),
                reason: format!("failed to re-encode payload: {error}"),
            })?;
        if canonical != record.canonical_order
            || order.order_id != *record.order_id.as_bytes()
            || order.manifest_digest != *record.manifest_digest.as_bytes()
            || order.manifest_cid.as_slice() != record.manifest_root_cid.as_bytes()
        {
            return Err(PinRegistryError::InvalidReplicationOrder {
                order_id_hex,
                reason: "payload is non-canonical or substituted from its ledger record".to_owned(),
            });
        }
        let target_replicas = usize::from(order.target_replicas);
        if record.provider_completions.len() > target_replicas {
            return Err(PinRegistryError::InvalidReplicationOrder {
                order_id_hex,
                reason: "provider completion count exceeds the redundancy target".to_owned(),
            });
        }
        let mut completed_providers = HashSet::new();
        for completion in &record.provider_completions {
            if !order
                .assignments
                .iter()
                .any(|assignment| assignment.provider_id == *completion.provider_id.as_bytes())
                || !completed_providers.insert(*completion.provider_id.as_bytes())
                || completion.completion_epoch < record.issued_epoch
                || completion.completion_epoch > record.deadline_epoch
            {
                return Err(PinRegistryError::InvalidReplicationOrder {
                    order_id_hex,
                    reason: "provider completion state is duplicate, unassigned, or outside the ledger epoch window".to_owned(),
                });
            }
        }
        let lifecycle_consistent = match record.status {
            ReplicationOrderStatus::Pending | ReplicationOrderStatus::Expired(_) => {
                record.provider_completions.len() < target_replicas
            }
            ReplicationOrderStatus::Completed(epoch) => {
                record.provider_completions.len() == target_replicas
                    && record
                        .provider_completions
                        .last()
                        .is_some_and(|completion| completion.completion_epoch == epoch)
            }
        };
        if !lifecycle_consistent {
            return Err(PinRegistryError::InvalidReplicationOrder {
                order_id_hex,
                reason: "lifecycle is inconsistent with provider completion state".to_owned(),
            });
        }
        let order_json = replication_order_to_json(&order).map_err(|source| {
            PinRegistryError::SerializeReplicationOrder {
                order_id_hex: order_id_hex.clone(),
                source,
            }
        })?;

        let providers = order
            .assignments
            .iter()
            .map(|assignment| hex::encode(assignment.provider_id))
            .collect();
        let provider_completions = record
            .provider_completions
            .iter()
            .map(|completion| RegistryReplicationCompletion {
                provider_hex: hex::encode(completion.provider_id.as_bytes()),
                completed_by: completion.completed_by.to_string(),
                completion_epoch: completion.completion_epoch,
            })
            .collect();

        let status = match record.status {
            ReplicationOrderStatus::Pending => ReplicationOrderStatusProjection::Pending,
            ReplicationOrderStatus::Completed(epoch) => {
                ReplicationOrderStatusProjection::Completed { epoch }
            }
            ReplicationOrderStatus::Expired(epoch) => {
                ReplicationOrderStatusProjection::Expired { epoch }
            }
        };

        Ok(Self {
            order_id_hex,
            manifest_digest_hex: record.manifest_digest.as_bytes().encode_hex::<String>(),
            manifest_cid: order.manifest_cid.clone(),
            issued_by: record.issued_by.to_string(),
            issued_epoch: record.issued_epoch,
            deadline_epoch: record.deadline_epoch,
            status,
            canonical_order_b64: BASE64_STD.encode(&record.canonical_order),
            order_json,
            providers,
            provider_completions,
        })
    }

    pub(crate) fn status_label(&self) -> &'static str {
        self.status.label()
    }

    pub(crate) fn manifest_digest_hex(&self) -> &str {
        &self.manifest_digest_hex
    }

    pub(crate) fn manifest_cid(&self) -> &[u8] {
        &self.manifest_cid
    }

    pub(crate) fn providers(&self) -> &[String] {
        &self.providers
    }

    /// Epoch when the replication order was issued.
    pub(crate) fn issued_epoch(&self) -> u64 {
        self.issued_epoch
    }

    /// Epoch by which the replication order must complete.
    pub(crate) fn deadline_epoch(&self) -> u64 {
        self.deadline_epoch
    }

    /// Completion epoch when the order succeeded, if applicable.
    pub(crate) fn completion_epoch(&self) -> Option<u64> {
        match self.status {
            ReplicationOrderStatusProjection::Completed { epoch } => Some(epoch),
            _ => None,
        }
    }

    /// Whether the order expired without completion.
    pub(crate) fn is_expired(&self) -> bool {
        matches!(
            self.status,
            ReplicationOrderStatusProjection::Expired { .. }
        )
    }

    pub(crate) fn to_json(&self) -> Result<Value, json::Error> {
        let mut map = Map::new();
        map.insert(
            "order_id_hex".into(),
            Value::String(self.order_id_hex.clone()),
        );
        map.insert(
            "manifest_digest_hex".into(),
            Value::String(self.manifest_digest_hex.clone()),
        );
        map.insert("issued_by".into(), Value::String(self.issued_by.clone()));
        map.insert("issued_epoch".into(), json::to_value(&self.issued_epoch)?);
        map.insert(
            "deadline_epoch".into(),
            json::to_value(&self.deadline_epoch)?,
        );
        map.insert("status".into(), self.status.to_json()?);
        map.insert(
            "canonical_order_b64".into(),
            Value::String(self.canonical_order_b64.clone()),
        );
        map.insert("order".into(), self.order_json.clone());
        let provider_completions = self
            .provider_completions
            .iter()
            .map(RegistryReplicationCompletion::to_json)
            .collect::<Result<Vec<_>, _>>()?;
        map.insert(
            "provider_completions".into(),
            Value::Array(provider_completions),
        );
        let providers = self
            .providers
            .iter()
            .map(|provider| Value::String(provider.clone()))
            .collect();
        map.insert("providers".into(), Value::Array(providers));
        Ok(Value::Object(map))
    }
}

impl RegistryReplicationCompletion {
    fn to_json(&self) -> Result<Value, json::Error> {
        let mut map = Map::new();
        map.insert(
            "provider_hex".into(),
            Value::String(self.provider_hex.clone()),
        );
        map.insert(
            "completed_by".into(),
            Value::String(self.completed_by.clone()),
        );
        map.insert(
            "completion_epoch".into(),
            json::to_value(&self.completion_epoch)?,
        );
        Ok(Value::Object(map))
    }
}

impl ReplicationOrderStatusProjection {
    fn label(&self) -> &'static str {
        match self {
            ReplicationOrderStatusProjection::Pending => "pending",
            ReplicationOrderStatusProjection::Completed { .. } => "completed",
            ReplicationOrderStatusProjection::Expired { .. } => "expired",
        }
    }

    fn to_json(&self) -> Result<Value, json::Error> {
        let mut map = Map::new();
        map.insert("state".into(), Value::String(self.label().to_owned()));
        match self {
            ReplicationOrderStatusProjection::Completed { epoch }
            | ReplicationOrderStatusProjection::Expired { epoch } => {
                map.insert("epoch".into(), json::to_value(epoch)?);
            }
            ReplicationOrderStatusProjection::Pending => {}
        }
        Ok(Value::Object(map))
    }
}

fn pin_policy_to_json(policy: &PinPolicy) -> Result<Value, json::Error> {
    let mut map = Map::new();
    map.insert("min_replicas".into(), json::to_value(&policy.min_replicas)?);
    map.insert(
        "storage_class".into(),
        Value::String(storage_class_label(policy.storage_class).to_owned()),
    );
    map.insert(
        "retention_epoch".into(),
        json::to_value(&policy.retention_epoch)?,
    );
    Ok(Value::Object(map))
}

fn replication_order_to_json(order: &ReplicationOrderV1) -> Result<Value, json::Error> {
    let mut map = Map::new();
    map.insert("version".into(), json::to_value(&order.version)?);
    map.insert(
        "order_id_hex".into(),
        Value::String(order.order_id.encode_hex::<String>()),
    );
    map.insert(
        "manifest_cid_b64".into(),
        Value::String(BASE64_STD.encode(&order.manifest_cid)),
    );
    map.insert(
        "manifest_digest_hex".into(),
        Value::String(order.manifest_digest.encode_hex::<String>()),
    );
    map.insert(
        "chunking_profile".into(),
        Value::String(order.chunking_profile.clone()),
    );
    map.insert(
        "target_replicas".into(),
        json::to_value(&order.target_replicas)?,
    );
    let assignments = order
        .assignments
        .iter()
        .map(|assignment| {
            let mut assignment_json = Map::new();
            assignment_json.insert(
                "provider_id_hex".into(),
                Value::String(assignment.provider_id.encode_hex::<String>()),
            );
            assignment_json.insert("slice_gib".into(), json::to_value(&assignment.slice_gib)?);
            assignment_json.insert(
                "lane".into(),
                assignment
                    .lane
                    .as_ref()
                    .map(|lane| Value::String(lane.clone()))
                    .unwrap_or(Value::Null),
            );
            Ok(Value::Object(assignment_json))
        })
        .collect::<Result<Vec<_>, json::Error>>()?;
    map.insert("assignments".into(), Value::Array(assignments));
    map.insert("issued_at".into(), json::to_value(&order.issued_at)?);
    map.insert("deadline_at".into(), json::to_value(&order.deadline_at)?);
    let mut sla = Map::new();
    sla.insert(
        "ingest_deadline_secs".into(),
        json::to_value(&order.sla.ingest_deadline_secs)?,
    );
    sla.insert(
        "min_availability_percent_milli".into(),
        json::to_value(&order.sla.min_availability_percent_milli)?,
    );
    sla.insert(
        "min_por_success_percent_milli".into(),
        json::to_value(&order.sla.min_por_success_percent_milli)?,
    );
    map.insert("sla".into(), Value::Object(sla));
    map.insert("metadata".into(), json::to_value(&order.metadata)?);
    Ok(Value::Object(map))
}

fn storage_class_label(class: StorageClass) -> &'static str {
    match class {
        StorageClass::Hot => "hot",
        StorageClass::Warm => "warm",
        StorageClass::Cold => "cold",
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STD};
    use iroha_crypto::PublicKey;
    use iroha_data_model::{
        account::AccountId,
        domain::DomainId,
        metadata::Metadata,
        sorafs::{
            capacity::{CapacityDeclarationRecord, CapacityFeeLedgerEntry},
            pin_registry::{
                ChunkerProfileHandle, ManifestAliasBinding, ManifestDigest, ManifestRootCid,
                PinManifestRecord, PinPolicy, PinStatus, ReplicationOrderCompletionRecord,
                ReplicationOrderId, ReplicationOrderRecord, ReplicationOrderStatus, StorageClass,
            },
            pricing::ProviderCreditRecord,
        },
    };
    use sorafs_manifest::{
        capacity::{
            CapacityDeclarationV1, ChunkerCommitmentV1, LaneCommitmentV1, PricingScheduleV1,
            REPLICATION_ORDER_VERSION_V1, ReplicationAssignmentV1, ReplicationOrderSlaV1,
            ReplicationOrderV1,
        },
        provider_advert::StakePointer,
    };

    use super::*;

    fn fixture_manifest_root_cid() -> ManifestRootCid {
        let manifest: sorafs_manifest::ManifestV1 = norito::decode_from_bytes(include_bytes!(
            "../../../../fixtures/sorafs_gateway/1.0.0/manifest_v1.to"
        ))
        .expect("decode canonical SoraFS fixture manifest");
        ManifestRootCid::try_from_slice(&manifest.root_cid)
            .expect("fixture manifest root CID must be canonical")
    }

    fn sample_declaration() -> (ProviderId, CapacityDeclarationRecord, CapacityDeclarationV1) {
        let provider_id = ProviderId::new([0x11; 32]);
        let declaration = CapacityDeclarationV1 {
            version: sorafs_manifest::capacity::CAPACITY_DECLARATION_VERSION_V1,
            provider_id: provider_id.as_bytes().to_owned(),
            stake: StakePointer {
                pool_id: [0xAA; 32],
                stake_amount: "42".parse().expect("canonical stake quantity"),
            },
            committed_capacity_gib: 1_024,
            chunker_commitments: vec![ChunkerCommitmentV1 {
                profile_id: "sorafs.sf1@1.0.0".to_owned(),
                profile_aliases: Some(vec!["sorafs-sf1".to_owned()]),
                committed_gib: 1_024,
                capability_refs: vec![],
            }],
            lane_commitments: vec![LaneCommitmentV1 {
                lane_id: "global".to_owned(),
                max_gib: 1_024,
            }],
            pricing: Some(PricingScheduleV1 {
                currency: "xor".to_owned(),
                rate_per_gib_hour_milliu: 1_000,
                min_commitment_hours: Some(24),
                notes: Some("standard tier".to_owned()),
            }),
            valid_from: 1_700_000_000,
            valid_until: 1_700_086_400,
            metadata: Vec::new(),
        };
        let encoded = norito::to_bytes(&declaration).expect("encode declaration");
        let record = CapacityDeclarationRecord::new(
            provider_id,
            encoded,
            declaration.committed_capacity_gib,
            42,
            48,
            96,
            Metadata::default(),
        );
        (provider_id, record, declaration)
    }

    #[test]
    fn build_declarations_decodes_records() {
        let (provider_id, record, declaration) = sample_declaration();
        let entries = [(provider_id, record)];
        let declarations = build_declarations(entries.iter().map(|(id, rec)| (id, rec)))
            .expect("build declarations");
        assert_eq!(declarations.len(), 1);
        let json_value = declarations[0]
            .declaration_json
            .as_object()
            .expect("json object");
        let norito_b64 = json_value
            .get("norito_b64")
            .and_then(Value::as_str)
            .expect("norito base64");
        let canonical_bytes = BASE64_STD
            .decode(norito_b64.as_bytes())
            .expect("decode base64 payload");
        let decoded: CapacityDeclarationV1 =
            norito::decode_from_bytes(&canonical_bytes).expect("decode declaration bytes");
        assert_eq!(decoded.provider_id, declaration.provider_id);
        assert_eq!(declarations[0].registered_epoch, 42);
    }

    #[test]
    fn build_fee_ledger_projects_entries() {
        let provider_id = ProviderId::new([0x22; 32]);
        let storage_fee: Quantity = "0.0000000001".parse().expect("sub-nano quantity");
        let egress_fee: Quantity = "12000".parse().expect("egress quantity");
        let accrued_fee = storage_fee
            .checked_add(&egress_fee)
            .expect("representable accrued fee");
        let entry = CapacityFeeLedgerEntry {
            provider_id,
            total_declared_gib: 10_240,
            total_utilised_gib: 8_192,
            storage_fee: storage_fee.clone(),
            egress_fee: egress_fee.clone(),
            accrued_fee: accrued_fee.clone(),
            expected_settlement: "84000.25".parse().expect("settlement quantity"),
            penalty_slashed: Quantity::zero(),
            penalty_events: 0,
            last_updated_epoch: 128,
            last_window_start_epoch: 120,
            last_window_end_epoch: 128,
            last_nonce: 7,
        };
        let entries = [(provider_id, entry)];
        let ledger = build_fee_ledger(entries.iter().map(|(id, rec)| (id, rec)));
        assert_eq!(ledger.len(), 1);
        assert_eq!(
            ledger[0].provider_id_hex,
            provider_id.as_bytes().encode_hex::<String>()
        );
        assert_eq!(ledger[0].total_utilised_gib, 8_192);
        assert_eq!(ledger[0].storage_fee, storage_fee);
        assert_eq!(ledger[0].egress_fee, egress_fee);
        assert_eq!(ledger[0].accrued_fee, accrued_fee);
        assert_eq!(ledger[0].expected_settlement.to_string(), "84000.25");

        let json = ledger[0].clone().into_json().expect("serialize fee ledger");
        let object = json.as_object().expect("fee ledger object");
        assert_eq!(
            object.get("storage_fee").and_then(Value::as_str),
            Some("0.0000000001")
        );
        for retired in [
            "storage_fee_nano",
            "egress_fee_nano",
            "accrued_fee_nano",
            "expected_settlement_nano",
            "penalty_slashed_nano",
        ] {
            assert!(
                !object.contains_key(retired),
                "retired fixed-nano field {retired} must not be exposed"
            );
        }
    }

    #[test]
    fn build_credit_ledger_preserves_canonical_quantities() {
        let provider_id = ProviderId::new([0x23; 32]);
        let available_credit: Quantity = "340282366920938463463374607431768211456.0000000001"
            .parse()
            .expect("quantity wider than u128 with a sub-nano fraction");
        let mut record = ProviderCreditRecord::new(
            provider_id,
            available_credit.clone(),
            "12.5".parse().expect("bonded quantity"),
            "10".parse().expect("required bond quantity"),
            "0.0000000000000000000000000001"
                .parse()
                .expect("scale-28 settlement quantity"),
            32,
            64,
            Metadata::default(),
        );
        record.slashed = "0.25".parse().expect("slashed quantity");
        let entries = [(provider_id, record)];

        let ledger = build_credit_ledger(entries.iter().map(|(id, rec)| (id, rec)));
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger[0].available_credit, available_credit);

        let json = ledger[0]
            .clone()
            .into_json()
            .expect("serialize credit ledger");
        let object = json.as_object().expect("credit ledger object");
        assert_eq!(
            object.get("available_credit").and_then(Value::as_str),
            Some("340282366920938463463374607431768211456.0000000001")
        );
        assert_eq!(
            object.get("expected_settlement").and_then(Value::as_str),
            Some("0.0000000000000000000000000001")
        );
        for retired in [
            "available_credit_nano",
            "bonded_nano",
            "required_bond_nano",
            "expected_settlement_nano",
            "slashed_nano",
        ] {
            assert!(
                !object.contains_key(retired),
                "retired fixed-nano field {retired} must not be exposed"
            );
        }
    }

    #[test]
    fn build_disputes_projects_entries() {
        use sorafs_manifest::capacity::{
            CapacityDisputeEvidenceV1, CapacityDisputeKind, CapacityDisputeV1,
        };

        let provider_id = ProviderId::new([0x33; 32]);
        let dispute = CapacityDisputeV1 {
            version: sorafs_manifest::capacity::CAPACITY_DISPUTE_VERSION_V1,
            provider_id: provider_id.as_bytes().to_owned(),
            complainant_id: [0x44; 32],
            replication_order_id: Some([0x55; 32]),
            kind: CapacityDisputeKind::ReplicationShortfall,
            evidence: CapacityDisputeEvidenceV1 {
                evidence_digest: [0xAB; 32],
                media_type: Some("application/zip".into()),
                uri: Some("https://example.com/evidence.zip".into()),
                size_bytes: Some(1024),
            },
            submitted_epoch: 1_700_100_000,
            description: "provider missed ingestion window".into(),
            requested_remedy: Some("slash stake".into()),
        };
        let payload = norito::to_bytes(&dispute).expect("encode dispute");
        let canonical_payload = payload.clone();
        let dispute_id = CapacityDisputeId::new(*blake3::hash(&payload).as_bytes());
        let evidence = CapacityDisputeEvidence {
            digest: dispute.evidence.evidence_digest,
            media_type: dispute.evidence.media_type.clone(),
            uri: dispute.evidence.uri.clone(),
            size_bytes: dispute.evidence.size_bytes,
        };
        let record = CapacityDisputeRecord::new_pending(
            dispute_id,
            provider_id,
            dispute.complainant_id,
            dispute.replication_order_id,
            dispute.kind as u8,
            dispute.submitted_epoch,
            dispute.description.clone(),
            dispute.requested_remedy.clone(),
            evidence,
            payload,
        );

        let entries = [(dispute_id, record)];
        let disputes = build_disputes(entries.iter().map(|(id, rec)| (id, rec)));
        assert_eq!(disputes.len(), 1);
        let json = disputes[0].clone().into_json().expect("serialize dispute");
        let object = json.as_object().expect("json object");
        assert_eq!(
            object.get("kind").and_then(Value::as_str),
            Some("replication_shortfall")
        );
        assert_eq!(
            object.get("status").and_then(Value::as_str),
            Some("pending")
        );
        assert_eq!(
            object
                .get("dispute_id_hex")
                .and_then(Value::as_str)
                .map(str::len),
            Some(64)
        );
        let dispute_b64 = object
            .get("dispute_b64")
            .and_then(Value::as_str)
            .expect("dispute base64 field");
        let decoded = BASE64_STD
            .decode(dispute_b64.as_bytes())
            .expect("decode dispute payload");
        assert_eq!(decoded, canonical_payload);
    }

    #[test]
    fn registry_manifest_to_json_includes_alias() {
        let digest = ManifestDigest::new([0xAA; 32]);
        let chunker = ChunkerProfileHandle {
            profile_id: 1,
            namespace: "sorafs".to_owned(),
            name: "sf1".to_owned(),
            semver: "1.0.0".to_owned(),
            multihash_code: 0x1F,
        };
        let policy = PinPolicy {
            min_replicas: 2,
            storage_class: StorageClass::Hot,
            retention_epoch: 512,
        };
        let alias = ManifestAliasBinding {
            name: "docs".to_owned(),
            namespace: "sora".to_owned(),
            proof: vec![0xAB, 0xCD, 0xEF],
        };

        let public_key: PublicKey =
            "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C"
                .parse()
                .expect("public key");
        let submitter = AccountId::new(public_key);
        let mut record = PinManifestRecord::new(
            digest,
            fixture_manifest_root_cid(),
            chunker,
            [0xBB; 32],
            [0; 32],
            0,
            policy,
            submitter,
            42,
            Some(alias.clone()),
            None,
            Metadata::default(),
        );
        record.approve(64, Some([0x10; 32]));

        let manifest = RegistryManifest::from_store(&digest, &record).expect("registry manifest");
        let value = manifest.to_json().expect("serialize manifest");
        let object = value.as_object().expect("manifest json object");
        let expected_digest = hex::encode(digest.as_bytes());
        assert_eq!(
            object.get("digest_hex").and_then(Value::as_str),
            Some(expected_digest.as_str())
        );
        let status = object
            .get("status")
            .and_then(Value::as_object)
            .expect("status json");
        assert_eq!(
            status.get("state").and_then(Value::as_str),
            Some("approved")
        );
        let alias_json = object
            .get("alias")
            .and_then(Value::as_object)
            .expect("alias json");
        assert_eq!(alias_json.get("name").and_then(Value::as_str), Some("docs"));
        let expected_proof = BASE64_STD.encode(alias.proof);
        assert_eq!(
            alias_json.get("proof_b64").and_then(Value::as_str),
            Some(expected_proof.as_str())
        );
    }

    #[test]
    fn registry_replication_order_serializes_providers() {
        let manifest_root_cid = fixture_manifest_root_cid();
        let order_payload = ReplicationOrderV1 {
            version: REPLICATION_ORDER_VERSION_V1,
            order_id: [0x11; 32],
            manifest_cid: manifest_root_cid.as_bytes().to_vec(),
            manifest_digest: [0x55; 32],
            chunking_profile: "sorafs.sf1@1.0.0".to_owned(),
            target_replicas: 1,
            assignments: vec![
                ReplicationAssignmentV1 {
                    provider_id: [0x22; 32],
                    slice_gib: 1,
                    lane: None,
                },
                ReplicationAssignmentV1 {
                    provider_id: [0x33; 32],
                    slice_gib: 1,
                    lane: None,
                },
            ],
            issued_at: 1_699_999_000,
            deadline_at: 1_700_000_000,
            sla: ReplicationOrderSlaV1 {
                ingest_deadline_secs: 1_000,
                min_availability_percent_milli: 99_000,
                min_por_success_percent_milli: 98_000,
            },
            metadata: Vec::new(),
        };
        let canonical = norito::to_bytes(&order_payload).expect("encode order");
        let public_key: PublicKey =
            "ed0120BDF918243253B1E731FA096194C8928DA37C4D3226F97EEBD18CF5523D758D6C"
                .parse()
                .expect("public key");
        let issuer = AccountId::new(public_key);
        let record = ReplicationOrderRecord {
            order_id: ReplicationOrderId::new(order_payload.order_id),
            manifest_digest: ManifestDigest::new([0x55; 32]),
            manifest_root_cid,
            issued_by: issuer.clone(),
            issued_epoch: 10,
            deadline_epoch: 20,
            canonical_order: canonical.clone(),
            provider_completions: vec![ReplicationOrderCompletionRecord {
                provider_id: ProviderId::new(order_payload.assignments[0].provider_id),
                completed_by: issuer,
                completion_epoch: 15,
            }],
            status: ReplicationOrderStatus::Completed(15),
        };

        let order = RegistryReplicationOrder::from_store(&record.order_id, &record)
            .expect("registry order");
        let value = order.to_json().expect("serialize order");
        let object = value.as_object().expect("order json object");
        let providers = object
            .get("providers")
            .and_then(Value::as_array)
            .expect("providers array");
        assert_eq!(providers.len(), 2);
        let expected0 = hex::encode(order_payload.assignments[0].provider_id);
        let expected1 = hex::encode(order_payload.assignments[1].provider_id);
        assert_eq!(providers[0].as_str(), Some(expected0.as_str()));
        assert_eq!(providers[1].as_str(), Some(expected1.as_str()));
        let completions = object
            .get("provider_completions")
            .and_then(Value::as_array)
            .expect("provider completions array");
        assert_eq!(completions.len(), 1);
        assert_eq!(
            completions[0]
                .as_object()
                .and_then(|completion| completion.get("provider_hex"))
                .and_then(Value::as_str),
            Some(expected0.as_str())
        );
        assert_eq!(
            object
                .get("canonical_order_b64")
                .and_then(Value::as_str)
                .map(|s| BASE64_STD.decode(s.as_bytes()).expect("decode base64")),
            Some(canonical)
        );
    }
}
