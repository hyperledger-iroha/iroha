//! Source-complete commit attestations with one immutable result witness per certificate.

use std::{fmt, sync::Arc};

use mv::allocation::{
    AllocationBudget, AllocationRefusal, ChargedBuffer, ChargedBufferError, ChargedShared,
    PrepaidSharedError,
};
use norito::core as ncore;

/// Largest canonical application result carried once by a flagged certificate.
pub const MAX_RESULT_WITNESS_BYTES: usize = 64 * 1024;
/// Largest compact per-member signature; result bytes have their own shared field.
pub const MAX_ATTESTATION_SIGNATURE_BYTES: usize = 256;

/// A canonical result preimage whose clones share one immutable backing owner.
///
/// Decoding creates explicitly untrusted storage. Production retention requires admission to
/// the original pool, independently of cryptographic validity. No mutable or naked Vec escape
/// exists; the original charged backing and shared control survive every retained clone.
pub struct ResultWitness {
    storage: Storage,
}

enum Storage {
    Untrusted {
        source: Arc<Vec<u8>>,
        pending: Option<ChargedBuffer<u8>>,
    },
    Admitted(ChargedShared<ChargedBuffer<u8>>),
}

/// Typed refusal to retain a result witness from one exact original pool.
#[derive(Debug)]
pub enum WitnessAdmissionError {
    /// Empty or oversized source bytes cannot be a complete result witness.
    Length {
        /// Actual source byte count.
        length: usize,
    },
    /// An existing charged owner belongs to a different original pool.
    ForeignBudget,
    /// Exact byte backing could not be admitted or allocated.
    Buffer(ChargedBufferError),
    /// Original pool refused the exact shared-control layout.
    ControlAdmission(AllocationRefusal),
    /// The already admitted control allocation was refused by the physical allocator.
    ControlAllocation(PrepaidSharedError),
}
impl WitnessAdmissionError {
    /// Whether retrying the unchanged originals after local resource recovery is meaningful.
    #[must_use]
    pub const fn is_local_refusal(&self) -> bool {
        !matches!(self, Self::Length { .. } | Self::ForeignBudget)
    }
}
impl fmt::Display for WitnessAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Length { length } => write!(f, "invalid result witness length {length}"),
            Self::ForeignBudget => f.write_str("result witness belongs to another allocation pool"),
            Self::Buffer(error) => error.fmt(f),
            Self::ControlAdmission(error) => error.fmt(f),
            Self::ControlAllocation(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for WitnessAdmissionError {}

impl ResultWitness {
    /// Retain bounded decoded or fixture bytes without granting production admission.
    ///
    /// # Errors
    /// Rejects empty or oversized bytes before allocating a shared control.
    pub fn from_untrusted(bytes: Vec<u8>) -> Result<Self, WitnessAdmissionError> {
        Self::check_len(bytes.len())?;
        Ok(Self {
            storage: Storage::Untrusted {
                source: Arc::new(bytes),
                pending: None,
            },
        })
    }
    fn check_len(length: usize) -> Result<(), WitnessAdmissionError> {
        if length == 0 || length > MAX_RESULT_WITNESS_BYTES {
            Err(WitnessAdmissionError::Length { length })
        } else {
            Ok(())
        }
    }
    /// Move exact charged backing into an original-pool shared owner.
    ///
    /// # Errors
    /// Every refusal returns the same original buffer, without copying, resizing or dropping it.
    pub fn from_charged(
        bytes: ChargedBuffer<u8>,
        budget: &AllocationBudget,
    ) -> Result<Self, (ChargedBuffer<u8>, WitnessAdmissionError)> {
        if let Err(error) = Self::check_len(bytes.as_slice().len()) {
            return Err((bytes, error));
        }
        if !bytes.belongs_to(budget) {
            return Err((bytes, WitnessAdmissionError::ForeignBudget));
        }
        let mut reservation =
            match budget.try_reserve(ChargedShared::<ChargedBuffer<u8>>::allocation_layout()) {
                Ok(value) => value,
                Err(error) => return Err((bytes, WitnessAdmissionError::ControlAdmission(error))),
            };
        match ChargedShared::from_reservation(bytes, &mut reservation) {
            Ok(owner) => Ok(Self {
                storage: Storage::Admitted(owner),
            }),
            Err((bytes, error)) => Err((bytes, WitnessAdmissionError::ControlAllocation(error))),
        }
    }
    /// Admit this decoded witness in place from the supplied original pool.
    ///
    /// After backing allocation succeeds, any shared-control refusal retains that exact
    /// buffer inside this witness. Retry the same owner; `as_slice` already borrows the
    /// copied backing and keeps the same pointer through final admission. Cloning an
    /// incompletely admitted witness creates an explicitly untrusted source handle and
    /// does not duplicate or transfer its pending allocation. Fully admitted clones share.
    ///
    /// # Errors
    /// Foreign pending or admitted storage is rejected without changing its owners. A local
    /// refusal preserves every allocation already completed inside this original witness.
    pub fn admit(&mut self, budget: &AllocationBudget) -> Result<(), WitnessAdmissionError> {
        if let Storage::Admitted(bytes) = &self.storage {
            return bytes
                .belongs_to(budget)
                .then_some(())
                .ok_or(WitnessAdmissionError::ForeignBudget);
        }
        let Storage::Untrusted { source, pending } = &mut self.storage else {
            unreachable!("admitted owner handled above")
        };
        if let Some(bytes) = pending.as_ref() {
            if !bytes.belongs_to(budget) {
                return Err(WitnessAdmissionError::ForeignBudget);
            }
        } else {
            let mut bytes =
                ChargedBuffer::new(source.len(), budget).map_err(WitnessAdmissionError::Buffer)?;
            bytes
                .append(source.as_slice())
                .expect("exact original-pool byte capacity");
            *pending = Some(bytes);
        }
        let bytes = pending.take().expect("original backing was retained above");
        match Self::from_charged(bytes, budget) {
            Ok(admitted) => {
                self.storage = admitted.storage;
                Ok(())
            }
            Err((bytes, error)) => {
                *pending = Some(bytes);
                Err(error)
            }
        }
    }
    /// Whether backing and shared control belong to this exact original pool.
    #[must_use]
    pub fn admitted_to(&self, budget: &AllocationBudget) -> bool {
        match &self.storage {
            Storage::Untrusted { .. } => false,
            Storage::Admitted(bytes) => bytes.belongs_to(budget),
        }
    }
    /// Borrow the occupied bytes without escaping their immutable allocation owner.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        match &self.storage {
            Storage::Untrusted { source, pending } => pending
                .as_ref()
                .map_or_else(|| source.as_slice(), |bytes| bytes.as_slice()),
            Storage::Admitted(bytes) => bytes.as_slice(),
        }
    }
}
impl Clone for ResultWitness {
    fn clone(&self) -> Self {
        Self {
            storage: match &self.storage {
                Storage::Untrusted { source, .. } => Storage::Untrusted {
                    source: Arc::clone(source),
                    pending: None,
                },
                Storage::Admitted(bytes) => Storage::Admitted(bytes.clone()),
            },
        }
    }
}
impl fmt::Debug for ResultWitness {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ResultWitness")
            .field(&self.as_slice())
            .finish()
    }
}
impl PartialEq for ResultWitness {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}
impl Eq for ResultWitness {}
impl ncore::SerializePayload for ResultWitness {
    fn serialize(&self, encoder: &mut ncore::Encoder<'_>) -> Result<(), ncore::Error> {
        ncore::write_seq_len(encoder, self.as_slice().len() as u64)?;
        encoder.write_all(self.as_slice())?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(8 + self.as_slice().len())
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.encoded_len_hint()
    }
}
impl<'de> ncore::DecodeFromSlice<'de> for ResultWitness {
    fn decode_from_slice(bytes: &'de [u8]) -> Result<(Self, usize), ncore::Error> {
        let (length, prefix) = ncore::read_seq_len_slice(bytes)?;
        if length == 0 || length > MAX_RESULT_WITNESS_BYTES {
            return Err(ncore::Error::FieldLengthExceeded {
                length: length as u64,
                limit: MAX_RESULT_WITNESS_BYTES as u64,
            });
        }
        let used = prefix
            .checked_add(length)
            .ok_or(ncore::Error::LengthMismatch)?;
        let data = bytes
            .get(prefix..used)
            .ok_or(ncore::Error::LengthMismatch)?;
        let value =
            Self::from_untrusted(data.to_vec()).map_err(|_| ncore::Error::LengthMismatch)?;
        ncore::note_payload_access(bytes, used);
        Ok((value, used))
    }
}
impl<'de> ncore::DeserializePayload<'de> for ResultWitness {
    fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("bounded canonical result witness")
    }
    fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let bytes = ncore::payload_slice_from_ptr(std::ptr::from_ref(archived).cast())?;
        <Self as ncore::DecodeFromSlice>::decode_from_slice(bytes).map(|(value, _)| value)
    }
}
impl norito::NoritoSchema for ResultWitness {
    fn nominal_name() -> String {
        "iroha_sumeragi::ResultWitness".to_owned()
    }
    fn static_frame_name() -> Option<&'static str> {
        Some("iroha_sumeragi::ResultWitness")
    }
}

