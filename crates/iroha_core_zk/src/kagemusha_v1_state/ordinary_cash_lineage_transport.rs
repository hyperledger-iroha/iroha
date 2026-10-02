//! Main-owned global lineage transport. Public bytes never constitute a State or money grant.
use super::*;
use iroha_data_model::kagemusha::KagemushaOrdinaryLineageRequestOperationV1;

impl KagemushaNativeOrdinaryCashOwnerV1 {
    /// Sign the exact pending CAS request through the actual Native account/session caller.
    /// The one-call signing fence is durable before that caller receives its closed borrow.
    /// A retained signature is reused exactly; an uncertain invocation is never repeated.
    /// # Errors
    /// Refuses absent request, foreign/expired custody, unknown invocation or failed durability.
    pub fn sign_retained_lineage_request(
        &mut self,
        sign: impl FnOnce(
            &KagemushaAuthenticatedOrdinaryLineageAccountSigningV1<'_>,
        ) -> Result<[u8; 64], KagemushaStateErrorV1>,
    ) -> Result<Vec<Vec<u8>>, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let financial = self.publication.cash_financial();
        let current = self.control.loan(financial).map_err(material)?;
        let request_original = self
            .lineage_cas
            .pending_request_original(financial, &current)
            .map_err(material)?
            .canonical_bytes()
            .map_err(material)?;
        if self
            .lineage_cas
            .retained_account_signature(financial, &current)
            .map_err(material)?
            .is_none()
        {
            self.lineage_cas
                .fence_account_signing(financial, &current)
                .map_err(material)?;
            let signature = {
                let original = self
                    .lineage_cas
                    .account_signing_original(financial, current)
                    .map_err(material)?;
                let signature = sign(&original)?;
                original.recheck().map_err(material)?;
                signature
            };
            let current = self.control.loan(financial).map_err(material)?;
            self.lineage_cas
                .capture_account_signature(financial, &current, signature)
                .map_err(material)?;
        }
        let current = self.control.loan(financial).map_err(material)?;
        let (actual, signature) = self
            .lineage_cas
            .signed_request_originals(financial, &current)
            .map_err(material)?;
        if actual != request_original {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        self.require_current_financial_control()?;
        Ok(vec![actual, signature.to_vec()])
    }

    /// Authenticate exact Core signature, DATA row and complete certified World into this CAS WAL.
    /// Its distinct post-fsync clock acknowledgment is mandatory. No State or outbox advances here.
    /// # Errors
    /// Refuses substituted originals, unavailable current FI or any uncertain append/acknowledgment.
    pub fn accept_retained_lineage_result(
        &mut self,
        signed_original: &[u8],
        data_record_original: &[u8],
        authority_original: &[u8],
    ) -> Result<DigestV1, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        let financial = self.publication.cash_financial();
        let current = self.control.loan(financial).map_err(material)?;
        let operation = self
            .lineage_cas
            .pending_request_original(financial, &current)
            .map_err(material)?
            .operation
            .clone();
        let request_sha256 = self
            .lineage_cas
            .accept_result(
                financial,
                &current,
                signed_original,
                data_record_original,
                authority_original,
            )
            .map_err(material)?;
        if let KagemushaOrdinaryLineageRequestOperationV1::Anchor(anchor) = operation {
            if anchor.as_ref() != &self.initial_lineage_anchor {
                self.recovery_failed = true;
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
            if self.acknowledge_initial_lineage_anchor()? != Some(request_sha256) {
                self.recovery_failed = true;
                return Err(KagemushaStateErrorV1::SnapshotIntegrity);
            }
        }
        self.require_current_financial_control()?;
        Ok(request_sha256)
    }

    /// Complete Main's acknowledgment using only its genuine already-acknowledged CAS anchor.
    /// This also recovers the narrow crash gap after CAS acknowledgment and before Main fsync.
    /// # Errors
    /// Refuses changed custody, conflicting anchors, stale current FI or uncertain Main persistence.
    pub fn acknowledge_initial_lineage_anchor(
        &mut self,
    ) -> Result<Option<DigestV1>, KagemushaStateErrorV1> {
        self.require_current_financial_control()?;
        if let Some(key) = self.anchor_request_sha256 {
            self.require_initial_lineage_anchor_current()?;
            return Ok(Some(key));
        }
        let financial = self.publication.cash_financial();
        let Some(key) = self
            .lineage_cas
            .acknowledged_anchor_request(financial, &self.initial_lineage_anchor)
            .map_err(material)?
        else {
            return Ok(None);
        };
        let current = self.control.loan(financial).map_err(material)?;
        self.lineage_cas
            .anchor_receipt(key, financial, &self.initial_lineage_anchor)
            .map_err(material)?
            .recheck_for_effect(financial, &current)
            .map_err(material)?;
        self.persist(&Record::LineageAnchorAcknowledged {
            request_original_sha256: key,
        })?;
        self.anchor_request_sha256 = Some(key);
        self.require_initial_lineage_anchor_current()?;
        Ok(Some(key))
    }

    pub(super) fn recheck_initial_lineage_anchor_historical(
        &self,
    ) -> Result<(), KagemushaStateErrorV1> {
        if let Some(key) = self.anchor_request_sha256 {
            let financial = self.publication.cash_financial();
            self.lineage_cas
                .anchor_receipt(key, financial, &self.initial_lineage_anchor)
                .map_err(material)?
                .recheck_historical(financial)
                .map_err(material)?;
        }
        Ok(())
    }
    pub(super) fn require_initial_lineage_anchor_current(
        &self,
    ) -> Result<(), KagemushaStateErrorV1> {
        let key = self
            .anchor_request_sha256
            .ok_or(KagemushaStateErrorV1::InvalidCandidateStage)?;
        let financial = self.publication.cash_financial();
        let current = self.control.loan(financial).map_err(material)?;
        self.lineage_cas
            .anchor_receipt(key, financial, &self.initial_lineage_anchor)
            .map_err(material)?
            .recheck_for_effect(financial, &current)
            .map_err(material)
    }
    pub(super) fn replay_lineage_anchor_acknowledgment(
        &mut self,
        key: DigestV1,
    ) -> Result<(), KagemushaStateErrorV1> {
        if self.anchor_request_sha256.is_some() || self.pending.is_some() || key == [0; 32] {
            return Err(KagemushaStateErrorV1::SnapshotIntegrity);
        }
        let financial = self.publication.cash_financial();
        self.lineage_cas
            .anchor_receipt(key, financial, &self.initial_lineage_anchor)
            .map_err(material)?
            .recheck_historical(financial)
            .map_err(material)?;
        self.anchor_request_sha256 = Some(key);
        Ok(())
    }
}
