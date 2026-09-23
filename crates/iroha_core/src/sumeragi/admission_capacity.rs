//! Immutable signed RS16 geometry published only after authenticated startup recovery.
//!
//! This owner is neither a live lane/global authority nor a complete admission
//! feasibility receipt. Global carrier overhead and native input descriptor size
//! must be counted by their canonical encoding owners before durable acceptance.
//! TODO: feed this authenticated bound into the shared preacceptance check once
//! global, native and transport envelope counts are enforced together.

use std::sync::OnceLock;

use iroha_config::parameters::actual::SumeragiV2Config;
use iroha_data_model::{NetworkId, block::consensus_v2 as wire};

use super::v2::VerifiedHeightContext;

/// Why authenticated startup capacity cannot be observed from this handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AdmissionCapacityUnavailableV1 {
    /// Recovery has not yet authenticated its immutable context.
    #[error("authenticated admission capacity awaits startup recovery")]
    Pending,
    /// Emergency read-only startup never authorizes transaction admission.
    #[error("admission is disabled in emergency read-only startup")]
    Disabled,
    /// The process consensus owner has failed closed.
    #[error("admission requires process restart")]
    RestartRequired,
}

/// Network-bound signed RS16 capacity shared by global and frozen lane contexts.
///
/// Fields and construction are private. The production constructor consumes a
/// read-only projection from the actual recovered `VerifiedHeightContext`, never
/// raw metadata, configuration defaults, or a caller-provided layout. Successor
/// verification preserves this layout and lane opening copies it from that
/// authenticated global context. This does not grant signing or queue ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthenticatedAdmissionCapacityV1 {
    network_id: NetworkId,
    protocol_version: u16,
    layout: wire::DataAvailabilityLayout,
}

impl AuthenticatedAdmissionCapacityV1 {
    /// Exact immutable network whose signed lineage supplies this layout.
    #[must_use]
    pub const fn network_id(&self) -> NetworkId {
        self.network_id
    }

    /// Signed protocol version from authenticated recovery.
    #[must_use]
    pub const fn protocol_version(&self) -> u16 {
        self.protocol_version
    }

    /// Borrow the authenticated immutable layout for canonical encoding owners.
    #[must_use]
    pub const fn layout(&self) -> wire::DataAvailabilityLayout {
        self.layout
    }

    /// Check an actual canonical payload length against the signed RS16 geometry.
    ///
    /// `bytes` must include the complete canonical body that the relevant DA
    /// encoder will receive. An entrypoint length or certificate length alone
    /// cannot establish full global/native carrier feasibility. This function
    /// makes no claim that a body exists, is valid, or has enough storage owners.
    /// Local frame and aggregate storage limits remain additional restrictions;
    /// startup requires local body capacity to cover the whole signed envelope.
    ///
    /// # Errors
    /// Rejects a foreign network, empty/oversized body, invalid signed geometry,
    /// or a stripe-rounded encoded body outside the signed chunk capacity.
    pub fn check_payload_size(
        &self,
        network_id: &NetworkId,
        bytes: u64,
    ) -> Result<Rs16PayloadGeometryV1, String> {
        if network_id != &self.network_id {
            return Err("admission capacity belongs to another network".to_owned());
        }
        let chunk_count = wire::expected_encoded_chunk_count(bytes, self.layout)
            .map_err(|error| format!("payload exceeds authenticated RS16 geometry: {error}"))?;
        if chunk_count > self.layout.max_chunk_count {
            return Err("payload exceeds signed RS16 chunk count".to_owned());
        }
        let encoded_bytes = u64::from(chunk_count)
            .checked_mul(u64::from(self.layout.chunk_size_bytes))
            .ok_or_else(|| "RS16 encoded body size overflows".to_owned())?;
        if encoded_bytes > wire::MAX_DA_ENCODED_PAYLOAD_BYTES {
            return Err("payload exceeds protocol encoded RS16 bound".to_owned());
        }
        Ok(Rs16PayloadGeometryV1 {
            payload_bytes: bytes,
            chunk_count,
            encoded_bytes,
        })
    }
}

/// Exact geometry of one complete body, not evidence of availability or authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rs16PayloadGeometryV1 {
    payload_bytes: u64,
    chunk_count: u32,
    encoded_bytes: u64,
}
impl Rs16PayloadGeometryV1 {
    /// Exact canonical unpadded body length checked by the encoder owner.
    #[must_use]
    pub const fn payload_bytes(self) -> u64 {
        self.payload_bytes
    }
    /// Total data plus parity shards, including final stripe padding.
    #[must_use]
    pub const fn chunk_count(self) -> u32 {
        self.chunk_count
    }
    /// Total encoded bytes across all data and parity shards.
    #[must_use]
    pub const fn encoded_bytes(self) -> u64 {
        self.encoded_bytes
    }
}

