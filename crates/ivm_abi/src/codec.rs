//! Canonical Norito helpers shared by ABI producers and consumers.
use norito::{
    NoritoSerialize,
    codec::{Decode, Encode},
    core::DecodeLimits,
};
use crate::VMError;
/// Encode a Norito value using the V1 canonical layout.
///
/// The result is independent of any ambient decode/encode flag guard.
///
/// # Errors
///
/// Returns [`VMError::NoritoInvalid`] when the value cannot be encoded.
pub fn encode_canonical_norito<T>(value: &T) -> Result<Vec<u8>, VMError>
where
    T: NoritoSerialize,
{
    norito::encode_canonical(value).map_err(|_| VMError::NoritoInvalid)
}
/// Return conservative resource limits derived from one complete Norito frame.
///
/// Packed boolean sequences may carry eight logical elements per encoded byte,
/// so sequence and cumulative element budgets use an eightfold allowance.
/// Allocation receives a wider multiplier plus a fixed 64 KiB floor for small
/// structural values. Saturating arithmetic keeps malformed length inputs
/// fail-closed.
#[must_use]
pub const fn canonical_norito_decode_limits(payload_len: usize) -> DecodeLimits {
    norito::canonical_decode_limits(payload_len)
}
/// Decode one complete Norito frame using the V1 canonical layout.
///
/// Norito frames carry layout flags and can therefore have multiple byte
/// representations for the same semantic value. Consensus and ABI boundaries
/// accept only the representation produced by [`norito::core::default_encode_flags`].
/// The guard also prevents an ambient decoder layout from changing the
/// canonicality decision.
///
/// # Errors
///
/// Returns [`VMError::NoritoInvalid`] when the payload does not decode or when
/// its byte representation is not the canonical V1 representation.
pub fn decode_canonical_norito<T>(payload: &[u8]) -> Result<T, VMError>
where
    T: Decode + Encode,
{
    norito::decode_canonical(payload).map_err(|_| VMError::NoritoInvalid)
}
/// Decode one canonical V1 frame under both default and schema-specific limits.
///
/// Norito composes nested decode-limit scopes by selecting the stricter member
/// of each budget. The payload-derived default therefore remains active even
/// if a caller accidentally supplies a looser schema limit.
///
/// # Errors
///
/// Returns [`VMError::NoritoInvalid`] when decoding exceeds either resource
/// budget, the frame is malformed, or re-encoding is not byte-for-byte
/// canonical.
pub fn decode_canonical_norito_with_limits<T>(
    payload: &[u8],
    limits: DecodeLimits,
) -> Result<T, VMError>
where
    T: Decode + Encode,
{
    norito::decode_canonical_with_limits(payload, limits).map_err(|_| VMError::NoritoInvalid)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_alternate_layout_and_restores_ambient_flags() {
        let value = vec!["first".to_owned(), "second".to_owned()];
        let canonical = {
            let _canonical =
                norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
            norito::to_bytes(&value).expect("encode canonical fixture")
        };
        assert_eq!(
            decode_canonical_norito::<Vec<String>>(&canonical),
            Ok(value.clone())
        );
        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            norito::to_bytes(&value).expect("encode alternate-layout fixture")
        };
        assert_ne!(alternate, canonical);
        assert_eq!(
            norito::decode_from_bytes::<Vec<String>>(&alternate)
                .expect("ordinary Norito accepts the advertised layout"),
            value
        );
        assert_eq!(
            decode_canonical_norito::<Vec<String>>(&alternate),
            Err(VMError::NoritoInvalid)
        );
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
        let ambient_payload = b"unrelated outer payload context";
        let _ambient_payload = norito::core::PayloadCtxGuard::enter(ambient_payload);
        let ambient_payload_context = norito::core::payload_ctx();
        let ambient_before = norito::to_bytes(&value).expect("encode ambient fixture");
        assert_eq!(
            decode_canonical_norito::<Vec<String>>(&canonical),
            Ok(value.clone())
        );
        assert_eq!(
            norito::core::payload_ctx(),
            ambient_payload_context,
            "canonical decoding must restore the caller's payload context"
        );
        assert_eq!(
            norito::to_bytes(&value).expect("encode after canonical decode"),
            ambient_before,
            "canonical decoding must restore the caller's ambient layout"
        );
    }
    #[test]
    fn generic_canonical_decode_rejects_forged_sequence_length_under_default_limits() {
        const FORGED_LENGTH: u64 = 1 << 40;
        let bare = FORGED_LENGTH.to_le_bytes();
        let payload = norito::core::frame_bare_with_header_flags::<Vec<u64>>(
            &bare,
            norito::core::default_encode_flags(),
        )
        .expect("frame forged vector length with a valid header and checksum");
        assert_eq!(
            decode_canonical_norito::<Vec<u64>>(&payload),
            Err(VMError::NoritoInvalid)
        );
    }
}
