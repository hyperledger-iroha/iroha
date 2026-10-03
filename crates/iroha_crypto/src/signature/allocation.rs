//! Original-pool custody of the canonical signature's exact payload allocation.

use std::alloc::Layout;

use iroha_allocation::{
    AllocationBudget, AllocationCharge, ChargedBuffer, ChargedBufferFromChargeError,
};
use iroha_primitives::const_vec::ConstVec;

use super::Signature;

/// Immutable canonical signature bytes with their original prepaid allocation charge.
///
/// This move-only owner exposes no mutation or safe allocation extraction. The
/// signature payload is destroyed before its credit is refunded. Any ordinary
/// clone obtained from the borrowed signature creates a separate, unfunded owner.
/// Copying an existing byte sequence does not establish signature validity.
pub struct ChargedSignature {
    signature: Signature,
    charge: AllocationCharge,
}

/// Original source-custody or exact physical allocation failure while copying a signature.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignatureAllocationError {
    /// The offered charge belongs to another original finite pool.
    ForeignPool,
    /// The exact layout differs or the allocator refused its admitted request.
    Allocation(ChargedBufferFromChargeError),
}

impl std::fmt::Display for SignatureAllocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ForeignPool => formatter.write_str("signature charge belongs to another pool"),
            Self::Allocation(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for SignatureAllocationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ForeignPool => None,
            Self::Allocation(error) => Some(error),
        }
    }
}

impl Signature {
    /// Exact layout of the retained payload, including an empty or malformed value.
    ///
    /// This only measures existing allocation geometry. It neither allocates nor
    /// authenticates the signature and introduces no additional wire encoding.
    #[must_use]
    pub fn retained_allocation_layout(&self) -> Layout {
        Layout::array::<u8>(self.payload.len())
            .expect("an existing signature Box has a representable allocation layout")
    }

    /// Copy the exact existing payload into prepaid storage from this original pool.
    ///
    /// This invokes no ordinary Clone, verifier, parser, cache or new admission.
    /// The source is unchanged, including empty/all-zero/noncanonical bytes.
    /// Callers remain responsible for authenticating those bytes independently.
    /// Empty payloads allocate no backing but still preserve the offered charge.
    ///
    /// # Errors
    /// Returns the same charge on a foreign pool, size/alignment mismatch or
    /// physical allocator refusal; refusal does not refund or reacquire credit.
    pub fn try_clone_from_charge(
        &self,
        budget: &AllocationBudget,
        charge: AllocationCharge,
    ) -> Result<ChargedSignature, (AllocationCharge, SignatureAllocationError)> {
        if !charge.belongs_to(budget) {
            return Err((charge, SignatureAllocationError::ForeignPool));
        }
        let mut bytes = ChargedBuffer::<u8>::try_from_charge(self.payload.len(), charge)
            .map_err(|(charge, error)| (charge, SignatureAllocationError::Allocation(error)))?;
        bytes
            .append(&self.payload)
            .expect("source length equals the exact original prepaid capacity");
        Ok(Self::bind_payload_allocation(bytes))
    }

    #[allow(unsafe_code)]
    fn bind_payload_allocation(bytes: ChargedBuffer<u8>) -> ChargedSignature {
        // SAFETY: length equals exact capacity, including zero. Converting the
        // Vec to a Box therefore cannot resize. The same original charge moves
        // immediately beside this sole canonical payload; no mutation/escape
        // API is exposed. The Box<u8> destructor cannot unwind, and field order
        // destroys it before credit is returned, including during unwinding.
        let (bytes, charge) = unsafe { bytes.into_allocation_parts() };
        let signature = Signature {
            payload: ConstVec::new(bytes.into_boxed_slice()),
        };
        ChargedSignature { signature, charge }
    }
}

impl ChargedSignature {
    /// Borrow the canonical signature without moving its payload or charge.
    #[must_use]
    pub fn get(&self) -> &Signature {
        &self.signature
    }

    /// Transfer the exact canonical allocation and charge into an audited owner.
    ///
    /// # Safety
    /// Immediately retain both values in a move-only canonical payload/ledger.
    /// Destroy the original signature allocation before refunding its charge,
    /// including every refusal and unwind. Do not mutate, replace, clone or
    /// export its payload while refund custody stays behind. Any independent
    /// clone needs its own admission and must not reuse this original charge.
    #[allow(unsafe_code)]
    pub unsafe fn into_allocation_parts(self) -> (Signature, AllocationCharge) {
        let Self { signature, charge } = self;
        (signature, charge)
    }
}
