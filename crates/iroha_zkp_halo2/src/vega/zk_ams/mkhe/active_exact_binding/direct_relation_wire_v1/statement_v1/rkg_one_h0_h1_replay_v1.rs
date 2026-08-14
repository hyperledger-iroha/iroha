//! Roster-free replay of the paired RKG-round-one polynomial statements.

use super::super::super::super::{
    ZkAmsMkheErrorV1,
    direct_collective_eval_ceremony::{
        DIRECT_POLYNOMIAL_STREAM_DOMAIN_V1, ZkAmsMkheDirectCeremonyContextV1,
        ZkAmsMkheDirectCeremonyRoundV1, ZkAmsMkheDirectPolynomialRoleV1,
    },
    direct_object_transport::{
        ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1, ZkAmsMkheDirectObjectKindV1,
        ZkAmsMkheDirectObjectReadAtProviderV1, ZkAmsMkheDirectObjectReadReceiptV1,
        ZkAmsMkheDirectObjectReadTransactionV1,
    },
    manifest::{RELEASE_MODULI_V1, ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1},
};
use super::super::super::{
    PersistentDirectRelationV1, VerifiedPersistentWitnessDirectRelationUseV1,
};
use super::{DirectRelationPublicObjectsV1, ExpectedDirectRelationStatementV1};
use crate::vega::sponge::Keccak256;

const RESIDUE_BYTES_V1: usize = core::mem::size_of::<u64>();
const RELEASE_RNS_LIMBS_V1: usize = RELEASE_MODULI_V1.len();
const EXACT_POLYNOMIAL_BYTES_V1: usize =
    RELEASE_RNS_LIMBS_V1 * ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 * RESIDUE_BYTES_V1;
const EXACT_PAIR_BYTES_V1: usize = 2 * EXACT_POLYNOMIAL_BYTES_V1;
const RESIDUES_PER_READ_V1: usize = ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 / RESIDUE_BYTES_V1;
const READ_CALLS_PER_LIMB_V1: usize = ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 / RESIDUES_PER_READ_V1;
const READ_CALLS_PER_OBJECT_V1: usize = RELEASE_RNS_LIMBS_V1 * READ_CALLS_PER_LIMB_V1;
const READ_CALLS_PER_PAIR_V1: usize = 2 * READ_CALLS_PER_OBJECT_V1;
const PAIR_WORKSPACE_BYTES_V1: usize = 2 * ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 * RESIDUE_BYTES_V1;
const PAIR_WORKSPACE_AND_SCRATCH_BYTES_V1: usize =
    PAIR_WORKSPACE_BYTES_V1 + ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1;
const INTERLEAVED_COMMON_A_WORKSPACE_BYTES_V1: usize =
    PAIR_WORKSPACE_AND_SCRATCH_BYTES_V1 + ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 * RESIDUE_BYTES_V1;
const _: () = {
    assert!(EXACT_POLYNOMIAL_BYTES_V1 == 39_845_888);
    assert!(EXACT_PAIR_BYTES_V1 == 79_691_776);
    assert!(RELEASE_RNS_LIMBS_V1 == 38);
    assert!(RESIDUES_PER_READ_V1 == 1_024);
    assert!(READ_CALLS_PER_OBJECT_V1 == 4_864);
    assert!(READ_CALLS_PER_PAIR_V1 == 9_728);
    assert!(PAIR_WORKSPACE_BYTES_V1 == 2_097_152);
    assert!(PAIR_WORKSPACE_AND_SCRATCH_BYTES_V1 == 2_105_344);
    assert!(INTERLEAVED_COMMON_A_WORKSPACE_BYTES_V1 == 3_153_920);
};

/// Paired, snapshot-bound replay of the two RKG-round-one polynomial statements.
pub(in super::super::super) struct DirectRkgOneH0H1StatementReplayV1 {
    h0_transaction: ZkAmsMkheDirectObjectReadTransactionV1,
    h1_transaction: ZkAmsMkheDirectObjectReadTransactionV1,
    h0_hash: Keccak256,
    h1_hash: Keccak256,
    h0_statement_digest: [u8; 32],
    h1_statement_digest: [u8; 32],
    next_limb: usize,
    poisoned: bool,
}

/// Move-only proof that both typed RKG-round-one statements were replayed completely.
pub(in super::super::super) struct CompletedDirectRkgOneH0H1StatementReplayV1 {
    _h0_receipt: ZkAmsMkheDirectObjectReadReceiptV1,
    _h1_receipt: ZkAmsMkheDirectObjectReadReceiptV1,
}

impl DirectRkgOneH0H1StatementReplayV1 {
    /// Bind the sealed relation axes and both typed objects before reading payload bytes.
    pub(in super::super::super) fn begin<P>(
        context: ZkAmsMkheDirectCeremonyContextV1,
        capability: &VerifiedPersistentWitnessDirectRelationUseV1,
        objects: DirectRelationPublicObjectsV1,
        provider: &mut P,
    ) -> Result<Self, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        capability.validate()?;
        let expected = ExpectedDirectRelationStatementV1::new(context, capability, objects)?;
        if expected.relation() != PersistentDirectRelationV1::RkgRoundOne {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let DirectRelationPublicObjectsV1::RkgRoundOne { h0, h1 } = objects else {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        };
        let h0_transaction = ZkAmsMkheDirectObjectReadTransactionV1::begin(
            ZkAmsMkheDirectObjectKindV1::RkgH0,
            h0.pointer,
            provider,
        )?;
        let h1_transaction = ZkAmsMkheDirectObjectReadTransactionV1::begin(
            ZkAmsMkheDirectObjectKindV1::RkgH1,
            h1.pointer,
            provider,
        )?;
        let party = capability.party.to_bytes();
        Ok(Self {
            h0_transaction,
            h1_transaction,
            h0_hash: polynomial_hash_prefix(
                context.digest(),
                capability.party_index,
                party,
                ZkAmsMkheDirectPolynomialRoleV1::RkgH0,
            ),
            h1_hash: polynomial_hash_prefix(
                context.digest(),
                capability.party_index,
                party,
                ZkAmsMkheDirectPolynomialRoleV1::RkgH1,
            ),
            h0_statement_digest: h0.statement_digest,
            h1_statement_digest: h1.statement_digest,
            next_limb: 0,
            poisoned: false,
        })
    }

