//! Canonical public projections crossing the native coordinator boundary.
//!
//! These archives contain selectors, never private witnesses or serialized verified capabilities.
//! A backend must resolve them against its authenticated, durable operation state on every call.
//! Shape checks and even a valid authorization signature do not replace release admission,
//! hardware challenge consumption, current-state reconciliation or recursive proof verification.

use crate::kagemusha_device_bridge_v1::sender_payload::{
    SenderCommandBodyV1, SenderCommandV1, SenderErrorV1, SenderPreparationSelectorV1,
    SenderWalletContextV1,
};
use norito::{
    DecodeLimits, NoritoDeserialize, NoritoSerialize,
    codec::{Decode, Encode},
};

const VERSION: u16 = 1;
/// Per-archive allocation and wire limit, independent of history depth.
pub const KAGEMUSHA_CORE_COORDINATOR_ARCHIVE_MAX_BYTES_V1: usize = 16 * 1024;

/// Closed coordinator archive failures; no variant authorizes a monetary operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KagemushaCoreCoordinatorArchiveErrorV1 {
    /// Empty, oversized or resource-amplifying input.
    Size,
    /// Wrong schema, trailing data or noncanonical Norito bytes.
    CanonicalEncoding,
    /// Invalid version, selector, context or authorization binding.
    Binding,
}

type Result<T> = std::result::Result<T, KagemushaCoreCoordinatorArchiveErrorV1>;

/// Public preparation projection retained under the caller's original operation ID.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.core.v1.sender-preparation")]
pub struct KagemushaCoreSenderPreparationArchiveV1 {
    /// Sole canonical archive version, one.
    pub version: u16,
    /// Caller-persisted, nonzero operation ID.
    pub operation_id: [u8; 32],
    /// Immutable sender creation context from the authenticated session.
    pub context: SenderWalletContextV1,
    /// Digest of the exact operation ID, creation context and public inputs.
    pub inputs_digest: [u8; 32],
}

impl KagemushaCoreSenderPreparationArchiveV1 {
    /// Validate shape without claiming that Core has prepared this operation.
    pub fn validate_shape(&self) -> Result<()> {
        require(self.version == VERSION)?;
        nonzero(&self.operation_id)?;
        nonzero(&self.inputs_digest)?;
        self.context.validate_shape().map_err(sender_error)
    }

    /// Encode this exact bounded, versioned canonical projection.
    pub fn encode_canonical(&self) -> Result<Vec<u8>> {
        self.validate_shape()?;
        encode(self)
    }

    /// Decode exact canonical bytes and reject invalid public bindings.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self> {
        let archive: Self = decode(bytes)?;
        archive.validate_shape()?;
        Ok(archive)
    }
}

/// Public candidate projection returned only after the native backend persists its real proof.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.core.v1.sender-candidate")]
pub struct KagemushaCoreSenderCandidateArchiveV1 {
    /// Sole canonical archive version, one.
    pub version: u16,
    /// Original caller identity and preparation inputs.
    pub preparation: KagemushaCoreSenderPreparationArchiveV1,
    /// Exact sealed hardware preparation selector.
    pub selector: SenderPreparationSelectorV1,
    /// Digest of the retained recursive candidate.
    pub candidate_digest: [u8; 32],
    /// Canonical, low-S P-256 Core authorization for this exact hardware commit.
    pub hardware_commit_authorization: Vec<u8>,
}

impl KagemushaCoreSenderCandidateArchiveV1 {
    /// Validate public bindings and the existing operation-7 authorization contract.
    ///
    /// The native backend must still resolve the candidate from its durable journal. A caller
    /// cannot turn an archive or a self-selected signing key into an admitted Core capability.
    pub fn validate_shape(&self) -> Result<()> {
        require(self.version == VERSION)?;
        self.preparation.validate_shape()?;
        require(self.selector.inputs_digest == self.preparation.inputs_digest)?;
        if self.hardware_commit_authorization.is_empty()
            || self.hardware_commit_authorization.len() > 2 * 1024
        {
            return Err(KagemushaCoreCoordinatorArchiveErrorV1::Size);
        }
        // Reuse the same canonical commit validation as the hardware boundary. This binds the
        // authorization key, operation, input digest, preparation, candidate, release and epoch.
        SenderCommandV1 {
            version: VERSION,
            operation: 7,
            operation_id: self.preparation.operation_id,
            context: self.preparation.context.clone(),
            body: SenderCommandBodyV1::Commit {
                selector: self.selector.clone(),
                candidate_digest: self.candidate_digest,
                hardware_authorization: self.hardware_commit_authorization.clone(),
            },
        }
        .validate_shape()
        .map_err(sender_error)
    }

