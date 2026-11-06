//! Bounded canonical Action-frame validation with original allocation custody.
//!
//! The outer Action, executable sequence, metadata and argument records are
//! borrowed. Account controllers use their funded destination. Each dynamic
//! instruction or complex filter is validated one at a time through its typed
//! canonical codec inside a prepaid transient scratch scope; no root Action or
//! whole executable graph is materialized. Unknown registry IDs and variants
//! are refused. This establishes wire semantics, not invocation authorization,
//! callable schemas, execution, activation or authenticated lifecycle coverage.

#[path = "action_frame_argument_record.rs"]
mod argument_record;
#[path = "action_frame_cursor.rs"]
mod cursor;
#[path = "action_frame_custody.rs"]
mod custody;

use crate::{
    account::prepared_decode::{AccountDecodeError, AccountDestination},
    trigger::action::Action,
};
use iroha_allocation::{AllocationBudget, AllocationReservation, ChargedBuffer};
use iroha_primitives::json::MAX_JSON_BYTES;
use norito::core::{self as ncore, DecodeBudgetContext, DecodeLimits};

use super::{TriggerLifecycleActionSemanticsV1, TriggerLifecycleStateKindV1};
pub use cursor::ActionScannerLimitsV1;
use cursor::{Budget, Cursor, ScanError};
pub use custody::{ActionFrameProductionFailureV1, RetainedActionFrameBytesV1};

/// Debit an embedded frame discovered by a typed lifecycle subtree decoder.
/// Ordinary decoding and producer-only prepaid scopes have no scanner counters.
pub(crate) fn charge_typed_nested_frame(
    len: usize,
    instruction: bool,
) -> Result<(), norito::Error> {
    cursor::charge_typed_nested_frame(len, instruction)
}

const SCANNER_FLAGS: u8 = ncore::header_flags::COMPACT_LEN;

/// The first original failure remains owned by the scanner until it is dropped.
#[derive(Debug)]
pub enum ActionFrameScanFailureV1 {
    /// Fixed structural/unsupported-kind refusal, without an allocated diagnostic.
    Structure(&'static str),
    /// Original Norito envelope, resource or canonical-comparison failure.
    Codec(norito::Error),
    /// Original cumulative encoder work or exact-comparison refusal.
    Encode(ncore::BoundedEncodeWorkError),
    /// Original funded account decoder failure, without replacement or retry.
    Authority(AccountDecodeError),
    /// Original exact-backing admission or allocator refusal, before any copy.
    Capture(iroha_allocation::ChargedBufferError),
    /// Original admission refusal for audited semantic-normalization scratch.
    Scratch(iroha_allocation::AllocationRefusal),
    /// Original typed-subtree failure and its still-retained physical scratch ceiling.
    Typed {
        /// Original refusal without a replacement diagnostic.
        error: norito::Error,
        /// Original reservation retained until the error and partial decode retire.
        scratch: AllocationReservation,
    },
}

/// A refused preparation retains both its first typed failure and any captured
/// frame. No validation marker is attached to that frame. Dropping this value
/// disposes of its owned frame; moving it out retains the exact same charge.
#[derive(Debug)]
pub struct ActionFrameCaptureFailureV1<'range> {
    frame: Option<RetainedActionFrameBytesV1>,
    failure: &'range ActionFrameScanFailureV1,
}

impl<'range> ActionFrameCaptureFailureV1<'range> {
    /// The same first failure remains retained by the fail-closed scanner.
    pub fn failure(&self) -> &'range ActionFrameScanFailureV1 {
        self.failure
    }

    /// Full original captured bytes, or absence if no capture was performed.
    pub fn frame(&self) -> Option<&RetainedActionFrameBytesV1> {
        self.frame.as_ref()
    }

    /// Move the complete byte owner without recapture, refund or replacement.
    /// The first failure stays with its scanner. This is not permission to reset
    /// that scanner, retry the range, or apply a lifecycle mutation.
    pub fn into_frame(self) -> Option<RetainedActionFrameBytesV1> {
        self.frame
    }
}

impl From<ScanError> for ActionFrameScanFailureV1 {
    fn from(error: ScanError) -> Self {
        Self::Structure(error.message())
    }
}
impl From<norito::Error> for ActionFrameScanFailureV1 {
    fn from(error: norito::Error) -> Self {
        Self::Codec(error)
    }
}
impl From<AccountDecodeError> for ActionFrameScanFailureV1 {
    fn from(error: AccountDecodeError) -> Self {
        Self::Authority(error)
    }
}

/// One cumulative, fail-closed scan range. There is no default or reset method.
///
/// Borrowed `scan` requires the caller's existing immutable frame custody.
/// `capture_and_scan` first funds and owns an exact copy of the supplied bytes.
/// The pool also funds actual account backing and decode counter control, but
/// neither path funds the original source or enclosing execution journal.
pub struct BoundedActionFrameScannerV1<'pool> {
    pool: &'pool AllocationBudget,
    budget: Budget,
    account_work: DecodeBudgetContext,
    failure: Option<ActionFrameScanFailureV1>,
    typed_scratch_bytes: usize,
}

