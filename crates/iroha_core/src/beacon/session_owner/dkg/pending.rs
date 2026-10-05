//! Physically prepared output rows; only exact initialized bytes may change before signing.

use super::*;
use crate::beacon::LocalGlobalThresholdBeaconDkgErrorV1;
use iroha_crypto::{Algorithm, KeyPair};

pub(in crate::beacon) trait SignedRow {
    fn preimage<'a>(
        session: &'a GlobalThresholdBeaconDkgSessionV1,
        row: &'a Self,
    ) -> DkgSignaturePreimage<'a>;
    fn install_signature(&mut self, signature: Signature);
}
macro_rules! signed_row {
    ($ty:ty, $variant:ident) => {
        impl SignedRow for $ty {
            fn preimage<'a>(
                session: &'a GlobalThresholdBeaconDkgSessionV1,
                row: &'a Self,
            ) -> DkgSignaturePreimage<'a> {
                DkgSignaturePreimage::$variant(session, row)
            }
            fn install_signature(&mut self, signature: Signature) {
                self.signature = signature;
            }
        }
    };
}
signed_row!(GlobalThresholdBeaconDkgRecipientKeyV1, RecipientKey);
signed_row!(GlobalThresholdBeaconDkgDealerCommitmentV1, DealerCommitment);
signed_row!(GlobalThresholdBeaconDkgEncryptedShareV1, EncryptedShare);
signed_row!(GlobalThresholdBeaconDkgShareAcceptanceV1, ShareAcceptance);

/// Private mutable construction owner; its canonical fields never escape until sealed.
/// Field order keeps every actual allocation before its original refund ledger.
pub(in crate::beacon) struct PendingRow<T> {
    payload: Option<T>,
    signature: Option<ChargedBuffer<u8>>,
    charges: Option<ChargedBuffer<AllocationCharge>>,
    budget: AllocationBudget,
}
impl<T> PendingRow<T> {
    fn prepare(
        mut demand: Demand,
        budget: &AllocationBudget,
        build: impl FnOnce(&mut Construction<'_, '_>) -> Result<T, SessionGraphError>,
    ) -> Result<Self, SessionGraphError> {
        demand.array::<u8>(Algorithm::BlsNormal.signature_payload_len())?;
        let mut reservation = budget.try_reserve_bytes(demand.total_bytes()?)?;
        let mut construction = Construction::with_demand(demand, budget, &mut reservation)?;
        let payload = build(&mut construction)?;
        let signature = construction.buffer(Algorithm::BlsNormal.signature_payload_len())?;
        let charges = construction
            .charges
            .take()
            .expect("prepared original ledger");
        if charges.as_slice().len() + 1 != charges.capacity()
            || construction.reservation.remaining_bytes() != 0
        {
            drop(payload);
            drop(signature);
            drop(charges);
            return Err(SessionGraphError::PlanChanged);
        }
        Ok(Self {
            payload: Some(payload),
            signature: Some(signature),
            charges: Some(charges),
            budget: budget.clone(),
        })
    }
    pub(in crate::beacon) fn record(&self) -> &T {
        self.payload.as_ref().expect("unfinished prepared row")
    }
    #[cfg(test)]
    pub(in crate::beacon) fn restoration_backing(&self) -> (*const u8, usize, usize) {
        let bytes = self.signature.as_ref().expect("original pending signature");
        (
            bytes.as_slice().as_ptr(),
            bytes.capacity(),
            bytes.as_slice().len(),
        )
    }
    // No generic mutable accessor is exposed: only exact-length byte replacements below.
    pub(in crate::beacon) fn sign(
        mut self,
        signer: &KeyPair,
        session: &GlobalThresholdBeaconDkgSessionV1,
        workspace: &mut DkgMessageWorkspace,
    ) -> Result<RetainedPayload<T>, LocalGlobalThresholdBeaconDkgErrorV1>
    where
        T: SignedRow,
    {
        let bytes = workspace.write(T::preimage(session, self.record()))?;
        let output = self
            .signature
            .take()
            .expect("original prepared signing output");
        #[cfg(all(test, sumeragi_core_mutation = "HC91"))]
        let output = {
            // Deliberate mutant: discard the pre-claim physical output and
            // reconstruct it only when the original secret is being signed.
            drop(output);
            let mut reservation = self
                .budget
                .try_reserve_bytes(Algorithm::BlsNormal.signature_payload_len())
                .map_err(SessionGraphError::from)?;
            ChargedBuffer::from_reservation(
                Algorithm::BlsNormal.signature_payload_len(),
                &mut reservation,
            )
            .map_err(SessionGraphError::from)?
        };
        let signature = match Signature::try_new_bls_prepaid(signer.private_key(), bytes, output) {
            Ok(signature) => signature,
            Err((output, error)) => {
                self.signature = Some(output);
                return Err(error.into());
            }
        };
        self.publish_signature(signature)
    }

