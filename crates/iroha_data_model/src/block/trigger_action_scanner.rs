//! Concrete, deliberately incomplete scanner for the unreleased Action frame carrier.
//!
//! This scanner is not installed as `TriggerLifecycleActionValidatorV1`. It proves
//! the supported closed subset and refuses all other nested kinds. Full variant
//! coverage, codec/schema scratch custody and the execution owner's complete
//! physical ceiling must be closed before that trait is implemented or activated.
//! The legacy scan paths check only the outer argument-record bytes. The explicit
//! `scan_owned_with_canonical_arguments` seam also checks the inner record/atom wire,
//! but not the authenticated callable schema or pointer payload semantics.
//! Registration numeric values are a structural subset; registration and execution
//! semantics remain separate.
//! It never constructs an owned Action, executable, filter, metadata map or JSON tree.

#[path = "action_frame_cursor.rs"]
mod cursor;
#[path = "action_frame_custody.rs"]
mod custody;
#[path = "action_frame_argument_record.rs"]
mod argument_record;

use crate::{
    account::prepared_decode::{AccountDecodeError, AccountDestination},
    trigger::action::Action,
};
use iroha_allocation::AllocationBudget;
use iroha_primitives::json::MAX_JSON_BYTES;
use norito::core::{self as ncore, DecodeBudgetContext, DecodeLimits};

use super::{TriggerLifecycleActionSemanticsV1, TriggerLifecycleStateKindV1};
pub use cursor::ActionScannerLimitsV1;
use cursor::{Budget, Cursor, ScanError};
pub use custody::{ActionFrameProductionFailureV1, RetainedActionFrameBytesV1};

const SCANNER_FLAGS: u8 = ncore::header_flags::COMPACT_LEN;

/// The first original failure remains owned by the scanner until it is dropped.
#[derive(Debug)]
pub enum ActionFrameScanFailureV1 {
    /// Fixed structural/unsupported-kind refusal, without an allocated diagnostic.
    Structure(&'static str),
    /// Original Norito envelope, resource or canonical-comparison failure.
    Codec(norito::Error),
    /// Original funded account decoder failure, without replacement or retry.
    Authority(AccountDecodeError),
    /// Original exact-backing admission or allocator refusal, before any copy.
    Capture(iroha_allocation::ChargedBufferError),
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
}

impl<'pool> BoundedActionFrameScannerV1<'pool> {
    /// Fund one cumulative account decoder control before any nested decode.
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
        })
    }

    /// Validate a full frame against the fixed canonical Action codec.
    ///
    /// A refusal permanently closes this scanner. Subsequent calls return the
    /// same first original failure without parsing, allocating or resetting work.
    /// Success authenticates no ledger state and does not establish frame custody.
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
    /// remains restricted to this scanner's supported subset.
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

    /// Scan the same Action owner and the canonical wire of its supported inner record.
    ///
    /// Unlike `scan_owned`, a supported one-call Batch must contain a canonical
    /// `EntrypointArgumentRecordV1`, not merely bounded opaque argument bytes.
    /// The inner frame is borrowed directly from this Action owner, shares every
    /// cumulative limit, and counts as another frame. Both success and refusal
    /// retain the exact original owner and pool charge. This still authenticates
    /// no callable schema, pointer semantics, registration or lifecycle mutation.
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
        validate_argument_wire: bool,
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
            let scanned = if validate_argument_wire {
                self.scan_once_with_canonical_arguments(frame.as_bytes())
            } else {
                self.scan_once(frame.as_bytes())
            };
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
    /// Success means only the existing supported subset passed. It does not make
    /// a globally validated Action or authenticate supplied bytes, their producer,
    /// lifecycle state, journal, retry semantics or an output ceiling. The source
    /// bytes must already have their own acquisition/serialization custody.
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

    fn scan_once_with_canonical_arguments(
        &mut self,
        frame: &[u8],
    ) -> Result<TriggerLifecycleActionSemanticsV1, ActionFrameScanFailureV1> {
        self.budget.begin_frame(frame.len())?;
        self.scan_admitted_frame_with_arguments(frame, true)
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
        validate_argument_wire: bool,
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

        if validate_argument_wire {
            if let Some((record, enclosing)) =
                scan_executable_with_record(executable, &mut self.budget)?
            {
                argument_record::scan_nested(record, enclosing, &mut self.budget)?;
            }
        } else {
            scan_executable(executable, &mut self.budget)?;
        }
        cursor::repeats(repeats, &mut self.budget)?;
        self.scan_authority(authority)?;
        let (filter_kind, scheduled) =
            scan_filter(filter, authority.full_bytes(), &mut self.budget)?;
        let retry = cursor::retry_policy(retry, &mut self.budget)?;
        // This is the real Action constructor invariant, not a lifecycle hint.
        if retry.is_some() && !scheduled {
            return Err(ScanError::InvalidValue.into());
        }
        scan_metadata(metadata, &mut self.budget)?;
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
        // Multisig has additional sequence/sort/scratch obligations. Refuse it
        // until that concrete branch has its own cumulative scanner coverage.
        if authority.u32(&mut self.budget)? != 0 {
            return Err(ScanError::UnsupportedAuthority.into());
        }
        let _public_key = authority.field(&mut self.budget)?;
        authority.finish()?;
        // Bound the additional payload decode and exact serialization traversals
        // before entering either one. Crypto validation is separately limited by
        // the funded cumulative decoder and its closed key algorithm set.
        let work = u64::try_from(bytes.len()).map_err(|_| ScanError::Limit)?;
        self.budget
            .work(work.checked_mul(4).ok_or(ScanError::Limit)?)?;
        let mut destination = AccountDestination::new(self.pool);
        self.account_work
            .with(|| -> Result<(), ActionFrameScanFailureV1> {
                let _payload = ncore::PayloadCtxGuard::enter_with_flags(bytes, SCANNER_FLAGS);
                destination.decode(bytes, self.pool, None)?;
                if !destination.belongs_to(self.pool) {
                    return Err(ScanError::InvalidValue.into());
                }
                let account = destination
                    .complete
                    .as_ref()
                    .ok_or(ScanError::InvalidValue)?;
                // Exact stream comparison rejects accepted-but-normalized account
                // encodings without allocating a second account or output Vec.
                norito::codec::verify_exact_payload(&account.value, bytes)?;
                Ok(())
            })
        // Destination drops the actual account before its retained charge.
    }
}

