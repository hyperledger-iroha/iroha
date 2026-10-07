//! Shared bounded history original codec; only installed verifiers can authenticate it.
use crate::kagemusha_wallet_artifacts_v1::producer_inventory::QualifiedReceiptSourceV1;
use ff::PrimeField;
use iroha_kagemusha_proof::finality::{
    continuity::SourceNodeEvidence,
    history::{HistorySlot, HistoryState},
    native::{HistoryPrefix, InstalledFinality},
};
use iroha_pasta::{Ep, Eq, Fp, msm::MemoryBudget};
use iroha_plonk_recursion::AccumulatorT;

/// Maximum whole canonical proof/state frame; independent of historical height.
pub const HISTORY_ORIGINAL_MAX_BYTES_V1: usize = 64 * 1024;
/// Invalid DATA or failed full source/claim verification.
#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum HistoryOriginalErrorV1 {
    /// Malformed, noncanonical or overbound original.
    #[error("invalid bounded history original")]
    Encoding,
    /// Source, endpoint or full proof/claim verification failed.
    #[error(transparent)]
    Proof(#[from] iroha_kagemusha_proof::finality::native::Error),
}
#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::FinalitySlotOriginalV1")]
struct SlotOriginal {
    pending: bool,
    epoch: u64,
    context: [u8; 32],
    boundary_height: u64,
    predecessor: [u8; 32],
    parameters: [u64; 6],
}
impl From<HistorySlot> for SlotOriginal {
    fn from(value: HistorySlot) -> Self {
        Self {
            pending: value.pending,
            epoch: value.epoch,
            context: value.context,
            boundary_height: value.boundary_height,
            predecessor: value.predecessor,
            parameters: value.parameters,
        }
    }
}
impl From<SlotOriginal> for HistorySlot {
    fn from(value: SlotOriginal) -> Self {
        Self {
            pending: value.pending,
            epoch: value.epoch,
            context: value.context,
            boundary_height: value.boundary_height,
            predecessor: value.predecessor,
            parameters: value.parameters,
        }
    }
}
/// One canonical bounded original history proof and untrusted state opening.
/// Its preserved schema is shared by durable ledger prefixes and compact registration;
/// decoding never constructs a verified history capability.
#[derive(Clone, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_state_v1::FinalityPrefixOriginalV1")]
pub struct HistoryOriginalV1 {
    next_height: u64,
    current: SlotOriginal,
    following: SlotOriginal,
    result: [u8; 32],
    tape_root: [u8; 32],
    frame_len: u32,
    endpoints: [[u8; 32]; 6],
    proof: Vec<u8>,
    pallas: Vec<u8>,
    vesta: Vec<u8>,
}
impl HistoryOriginalV1 {
    /// Claimed next height for local DATA consistency checks; only restoration authenticates it.
    pub const fn next_height(&self) -> u64 {
        self.next_height
    }

