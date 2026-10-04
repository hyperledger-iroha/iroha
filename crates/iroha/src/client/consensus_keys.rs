//! Bounded operator observations of registered consensus-key lifecycles.

use super::*;
use iroha_data_model::consensus::ConsensusKeyRecord;

// The operator route returns the newest 128 records. This observation is not a
// complete registry or a finalized state proof; missing older records are ambiguous.
const CONSENSUS_KEY_SNAPSHOT_MAX_RECORDS: usize = 128;
const CONSENSUS_KEY_SNAPSHOT_MAX_BYTES: usize = 1024 * 1024;

impl Client {
    /// Read the bounded consensus-key snapshot using an operator-signed request.
    ///
    /// The route returns at most the newest 128 records, without a state proof or
    /// height binding. Absence does not establish that a key is unregistered.
    ///
    /// # Errors
    /// Rejects missing operator credentials, transport/deadline failures, noncanonical
    /// Norito, excessive responses, and duplicate record identities.
    pub fn get_sumeragi_consensus_keys(&self) -> Result<Vec<ConsensusKeyRecord>> {
        self.ensure_activation_evidence_deadline()?;
        let url = join_torii_url(
            &self.torii_url,
            iroha_torii_shared::route_catalog::sumeragi::CONSENSUS_KEYS.path(),
        );
        let response = self.send_builder(
            self.operator_signed_request(HttpMethod::GET, url, Vec::new())?
                .header("Accept", APPLICATION_NORITO)
                .max_response_bytes(CONSENSUS_KEY_SNAPSHOT_MAX_BYTES),
        )?;
        let records: Vec<ConsensusKeyRecord> = Self::decode_canonical_norito_response(
            &response,
            CONSENSUS_KEY_SNAPSHOT_MAX_BYTES,
            "sumeragi.consensus_keys.read",
        )?;
        if records.len() > CONSENSUS_KEY_SNAPSHOT_MAX_RECORDS {
            return Err(eyre!("consensus-key snapshot exceeds its record limit"));
        }
        let mut identities = std::collections::BTreeSet::new();
        if records.iter().any(|record| !identities.insert(&record.id)) {
            return Err(eyre!("consensus-key snapshot repeats a record identity"));
        }
        self.ensure_activation_evidence_deadline()?;
        Ok(records)
    }
}
