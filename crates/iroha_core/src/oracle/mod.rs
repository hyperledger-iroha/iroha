//! Oracle host helpers.
//!
//! This module wires the data-model oracle helpers into host-side validation so
//! connectors and on-chain aggregation can reuse a single deterministic path.

use iroha_crypto::Hash;
#[cfg(test)]
use iroha_crypto::SignatureOf;
use iroha_data_model::oracle::{
    AggregationOutput, ConnectorRequest, FeedConfig, FeedConfigVersion, FeedSlot, Observation,
    OracleAggregationError, OracleId, ReplayKey, aggregate_observations,
};
#[cfg(feature = "json")]
use mv::json::JsonKeyCodec;
use norito::codec::{Decode, Encode};
#[cfg(feature = "json")]
use norito::json;

/// Deterministic admission pipeline for oracle observations.
#[derive(Debug)]
pub struct ObservationAdmission<'a> {
    config: &'a FeedConfig,
    slot: FeedSlot,
    request_hash: Hash,
}

impl<'a> ObservationAdmission<'a> {
    /// Create a new admission pipeline for a specific slot/request.
    #[must_use]
    pub fn new(config: &'a FeedConfig, slot: FeedSlot, request_hash: Hash) -> Self {
        Self {
            config,
            slot,
            request_hash,
        }
    }

    /// Validate an observation for cadence/feed/config/connector pins.
    ///
    /// # Errors
    ///
    /// Returns [`OracleAggregationError`] when observation metadata does not match the
    /// expected feed, slot, or request hash.
    pub fn admit(&mut self, observation: &Observation) -> Result<(), OracleAggregationError> {
        self.config
            .validate_observation_meta(&observation.body)
            .map_err(OracleAggregationError::from)?;

        if observation.body.slot != self.slot {
            return Err(OracleAggregationError::SlotMismatch {
                expected: self.slot,
                provided: observation.body.slot,
            });
        }
        if observation.body.request_hash != self.request_hash {
            return Err(OracleAggregationError::RequestHashMismatch {
                expected: self.request_hash,
                provided: observation.body.request_hash,
            });
        }

        Ok(())
    }
}

/// Aggregate admitted observations into a report/outcome.
///
/// # Errors
///
/// Returns [`OracleAggregationError`] when observations violate feed rules or
/// aggregation fails.
pub fn aggregate(
    config: &FeedConfig,
    slot: FeedSlot,
    request_hash: Hash,
    submitter: iroha_data_model::oracle::OracleId,
    observations: &[Observation],
) -> Result<AggregationOutput, OracleAggregationError> {
    aggregate_observations(config, slot, request_hash, submitter, observations)
}

/// Validate a connector request before hashing/sending.
///
/// # Errors
///
/// Returns [`OracleAggregationError`] when the request feed id/version or redaction
/// policy is invalid.
pub fn validate_connector_request(
    config: &FeedConfig,
    request: &ConnectorRequest,
) -> Result<(), OracleAggregationError> {
    if request.feed_id != config.feed_id
        || request.feed_config_version != config.feed_config_version
    {
        return Err(OracleAggregationError::Model(
            iroha_data_model::oracle::OracleModelError::FeedVersionMismatch {
                expected: config.feed_config_version,
                provided: request.feed_config_version,
            },
        ));
    }
    request
        .validate_redaction()
        .map_err(OracleAggregationError::from)
}

/// In-memory aggregator that validates observations and produces a report/outcome.
#[derive(Debug)]
pub struct OracleAggregator<'a> {
    admission: ObservationAdmission<'a>,
    buffer: Vec<Observation>,
    seen_providers: std::collections::BTreeSet<iroha_data_model::oracle::OracleId>,
}

impl<'a> OracleAggregator<'a> {
    /// Create a new aggregator for a feed slot/request.
    #[must_use]
    pub fn new(config: &'a FeedConfig, slot: FeedSlot, request_hash: Hash) -> Self {
        Self {
            admission: ObservationAdmission::new(config, slot, request_hash),
            buffer: Vec::new(),
            seen_providers: std::collections::BTreeSet::new(),
        }
    }

    /// Validate and buffer an observation.
    ///
    /// # Errors
    ///
    /// Returns [`OracleAggregationError`] when the observation metadata is invalid,
    /// duplicated, or exceeds the configured observer cap.
    pub fn push(&mut self, observation: Observation) -> Result<(), OracleAggregationError> {
        self.admission.admit(&observation)?;
        if !self
            .seen_providers
            .insert(observation.body.provider_id.clone())
        {
            return Err(OracleAggregationError::DuplicateObservation {
                oracle_id: observation.body.provider_id.clone(),
            });
        }
        if self.buffer.len() >= usize::from(self.admission.config.max_observers) {
            return Err(OracleAggregationError::TooManyObservations {
                provided: self.buffer.len() + 1,
                max: self.admission.config.max_observers,
            });
        }
        self.buffer.push(observation);
        Ok(())
    }

