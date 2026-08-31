//! Canonical signed-transaction carrier classification for Kagemusha operations.

use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

use crate::{
    isi::{
        InstructionBox,
        offline::{RedeemKagemushaRecursiveV4, TopUpKagemushaRecursiveV4},
    },
    offline::{
        KagemushaRecursiveSpendRedeemRequestV4, KagemushaRecursiveSpendTopUpRequestV4,
        KagemushaRequestAuthorizationV2, KagemushaValidationError,
    },
    transaction::{Executable, SignedTransaction, TransactionEntrypoint},
};

/// Domain separator for the digest of one complete, authorized operation request.
pub const KAGEMUSHA_OPERATION_REQUEST_DIGEST_DOMAIN_V4: &[u8] =
    b"iroha:offline:kagemusha:operation-request:v4\0";

/// Canonical first-release Kagemusha operation kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Decode, Encode, IntoSchema)]
pub enum KagemushaOperationKindV4 {
    /// Online balance converted into an initial offline note.
    TopUp,
    /// Offline value returned to an online account.
    Redeem,
}

impl KagemushaOperationKindV4 {
    const fn digest_tag(self) -> &'static [u8] {
        match self {
            Self::TopUp => b"top_up",
            Self::Redeem => b"redeem",
        }
    }
}

/// Borrowed request carried by one canonical Kagemusha transaction.
#[derive(Clone, Copy, Debug)]
pub enum KagemushaOperationRequestV4<'a> {
    /// Canonical top-up request.
    TopUp(&'a KagemushaRecursiveSpendTopUpRequestV4),
    /// Canonical redemption request.
    Redeem(&'a KagemushaRecursiveSpendRedeemRequestV4),
}

impl<'request> KagemushaOperationRequestV4<'request> {
    /// Return the operation kind.
    #[must_use]
    pub const fn kind(self) -> KagemushaOperationKindV4 {
        match self {
            Self::TopUp(_) => KagemushaOperationKindV4::TopUp,
            Self::Redeem(_) => KagemushaOperationKindV4::Redeem,
        }
    }

    /// Return the complete self-contained authorization.
    #[must_use]
    pub const fn authorization(self) -> &'request KagemushaRequestAuthorizationV2 {
        match self {
            Self::TopUp(request) => &request.authorization,
            Self::Redeem(request) => &request.authorization,
        }
    }

    /// Return the stable operation identifier.
    #[must_use]
    pub const fn operation_id(self) -> [u8; 32] {
        self.authorization().operation_id
    }

    fn validate(self) -> Result<(), KagemushaValidationError> {
        match self {
            Self::TopUp(request) => request.validate_public_binding(),
            Self::Redeem(request) => request.validate_public_binding(),
        }
    }

    fn canonical_bytes(self) -> Result<Vec<u8>, KagemushaValidationError> {
        match self {
            Self::TopUp(request) => norito::encode_canonical(request),
            Self::Redeem(request) => norito::encode_canonical(request),
        }
        .map_err(Into::into)
    }

    /// Validate and hash every authorized request field with the operation-kind tag.
    ///
    /// # Errors
    ///
    /// Returns a closed request-validation or canonical-length failure when the
    /// request cannot be used as a Kagemusha operation identity.
    pub fn canonical_request_digest(self) -> Result<[u8; 32], KagemushaOperationCarrierErrorV4> {
        self.validate()
            .map_err(KagemushaOperationCarrierErrorV4::InvalidRequest)?;
        let bytes = self
            .canonical_bytes()
            .map_err(KagemushaOperationCarrierErrorV4::InvalidRequest)?;
        let byte_len = u64::try_from(bytes.len())
            .map_err(|_| KagemushaOperationCarrierErrorV4::RequestTooLarge)?;
        Ok(Hash::new_from_chunks(&[
            KAGEMUSHA_OPERATION_REQUEST_DIGEST_DOMAIN_V4,
            self.kind().digest_tag(),
            &byte_len.to_le_bytes(),
            &bytes,
        ])
        .into())
    }
}

/// One validated operation found in its sole permitted signed carrier shape.
#[derive(Clone, Copy, Debug)]
pub struct KagemushaOperationCarrierV4<'a> {
    request: KagemushaOperationRequestV4<'a>,
    canonical_request_digest: [u8; 32],
}

impl<'a> KagemushaOperationCarrierV4<'a> {
    fn new(
        request: KagemushaOperationRequestV4<'a>,
    ) -> Result<Self, KagemushaOperationCarrierErrorV4> {
        let canonical_request_digest = request.canonical_request_digest()?;
        Ok(Self {
            request,
            canonical_request_digest,
        })
    }