    /// Replay exactly the next paired limb into the two caller-owned workspaces.
    pub(in super::super::super) fn replay_next_limb_pair_into<P>(
        &mut self,
        provider: &mut P,
        h0: &mut [u64],
        h1: &mut [u64],
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        if self.poisoned {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        self.poisoned = true;
        let result = self.replay_next_limb_pair_into_inner(provider, h0, h1);
        if result.is_ok() {
            self.poisoned = false;
        }
        result
    }

    fn replay_next_limb_pair_into_inner<P>(
        &mut self,
        provider: &mut P,
        h0: &mut [u64],
        h1: &mut [u64],
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        if h0.len() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
            || h1.len() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
            || self.next_limb >= RELEASE_RNS_LIMBS_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        let limb = self.next_limb;
        let modulus = RELEASE_MODULI_V1[limb];
        absorb_limb_header(&mut self.h0_hash, limb, modulus)?;
        absorb_limb_header(&mut self.h1_hash, limb, modulus)?;
        let mut scratch = [0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1];
        replay_limb(
            &mut self.h0_transaction,
            &mut self.h0_hash,
            provider,
            h0,
            modulus,
            &mut scratch,
        )?;
        replay_limb(
            &mut self.h1_transaction,
            &mut self.h1_hash,
            provider,
            h1,
            modulus,
            &mut scratch,
        )?;
        self.next_limb = self
            .next_limb
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?;
        Ok(())
    }

    /// Complete both content-address and statement-digest checks atomically.
    pub(in super::super::super) fn finish<P>(
        self,
        provider: &mut P,
    ) -> Result<CompletedDirectRkgOneH0H1StatementReplayV1, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        if self.poisoned || self.next_limb != RELEASE_RNS_LIMBS_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        let h0_receipt = self.h0_transaction.finish(provider)?;
        let h1_receipt = self.h1_transaction.finish(provider)?;
        if self.h0_hash.finalize() != self.h0_statement_digest
            || self.h1_hash.finalize() != self.h1_statement_digest
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let h0_snapshot = h0_receipt.snapshot();
        let h1_snapshot = h1_receipt.snapshot();
        if h0_snapshot.provider_identity() != h1_snapshot.provider_identity()
            || h0_snapshot.snapshot_identity() != h1_snapshot.snapshot_identity()
        {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(CompletedDirectRkgOneH0H1StatementReplayV1 {
            _h0_receipt: h0_receipt,
            _h1_receipt: h1_receipt,
        })
    }
}

fn polynomial_hash_prefix(
    context_digest: [u8; 32],
    party_index: u8,
    party: [u8; 32],
    role: ZkAmsMkheDirectPolynomialRoleV1,
) -> Keccak256 {
    let mut hash = Keccak256::new();
    hash.update(DIRECT_POLYNOMIAL_STREAM_DOMAIN_V1);
    hash.update(&context_digest);
    hash.update(&[
        ZkAmsMkheDirectCeremonyRoundV1::RkgRoundOne as u8,
        role as u8,
    ]);
    hash.update(&[party_index]);
    hash.update(&party);
    hash.update(&(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u32).to_be_bytes());
    hash.update(&[RELEASE_RNS_LIMBS_V1 as u8]);
    hash
}

fn absorb_limb_header(
    hash: &mut Keccak256,
    limb: usize,
    modulus: u64,
) -> Result<(), ZkAmsMkheErrorV1> {
    let limb = u8::try_from(limb).map_err(|_| ZkAmsMkheErrorV1::InvalidPolynomial)?;
    hash.update(&[limb]);
    hash.update(&modulus.to_be_bytes());
    Ok(())
}

fn replay_limb<P>(
    transaction: &mut ZkAmsMkheDirectObjectReadTransactionV1,
    hash: &mut Keccak256,
    provider: &mut P,
    destination: &mut [u64],
    modulus: u64,
    scratch: &mut [u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1],
) -> Result<(), ZkAmsMkheErrorV1>
where
    P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
{
    for residues in destination.chunks_exact_mut(RESIDUES_PER_READ_V1) {
        if transaction.read_next(provider, scratch)? != scratch.len() {
            return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
        }
        for (residue, encoded) in residues.iter_mut().zip(scratch.chunks_exact(8)) {
            let value = u64::from_be_bytes([
                encoded[0], encoded[1], encoded[2], encoded[3], encoded[4], encoded[5], encoded[6],
                encoded[7],
            ]);
            if value >= modulus {
                return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
            }
            *residue = value;
        }
        hash.update(scratch);
    }
    Ok(())
}

#[cfg(test)]
#[path = "rkg_one_h0_h1_replay_v1_tests.rs"]
mod tests;