fn scan_executable(value: Cursor<'_>, budget: &mut Budget) -> Result<(), ScanError> {
    scan_executable_with_record(value, budget).map(|_| ())
}

fn scan_executable_with_record<'frame>(
    mut value: Cursor<'frame>,
    budget: &mut Budget,
) -> Result<Option<(&'frame [u8], Cursor<'frame>)>, ScanError> {
    let mut argument_record = None;
    match value.u32(budget)? {
        0 => {
            // ConstVec uses the element sequence codec even for empty values.
            // A dynamic InstructionBox registry must never be skipped by length.
            let mut sequence = value.field(budget)?;
            let count = sequence.u64(budget)?;
            budget.elements(count)?;
            if count != 0 {
                return Err(ScanError::UnsupportedExecutable);
            }
            sequence.finish()?;
        }
        4 => {
            let mut sequence = value.field(budget)?;
            let count = sequence.u64(budget)?;
            budget.elements(count)?;
            match count {
                0 => {} // Preserve the original empty Batch subset.
                1 => {
                    let mut item = sequence.field(budget)?;
                    if item.u32(budget)? != 1 {
                        return Err(ScanError::UnsupportedExecutable);
                    }
                    argument_record = Some(scan_contract_invocation_record(
                        item.field(budget)?,
                        budget,
                    )?);
                    item.finish()?;
                }
                _ => return Err(ScanError::UnsupportedExecutable),
            }
            sequence.finish()?;
        }
        2 => {
            let mut bytecode = value.field(budget)?;
            let mut bytes = bytecode.field(budget)?;
            let _ = bytes.byte_vector(budget)?;
            bytes.finish()?;
            bytecode.finish()?;
        }
        _ => return Err(ScanError::UnsupportedExecutable),
    }
    value.finish()?;
    Ok(argument_record)
}

// This is the narrow one-call Batch used by the scheduled-contract SDK. It
// checks original outer field grammar only, not the argument record's nested
// schema/atoms, deployed code binding, execution or lifecycle semantics.
#[cfg(test)]
fn scan_contract_invocation(value: Cursor<'_>, budget: &mut Budget) -> Result<(), ScanError> {
    scan_contract_invocation_record(value, budget).map(|_| ())
}

