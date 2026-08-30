//! Deterministic, bounded payload dispersal for Sumeragi v2.
//!
//! This module is transport, not consensus. It derives the one canonical
//! chunk sequence committed by [`wire::PayloadManifest`], buffers authenticated
//! chunks for the active session, and reconstructs exact canonical block
//! bytes. Partial acquisition is deliberately volatile: after restart the
//! node reacquires shards, while the reconstructed canonical body crosses the
//! separate durable body-store boundary before validation or voting. The
//! reducer sees only the resulting body-availability token; READY/DELIVER
//! state and collector selection do not exist here.
use super::v2_transport::AuthenticatedPayloadChunk;
use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::consensus_v2 as wire;
use iroha_primitives::erasure::rs16;
#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};
use thiserror::Error;
/// Canonical encoded payload and the manifest committing to every chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EncodedV2Payload {
    manifest: wire::PayloadManifest,
    chunks: Vec<Vec<u8>>,
}
impl EncodedV2Payload {
    /// Borrow the canonical manifest.
    pub(crate) const fn manifest(&self) -> &wire::PayloadManifest {
        &self.manifest
    }
    /// Consume the encoded payload.
    pub(crate) fn into_parts(self) -> (wire::PayloadManifest, Vec<Vec<u8>>) {
        (self.manifest, self.chunks)
    }
}
/// Result of admitting one authenticated chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChunkAdmission {
    /// A previously missing canonical chunk was buffered.
    Buffered,
    /// The exact chunk is already present in this bounded session.
    Duplicate,
}
/// Bounded in-memory reconstruction session for one immutable manifest.
#[derive(Debug)]
pub(crate) struct V2ChunkSession {
    manifest: wire::PayloadManifest,
    manifest_hash: HashOf<wire::PayloadManifest>,
    chunks: Vec<Option<Vec<u8>>>,
    stripe_width: usize,
    shards_needed_by_stripe: Vec<u16>,
    stripes_waiting: usize,
    #[cfg(test)]
    reconstruction_attempts: AtomicUsize,
    #[cfg(test)]
    payload_allocation_attempts: AtomicUsize,
}
impl V2ChunkSession {
    /// Open an empty bounded session for the exact validated manifest.
    pub(crate) fn open(
        context: &wire::HeightContext,
        manifest: wire::PayloadManifest,
    ) -> Result<Self, V2ChunkError> {
        manifest.validate(context)?;
        let chunk_count = manifest.chunk_hashes.len();
        let stripe_width = usize::from(manifest.layout.data_shards)
            .checked_add(usize::from(manifest.layout.parity_shards))
            .ok_or(V2ChunkError::InvalidErasureLayout)?;
        if stripe_width == 0 || !chunk_count.is_multiple_of(stripe_width) {
            return Err(V2ChunkError::InvalidErasureLayout);
        }
        let stripe_count = chunk_count / stripe_width;
        let manifest_hash = HashOf::new(&manifest);
        let data_shards = manifest.layout.data_shards;
        Ok(Self {
            manifest,
            manifest_hash,
            chunks: vec![None; chunk_count],
            stripe_width,
            shards_needed_by_stripe: vec![data_shards; stripe_count],
            stripes_waiting: stripe_count,
            #[cfg(test)]
            reconstruction_attempts: AtomicUsize::new(0),
            #[cfg(test)]
            payload_allocation_attempts: AtomicUsize::new(0),
        })
    }
    /// Borrow the immutable manifest.
    pub(crate) const fn manifest(&self) -> &wire::PayloadManifest {
        &self.manifest
    }
    /// Buffer one structurally and cryptographically authenticated chunk.
    ///
    /// The authentication seal carries the hash already verified with the
    /// sender signature. This boundary independently rechecks manifest
    /// identity, index, length, and that cached hash before retaining bounded
    /// session memory.
    pub(crate) fn admit(
        &mut self,
        authenticated: AuthenticatedPayloadChunk,
    ) -> Result<ChunkAdmission, V2ChunkError> {
        let (chunk, chunk_hash) = authenticated.into_parts();
        if chunk.manifest_hash != self.manifest_hash {
            return Err(V2ChunkError::ManifestMismatch);
        }
        let index = usize::try_from(chunk.index).map_err(|_| V2ChunkError::ChunkIndexOutOfRange)?;
        let expected_hash = self
            .manifest
            .chunk_hashes
            .get(index)
            .ok_or(V2ChunkError::ChunkIndexOutOfRange)?;
        if chunk_hash != *expected_hash {
            return Err(V2ChunkError::ChunkHashMismatch);
        }
        self.validate_chunk_len(&chunk.bytes)?;
        self.admit_validated_owned(index, chunk.bytes)
    }
    /// Buffer already-authenticated bytes at an exact manifest index.
    pub(crate) fn admit_bytes(
        &mut self,
        index: u32,
        bytes: &[u8],
    ) -> Result<ChunkAdmission, V2ChunkError> {
        let index = usize::try_from(index).map_err(|_| V2ChunkError::ChunkIndexOutOfRange)?;
        if let Some(existing) = self
            .chunks
            .get(index)
            .ok_or(V2ChunkError::ChunkIndexOutOfRange)?
        {
            if existing == bytes {
                return Ok(ChunkAdmission::Duplicate);
            }
            self.validate_chunk(index, bytes)?;
            return Err(V2ChunkError::ConflictingChunk);
        }
        self.validate_chunk(index, bytes)?;
        self.admit_validated_owned(index, bytes.to_vec())
    }
    fn admit_validated_owned(
        &mut self,
        index: usize,
        bytes: Vec<u8>,
    ) -> Result<ChunkAdmission, V2ChunkError> {
        let stripe = index / self.stripe_width;
        let slot = self
            .chunks
            .get_mut(index)
            .ok_or(V2ChunkError::ChunkIndexOutOfRange)?;
        if let Some(existing) = slot {
            return if existing == &bytes {
                Ok(ChunkAdmission::Duplicate)
            } else {
                Err(V2ChunkError::ConflictingChunk)
            };
        }
        let shards_needed = self
            .shards_needed_by_stripe
            .get_mut(stripe)
            .ok_or(V2ChunkError::InvalidErasureLayout)?;
        if *shards_needed == 1 && self.stripes_waiting == 0 {
            return Err(V2ChunkError::InvalidErasureLayout);
        }
        *slot = Some(bytes);
        if *shards_needed > 0 {
            *shards_needed -= 1;
            if *shards_needed == 0 {
                self.stripes_waiting -= 1;
            }
        }
        Ok(ChunkAdmission::Buffered)
    }
    /// Reconstruct and verify the canonical payload once enough chunks exist.
    ///
    /// RS16 reconstruction needs any `data_shards` chunks per stripe. Missing
    /// parity chunks are not materialized unless needed to recover data.
    pub(crate) fn reconstruct(&self) -> Result<Option<Vec<u8>>, V2ChunkError> {
        #[cfg(test)]
        self.reconstruction_attempts.fetch_add(1, Ordering::Relaxed);
        if self.stripes_waiting != 0 {
            return Ok(None);
        }
        let payload = self.reconstruct_rs16()?;
        if u64::try_from(payload.len()).unwrap_or(u64::MAX) != self.manifest.payload_size_bytes
            || Hash::new(&payload) != self.manifest.subject.payload_hash
        {
            return Err(V2ChunkError::PayloadMismatch);
        }
        Ok(Some(payload))
    }
    /// Return how often reconstruction was attempted in this test session.
    #[cfg(test)]
    pub(crate) fn reconstruction_attempts(&self) -> usize {
        self.reconstruction_attempts.load(Ordering::Relaxed)
    }
    /// Return how often reconstruction reached payload allocation in tests.
    #[cfg(test)]
    pub(crate) fn payload_allocation_attempts(&self) -> usize {
        self.payload_allocation_attempts.load(Ordering::Relaxed)
    }
    fn validate_chunk(&self, index: usize, bytes: &[u8]) -> Result<(), V2ChunkError> {
        let expected_hash = self
            .manifest
            .chunk_hashes
            .get(index)
            .ok_or(V2ChunkError::ChunkIndexOutOfRange)?;
        self.validate_chunk_len(bytes)?;
        if Hash::new(bytes) != *expected_hash {
            return Err(V2ChunkError::ChunkHashMismatch);
        }
        Ok(())
    }
    fn validate_chunk_len(&self, bytes: &[u8]) -> Result<(), V2ChunkError> {
        let chunk_size = usize::try_from(self.manifest.layout.chunk_size_bytes)
            .map_err(|_| V2ChunkError::InvalidChunkLength)?;
        if bytes.len() != chunk_size || bytes.is_empty() {
            return Err(V2ChunkError::InvalidChunkLength);
        }
        Ok(())
    }
    fn reconstruct_rs16(&self) -> Result<Vec<u8>, V2ChunkError> {
        let data_shards = usize::from(self.manifest.layout.data_shards);
        let parity_shards = usize::from(self.manifest.layout.parity_shards);
        let stripe_width = data_shards
            .checked_add(parity_shards)
            .ok_or(V2ChunkError::InvalidErasureLayout)?;
        if stripe_width != self.stripe_width
            || stripe_width == 0
            || !self.chunks.len().is_multiple_of(stripe_width)
        {
            return Err(V2ChunkError::InvalidErasureLayout);
        }
        let chunk_size = usize::try_from(self.manifest.layout.chunk_size_bytes)
            .map_err(|_| V2ChunkError::InvalidChunkLength)?;
        if !chunk_size.is_multiple_of(2) {
            return Err(V2ChunkError::InvalidErasureLayout);
        }
        let symbol_count = chunk_size / 2;
        let payload_size = usize::try_from(self.manifest.payload_size_bytes)
            .map_err(|_| V2ChunkError::PayloadTooLarge)?;
        #[cfg(test)]
        self.payload_allocation_attempts
            .fetch_add(1, Ordering::Relaxed);
        let mut payload = Vec::with_capacity(payload_size);
        for stripe in self.chunks.chunks_exact(stripe_width) {
            if stripe.iter().take(data_shards).all(Option::is_some) {
                for shard in stripe.iter().take(data_shards) {
                    payload.extend_from_slice(
                        shard.as_deref().ok_or(V2ChunkError::ReconstructionFailed)?,
                    );
                }
                continue;
            }
            let mut symbols = stripe
                .iter()
                .map(|chunk| {
                    chunk
                        .as_deref()
                        .map(|bytes| rs16::symbols_from_chunk(symbol_count, bytes))
                })
                .collect::<Vec<_>>();
            rs16::reconstruct_shards(&mut symbols, data_shards, parity_shards)
                .map_err(|_| V2ChunkError::ReconstructionFailed)?;
            for shard in symbols.iter().take(data_shards) {
                let bytes = rs16::chunk_from_symbols(
                    shard.as_ref().ok_or(V2ChunkError::ReconstructionFailed)?,
                    chunk_size,
                )
                .map_err(|_| V2ChunkError::ReconstructionFailed)?;
                payload.extend_from_slice(&bytes);
            }
        }
        payload.truncate(payload_size);
        Ok(payload)
    }
}
/// Encode exact canonical payload bytes using the height-frozen DA layout.
pub(crate) fn encode_payload(
    context: &wire::HeightContext,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    payload: &[u8],
) -> Result<EncodedV2Payload, V2ChunkError> {
    context.validate()?;
    if round.context_id != context.id()
        || round.height != context.height
        || Hash::new(payload) != subject.payload_hash
    {
        return Err(V2ChunkError::PayloadMismatch);
    }
    let payload_len = u64::try_from(payload.len()).map_err(|_| V2ChunkError::PayloadTooLarge)?;
    if payload.is_empty() || payload_len > context.da_layout.max_payload_size_bytes {
        return Err(V2ChunkError::PayloadTooLarge);
    }
    let chunks = wire::encode_payload_chunks(context.da_layout, payload)?;
    let manifest = wire::PayloadManifest::derive(context, round, subject, payload_len, &chunks)?;
    Ok(EncodedV2Payload { manifest, chunks })
}
/// Deterministic chunk encoding, buffering, or reconstruction failure.
#[derive(Debug, Error)]
pub(crate) enum V2ChunkError {
    /// Manifest or height context failed canonical structural validation.
    #[error(transparent)]
    Wire(#[from] wire::ValidationError),
    /// A payload or reconstructed body does not match its subject.
    #[error("Sumeragi v2 payload bytes do not match the manifest subject")]
    PayloadMismatch,
    /// Payload length is zero, over the height limit, or not representable.
    #[error("Sumeragi v2 payload length is outside the height limits")]
    PayloadTooLarge,
    /// Chunk referenced another manifest.
    #[error("Sumeragi v2 chunk references another manifest")]
    ManifestMismatch,
    /// Chunk index is outside the committed sequence.
    #[error("Sumeragi v2 chunk index is outside the manifest")]
    ChunkIndexOutOfRange,
    /// Chunk length differs from the layout-defined exact length.
    #[error("Sumeragi v2 chunk has an invalid length")]
    InvalidChunkLength,
    /// Chunk bytes do not match their committed hash.
    #[error("Sumeragi v2 chunk hash mismatch")]
    ChunkHashMismatch,
    /// An occupied session slot already contains different bytes.
    #[error("Sumeragi v2 chunk conflicts with an existing buffered chunk")]
    ConflictingChunk,
    /// RS16 layout arithmetic or profile is invalid.
    #[error("invalid Sumeragi v2 RS16 layout")]
    InvalidErasureLayout,
    /// Enough shards existed but deterministic RS16 recovery failed.
    #[error("Sumeragi v2 RS16 reconstruction failed")]
    ReconstructionFailed,
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{NetworkId, block::BlockHeader, peer::PeerId};
    fn test_network_id() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([0x92; Hash::LENGTH]),
        ))
    }
    fn context() -> wire::HeightContext {
        let mut roster = (1_u8..=4)
            .map(|seed| {
                let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .expect("deterministic key");
                wire::ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                }
            })
            .collect::<Vec<_>>();
        roster.sort_by(|left, right| left.validator.cmp(&right.validator));
        wire::HeightContext {
            network_id: test_network_id(),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 2,
            epoch: 0,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: Some(parent_qc(&roster)),
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"nexus amx context"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 8,
                data_shards: 3,
                parity_shards: 2,
                max_payload_size_bytes: 1024,
                max_chunk_count: 256,
            },
            leader_seed: [0x55; 32],
        }
    }
    fn parent_qc(roster: &[wire::ValidatorPower]) -> wire::QuorumCertificate {
        let parent_context = wire::HeightContext {
            network_id: test_network_id(),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(roster).expect("quorum"),
            roster: roster.to_vec(),
            nexus_amx_context_hash: Hash::new(b"nexus amx context"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 8,
                data_shards: 3,
                parity_shards: 2,
                max_payload_size_bytes: 1024,
                max_chunk_count: 256,
            },
            leader_seed: [0x55; 32],
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"parent block")),
            payload_hash: Hash::new(b"parent payload"),
        };
        wire::QuorumCertificate {
            round: wire::ConsensusRound {
                context_id: parent_context.id(),
                height: 1,
                view: 0,
            },
            proposal_round: wire::ConsensusRound {
                context_id: parent_context.id(),
                height: 1,
                view: 0,
            },
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"chunk fixture parent state"),
                Hash::new(b"chunk fixture post state"),
                Hash::new(b"chunk fixture ordinary writes"),
                1,
                Hash::new(b"chunk fixture executed block wire"),
            ),
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xA5; 48],
        }
    }
    fn encode_fixture(payload: &[u8]) -> (wire::HeightContext, EncodedV2Payload) {
        let context = context();
        let subject = wire::BlockSubject {
            parent_block_hash: context
                .parent_commit_qc
                .as_ref()
                .map(|qc| qc.subject.block_hash),
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block")),
            payload_hash: Hash::new(payload),
        };
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 4,
        };
        let encoded = encode_payload(&context, round, subject, payload).expect("encode payload");
        (context, encoded)
    }
    #[test]
    fn rs16_zero_data_or_parity_shards_are_rejected() {
        let payload = b"invalid RS16 layout";
        for (data_shards, parity_shards) in [(0, 2), (3, 0)] {
            let mut context = context();
            context.da_layout.data_shards = data_shards;
            context.da_layout.parity_shards = parity_shards;
            let subject = wire::BlockSubject {
                parent_block_hash: context
                    .parent_commit_qc
                    .as_ref()
                    .map(|qc| qc.subject.block_hash),
                block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block")),
                payload_hash: Hash::new(payload),
            };
            let round = wire::ConsensusRound {
                context_id: context.id(),
                height: context.height,
                view: 4,
            };
            assert!(matches!(
                encode_payload(&context, round, subject, payload),
                Err(V2ChunkError::Wire(
                    wire::ValidationError::InvalidDataAvailabilityLayout
                ))
            ));
        }
    }
    #[test]
    fn rs16_reconstructs_directly_from_complete_data_stripes() {
        let payload = b"RS16 data-only fast path spanning deterministic stripes";
        let (context, encoded) = encode_fixture(payload);
        let data_shards = usize::from(context.da_layout.data_shards);
        let width = usize::from(context.da_layout.data_shards + context.da_layout.parity_shards);
        let mut session =
            V2ChunkSession::open(&context, encoded.manifest.clone()).expect("open session");
        for (index, chunk) in encoded.chunks.iter().enumerate() {
            if index % width < data_shards {
                session
                    .admit_bytes(u32::try_from(index).expect("index"), chunk)
                    .expect("buffer data shard");
            }
        }
        assert_eq!(
            session.reconstruct().expect("reconstruct from data shards"),
            Some(payload.to_vec())
        );
        for index in 0..encoded.chunks.len() {
            assert_eq!(
                session.chunks[index].is_some(),
                index % width < data_shards,
                "only data shards should be buffered at index {index}"
            );
        }
    }
    #[test]
    fn rs16_recovers_missing_data_from_parity() {
        let payload = b"RS16 parity recovery spanning deterministic stripes";
        let (context, encoded) = encode_fixture(payload);
        let width = usize::from(context.da_layout.data_shards + context.da_layout.parity_shards);
        let mut session =
            V2ChunkSession::open(&context, encoded.manifest.clone()).expect("open session");
        for (index, chunk) in encoded.chunks.iter().enumerate() {
            let within = index % width;
            if within != 0 && within != width - 1 {
                session
                    .admit_bytes(u32::try_from(index).expect("index"), chunk)
                    .expect("buffer recovery shard");
            }
        }
        assert_eq!(
            session.reconstruct().expect("recover missing data shard"),
            Some(payload.to_vec())
        );
    }
    #[test]
    fn rs16_recovers_missing_data_from_any_quorum_per_stripe() {
        let payload = b"RS16 payload spanning more than one deterministic stripe";
        let (context, encoded) = encode_fixture(payload);
        let width = usize::from(context.da_layout.data_shards + context.da_layout.parity_shards);
        for first_missing in 0..width {
            for second_missing in first_missing + 1..width {
                let mut session =
                    V2ChunkSession::open(&context, encoded.manifest.clone()).expect("open session");
                for (index, chunk) in encoded.chunks.iter().enumerate() {
                    let within = index % width;
                    if within == first_missing || within == second_missing {
                        continue;
                    }
                    session
                        .admit_bytes(u32::try_from(index).expect("index"), chunk)
                        .expect("buffer shard");
                }
                assert_eq!(
                    session.reconstruct().expect("reconstruct"),
                    Some(payload.to_vec()),
                    "failed with missing shard positions {first_missing} and {second_missing}"
                );
            }
        }
    }
    #[test]
    fn parity_first_out_of_order_admission_becomes_ready_once() {
        let payload = b"RS16 parity-first payload spanning deterministic stripes";
        let (context, encoded) = encode_fixture(payload);
        let data_shards = usize::from(context.da_layout.data_shards);
        let stripe_width =
            usize::from(context.da_layout.data_shards + context.da_layout.parity_shards);
        let stripe_count = encoded.chunks.len() / stripe_width;
        assert!(stripe_count > 1, "fixture must span multiple stripes");

        let mut selected = Vec::with_capacity(stripe_count * data_shards);
        for stripe in (0..stripe_count).rev() {
            for within in (stripe_width - data_shards..stripe_width).rev() {
                selected.push(stripe * stripe_width + within);
            }
        }
        let final_index = selected.pop().expect("fixture has a final required shard");
        let mut session =
            V2ChunkSession::open(&context, encoded.manifest.clone()).expect("open session");
        for index in selected {
            session
                .admit_bytes(
                    u32::try_from(index).expect("chunk index"),
                    &encoded.chunks[index],
                )
                .expect("buffer out-of-order recovery shard");
        }
        assert_eq!(session.stripes_waiting, 1);
        assert_eq!(session.reconstruct().expect("reconstruction pending"), None);
        assert_eq!(session.payload_allocation_attempts(), 0);

        session
            .admit_bytes(
                u32::try_from(final_index).expect("final chunk index"),
                &encoded.chunks[final_index],
            )
            .expect("buffer final required recovery shard");
        assert_eq!(session.stripes_waiting, 0);
        assert_eq!(
            session.reconstruct().expect("reconstruct payload"),
            Some(payload.to_vec())
        );
        assert_eq!(session.payload_allocation_attempts(), 1);
    }
    #[test]
    fn incomplete_multi_stripe_session_returns_before_payload_allocation() {
        let payload = b"RS16 payload spanning more than one deterministic stripe";
        let (context, encoded) = encode_fixture(payload);
        let data_shards = usize::from(context.da_layout.data_shards);
        let stripe_width =
            usize::from(context.da_layout.data_shards + context.da_layout.parity_shards);
        assert!(
            encoded.chunks.len() > stripe_width,
            "fixture must span more than one stripe"
        );
        let mut session =
            V2ChunkSession::open(&context, encoded.manifest.clone()).expect("open session");
        assert_eq!(session.stripes_waiting, encoded.chunks.len() / stripe_width);
        for (index, chunk) in encoded.chunks.iter().take(data_shards).enumerate() {
            session
                .admit_bytes(u32::try_from(index).expect("index"), chunk)
                .expect("buffer complete first stripe data");
        }
        assert_eq!(
            session.stripes_waiting,
            encoded.chunks.len() / stripe_width - 1,
            "one complete stripe must leave only the later stripes pending"
        );
        assert_eq!(
            session
                .admit_bytes(0, &encoded.chunks[0])
                .expect("accept duplicate data shard"),
            ChunkAdmission::Duplicate
        );
        assert_eq!(
            session.stripes_waiting,
            encoded.chunks.len() / stripe_width - 1,
            "a duplicate must not advance stripe readiness"
        );
        assert_eq!(session.reconstruction_attempts(), 0);
        assert_eq!(session.payload_allocation_attempts(), 0);
        assert_eq!(session.reconstruct().expect("pending reconstruction"), None);
        assert_eq!(session.reconstruction_attempts(), 1);
        assert_eq!(
            session.payload_allocation_attempts(),
            0,
            "an incomplete later stripe must be found before payload allocation"
        );
    }
    #[test]
    fn corruption_duplicates_and_insufficient_shards_are_rejected_or_pending() {
        let payload = b"adversarial chunk payload";
        let (context, encoded) = encode_fixture(payload);
        let mut session =
            V2ChunkSession::open(&context, encoded.manifest.clone()).expect("open session");
        let mut corrupt = encoded.chunks[0].clone();
        corrupt[0] ^= 0x80;
        assert!(matches!(
            session.admit_bytes(0, &corrupt),
            Err(V2ChunkError::ChunkHashMismatch)
        ));
        assert_eq!(
            session
                .admit_bytes(0, &encoded.chunks[0])
                .expect("buffer canonical chunk"),
            ChunkAdmission::Buffered
        );
        assert_eq!(
            session
                .admit_bytes(0, &encoded.chunks[0])
                .expect("accept exact duplicate"),
            ChunkAdmission::Duplicate
        );
        let stripes_waiting = session.stripes_waiting;
        assert!(matches!(
            session.admit_bytes(0, &corrupt),
            Err(V2ChunkError::ChunkHashMismatch)
        ));
        assert_eq!(session.stripes_waiting, stripes_waiting);
        assert_eq!(session.reconstruct().expect("pending reconstruction"), None);
    }
    #[test]
    fn partial_chunks_are_volatile_across_sessions() {
        let payload = b"volatile bounded chunk acquisition";
        let (context, encoded) = encode_fixture(payload);
        let mut session = V2ChunkSession::open(&context, encoded.manifest.clone())
            .expect("open volatile session");
        assert_eq!(
            session
                .admit_bytes(0, &encoded.chunks[0])
                .expect("buffer one shard"),
            ChunkAdmission::Buffered
        );
        drop(session);
        let restarted = V2ChunkSession::open(&context, encoded.manifest)
            .expect("restart with an empty volatile session");
        assert!(restarted.chunks.iter().all(Option::is_none));
        assert_eq!(
            restarted.reconstruct().expect("reconstruction pending"),
            None
        );
    }
    #[test]
    fn encoding_is_deterministic_and_subject_bound() {
        let payload = b"same payload";
        let (context, first) = encode_fixture(payload);
        let second = encode_payload(
            &context,
            first.manifest.round,
            first.manifest.subject,
            payload,
        )
        .expect("repeat encoding");
        assert_eq!(first, second);
        let mut wrong = first.manifest.subject;
        wrong.payload_hash = Hash::new(b"another payload");
        assert!(matches!(
            encode_payload(&context, first.manifest.round, wrong, payload),
            Err(V2ChunkError::PayloadMismatch)
        ));
    }
}
