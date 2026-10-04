//! Immutable Sumeragi certificate bytes and their original allocation custody.
//!
//! The sole canonical record contains the consensus header, CommitQC, result preimage and original availability frame.
//! Runtime ownership is never encoded. Ordinary decoding produces untrusted bytes; publication requires
//! explicit admission against the original execution budget. Shared clones keep each exact
//! backing allocation and its prepaid control allocation alive through the last retained block.

use std::{
    cmp::Ordering,
    fmt,
    hash::{Hash, Hasher},
    sync::Arc,
};

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_allocation::{
    AllocationBudget, AllocationRefusal, ChargedBuffer, ChargedBufferError, ChargedShared,
    PrepaidSharedError,
};
use iroha_schema::{Declaration, IntoSchema, MetaMap, Metadata, NamedFieldsMeta, TypeId};
use norito::{
    codec::{Decode, Encode},
    core as ncore,
};

/// Finality artifacts of one block, stored as immutable canonical bytes.
///
/// Admission proves resource custody only. The consensus verifier must independently verify
/// the header, exact quorum, result and original availability signatures. Genesis stores a result-only artifact whose unsigned
/// execution result becomes authenticated only by a verified successor's parent-result link.
/// Decoding and [`Self::from_untrusted_parts`] never authorize production publication.
pub struct CommitCertificate {
    storage: Storage,
}

enum Storage {
    Untrusted(Arc<CanonicalParts>),
    Admitted(ChargedShared<ChargedCertificateParts>),
}

// Exactly the canonical four-field record. No storage-state tag is serialized or accepted.
#[derive(Encode, Decode, DeriveJsonSerialize, DeriveJsonDeserialize)]
#[norito(deny_unknown_fields, decode_fields)]
struct CanonicalParts {
    #[norito(
        with = "crate::json_helpers::base64_vec",
        bounded_with = "crate::json_helpers::base64_vec::serialize_bounded"
    )]
    consensus_header: Vec<u8>,
    #[norito(
        with = "crate::json_helpers::base64_vec",
        bounded_with = "crate::json_helpers::base64_vec::serialize_bounded"
    )]
    commit_qc: Vec<u8>,
    #[norito(
        with = "crate::json_helpers::base64_vec",
        bounded_with = "crate::json_helpers::base64_vec::serialize_bounded"
    )]
    result_preimage: Vec<u8>,
    #[norito(
        with = "crate::json_helpers::base64_vec",
        bounded_with = "crate::json_helpers::base64_vec::serialize_bounded"
    )]
    availability: Vec<u8>,
}

mod prepared;
pub use prepared::{CertificateCustodyError, PreparedCommitCertificate};

impl<D> ncore::DecodeRecordFields<D> for CommitCertificate
where
    D: ncore::FieldDestination
        + ncore::DecodeField<0, Vec<u8>>
        + ncore::DecodeField<1, Vec<u8>>
        + ncore::DecodeField<2, Vec<u8>>
        + ncore::DecodeField<3, Vec<u8>>,
{
    // Spell the destination outputs directly; the canonical storage record stays private.
    type Values = (
        <D as ncore::DecodeField<0, Vec<u8>>>::Value,
        <D as ncore::DecodeField<1, Vec<u8>>>::Value,
        <D as ncore::DecodeField<2, Vec<u8>>>::Value,
        <D as ncore::DecodeField<3, Vec<u8>>>::Value,
    );
    fn decode_fields(
        bytes: &[u8],
        destination: &mut D,
    ) -> Result<(Self::Values, usize), ncore::DecodeIntoError<D::Error>> {
        <CanonicalParts as ncore::DecodeRecordFields<D>>::decode_fields(bytes, destination)
    }
}

/// Original fixed allocations awaiting one immutable certificate control owner.
///
/// A failed constructor returns this same value, preserving every backing pointer and credit.
/// These buffers may be retained for a retry; extracting them does not strip their charges.
pub struct ChargedCertificateParts {
    /// Canonical consensus header bytes, admitted by the original execution pool.
    pub consensus_header: ChargedBuffer<u8>,
    /// Canonical exact-quorum certificate bytes from that same pool.
    pub commit_qc: ChargedBuffer<u8>,
    /// Original canonical execution-result preimage from that same pool.
    pub result_preimage: ChargedBuffer<u8>,
    /// Complete canonical original signed-availability frame from that same pool.
    pub availability: ChargedBuffer<u8>,
}

