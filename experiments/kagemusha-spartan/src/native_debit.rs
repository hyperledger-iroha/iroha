//! Proof-engine verification of the protected custody owner's local debit signature.
use super::*;
use iroha_data_model::kagemusha::{KagemushaDevicePublicKeyV1, KagemushaDeviceSignatureV1};
use sha2::{Digest, Sha256};

/// Hardware-backed evidence originating in an exclusive Native custody owner.
#[derive(Clone)]
pub struct DebitApprovalEvidence {
    pub slot: [u8; 32],
    pub key: KagemushaDevicePublicKeyV1,
    pub nonce: [u8; 32],
    pub signature: KagemushaDeviceSignatureV1,
}

impl DebitApprovalEvidence {
    /// Both the platform's original challenge and the verifying proof engine use this framing.
    #[allow(clippy::too_many_arguments)]
    pub fn message(
        manifest: &[u8; 32],
        slot: &[u8; 32],
        wallet: &[u8; 32],
        operation: &[u8; 32],
        capsule: &[u8; 32],
        review: &[u8; 32],
        payment_key: &KagemushaDevicePublicKeyV1,
        key: &KagemushaDevicePublicKeyV1,
        nonce: &[u8; 32],
    ) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b"iroha.kagemusha.spartan.local-debit-confirmation.v1\0");
        for part in [manifest, slot, wallet, operation, capsule, review] {
            h.update(part);
        }
        h.update(payment_key.as_ref());
        h.update(key.as_ref());
        h.update(nonce);
        h.finalize().into()
    }

    pub(super) fn verify(
        &self,
        prepared: &PreparedOperation,
        parameters: &AuthenticatedParameters,
    ) -> Result<()> {
        let c = prepared.capsule();
        let payment_key = KagemushaDevicePublicKeyV1::from_sec1_bytes(c.payment_key())
            .map_err(|_| Error::Malformed("debit payment key"))?;
        if !matches!(c.kind(), 3 | 6)
            || self.nonce == [0; 32]
            || self.slot == [0; 32]
            || self.key == payment_key
        {
            return Err(Error::AuthorityMismatch("debit confirmation scope/key"));
        }
        let review = Sha256::digest(prepared.review_bytes()?).into();
        let message = Self::message(
            parameters.manifest_digest(),
            &self.slot,
            c.wallet(),
            c.operation_id(),
            prepared.capsule_digest(),
            &review,
            &payment_key,
            &self.key,
            &self.nonce,
        );
        self.signature
            .verify(&self.key, &message)
            .map_err(|_| Error::AuthorityMismatch("protected debit signature"))
    }
}

/// Fresh selection consumes a volatile authorization. Selected recovery can only use archived evidence.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DebitBoundary {
    BeforeSelection,
    AnchoredSelection([u8; 32]),
    AfterSelection,
}

/// Installed Native custody is the origin of both the hardware key and the marker-selected original.
/// The application ABI never accepts a substitute implementation or a financial approval bit.
pub trait DebitAuthorizationSource: Send + Sync {
    fn evidence(
        &self,
        boundary: DebitBoundary,
        prepared: &PreparedOperation,
        parameters: &AuthenticatedParameters,
    ) -> Result<DebitApprovalEvidence>;
}