impl<'pool> BoundedActionFrameScannerV1<'pool> {
    /// Fund one cumulative account decoder control before any nested decode.
    ///
    /// # Errors
    /// Returns the pool refusal when the account decoder control cannot be funded.
    pub fn try_new(
        pool: &'pool AllocationBudget,
        limits: ActionScannerLimitsV1,
        account_limits: DecodeLimits,
    ) -> Result<Self, ActionFrameScanFailureV1> {
        Ok(Self {
            pool,
            budget: Budget::new(limits),
            account_work: DecodeBudgetContext::try_new_owned(account_limits, pool)?,
            failure: None,
            typed_scratch_bytes: account_limits.max_total_allocated_bytes(),
        })
    }

    /// Validate a full frame against the fixed canonical Action codec.
    ///
    /// A refusal permanently closes this scanner. Subsequent calls return the
    /// same first original failure without parsing, allocating or resetting work.
    /// Success authenticates no ledger state and does not establish frame custody.
    ///
    /// # Errors
    /// Returns this scanner's first retained refusal for a non-canonical, unsupported
    /// or over-limit frame.
    pub fn scan(
        &mut self,
        frame: &[u8],
    ) -> Result<TriggerLifecycleActionSemanticsV1, &ActionFrameScanFailureV1> {
        if self.failure.is_none() {
            match self.scan_once(frame) {
                Ok(semantics) => return Ok(semantics),
                Err(error) => {
                    self.budget.refuse();
                    self.failure = Some(error);
                }
            }
        }
        Err(self.failure.as_ref().expect("first refusal retained"))
    }

