//! Bounded public reads of committee preparation and exact finality attachments.

use super::*;
use iroha_data_model::{
    nexus::ValidatorCommitteeStatusV1, sumeragi::finality::NativeFinalityLimits,
};

const COMMITTEE_STATUS_RESPONSE_MAX_BYTES: usize = 16 * 1024 * 1024;

impl Client {
    /// Fetch one committee attempt as canonical Norito under the normal request deadline.
    ///
    /// Omit `target_epoch` to inspect the next scheduling epoch. These are progress
    /// observations; the caller must authenticate both finality attachments from an
    /// independently configured native chain and signed genesis before provisioning or signing.
    /// An omitted target is a server-selected, untrusted observation; this transport reader
    /// does not infer epoch authority from decoded result bytes or authenticate a QC.
    ///
    /// # Errors
    /// Rejects transport, HTTP status, media type, body bound, codec, network and target mismatches.
    pub fn get_validator_committee_status(
        &self,
        target_epoch: Option<u64>,
    ) -> Result<ValidatorCommitteeStatusV1> {
        let mut path = iroha_torii_shared::uri::NEXUS_VALIDATOR_COMMITTEE.to_owned();
        if let Some(epoch) = target_epoch {
            path.push_str(&format!("?target_epoch={epoch}"));
        }
        self.ensure_activation_evidence_deadline()?;
        let response = self.send_builder(self.canonical_norito_get_request(
            &path,
            COMMITTEE_STATUS_RESPONSE_MAX_BYTES,
            ActivationEvidenceReadAuth::Public,
        )?)?;
        let status: ValidatorCommitteeStatusV1 = Self::decode_canonical_norito_response(
            &response,
            COMMITTEE_STATUS_RESPONSE_MAX_BYTES,
            "Failed to get validator committee status",
        )?;
        let source = status
            .latest_finality
            .decode_block(NativeFinalityLimits {
                block_bytes: COMMITTEE_STATUS_RESPONSE_MAX_BYTES,
                journal_bytes: COMMITTEE_STATUS_RESPONSE_MAX_BYTES,
                block_count: 256,
                allocated_bytes: 64 * 1024 * 1024,
            })
            .map_err(|error| eyre!("noncanonical native committee source: {error}"))?;
        if status.network_id != self.network_id
            || target_epoch.is_some_and(|epoch| status.target_epoch != epoch)
            || status.target_epoch == 0
            || source.header().height().get() < 2
            || source
                .external_transactions()
                .any(|transaction| transaction.network_id() != Some(&self.network_id))
        {
            return Err(eyre!(
                "committee observation differs from requested network, target or native source"
            ));
        }
        self.ensure_activation_evidence_deadline()?;
        Ok(status)
    }
}
