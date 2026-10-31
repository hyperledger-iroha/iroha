//! Original-pool ownership of the canonical UID binding's account graph.
//!
//! The derived UID record walk and existing controller leaf decoder own all successful
//! key/member allocations. State lookup keys and the separate policy roster are not
//! owned here. No DTO clone or replacement canonical byte buffer is constructed.

use super::AssetUidBindingV1;
use crate::{
    account::{
        AccountId,
        prepared_decode::{AccountDecodeError, AccountDestination, ControllerCustody},
    },
    asset::AssetDefinitionId,
};
use iroha_allocation::{AllocationBudget, ChargedBufferError};
use iroha_crypto::Hash;
use norito::core::{
    self as ncore, CanonicalField, DecodeField, DecodeIntoError, FieldDestination,
    OwnedPreparedDecodeError, PreparedDecodeError, PreparedDecodeWorkspace,
};

/// Original leaf failure from a UID binding's funded account destination.
#[derive(Debug, thiserror::Error)]
pub enum AssetUidBindingDecodeError {
    /// The unchanged canonical field error.
    #[error(transparent)]
    Codec(#[from] norito::Error),
    /// Intrinsic malformed multisig controller.
    #[error(transparent)]
    Controller(#[from] crate::account::MultisigPolicyError),
    /// Original pool or native allocator refusal for concrete graph backing.
    #[error(transparent)]
    Allocation(#[from] ChargedBufferError),
    /// Changed source, incomplete destination, or consumed original owner.
    #[error("asset UID binding destination: {0}")]
    Invariant(&'static str),
}
use AssetUidBindingDecodeError as Error;
impl From<AccountDecodeError> for Error {
    fn from(error: AccountDecodeError) -> Self {
        match error {
            AccountDecodeError::Codec(error) => Self::Codec(error),
            AccountDecodeError::Multisig(error) => Self::Controller(error),
            AccountDecodeError::Allocation(error) => Self::Allocation(error),
            AccountDecodeError::Invariant(error) => Self::Invariant(error),
        }
    }
}
fn destination(error: Error) -> DecodeIntoError<Error> {
    match error {
        Error::Codec(error) => DecodeIntoError::Codec(error),
        error => DecodeIntoError::Destination(error),
    }
}
fn attempt(error: Error) -> OwnedPreparedDecodeError<Error> {
    OwnedPreparedDecodeError::Decode(PreparedDecodeError::Destination(error))
}

/// Immutable UID binding paired with its actual original controller charges.
///
/// The value drops before its charges. Borrowing grants no State authority and
/// exposes no uncharged extraction, mutable access, or cloning operation.
pub struct AllocatedAssetUidBindingV1 {
    value: AssetUidBindingV1,
    custody: ControllerCustody,
}
impl AllocatedAssetUidBindingV1 {
    /// Borrow the complete, canonically authenticated binding.
    #[must_use]
    pub fn canonical(&self) -> &AssetUidBindingV1 {
        &self.value
    }

    /// Whether every retained account allocation belongs to this exact pool.
    #[must_use]
    pub fn belongs_to(&self, pool: &AllocationBudget) -> bool {
        self.custody.belongs_to(pool)
    }

    /// Exact bytes retained by the account graph and its physical charge ledger.
    #[must_use]
    pub fn allocation_bytes(&self) -> Option<usize> {
        self.custody.bytes()
    }
}

/// Partial binding decoder retaining the original controls and controller prefix.
///
/// Late refusal retains both counter controls, keys, members and spans. Retry
/// uses that same physical workspace; finish or cancellation retires it only
/// after the complete value has moved with its original controller charges.
pub struct PendingAssetUidBindingDecodeV1 {
    workspace: Option<PreparedDecodeWorkspace>,
    destination: BindingDestination,
}
impl PendingAssetUidBindingDecodeV1 {
    /// Bind the original finite pool without allocating controls or graph backing.
    #[must_use]
    pub fn new(pool: &AllocationBudget) -> Self {
        Self {
            workspace: None,
            destination: BindingDestination::new(pool),
        }
    }
    /// Whether all controls and partial graph backing retain this exact pool.
    #[must_use]
    pub fn belongs_to(&self, pool: &AllocationBudget) -> bool {
        self.workspace
            .as_ref()
            .is_none_or(|workspace| workspace.belongs_to(pool))
            && self.destination.belongs_to(pool)
    }
    /// Decode the original complete frame, retaining admitted controls on failure.
    ///
    /// # Errors
    /// Preserves original header, pool, allocator, scope and field causes. A new
    /// source or a consumed destination cannot replace the retained attempt.
    pub fn decode(
        &mut self,
        bytes: &[u8],
        limits: norito::DecodeLimits,
    ) -> Result<(), OwnedPreparedDecodeError<Error>> {
        if self.destination.taken {
            return Err(attempt(Error::Invariant(
                "completed binding was already taken",
            )));
        }
        self.destination.valid = false;
        let identity = (bytes.as_ptr() as usize, bytes.len(), Hash::new(bytes));
        if self
            .destination
            .source
            .is_some_and(|source| source != identity)
        {
            return Err(attempt(Error::Invariant("original binding bytes changed")));
        }
        self.destination.source = Some(identity);
        let pool = self.destination.pool.clone();
        PreparedDecodeWorkspace::decode_canonical_into_with_owned_controls::<AssetUidBindingV1, _>(
            bytes,
            limits,
            &pool,
            &mut self.workspace,
            &mut self.destination,
        )?;
        self.destination.valid = true;
        Ok(())
    }
    /// Move the funded graph out, then retire decoder-only controls and spans.
    ///
    /// # Errors
    /// Incomplete attempts retain every control and graph allocation for retry.
    pub fn finish(&mut self) -> Result<AllocatedAssetUidBindingV1, Error> {
        let complete = self.destination.finish()?;
        self.workspace = None;
        Ok(complete)
    }
}

struct BindingDestination {
    pool: AllocationBudget,
    source: Option<(usize, usize, Hash)>,
    account: AccountDestination,
    definition: Option<AssetDefinitionId>,
    complete: Option<AllocatedAssetUidBindingV1>,
    fields_complete: bool,
    valid: bool,
    taken: bool,
}
impl BindingDestination {
    /// Bind the original pool without decoding or allocating backing.
    #[must_use]
    fn new(pool: &AllocationBudget) -> Self {
        Self {
            pool: pool.clone(),
            source: None,
            account: AccountDestination::new(pool),
            definition: None,
            complete: None,
            fields_complete: false,
            valid: false,
            taken: false,
        }
    }

    /// Whether all completed and partial backing belongs to this original pool.
    #[must_use]
    fn belongs_to(&self, pool: &AllocationBudget) -> bool {
        self.pool.same_pool(pool)
            && self.account.belongs_to(pool)
            && self
                .complete
                .as_ref()
                .is_none_or(|value| value.belongs_to(pool))
    }

    /// Move the complete original owner out after full frame authentication.
    ///
    /// # Errors
    /// Incomplete attempts retain every original allocation and remain retryable.
    fn finish(&mut self) -> Result<AllocatedAssetUidBindingV1, Error> {
        if !self.valid || self.taken {
            return Err(Error::Invariant("unfinished binding destination"));
        }
        let complete = self
            .complete
            .take()
            .ok_or(Error::Invariant("missing complete binding"))?;
        self.valid = false;
        self.taken = true;
        // Only transient span/parser backing retires; graph charges move with the value.
        self.account = AccountDestination::new(&self.pool);
        Ok(complete)
    }
}
impl FieldDestination for BindingDestination {
    type Error = Error;
}
impl DecodeField<0, AssetDefinitionId> for BindingDestination {
    type Value = ();
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, AssetDefinitionId>,
    ) -> Result<(), DecodeIntoError<Error>> {
        if self.taken {
            return Err(destination(Error::Invariant(
                "completed binding was already taken",
            )));
        }
        self.valid = false;
        self.fields_complete = false;
        let value = field.with_payload(|bytes| {
            // The canonical definition contains only a byte-aligned UUID. Its existing
            // decoder allocates neither a graph nor realignment scratch.
            const {
                assert!(std::mem::align_of::<AssetDefinitionId>() == 1);
            }
            let (value, used) = ncore::decode_field_canonical::<AssetDefinitionId>(bytes)?;
            if used != bytes.len() {
                return Err(DecodeIntoError::Codec(norito::Error::LengthMismatch));
            }
            ncore::finish_context_fields(bytes.as_ptr(), used)?;
            Ok(value)
        })?;
        self.definition = Some(value);
        Ok(())
    }
}
impl DecodeField<1, AccountId> for BindingDestination {
    type Value = ();
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, AccountId>,
    ) -> Result<(), DecodeIntoError<Error>> {
        field.with_payload(|bytes| {
            self.account
                .decode(
                    bytes,
                    &self.pool,
                    self.complete.as_ref().map(|value| &value.value.account_id),
                )
                .map_err(|error| destination(error.into()))
        })
    }
}
impl DecodeField<2, [u8; 32]> for BindingDestination {
    type Value = ();
    fn decode_field(
        &mut self,
        field: CanonicalField<'_, [u8; 32]>,
    ) -> Result<(), DecodeIntoError<Error>> {
        // Raw-array boundaries and exact width belong to the generated field walk.
        let uid = field.with_payload(|bytes| {
            bytes
                .try_into()
                .map_err(|_| DecodeIntoError::Codec(norito::Error::LengthMismatch))
        })?;
        let definition = self
            .definition
            .take()
            .ok_or_else(|| destination(Error::Invariant("missing binding definition")))?;
        if let Some(complete) = self.complete.as_ref() {
            if complete.value.asset_definition_id != definition || complete.value.uid != uid {
                return Err(destination(Error::Invariant(
                    "original binding fields changed",
                )));
            }
        } else {
            let account =
                self.account.complete.take().ok_or_else(|| {
                    destination(Error::Invariant("missing original binding account"))
                })?;
            self.complete = Some(AllocatedAssetUidBindingV1 {
                value: AssetUidBindingV1 {
                    asset_definition_id: definition,
                    account_id: account.value,
                    uid,
                },
                custody: account.custody,
            });
        }
        self.fields_complete = true;
        Ok(())
    }
}
impl norito::SerializePayload for BindingDestination {
    fn serialize(&self, writer: &mut ncore::Encoder<'_>) -> Result<(), norito::Error> {
        if !self.fields_complete {
            return Err(norito::Error::LengthMismatch);
        }
        norito::SerializePayload::serialize(
            &self
                .complete
                .as_ref()
                .ok_or(norito::Error::LengthMismatch)?
                .value,
            writer,
        )
    }
}
impl ncore::PreparedRecordDestination<AssetUidBindingV1> for BindingDestination {
    fn reset(&mut self) {
        self.valid = false;
        self.fields_complete = false;
    }
}

/// Decode one binding with original control, key and multisig graph custody.
///
/// # Errors
/// Returns original physical or canonical failures without formatting or graph cloning.
pub fn decode_asset_uid_binding_with_owned_controls(
    bytes: &[u8],
    limits: norito::DecodeLimits,
    pool: &AllocationBudget,
) -> Result<AllocatedAssetUidBindingV1, OwnedPreparedDecodeError<Error>> {
    let mut pending = PendingAssetUidBindingDecodeV1::new(pool);
    pending.decode(bytes, limits)?;
    pending.finish().map_err(attempt)
}

#[cfg(test)]
mod tests;