    /// Validate the exact original output and ledger before restoring any bytes.
    fn validate_restored_signature(&self, signature: &Signature) -> Result<(), SessionGraphError> {
        let output = self
            .signature
            .as_ref()
            .ok_or(SessionGraphError::PlanChanged)?;
        let ledger = self
            .charges
            .as_ref()
            .ok_or(SessionGraphError::PlanChanged)?;
        if !output.belongs_to(&self.budget)
            || !output.as_slice().is_empty()
            || output.capacity() != Algorithm::BlsNormal.signature_payload_len()
            || signature.payload().len() != output.capacity()
            || !ledger.belongs_to(&self.budget)
            || ledger.as_slice().len() + 1 != ledger.capacity()
            || ledger
                .as_slice()
                .iter()
                .any(|charge| !charge.belongs_to(&self.budget))
        {
            return Err(SessionGraphError::PlanChanged);
        }
        Ok(())
    }

    fn fill_restored_signature(&mut self, signature: &Signature) {
        self.signature
            .as_mut()
            .expect("checked original signature output")
            .append(signature.payload())
            .expect("checked exact original signature capacity");
    }

    /// Publish only after the local restore owner checked and filled both rows.
    /// This consumes the original output, never a signing producer or replacement.
    pub(in crate::beacon) fn finish_restored(self) -> RetainedPayload<T>
    where
        T: SignedRow,
    {
        let mut original = self;
        let output = original.signature.take().expect("restored original output");
        let signature =
            iroha_crypto::ChargedSignature::try_from_preallocated(&original.budget, output)
                .unwrap_or_else(|(_, error)| {
                    panic!("checked original restored signature: {error}")
                });
        original
            .publish_signature(signature)
            .unwrap_or_else(|error| panic!("checked original restored row custody: {error}"))
    }

    #[allow(unsafe_code)]
    fn publish_signature(
        mut self,
        signature: iroha_crypto::ChargedSignature,
    ) -> Result<RetainedPayload<T>, LocalGlobalThresholdBeaconDkgErrorV1>
    where
        T: SignedRow,
    {
        if !signature.belongs_to(&self.budget) {
            return Err(SessionGraphError::PlanChanged.into());
        }
        // SAFETY: the canonical signature and exact original charge move together
        // into this immutable row's original ledger. No growth/clone/export follows.
        let (signature, charge) = unsafe { signature.into_allocation_parts() };
        let payload = self.payload.as_mut().expect("unfinished row");
        payload.install_signature(signature);
        let ledger = self.charges.as_mut().expect("original row ledger");
        if let Err(charge) = ledger.try_push(charge) {
            std::mem::forget(charge);
            return Err(SessionGraphError::PlanChanged.into());
        }
        let payload = self.payload.take().expect("completed row");
        let ledger = self.charges.take().expect("completed row ledger");
        // SAFETY: all nested storage came from the same original construction;
        // the sole private mutation only filled existing capacity or moved the
        // prepaid signature into its reserved ledger slot.
        match unsafe { RetainedPayload::try_new(payload, ledger, &self.budget) } {
            Ok(owner) => Ok(owner),
            Err((payload, ledger, error)) => {
                drop(payload);
                drop(ledger);
                Err(SessionGraphError::from(error).into())
            }
        }
    }
}
impl PendingRow<GlobalThresholdBeaconDkgRecipientKeyV1> {
    pub(in crate::beacon) fn validate_restoration(
        &self,
        source: &GlobalThresholdBeaconDkgRecipientKeyV1,
    ) -> Result<(), SessionGraphError> {
        let row = self.record();
        if row.recipient_index != source.recipient_index
            || row.validator != source.validator
            || row.mlkem768_public_key.len() != source.mlkem768_public_key.len()
        {
            return Err(SessionGraphError::PlanChanged);
        }
        self.validate_restored_signature(&source.signature)
    }

