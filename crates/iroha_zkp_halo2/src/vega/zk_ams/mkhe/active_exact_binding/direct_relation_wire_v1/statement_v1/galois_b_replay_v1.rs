//! Roster-free replay of one direct-ceremony Galois `b_i` statement.

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
const RESIDUES_PER_READ_V1: usize = ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1 / RESIDUE_BYTES_V1;
const READ_CALLS_PER_LIMB_V1: usize = ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 / RESIDUES_PER_READ_V1;
const READ_CALLS_PER_OBJECT_V1: usize = RELEASE_RNS_LIMBS_V1 * READ_CALLS_PER_LIMB_V1;
const LIMB_WORKSPACE_BYTES_V1: usize = ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 * RESIDUE_BYTES_V1;
const REPLAY_LIVE_PAYLOAD_BYTES_V1: usize =
    LIMB_WORKSPACE_BYTES_V1 + ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1;
const _: () = {
    assert!(ZkAmsMkheDirectCeremonyRoundV1::Galois as u8 == 4);
    assert!(ZkAmsMkheDirectPolynomialRoleV1::GaloisB as u8 == 4);
    assert!(RELEASE_RNS_LIMBS_V1 == 38);
    assert!(EXACT_POLYNOMIAL_BYTES_V1 == 39_845_888);
    assert!(RESIDUES_PER_READ_V1 == 1_024);
    assert!(READ_CALLS_PER_LIMB_V1 == 128);
    assert!(READ_CALLS_PER_OBJECT_V1 == 4_864);
    assert!(LIMB_WORKSPACE_BYTES_V1 == 1_048_576);
    assert!(REPLAY_LIVE_PAYLOAD_BYTES_V1 == 1_056_768);
};

/// Snapshot-bound replay of one role-typed Galois `b_i` polynomial.
pub(in super::super::super) struct DirectGaloisBStatementReplayV1 {
    transaction: ZkAmsMkheDirectObjectReadTransactionV1,
    statement_hash: Keccak256,
    expected_statement_digest: [u8; 32],
    next_limb: usize,
    poisoned: bool,
}

/// Move-only proof that the typed Galois `b_i` statement replayed completely.
pub(in super::super::super) struct CompletedDirectGaloisBStatementReplayV1 {
    _read_receipt: ZkAmsMkheDirectObjectReadReceiptV1,
}

impl DirectGaloisBStatementReplayV1 {
    /// Bind the sealed relation axes and typed object before reading payload bytes.
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
        if expected.relation() != PersistentDirectRelationV1::Galois {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        let DirectRelationPublicObjectsV1::Galois { b } = objects else {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        };
        let transaction = ZkAmsMkheDirectObjectReadTransactionV1::begin(
            ZkAmsMkheDirectObjectKindV1::GaloisB,
            b.pointer,
            provider,
        )?;
        Ok(Self {
            transaction,
            statement_hash: polynomial_hash_prefix(
                context.digest(),
                capability.party_index,
                capability.party.to_bytes(),
            ),
            expected_statement_digest: b.statement_digest,
            next_limb: 0,
            poisoned: false,
        })
    }

    /// Replay exactly the next complete canonical RNS limb.
    pub(in super::super::super) fn replay_next_limb_into<P>(
        &mut self,
        provider: &mut P,
        output: &mut [u64],
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        if self.poisoned {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        self.poisoned = true;
        let result = self.replay_next_limb_into_inner(provider, output);
        if result.is_ok() {
            self.poisoned = false;
        }
        result
    }

    fn replay_next_limb_into_inner<P>(
        &mut self,
        provider: &mut P,
        output: &mut [u64],
    ) -> Result<(), ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        if output.len() != ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1
            || self.next_limb >= RELEASE_RNS_LIMBS_V1
        {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        let limb = self.next_limb;
        let modulus = RELEASE_MODULI_V1[limb];
        self.statement_hash.update(
            &u8::try_from(limb)
                .map_err(|_| ZkAmsMkheErrorV1::InvalidPolynomial)?
                .to_be_bytes(),
        );
        self.statement_hash.update(&modulus.to_be_bytes());
        let mut scratch = [0_u8; ZK_AMS_MKHE_DIRECT_OBJECT_READ_BYTES_V1];
        for residues in output.chunks_exact_mut(RESIDUES_PER_READ_V1) {
            if self.transaction.read_next(provider, &mut scratch)? != scratch.len() {
                return Err(ZkAmsMkheErrorV1::InvalidWireEncoding);
            }
            for (residue, encoded) in residues.iter_mut().zip(scratch.chunks_exact(8)) {
                let value = u64::from_be_bytes(
                    encoded
                        .try_into()
                        .map_err(|_| ZkAmsMkheErrorV1::InvalidWireEncoding)?,
                );
                if value >= modulus {
                    return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
                }
                *residue = value;
            }
            self.statement_hash.update(&scratch);
        }
        self.next_limb = self
            .next_limb
            .checked_add(1)
            .ok_or(ZkAmsMkheErrorV1::InvalidPolynomial)?;
        Ok(())
    }

    /// Complete both the content-address and polynomial-statement checks.
    pub(in super::super::super) fn finish<P>(
        self,
        provider: &mut P,
    ) -> Result<CompletedDirectGaloisBStatementReplayV1, ZkAmsMkheErrorV1>
    where
        P: ZkAmsMkheDirectObjectReadAtProviderV1 + ?Sized,
    {
        if self.poisoned || self.next_limb != RELEASE_RNS_LIMBS_V1 {
            return Err(ZkAmsMkheErrorV1::InvalidPolynomial);
        }
        let read_receipt = self.transaction.finish(provider)?;
        if self.statement_hash.finalize() != self.expected_statement_digest {
            return Err(ZkAmsMkheErrorV1::InvalidKeyMaterial);
        }
        Ok(CompletedDirectGaloisBStatementReplayV1 {
            _read_receipt: read_receipt,
        })
    }
}

fn polynomial_hash_prefix(context_digest: [u8; 32], party_index: u8, party: [u8; 32]) -> Keccak256 {
    let mut hash = Keccak256::new();
    hash.update(DIRECT_POLYNOMIAL_STREAM_DOMAIN_V1);
    hash.update(&context_digest);
    hash.update(&[
        ZkAmsMkheDirectCeremonyRoundV1::Galois as u8,
        ZkAmsMkheDirectPolynomialRoleV1::GaloisB as u8,
    ]);
    hash.update(&[party_index]);
    hash.update(&party);
    hash.update(&(ZK_AMS_MKHE_RELEASE_RING_DEGREE_V1 as u32).to_be_bytes());
    hash.update(&[RELEASE_RNS_LIMBS_V1 as u8]);
    hash
}

#[cfg(test)]
#[path = "galois_b_replay_v1_tests.rs"]
mod tests;
