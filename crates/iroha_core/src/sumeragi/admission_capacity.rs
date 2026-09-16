//! Immutable signed RS16 geometry published only after authenticated startup recovery.
//!
//! This owner is neither a live lane/global authority nor a complete admission
//! feasibility receipt. Global carrier overhead and native input descriptor size
//! must be counted by their canonical encoding owners before durable acceptance.
//! TODO: feed this authenticated bound into the shared preacceptance check once
//! global, native and transport envelope counts are enforced together.

use std::sync::OnceLock;

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
    /// Local operational candidate/frame limits remain additional restrictions.
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
pub(super) fn publish_authenticated_capacity(
    slot: &OnceLock<AuthenticatedAdmissionCapacityV1>,
    verified: &VerifiedHeightContext,
) -> Result<(), String> {
    let context = verified.context();
    let capacity = AuthenticatedAdmissionCapacityV1 {
        network_id: context.network_id,
        protocol_version: context.protocol_version,
        layout: context.da_layout,
    };
    // Reuse the same exact geometry kernel as the native and global DA codec.
    capacity.check_payload_size(&capacity.network_id, capacity.layout.max_payload_size_bytes)?;
    slot.set(capacity)
        .map_err(|_| "authenticated admission capacity was already published".to_owned())
}

#[cfg(all(test, feature = "bls"))]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    use iroha_model_base::peer::PeerId;

    fn verified_context() -> VerifiedHeightContext {
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
        let (kagemusha_mint_finality_epoch_id, kagemusha_mint_finality_epoch_roster) =
            crate::kagemusha_v1_test_fixtures::mint_finality_roster_and_id(network_id, 1, &roster);
        let context = wire::HeightContext {
            network_id,
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 1,
            epoch_end_height: 100,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).unwrap(),
            roster,
            kagemusha_mint_finality_epoch_id,
            kagemusha_mint_finality_epoch_roster,
            nexus_amx_context_hash: Hash::new(b"nexus"),
            execution_policy_hash: Hash::new(b"policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 64,
                data_shards: 2,
                parity_shards: 1,
                max_payload_size_bytes: 256,
                max_chunk_count: 6,
            },
            leader_seed: [9; 32],
        };
        let pops = keys
            .iter()
            .map(|key| iroha_crypto::bls_normal_pop_prove(key.private_key()).unwrap())
            .collect();
        VerifiedHeightContext::genesis(context, pops).unwrap()
    }

    #[test]
    fn recovered_capacity_is_exact_once_and_handle_pending_is_explicit() {
        let (handle, _, _) = super::super::test_sumeragi_handle(1);
        assert_eq!(
            handle.authenticated_admission_capacity(),
            Err(AdmissionCapacityUnavailableV1::Pending)
        );
        let verified = verified_context();
        publish_authenticated_capacity(&handle.admission_capacity, &verified).unwrap();
        let capacity = handle.authenticated_admission_capacity().unwrap();
        assert_eq!(capacity.layout(), verified.context().da_layout);
        assert_eq!(capacity.network_id(), verified.context().network_id);
        assert_eq!(capacity.protocol_version(), wire::PROTOCOL_VERSION);
        assert!(publish_authenticated_capacity(&handle.admission_capacity, &verified).is_err());
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
        publish_authenticated_capacity(&slot, &verified).unwrap();
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
}