    /// Return the validated request.
    #[must_use]
    pub const fn request(self) -> KagemushaOperationRequestV4<'a> {
        self.request
    }

    /// Return the operation kind.
    #[must_use]
    pub const fn kind(self) -> KagemushaOperationKindV4 {
        self.request.kind()
    }

    /// Return the stable operation identifier.
    #[must_use]
    pub const fn operation_id(self) -> [u8; 32] {
        self.request.operation_id()
    }

    /// Return a domain-separated digest of every authorized request field.
    #[must_use]
    pub const fn canonical_request_digest(self) -> [u8; 32] {
        self.canonical_request_digest
    }
}

/// Closed failure while classifying a Kagemusha operation carrier.
#[derive(Debug, Error)]
pub enum KagemushaOperationCarrierErrorV4 {
    /// A Kagemusha instruction appeared outside the sole direct external entrypoint.
    #[error("a Kagemusha operation requires one direct external signed transaction")]
    NonExternalEntrypoint,
    /// A Kagemusha instruction appeared in a batch, overlay, or mixed instruction list.
    #[error("a Kagemusha operation must be the only instruction in its signed transaction")]
    NonCanonicalExecutable,
    /// The request failed its complete public binding contract.
    #[error("the Kagemusha operation request is invalid: {0}")]
    InvalidRequest(#[source] KagemushaValidationError),
    /// The complete canonical request length cannot be represented.
    #[error("the canonical Kagemusha operation request is too large")]
    RequestTooLarge,
}

fn operation_request(instruction: &InstructionBox) -> Option<KagemushaOperationRequestV4<'_>> {
    let instruction = instruction.as_any();
    if let Some(top_up) = instruction.downcast_ref::<TopUpKagemushaRecursiveV4>() {
        Some(KagemushaOperationRequestV4::TopUp(&top_up.request))
    } else {
        instruction
            .downcast_ref::<RedeemKagemushaRecursiveV4>()
            .map(|redeem| KagemushaOperationRequestV4::Redeem(&redeem.request))
    }
}

fn contains_operation(transaction: &SignedTransaction) -> bool {
    let executable = transaction.instructions();
    executable
        .explicit_instructions()
        .any(|instruction| operation_request(instruction).is_some())
        || matches!(
            executable,
            Executable::IvmProved(proved)
                if proved
                    .overlay
                    .iter()
                    .any(|instruction| operation_request(instruction).is_some())
        )
}

/// Classify the sole canonical signed-transaction carrier for a Kagemusha operation.
///
/// An ordinary transaction returns `Ok(None)`. Once either Kagemusha operation
/// is present, the executable must be exactly one direct native instruction.
/// Batches and proved overlays are rejected even when they contain no other
/// native instruction alongside the operation.
///
/// # Errors
///
/// Returns a closed carrier-shape or request-validation failure when a
/// Kagemusha instruction is present but is not one valid singleton carrier.
pub fn classify_kagemusha_operation_transaction_v4(
    transaction: &SignedTransaction,
) -> Result<Option<KagemushaOperationCarrierV4<'_>>, KagemushaOperationCarrierErrorV4> {
    if !contains_operation(transaction) {
        return Ok(None);
    }
    let Executable::Instructions(instructions) = transaction.instructions() else {
        return Err(KagemushaOperationCarrierErrorV4::NonCanonicalExecutable);
    };
    let [instruction] = instructions.as_ref() else {
        return Err(KagemushaOperationCarrierErrorV4::NonCanonicalExecutable);
    };
    let request = operation_request(instruction)
        .ok_or(KagemushaOperationCarrierErrorV4::NonCanonicalExecutable)?;
    KagemushaOperationCarrierV4::new(request).map(Some)
}

/// Classify a Kagemusha operation at the complete entrypoint trust boundary.
///
/// Only [`TransactionEntrypoint::External`] may carry an operation. A sealed
/// reveal containing either operation is rejected rather than treated as an
/// alternate production path.
///
/// # Errors
///
/// Returns a closed carrier-shape or request-validation failure when a
/// Kagemusha instruction is present but is not one valid external carrier.
pub fn classify_kagemusha_operation_entrypoint_v4(
    entrypoint: &TransactionEntrypoint,
) -> Result<Option<KagemushaOperationCarrierV4<'_>>, KagemushaOperationCarrierErrorV4> {
    match entrypoint {
        TransactionEntrypoint::External(transaction) => {
            classify_kagemusha_operation_transaction_v4(transaction)
        }
        TransactionEntrypoint::SealedReveal(reveal)
            if contains_operation(reveal.signed_transaction()) =>
        {
            Err(KagemushaOperationCarrierErrorV4::NonExternalEntrypoint)
        }
        TransactionEntrypoint::SealedCommitment(_)
        | TransactionEntrypoint::SealedReveal(_)
        | TransactionEntrypoint::Time(_) => Ok(None),
    }
}