    /// Validate only the complete canonical wire representation of an inner argument record.
    ///
    /// Success returns the original borrowed slice and allocates no decoded record graph.
    /// This does not authenticate its schema hash, pointer payloads, callable or ledger source.
    /// It consumes this scanner's cumulative frame/work/field/element limits. Refusal closes
    /// the same range and retains its first original error; there is no independent reset.
    ///
    /// # Errors
    /// Returns this scanner's first retained refusal for a non-canonical or over-limit record.
    pub fn validate_argument_record_wire<'frame>(
        &mut self,
        frame: &'frame [u8],
    ) -> Result<&'frame [u8], &ActionFrameScanFailureV1> {
        if self.failure.is_none() {
            match argument_record::scan(frame, &mut self.budget) {
                Ok(()) => return Ok(frame),
                Err(error) => {
                    self.budget.refuse();
                    self.failure = Some(error);
                }
            }
        }
        Err(self.failure.as_ref().expect("first refusal retained"))
    }

    /// Scan a completed producer owner without recapture or early charge release.
    /// A failure retains the same allocation and first scanner refusal. Success
    /// validates the complete Action wire grammar.
    ///
    /// # Errors
    /// Returns the same owner together with this scanner's first retained refusal.
    pub fn scan_owned(
        &mut self,
        frame: RetainedActionFrameBytesV1,
    ) -> Result<
        (
            RetainedActionFrameBytesV1,
            TriggerLifecycleActionSemanticsV1,
        ),
        ActionFrameCaptureFailureV1<'_>,
    > {
        self.scan_owned_impl(frame, false)
    }

    /// Validate a retained Action and every present argument record's inner wire.
    ///
    /// This explicit additional policy is stricter than canonical model validation:
    /// the model stores a bounded opaque ContractArgumentRecord byte vector.
    /// Callable schema hashes and pointer semantics still require execution context.
    /// The policy also follows typed instructions containing nested trigger Actions.
    ///
    /// # Errors
    /// Returns the same owner together with this scanner's first retained refusal,
    /// including a non-canonical inner argument record.
    pub fn scan_owned_with_canonical_arguments(
        &mut self,
        frame: RetainedActionFrameBytesV1,
    ) -> Result<
        (
            RetainedActionFrameBytesV1,
            TriggerLifecycleActionSemanticsV1,
        ),
        ActionFrameCaptureFailureV1<'_>,
    > {
        self.scan_owned_impl(frame, true)
    }

    fn scan_owned_impl(
        &mut self,
        frame: RetainedActionFrameBytesV1,
        validate_arguments: bool,
    ) -> Result<
        (
            RetainedActionFrameBytesV1,
            TriggerLifecycleActionSemanticsV1,
        ),
        ActionFrameCaptureFailureV1<'_>,
    > {
        if self.failure.is_none() && !frame.belongs_to(self.pool) {
            self.budget.refuse();
            self.failure = Some(ActionFrameScanFailureV1::Structure(
                "Action frame belongs to a different original pool",
            ));
        }
        if self.failure.is_none() {
            let scanned = self
                .budget
                .begin_frame(frame.as_bytes().len())
                .map_err(ActionFrameScanFailureV1::from)
                .and_then(|()| {
                    self.scan_admitted_frame_with_arguments(frame.as_bytes(), validate_arguments)
                });
            match scanned {
                Ok(semantics) => return Ok((frame, semantics)),
                Err(error) => {
                    self.budget.refuse();
                    self.failure = Some(error);
                }
            }
        }
        Err(ActionFrameCaptureFailureV1 {
            frame: Some(frame),
            failure: self.failure.as_ref().expect("first refusal retained"),
        })
    }

    /// Admit and retain the complete supplied frame before scanning its bytes.
    ///
    /// Frame/count limits and the full copy-work charge precede backing admission,
    /// allocation and copying. The original pool pays the exact u8 layout. A scan
    /// failure returns that same complete immutable owner and latches its first
    /// original typed error. Admission failure returns no byte owner. A previously
    /// refused scanner performs no allocation, copy or subsequent scan.
    ///
    /// Success validates the complete Action wire grammar but does not authenticate
    /// supplied bytes, their producer,
    /// lifecycle state, journal, retry semantics or an output ceiling. The source
    /// bytes must already have their own acquisition/serialization custody.
    ///
    /// # Errors
    /// Returns the admission refusal without a byte owner, or the captured owner together
    /// with this scanner's first retained refusal.
    pub fn capture_and_scan(
        &mut self,
        source: &[u8],
    ) -> Result<
        (
            RetainedActionFrameBytesV1,
            TriggerLifecycleActionSemanticsV1,
        ),
        ActionFrameCaptureFailureV1<'_>,
    > {
        if self.failure.is_none() {
            match self.capture_and_scan_once(source) {
                Ok(result) => return Ok(result),
                Err((frame, failure)) => {
                    self.budget.refuse();
                    self.failure = Some(failure);
                    return Err(ActionFrameCaptureFailureV1 {
                        frame,
                        failure: self.failure.as_ref().expect("first refusal retained"),
                    });
                }
            }
        }
        Err(ActionFrameCaptureFailureV1 {
            frame: None,
            failure: self.failure.as_ref().expect("first refusal retained"),
        })
    }

    fn capture_and_scan_once(
        &mut self,
        source: &[u8],
    ) -> Result<
        (
            RetainedActionFrameBytesV1,
            TriggerLifecycleActionSemanticsV1,
        ),
        (Option<RetainedActionFrameBytesV1>, ActionFrameScanFailureV1),
    > {
        self.budget
            .begin_frame(source.len())
            .map_err(|failure| (None, failure.into()))?;
        let copy_work = u64::try_from(source.len())
            .map_err(|_| (None, ActionFrameScanFailureV1::from(ScanError::Limit)))?;
        self.budget
            .work(copy_work)
            .map_err(|failure| (None, failure.into()))?;
        let frame = RetainedActionFrameBytesV1::try_copy(source, self.pool)
            .map_err(|failure| (None, ActionFrameScanFailureV1::Capture(failure)))?;
        match self.scan_admitted_frame(frame.as_bytes()) {
            Ok(semantics) => Ok((frame, semantics)),
            Err(failure) => Err((Some(frame), failure)),
        }
    }

    /// Inspect the retained first original failure without changing the range.
    pub fn failure(&self) -> Option<&ActionFrameScanFailureV1> {
        self.failure.as_ref()
    }

    fn scan_once(
        &mut self,
        frame: &[u8],
    ) -> Result<TriggerLifecycleActionSemanticsV1, ActionFrameScanFailureV1> {
        self.budget.begin_frame(frame.len())?;
        self.scan_admitted_frame(frame)
    }

    // Each caller performs exactly one begin_frame before entering this parser.
    // The custody path additionally charges the full copy before allocation.
    fn scan_admitted_frame(
        &mut self,
        frame: &[u8],
    ) -> Result<TriggerLifecycleActionSemanticsV1, ActionFrameScanFailureV1> {
        self.scan_admitted_frame_with_arguments(frame, false)
    }

    fn scan_admitted_frame_with_arguments(
        &mut self,
        frame: &[u8],
        validate_arguments: bool,
    ) -> Result<TriggerLifecycleActionSemanticsV1, ActionFrameScanFailureV1> {
        // Exact schema, flags, padding, uncompressed length and CRC precede any
        // nested interpretation. Ambient layout flags cannot select another grammar.
        if ncore::default_encode_flags() != SCANNER_FLAGS {
            return Err(ActionFrameScanFailureV1::Structure(
                "canonical Action codec flags changed",
            ));
        }
        let payload = norito::canonical_frame_payload::<Action>(frame, SCANNER_FLAGS)?;
        let mut action = Cursor::root(payload, &mut self.budget)?;
        let executable = action.field(&mut self.budget)?;
        let repeats = action.field(&mut self.budget)?;
        let authority = action.field(&mut self.budget)?;
        let filter = action.field(&mut self.budget)?;
        let retry = action.field(&mut self.budget)?;
        let metadata = action.field(&mut self.budget)?;
        action.finish()?;

        crate::transaction::executable::with_canonical_argument_record_validation(
            validate_arguments,
            || {
                scan_executable(
                    executable,
                    &mut self.budget,
                    self.pool,
                    &self.account_work,
                    self.typed_scratch_bytes,
                    validate_arguments,
                )
            },
        )?;
        cursor::repeats(repeats, &mut self.budget)?;
        self.scan_authority(authority)?;
        let (filter_kind, scheduled) = scan_filter(
            filter,
            authority.full_bytes(),
            &mut self.budget,
            self.pool,
            &self.account_work,
            self.typed_scratch_bytes,
        )?;
        let retry = cursor::retry_policy(retry, &mut self.budget)?;
        // This is the real Action constructor invariant, not a lifecycle hint.
        if retry.is_some() && !scheduled {
            return Err(ScanError::InvalidValue.into());
        }
        scan_metadata(metadata, &mut self.budget, self.pool, &self.account_work)?;
        Ok(TriggerLifecycleActionSemanticsV1 {
            filter_kind,
            has_retry_policy: retry.is_some(),
        })
    }

    fn scan_authority(
        &mut self,
        mut authority: Cursor<'_>,
    ) -> Result<(), ActionFrameScanFailureV1> {
        let bytes = authority.full_bytes();
        // Traverse the complete controller shape under the same field, depth and
        // element limits before its physically funded semantic decoder runs.
        match authority.u32(&mut self.budget)? {
            0 => {
                let _public_key = authority.field(&mut self.budget)?;
            }
            1 => {
                let mut policy = authority.field(&mut self.budget)?;
                let mut version = policy.field(&mut self.budget)?;
                version.byte(&mut self.budget)?;
                version.finish()?;
                let mut threshold = policy.field(&mut self.budget)?;
                threshold.byte(&mut self.budget)?;
                threshold.byte(&mut self.budget)?;
                threshold.finish()?;
                let mut members = policy.field(&mut self.budget)?;
                let count = members.u64(&mut self.budget)?;
                self.budget.elements(count)?;
                for _ in 0..count {
                    let mut member = members.field(&mut self.budget)?;
                    let _public_key = member.field(&mut self.budget)?;
                    let mut weight = member.field(&mut self.budget)?;
                    weight.byte(&mut self.budget)?;
                    weight.byte(&mut self.budget)?;
                    weight.finish()?;
                    member.finish()?;
                }
                members.finish()?;
                policy.finish()?;
            }
            _ => return Err(ScanError::UnsupportedAuthority.into()),
        }
        authority.finish()?;
        // Bound the additional payload decode and exact serialization traversals
        // before entering either one. Crypto validation is separately limited by
        // the funded cumulative decoder and its closed key algorithm set.
        let visits = bytes.len().checked_mul(1024).ok_or(ScanError::Limit)?;
        self.budget
            .work(u64::try_from(visits).map_err(|_| ScanError::Limit)?)?;
        let limits = ncore::EncodeWorkLimits {
            max_frame_bytes: bytes.len(),
            max_byte_visits: visits,
            max_operations: visits,
            max_depth: (self.budget.limits.max_depth as usize)
                .saturating_sub(authority.enclosing_depth()),
        };
        let mut destination = AccountDestination::new(self.pool);
        let decode_work = self.account_work.with_enclosing_depth(
            authority.enclosing_depth(),
            self.budget.limits.max_depth as usize,
        )?;
        ncore::with_bounded_encode_work(limits, || {
            Ok(
                decode_work.with(|| -> Result<(), ActionFrameScanFailureV1> {
                    let _payload = ncore::PayloadCtxGuard::enter_with_flags(bytes, SCANNER_FLAGS);
                    destination.decode(bytes, self.pool, None)?;
                    if !destination.belongs_to(self.pool) {
                        return Err(ScanError::InvalidValue.into());
                    }
                    let account = destination
                        .complete
                        .as_ref()
                        .ok_or(ScanError::InvalidValue)?;
                    // Exact stream comparison rejects normalization without a
                    // second account or replacement output allocation.
                    norito::codec::verify_exact_payload(&account.value, bytes)?;
                    Ok(())
                }),
            )
        })
        .map_err(ActionFrameScanFailureV1::Encode)?
        // Destination drops the actual account before its retained charge.
    }
}

