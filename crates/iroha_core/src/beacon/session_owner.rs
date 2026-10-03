//! Exact original-pool construction of the sole canonical public session graph.
//!
//! This module owns actual nested buffers, compact keys and signature bytes. It
//! neither grants transcript validity nor funds input decoding, validation scratch
//! or the eventual shared control. The authenticated session constructor must bind
//! this graph and its original ledger into that fully validated runtime owner.

use std::alloc::Layout;

use iroha_allocation::{
    AllocationBudget, AllocationCharge, AllocationRefusal, AllocationReservation, ChargedBuffer,
    InsufficientReservation, PrepaidBufferError, RetainedPayload, RetainedPayloadError,
};
use iroha_crypto::{PublicKey, PublicKeyAllocationError, Signature, SignatureAllocationError};
use iroha_data_model::consensus::{
    GlobalThresholdBeaconDkgDealerCommitmentV1, GlobalThresholdBeaconDkgEncryptedShareV1,
    GlobalThresholdBeaconDkgRecipientKeyV1, GlobalThresholdBeaconDkgShareAcceptanceV1,
    GlobalThresholdBeaconDkgTranscriptV1, GlobalThresholdBeaconKeySessionV1,
    GlobalThresholdBeaconPublicShareV1,
};
use iroha_model_base::peer::PeerId;