/// Opaque compact application signature with no heap backing or mutable allocation escape.
///
/// The sole canonical codec emits its occupied bytes as one byte sequence. The private unused
/// capacity is always zero and is never encoded. The application verifies every signature against the shared result witness and exact signer.
#[derive(Clone, Copy)]
pub struct AttestationSignature {
    bytes: [u8; MAX_ATTESTATION_SIGNATURE_BYTES],
    len: u16,
}

/// A attestation signature exceeded the fixed protocol capacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttestationSignatureError {
    /// Requested occupied byte length.
    pub length: usize,
}
impl fmt::Display for AttestationSignatureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "attestation signature length {} exceeds {}",
            self.length, MAX_ATTESTATION_SIGNATURE_BYTES
        )
    }
}
impl std::error::Error for AttestationSignatureError {}

impl AttestationSignature {
    /// Empty signature bytes. Application validation must still check whether demand exists.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            bytes: [0; MAX_ATTESTATION_SIGNATURE_BYTES],
            len: 0,
        }
    }

    /// Copy bounded canonical application bytes without allocating.
    ///
    /// # Errors
    /// Rejects lengths above the protocol cap before copying any bytes.
    pub fn try_from_slice(bytes: &[u8]) -> Result<Self, AttestationSignatureError> {
        if bytes.len() > MAX_ATTESTATION_SIGNATURE_BYTES {
            return Err(AttestationSignatureError {
                length: bytes.len(),
            });
        }
        let mut witness = Self::empty();
        witness.bytes[..bytes.len()].copy_from_slice(bytes);
        witness.len = u16::try_from(bytes.len()).expect("protocol bound fits u16");
        Ok(witness)
    }
    /// Occupied canonical application bytes, without the unused capacity.
    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
    /// Whether no application signature bytes are carried.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
    /// Number of occupied bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }
}
impl Default for AttestationSignature {
    fn default() -> Self {
        Self::empty()
    }
}
impl fmt::Debug for AttestationSignature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AttestationSignature")
            .field(&self.as_slice())
            .finish()
    }
}
impl PartialEq for AttestationSignature {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}
impl Eq for AttestationSignature {}
impl std::hash::Hash for AttestationSignature {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(self.as_slice(), state);
    }
}