impl super::TriggerLifecycleActionValidatorV1 for BoundedActionFrameScannerV1<'_> {
    fn validate_canonical_action(
        &mut self,
        frame: &[u8],
        payload: &[u8],
    ) -> Result<TriggerLifecycleActionSemanticsV1, String> {
        if self.failure.is_none() {
            let checked = (|| -> Result<(), ActionFrameScanFailureV1> {
                self.budget
                    .work(u64::try_from(frame.len()).map_err(|_| ScanError::Limit)?)?;
                let actual = norito::canonical_frame_payload::<Action>(frame, SCANNER_FLAGS)?;
                if actual.as_ptr() != payload.as_ptr() || actual.len() != payload.len() {
                    return Err(ActionFrameScanFailureV1::Structure(
                        "Action payload does not belong to its original frame",
                    ));
                }
                Ok(())
            })();
            if let Err(failure) = checked {
                self.budget.refuse();
                self.failure = Some(failure);
            }
        }
        self.scan(frame).map_err(|failure| match failure {
            ActionFrameScanFailureV1::Structure(message) => (*message).to_owned(),
            _ => "bounded canonical Action validation refused".to_owned(),
        })
    }
}

fn typed_subtree(
    value: Cursor<'_>,
    budget: &mut Budget,
    pool: &AllocationBudget,
    decode_work: &DecodeBudgetContext,
    scratch_bytes: usize,
    validate: impl FnOnce(&[u8], ncore::EncodeWorkLimits) -> Result<(), norito::Error>,
) -> Result<(), ActionFrameScanFailureV1> {
    let bytes = value.full_bytes();
    // Budget every admitted subtree before typed parsing. The canonical writer
    // has the same hard ceiling; it cannot do unbounded work during comparison.
    let visits = bytes.len().checked_mul(1024).ok_or(ScanError::Limit)?;
    budget.work(u64::try_from(visits).map_err(|_| ScanError::Limit)?)?;
    let limits = ncore::EncodeWorkLimits {
        max_frame_bytes: bytes.len(),
        max_byte_visits: visits,
        max_operations: visits,
        max_depth: (budget.limits.max_depth as usize).saturating_sub(value.enclosing_depth()),
    };
    let decode_work = decode_work
        .with_enclosing_depth(value.enclosing_depth(), budget.limits.max_depth as usize)?;
    let scratch = pool
        .try_reserve_bytes(scratch_bytes)
        .map_err(ActionFrameScanFailureV1::Scratch)?;
    let result = cursor::with_typed_frame_budget(budget, || {
        ncore::with_prepaid_validation(&scratch, &decode_work, || {
            crate::isi::with_lifecycle_instruction_validation(|| validate(bytes, limits))
        })
    });
    if let Err(error) = result {
        return Err(ActionFrameScanFailureV1::Typed { error, scratch });
    }
    Ok(())
}