    /// Encode the bounded canonical candidate projection after binding validation.
    pub fn encode_canonical(&self) -> Result<Vec<u8>> {
        self.validate_shape()?;
        encode(self)
    }

    /// Decode canonical bytes, including the exact nested preparation and authorization.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self> {
        let archive: Self = decode(bytes)?;
        archive.validate_shape()?;
        Ok(archive)
    }
}

/// Public recovery projection for one retained sender operation and terminal identity.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.core.v1.sender-recovery")]
pub struct KagemushaCoreSenderRecoveryArchiveV1 {
    /// Sole canonical archive version, one.
    pub version: u16,
    /// Original caller-persisted operation ID.
    pub operation_id: [u8; 32],
    /// Exact terminal credit or redemption ID selected by the native index.
    pub terminal_id: [u8; 32],
    /// Original creation context, retained even after ordinary epoch rotation.
    pub context: SenderWalletContextV1,
    /// Exact original public-input binding.
    pub inputs_digest: [u8; 32],
}

impl KagemushaCoreSenderRecoveryArchiveV1 {
    /// Validate selectors without asserting that the operation exists or is terminal.
    pub fn validate_shape(&self) -> Result<()> {
        require(self.version == VERSION)?;
        nonzero(&self.operation_id)?;
        nonzero(&self.terminal_id)?;
        nonzero(&self.inputs_digest)?;
        self.context.validate_shape().map_err(sender_error)
    }

    /// Encode this exact bounded, versioned recovery projection.
    pub fn encode_canonical(&self) -> Result<Vec<u8>> {
        self.validate_shape()?;
        encode(self)
    }

    /// Decode exact canonical bytes and reject invalid selectors or contexts.
    pub fn decode_canonical_exact(bytes: &[u8]) -> Result<Self> {
        let archive: Self = decode(bytes)?;
        archive.validate_shape()?;
        Ok(archive)
    }
}

fn require(condition: bool) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(KagemushaCoreCoordinatorArchiveErrorV1::Binding)
    }
}

fn nonzero(digest: &[u8; 32]) -> Result<()> {
    require(digest != &[0; 32])
}

fn sender_error(error: SenderErrorV1) -> KagemushaCoreCoordinatorArchiveErrorV1 {
    match error {
        SenderErrorV1::Size => KagemushaCoreCoordinatorArchiveErrorV1::Size,
        SenderErrorV1::CanonicalEncoding => {
            KagemushaCoreCoordinatorArchiveErrorV1::CanonicalEncoding
        }
        _ => KagemushaCoreCoordinatorArchiveErrorV1::Binding,
    }
}

fn decode<T>(bytes: &[u8]) -> Result<T>
where
    T: NoritoSerialize,
    for<'de> T: NoritoDeserialize<'de>,
{
    let maximum = KAGEMUSHA_CORE_COORDINATOR_ARCHIVE_MAX_BYTES_V1;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(KagemushaCoreCoordinatorArchiveErrorV1::Size);
    }
    norito::decode_canonical_with_limits(
        bytes,
        DecodeLimits::new(maximum, maximum, maximum * 4, maximum * 8, 32),
    )
    .map_err(|_| KagemushaCoreCoordinatorArchiveErrorV1::CanonicalEncoding)
}

fn encode<T: NoritoSerialize>(archive: &T) -> Result<Vec<u8>> {
    let bytes = norito::encode_canonical(archive)
        .map_err(|_| KagemushaCoreCoordinatorArchiveErrorV1::CanonicalEncoding)?;
    if bytes.len() > KAGEMUSHA_CORE_COORDINATOR_ARCHIVE_MAX_BYTES_V1 {
        return Err(KagemushaCoreCoordinatorArchiveErrorV1::Size);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests;