/// Concrete original source or physical construction failure; never an authentication result.
#[derive(Debug, thiserror::Error)]
pub(super) enum SessionGraphError {
    /// Original finite admission is retained without formatting or source substitution.
    #[error(transparent)]
    Admission(#[from] AllocationRefusal),
    /// Fixed backing or its exact prepaid partition could not be constructed.
    #[error(transparent)]
    Buffer(#[from] PrepaidBufferError),
    /// A checked construction layout exceeded its unchanged original remainder.
    #[error(transparent)]
    Reservation(#[from] InsufficientReservation),
    /// Canonical compact-key backing refused its exact original charge.
    #[error(transparent)]
    PublicKey(#[from] PublicKeyAllocationError),
    /// Canonical signature backing refused its exact original charge.
    #[error(transparent)]
    Signature(#[from] SignatureAllocationError),
    /// A sealed original payload/ledger source invariant failed.
    #[error(transparent)]
    Retention(#[from] RetainedPayloadError),
    /// A complete checked demand and materialization disagreed.
    #[error("canonical beacon session allocation plan changed")]
    PlanChanged,
}

#[derive(Default)]
struct Demand {
    bytes: usize,
    charges: usize,
}
impl Demand {
    fn add(&mut self, layout: Layout) -> Result<(), SessionGraphError> {
        self.bytes = self
            .bytes
            .checked_add(layout.size())
            .ok_or(AllocationRefusal::DemandOverflow)?;
        self.charges = self
            .charges
            .checked_add(1)
            .ok_or(AllocationRefusal::DemandOverflow)?;
        Ok(())
    }
    fn array<T>(&mut self, count: usize) -> Result<(), SessionGraphError> {
        self.add(Layout::array::<T>(count).map_err(|_| AllocationRefusal::DemandOverflow)?)
    }
    fn for_session(record: &GlobalThresholdBeaconKeySessionV1) -> Result<Self, SessionGraphError> {
        let mut demand = Self::default();
        let dkg = &record.adaptive_dkg;
        demand.array::<GlobalThresholdBeaconPublicShareV1>(record.public_shares.len())?;
        demand.array::<GlobalThresholdBeaconDkgDealerCommitmentV1>(dkg.dealer_commitments.len())?;
        for dealer in &dkg.dealer_commitments {
            demand.array::<[u8; 96]>(dealer.coefficient_commitments.len())?;
            demand.add(dealer.signature.retained_allocation_layout())?;
        }
        demand.array::<GlobalThresholdBeaconDkgRecipientKeyV1>(dkg.recipient_keys.len())?;
        for recipient in &dkg.recipient_keys {
            demand.add(
                recipient
                    .validator
                    .public_key()
                    .retained_allocation_layout(),
            )?;
            demand.array::<u8>(recipient.mlkem768_public_key.len())?;
            demand.add(recipient.signature.retained_allocation_layout())?;
        }
        demand.array::<GlobalThresholdBeaconDkgEncryptedShareV1>(dkg.encrypted_shares.len())?;
        for edge in &dkg.encrypted_shares {
            demand.array::<u8>(edge.mlkem768_ciphertext.len())?;
            demand.array::<u8>(edge.encrypted_share.len())?;
            demand.add(edge.signature.retained_allocation_layout())?;
        }
        demand.array::<GlobalThresholdBeaconDkgShareAcceptanceV1>(dkg.share_acceptances.len())?;
        for acceptance in &dkg.share_acceptances {
            demand.add(acceptance.signature.retained_allocation_layout())?;
        }
        demand.array::<u16>(dkg.qualified_dealers.len())?;
        Ok(demand)
    }
    fn total_bytes(&self) -> Result<usize, SessionGraphError> {
        let ledger = Layout::array::<AllocationCharge>(self.charges)
            .map_err(|_| AllocationRefusal::DemandOverflow)?;
        self.bytes
            .checked_add(ledger.size())
            .ok_or(AllocationRefusal::DemandOverflow.into())
    }
}

// Declared before every constructed payload local so their actual allocations
// retire before the ledger on normal refusal. Like the existing schedule owner,
// uncertain partial destruction during unwind conservatively retains its credits.
struct Construction<'a> {
    budget: &'a AllocationBudget,
    reservation: AllocationReservation,
    charges: Option<ChargedBuffer<AllocationCharge>>,
}
impl Drop for Construction<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            if let Some(charges) = self.charges.take() {
                std::mem::forget(charges);
            }
        }
    }
}
impl<'a> Construction<'a> {
    fn new(
        source: &GlobalThresholdBeaconKeySessionV1,
        budget: &'a AllocationBudget,
    ) -> Result<Self, SessionGraphError> {
        let demand = Demand::for_session(source)?;
        let mut reservation = budget.try_reserve_bytes(demand.total_bytes()?)?;
        let charges = ChargedBuffer::from_reservation(demand.charges, &mut reservation)?;
        Ok(Self {
            budget,
            reservation,
            charges: Some(charges),
        })
    }
    fn retain(&mut self, charge: AllocationCharge) -> Result<(), SessionGraphError> {
        if let Err(charge) = self
            .charges
            .as_mut()
            .expect("original construction ledger")
            .try_push(charge)
        {
            // The payload is already alive. Never turn an internal plan defect
            // into released capacity for an allocation not yet destroyed.
            std::mem::forget(charge);
            return Err(SessionGraphError::PlanChanged);
        }
        Ok(())
    }
    fn buffer<T>(&mut self, count: usize) -> Result<ChargedBuffer<T>, SessionGraphError> {
        Ok(ChargedBuffer::from_reservation(
            count,
            &mut self.reservation,
        )?)
    }
    #[allow(unsafe_code)]
    fn vector<T>(&mut self, values: ChargedBuffer<T>) -> Result<Vec<T>, SessionGraphError> {
        // SAFETY: exact backing moves into an immutable canonical field, with
        // every nested allocation separately retained in this same ledger.
        // No field can grow, mutate or escape the final RetainedPayload owner.
        let (values, charge) = unsafe { values.into_allocation_parts() };
        self.retain(charge)?;
        Ok(values)
    }
    fn copied<T: Copy>(&mut self, source: &[T]) -> Result<Vec<T>, SessionGraphError> {
        let mut values = self.buffer(source.len())?;
        for value in source {
            values.push_reserved(*value);
        }
        self.vector(values)
    }
    #[allow(unsafe_code)]
    fn key(&mut self, source: &PublicKey) -> Result<PublicKey, SessionGraphError> {
        let charge = self
            .reservation
            .try_split(source.retained_allocation_layout())?;
        let owned = match source.try_clone_from_charge(self.budget, charge) {
            Ok(owned) => owned,
            Err((charge, error)) => {
                drop(charge);
                return Err(error.into());
            }
        };
        // SAFETY: exact immutable compact bytes move directly to a canonical
        // recipient field; this original charge outlives that field allocation.
        let (key, charge) = unsafe { owned.into_allocation_parts() };
        self.retain(charge)?;
        Ok(key)
    }
    #[allow(unsafe_code)]
    fn signature(&mut self, source: &Signature) -> Result<Signature, SessionGraphError> {
        let charge = self
            .reservation
            .try_split(source.retained_allocation_layout())?;
        let owned = match source.try_clone_from_charge(self.budget, charge) {
            Ok(owned) => owned,
            Err((charge, error)) => {
                drop(charge);
                return Err(error.into());
            }
        };
        // SAFETY: transfer the same canonical payload without ordinary Clone,
        // mutation or export. Its charge stays in the complete original ledger.
        let (signature, charge) = unsafe { owned.into_allocation_parts() };
        self.retain(charge)?;
        Ok(signature)
    }
    fn dealers(
        &mut self,
        source: &[GlobalThresholdBeaconDkgDealerCommitmentV1],
    ) -> Result<Vec<GlobalThresholdBeaconDkgDealerCommitmentV1>, SessionGraphError> {
        let mut output = self.buffer(source.len())?;
        for source in source {
            let coefficient_commitments = self.copied(&source.coefficient_commitments)?;
            let signature = self.signature(&source.signature)?;
            output.push_reserved(GlobalThresholdBeaconDkgDealerCommitmentV1 {
                dealer_index: source.dealer_index,
                coefficient_commitments,
                constant_term_proof: source.constant_term_proof,
                signature,
            });
        }
        self.vector(output)
    }
    fn recipients(
        &mut self,
        source: &[GlobalThresholdBeaconDkgRecipientKeyV1],
    ) -> Result<Vec<GlobalThresholdBeaconDkgRecipientKeyV1>, SessionGraphError> {
        let mut output = self.buffer(source.len())?;
        for source in source {
            let validator = PeerId::new(self.key(source.validator.public_key())?);
            let mlkem768_public_key = self.copied(&source.mlkem768_public_key)?;
            let signature = self.signature(&source.signature)?;
            output.push_reserved(GlobalThresholdBeaconDkgRecipientKeyV1 {
                recipient_index: source.recipient_index,
                validator,
                x25519_public_key: source.x25519_public_key,
                mlkem768_public_key,
                signature,
            });
        }
        self.vector(output)
    }
    fn edges(
        &mut self,
        source: &[GlobalThresholdBeaconDkgEncryptedShareV1],
    ) -> Result<Vec<GlobalThresholdBeaconDkgEncryptedShareV1>, SessionGraphError> {
        let mut output = self.buffer(source.len())?;
        for source in source {
            let mlkem768_ciphertext = self.copied(&source.mlkem768_ciphertext)?;
            let encrypted_share = self.copied(&source.encrypted_share)?;
            let signature = self.signature(&source.signature)?;
            output.push_reserved(GlobalThresholdBeaconDkgEncryptedShareV1 {
                dealer_index: source.dealer_index,
                recipient_index: source.recipient_index,
                dealer_commitment_hash: source.dealer_commitment_hash,
                recipient_key_hash: source.recipient_key_hash,
                delivery_height: source.delivery_height,
                ephemeral_x25519_public_key: source.ephemeral_x25519_public_key,
                mlkem768_ciphertext,
                encrypted_share,
                signature,
            });
        }
        self.vector(output)
    }
    fn acceptances(
        &mut self,
        source: &[GlobalThresholdBeaconDkgShareAcceptanceV1],
    ) -> Result<Vec<GlobalThresholdBeaconDkgShareAcceptanceV1>, SessionGraphError> {
        let mut output = self.buffer(source.len())?;
        for source in source {
            let signature = self.signature(&source.signature)?;
            output.push_reserved(GlobalThresholdBeaconDkgShareAcceptanceV1 {
                dealer_index: source.dealer_index,
                recipient_index: source.recipient_index,
                dealer_commitment_hash: source.dealer_commitment_hash,
                encrypted_share_hash: source.encrypted_share_hash,
                accepted_height: source.accepted_height,
                signature,
            });
        }
        self.vector(output)
    }
}

/// Retain one complete canonical graph after preadmitting every destination allocation.
/// This copies only original bytes; the result does not claim semantic validity.
///
/// TODO: consume this exact materializer in the canonical validated-session/World
/// cutover together with physical verifier workspaces and explicit shared-control
/// admission. Do not wrap an existing ordinary deep clone or expose an unfunded
/// parallel constructor when that integration lands.
#[allow(unsafe_code)]
pub(super) fn retain_canonical_session(
    source: &GlobalThresholdBeaconKeySessionV1,
    budget: &AllocationBudget,
) -> Result<RetainedPayload<GlobalThresholdBeaconKeySessionV1>, SessionGraphError> {
    let mut construction = Construction::new(source, budget)?;
    let public_shares = construction.copied(&source.public_shares)?;
    let source_dkg = &source.adaptive_dkg;
    let dealer_commitments = construction.dealers(&source_dkg.dealer_commitments)?;
    let recipient_keys = construction.recipients(&source_dkg.recipient_keys)?;
    let encrypted_shares = construction.edges(&source_dkg.encrypted_shares)?;
    let share_acceptances = construction.acceptances(&source_dkg.share_acceptances)?;
    let qualified_dealers = construction.copied(&source_dkg.qualified_dealers)?;
    let adaptive_dkg = GlobalThresholdBeaconDkgTranscriptV1 {
        session: source_dkg.session,
        generator_h: source_dkg.generator_h,
        generator_v: source_dkg.generator_v,
        dealer_commitments,
        recipient_keys,
        encrypted_shares,
        share_acceptances,
        qualified_dealers,
        event_hash: source_dkg.event_hash,
        finalized_at_height: source_dkg.finalized_at_height,
    };
    let record = GlobalThresholdBeaconKeySessionV1 {
        version: source.version,
        network_id: source.network_id,
        session_id: source.session_id,
        roster_hash: source.roster_hash,
        committee_size: source.committee_size,
        threshold: source.threshold,
        group_public_key: source.group_public_key,
        public_shares,
        adaptive_dkg,
        dkg_contribution_hash: source.dkg_contribution_hash,
        transcript_hash: source.transcript_hash,
    };
    if construction.reservation.remaining_bytes() != 0 {
        return Err(SessionGraphError::PlanChanged);
    }
    let charges = construction
        .charges
        .take()
        .expect("complete original ledger");
    if charges.as_slice().len() != charges.capacity() {
        drop(record);
        drop(charges);
        return Err(SessionGraphError::PlanChanged);
    }
    // SAFETY: every exact destination buffer, key and signature was allocated
    // from the single original reservation and transferred unchanged above.
    // The private immutable record has no allocation escape or mutable access.
    match unsafe { RetainedPayload::try_new(record, charges, budget) } {
        Ok(owner) => Ok(owner),
        Err((record, charges, error)) => {
            drop(record);
            drop(charges);
            Err(error.into())
        }
    }
}

#[cfg(test)]
mod tests;
