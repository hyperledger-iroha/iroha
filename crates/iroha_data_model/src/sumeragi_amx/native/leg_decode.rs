//! One-pass original-pool custody for the closed canonical native AMX leg.
//!
//! Each concrete allocation is admitted where canonical traversal reaches it.
//! There is no whole-frame preview, quota reset or unfunded token bank. Original
//! input and diagnostic Strings remain enclosing-owner obligations. The completed
//! owner keeps the actual canonical counter controls and graph for a Worker borrow.
//! TODO: resume partially decoded backing and fund existing diagnostic Strings
//! without changing their canonical error ordering.

use super::AmxTransferLegV1;
use crate::{
    account::{AccountController, AccountId, MultisigMember, MultisigPolicy},
    asset::{AssetBalanceScope, AssetDefinitionId, AssetId},
    sumeragi_finality::MAX_RESULT_PREIMAGE_BYTES,
};
use iroha_allocation::{AllocationBudget, AllocationCharge, ChargedBuffer, ChargedBufferError};
use iroha_crypto::{
    ChargedPublicKey, PreparedPublicKeyDecode, PublicKey, PublicKeyDecodeAdmissionError,
};
use iroha_model_base::topology::DataSpaceId;
use iroha_primitives::numeric::{
    ChargedQuantity, PreparedQuantityDecode, Quantity, QuantityDecodeAdmissionError,
};
use norito::core::{
    self as ncore, CanonicalField, ChargedElementSequence, DecodeField, DecodeIntoError,
    DecodeRecordFields, FieldDestination, PreparedDecodeError, PreparedDecodeScopeError,
    PreparedDecodeWorkspace, PreparedRecordDestination,
};
use std::fmt;