fn validate_typed_payload<T>(
    bytes: &[u8],
    limits: ncore::EncodeWorkLimits,
) -> Result<(), norito::Error>
where
    T: norito::SerializePayload + for<'de> norito::DeserializePayload<'de>,
{
    ncore::with_bounded_encode_work(limits, || {
        let _flags = ncore::DecodeFlagsGuard::enter(SCANNER_FLAGS);
        let (value, used) = ncore::decode_field_canonical::<T>(bytes)?;
        if used != bytes.len() {
            return Err(norito::Error::LengthMismatch);
        }
        norito::codec::verify_exact_payload(&value, bytes)
    })
    .map_err(|error| match error {
        ncore::BoundedEncodeWorkError::Codec(error) => error,
        ncore::BoundedEncodeWorkError::Refused(reason) => norito::Error::InvalidValue {
            context: match reason {
                ncore::EncodeWorkRefusal::ByteVisits => "Action subtree byte visit limit",
                ncore::EncodeWorkRefusal::Operations => "Action subtree operation limit",
                ncore::EncodeWorkRefusal::Depth => "Action subtree serializer depth limit",
                ncore::EncodeWorkRefusal::FrameBytes => "Action subtree frame byte limit",
                ncore::EncodeWorkRefusal::NestedScope => "Action subtree nested encoder scope",
                ncore::EncodeWorkRefusal::Source => "Action subtree source encoding refusal",
            },
        },
    })
}

fn scan_instruction(
    mut value: Cursor<'_>,
    budget: &mut Budget,
    pool: &AllocationBudget,
    decode_work: &DecodeBudgetContext,
    scratch_bytes: usize,
) -> Result<(), ActionFrameScanFailureV1> {
    let mut name = value.field(budget)?;
    let wire_id = name.string(budget)?;
    name.finish()?;
    let mut framed = value.field(budget)?;
    let bytes = framed.byte_vector(budget)?;
    framed.finish()?;
    value.finish()?;
    budget.begin_frame(bytes.len())?;
    // Registry lookup traverses one fixed release inventory without constructing
    // the ordinary mutable registry or copying an instruction's outer pair.
    budget.work(65_536)?;
    let frame = framed.nested_root(bytes, budget)?;
    typed_subtree(
        frame,
        budget,
        pool,
        decode_work,
        scratch_bytes,
        |bytes, limits| crate::isi::registry::validate_lifecycle_frame(wire_id, bytes, limits),
    )
}

fn scan_instruction_sequence(
    mut sequence: Cursor<'_>,
    budget: &mut Budget,
    pool: &AllocationBudget,
    decode_work: &DecodeBudgetContext,
    scratch_bytes: usize,
) -> Result<(), ActionFrameScanFailureV1> {
    let count = sequence.u64(budget)?;
    budget.elements(count)?;
    for _ in 0..count {
        scan_instruction(
            sequence.field(budget)?,
            budget,
            pool,
            decode_work,
            scratch_bytes,
        )?;
    }
    sequence.finish()?;
    Ok(())
}

fn scan_bytecode(mut value: Cursor<'_>, budget: &mut Budget) -> Result<(), ScanError> {
    let mut bytes = value.field(budget)?;
    bytes.byte_vector(budget)?;
    bytes.finish()?;
    value.finish()
}