fn scan_contract_invocation_record<'frame>(
    mut value: Cursor<'frame>,
    budget: &mut Budget,
) -> Result<(&'frame [u8], Cursor<'frame>), ScanError> {
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
    let name = entrypoint.bounded_string(255, budget)?;
    entrypoint.finish()?;
    // Retain the SDK's nonempty, untrimmed UTF-8 selector subset. Charge both
    // boundary traversals before trim; no selector String is constructed.
    budget.work(
        u64::try_from(name.len())
            .map_err(|_| ScanError::Limit)?
            .checked_mul(2)
            .ok_or(ScanError::Limit)?,
    )?;
    if name.is_empty() || name.trim() != name {
        return Err(ScanError::UnsupportedExecutable);
    }

    let mut arguments = value.field(budget)?;
    let Some(mut record) = arguments.option(budget)? else {
        return Err(ScanError::UnsupportedExecutable);
    };
    // ContractArgumentRecord delegates directly to Vec<u8>. Probe the fixed
    // sequence length before any payload traversal, then let byte_vector enforce
    // the original framing and cumulative element/work budgets. The repeated
    // eight-byte prefix read is deliberately charged twice.
    let mut prefix = record;
    let count = prefix.u64(budget)?;
    if count == 0
        || count > crate::transaction::executable::MAX_CONTRACT_ARGUMENT_RECORD_BYTES as u64
    {
        return Err(ScanError::UnsupportedExecutable);
    }
    let original_record = record.byte_vector(budget)?;
    record.finish()?;
    arguments.finish()?;
    value.finish()?;
    // Carry this actual field cursor into the nested frame; do not reset depth.
    Ok((original_record, record))
}

fn scan_contract_address(mut value: Cursor<'_>, budget: &mut Budget) -> Result<(), ScanError> {
    use crate::smart_contract::{CONTRACT_ADDRESS_HRP, CONTRACT_ADDRESS_LITERAL_LEN_V1};
    use bech32::{Bech32, Bech32m, primitives::decode::UncheckedHrpstring};

    let text = value.bounded_string(CONTRACT_ADDRESS_LITERAL_LEN_V1, budget)?;
    value.finish()?;
    if text.len() != CONTRACT_ADDRESS_LITERAL_LEN_V1 {
        return Err(ScanError::InvalidValue);
    }
    // The original V1 literal has 60 ASCII bytes, six HRP bytes and 29 decoded
    // bytes. Prepay fixed Hrp initialization/copies plus character validation,
    // both finite checksum passes and streaming payload conversion. The borrowed
    // parser carries fixed stack state only; never call allocating decode/parse
    // or format its error. This conservative debit is independent of contents.
    budget.work(4096)?;
    let unchecked = UncheckedHrpstring::new(text).map_err(|_| ScanError::InvalidValue)?;
    if unchecked.validate_checksum::<Bech32m>().is_err()
        && !unchecked.has_valid_checksum::<Bech32>()
    {
        return Err(ScanError::InvalidValue);
    }
    // Both original checksum algorithms have six characters, as in bech32::decode.
    let checked = unchecked.remove_checksum::<Bech32m>();
    if checked.hrp().as_str() != CONTRACT_ADDRESS_HRP {
        return Err(ScanError::InvalidValue);
    }
    let mut bytes = checked.byte_iter();
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
) -> Result<(TriggerLifecycleStateKindV1, bool), ScanError> {
    let result = match value.u32(budget)? {
        1 => {
            let mut data = value.field(budget)?;
            if data.u32(budget)? != 0 {
                return Err(ScanError::UnsupportedFilter);
            }
            data.finish()?;
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
                scan_name(trigger_id.field(budget)?, budget)?;
                trigger_id.finish()?;
            }
            id.finish()?;
            let mut authority = by_call.field(budget)?;
            let Some(authority_value) = authority.option(budget)? else {
                // Action::ensure_authority rewrites None to Some, so None is
                // not a canonical persistent Action even when decoding accepts it.
                return Err(ScanError::NonCanonical);
            };
            let candidate = authority_value.full_bytes();
            budget.work(u64::try_from(candidate.len()).map_err(|_| ScanError::Limit)?)?;
            // The compared original subtree was already completely validated.
            if candidate != original_authority {
                return Err(ScanError::InvalidValue);
            }
            authority.finish()?;
            by_call.finish()?;
            (TriggerLifecycleStateKindV1::ByCall, false)
        }
        // Pipeline remains unsupported. TriggerCompleted is forbidden by Action.
        _ => return Err(ScanError::UnsupportedFilter),
    };
    value.finish()?;
    Ok(result)
}