impl ncore::SerializePayload for AttestationSignature {
    fn serialize(&self, encoder: &mut ncore::Encoder<'_>) -> Result<(), ncore::Error> {
        ncore::write_seq_len(encoder, u64::from(self.len))?;
        encoder.write_all(self.as_slice())?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        Some(8 + self.len())
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        Some(8 + self.len())
    }
}
impl<'de> ncore::DecodeFromSlice<'de> for AttestationSignature {
    fn decode_from_slice(bytes: &'de [u8]) -> Result<(Self, usize), ncore::Error> {
        let (length, prefix) = ncore::read_seq_len_slice(bytes)?;
        if length > MAX_ATTESTATION_SIGNATURE_BYTES {
            return Err(ncore::Error::FieldLengthExceeded {
                length: u64::try_from(length).unwrap_or(u64::MAX),
                limit: MAX_ATTESTATION_SIGNATURE_BYTES as u64,
            });
        }
        let used = prefix
            .checked_add(length)
            .ok_or(ncore::Error::LengthMismatch)?;
        let data = bytes
            .get(prefix..used)
            .ok_or(ncore::Error::LengthMismatch)?;
        let witness = Self::try_from_slice(data).map_err(|_| ncore::Error::LengthMismatch)?;
        ncore::note_payload_access(bytes, used);
        Ok((witness, used))
    }
}
impl<'de> ncore::DeserializePayload<'de> for AttestationSignature {
    fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("canonical bounded attestation signature")
    }
    fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        let bytes = ncore::payload_slice_from_ptr(std::ptr::from_ref(archived).cast())?;
        <Self as ncore::DecodeFromSlice>::decode_from_slice(bytes).map(|(witness, _)| witness)
    }
}
impl norito::NoritoSchema for AttestationSignature {
    fn nominal_name() -> String {
        "iroha_sumeragi::AttestationSignature".to_owned()
    }
    fn static_frame_name() -> Option<&'static str> {
        Some("iroha_sumeragi::AttestationSignature")
    }
}