fn scan_hash(mut value: Cursor<'_>, budget: &mut Budget) -> Result<(), ScanError> {
    for _ in 0..3 {
        value.u64(budget)?;
    }
    let last = value.u64(budget)?;
    value.finish()?;
    if last & (1u64 << 56) == 0 {
        return Err(ScanError::InvalidValue);
    }
    Ok(())
}

fn scan_executable(
    mut value: Cursor<'_>,
    budget: &mut Budget,
    pool: &AllocationBudget,
    decode_work: &DecodeBudgetContext,
    scratch_bytes: usize,
    validate_arguments: bool,
) -> Result<(), ActionFrameScanFailureV1> {
    match value.u32(budget)? {
        0 => scan_instruction_sequence(
            value.field(budget)?,
            budget,
            pool,
            decode_work,
            scratch_bytes,
        )?,
        1 => scan_call(value.field(budget)?, budget, validate_arguments)?,
        2 => scan_bytecode(value.field(budget)?, budget)?,
        3 => {
            let mut proved = value.field(budget)?;
            scan_bytecode(proved.field(budget)?, budget)?;
            scan_instruction_sequence(
                proved.field(budget)?,
                budget,
                pool,
                decode_work,
                scratch_bytes,
            )?;
            scan_hash(proved.field(budget)?, budget)?;
            scan_hash(proved.field(budget)?, budget)?;
            proved.finish()?;
        }
        4 => {
            let mut sequence = value.field(budget)?;
            let count = sequence.u64(budget)?;
            budget.elements(count)?;
            for _ in 0..count {
                let mut item = sequence.field(budget)?;
                match item.u32(budget)? {
                    0 => scan_instruction(
                        item.field(budget)?,
                        budget,
                        pool,
                        decode_work,
                        scratch_bytes,
                    )?,
                    1 => scan_call(item.field(budget)?, budget, validate_arguments)?,
                    _ => return Err(ScanError::UnsupportedExecutable.into()),
                }
                item.finish()?;
            }
            sequence.finish()?;
        }
        _ => return Err(ScanError::UnsupportedExecutable.into()),
    }
    value.finish()?;
    Ok(())
}

fn scan_call(
    value: Cursor<'_>,
    budget: &mut Budget,
    validate_arguments: bool,
) -> Result<(), ActionFrameScanFailureV1> {
    if let Some((record, enclosing)) = scan_contract_invocation_record(value, budget)? {
        if validate_arguments {
            argument_record::scan_nested(record, enclosing, budget)?;
        }
    }
    Ok(())
}

// Borrow the invocation fields before canonical argument-record validation.
// Callable schema authorization and deployed-code binding remain execution work.
#[cfg(test)]
fn scan_contract_invocation(value: Cursor<'_>, budget: &mut Budget) -> Result<(), ScanError> {
    scan_contract_invocation_record(value, budget).map(|_| ())
}

fn scan_contract_invocation_record<'frame>(
    mut value: Cursor<'frame>,
    budget: &mut Budget,
) -> Result<Option<(&'frame [u8], Cursor<'frame>)>, ScanError> {
    scan_contract_address(value.field(budget)?, budget)?;
    let mut hash = value.field(budget)?;
    // Hash serializes exactly 32 raw bytes. Preserve its original marker rule;
    // it is a wire invariant, not authentication of a code-hash preimage.
    for _ in 0..3 {
        hash.u64(budget)?;
    }
    let last = hash.u64(budget)?;
    hash.finish()?;
    if last & (1u64 << 56) == 0 {
        return Err(ScanError::InvalidValue);
    }

    let mut entrypoint = value.field(budget)?;
    // The model stores an arbitrary UTF-8 selector. Callable/schema admission
    // owns selector semantics; the lifecycle scanner establishes exact wire.
    entrypoint.string(budget)?;
    entrypoint.finish()?;

    let mut arguments = value.field(budget)?;
    let Some(mut record) = arguments.option(budget)? else {
        arguments.finish()?;
        value.finish()?;
        return Ok(None);
    };
    // ContractArgumentRecord delegates directly to Vec<u8>. Probe the fixed
    // sequence length before any payload traversal, then let byte_vector enforce
    // the original framing and cumulative element/work budgets. The repeated
    // eight-byte prefix read is deliberately charged twice.
    let mut prefix = record;
    let count = prefix.u64(budget)?;
    if count > crate::transaction::executable::MAX_CONTRACT_ARGUMENT_RECORD_BYTES as u64 {
        return Err(ScanError::UnsupportedExecutable);
    }
    let original_record = record.byte_vector(budget)?;
    record.finish()?;
    arguments.finish()?;
    value.finish()?;
    // Carry this actual field cursor into the nested frame; do not reset depth.
    Ok(Some((original_record, record)))
}