fn scan_name<'a>(mut value: Cursor<'a>, budget: &mut Budget) -> Result<&'a str, ScanError> {
    let text = value.string(budget)?;
    value.finish()?;
    budget.work(u64::try_from(text.len()).map_err(|_| ScanError::Limit)?)?;
    // Visible ASCII is already NFC and avoids the unowned ICU normalizer scratch.
    if text.is_empty()
        || text.len() > 255
        || text
            .bytes()
            .any(|byte| !(0x21..=0x7e).contains(&byte) || matches!(byte, b'@' | b'#' | b'$'))
    {
        return Err(ScanError::UnsupportedMetadata);
    }
    Ok(text)
}

fn scan_metadata(mut value: Cursor<'_>, budget: &mut Budget) -> Result<(), ScanError> {
    let count = value.u64(budget)?;
    budget.elements(count)?;
    let mut previous: Option<&str> = None;
    for _ in 0..count {
        let mut entry = value.field(budget)?;
        let name = scan_name(entry.field(budget)?, budget)?;
        if let Some(prior) = previous {
            budget
                .work(u64::try_from(prior.len().min(name.len())).map_err(|_| ScanError::Limit)?)?;
            if prior >= name {
                return Err(ScanError::NonCanonical);
            }
        }
        previous = Some(name);
        // Json is a derived one-field JsonWireRef containing a canonical string.
        let mut json = entry.field(budget)?;
        let mut string = json.field(budget)?;
        // Json's fixed V1 byte ceiling is independent of caller scan limits.
        // Refuse from the length prefix before UTF-8 or canonical-text traversal.
        let text = string.bounded_string(MAX_JSON_BYTES, budget)?;
        string.finish()?;
        json.finish()?;
        entry.finish()?;
        budget.work(u64::try_from(text.len()).map_err(|_| ScanError::Limit)?)?;
        // These literals and unescaped printable ASCII strings are a closed
        // canonical subset. Only the two registration keys additionally accept
        // canonical decimal u64. This scanner does not enforce registration rules.
        if !matches!(text, "null" | "true" | "false" | "[]" | "{}") {
            let bytes = text.as_bytes();
            if bytes.len() < 2
                || bytes[0] != b'"'
                || bytes[bytes.len() - 1] != b'"'
                || bytes[1..bytes.len() - 1]
                    .iter()
                    .any(|byte| !(0x20..=0x7e).contains(byte) || matches!(*byte, b'"' | b'\\'))
            {
                // Preserve the existing nonnumeric subset at every name, including
                // reserved names. Only add numbers at the original registration
                // keys; the producer enforces the narrower exact numeric pair.
                budget.work(
                    u64::try_from(name.len())
                        .map_err(|_| ScanError::Limit)?
                        .checked_mul(2)
                        .ok_or(ScanError::Limit)?,
                )?;
                if !matches!(name, "__registered_at_ms" | "__registered_block_height")
                    || !canonical_registration_u64(text, budget)?
                {
                    return Err(ScanError::UnsupportedMetadata);
                }
            }
        }
    }
    value.finish()
}

fn canonical_registration_u64(text: &str, budget: &mut Budget) -> Result<bool, ScanError> {
    // Json::from(u64) emits decimal text inside JsonWireRef, not an integer
    // payload. Check the original spelling without allocation, normalization or
    // dynamic parse errors. Charge the one finite digit pass before reading it.
    if text.is_empty() || text.len() > 20 {
        return Ok(false);
    }
    budget.work(u64::try_from(text.len()).map_err(|_| ScanError::Limit)?)?;
    let mut number = 0u64;
    for (index, byte) in text.bytes().enumerate() {
        if !byte.is_ascii_digit() || (index == 0 && byte == b'0' && text.len() != 1) {
            return Ok(false);
        }
        let Some(next) = number
            .checked_mul(10)
            .and_then(|n| n.checked_add(u64::from(byte - b'0')))
        else {
            return Ok(false);
        };
        number = next;
    }
    Ok(true)
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