/// One source-complete commit share. A certificate keeps its witness only once.
#[derive(Clone, Debug, PartialEq, Eq, norito::Encode, norito::Decode)]
pub struct CommitAttestation {
    /// Canonical preimage of the exact signed result R.
    pub witness: ResultWitness,
    /// Compact application signature under the member's authenticated authority.
    pub signature: AttestationSignature,
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::codec::{DecodeAll as _, Encode as _};

    #[test]
    fn source_admission_clone_and_final_refund_preserve_actual_backing() {
        let budget = AllocationBudget::new(1 << 20);
        let foreign = AllocationBudget::new(1 << 20);
        let mut bytes = ChargedBuffer::new(3, &budget).unwrap();
        bytes.append(&[1, 2, 3]).unwrap();
        let pointer = bytes.as_slice().as_ptr();
        let value = ResultWitness::from_charged(bytes, &budget)
            .unwrap_or_else(|(_, error)| panic!("funded witness rejected: {error:?}"));
        let reserved = budget.reserved_bytes();
        assert!(reserved > 3);
        assert!(value.admitted_to(&budget));
        assert!(!value.admitted_to(&foreign));
        assert_eq!(pointer, value.as_slice().as_ptr());
        let mut clone = value.clone();
        assert_eq!(clone.as_slice().as_ptr(), pointer);
        assert_eq!(budget.reserved_bytes(), reserved);
        assert!(matches!(
            clone.admit(&foreign),
            Err(WitnessAdmissionError::ForeignBudget)
        ));
        drop(value);
        assert_eq!(budget.reserved_bytes(), reserved);
        assert_eq!(clone.as_slice(), &[1, 2, 3]);
        drop(clone);
        assert_eq!(budget.reserved_bytes(), 0);
    }
    #[test]
    fn refused_control_returns_same_original_buffer_for_retry() {
        let budget = AllocationBudget::new(3);
        let mut bytes = ChargedBuffer::new(3, &budget).unwrap();
        bytes.append(&[1, 2, 3]).unwrap();
        let pointer = bytes.as_slice().as_ptr();
        let (bytes, error) = ResultWitness::from_charged(bytes, &budget).unwrap_err();
        assert!(matches!(error, WitnessAdmissionError::ControlAdmission(_)));
        assert_eq!(bytes.as_slice().as_ptr(), pointer);
        assert_eq!(budget.reserved_bytes(), 3);
        drop(bytes);
        assert_eq!(budget.reserved_bytes(), 0);
    }
    #[test]
    fn in_place_control_refusal_keeps_original_copy_across_retry_and_untrusted_clone() {
        let budget = AllocationBudget::new(4096);
        let occupied = ChargedBuffer::<u8>::new(3896, &budget).unwrap();
        let mut witness = ResultWitness::from_untrusted(vec![7; 200]).unwrap();
        let source_pointer = witness.as_slice().as_ptr();
        let canonical = witness.encode();
        assert!(matches!(
            witness.admit(&budget),
            Err(WitnessAdmissionError::ControlAdmission(_))
        ));
        let original_copy = witness.as_slice().as_ptr();
        assert_ne!(original_copy, source_pointer);
        assert_eq!(budget.reserved_bytes(), 4096);
        assert!(!witness.admitted_to(&budget));
        assert_eq!(witness.encode(), canonical);
        let clone = witness.clone();
        assert_eq!(clone.as_slice().as_ptr(), source_pointer);
        assert!(!clone.admitted_to(&budget));
        drop(clone);
        assert_eq!(budget.reserved_bytes(), 4096);
        let foreign = AllocationBudget::new(4096);
        assert!(matches!(
            witness.admit(&foreign),
            Err(WitnessAdmissionError::ForeignBudget)
        ));
        assert_eq!(foreign.reserved_bytes(), 0);
        assert_eq!(witness.as_slice().as_ptr(), original_copy);
        assert!(witness.admit(&budget).unwrap_err().is_local_refusal());
        assert_eq!(witness.as_slice().as_ptr(), original_copy);
        drop(occupied);
        witness.admit(&budget).unwrap();
        assert!(witness.admitted_to(&budget));
        assert_eq!(witness.as_slice().as_ptr(), original_copy);
        assert_eq!(witness.encode(), canonical);
        let clone = witness.clone();
        assert_eq!(clone.as_slice().as_ptr(), original_copy);
        let charged = budget.reserved_bytes();
        witness.admit(&budget).unwrap();
        assert_eq!(budget.reserved_bytes(), charged);
        drop(witness);
        assert_eq!(budget.reserved_bytes(), charged);
        drop(clone);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn dropping_pending_admission_frees_only_its_actual_original_backing() {
        let budget = AllocationBudget::new(200);
        let mut witness = ResultWitness::from_untrusted(vec![1; 200]).unwrap();
        assert!(matches!(
            witness.admit(&budget),
            Err(WitnessAdmissionError::ControlAdmission(_))
        ));
        let clone = witness.clone();
        assert_eq!(budget.reserved_bytes(), 200);
        drop(witness);
        assert_eq!(budget.reserved_bytes(), 0);
        assert_eq!(clone.as_slice(), &[1; 200]);
        assert!(!clone.admitted_to(&budget));
    }

    #[test]
    fn sole_canonical_byte_sequence_never_encodes_admission_state() {
        let budget = AllocationBudget::new(1 << 20);
        let bytes = vec![9; MAX_RESULT_WITNESS_BYTES];
        let untrusted = ResultWitness::from_untrusted(bytes.clone()).unwrap();
        let mut admitted = untrusted.clone();
        admitted.admit(&budget).unwrap();
        let encoded = admitted.encode();
        assert_eq!(encoded, bytes.encode());
        assert_eq!(encoded, untrusted.encode());
        let decoded = ResultWitness::decode_all(&mut encoded.as_slice()).unwrap();
        assert_eq!(decoded, admitted);
        assert!(!decoded.admitted_to(&budget));
        assert!(
            ResultWitness::decode_all(
                &mut [u64::MAX.to_le_bytes().as_slice(), &[1]]
                    .concat()
                    .as_slice()
            )
            .is_err()
        );
        assert!(
            ResultWitness::decode_all(
                &mut (MAX_RESULT_WITNESS_BYTES as u64 + 1)
                    .to_le_bytes()
                    .as_slice()
            )
            .is_err()
        );
        assert!(ResultWitness::decode_all(&mut 0_u64.to_le_bytes().as_slice()).is_err());
        assert!(ResultWitness::decode_all(&mut 3_u64.to_le_bytes().as_slice()).is_err());
    }
}