impl ChargedCertificateParts {
    /// Whether all original backing allocations belong to the supplied pool.
    #[must_use]
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.consensus_header.belongs_to(budget)
            && self.commit_qc.belongs_to(budget)
            && self.result_preimage.belongs_to(budget)
            && self.availability.belongs_to(budget)
    }
}

impl fmt::Debug for ChargedCertificateParts {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ChargedCertificateParts")
            .field("consensus_header", &self.consensus_header.as_slice())
            .field("commit_qc", &self.commit_qc.as_slice())
            .field("result_preimage", &self.result_preimage.as_slice())
            .field("availability", &self.availability.as_slice())
            .finish()
    }
}

/// Typed refusal to admit certificate storage; no implicit replacement pool is created.
#[derive(Debug)]
pub enum CertificateAdmissionError {
    /// An input buffer or already admitted certificate belongs to a different pool.
    ForeignBudget,
    /// Copying explicitly untrusted bytes could not obtain original-pool backing storage.
    Buffer(ChargedBufferError),
    /// The original pool refused the exact shared-control layout before allocation.
    ControlAdmission(AllocationRefusal),
    /// The admitted shared-control allocation could not be completed.
    ControlAllocation(PrepaidSharedError),
}
impl CertificateAdmissionError {
    /// Whether retrying the unchanged originals after local recovery is meaningful.
    #[must_use]
    pub const fn is_local_refusal(&self) -> bool {
        !matches!(self, Self::ForeignBudget)
    }
}
impl fmt::Display for CertificateAdmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ForeignBudget => {
                f.write_str("certificate bytes differ from original execution pool")
            }
            Self::Buffer(error) => error.fmt(f),
            Self::ControlAdmission(error) => error.fmt(f),
            Self::ControlAllocation(error) => error.fmt(f),
        }
    }
}
impl std::error::Error for CertificateAdmissionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::ForeignBudget => None,
            Self::Buffer(error) => Some(error),
            Self::ControlAdmission(error) => Some(error),
            Self::ControlAllocation(error) => Some(error),
        }
    }
}

impl CommitCertificate {
    /// Take explicitly untrusted canonical artifacts without granting publication admission.
    /// This constructor is for decoded/offchain fixtures; production uses charged originals.
    #[must_use]
    pub fn from_untrusted_parts(
        consensus_header: Vec<u8>,
        commit_qc: Vec<u8>,
        result_preimage: Vec<u8>,
        availability: Vec<u8>,
    ) -> Self {
        Self {
            storage: Storage::Untrusted(Arc::new(CanonicalParts {
                consensus_header,
                commit_qc,
                result_preimage,
                availability,
            })),
        }
    }