/// Original parser cause or local original-pool refusal, never a protocol vote.
#[derive(Debug, thiserror::Error)]
pub enum AmxLegDecodeErrorV1 {
    /// The original fixed transfer payload bound was exceeded.
    #[error("{0}")]
    Record(&'static str),
    /// Original canonical field, cumulative work or scalar validation cause.
    #[error(transparent)]
    Codec(#[from] norito::Error),
    /// Canonical attempt cause captured before its original scopes retired.
    #[error(transparent)]
    Decode(ncore::DecodeAttemptError),
    /// Original physically prepared counter-control failure.
    #[error(transparent)]
    Scope(#[from] PreparedDecodeScopeError),
    /// Original pool admission/release cause or genuine allocator refusal.
    #[error(transparent)]
    Allocation(#[from] ChargedBufferError),
    /// A closed source/destination invariant changed without a protocol rejection.
    #[error("native AMX leg destination: {0}")]
    Invariant(&'static str),
}
use AmxLegDecodeErrorV1 as Error;
impl From<DecodeIntoError<Error>> for Error {
    fn from(error: DecodeIntoError<Error>) -> Self {
        match error {
            DecodeIntoError::Codec(error) => Self::Codec(error),
            DecodeIntoError::Destination(error) => error,
        }
    }
}
impl From<PublicKeyDecodeAdmissionError> for Error {
    fn from(error: PublicKeyDecodeAdmissionError) -> Self {
        match error {
            PublicKeyDecodeAdmissionError::Codec(error) => Self::Codec(error),
            PublicKeyDecodeAdmissionError::Allocation(error) => Self::Allocation(error),
        }
    }
}
impl From<QuantityDecodeAdmissionError> for Error {
    fn from(error: QuantityDecodeAdmissionError) -> Self {
        match error {
            QuantityDecodeAdmissionError::Codec(error) => Self::Codec(error),
            QuantityDecodeAdmissionError::Allocation(error) => Self::Allocation(error),
            QuantityDecodeAdmissionError::Incomplete => {
                Self::Invariant("unfinished native Quantity")
            }
        }
    }
}
impl From<ncore::SequenceAdmissionError> for Error {
    fn from(error: ncore::SequenceAdmissionError) -> Self {
        match error {
            ncore::SequenceAdmissionError::Codec(error) => Self::Codec(error),
            ncore::SequenceAdmissionError::Allocation(error) => Self::Allocation(error),
        }
    }
}
fn destination(error: Error) -> DecodeIntoError<Error> {
    match error {
        Error::Codec(error) => DecodeIntoError::Codec(error),
        error => DecodeIntoError::Destination(error),
    }
}

/// One complete immutable leg whose canonical values are destroyed before all refunds.
///
/// The member Vec, each compact key, native Quantity digits and key-ledger
/// buffers retain their actual original-pool charges. Transient span scratch is
/// already reclaimed on return. This is an inline move-only owner; no shared
/// shell, mutation, replacement or extraction API can detach that graph.
/// Ordinary clones made from the borrow are separate unfunded graphs.
pub struct AllocatedAmxTransferLegV1 {
    value: AmxTransferLegV1,
    source: ControllerCustody,
    destination: ControllerCustody,
    amount: AllocationCharge,
}
impl fmt::Debug for AllocatedAmxTransferLegV1 {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output
            .debug_tuple("AllocatedAmxTransferLegV1")
            .field(self.canonical())
            .finish()
    }
}
impl AllocatedAmxTransferLegV1 {
    /// Borrow the exact canonical transfer without separating its funding.
    #[must_use]
    pub fn canonical(&self) -> &AmxTransferLegV1 {
        &self.value
    }
    /// Check all real retained allocations, including the key-ledger buffers.
    #[must_use]
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.source.belongs_to(budget)
            && self.destination.belongs_to(budget)
            && self.amount.belongs_to(budget)
    }
    /// Exact retained layout sum, excluding already-destroyed span scratch.
    #[must_use]
    pub fn allocation_bytes(&self) -> Option<usize> {
        self.source
            .bytes()?
            .checked_add(self.destination.bytes()?)?
            .checked_add(self.amount.layout().size())
    }
}

/// A completed canonical attempt with its original physical counter controls.
///
/// The graph is declared first so its actual nested values and ledgers retire
/// before the workspace refunds either counter control. No extraction, clone,
/// replacement or re-entry API separates these owners.
pub struct CompletedAmxTransferLegDecodeV1 {
    leg: AllocatedAmxTransferLegV1,
    workspace: PreparedDecodeWorkspace,
}
impl fmt::Debug for CompletedAmxTransferLegDecodeV1 {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output
            .debug_tuple("CompletedAmxTransferLegDecodeV1")
            .field(self.canonical())
            .finish()
    }
}
impl CompletedAmxTransferLegDecodeV1 {
    /// Borrow the same completed canonical graph without decoder work.
    #[must_use]
    pub fn canonical(&self) -> &AmxTransferLegV1 {
        self.leg.canonical()
    }
    /// Require all graph and workspace allocations to retain this original pool.
    #[must_use]
    pub fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        self.leg.belongs_to(budget) && self.workspace.belongs_to(budget)
    }
    /// Exact retained graph and physical counter layouts, without retired scratch.
    #[must_use]
    pub fn allocation_bytes(&self) -> Option<usize> {
        PreparedDecodeWorkspace::allocation_layouts()
            .iter()
            .try_fold(self.leg.allocation_bytes()?, |sum, layout| {
                sum.checked_add(layout.size())
            })
    }
}

enum ControllerCustody {
    Single(AllocationCharge),
    Multisig {
        members: AllocationCharge,
        keys: ChargedBuffer<AllocationCharge>,
    },
}
impl ControllerCustody {
    fn belongs_to(&self, budget: &AllocationBudget) -> bool {
        match self {
            Self::Single(charge) => charge.belongs_to(budget),
            Self::Multisig { members, keys } => {
                members.belongs_to(budget)
                    && keys.belongs_to(budget)
                    && keys
                        .as_slice()
                        .iter()
                        .all(|charge| charge.belongs_to(budget))
            }
        }
    }
    fn bytes(&self) -> Option<usize> {
        match self {
            Self::Single(charge) => Some(charge.layout().size()),
            Self::Multisig { members, keys } => {
                let ledger = std::alloc::Layout::array::<AllocationCharge>(keys.capacity())
                    .ok()?
                    .size();
                keys.as_slice()
                    .iter()
                    .try_fold(members.layout().size().checked_add(ledger)?, |sum, key| {
                        sum.checked_add(key.layout().size())
                    })
            }
        }
    }
}
struct ChargedAccount {
    value: AccountId,
    custody: ControllerCustody,
}
struct ChargedAsset {
    value: AssetId,
    custody: ControllerCustody,
}
struct ChargedMember {
    value: MultisigMember,
    charge: AllocationCharge,
}
struct ChargedPolicy {
    value: MultisigPolicy,
    custody: ControllerCustody,
}

/// Borrow the original canonical frame and original finite pool for every retry.
///
/// No decoded destination is retained after failure: already materialized
/// fields are destroyed before physical refunds, while the original cumulative
/// decoder work stays consumed. Preserving backing across whole Worker retries
/// requires a subsequent actual execution-attempt owner and is not claimed here.
pub struct PendingAmxTransferLegDecodeV1<'source> {
    source: &'source [u8],
    budget: &'source AllocationBudget,
}
impl<'source> PendingAmxTransferLegDecodeV1<'source> {
    /// Retain the original source borrow without planning or allocating a graph.
    #[must_use]
    pub fn new(source: &'source [u8], budget: &'source AllocationBudget) -> Self {
        Self { source, budget }
    }
    /// Original signed payload slice identity, unchanged across local retries.
    #[must_use]
    pub fn original_source(&self) -> &'source [u8] {
        self.source
    }
    /// Decode once, retaining every real successful nested allocation.
    ///
    /// # Errors
    /// Preserves parser, original-pool release and allocator causes. Existing
    /// `Error::Message` diagnostic Strings remain separately unfunded storage.
    pub fn try_decode(&self) -> Result<AllocatedAmxTransferLegV1, Error> {
        let CompletedAmxTransferLegDecodeV1 { leg, workspace: _ } = self.try_decode_retained()?;
        Ok(leg)
    }

    /// Decode once and keep the actual canonical controls with the completed graph.
    ///
    /// This move-only completion can survive an enclosing execution refusal. Borrowing
    /// it does not enter another decoder scope, reset quotas or grant execution authority.
    /// Failed partial destinations still retire normally; their resume is not implemented.
    ///
    /// # Errors
    /// Preserves the same original parser, pool and allocator causes as `try_decode`.
    pub fn try_decode_retained(&self) -> Result<CompletedAmxTransferLegDecodeV1, Error> {
        if self.source.is_empty() || self.source.len() > MAX_RESULT_PREIMAGE_BYTES {
            return Err(Error::Record(
                "native AMX transfer payload exceeds its canonical bound",
            ));
        }
        let mut reservation = self
            .budget
            .try_reserve_layouts(PreparedDecodeWorkspace::allocation_layouts())
            .map_err(ChargedBufferError::from)?;
        let mut workspace =
            PreparedDecodeWorkspace::from_reservation(self.budget, &mut reservation)?;
        drop(reservation);
        let mut destination = LegDestination::new(self.budget);
        workspace
            .decode_canonical_into::<AmxTransferLegV1, _>(
                self.source,
                norito::canonical_decode_limits(self.source.len()),
                &mut destination,
            )
            .map_err(|error| match error {
                PreparedDecodeError::Codec(error) => Error::Decode(error),
                PreparedDecodeError::Destination(error) => error,
                PreparedDecodeError::Scope(error) => Error::Scope(error),
            })?;
        let leg = destination.finish()?;
        Ok(CompletedAmxTransferLegDecodeV1 { leg, workspace })
    }
}