    pub(in crate::beacon) fn restore_publication(
        &mut self,
        source: &GlobalThresholdBeaconDkgRecipientKeyV1,
    ) {
        let row = self.payload.as_mut().expect("checked original recipient");
        row.x25519_public_key = source.x25519_public_key;
        row.mlkem768_public_key
            .as_mut_slice()
            .copy_from_slice(&source.mlkem768_public_key);
        self.fill_restored_signature(&source.signature);
    }
    pub(in crate::beacon) fn recipient(
        index: u16,
        key: &PublicKey,
        budget: &AllocationBudget,
    ) -> Result<Self, SessionGraphError> {
        let mut demand = Demand::default();
        demand.add(key.retained_allocation_layout())?;
        demand.array::<u8>(soranet_pq::MlKemSuite::MlKem768.public_key_len())?;
        Self::prepare(demand, budget, |construction| {
            let validator = PeerId::new(construction.key(key)?);
            let bytes = construction.copied(&[0; 1184])?;
            Ok(GlobalThresholdBeaconDkgRecipientKeyV1 {
                recipient_index: index,
                validator,
                x25519_public_key: [0; 32],
                mlkem768_public_key: bytes,
                signature: Signature::from_bytes(&[]),
            })
        })
    }
    pub(in crate::beacon) fn fill_recipient(
        &mut self,
        key: &iroha_crypto::hybrid::HybridPublicKey,
    ) {
        let row = self.payload.as_mut().expect("unfinished recipient");
        row.x25519_public_key = key.x25519_bytes();
        row.mlkem768_public_key
            .as_mut_slice()
            .copy_from_slice(key.kyber_bytes());
    }
}
impl PendingRow<GlobalThresholdBeaconDkgDealerCommitmentV1> {
    pub(in crate::beacon) fn validate_restoration(
        &self,
        source: &GlobalThresholdBeaconDkgDealerCommitmentV1,
    ) -> Result<(), SessionGraphError> {
        let row = self.record();
        if row.dealer_index != source.dealer_index
            || row.coefficient_commitments.len() != source.coefficient_commitments.len()
        {
            return Err(SessionGraphError::PlanChanged);
        }
        self.validate_restored_signature(&source.signature)
    }

    pub(in crate::beacon) fn restore_publication(
        &mut self,
        source: &GlobalThresholdBeaconDkgDealerCommitmentV1,
    ) {
        let row = self.payload.as_mut().expect("checked original dealer");
        row.coefficient_commitments
            .as_mut_slice()
            .copy_from_slice(&source.coefficient_commitments);
        row.constant_term_proof.commitment = source.constant_term_proof.commitment;
        row.constant_term_proof.response = source.constant_term_proof.response;
        self.fill_restored_signature(&source.signature);
    }
    pub(in crate::beacon) fn dealer(
        index: u16,
        threshold: u16,
        budget: &AllocationBudget,
    ) -> Result<Self, SessionGraphError> {
        let mut demand = Demand::default();
        demand.array::<[u8; 96]>(usize::from(threshold))?;
        Self::prepare(demand, budget, |construction| {
            let mut coefficients = construction.buffer(usize::from(threshold))?;
            for _ in 0..threshold {
                coefficients.push_reserved([0; 96]);
            }
            Ok(GlobalThresholdBeaconDkgDealerCommitmentV1 {
                dealer_index: index,
                coefficient_commitments: construction.vector(coefficients)?,
                constant_term_proof:
                    iroha_data_model::consensus::GlobalThresholdBeaconDkgConstantProofV1 {
                        commitment: [0; 96],
                        response: [0; 32],
                    },
                signature: Signature::from_bytes(&[]),
            })
        })
    }
    pub(in crate::beacon) fn fill_dealer(
        &mut self,
        source: &iroha_crypto::threshold_bls::ValidatedDealerCommitment<
            iroha_crypto::threshold_bls::BeaconPurpose,
        >,
    ) {
        let row = self.payload.as_mut().expect("unfinished dealer");
        assert_eq!(
            row.coefficient_commitments.len(),
            source.coefficients().len()
        );
        for (destination, source) in row
            .coefficient_commitments
            .iter_mut()
            .zip(source.coefficients())
        {
            *destination = *source.as_bytes();
        }
        row.constant_term_proof.commitment = *source.constant_proof().commitment_bytes();
        row.constant_term_proof.response = *source.constant_proof().response_bytes();
    }
}
impl PendingRow<GlobalThresholdBeaconDkgEncryptedShareV1> {
    pub(in crate::beacon) fn validate_restoration(
        &self,
        source: &GlobalThresholdBeaconDkgEncryptedShareV1,
    ) -> Result<(), SessionGraphError> {
        let row = self.record();
        if row.dealer_index != source.dealer_index
            || row.recipient_index != source.recipient_index
            || row.mlkem768_ciphertext.len() != source.mlkem768_ciphertext.len()
            || row.encrypted_share.len() != source.encrypted_share.len()
        {
            return Err(SessionGraphError::PlanChanged);
        }
        self.validate_restored_signature(&source.signature)
    }