    /// Move actual original charged buffers into one prepaid shared immutable owner.
    ///
    /// # Errors
    /// Returns all four original buffers unchanged on a source, admission or allocator refusal.
    /// Retain these buffers for retry; do not rerun execution or reconstruct the result preimage.
    #[expect(
        clippy::result_large_err,
        reason = "refusal returns all original funded buffers without allocating"
    )]
    pub fn from_charged_parts(
        consensus_header: ChargedBuffer<u8>,
        commit_qc: ChargedBuffer<u8>,
        result_preimage: ChargedBuffer<u8>,
        availability: ChargedBuffer<u8>,
        budget: &AllocationBudget,
    ) -> Result<Self, (ChargedCertificateParts, CertificateAdmissionError)> {
        Self::from_charged_owner(
            ChargedCertificateParts {
                consensus_header,
                commit_qc,
                result_preimage,
                availability,
            },
            budget,
        )
    }

    /// Retry shared-control admission with the unchanged original buffer owner.
    ///
    /// # Errors
    /// Returns that owner on every failure, without copying, dropping or resizing any buffer.
    #[expect(
        clippy::result_large_err,
        reason = "refusal returns the original funded owner without allocating"
    )]
    pub fn from_charged_owner(
        parts: ChargedCertificateParts,
        budget: &AllocationBudget,
    ) -> Result<Self, (ChargedCertificateParts, CertificateAdmissionError)> {
        if !parts.belongs_to(budget) {
            return Err((parts, CertificateAdmissionError::ForeignBudget));
        }
        let mut reservation = match budget
            .try_reserve(ChargedShared::<ChargedCertificateParts>::allocation_layout())
        {
            Ok(reservation) => reservation,
            Err(error) => return Err((parts, CertificateAdmissionError::ControlAdmission(error))),
        };
        match ChargedShared::from_reservation(parts, &mut reservation) {
            Ok(owner) => Ok(Self {
                storage: Storage::Admitted(owner),
            }),
            Err((parts, error)) => {
                Err((parts, CertificateAdmissionError::ControlAllocation(error)))
            }
        }
    }

    /// Explicitly admit decoded bytes using the supplied original execution pool.
    ///
    /// Untrusted bytes are copied into new exact charged backing and charged shared control.
    /// An already admitted certificate from this pool is shared without any allocation. An
    /// admitted certificate from another pool is rejected, never silently recharged or copied.
    ///
    /// # Errors
    /// Returns a typed source/admission/allocator refusal. The borrowed source remains intact.
    pub fn admit(&self, budget: &AllocationBudget) -> Result<Self, CertificateAdmissionError> {
        fn copy(
            bytes: &[u8],
            budget: &AllocationBudget,
        ) -> Result<ChargedBuffer<u8>, CertificateAdmissionError> {
            let mut buffer = ChargedBuffer::new(bytes.len(), budget)
                .map_err(CertificateAdmissionError::Buffer)?;
            buffer
                .append(bytes)
                .expect("exact byte capacity was admitted before copying");
            Ok(buffer)
        }
        if matches!(self.storage, Storage::Admitted(_)) {
            return self
                .admitted_to(budget)
                .then(|| self.clone())
                .ok_or(CertificateAdmissionError::ForeignBudget);
        }
        let header = copy(self.consensus_header(), budget)?;
        let qc = copy(self.commit_qc(), budget)?;
        let preimage = copy(self.result_preimage(), budget)?;
        let availability = copy(self.availability(), budget)?;
        Self::from_charged_parts(header, qc, preimage, availability, budget)
            .map_err(|(_parts, error)| error)
    }

    /// Whether this immutable owner retains original backing and control from this exact pool.
    /// Decoded untrusted storage always returns false, including for an otherwise equal pool.
    #[must_use]
    pub fn admitted_to(&self, budget: &AllocationBudget) -> bool {
        match &self.storage {
            Storage::Untrusted(_) => false,
            // Only from_charged_owner constructs this state, checking every backing source
            // before allocating this control from the very same supplied budget.
            Storage::Admitted(parts) => parts.belongs_to(budget),
        }
    }

    /// Whether both admitted values retain the same immutable original allocation.
    /// Equal untrusted bytes never establish physical custody identity.
    #[must_use]
    pub fn ptr_eq(left: &Self, right: &Self) -> bool {
        match (&left.storage, &right.storage) {
            (Storage::Admitted(left), Storage::Admitted(right)) => {
                ChargedShared::ptr_eq(left, right)
            }
            _ => false,
        }
    }

    /// Borrow canonical Sumeragi header bytes; no mutable or naked allocation escape exists.
    #[must_use]
    pub fn consensus_header(&self) -> &[u8] {
        match &self.storage {
            Storage::Untrusted(parts) => &parts.consensus_header,
            Storage::Admitted(parts) => parts.consensus_header.as_slice(),
        }
    }
    /// Borrow canonical exact-quorum certificate bytes.
    #[must_use]
    pub fn commit_qc(&self) -> &[u8] {
        match &self.storage {
            Storage::Untrusted(parts) => &parts.commit_qc,
            Storage::Admitted(parts) => parts.commit_qc.as_slice(),
        }
    }
    /// Borrow the canonical certified execution-result preimage.
    #[must_use]
    pub fn result_preimage(&self) -> &[u8] {
        match &self.storage {
            Storage::Untrusted(parts) => &parts.result_preimage,
            Storage::Admitted(parts) => parts.result_preimage.as_slice(),
        }
    }
    /// Borrow the mandatory canonical original signed-availability frame.
    /// Only the exact result-only genesis artifact has an empty frame; semantic verifiers
    /// enforce that exception against the independently authenticated genesis identity.
    #[must_use]
    pub fn availability(&self) -> &[u8] {
        match &self.storage {
            Storage::Untrusted(parts) => &parts.availability,
            Storage::Admitted(parts) => parts.availability.as_slice(),
        }
    }
    /// Total opaque payload byte count, excluding the containing canonical record frame.
    #[must_use]
    pub fn payload_len(&self) -> usize {
        self.consensus_header()
            .len()
            .saturating_add(self.commit_qc().len())
            .saturating_add(self.result_preimage().len())
            .saturating_add(self.availability().len())
    }
    fn parts(&self) -> (&[u8], &[u8], &[u8], &[u8]) {
        (
            self.consensus_header(),
            self.commit_qc(),
            self.result_preimage(),
            self.availability(),
        )
    }
    fn binary(&self) -> BinaryRef<'_> {
        BinaryRef {
            consensus_header: ByteSequence(self.consensus_header()),
            commit_qc: ByteSequence(self.commit_qc()),
            result_preimage: ByteSequence(self.result_preimage()),
            availability: ByteSequence(self.availability()),
        }
    }
    fn json(&self) -> JsonRef<'_> {
        JsonRef {
            consensus_header: self.consensus_header(),
            commit_qc: self.commit_qc(),
            result_preimage: self.result_preimage(),
            availability: self.availability(),
        }
    }
    fn from_decoded(parts: CanonicalParts) -> Result<Self, ncore::Error> {
        ncore::reserve_decode_arc_allocation::<CanonicalParts>()?;
        Ok(Self {
            storage: Storage::Untrusted(Arc::new(parts)),
        })
    }
}

