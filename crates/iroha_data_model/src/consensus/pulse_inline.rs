//! The sole generated pulse/context/anchor walks into their original inline fields.
//!
//! These records contain no owned heap child. This destination avoids an ordinary
//! archived alignment copy without granting signature, epoch or finality authority.

use super::{
    FinalizedGlobalThresholdBeaconPulseV1, GlobalThresholdBeaconChainAnchorV1,
    GlobalThresholdBeaconPulseContextV1,
};
#[cfg(test)]
use crate::NetworkId;
use crate::inline_fields::{InlineFields, InlineLeaf};
#[cfg(test)]
use iroha_crypto::HashOf;
#[cfg(test)]
use norito::core::{CanonicalField, DecodeField, FieldDestination};
use norito::core::{DecodeIntoError, DecodeRecordFields, Error};
#[cfg(test)]
use std::convert::Infallible;

impl InlineLeaf for GlobalThresholdBeaconPulseContextV1 {
    fn read(bytes: &[u8]) -> Result<Self, Error> {
        let ((instance, epoch, epoch_context_id, parent_consensus_hash, parent_result), used) =
            Self::decode_fields(bytes, &mut InlineFields).map_err(DecodeIntoError::into_codec)?;
        if used != bytes.len() {
            return Err(Error::LengthMismatch);
        }
        Ok(Self {
            instance,
            epoch,
            epoch_context_id,
            parent_consensus_hash,
            parent_result,
        })
    }
}

impl InlineLeaf for GlobalThresholdBeaconChainAnchorV1 {
    fn read(bytes: &[u8]) -> Result<Self, Error> {
        let ((height, block_hash), used) =
            Self::decode_fields(bytes, &mut InlineFields).map_err(DecodeIntoError::into_codec)?;
        if used != bytes.len() {
            return Err(Error::LengthMismatch);
        }
        Ok(Self { height, block_hash })
    }
}

impl FinalizedGlobalThresholdBeaconPulseV1 {
    /// Decode the complete original pulse payload into its inline fields.
    ///
    /// This uses the same generated field order, prefixes and canonical leaves
    /// as ordinary decoding, with no alignment scratch or heap children. The
    /// caller installs the actual advertised layout flags and original logical
    /// decode scope; prepared blocks do so through the same optional-field kernel.
    /// A framed caller must still authenticate the complete original frame.
    ///
    /// This is transport decoding only. Version, signature, seed, pulse identity,
    /// native epoch context and finalized ancestry still require their existing
    /// protocol validators. A standalone decoded value confers no custody or
    /// finality authority. In a retained block these Copy fields live inside its
    /// original prepaid block control.
    ///
    /// # Errors
    /// Returns the original framing, marked-hash, logical field/depth limit or
    /// complete-consumption failure without converting a refusal into invalidity.
    pub fn decode_inline_payload(bytes: &[u8]) -> Result<Self, Error> {
        let (
            (
                version,
                network_id,
                session_id,
                roster_hash,
                transcript_hash,
                context,
                height,
                round,
                finalized_chain_anchor,
                signature,
                seed,
                pulse_id,
            ),
            used,
        ) = Self::decode_fields(bytes, &mut InlineFields).map_err(DecodeIntoError::into_codec)?;
        if used != bytes.len() {
            return Err(Error::LengthMismatch);
        }
        Ok(Self {
            version,
            network_id,
            session_id,
            roster_hash,
            transcript_hash,
            context,
            height,
            round,
            finalized_chain_anchor,
            signature,
            seed,
            pulse_id,
        })
    }
}

#[cfg(test)]
mod tests;
