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
    /// The initialized payload does not fill its exact prepaid allocation.
    IncompleteBuffer {
        /// Number of initialized payload bytes.
        initialized: usize,
        /// Exact original backing capacity.
        capacity: usize,
    },
    /// The exact layout differs or the allocator refused its admitted request.
    Allocation(ChargedBufferFromChargeError),
}

impl std::fmt::Display for SignatureAllocationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ForeignPool => formatter.write_str("signature charge belongs to another pool"),
            Self::IncompleteBuffer {
                initialized,
                capacity,
            } => write!(
                formatter,
                "signature initialized length {initialized} differs from capacity {capacity}"
            ),
            Self::Allocation(error) => error.fmt(formatter),
        }
    }
}
impl std::error::Error for SignatureAllocationError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ForeignPool | Self::IncompleteBuffer { .. } => None,
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
    pub(crate) fn bind_payload_allocation(bytes: ChargedBuffer<u8>) -> ChargedSignature {
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

/// BLS signing failure which returns the caller's original output allocation.
#[cfg(feature = "bls")]
#[derive(Debug, thiserror::Error)]
pub enum PrepaidBlsSignatureError {
    /// Only the two existing BLS algorithms have this fixed producer.
    #[error("prepaid BLS signing requires a BLS private key")]
    Algorithm,
    /// The caller did not supply an empty exact-size output buffer.
    #[error(
        "prepaid BLS output geometry is invalid: expected {expected}, capacity {capacity}, initialized {initialized}"
    )]
    OutputGeometry {
        /// Signature length required by the selected orientation.
        expected: usize,
        /// Original output capacity supplied by the caller.
        capacity: usize,
        /// Bytes already initialized before this attempt.
        initialized: usize,
    },
    /// The original typed crypto or entropy failure, before output publication.
    #[error(transparent)]
    Signing(#[from] super::bls::BlsSigningError),
}

impl ChargedSignature {
    /// Whether the canonical signature retains this exact original allocation pool.
    #[must_use]
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.charge.belongs_to(budget)
    }

    /// Bind an already initialized exact-size payload without allocating or encoding.
    ///
    /// This establishes custody only; it neither parses nor authorizes the bytes.
    /// The same backing and original charge remain inseparable until deallocation.
    ///
    /// # Errors
    /// Returns the unchanged buffer on a foreign pool or a partially filled allocation.
    pub fn try_from_preallocated(
        budget: &AllocationBudget,
        bytes: ChargedBuffer<u8>,
    ) -> Result<Self, (ChargedBuffer<u8>, SignatureAllocationError)> {
        if !bytes.belongs_to(budget) {
            return Err((bytes, SignatureAllocationError::ForeignPool));
        }
        if bytes.as_slice().len() != bytes.capacity() {
            let error = SignatureAllocationError::IncompleteBuffer {
                initialized: bytes.as_slice().len(),
                capacity: bytes.capacity(),
            };
            return Err((bytes, error));
        }
        Ok(Signature::bind_payload_allocation(bytes))
    }
}

#[cfg(feature = "bls")]
impl Signature {
    /// Sign the existing contextual BLS relation into an original prepaid allocation.
    ///
    /// The caller must physically allocate an empty exact-size output before this
    /// call. Successful signing performs no heap allocation and moves that same
    /// backing into the canonical signature. The fixed kernel, checked key parser
    /// and entropy policy are shared with ordinary BLS signing.
    ///
    /// # Errors
    /// Returns the unchanged output owner on unsupported algorithms, wrong geometry,
    /// invalid key bytes or an original entropy failure. No signing randomness is
    /// consumed before algorithm and output geometry validation.
    pub fn try_new_bls_prepaid(
        private_key: &crate::PrivateKey,
        payload: &[u8],
        mut output: ChargedBuffer<u8>,
    ) -> Result<ChargedSignature, (ChargedBuffer<u8>, PrepaidBlsSignatureError)> {
        use crate::secrecy::ExposeSecret;
        let expected = match private_key.algorithm() {
            crate::Algorithm::BlsNormal => 96,
            crate::Algorithm::BlsSmall => 48,
            _ => return Err((output, PrepaidBlsSignatureError::Algorithm)),
        };
        if output.capacity() != expected || !output.as_slice().is_empty() {
            let error = PrepaidBlsSignatureError::OutputGeometry {
                expected,
                capacity: output.capacity(),
                initialized: output.as_slice().len(),
            };
            return Err((output, error));
        }
        let signed = match private_key.0.expose_secret() {
            crate::PrivateKeyInner::BlsNormal(key) => key.try_sign_fixed(payload),
            crate::PrivateKeyInner::BlsSmall(key) => key.try_sign_fixed(payload),
            _ => unreachable!("the algorithm was checked before acquiring any signing input"),
        };
        let signed = match signed {
            Ok(signed) => signed,
            Err(error) => return Err((output, PrepaidBlsSignatureError::Signing(error))),
        };
        output
            .append(signed.as_slice())
            .expect("fixed BLS signature fills exact admitted output");
        Ok(Self::bind_payload_allocation(output))
    }
}

#[cfg(test)]
#[path = "allocation/prepaid_tests.rs"]
mod prepaid_tests;
