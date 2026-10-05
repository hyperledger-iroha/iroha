//! Original aggregate-only checkpoint custody, independent of the three DKG phase banks.
//!
//! Raw bindings authenticate ciphertext context only. The caller must verify the
//! original native finalized session, durable intent/head chain and frozen expiry.
//! This primitive grants neither protocol authority nor runtime signing capability.

use super::checkpoint::{DkgCheckpointBindingV1, DkgCheckpointErrorV1, DkgCheckpointSourceV1};
use super::*;
use crate::{KeyPair, PrivateKey, PrivateKeyInner, secrecy::ExposeSecret as _};
use iroha_allocation::{AllocationBudget, AllocationRefusal, ChargedBuffer};
use norito::core::{
    CanonicalField, DecodeField, DecodeIntoError, Encoder, FieldDestination,
    PreparedDecodeWorkspace, PreparedRecordDestination, SerializePayload,
};
use norito::{NoritoDeserialize, NoritoSchema, NoritoSerialize};
use std::{alloc::Layout, convert::Infallible, io::Write};

const DOMAIN: &[u8] = b"iroha.dkg.local-aggregate-checkpoint.v1\0";

/// Exact associated data for the original finalized aggregate, without protocol authority.
///
/// The intent hash authenticates the original same-boot expiry, held claim and
/// source/FIFO identities. Its durable verification remains the caller's duty;
/// a local AEAD/head cannot detect deletion of an entire later suffix.
#[derive(Clone, Copy, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito(decode_fields)]
#[norito_schema(
    name = "iroha_crypto::threshold_bls::aggregate_checkpoint::DkgAggregateCheckpointBindingV1"
)]
pub struct DkgAggregateCheckpointBindingV1 {
    /// Original network pin.
    pub network_id: [u8; 32],
    /// Original authenticated attempt identity.
    pub attempt_id: [u8; 32],
    /// Frozen authority generation.
    pub authority_generation: u64,
    /// Original session identity.
    pub session_id: [u8; 32],
    /// Complete ordered roster digest.
    pub roster_hash: [u8; 32],
    /// Exact one-based recipient seat.
    pub seat_index: u16,
    /// Original lifecycle public-key digest.
    pub lifecycle_key_hash: [u8; 32],
    /// Original provider handle digest.
    pub provider_handle_hash: [u8; 32],
    /// Original provider revision.
    pub provider_revision: u64,
    /// Frozen session start.
    pub start_height: u64,
    /// Frozen commitments cutoff.
    pub commitments_end_height: u64,
    /// Frozen deliveries cutoff.
    pub deliveries_end_height: u64,
    /// Frozen acceptances cutoff.
    pub acceptances_end_height: u64,
    /// Actual finalization height, at or after the acceptances cutoff.
    pub finalized_at_height: u64,
    /// Exact executed native tip; signed genesis does not authorize an aggregate.
    pub source: DkgCheckpointSourceV1,
    /// Original wider authenticated attempt cutoff.
    pub cutoff_height: u64,
    /// Digest of the complete canonical finalized public session.
    pub public_session_hash: [u8; 32],
    /// Complete verified crypto transcript digest.
    pub transcript_hash: [u8; 32],
    /// Original encrypted accepted checkpoint, never a replacement contribution set.
    pub accepted_checkpoint_hash: [u8; 32],
    /// Original complete accepted durable head.
    pub accepted_head_hash: [u8; 32],
    /// Original extraction intent including claim, source, generations and frozen expiry.
    pub extraction_intent_hash: [u8; 32],
}

impl DkgAggregateCheckpointBindingV1 {
    /// Hash the full canonical public source without an intermediate encoded frame.
    ///
    /// # Errors
    /// Retains the original canonical serializer refusal.
    pub fn public_session_digest<T: NoritoSerialize>(value: &T) -> Result<[u8; 32], norito::Error> {
        let mut writer = HashWriter(Sha256::new());
        writer
            .0
            .update(b"iroha.dkg.local-aggregate.public-session.v1\0");
        norito::core::write_canonical_to_writer(value, &mut writer)?;
        Ok(writer.0.finalize().into())
    }
}