    /// Finalize aggregation with a given submitter.
    ///
    /// # Errors
    ///
    /// Propagates [`OracleAggregationError`] from [`aggregate`] if aggregation fails.
    pub fn finalize(
        &self,
        submitter: iroha_data_model::oracle::OracleId,
    ) -> Result<AggregationOutput, OracleAggregationError> {
        aggregate(
            self.admission.config,
            self.admission.slot,
            self.admission.request_hash,
            submitter,
            &self.buffer,
        )
    }
}

/// Key identifying a buffered observation window for `(feed, version, slot, request)`.
#[derive(
    Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Encode, Decode, iroha_schema::IntoSchema,
)]
#[cfg_attr(
    feature = "json",
    derive(
        iroha_data_model::DeriveJsonSerialize,
        iroha_data_model::DeriveJsonDeserialize
    )
)]
pub struct ObservationWindowKey {
    /// Feed identifier.
    pub feed_id: iroha_data_model::oracle::FeedId,
    /// Feed configuration version.
    pub feed_config_version: FeedConfigVersion,
    /// Slot index for the buffered observations.
    pub slot: FeedSlot,
    /// Canonical request hash shared across the committee.
    pub request_hash: Hash,
}

impl ObservationWindowKey {
    /// Construct a new window key.
    #[must_use]
    pub fn new(
        feed_id: iroha_data_model::oracle::FeedId,
        feed_config_version: FeedConfigVersion,
        slot: FeedSlot,
        request_hash: Hash,
    ) -> Self {
        Self {
            feed_id,
            feed_config_version,
            slot,
            request_hash,
        }
    }

    /// Derive the replay key used for replay protection.
    #[must_use]
    pub fn replay_key(&self) -> ReplayKey {
        ReplayKey::new(
            self.feed_id.clone(),
            self.feed_config_version,
            self.slot,
            self.request_hash,
        )
    }
}

#[cfg(feature = "json")]
impl JsonKeyCodec for ObservationWindowKey {
    fn encode_json_key(&self, out: &mut String) {
        let mut buf = String::new();
        json::JsonSerialize::json_serialize(self, &mut buf);
        json::write_json_string(&buf, out);
    }

    fn decode_json_key(encoded: &str) -> Result<Self, json::Error> {
        let mut parser = json::Parser::new(encoded);
        json::JsonDeserialize::json_deserialize(&mut parser)
    }
}

/// Buffered observations for a single `(feed, slot, request_hash)` window.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, iroha_schema::IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(
        iroha_data_model::DeriveJsonSerialize,
        iroha_data_model::DeriveJsonDeserialize
    )
)]
pub struct ObservationWindow {
    /// Configuration version pinned for this window.
    pub feed_config_version: FeedConfigVersion,
    /// Canonical request hash for the window.
    pub request_hash: Hash,
    /// Slot index for this window.
    pub slot: FeedSlot,
    /// Buffered observations keyed by provider id.
    pub observations: std::collections::BTreeMap<OracleId, Observation>,
}

impl ObservationWindow {
    /// Create an empty window for the given key.
    #[must_use]
    pub fn new(key: &ObservationWindowKey) -> Self {
        Self {
            feed_config_version: key.feed_config_version,
            request_hash: key.request_hash,
            slot: key.slot,
            observations: std::collections::BTreeMap::new(),
        }
    }

    /// Insert an observation after validating feed metadata and caps.
    ///
    /// # Errors
    ///
    /// Returns [`OracleAggregationError`] when the observation metadata does not match the
    /// window or exceeds configured caps.
    pub fn push(
        &mut self,
        config: &FeedConfig,
        observation: Observation,
    ) -> Result<(), OracleAggregationError> {
        config
            .validate_observation_meta(&observation.body)
            .map_err(OracleAggregationError::from)?;
        if observation.body.slot != self.slot {
            return Err(OracleAggregationError::SlotMismatch {
                expected: self.slot,
                provided: observation.body.slot,
            });
        }
        if observation.body.request_hash != self.request_hash {
            return Err(OracleAggregationError::RequestHashMismatch {
                expected: self.request_hash,
                provided: observation.body.request_hash,
            });
        }
        if observation.body.feed_config_version != self.feed_config_version {
            return Err(OracleAggregationError::Model(
                iroha_data_model::oracle::OracleModelError::FeedVersionMismatch {
                    expected: self.feed_config_version,
                    provided: observation.body.feed_config_version,
                },
            ));
        }
        let provider_id = observation.body.provider_id.clone();
        if self.observations.len() >= usize::from(config.max_observers) {
            return Err(OracleAggregationError::TooManyObservations {
                provided: self.observations.len() + 1,
                max: config.max_observers,
            });
        }
        if self
            .observations
            .insert(provider_id.clone(), observation)
            .is_some()
        {
            return Err(OracleAggregationError::DuplicateObservation {
                oracle_id: provider_id,
            });
        }

        Ok(())
    }

    /// Consume the window and return observations in deterministic order.
    #[must_use]
    pub fn into_sorted(self) -> Vec<Observation> {
        let mut observations: Vec<_> = self.observations.into_values().collect();
        observations.sort_by(|left, right| left.body.provider_id.cmp(&right.body.provider_id));
        observations
    }