/// Publish once at the actual recovered context boundary, before readiness.
///
/// Repeated initialization is a lifecycle error even when its bytes match; no
/// later height can replace this process-wide capacity projection. The current
/// context and State remain the independent owners of ongoing live authority.
/// `config` is the actual configuration's validated `v2_config` projection. Its
/// physical body allocations must cover the signed envelope before publication.
pub(super) fn publish_authenticated_capacity(
    slot: &OnceLock<AuthenticatedAdmissionCapacityV1>,
    verified: &VerifiedHeightContext,
    config: &SumeragiV2Config,
) -> Result<(), String> {
    let context = verified.context();
    let capacity = AuthenticatedAdmissionCapacityV1 {
        network_id: context.network_id,
        protocol_version: context.protocol_version,
        layout: context.da_layout,
    };
    // Reuse the same exact geometry kernel as the native and global DA codec.
    capacity.check_payload_size(&capacity.network_id, capacity.layout.max_payload_size_bytes)?;
    require_local_payload_capacity(capacity.layout, config)?;
    slot.set(capacity)
        .map_err(|_| "authenticated admission capacity was already published".to_owned())
}

/// Require local resources for the entire signed payload envelope.
///
/// The actual configuration validator derives ready-body and isolated source
/// allocations from the configured body capacity. A smaller local candidate
/// limit cannot redefine the signed protocol envelope. Check the retained
/// allocation projection as well, so an inconsistent projection cannot publish
/// admission capacity or construct a candidate consumer.
pub(super) fn require_local_payload_capacity(
    layout: wire::DataAvailabilityLayout,
    config: &SumeragiV2Config,
) -> Result<(), String> {
    let required = layout.max_payload_size_bytes;
    if required == 0 {
        return Err("signed RS16 payload capacity must be non-zero".to_owned());
    }
    for (owner, available) in [
        ("configured block payload", config.limits.max_payload_bytes),
        ("ready-body allocation", config.limits.ready_body_bytes),
        (
            "per-source body allocation",
            config.limits.body_source_bytes,
        ),
    ] {
        if available < required {
            return Err(format!(
                "local {owner} capacity {available} is below signed RS16 payload capacity {required}"
            ));
        }
    }
    Ok(())
}

#[cfg(all(test, feature = "bls"))]
mod tests {
    use super::*;
    use iroha_config::parameters::actual::Sumeragi;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_model_base::peer::PeerId;
    use std::{num::NonZeroUsize, time::Duration};

    fn verified_context() -> VerifiedHeightContext {
        verified_context_with_layout(wire::DataAvailabilityLayout {
            encoding: wire::PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 64,
            data_shards: 2,
            parity_shards: 1,
            max_payload_size_bytes: 256,
            max_chunk_count: 6,
        })
    }