#[derive(NoritoSerialize, NoritoDeserialize, NoritoSchema)]
#[norito(decode_fields)]
#[norito_schema(
    name = "iroha_crypto::threshold_bls::aggregate_checkpoint::DkgAggregatePrivateRecordV1"
)]
struct AggregateRecord {
    version: u16,
    purpose: u8,
    parameters_digest: [u8; 32],
    seat_index: u16,
    components: [u8; 96],
}
impl AggregateRecord {
    fn empty() -> Self {
        Self {
            version: 0,
            purpose: 0,
            parameters_digest: [0; 32],
            seat_index: 0,
            components: [0; 96],
        }
    }
}
impl Drop for AggregateRecord {
    fn drop(&mut self) {
        self.components.zeroize();
    }
}
struct Destination {
    version: u16,
    purpose: u8,
    parameters_digest: [u8; 32],
    seat_index: u16,
    components: Option<ChargedBuffer<Zeroizing<[[u8; 32]; 3]>>>,
}
impl Destination {
    fn erase(&mut self) {
        self.version = 0;
        self.purpose = 0;
        self.parameters_digest = [0; 32];
        self.seat_index = 0;
        if let Some(parts) = self.components.as_mut() {
            parts.as_mut_slice()[0].zeroize();
        }
    }
    fn record(&self) -> AggregateRecord {
        let mut record = AggregateRecord {
            version: self.version,
            purpose: self.purpose,
            parameters_digest: self.parameters_digest,
            seat_index: self.seat_index,
            components: [0; 96],
        };
        if let Some(parts) = &self.components {
            for (out, part) in record
                .components
                .chunks_exact_mut(32)
                .zip(parts.as_slice()[0].iter())
            {
                out.copy_from_slice(part);
            }
        }
        record
    }
}
impl FieldDestination for Destination {
    type Error = Infallible;
}
macro_rules! scalar_field {
    ($index:literal, $ty:ty, $field:ident) => {
        impl DecodeField<$index, $ty> for Destination {
            type Value = ();
            fn decode_field(
                &mut self,
                field: CanonicalField<'_, $ty>,
            ) -> Result<(), DecodeIntoError<Infallible>> {
                self.$field = field.with_payload(|bytes| {
                    let (value, used) =
                        <$ty as norito::core::DecodeFromSlice>::decode_from_slice(bytes)?;
                    if used != bytes.len() {
                        return Err(norito::Error::LengthMismatch.into());
                    }
                    Ok(value)
                })?;
                Ok(())
            }
        }
    };
}
scalar_field!(0, u16, version);
scalar_field!(1, u8, purpose);
scalar_field!(3, u16, seat_index);
impl DecodeField<2, [u8; 32]> for Destination {
    type Value = ();
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, [u8; 32]>,
    ) -> Result<(), DecodeIntoError<Infallible>> {
        field.with_payload(|bytes| {
            if bytes.len() != 32 {
                return Err(norito::Error::LengthMismatch.into());
            }
            self.parameters_digest.copy_from_slice(bytes);
            Ok(())
        })
    }
}
impl DecodeField<4, [u8; 96]> for Destination {
    type Value = ();
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, [u8; 96]>,
    ) -> Result<(), DecodeIntoError<Infallible>> {
        field.with_payload(|bytes| {
            if bytes.len() != 96 {
                return Err(norito::Error::LengthMismatch.into());
            }
            let parts = self
                .components
                .as_mut()
                .expect("single-use prepared original scalar leaf");
            for (out, part) in parts.as_mut_slice()[0]
                .iter_mut()
                .zip(bytes.chunks_exact(32))
            {
                out.copy_from_slice(part);
            }
            Ok(())
        })
    }
}
impl SerializePayload for Destination {
    fn serialize(&self, encoder: &mut Encoder<'_>) -> Result<(), norito::Error> {
        self.record().serialize(encoder)
    }
}
impl PreparedRecordDestination<AggregateRecord> for Destination {
    fn reset(&mut self) {
        self.erase();
    }
}
struct SecretBytes(ChargedBuffer<u8>);
impl Drop for SecretBytes {
    fn drop(&mut self) {
        self.0.as_mut_slice().zeroize();
    }
}
struct HashWriter(Sha256);
impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Move-only original 96-byte physical scalar owner, verified against its public transcript.
///
/// It cannot clone, grow, serialize or expose an owned component copy. Its real
/// zeroizing leaf remains charged to the original pool through borrowed export.
pub struct RetainedDkgAggregateV1<P: ThresholdBlsPurpose> {
    components: ChargedBuffer<Zeroizing<[[u8; 32]; 3]>>,
    session_id: [u8; 32],
    transcript_hash: [u8; 32],
    seat_index: u16,
    marker: PhantomData<P>,
}
impl<P: ThresholdBlsPurpose> RetainedDkgAggregateV1<P> {
    /// Borrow actual original components only for immediate supervisor credential custody.
    #[must_use]
    pub fn components_for_runtime_custody(&self) -> &[[u8; 32]; 3] {
        &self.components.as_slice()[0]
    }
    /// Exact physical scalar allocation retained by this original owner.
    #[must_use]
    pub fn original_backing_bytes(&self) -> usize {
        self.components.capacity() * std::mem::size_of::<Zeroizing<[[u8; 32]; 3]>>()
    }
    /// Exact one-based seat carried by this private owner.
    #[must_use]
    pub const fn seat_index(&self) -> u16 {
        self.seat_index
    }
    /// Exact original session identity.
    #[must_use]
    pub const fn session_id(&self) -> &[u8; 32] {
        &self.session_id
    }
    /// Exact original public transcript digest.
    #[must_use]
    pub const fn transcript_hash(&self) -> &[u8; 32] {
        &self.transcript_hash
    }
    /// Verify that the real scalar backing retains the original operation pool.
    #[must_use]
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.components.belongs_to(budget)
    }
}

