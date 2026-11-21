//! Choose a protected local finality checkpoint for one Load without changing its public proof.
use super::*;
use iroha_data_model::sumeragi_finality::SumeragiCommitCheckpointV1;

impl NativeEngine {
    /// Select a prior verified checkpoint recovered from the installed wallet's protected marker.
    /// This boundary takes no checkpoint data from a network response or operation witness.
    pub fn with_protected_epoch(
        &self,
        params: &AuthenticatedParameters,
        checkpoint: &SumeragiCommitCheckpointV1,
    ) -> Result<Self> {
        let selected = params
            .inner
            .finality
            .restore_trusted_local_epoch(checkpoint)
            .map_err(|error| Error::Rejected(error.to_string()))?;
        Ok(Self {
            selected_epoch: Some(selected),
            selection_owner: Arc::new(()),
            ..self.clone()
        })
    }
}

impl ExactOperationWitness {
    /// Unverified lookup hint used only to select a locally authorized epoch. Prepare verifies the certificate.
    pub fn load_epoch_selector(&self) -> Result<Option<u64>> {
        let Self::Load {
            finality_original, ..
        } = self
        else {
            return Ok(None);
        };
        let original = KagemushaWalletLoadFinalityV1::decode_canonical(finality_original)
            .map_err(|error| Error::Rejected(error.to_string()))?;
        original
            .certificate
            .epoch_id()
            .map(Some)
            .map_err(|error| Error::Rejected(error.to_string()))
    }
}

impl RecoveryCapsule {
    /// Read the Load lookup hint from the same bounded original that Resume subsequently verifies.
    pub fn load_epoch_selector(&self) -> Result<Option<u64>> {
        if self.kind() != 2 {
            return Ok(None);
        }
        let r: RecoveryFrame = crate::native::decode(&self.frame.recovery)?;
        if r.version != 1
            || r.profile.len() != 64
            || crate::native::encode(&r)? != self.frame.recovery
        {
            return Err(Error::Malformed("canonical recovery payload"));
        }
        let epoch =
            crate::native::witness_codec::decode_witness(&r.witness)?.load_epoch_selector()?;
        epoch
            .map(Some)
            .ok_or(Error::Malformed("retained Load original"))
    }
}