impl Clone for CommitCertificate {
    fn clone(&self) -> Self {
        Self {
            storage: match &self.storage {
                Storage::Untrusted(parts) => Storage::Untrusted(Arc::clone(parts)),
                Storage::Admitted(parts) => Storage::Admitted(parts.clone()),
            },
        }
    }
}
impl fmt::Debug for CommitCertificate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CommitCertificate")
            .field("consensus_header", &self.consensus_header())
            .field("commit_qc", &self.commit_qc())
            .field("result_preimage", &self.result_preimage())
            .field("availability", &self.availability())
            .finish()
    }
}
impl PartialEq for CommitCertificate {
    fn eq(&self, other: &Self) -> bool {
        self.parts() == other.parts()
    }
}
impl Eq for CommitCertificate {}
impl PartialOrd for CommitCertificate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for CommitCertificate {
    fn cmp(&self, other: &Self) -> Ordering {
        self.parts().cmp(&other.parts())
    }
}
impl Hash for CommitCertificate {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.parts().hash(state);
    }
}

// Vec<u8>'s exact byte sequence payload, borrowed from either runtime owner.
struct ByteSequence<'a>(&'a [u8]);
impl ncore::SerializePayload for ByteSequence<'_> {
    fn serialize(&self, encoder: &mut ncore::Encoder<'_>) -> Result<(), ncore::Error> {
        ncore::write_seq_len(
            encoder,
            u64::try_from(self.0.len()).map_err(|_| ncore::Error::LengthMismatch)?,
        )?;
        encoder.write_all(self.0)?;
        Ok(())
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        self.0.len().checked_add(8)
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        self.0.len().checked_add(8)
    }
}
#[derive(Encode)]
struct BinaryRef<'a> {
    consensus_header: ByteSequence<'a>,
    commit_qc: ByteSequence<'a>,
    result_preimage: ByteSequence<'a>,
    availability: ByteSequence<'a>,
}
#[derive(DeriveJsonSerialize)]
struct JsonRef<'a> {
    #[norito(
        with = "crate::json_helpers::base64_vec",
        bounded_with = "crate::json_helpers::base64_vec::serialize_bounded"
    )]
    consensus_header: &'a [u8],
    #[norito(
        with = "crate::json_helpers::base64_vec",
        bounded_with = "crate::json_helpers::base64_vec::serialize_bounded"
    )]
    commit_qc: &'a [u8],
    #[norito(
        with = "crate::json_helpers::base64_vec",
        bounded_with = "crate::json_helpers::base64_vec::serialize_bounded"
    )]
    result_preimage: &'a [u8],
    #[norito(
        with = "crate::json_helpers::base64_vec",
        bounded_with = "crate::json_helpers::base64_vec::serialize_bounded"
    )]
    availability: &'a [u8],
}
impl ncore::SerializePayload for CommitCertificate {
    fn serialize(&self, encoder: &mut ncore::Encoder<'_>) -> Result<(), ncore::Error> {
        ncore::SerializePayload::serialize(&self.binary(), encoder)
    }
    fn encoded_len_hint(&self) -> Option<usize> {
        ncore::SerializePayload::encoded_len_hint(&self.binary())
    }
    fn encoded_len_exact(&self) -> Option<usize> {
        ncore::SerializePayload::encoded_len_exact(&self.binary())
    }
}
impl<'de> ncore::DeserializePayload<'de> for CommitCertificate {
    fn deserialize(archived: &'de ncore::Archived<Self>) -> Self {
        Self::try_deserialize(archived).expect("canonical certificate archive")
    }
    fn try_deserialize(archived: &'de ncore::Archived<Self>) -> Result<Self, ncore::Error> {
        // Archived<T> is an opaque address marker; the sole accepted payload is the
        // canonical four-field record, never the private runtime Storage representation.
        Self::from_decoded(
            <CanonicalParts as ncore::DeserializePayload>::try_deserialize(archived.cast())?,
        )
    }
}
impl norito::NoritoSchema for CommitCertificate {
    fn nominal_name() -> String {
        Self::static_frame_name()
            .expect("fixed identity")
            .to_owned()
    }
    fn static_frame_name() -> Option<&'static str> {
        Some("iroha_data_model::block::commit_certificate::CommitCertificate")
    }
}
impl TypeId for CommitCertificate {
    fn id() -> String {
        "CommitCertificate".to_owned()
    }
}
impl IntoSchema for CommitCertificate {
    fn type_name() -> String {
        "CommitCertificate".to_owned()
    }
    fn update_schema_map(map: &mut MetaMap) {
        if !map.contains_key::<Self>() {
            map.insert::<Self>(Metadata::Struct(NamedFieldsMeta {
                declarations: [
                    "consensus_header",
                    "commit_qc",
                    "result_preimage",
                    "availability",
                ]
                .into_iter()
                .map(|name| Declaration {
                    name: name.to_owned(),
                    ty: std::any::TypeId::of::<Vec<u8>>(),
                })
                .collect(),
            }));
            Vec::<u8>::update_schema_map(map);
        }
    }
}
impl norito::json::FastJsonWrite for CommitCertificate {
    fn json_object_field_order() -> Option<&'static [&'static str]> {
        Some(&[
            "consensus_header",
            "commit_qc",
            "result_preimage",
            "availability",
        ])
    }
    fn write_json(&self, out: &mut String) {
        norito::json::FastJsonWrite::write_json(&self.json(), out);
    }
    fn write_json_to(
        &self,
        out: &mut dyn norito::json::JsonWriteSink,
    ) -> Result<(), norito::json::BoundedJsonError> {
        norito::json::FastJsonWrite::write_json_to(&self.json(), out)
    }
}
impl norito::json::JsonDeserialize for CommitCertificate {
    fn json_deserialize(
        parser: &mut norito::json::Parser<'_>,
    ) -> Result<Self, norito::json::Error> {
        let parts = <CanonicalParts as norito::json::JsonDeserialize>::json_deserialize(parser)?;
        Self::from_decoded(parts).map_err(|error| norito::json::Error::Message(error.to_string()))
    }
    fn json_from_value(value: &norito::json::Value) -> Result<Self, norito::json::Error> {
        let parts = <CanonicalParts as norito::json::JsonDeserialize>::json_from_value(value)?;
        Self::from_decoded(parts).map_err(|error| norito::json::Error::Message(error.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use norito::codec::DecodeAll as _;

    fn sample() -> CommitCertificate {
        CommitCertificate::from_untrusted_parts(vec![1, 2, 3], vec![4, 5], vec![6; 40], vec![7; 4])
    }
    fn buffer(bytes: &[u8], budget: &AllocationBudget) -> ChargedBuffer<u8> {
        let mut value = ChargedBuffer::new(bytes.len(), budget).unwrap();
        value.append(bytes).unwrap();
        value
    }
    fn parts(budget: &AllocationBudget) -> ChargedCertificateParts {
        ChargedCertificateParts {
            consensus_header: buffer(&[1, 2, 3], budget),
            commit_qc: buffer(&[4, 5], budget),
            result_preimage: buffer(&[6; 40], budget),
            availability: buffer(&[7; 4], budget),
        }
    }
    fn control_size() -> usize {
        ChargedShared::<ChargedCertificateParts>::allocation_layout().size()
    }

    #[test]
    fn untrusted_constructor_preserves_order_and_never_admits() {
        let cert = sample();
        assert_eq!(cert.consensus_header(), &[1, 2, 3]);
        assert_eq!(cert.commit_qc(), &[4, 5]);
        assert_eq!(cert.result_preimage(), &[6; 40]);
        assert_eq!(cert.availability(), &[7; 4]);
        assert!(!cert.admitted_to(&AllocationBudget::new(usize::MAX)));
        let clone = cert.clone();
        assert_eq!(
            clone.result_preimage().as_ptr(),
            cert.result_preimage().as_ptr()
        );
    }
    #[test]
    fn payload_len_sums_all_parts() {
        assert_eq!(sample().payload_len(), 49);
        assert_eq!(
            CommitCertificate::from_untrusted_parts(Vec::new(), Vec::new(), Vec::new(), Vec::new())
                .payload_len(),
            0
        );
    }
    #[test]
    fn shared_charged_constructor_keeps_original_pointers_until_last_clone() {
        let budget = AllocationBudget::new(49 + control_size());
        let owner = parts(&budget);
        let pointers = (
            owner.consensus_header.as_slice().as_ptr(),
            owner.commit_qc.as_slice().as_ptr(),
            owner.result_preimage.as_slice().as_ptr(),
            owner.availability.as_slice().as_ptr(),
        );
        let cert = CommitCertificate::from_charged_owner(owner, &budget).unwrap();
        assert_eq!(
            (
                cert.consensus_header().as_ptr(),
                cert.commit_qc().as_ptr(),
                cert.result_preimage().as_ptr(),
                cert.availability().as_ptr()
            ),
            pointers
        );
        assert!(cert.admitted_to(&budget.clone()));
        assert_eq!(budget.reserved_bytes(), 49 + control_size());
        let clone = cert.clone();
        let shared = cert.admit(&budget).unwrap();
        assert_eq!(clone.result_preimage().as_ptr(), pointers.2);
        assert_eq!(shared.result_preimage().as_ptr(), pointers.2);
        drop(cert);
        drop(clone);
        assert_eq!(budget.reserved_bytes(), 49 + control_size());
        drop(shared);
        assert_eq!(budget.reserved_bytes(), 0);
    }
    #[test]
    fn control_refusal_returns_original_parts_for_retry_without_refund() {
        let budget = AllocationBudget::new(49 + control_size());
        let owner = parts(&budget);
        let pointer = owner.result_preimage.as_slice().as_ptr();
        let availability_pointer = owner.availability.as_slice().as_ptr();
        let occupied = buffer(&[7], &budget);
        let (returned, error) = CommitCertificate::from_charged_owner(owner, &budget).unwrap_err();
        assert!(matches!(
            error,
            CertificateAdmissionError::ControlAdmission(AllocationRefusal::Capacity { .. })
        ));
        assert!(error.is_local_refusal());
        assert_eq!(returned.result_preimage.as_slice().as_ptr(), pointer);
        assert_eq!(
            returned.availability.as_slice().as_ptr(),
            availability_pointer
        );
        assert_eq!(budget.reserved_bytes(), 50);
        drop(occupied);
        let cert = CommitCertificate::from_charged_owner(returned, &budget).unwrap();
        assert_eq!(cert.result_preimage().as_ptr(), pointer);
        assert_eq!(cert.availability().as_ptr(), availability_pointer);
        drop(cert);
        assert_eq!(budget.reserved_bytes(), 0);
    }
    #[test]
    fn foreign_pool_and_mixed_sources_cannot_authorize_publication() {
        let budget = AllocationBudget::new(4096);
        let foreign = AllocationBudget::new(4096);
        let mut owner = parts(&budget);
        let original = std::mem::replace(&mut owner.commit_qc, buffer(&[4, 5], &foreign));
        let (mut returned, error) =
            CommitCertificate::from_charged_owner(owner, &budget).unwrap_err();
        assert!(matches!(error, CertificateAdmissionError::ForeignBudget));
        assert!(!error.is_local_refusal());
        assert_eq!(budget.reserved_bytes(), 49);
        returned.commit_qc = original;
        assert_eq!(foreign.reserved_bytes(), 0);
        let cert = CommitCertificate::from_charged_owner(returned, &budget).unwrap();
        assert!(!cert.admitted_to(&foreign));
        assert!(matches!(
            cert.admit(&foreign),
            Err(CertificateAdmissionError::ForeignBudget)
        ));
        assert_eq!(foreign.reserved_bytes(), 0);
    }
    #[test]
    fn explicit_untrusted_admission_copies_to_charged_immutable_storage() {
        let budget = AllocationBudget::new(49 + control_size());
        let decoded = sample();
        let admitted = decoded.admit(&budget).unwrap();
        assert_eq!(admitted, decoded);
        assert_ne!(
            admitted.result_preimage().as_ptr(),
            decoded.result_preimage().as_ptr()
        );
        assert!(admitted.admitted_to(&budget));
        assert!(!decoded.admitted_to(&budget));
        drop(admitted);
        assert_eq!(budget.reserved_bytes(), 0);
    }
    #[test]
    fn untrusted_admission_refusal_preserves_source_and_refunds_partial_copy() {
        let budget = AllocationBudget::new(48);
        let decoded = sample();
        let pointer = decoded.result_preimage().as_ptr();
        assert!(matches!(
            decoded.admit(&budget),
            Err(CertificateAdmissionError::Buffer(_))
        ));
        assert_eq!(budget.reserved_bytes(), 0);
        assert_eq!(decoded.result_preimage().as_ptr(), pointer);
    }
    #[test]
    fn codec_round_trip_decodes_untrusted_and_has_one_four_field_layout() {
        let cert = sample();
        let bytes = cert.encode();
        let decoded = CommitCertificate::decode_all(&mut bytes.as_slice()).expect("decode");
        assert_eq!(decoded, cert);
        let framed = norito::to_bytes(&cert).expect("framed encode");
        let decoded: CommitCertificate = norito::decode_from_bytes(&framed).expect("framed decode");
        assert_eq!(decoded, cert);
        let budget = AllocationBudget::new(4096);
        let admitted = cert.admit(&budget).unwrap();
        assert!(!decoded.admitted_to(&budget));
        assert_eq!(admitted.encode(), bytes);
        assert_eq!(norito::to_bytes(&admitted).unwrap(), framed);
        let reference = wire::CommitCertificate {
            consensus_header: vec![1, 2, 3],
            commit_qc: vec![4, 5],
            result_preimage: vec![6; 40],
            availability: vec![7; 4],
        };
        assert_eq!(bytes, reference.encode());
        assert_eq!(framed, norito::to_bytes(&reference).unwrap());
        let mut schema = MetaMap::new();
        CommitCertificate::update_schema_map(&mut schema);
        wire::CommitCertificate::update_schema_map(&mut schema);
        assert_eq!(
            schema.get::<CommitCertificate>(),
            schema.get::<wire::CommitCertificate>()
        );
        assert_eq!(
            <CommitCertificate as TypeId>::id(),
            <wire::CommitCertificate as TypeId>::id()
        );
        assert_eq!(
            <CommitCertificate as IntoSchema>::type_name(),
            <wire::CommitCertificate as IntoSchema>::type_name()
        );
    }
    #[test]
    fn json_round_trip_uses_base64_and_bounded_writer() {
        let cert = sample();
        let json = norito::json::to_json(&cert).expect("json");
        assert_eq!(
            json,
            r#"{"consensus_header":"AQID","commit_qc":"BAU=","result_preimage":"BgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBgYGBg==","availability":"BwcHBw=="}"#
        );
        assert_eq!(
            norito::json::to_json_bounded(&cert, json.len()).unwrap(),
            json
        );
        assert!(norito::json::to_json_bounded(&cert, json.len() - 1).is_err());
        let parsed: CommitCertificate = norito::json::from_str(&json).expect("parse");
        assert_eq!(parsed, cert);
        assert!(
            norito::json::from_str::<CommitCertificate>(
                r#"{"consensus_header":"","commit_qc":"","result_preimage":"","availability":"","extra":1}"#
            )
            .is_err()
        );
        let admitted = cert.admit(&AllocationBudget::new(4096)).unwrap();
        assert_eq!(norito::json::to_json(&admitted).unwrap(), json);
        let value = norito::json::from_str::<norito::json::Value>(&json).unwrap();
        assert_eq!(
            <CommitCertificate as norito::json::JsonDeserialize>::json_from_value(&value).unwrap(),
            cert
        );
        assert_eq!(
            <CommitCertificate as norito::json::FastJsonWrite>::json_object_field_order(),
            Some(
                &[
                    "consensus_header",
                    "commit_qc",
                    "result_preimage",
                    "availability"
                ][..]
            )
        );
    }
    #[test]
    fn decoded_control_allocation_is_charged_to_the_bounded_decoder() {
        let empty = || CanonicalParts {
            consensus_header: Vec::new(),
            commit_qc: Vec::new(),
            result_preimage: Vec::new(),
            availability: Vec::new(),
        };
        let control = ncore::owned_arc_allocation_bytes::<CanonicalParts>().unwrap();
        let refusal = ncore::with_decode_limits(
            norito::DecodeLimits::new(1024, 1024, 1024, control - 1, 32),
            || CommitCertificate::from_decoded(empty()),
        );
        assert!(refusal.is_err());
        let admitted_decode = ncore::with_decode_limits(
            norito::DecodeLimits::new(1024, 1024, 1024, control, 32),
            || CommitCertificate::from_decoded(empty()),
        )
        .unwrap();
        assert_eq!(admitted_decode.payload_len(), 0);
        assert!(!admitted_decode.admitted_to(&AllocationBudget::new(4096)));
        let bytes = norito::encode_canonical(&admitted_decode).unwrap();
        assert_eq!(
            norito::decode_canonical_with_limits::<CommitCertificate>(
                &bytes,
                norito::DecodeLimits::new(1024, 1024, 1024, 4096, 32)
            )
            .unwrap(),
            admitted_decode
        );
    }
    #[cfg(feature = "transparent_api")]
    #[test]
    fn signed_block_and_arc_clones_keep_original_certificate_custody() {
        let budget = AllocationBudget::new(49 + control_size());
        let certificate = CommitCertificate::from_charged_owner(parts(&budget), &budget).unwrap();
        let pointer = certificate.result_preimage().as_ptr();
        let mut block = crate::block::output_test_support::proposal(1);
        block.set_commit_certificate(Some(certificate));
        let retained = Arc::new(block);
        let kura_copy = Arc::clone(&retained);
        let independently_cloned_block = retained.as_ref().clone();
        assert_eq!(
            independently_cloned_block
                .commit_certificate()
                .unwrap()
                .result_preimage()
                .as_ptr(),
            pointer
        );
        drop(retained);
        drop(independently_cloned_block);
        assert_eq!(budget.reserved_bytes(), 49 + control_size());
        assert!(kura_copy.commit_certificate().unwrap().admitted_to(&budget));
        drop(kura_copy);
        assert_eq!(budget.reserved_bytes(), 0);
    }
    #[test]
    fn value_comparison_and_hash_ignore_storage_class() {
        use std::hash::DefaultHasher;
        let untrusted = sample();
        let admitted = untrusted.admit(&AllocationBudget::new(4096)).unwrap();
        assert_eq!(untrusted.partial_cmp(&admitted), Some(Ordering::Equal));
        let hash = |cert: &CommitCertificate| {
            let mut h = DefaultHasher::new();
            cert.hash(&mut h);
            h.finish()
        };
        assert_eq!(hash(&untrusted), hash(&admitted));
        assert_eq!(format!("{untrusted:?}"), format!("{admitted:?}"));
    }
    #[test]
    fn availability_owner_from_another_pool_is_rejected_without_replacement() {
        let budget = AllocationBudget::new(4096);
        let foreign = AllocationBudget::new(4096);
        let mut original = parts(&budget);
        original.availability = buffer(&[7; 4], &foreign);
        let pointer = original.availability.as_slice().as_ptr();
        let (returned, error) = CommitCertificate::from_charged_owner(original, &budget)
            .expect_err("foreign availability cannot confer publication custody");
        assert!(matches!(error, CertificateAdmissionError::ForeignBudget));
        assert_eq!(returned.availability.as_slice().as_ptr(), pointer);
        assert_eq!(foreign.reserved_bytes(), 4);
        assert_eq!(budget.reserved_bytes(), 45);
        drop(returned);
        assert_eq!(foreign.reserved_bytes(), 0);
        assert_eq!(budget.reserved_bytes(), 0);
    }
    #[test]
    fn missing_availability_field_is_rejected_in_binary_and_json() {
        #[derive(Encode, norito::NoritoSchema)]
        #[norito_schema(name = "iroha_data_model::block::commit_certificate::CommitCertificate")]
        struct MissingAvailability {
            consensus_header: Vec<u8>,
            commit_qc: Vec<u8>,
            result_preimage: Vec<u8>,
        }
        let incomplete = MissingAvailability {
            consensus_header: vec![1, 2, 3],
            commit_qc: vec![4, 5],
            result_preimage: vec![6; 40],
        };
        assert!(CommitCertificate::decode_all(&mut incomplete.encode().as_slice()).is_err());
        assert!(
            norito::decode_canonical::<CommitCertificate>(
                &norito::encode_canonical(&incomplete).unwrap()
            )
            .is_err()
        );
        assert!(
            norito::json::from_str::<CommitCertificate>(
                r#"{"consensus_header":"AQID","commit_qc":"BAU=","result_preimage":"Bg=="}"#
            )
            .is_err()
        );
    }
    mod wire {
        use super::*;
        #[derive(Encode, IntoSchema, norito::NoritoSchema)]
        #[norito_schema(name = "iroha_data_model::block::commit_certificate::CommitCertificate")]
        pub(super) struct CommitCertificate {
            pub(super) consensus_header: Vec<u8>,
            pub(super) commit_qc: Vec<u8>,
            pub(super) result_preimage: Vec<u8>,
            pub(super) availability: Vec<u8>,
        }
    }
}
