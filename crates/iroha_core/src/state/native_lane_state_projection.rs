//! Complete original native lane state values; equality with certified R supplies authority.
use iroha_crypto::HashOf;
use iroha_data_model::{block::BlockHeader, sumeragi_lanes::SumeragiLaneState};

/// Exact complete lane state values accompanying an original native carrier.
/// These values confer no authority until matched to the proof inside that carrier's R.
#[derive(Debug, Clone, norito::Encode, norito::Decode)]
#[norito(deny_unknown_fields)]
pub struct NativeLaneStateProjectionV1 {
    /// Exact carrier whose canonical native R commits these values.
    pub carrier_height: u64,
    /// Iroha carrier identity; the native header and result remain in its sole certificate.
    pub carrier_hash: HashOf<BlockHeader>,
    /// Complete ordered lane state, including explicit emptiness.
    pub lanes: SumeragiLaneState,
}

impl norito::NoritoSchema for NativeLaneStateProjectionV1 {
    fn nominal_name() -> String {
        "iroha_core::state::NativeLaneStateProjectionV1".to_owned()
    }
    fn static_frame_name() -> Option<&'static str> {
        Some("iroha_core::state::NativeLaneStateProjectionV1")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn projection_preserves_exact_identity_and_rejects_noncanonical_suffix() {
        let projection = NativeLaneStateProjectionV1 {
            carrier_height: 2,
            carrier_hash: HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
                b"native original carrier",
            )),
            lanes: SumeragiLaneState::default(),
        };
        let bytes = norito::encode_canonical(&projection).unwrap();
        let decoded: NativeLaneStateProjectionV1 = norito::decode_canonical_with_limits(
            &bytes,
            norito::canonical_decode_limits(bytes.len()),
        )
        .unwrap();
        assert_eq!(decoded.carrier_height, projection.carrier_height);
        assert_eq!(decoded.carrier_hash, projection.carrier_hash);
        assert_eq!(decoded.lanes, projection.lanes);
        assert_eq!(norito::encode_canonical(&decoded).unwrap(), bytes);
        let mut suffix = bytes;
        suffix.push(0);
        assert!(
            norito::decode_canonical_with_limits::<NativeLaneStateProjectionV1>(
                &suffix,
                norito::canonical_decode_limits(suffix.len())
            )
            .is_err()
        );
    }
}