    /// Number of buffered observations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.observations.len()
    }

    /// Returns `true` when there are no buffered observations.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.observations.is_empty()
    }
}

/// Alias for oracle feed event records.
pub type FeedEventRecord = iroha_data_model::events::data::oracle::FeedEventRecord;

#[cfg(test)]
mod tests {
    use std::{num::NonZeroU64, str::FromStr};

    use iroha_crypto::{Algorithm, KeyPair, Signature};
    use iroha_data_model::{
        account::AccountId,
        name::Name,
        oracle::{
            AggregationRule, FeedConfigVersion, ObservationBody, ObservationOutcome,
            ObservationValue, OutlierPolicy, RiskClass,
        },
    };

    use super::*;

    fn feed_id(name: &str) -> iroha_data_model::oracle::FeedId {
        iroha_data_model::oracle::FeedId(Name::from_str(name).expect("feed name"))
    }

    fn oracle(name: &str, domain: &str) -> iroha_data_model::oracle::OracleId {
        let seed = format!("{name}:{domain}");
        let keypair = KeyPair::from_seed(seed.into_bytes(), Algorithm::Ed25519);
        AccountId::new(keypair.public_key().clone())
    }

    fn sample_providers() -> Vec<iroha_data_model::oracle::OracleId> {
        vec![oracle("alice", "validators"), oracle("bob", "validators")]
    }

    fn sample_config() -> FeedConfig {
        FeedConfig {
            feed_id: feed_id("price_xor_usd"),
            feed_config_version: FeedConfigVersion(1),
            providers: sample_providers(),
            connector_id: "http-bin".to_string(),
            connector_version: 1,
            cadence_slots: NonZeroU64::new(5).unwrap(),
            aggregation: AggregationRule::MedianMad(300),
            outlier_policy: OutlierPolicy::Mad(350),
            min_signers: 2,
            committee_size: 2,
            risk_class: RiskClass::Medium,
            max_observers: 4,
            max_value_len: 128,
            max_error_rate_bps: 250,
            dispute_window_slots: NonZeroU64::new(10).unwrap(),
            replay_window_slots: NonZeroU64::new(4).unwrap(),
        }
    }

    fn sample_request_hash() -> Hash {
        Hash::new(b"price_xor_usd:slot10:request")
    }

    fn observation(provider: iroha_data_model::oracle::OracleId, value: i128) -> Observation {
        let body = ObservationBody {
            feed_id: feed_id("price_xor_usd"),
            feed_config_version: FeedConfigVersion(1),
            slot: 10,
            provider_id: provider,
            connector_id: "http-bin".to_string(),
            connector_version: 1,
            request_hash: sample_request_hash(),
            outcome: ObservationOutcome::Value(ObservationValue::new(value, 5)),
            timestamp_ms: Some(1_700_000_000_123),
        };
        let signature = Signature::from_hex("00".repeat(128)).expect("signature hex");
        Observation {
            body,
            signature: SignatureOf::from_signature(signature),
        }
    }

    #[test]
    fn aggregator_builds_report() {
        let config = sample_config();
        let mut agg = OracleAggregator::new(&config, 10, sample_request_hash());
        let providers = sample_providers();
        agg.push(observation(providers[0].clone(), 1_000_000))
            .expect("push first");
        agg.push(observation(providers[1].clone(), 1_002_000))
            .expect("push second");

        let output = agg
            .finalize(providers[0].clone())
            .expect("aggregation succeeds");
        assert!(matches!(
            output.outcome,
            iroha_data_model::oracle::FeedEventOutcome::Success(_)
        ));
        assert_eq!(config.feed_id, output.report.feed_id);
    }

    #[test]
    fn aggregator_rejects_replays() {
        let config = sample_config();
        let mut agg = OracleAggregator::new(&config, 10, sample_request_hash());
        let provider = sample_providers()[0].clone();
        let obs = observation(provider.clone(), 1_000_000);
        agg.push(obs.clone()).expect("first observation");
        let err = agg.push(obs).expect_err("duplicate observation rejected");
        assert!(matches!(
            err,
            OracleAggregationError::DuplicateObservation { .. }
        ));
    }

    #[test]
    fn aggregator_enforces_max_observers() {
        let mut config = sample_config();
        config.max_observers = 1;
        let mut agg = OracleAggregator::new(&config, 10, sample_request_hash());
        agg.push(observation(sample_providers()[0].clone(), 1_000_000))
            .expect("first ok");
        let err = agg
            .push(observation(sample_providers()[1].clone(), 1_000_100))
            .expect_err("cap enforced");
        assert!(matches!(
            err,
            OracleAggregationError::TooManyObservations { .. }
        ));
    }

    #[test]
    fn aggregator_rejects_slot_mismatch() {
        let config = sample_config();
        let mut agg = OracleAggregator::new(&config, 10, sample_request_hash());
        let mut obs = observation(sample_providers()[0].clone(), 1_000_000);
        obs.body.slot = 11;
        let err = agg.push(obs).expect_err("slot mismatch");
        assert!(matches!(
            err,
            OracleAggregationError::Model(
                iroha_data_model::oracle::OracleModelError::InactiveSlot { .. }
            )
        ));
    }
}