/// Prepaid aggregate-only original ciphertext, work, scalar leaf and canonical decode controls.
///
/// Preparation has no secret/RNG side effect. Production aggregates and seals
/// once; restoration only decrypts and checks the original canonical record and
/// public commitment. A used bank never resets into another phase or source.
pub struct PreparedDkgAggregateCheckpointV1<P: ThresholdBlsPurpose> {
    source: SecretBytes,
    work: SecretBytes,
    destination: Destination,
    workspace: PreparedDecodeWorkspace,
    parameters: AdaptiveThresholdBlsParameters<P>,
    seat_index: u16,
    binding: Option<[u8; 32]>,
    source_hash: Option<[u8; 32]>,
    transcript_hash: Option<[u8; 32]>,
    produced: bool,
    sealed: bool,
    taken: bool,
}
impl<P: ThresholdBlsPurpose> PreparedDkgAggregateCheckpointV1<P> {
    /// Exact five physical layouts admitted before claim, aggregation or randomness.
    ///
    /// # Errors
    /// Retains canonical geometry and original checked layout-overflow causes.
    pub fn allocation_layouts() -> Result<[Layout; 5], DkgCheckpointErrorV1> {
        let len = norito::canonical_frame_len(&AggregateRecord::empty())?
            .checked_add(28)
            .ok_or(AllocationRefusal::DemandOverflow)?;
        let envelope = Layout::array::<u8>(len).map_err(|_| AllocationRefusal::DemandOverflow)?;
        let controls = PreparedDecodeWorkspace::allocation_layouts();
        Ok([
            envelope,
            envelope,
            Layout::array::<Zeroizing<[[u8; 32]; 3]>>(1)
                .map_err(|_| AllocationRefusal::DemandOverflow)?,
            controls[0],
            controls[1],
        ])
    }
    /// Prepare the complete fixed original graph from one finite operation pool.
    ///
    /// # Errors
    /// Refuses invalid seat, original capacity, actual allocator or scope-control construction.
    pub fn new(
        parameters: &AdaptiveThresholdBlsParameters<P>,
        seat_index: u16,
        budget: &AllocationBudget,
    ) -> Result<Self, DkgCheckpointErrorV1> {
        validate_participant_index(parameters.session(), seat_index)?;
        let layouts = Self::allocation_layouts()?;
        let len = layouts[0].size();
        let mut reservation = budget.try_reserve_layouts(layouts)?;
        let mut source = ChargedBuffer::from_reservation(len, &mut reservation)?;
        let mut work = ChargedBuffer::from_reservation(len, &mut reservation)?;
        for _ in 0..len {
            source.push_reserved(0);
            work.push_reserved(0);
        }
        let mut components = ChargedBuffer::from_reservation(1, &mut reservation)?;
        components.push_reserved(Zeroizing::new([[0; 32]; 3]));
        let workspace = PreparedDecodeWorkspace::from_reservation(budget, &mut reservation)?;
        if reservation.remaining_bytes() != 0 {
            return Err(DkgCheckpointErrorV1::Binding);
        }
        Ok(Self {
            source: SecretBytes(source),
            work: SecretBytes(work),
            destination: Destination {
                version: 0,
                purpose: 0,
                parameters_digest: [0; 32],
                seat_index: 0,
                components: Some(components),
            },
            workspace,
            parameters: *parameters,
            seat_index,
            binding: None,
            source_hash: None,
            transcript_hash: None,
            produced: false,
            sealed: false,
            taken: false,
        })
    }
    /// Fixed original ciphertext extent.
    #[must_use]
    pub fn encrypted_record_capacity(&self) -> usize {
        self.source.0.capacity()
    }
    /// Whether every surviving real backing retains the original operation pool.
    #[must_use]
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.source.0.belongs_to(budget)
            && self.work.0.belongs_to(budget)
            && self.workspace.belongs_to(budget)
            && self
                .destination
                .components
                .as_ref()
                .is_none_or(|parts| parts.belongs_to(budget))
    }
    /// Sum actual surviving private allocation layouts for ownership controls only.
    #[must_use]
    pub fn original_backing_bytes(&self) -> usize {
        self.source.0.capacity()
            + self.work.0.capacity()
            + self.destination.components.as_ref().map_or(0, |parts| {
                parts.capacity() * std::mem::size_of::<Zeroizing<[[u8; 32]; 3]>>()
            })
            + PreparedDecodeWorkspace::allocation_layouts()
                .iter()
                .map(Layout::size)
                .sum::<usize>()
    }

    fn context(
        &self,
        binding: &DkgAggregateCheckpointBindingV1,
        lifecycle: &KeyPair,
        transcript: &AdaptiveThresholdBlsPublicTranscript<P>,
    ) -> Result<[u8; 32], DkgCheckpointErrorV1> {
        let source_matches = match binding.source {
            DkgCheckpointSourceV1::ExecutedNativeTip {
                height,
                block_hash,
                core_hash,
                result_hash,
            } => {
                height == binding.finalized_at_height
                    && height >= 2
                    && !is_zero(&block_hash)
                    && !is_zero(&core_hash)
                    && !is_zero(&result_hash)
            }
            DkgCheckpointSourceV1::SignedGenesisAuthorization { .. } => false,
        };
        if binding.network_id != *self.parameters.session().network_id()
            || binding.session_id != *self.parameters.session().session_id()
            || binding.roster_hash != *self.parameters.session().roster_hash()
            || binding.seat_index != self.seat_index
            || transcript.parameters() != &self.parameters
            || binding.transcript_hash != *transcript.transcript_hash()
            || binding.lifecycle_key_hash
                != DkgCheckpointBindingV1::lifecycle_key_digest(lifecycle.public_key())?
            || lifecycle.public_key().algorithm() != crate::Algorithm::BlsNormal
            || !source_matches
            || binding.start_height == 0
            || binding.start_height >= binding.commitments_end_height
            || binding.commitments_end_height >= binding.deliveries_end_height
            || binding.deliveries_end_height >= binding.acceptances_end_height
            || binding.finalized_at_height < binding.acceptances_end_height
            || binding.finalized_at_height >= binding.cutoff_height
            || binding.provider_revision == 0
            || [
                binding.attempt_id,
                binding.provider_handle_hash,
                binding.public_session_hash,
                binding.transcript_hash,
                binding.accepted_checkpoint_hash,
                binding.accepted_head_hash,
                binding.extraction_intent_hash,
            ]
            .iter()
            .any(|value| is_zero(value))
        {
            return Err(DkgCheckpointErrorV1::Binding);
        }
        let mut writer = HashWriter(Sha256::new());
        writer.0.update(DOMAIN);
        writer.0.update([P::ROLE_TAG]);
        norito::core::write_canonical_to_writer(binding, &mut writer)?;
        Ok(writer.0.finalize().into())
    }
    fn cipher(
        lifecycle: &PrivateKey,
        digest: &[u8; 32],
    ) -> Result<SymmetricEncryptor<ChaCha20Poly1305>, DkgCheckpointErrorV1> {
        let PrivateKeyInner::BlsNormal(secret) = lifecycle.0.expose_secret() else {
            return Err(DkgCheckpointErrorV1::Binding);
        };
        let hkdf = Hkdf::<Sha256>::new(Some(DOMAIN), secret.as_bytes());
        let mut key = Zeroizing::new([0; 32]);
        hkdf.expand(digest, &mut *key)
            .map_err(|_| ThresholdBlsError::HkdfExpand)?;
        Ok(SymmetricEncryptor::new_with_key(&key[..])?)
    }
    /// Aggregate the actual original complete qualified contributions and seal once.
    ///
    /// A refusal before production preserves the empty preparer. Once the
    /// aggregate is produced, it stays in its original leaf even on encryption
    /// failure; another call cannot sum contributions or sample another nonce.
    /// Successful publication retries borrow the identical original ciphertext.
    ///
    /// # Errors
    /// Retains original context, canonical scalar/equation, encoding and entropy causes.
    pub fn produce_original(
        &mut self,
        binding: &DkgAggregateCheckpointBindingV1,
        lifecycle: &KeyPair,
        transcript: &AdaptiveThresholdBlsPublicTranscript<P>,
        producer: impl FnOnce() -> Result<AdaptiveThresholdBlsSecretShare<P>, ThresholdBlsError>,
    ) -> Result<&[u8], DkgCheckpointErrorV1> {
        let digest = self.context(binding, lifecycle, transcript)?;
        if self.sealed {
            return self.encrypted_record_for(binding, lifecycle, transcript);
        }
        if self.produced || self.binding.is_some() || self.taken {
            return Err(DkgCheckpointErrorV1::Terminal);
        }
        // Context/capacity refusal is still pre-production. Crossing this point
        // consumes the one original producer, including an intrinsic equation
        // failure; no retry may aggregate or invoke that producer again.
        self.produced = true;
        self.binding = Some(digest);
        self.transcript_hash = Some(binding.transcript_hash);
        let aggregate = producer()?;
        if aggregate.index() != self.seat_index
            || aggregate.session_id != binding.session_id
            || aggregate.transcript_hash != binding.transcript_hash
        {
            return Err(DkgCheckpointErrorV1::Binding);
        }
        let mut components = aggregate.into_components_for_runtime_custody();
        self.destination
            .components
            .as_mut()
            .expect("original prepared scalar leaf")
            .as_mut_slice()[0] = Zeroizing::new(std::mem::take(&mut *components));
        self.destination.version = 1;
        self.destination.purpose = P::ROLE_TAG;
        self.destination.parameters_digest = self.parameters.digest();
        self.destination.seat_index = self.seat_index;
        let len = self.work.0.as_slice().len();
        let result = (|| {
            let record = self.destination.record();
            let mut output = &mut self.work.0.as_mut_slice()[12..len - 16];
            norito::core::write_canonical_to_writer(&record, &mut output)?;
            if !output.is_empty() {
                return Err(DkgCheckpointErrorV1::Encoding(
                    norito::Error::LengthMismatch,
                ));
            }
            Self::cipher(lifecycle.private_key(), &digest)?
                .encrypt_easy_in_place(&digest[..], self.work.0.as_mut_slice())?;
            self.source
                .0
                .as_mut_slice()
                .copy_from_slice(self.work.0.as_slice());
            Ok(())
        })();
        self.work.0.as_mut_slice().zeroize();
        result?;
        self.source_hash = Some(Sha256::digest(self.source.0.as_slice()).into());
        self.sealed = true;
        Ok(self.source.0.as_slice())
    }
    /// Borrow the exact immutable sealed ciphertext, including after scalar handoff.
    #[must_use]
    pub fn encrypted_record(&self) -> Option<&[u8]> {
        self.sealed.then(|| self.source.0.as_slice())
    }
    /// Borrow the original encrypted source retained after a failed restore.
    ///
    /// These immutable ciphertext bytes establish custody only. Until restore
    /// succeeds, this method grants neither canonical validity nor authority.
    #[must_use]
    pub fn original_encrypted_source(&self) -> Option<&[u8]> {
        self.source_hash.map(|_| self.source.0.as_slice())
    }

    /// Borrow the original cipher only under identical complete binding and signer.
    ///
    /// # Errors
    /// Refuses an unsealed or changed original producer/context without RNG.
    pub fn encrypted_record_for(
        &self,
        binding: &DkgAggregateCheckpointBindingV1,
        lifecycle: &KeyPair,
        transcript: &AdaptiveThresholdBlsPublicTranscript<P>,
    ) -> Result<&[u8], DkgCheckpointErrorV1> {
        if self.binding != Some(self.context(binding, lifecycle, transcript)?) {
            return Err(DkgCheckpointErrorV1::Binding);
        }
        self.encrypted_record()
            .ok_or(DkgCheckpointErrorV1::Terminal)
    }
    /// Restore complete original ciphertext without RNG, signing or re-aggregation.
    ///
    /// The first offered context/source stays pinned through canonical enclosing
    /// refusal. Every failure wipes actual plaintext; retries admit no new owner.
    /// A changed source cannot replace that prefix, and a successful bank cannot
    /// be reset/reused even before its one-time scalar take.
    ///
    /// # Errors
    /// Preserves exact AEAD/canonical/equation causes and refuses changed bindings.
    pub fn restore(
        &mut self,
        encrypted: &[u8],
        binding: &DkgAggregateCheckpointBindingV1,
        lifecycle: &KeyPair,
        transcript: &AdaptiveThresholdBlsPublicTranscript<P>,
        limits: norito::DecodeLimits,
    ) -> Result<(), DkgCheckpointErrorV1> {
        if self.produced || self.sealed || self.taken {
            return Err(DkgCheckpointErrorV1::Terminal);
        }
        self.destination.erase();
        self.work.0.as_mut_slice().zeroize();
        let digest = self.context(binding, lifecycle, transcript)?;
        if encrypted.len() != self.source.0.as_slice().len() {
            return Err(DkgCheckpointErrorV1::Binding);
        }
        let hash = Sha256::digest(encrypted).into();
        if self.binding.is_some_and(|old| old != digest)
            || self.source_hash.is_some_and(|old| old != hash)
        {
            return Err(DkgCheckpointErrorV1::Binding);
        }
        self.binding = Some(digest);
        self.source_hash = Some(hash);
        self.source.0.as_mut_slice().copy_from_slice(encrypted);
        self.work
            .0
            .as_mut_slice()
            .copy_from_slice(self.source.0.as_slice());
        let result = (|| {
            let plaintext = Self::cipher(lifecycle.private_key(), &digest)?
                .decrypt_easy_in_place(&digest[..], self.work.0.as_mut_slice())?;
            self.workspace.decode_canonical_into::<AggregateRecord, _>(
                plaintext,
                limits,
                &mut self.destination,
            )?;
            if self.destination.version != 1
                || self.destination.purpose != P::ROLE_TAG
                || self.destination.parameters_digest != self.parameters.digest()
                || self.destination.seat_index != self.seat_index
            {
                return Err(DkgCheckpointErrorV1::Binding);
            }
            let parts = &self
                .destination
                .components
                .as_ref()
                .expect("original prepared scalar leaf")
                .as_slice()[0];
            AdaptiveThresholdBlsSecretShare::from_components(
                transcript,
                self.seat_index,
                parts[0],
                parts[1],
                parts[2],
            )?;
            Ok(())
        })();
        self.work.0.as_mut_slice().zeroize();
        if result.is_err() {
            self.destination.erase();
        } else {
            self.sealed = true;
            self.transcript_hash = Some(binding.transcript_hash);
        }
        result
    }
    /// Move the actual original 96-byte allocation once while retaining ciphertext custody.
    ///
    /// # Errors
    /// Rejects unfinished or repeated extraction; no partial scalar owner escapes.
    pub fn take_original_aggregate(
        &mut self,
    ) -> Result<RetainedDkgAggregateV1<P>, DkgCheckpointErrorV1> {
        if !self.sealed || self.taken {
            return Err(DkgCheckpointErrorV1::Terminal);
        }
        let components = self
            .destination
            .components
            .take()
            .ok_or(DkgCheckpointErrorV1::Terminal)?;
        self.taken = true;
        Ok(RetainedDkgAggregateV1 {
            components,
            session_id: *self.parameters.session().session_id(),
            transcript_hash: self.transcript_hash.expect("sealed complete transcript"),
            seat_index: self.seat_index,
            marker: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests;
