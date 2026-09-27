//! Bounded public reads of committee preparation and exact finality attachments.

use super::*;
use iroha_data_model::nexus::ValidatorCommitteeStatusV1;

const COMMITTEE_STATUS_RESPONSE_MAX_BYTES: usize = 16 * 1024 * 1024;

impl Client {
    /// Fetch one committee attempt as canonical Norito under the normal request deadline.
    ///
    /// Omit `target_epoch` to inspect the next scheduling epoch. These are progress
    /// observations; the caller must authenticate both finality attachments from an
    /// independently trusted context before using them to provision or sign.
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
        let current = status
            .latest_finality
            .height_context
            .next_epoch_snapshot
            .as_ref()
            .map_or(
                &status
                    .latest_finality
                    .height_context
                    .kagemusha_mint_finality_authorization,
                |snapshot| &snapshot.kagemusha_mint_finality_authorization,
            );
        let expected_target = match target_epoch {
            Some(epoch) => epoch,
            None => current
                .epoch
                .checked_add(1)
                .ok_or_else(|| eyre!("committee target epoch overflow"))?,
        };
        if status.network_id != self.network_id
            || status.target_epoch != expected_target
            || status.latest_finality.height_context.network_id != self.network_id
        {
            return Err(eyre!(
                "committee status differs from the requested network or target"
            ));
        }
        self.ensure_activation_evidence_deadline()?;
        Ok(status)
    }
}