// A private one-attempt destination. Reset invalidates only; populated backing
// is never replaced or retired by reset, including after final frame rejection.
struct LegDestination<'pool> {
    source: Option<ChargedAsset>,
    destination: Option<ChargedAccount>,
    complete: Option<AllocatedAmxTransferLegV1>,
    budget: &'pool AllocationBudget,
    valid: bool,
}
impl<'pool> LegDestination<'pool> {
    fn new(budget: &'pool AllocationBudget) -> Self {
        Self {
            source: None,
            destination: None,
            complete: None,
            budget,
            valid: false,
        }
    }
    fn finish(mut self) -> Result<AllocatedAmxTransferLegV1, Error> {
        if !self.valid {
            return Err(Error::Invariant("unfinished native AMX leg"));
        }
        self.complete
            .take()
            .ok_or(Error::Invariant("missing native AMX leg"))
    }
}
impl FieldDestination for LegDestination<'_> {
    type Error = Error;
}
impl ncore::SerializePayload for LegDestination<'_> {
    fn serialize(&self, encoder: &mut ncore::Encoder<'_>) -> Result<(), norito::Error> {
        if !self.valid {
            return Err(norito::Error::LengthMismatch);
        }
        ncore::SerializePayload::serialize(
            self.complete
                .as_ref()
                .ok_or(norito::Error::LengthMismatch)?
                .canonical(),
            encoder,
        )
    }
}
impl PreparedRecordDestination<AmxTransferLegV1> for LegDestination<'_> {
    fn reset(&mut self) {
        self.valid = false;
    }
}
impl DecodeField<0, AssetId> for LegDestination<'_> {
    type Value = ();
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, AssetId>,
    ) -> Result<(), DecodeIntoError<Error>> {
        if self.source.is_some() || self.destination.is_some() || self.complete.is_some() {
            return Err(destination(Error::Invariant(
                "populated native AMX leg destination",
            )));
        }
        self.source =
            Some(field.with_payload(|bytes| asset(bytes, self.budget).map_err(destination))?);
        Ok(())
    }
}
impl DecodeField<1, AccountId> for LegDestination<'_> {
    type Value = ();
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, AccountId>,
    ) -> Result<(), DecodeIntoError<Error>> {
        if self.source.is_none() || self.destination.is_some() || self.complete.is_some() {
            return Err(destination(Error::Invariant(
                "native AMX leg destination order changed",
            )));
        }
        self.destination =
            Some(field.with_payload(|bytes| account(bytes, self.budget).map_err(destination))?);
        Ok(())
    }
}
impl DecodeField<2, Quantity> for LegDestination<'_> {
    type Value = ();
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, Quantity>,
    ) -> Result<(), DecodeIntoError<Error>> {
        if self.source.is_none() || self.destination.is_none() || self.complete.is_some() {
            return Err(destination(Error::Invariant(
                "native AMX leg amount order changed",
            )));
        }
        let amount = field.with_payload(|bytes| {
            PreparedQuantityDecode::try_decode_payload(bytes, self.budget)
                .map_err(|error| destination(error.into()))
        })?;
        // These Options were checked before decoding; taking them performs no
        // allocation, clone, callback or replacement of admitted backing.
        let source = self
            .source
            .take()
            .ok_or_else(|| destination(Error::Invariant("missing native AMX leg source")))?;
        let receiver = self
            .destination
            .take()
            .ok_or_else(|| destination(Error::Invariant("missing native AMX leg destination")))?;
        self.complete = Some(bind_leg(source, receiver, amount));
        self.valid = true;
        Ok(())
    }
}

