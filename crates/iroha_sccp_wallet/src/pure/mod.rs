//! Network-free SCCP wallet primitives (spec §7, §8).
//!
//! Everything here is deterministic and free of I/O, so SDK bridges can export it over FFI:
//!
//! - [`bundle`]: message proof bundles verified against a destination's roster state, with the
//!   signature set trimmed to exactly `t`;
//! - [`rotation`]: roster rotation chains verified against every §5.1.5 bound and split into
//!   `rotateRosters` batches;
//! - [`control`]: Parliament control bundles and the §7.1 control decisions;
//! - [`evm`]: EVM calldata, EIP-1559 transactions and signing, unsigned export and owner-only key
//!   files; [`tron`] and [`ton`] hold the other destination encodings.
//!
//! The byte-level entry points below take the headered Norito frames Torii serves
//! (`Accept: application/x-norito`) and are the stable surface the SDK bridges wrap
//! (`connect_norito_bridge`, `iroha_js_host`).

pub mod bundle;
pub mod control;
pub mod evm;
pub mod rotation;
pub mod ton;
pub mod tron;

use core::fmt;

use iroha_sccp::api::{SccpControlProofBundleV1, SccpMessageProofBundleV1, SccpRotationChainV1};

use self::{
    bundle::{BundleError, BundlePurposeV1, DestinationContextV1, VerifiedMessageBundleV1},
    control::{
        ControlBundleError, DestinationControlStateV1, VerifiedControlBundleV1,
        verify_control_bundle,
    },
    rotation::{RotationChainError, RotationPlanV1, verify_rotation_chain},
};

/// Why a framed Torii record could not be verified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    /// The bytes are not a headered Norito frame of the expected record.
    Decode(String),
    /// A message bundle failed verification.
    Bundle(BundleError),
    /// A control bundle failed verification.
    Control(ControlBundleError),
    /// A rotation chain failed verification.
    Rotation(RotationChainError),
}

impl fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Decode(message) => write!(formatter, "invalid Torii frame: {message}"),
            Self::Bundle(error) => error.fmt(formatter),
            Self::Control(error) => error.fmt(formatter),
            Self::Rotation(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for FrameError {}

fn decode_frame<T>(frame: &[u8]) -> Result<T, FrameError>
where
    T: for<'a> norito::NoritoDeserialize<'a> + norito::NoritoSerialize + norito::NoritoSchema,
{
    norito::decode_from_bytes::<T>(frame).map_err(|error| FrameError::Decode(error.to_string()))
}

/// Decode a framed `SccpMessageProofBundleV1` and verify it for `purpose`.
///
/// # Errors
///
/// Returns [`FrameError::Decode`] or [`FrameError::Bundle`].
pub fn verify_message_bundle_frame(
    frame: &[u8],
    context: &DestinationContextV1,
    purpose: BundlePurposeV1,
) -> Result<VerifiedMessageBundleV1, FrameError> {
    let bundle: SccpMessageProofBundleV1 = decode_frame(frame)?;
    bundle::verify_message_bundle(&bundle, context, purpose).map_err(FrameError::Bundle)
}

/// Decode a framed `SccpControlProofBundleV1` and verify it.
///
/// # Errors
///
/// Returns [`FrameError::Decode`] or [`FrameError::Control`].
pub fn verify_control_bundle_frame(
    frame: &[u8],
    context: &DestinationContextV1,
    applied: &DestinationControlStateV1,
) -> Result<VerifiedControlBundleV1, FrameError> {
    let bundle: SccpControlProofBundleV1 = decode_frame(frame)?;
    verify_control_bundle(&bundle, context, applied).map_err(FrameError::Control)
}

/// Decode a framed `SccpRotationChainV1` and verify it up to `target_generation`.
///
/// # Errors
///
/// Returns [`FrameError::Decode`] or [`FrameError::Rotation`].
pub fn verify_rotation_chain_frame(
    frame: &[u8],
    context: &DestinationContextV1,
    target_generation: u64,
) -> Result<RotationPlanV1, FrameError> {
    let chain: SccpRotationChainV1 = decode_frame(frame)?;
    verify_rotation_chain(
        &chain,
        &context.roster_state,
        target_generation,
        &context.taira_network_id,
        context.now_ms,
    )
    .map_err(FrameError::Rotation)
}

#[cfg(test)]
mod tests {
    use iroha_data_model::bridge::SccpNetworkV1;
    use iroha_sccp::v1::{hashes::word_address, proof::DestinationV1, roster::RosterStateV1};

    use super::*;

    fn context() -> DestinationContextV1 {
        DestinationContextV1 {
            taira_network_id: [0x11; 32],
            destination: DestinationV1 {
                network: SccpNetworkV1::EthereumMainnet,
                route_revision: 1,
                destination_word: word_address(&[0x22; 20]),
            },
            roster_state: RosterStateV1 {
                digest: [1; 32],
                generation: 4,
                valid_until_ms: 10,
                prev_digest: [0; 32],
                prev_valid_until_ms: 0,
            },
            now_ms: 5,
        }
    }

    #[test]
    fn frames_must_decode_before_verification() {
        assert!(matches!(
            verify_message_bundle_frame(&[1, 2, 3], &context(), BundlePurposeV1::Finalize),
            Err(FrameError::Decode(_))
        ));
        assert!(matches!(
            verify_control_bundle_frame(&[], &context(), &DestinationControlStateV1::default()),
            Err(FrameError::Decode(_))
        ));
        let frame = norito::to_bytes(&SccpRotationChainV1::default()).expect("frame");
        // An empty chain leaves the destination at generation 4.
        assert_eq!(
            verify_rotation_chain_frame(&frame, &context(), 4)
                .expect("empty plan")
                .final_state
                .generation,
            4
        );
        assert!(matches!(
            verify_rotation_chain_frame(&frame, &context(), 5),
            Err(FrameError::Rotation(
                RotationChainError::ChainTooShort { .. }
            ))
        ));
        let message_frame = norito::to_bytes(&SccpRotationChainV1::default()).expect("frame");
        assert!(matches!(
            verify_message_bundle_frame(&message_frame, &context(), BundlePurposeV1::Finalize),
            Err(FrameError::Decode(_))
        ));
        assert!(
            FrameError::Decode("x".to_owned())
                .to_string()
                .contains("frame")
        );
    }
}