    /// Retain the exact original proof/state of an already verified prefix.
    pub fn from_prefix(prefix: &HistoryPrefix) -> Self {
        let state = prefix.state();
        let evidence = prefix.evidence();
        Self {
            next_height: state.next_height,
            current: state.current.into(),
            following: state.following.into(),
            result: state.result,
            tape_root: state.tape_root.to_repr(),
            frame_len: state.frame_len,
            endpoints: evidence.endpoints.map(|word| word.to_repr()),
            proof: evidence.proof.clone(),
            pallas: evidence.pallas.to_bytes().to_vec(),
            vesta: evidence.vesta.to_bytes().to_vec(),
        }
    }
    fn parts(self) -> Result<(HistoryState, SourceNodeEvidence), HistoryOriginalErrorV1> {
        let field = |bytes| {
            Option::<Fp>::from(Fp::from_repr(bytes)).ok_or(HistoryOriginalErrorV1::Encoding)
        };
        let mut endpoints = [Fp::from(0); 6];
        for (index, value) in self.endpoints.into_iter().enumerate() {
            endpoints[index] = field(value)?;
        }
        let state = HistoryState {
            next_height: self.next_height,
            current: self.current.into(),
            following: self.following.into(),
            result: self.result,
            tape_root: field(self.tape_root)?,
            frame_len: self.frame_len,
        };
        if !state.is_canonical() {
            return Err(HistoryOriginalErrorV1::Encoding);
        }
        let evidence = SourceNodeEvidence {
            endpoints,
            proof: self.proof,
            pallas: AccumulatorT::<Ep>::from_bytes(&self.pallas)
                .map_err(|_| HistoryOriginalErrorV1::Encoding)?,
            vesta: AccumulatorT::<Eq>::from_bytes(&self.vesta)
                .map_err(|_| HistoryOriginalErrorV1::Encoding)?,
        };
        Ok((state, evidence))
    }
    /// Encode bounded original DATA. This does not authenticate history.
    /// # Errors
    /// Encoding failure or excessive complete frame size.
    pub fn encode_canonical(&self) -> Result<Vec<u8>, HistoryOriginalErrorV1> {
        let bytes = norito::encode_canonical(self).map_err(|_| HistoryOriginalErrorV1::Encoding)?;
        if bytes.len() > HISTORY_ORIGINAL_MAX_BYTES_V1 {
            return Err(HistoryOriginalErrorV1::Encoding);
        }
        Ok(bytes)
    }
    /// Decode exact bounded original DATA without acquiring authority.
    /// # Errors
    /// Empty, oversized, noncanonical or trailing original bytes.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, HistoryOriginalErrorV1> {
        if bytes.is_empty() || bytes.len() > HISTORY_ORIGINAL_MAX_BYTES_V1 {
            return Err(HistoryOriginalErrorV1::Encoding);
        }
        norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(bytes.len()))
            .map_err(|_| HistoryOriginalErrorV1::Encoding)
    }
    /// Reverify through the exact mounted producer's existing history verifier.
    /// # Errors
    /// Malformed original or any failed source/proof/claim binding.
    pub fn restore_producer(
        self,
        graph: &InstalledFinality,
        budget: MemoryBudget,
    ) -> Result<HistoryPrefix, HistoryOriginalErrorV1> {
        let (state, evidence) = self.parts()?;
        graph
            .restore_history(&state, evidence, budget)
            .map_err(Into::into)
    }
    /// Reverify through the qualified descriptor/VK-only owner, without opening a PK.
    /// # Errors
    /// Malformed original, failed binding or cooperative cancellation.
    pub fn restore_qualified(
        self,
        graph: &QualifiedReceiptSourceV1,
        budget: MemoryBudget,
        cancellation: Option<&iroha_pasta::CancellationToken>,
    ) -> Result<HistoryPrefix, HistoryOriginalErrorV1> {
        let (state, evidence) = self.parts()?;
        graph
            .restore_history(&state, evidence, budget, cancellation)
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_history_data_is_canonical_but_cannot_supply_an_empty_claim() {
        let slot = || SlotOriginal {
            pending: false,
            epoch: 0,
            context: [1; 32],
            boundary_height: 0,
            predecessor: [0; 32],
            parameters: [1; 6],
        };
        let value = HistoryOriginalV1 {
            next_height: 2,
            current: slot(),
            following: slot(),
            result: [0; 32],
            tape_root: Fp::from(0).to_repr(),
            frame_len: 0,
            endpoints: [Fp::from(0).to_repr(); 6],
            proof: vec![],
            pallas: vec![],
            vesta: vec![],
        };
        let bytes = value.encode_canonical().unwrap();
        let parsed = HistoryOriginalV1::decode_canonical(&bytes).unwrap();
        assert_eq!(parsed.encode_canonical().unwrap(), bytes);
        assert!(parsed.parts().is_err());
        let mut trailing = bytes;
        trailing.push(0);
        assert!(HistoryOriginalV1::decode_canonical(&trailing).is_err());
        assert!(HistoryOriginalV1::decode_canonical(&[]).is_err());
        assert!(
            HistoryOriginalV1::decode_canonical(&vec![0; HISTORY_ORIGINAL_MAX_BYTES_V1 + 1])
                .is_err()
        );
        let mut over = value;
        over.proof = vec![0; HISTORY_ORIGINAL_MAX_BYTES_V1];
        assert!(over.encode_canonical().is_err());
    }
}