#[allow(unsafe_code)]
fn bind_leg(
    source: ChargedAsset,
    destination: ChargedAccount,
    amount: ChargedQuantity,
) -> AllocatedAmxTransferLegV1 {
    // SAFETY: all following operations are infallible moves into one private
    // value-first owner. No field clone, validation or callback can intervene.
    let (amount, amount_charge) = unsafe { amount.into_allocation_parts() };
    AllocatedAmxTransferLegV1 {
        value: AmxTransferLegV1 {
            source: source.value,
            destination: destination.value,
            amount,
        },
        source: source.custody,
        destination: destination.custody,
        amount: amount_charge,
    }
}
struct AssetFields<'pool> {
    budget: &'pool AllocationBudget,
}
impl FieldDestination for AssetFields<'_> {
    type Error = Error;
}
impl DecodeField<0, AccountId> for AssetFields<'_> {
    type Value = ChargedAccount;
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, AccountId>,
    ) -> Result<ChargedAccount, DecodeIntoError<Error>> {
        field.with_payload(|bytes| account(bytes, self.budget).map_err(destination))
    }
}
impl DecodeField<1, AssetDefinitionId> for AssetFields<'_> {
    type Value = AssetDefinitionId;
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, AssetDefinitionId>,
    ) -> Result<AssetDefinitionId, DecodeIntoError<Error>> {
        field.with_payload(|bytes| {
            let (original, used) =
                <[u8; 16] as ncore::DecodeFromSlice<'_>>::decode_from_slice(bytes)?;
            if used != bytes.len() {
                return Err(norito::Error::LengthMismatch.into());
            }
            ncore::finish_context_fields(bytes.as_ptr(), used)?;
            AssetDefinitionId::from_uuid_bytes(original)
                .map_err(|error| DecodeIntoError::Codec(norito::Error::Message(error.to_string())))
        })
    }
}
impl DecodeField<2, AssetBalanceScope> for AssetFields<'_> {
    type Value = AssetBalanceScope;
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, AssetBalanceScope>,
    ) -> Result<AssetBalanceScope, DecodeIntoError<Error>> {
        field.with_payload(|bytes| scope(bytes).map_err(destination))
    }
}
fn asset(bytes: &[u8], budget: &AllocationBudget) -> Result<ChargedAsset, Error> {
    let mut fields = AssetFields { budget };
    let ((account, definition, scope), used) = AssetId::decode_fields(bytes, &mut fields)?;
    finish(bytes, used)?;
    Ok(ChargedAsset {
        value: AssetId::with_scope(definition, account.value, scope),
        custody: account.custody,
    })
}
fn field<T, R>(
    bytes: &[u8],
    offset: &mut usize,
    read: impl FnOnce(&[u8]) -> Result<R, Error>,
) -> Result<R, Error>
where
    T: for<'de> norito::DeserializePayload<'de> + norito::SerializePayload,
{
    ncore::framed_field::<T>(bytes, offset)?
        .with_payload(|bytes| read(bytes).map_err(destination))
        .map_err(Error::from)
}
fn scalar<T>(bytes: &[u8]) -> Result<T, Error>
where
    T: for<'de> ncore::DecodeFromSlice<'de>,
{
    let (value, used) = T::decode_from_slice(bytes)?;
    finish(bytes, used)?;
    Ok(value)
}
fn finish(bytes: &[u8], used: usize) -> Result<(), Error> {
    if used != bytes.len() {
        return Err(norito::Error::LengthMismatch.into());
    }
    ncore::finish_context_fields(bytes.as_ptr(), used)?;
    Ok(())
}
fn tag(bytes: &[u8]) -> Result<u32, Error> {
    let raw = bytes.get(..4).ok_or(norito::Error::LengthMismatch)?;
    Ok(u32::from_le_bytes(
        raw.try_into().map_err(|_| norito::Error::LengthMismatch)?,
    ))
}
fn invalid_tag() -> Error {
    norito::Error::Message("invalid enum discriminant".into()).into()
}
fn account(bytes: &[u8], budget: &AllocationBudget) -> Result<ChargedAccount, Error> {
    let mut offset = 4;
    let owner = match tag(bytes)? {
        0 => {
            let key = field::<PublicKey, _>(bytes, &mut offset, |bytes| {
                PreparedPublicKeyDecode::try_decode_payload(bytes, budget).map_err(Error::from)
            })?;
            bind_single(key)
        }
        1 => {
            let policy =
                field::<MultisigPolicy, _>(bytes, &mut offset, |bytes| policy(bytes, budget))?;
            ChargedAccount {
                value: AccountId {
                    controller: AccountController::Multisig(policy.value),
                },
                custody: policy.custody,
            }
        }
        _ => return Err(invalid_tag()),
    };
    finish(bytes, offset)?;
    Ok(owner)
}
#[allow(unsafe_code)]
fn bind_single(key: ChargedPublicKey) -> ChargedAccount {
    // SAFETY: immediately transfer both fields into one immutable value-first
    // owner. Constructing AccountId/Single only moves the existing compact Box.
    let (key, charge) = unsafe { key.into_allocation_parts() };
    ChargedAccount {
        value: AccountId {
            controller: AccountController::Single(key),
        },
        custody: ControllerCustody::Single(charge),
    }
}
fn policy(bytes: &[u8], budget: &AllocationBudget) -> Result<ChargedPolicy, Error> {
    let mut offset = 0;
    let version = field::<u8, _>(bytes, &mut offset, scalar::<u8>)?;
    let threshold = field::<u16, _>(bytes, &mut offset, scalar::<u16>)?;
    let members =
        field::<Vec<MultisigMember>, _>(bytes, &mut offset, |bytes| members(bytes, budget))?;
    // Retain complete member values with their ledgers through trailing framing
    // checks before invoking the original whole-policy relation.
    finish(bytes, offset)?;
    members.finish(version, threshold)
}
struct MemberBuffers {
    members: ChargedBuffer<MultisigMember>,
    keys: ChargedBuffer<AllocationCharge>,
}
impl MemberBuffers {
    fn new(count: usize, budget: &AllocationBudget) -> Result<Self, Error> {
        let members = ChargedBuffer::new(count, budget)?;
        let keys = ChargedBuffer::new(count, budget)?;
        Ok(Self { members, keys })
    }
    fn push(&mut self, member: ChargedMember) -> Result<(), Error> {
        // Check both original capacities before separating a nested owner. Both
        // bounded pushes below move values without allocation, callbacks or any
        // input-dependent assertion failure; no fallible work can intervene.
        if self.members.as_slice().len() >= self.members.capacity()
            || self.keys.as_slice().len() >= self.keys.capacity()
            || self.members.as_slice().len() != self.keys.as_slice().len()
        {
            return Err(Error::Invariant(
                "original member/charge destination is full",
            ));
        }
        self.members.push_reserved(member.value);
        self.keys.push_reserved(member.charge);
        Ok(())
    }
    #[allow(unsafe_code)]
    fn into_parts(self) -> MemberParts {
        // SAFETY: original spans selected exactly this capacity. The values and
        // member charge are immediately retained in a private value-first owner,
        // alongside every exact compact-key charge and that ledger's backing.
        let (members, charge) = unsafe { self.members.into_allocation_parts() };
        MemberParts {
            members: Some(members),
            charge,
            keys: self.keys,
        }
    }
}
struct MemberParts {
    members: Option<Vec<MultisigMember>>,
    charge: AllocationCharge,
    keys: ChargedBuffer<AllocationCharge>,
}
impl MemberParts {
    fn finish(mut self, version: u8, threshold: u16) -> Result<ChargedPolicy, Error> {
        let members = self
            .members
            .take()
            .ok_or(Error::Invariant("missing original member values"))?;
        // The sole original validator consumes this Vec unchanged. On rejection
        // or unwind it destroys all member/key values before the enclosing
        // original ledgers drop. Successful construction cannot reorder/grow it.
        let value =
            MultisigPolicy::from_serialized(version, threshold, members).map_err(policy_error)?;
        Ok(ChargedPolicy {
            value,
            custody: ControllerCustody::Multisig {
                members: self.charge,
                keys: self.keys,
            },
        })
    }
}
fn members(bytes: &[u8], budget: &AllocationBudget) -> Result<MemberParts, Error> {
    let sequence = ChargedElementSequence::try_from_payload(bytes, budget)?;
    let prepared = sequence.as_prepared();
    if prepared.used() != bytes.len() {
        return Err(norito::Error::LengthMismatch.into());
    }
    let count = prepared.len();
    let mut output = None;
    prepared.decode_elements::<MultisigMember, Error>(|_, field| {
        // decode_elements admits the original output-T work before this first
        // callback. Physical members/ledger backing must precede the member's
        // field/depth/key decode, never a later invalid field or whole preview.
        if output.is_none() {
            output = Some(MemberBuffers::new(count, budget).map_err(destination)?);
        }
        let member = field.with_payload(|bytes| member(bytes, budget).map_err(destination))?;
        output
            .as_mut()
            .ok_or_else(|| {
                DecodeIntoError::Destination(Error::Invariant(
                    "missing original member destination",
                ))
            })?
            .push(member)
            .map_err(destination)
    })?;
    let output = match output {
        Some(output) => output,
        None => MemberBuffers::new(0, budget)?,
    };
    // Complete framing and the same source-bound plan remain borrowed until
    // every element is decoded. The real span bank then dies before return.
    Ok(output.into_parts())
}
struct KeyParts {
    key: Option<PublicKey>,
    charge: AllocationCharge,
}
fn member(bytes: &[u8], budget: &AllocationBudget) -> Result<ChargedMember, Error> {
    let mut offset = 0;
    let key = field::<PublicKey, _>(bytes, &mut offset, |bytes| {
        PreparedPublicKeyDecode::try_decode_payload(bytes, budget).map_err(Error::from)
    })?;
    let weight = field::<u16, _>(bytes, &mut offset, scalar::<u16>)?;
    finish(bytes, offset)?;
    bind_member(key, weight)
}
#[allow(unsafe_code)]
fn bind_member(key: ChargedPublicKey, weight: u16) -> Result<ChargedMember, Error> {
    // SAFETY: keep the complete checked key and charge paired through the sole
    // original member relation. Its failure consumes/destroys key before refund.
    let (key, charge) = unsafe { key.into_allocation_parts() };
    let mut parts = KeyParts {
        key: Some(key),
        charge,
    };
    let key = parts
        .key
        .take()
        .ok_or(Error::Invariant("missing original member key"))?;
    let value = MultisigMember::new(key, weight).map_err(policy_error)?;
    Ok(ChargedMember {
        value,
        charge: parts.charge,
    })
}
fn policy_error(error: crate::account::MultisigPolicyError) -> Error {
    norito::Error::Message(error.to_string()).into()
}
fn scope(bytes: &[u8]) -> Result<AssetBalanceScope, Error> {
    let mut offset = 4;
    let scope = match tag(bytes)? {
        0 => AssetBalanceScope::Global,
        1 => AssetBalanceScope::Dataspace(field::<DataSpaceId, _>(bytes, &mut offset, |bytes| {
            let mut offset = 0;
            let value = field::<u64, _>(bytes, &mut offset, scalar::<u64>)?;
            finish(bytes, offset)?;
            Ok(DataSpaceId::new(value))
        })?),
        _ => return Err(invalid_tag()),
    };
    finish(bytes, offset)?;
    Ok(scope)
}

#[cfg(test)]
mod tests;