fn scan_contract_address(mut value: Cursor<'_>, budget: &mut Budget) -> Result<(), ScanError> {
    use crate::smart_contract::{CONTRACT_ADDRESS_HRP, CONTRACT_ADDRESS_LITERAL_LEN_V1};

    let text = value.bounded_string(CONTRACT_ADDRESS_LITERAL_LEN_V1, budget)?;
    value.finish()?;
    if text.len() != CONTRACT_ADDRESS_LITERAL_LEN_V1 {
        return Err(ScanError::InvalidValue);
    }
    // The original V1 literal has 60 ASCII bytes, six HRP bytes and 29 decoded
    // bytes. Prepay HRP and character validation, the checksum pass and
    // streaming payload conversion. The borrowed decoder carries fixed stack
    // state only; never call the allocating decode or format its error. This
    // conservative debit is independent of contents.
    budget.work(4096)?;
    // Accepts a Bech32m or Bech32 checksum, as the contract-address parser does.
    let decoded = crate::bech32::decode_borrowed(text).map_err(|_| ScanError::InvalidValue)?;
    if decoded.hrp().as_str() != CONTRACT_ADDRESS_HRP {
        return Err(ScanError::InvalidValue);
    }
    let mut bytes = decoded.bytes();
    if bytes.next() != Some(1) {
        return Err(ScanError::InvalidValue);
    }
    for _ in 0..28 {
        if bytes.next().is_none() {
            return Err(ScanError::InvalidValue);
        }
    }
    if bytes.next().is_some() {
        return Err(ScanError::InvalidValue);
    }
    Ok(())
}

fn scan_filter(
    mut value: Cursor<'_>,
    original_authority: &[u8],
    budget: &mut Budget,
    pool: &AllocationBudget,
    decode_work: &DecodeBudgetContext,
    scratch_bytes: usize,
) -> Result<(TriggerLifecycleStateKindV1, bool), ActionFrameScanFailureV1> {
    let result = match value.u32(budget)? {
        0 => {
            typed_subtree(
                value.field(budget)?,
                budget,
                pool,
                decode_work,
                scratch_bytes,
                validate_typed_payload::<crate::events::pipeline::PipelineEventFilterBox>,
            )?;
            (TriggerLifecycleStateKindV1::Pipeline, false)
        }
        1 => {
            let data = value.field(budget)?;
            let mut tag = data;
            if tag.u32(budget)? == 0 {
                tag.finish()?;
            } else {
                typed_subtree(
                    data,
                    budget,
                    pool,
                    decode_work,
                    scratch_bytes,
                    validate_typed_payload::<crate::events::data::DataEventFilter>,
                )?;
            }
            (TriggerLifecycleStateKindV1::Data, false)
        }
        2 => {
            let scheduled = cursor::time_filter(value.field(budget)?, budget)?;
            (TriggerLifecycleStateKindV1::Time, scheduled)
        }
        3 => {
            let mut by_call = value.field(budget)?;
            let mut id = by_call.field(budget)?;
            if let Some(mut trigger_id) = id.option(budget)? {
                scan_name(trigger_id.field(budget)?, budget, pool)?;
                trigger_id.finish()?;
            }
            id.finish()?;
            let mut authority = by_call.field(budget)?;
            let Some(authority_value) = authority.option(budget)? else {
                // Action::ensure_authority rewrites None to Some, so None is
                // not a canonical persistent Action even when decoding accepts it.
                return Err(ScanError::NonCanonical.into());
            };
            let candidate = authority_value.full_bytes();
            budget.work(u64::try_from(candidate.len()).map_err(|_| ScanError::Limit)?)?;
            // The compared original subtree was already completely validated.
            if candidate != original_authority {
                return Err(ScanError::InvalidValue.into());
            }
            authority.finish()?;
            by_call.finish()?;
            (TriggerLifecycleStateKindV1::ByCall, false)
        }
        // TriggerCompleted notifications are forbidden by Action.
        _ => return Err(ScanError::UnsupportedFilter.into()),
    };
    value.finish()?;
    Ok(result)
}

fn scan_name<'a>(
    mut value: Cursor<'a>,
    budget: &mut Budget,
    pool: &AllocationBudget,
) -> Result<&'a str, ActionFrameScanFailureV1> {
    use iroha_model_base::name::{MAX_NAME_BYTES, Name};
    let text = value.bounded_string(MAX_NAME_BYTES, budget)?;
    value.finish()?;
    // The same pinned NFC implementation used by Name owns its audited bounded
    // scratch for the entire call. No replacement string or Name is constructed.
    budget.work(u64::try_from(text.len()).map_err(|_| ScanError::Limit)? * 8)?;
    let scratch = Name::canonical_validation_scratch_bytes(text);
    budget.work(u64::try_from(scratch).map_err(|_| ScanError::Limit)?)?;
    let _scratch = pool
        .try_reserve_bytes(scratch)
        .map_err(ActionFrameScanFailureV1::Scratch)?;
    Name::validate_canonical(text).map_err(|_| ScanError::InvalidValue)?;
    Ok(text)
}

struct JsonScratch(ChargedBuffer<u8>);
impl AsRef<[u8]> for JsonScratch {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}
impl AsMut<[u8]> for JsonScratch {
    fn as_mut(&mut self) -> &mut [u8] {
        self.0.as_mut_slice()
    }
}
impl From<norito::json::Error> for ActionFrameScanFailureV1 {
    fn from(error: norito::json::Error) -> Self {
        Self::Codec(error.into_core_error())
    }
}

