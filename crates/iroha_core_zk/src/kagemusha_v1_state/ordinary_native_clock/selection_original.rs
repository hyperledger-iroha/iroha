//! Complete public clock-selection data, authenticated only by the installing runtime owner.
use super::*;

/// Complete checkpoint plus finite metadata; carried samples have a separate bound.
pub const KAGEMUSHA_ORDINARY_NATIVE_CLOCK_SELECTION_ORIGINAL_MAX_BYTES_V1: usize =
    16 * 1024 * 1024 + 4096;

/// Sole canonical public selection original. Decoding never admits the supplied checkpoint,
/// node pins or clock policy. Shipping owners must retain its independently signed descriptor.
#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_native_clock::ClockSelectionOriginalV1")]
pub struct KagemushaOrdinaryNativeClockSelectionOriginalV1 {
    version: u16,
    checkpoint: SumeragiFinalityCheckpoint,
    network: NetworkId,
    chain_id: String,
    nodes: [KagemushaOrdinaryNativeClockNodeV1; 4],
    policy: KagemushaOrdinaryNativeClockPolicyV1,
}
impl KagemushaOrdinaryNativeClockSelectionOriginalV1 {
    /// Strict complete canonical data decoder with no old-format fallback.
    /// # Errors
    /// Refuses malformed checkpoint/node/policy relationships, trailing data or an excessive bound.
    pub fn decode_original(raw: &[u8]) -> Result<Self> {
        if raw.is_empty()
            || raw.len() > KAGEMUSHA_ORDINARY_NATIVE_CLOCK_SELECTION_ORIGINAL_MAX_BYTES_V1
        {
            return Err(Rejected);
        }
        let value: Self =
            norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
                .map_err(|_| Rejected)?;
        if value.canonical_bytes()? != raw {
            return Err(Rejected);
        }
        Ok(value)
    }
    /// Sole complete original emitter; a emitted shape is not an independently admitted root.
    /// # Errors
    /// Refuses another version, invalid selection or excessive canonical frame.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        self.selected_originals()?;
        let raw = norito::encode_canonical(self).map_err(|_| Rejected)?;
        if raw.len() > KAGEMUSHA_ORDINARY_NATIVE_CLOCK_SELECTION_ORIGINAL_MAX_BYTES_V1 {
            return Err(Rejected);
        }
        Ok(raw)
    }
    /// Construct immutable shape-checked public originals. This method grants no installation
    /// authority, current observation, account session or Native clock owner.
    /// # Errors
    /// Refuses malformed relationships or another version.
    pub fn selected_originals(&self) -> Result<KagemushaOrdinaryNativeClockOriginalsV1> {
        if self.version != 1 {
            return Err(Rejected);
        }
        KagemushaOrdinaryNativeClockOriginalsV1::from_selected_originals(
            self.checkpoint.clone(),
            self.network,
            self.chain_id.clone(),
            self.nodes.clone(),
            self.policy,
        )
    }
    /// Exact complete checkpoint data, without a selected-root admission claim.
    #[must_use]
    pub fn checkpoint(&self) -> &SumeragiFinalityCheckpoint {
        &self.checkpoint
    }
    /// Exact four-node identity data in the signed inventory order.
    #[must_use]
    pub fn nodes(&self) -> &[KagemushaOrdinaryNativeClockNodeV1; 4] {
        &self.nodes
    }
    /// Exact governed finite policy data, without a current elapsed clock.
    #[must_use]
    pub fn policy(&self) -> KagemushaOrdinaryNativeClockPolicyV1 {
        self.policy
    }
    /// Public network scope, checked again against the installing Native release.
    #[must_use]
    pub fn network(&self) -> NetworkId {
        self.network
    }
}
impl KagemushaOrdinaryNativeClockOriginalsV1 {
    /// Emit exact public selection data. Authority remains with the actual independently
    /// admitted inventory that selected and retains these fields.
    /// # Errors
    /// Refuses canonical framing or an excessive full checkpoint.
    pub fn canonical_selection_original(&self) -> Result<Vec<u8>> {
        KagemushaOrdinaryNativeClockSelectionOriginalV1 {
            version: 1,
            checkpoint: self.checkpoint.clone(),
            network: self.network,
            chain_id: self.chain_id.clone(),
            nodes: self.nodes.clone(),
            policy: self.policy,
        }
        .canonical_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_clock_selection_exact_roundtrip_preserves_installed_identity() {
        use iroha_crypto::KeyPair;
        use iroha_data_model::sumeragi_finality::test_fixtures::NativeFinalityFixture;
        let mut native = NativeFinalityFixture::start("clock-selection-original-fixture");
        native.certify_with_world_root(
            native.block_with_submitted_work(native.next_header()),
            Hash::new(b"synthetic public clock selection World"),
        );
        let selected = KagemushaOrdinaryNativeClockOriginalsV1::from_selected_originals(
            native
                .verifier()
                .export_checkpoint(native.latest())
                .unwrap(),
            native.network_id(),
            "clock-selection-original-fixture".into(),
            std::array::from_fn(|i| KagemushaOrdinaryNativeClockNodeV1 {
                peer_id: PeerId::new(
                    KeyPair::from_seed(vec![i as u8 + 1; 32], Algorithm::BlsNormal)
                        .public_key()
                        .clone(),
                ),
                build_fingerprint: Hash::new(b"synthetic installed build"),
                config_fingerprint: Hash::new(b"synthetic installed config"),
            }),
            KagemushaOrdinaryNativeClockPolicyV1 {
                maximum_reply_age_ms: 10_000,
                maximum_node_skew_ms: 100,
                maximum_projection_age_ms: 120_000,
                maximum_persistence_age_ms: 1_000,
            },
        )
        .unwrap();
        let raw = selected.canonical_selection_original().unwrap();
        let decoded =
            KagemushaOrdinaryNativeClockSelectionOriginalV1::decode_original(&raw).unwrap();
        assert_eq!(
            decoded.selected_originals().unwrap().selection_digest(),
            selected.selection_digest()
        );
        let mut trailing = raw.clone();
        trailing.push(0);
        assert!(
            KagemushaOrdinaryNativeClockSelectionOriginalV1::decode_original(&trailing).is_err()
        );
        let mut changed = decoded.clone();
        changed.version = 2;
        assert!(changed.canonical_bytes().is_err());
        let mut changed = decoded.clone();
        changed.nodes[1] = changed.nodes[0].clone();
        assert!(changed.canonical_bytes().is_err());
        let mut changed = decoded;
        changed.policy.maximum_reply_age_ms = 0;
        assert!(changed.canonical_bytes().is_err());
    }
}