    pub(in crate::beacon) fn restore_publication(
        &mut self,
        source: &GlobalThresholdBeaconDkgEncryptedShareV1,
    ) {
        let row = self
            .payload
            .as_mut()
            .expect("checked original encrypted edge");
        row.dealer_commitment_hash = source.dealer_commitment_hash;
        row.recipient_key_hash = source.recipient_key_hash;
        row.delivery_height = source.delivery_height;
        row.ephemeral_x25519_public_key = source.ephemeral_x25519_public_key;
        row.mlkem768_ciphertext
            .as_mut_slice()
            .copy_from_slice(&source.mlkem768_ciphertext);
        row.encrypted_share
            .as_mut_slice()
            .copy_from_slice(&source.encrypted_share);
        self.fill_restored_signature(&source.signature);
    }

    pub(in crate::beacon) fn edge(
        dealer: u16,
        recipient: u16,
        budget: &AllocationBudget,
    ) -> Result<Self, SessionGraphError> {
        let mut demand = Demand::default();
        demand.array::<u8>(soranet_pq::MlKemSuite::MlKem768.ciphertext_len())?;
        demand.array::<u8>(124)?;
        Self::prepare(demand, budget, |construction| {
            Ok(GlobalThresholdBeaconDkgEncryptedShareV1 {
                dealer_index: dealer,
                recipient_index: recipient,
                dealer_commitment_hash: [0; 32],
                recipient_key_hash: [0; 32],
                delivery_height: 0,
                ephemeral_x25519_public_key: [0; 32],
                mlkem768_ciphertext: construction.copied(&[0; 1088])?,
                encrypted_share: construction.copied(&[0; 124])?,
                signature: Signature::from_bytes(&[]),
            })
        })
    }
    pub(in crate::beacon) fn bind_edge(
        &mut self,
        dealer_hash: [u8; 32],
        recipient_hash: [u8; 32],
        height: u64,
    ) {
        let row = self.payload.as_mut().expect("unfinished edge");
        row.dealer_commitment_hash = dealer_hash;
        row.recipient_key_hash = recipient_hash;
        row.delivery_height = height;
    }
    pub(in crate::beacon) fn fill_edge(
        &mut self,
        kem: &iroha_crypto::hybrid::HybridKemCiphertext,
        encrypted: &[u8; 124],
    ) {
        let row = self.payload.as_mut().expect("unfinished edge");
        row.ephemeral_x25519_public_key = *kem.ephemeral_public();
        row.mlkem768_ciphertext
            .as_mut_slice()
            .copy_from_slice(kem.kyber_ciphertext());
        row.encrypted_share
            .as_mut_slice()
            .copy_from_slice(encrypted);
    }
}
impl PendingRow<GlobalThresholdBeaconDkgShareAcceptanceV1> {
    pub(in crate::beacon) fn validate_restoration(
        &self,
        source: &GlobalThresholdBeaconDkgShareAcceptanceV1,
    ) -> Result<(), SessionGraphError> {
        let row = self.record();
        if row.dealer_index != source.dealer_index || row.recipient_index != source.recipient_index
        {
            return Err(SessionGraphError::PlanChanged);
        }
        self.validate_restored_signature(&source.signature)
    }

    pub(in crate::beacon) fn restore_publication(
        &mut self,
        source: &GlobalThresholdBeaconDkgShareAcceptanceV1,
    ) {
        let row = self.payload.as_mut().expect("checked original acceptance");
        row.dealer_commitment_hash = source.dealer_commitment_hash;
        row.encrypted_share_hash = source.encrypted_share_hash;
        row.accepted_height = source.accepted_height;
        self.fill_restored_signature(&source.signature);
    }

    pub(in crate::beacon) fn acceptance(
        dealer: u16,
        recipient: u16,
        budget: &AllocationBudget,
    ) -> Result<Self, SessionGraphError> {
        Self::prepare(Demand::default(), budget, |_| {
            Ok(GlobalThresholdBeaconDkgShareAcceptanceV1 {
                dealer_index: dealer,
                recipient_index: recipient,
                dealer_commitment_hash: [0; 32],
                encrypted_share_hash: [0; 32],
                accepted_height: 0,
                signature: Signature::from_bytes(&[]),
            })
        })
    }
    pub(in crate::beacon) fn bind_acceptance(
        &mut self,
        dealer_hash: [u8; 32],
        edge_hash: [u8; 32],
        height: u64,
    ) {
        let row = self.payload.as_mut().expect("unfinished acceptance");
        row.dealer_commitment_hash = dealer_hash;
        row.encrypted_share_hash = edge_hash;
        row.accepted_height = height;
    }
}