    fn verified_context_with_layout(
        da_layout: wire::DataAvailabilityLayout,
    ) -> VerifiedHeightContext {
        let mut keys = (1_u8..=4)
            .map(|seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap())
            .collect::<Vec<_>>();
        keys.sort_by(|a, b| a.public_key().cmp(b.public_key()));
        let roster = keys
            .iter()
            .map(|key| wire::ValidatorPower {
                validator: PeerId::new(key.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"capacity fixture",
        )));
        let (kagemusha_mint_finality_authorization, kagemusha_mint_finality_authority) =
            crate::kagemusha_v1_test_fixtures::mint_finality_genesis_authorization(
                network_id, 100, &roster,
            );
        let context = wire::HeightContext {
            network_id,
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).unwrap(),
            roster,
            kagemusha_mint_finality_authorization,
            kagemusha_mint_finality_authority,
            nexus_amx_context_hash: Hash::new(b"nexus"),
            execution_policy_hash: Hash::new(b"policy"),
            da_layout,
            leader_seed: [9; 32],
        };
        let pops = keys
            .iter()
            .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap())
            .collect();
        VerifiedHeightContext::genesis(context, pops).unwrap()
    }

    fn actual_config(payload_bytes: usize) -> SumeragiV2Config {
        let mut actual = Sumeragi::default();
        actual.block.max_payload_bytes = NonZeroUsize::new(payload_bytes).unwrap();
        actual
            .v2_config(Duration::from_secs(1), wire::ConsensusMode::Permissioned)
            .expect("actual local resource configuration is valid")
    }

    #[test]
    fn recovered_capacity_is_exact_once_and_handle_pending_is_explicit() {
        let (handle, _, _) = super::super::test_sumeragi_handle(1);
        assert_eq!(
            handle.authenticated_admission_capacity(),
            Err(AdmissionCapacityUnavailableV1::Pending)
        );
        let verified = verified_context();
        let config = actual_config(Sumeragi::default().block.max_payload_bytes.get());
        publish_authenticated_capacity(&handle.admission_capacity, &verified, &config).unwrap();
        let capacity = handle.authenticated_admission_capacity().unwrap();
        assert_eq!(capacity.layout(), verified.context().da_layout);
        assert_eq!(capacity.network_id(), verified.context().network_id);
        assert_eq!(capacity.protocol_version(), wire::PROTOCOL_VERSION);
        assert!(
            publish_authenticated_capacity(&handle.admission_capacity, &verified, &config).is_err()
        );
        assert_eq!(handle.authenticated_admission_capacity().unwrap(), capacity);
        assert_eq!(
            super::super::SumeragiHandle::emergency_fast_disabled()
                .authenticated_admission_capacity(),
            Err(AdmissionCapacityUnavailableV1::Disabled)
        );
        handle.output_guard.activate_restart_required();
        assert_eq!(
            handle.authenticated_admission_capacity(),
            Err(AdmissionCapacityUnavailableV1::RestartRequired)
        );
    }

    #[test]
    fn authenticated_capacity_counts_exact_rs16_stripes_and_rejects_wrong_network() {
        let verified = verified_context();
        let slot = OnceLock::new();
        let config = actual_config(Sumeragi::default().block.max_payload_bytes.get());
        publish_authenticated_capacity(&slot, &verified, &config).unwrap();
        let capacity = slot.get().unwrap();
        for length in [1_u64, 127, 128, 129, 255, 256] {
            let geometry = capacity
                .check_payload_size(&capacity.network_id(), length)
                .unwrap();
            let chunks =
                wire::encode_payload_chunks(capacity.layout(), &vec![0x73; length as usize])
                    .unwrap();
            assert_eq!(geometry.payload_bytes(), length);
            assert_eq!(geometry.chunk_count() as usize, chunks.len());
            assert_eq!(
                geometry.encoded_bytes() as usize,
                chunks.iter().map(Vec::len).sum::<usize>()
            );
        }
        assert!(
            capacity
                .check_payload_size(&capacity.network_id(), 0)
                .is_err()
        );
        assert!(
            capacity
                .check_payload_size(&capacity.network_id(), 257)
                .is_err()
        );
        let foreign =
            NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"foreign")));
        assert!(capacity.check_payload_size(&foreign, 1).is_err());
    }

    #[test]
    fn recovered_capacity_requires_local_body_capacity_before_once_publication() {
        let verified = verified_context_with_layout(wire::recommended_data_availability_layout());
        let signed_max = usize::try_from(verified.context().da_layout.max_payload_size_bytes)
            .expect("recommended signed capacity fits the host");
        for local_bytes in [signed_max - 1, signed_max, signed_max + 1] {
            let config = actual_config(local_bytes);
            let slot = OnceLock::new();
            let result = publish_authenticated_capacity(&slot, &verified, &config);
            if local_bytes < signed_max {
                let error = result.expect_err("local capacity cannot narrow signed semantics");
                assert!(error.contains("configured block payload"), "{error}");
                assert!(
                    slot.get().is_none(),
                    "refusal must not publish readiness capacity"
                );
                publish_authenticated_capacity(&slot, &verified, &actual_config(signed_max))
                    .expect("the same empty slot can admit a supported configuration");
            } else {
                result.expect("equal or greater local resource capacity supports the context");
            }
            assert_eq!(slot.get().unwrap().layout(), verified.context().da_layout);
            assert_eq!(config.limits.max_payload_bytes, local_bytes as u64);
        }
    }

    #[test]
    fn recommended_signed_capacity_is_covered_by_actual_default_allocations() {
        let actual = Sumeragi::default();
        let config = actual
            .v2_config(Duration::from_secs(1), wire::ConsensusMode::Permissioned)
            .expect("default resource configuration");
        let verified = verified_context_with_layout(wire::recommended_data_availability_layout());
        let signed_max = verified.context().da_layout.max_payload_size_bytes;
        assert!(config.limits.max_payload_bytes >= signed_max);
        assert!(config.limits.ready_body_bytes >= signed_max);
        assert!(config.limits.body_source_bytes >= signed_max);
        let slot = OnceLock::new();
        publish_authenticated_capacity(&slot, &verified, &config).unwrap();
        assert_eq!(slot.get().unwrap().layout(), verified.context().da_layout);
    }

    #[test]
    fn recovered_capacity_rejects_inconsistent_physical_allocations_without_publication() {
        let verified = verified_context_with_layout(wire::recommended_data_availability_layout());
        let signed_max = verified.context().da_layout.max_payload_size_bytes;
        let config = actual_config(usize::try_from(signed_max).unwrap());
        for source_allocation in [false, true] {
            let mut inconsistent = config.clone();
            let owner = if source_allocation {
                inconsistent.limits.body_source_bytes = signed_max - 1;
                "per-source body allocation"
            } else {
                inconsistent.limits.ready_body_bytes = signed_max - 1;
                "ready-body allocation"
            };
            let slot = OnceLock::new();
            let error = publish_authenticated_capacity(&slot, &verified, &inconsistent)
                .expect_err("no consumer may receive an underallocated signed envelope");
            assert!(error.contains(owner), "{error}");
            assert!(slot.get().is_none());
        }
    }
}