fn scan_metadata(
    mut value: Cursor<'_>,
    budget: &mut Budget,
    pool: &AllocationBudget,
    decode_work: &DecodeBudgetContext,
) -> Result<(), ActionFrameScanFailureV1> {
    let count = value.u64(budget)?;
    budget.elements(count)?;
    let mut previous: Option<&str> = None;
    for _ in 0..count {
        let mut entry = value.field(budget)?;
        let name = scan_name(entry.field(budget)?, budget, pool)?;
        if let Some(prior) = previous {
            budget
                .work(u64::try_from(prior.len().min(name.len())).map_err(|_| ScanError::Limit)?)?;
            if prior >= name {
                return Err(ScanError::NonCanonical.into());
            }
        }
        previous = Some(name);
        let mut json = entry.field(budget)?;
        let mut string = json.field(budget)?;
        let text = string.bounded_string(MAX_JSON_BYTES, budget)?;
        string.finish()?;
        json.finish()?;
        entry.finish()?;
        // Lexical preflight, visitors, string/number kernels and exact writer
        // comparison share the original cumulative decoder scope. Reserve a
        // conservative fixed number of byte visits before entering those kernels.
        budget.work(
            u64::try_from(text.len())
                .map_err(|_| ScanError::Limit)?
                .checked_mul(32)
                .ok_or(ScanError::Limit)?,
        )?;
        decode_work
            .with_enclosing_depth(string.enclosing_depth(), budget.limits.max_depth as usize)?
            .with(|| {
                norito::json::validate_canonical_json_with_buffer(
                    text,
                    |length| -> Result<JsonScratch, ActionFrameScanFailureV1> {
                        let mut bytes = ChargedBuffer::new(length, pool)
                            .map_err(ActionFrameScanFailureV1::Capture)?;
                        for _ in 0..length {
                            bytes.push_reserved(0);
                        }
                        Ok(JsonScratch(bytes))
                    },
                )
            })?;
    }
    value.finish()?;
    Ok(())
}

#[cfg(test)]
fn test_scan_error(error: ActionFrameScanFailureV1) -> ScanError {
    match error {
        ActionFrameScanFailureV1::Structure(message) => [
            ScanError::Refused,
            ScanError::Limit,
            ScanError::Length,
            ScanError::NonCanonical,
            ScanError::InvalidValue,
            ScanError::UnsupportedExecutable,
            ScanError::UnsupportedAuthority,
            ScanError::UnsupportedFilter,
        ]
        .into_iter()
        .find(|kind| kind.message() == message)
        .unwrap(),
        _ => ScanError::NonCanonical,
    }
}

#[cfg(test)]
fn test_scan_metadata(value: Cursor<'_>, budget: &mut Budget) -> Result<(), ScanError> {
    let pool = AllocationBudget::new(8 * 1024 * 1024);
    let work = DecodeBudgetContext::try_new_owned(
        DecodeLimits::new(1_000_000, 4 * 1024 * 1024, 8_000_000, 8 * 1024 * 1024, 32),
        &pool,
    )
    .unwrap();
    scan_metadata(value, budget, &pool, &work).map_err(test_scan_error)
}

#[cfg(test)]
fn test_scan_name<'a>(value: Cursor<'a>, budget: &mut Budget) -> Result<&'a str, ScanError> {
    let pool = AllocationBudget::new(8 * 1024 * 1024);
    scan_name(value, budget, &pool).map_err(test_scan_error)
}

#[cfg(test)]
fn test_scan_filter(
    value: Cursor<'_>,
    authority: &[u8],
    budget: &mut Budget,
) -> Result<(TriggerLifecycleStateKindV1, bool), ScanError> {
    let pool = AllocationBudget::new(8 * 1024 * 1024);
    let work = DecodeBudgetContext::try_new_owned(
        DecodeLimits::new(100_000, 2_000_000, 100_000, 2_000_000, 32),
        &pool,
    )
    .unwrap();
    scan_filter(value, authority, budget, &pool, &work, 2_000_000).map_err(test_scan_error)
}

#[cfg(test)]
fn test_scan_executable(value: Cursor<'_>, budget: &mut Budget) -> Result<(), ScanError> {
    let pool = AllocationBudget::new(8 * 1024 * 1024);
    let work = DecodeBudgetContext::try_new_owned(
        DecodeLimits::new(100_000, 2_000_000, 100_000, 2_000_000, 32),
        &pool,
    )
    .unwrap();
    scan_executable(value, budget, &pool, &work, 2_000_000, false).map_err(test_scan_error)
}

#[cfg(test)]
#[path = "trigger_action_scanner_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "action_frame_custody_tests.rs"]
mod custody_tests;

#[cfg(test)]
#[path = "action_frame_owned_scan_tests.rs"]
mod owned_scan_tests;

#[cfg(test)]
#[path = "action_frame_registration_batch_tests.rs"]
mod registration_batch_tests;

#[cfg(test)]
#[path = "action_frame_argument_record_tests.rs"]
mod argument_record_tests;
