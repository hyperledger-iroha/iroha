//! Direct receiver staging and recovery contracts for KAGEMUSHA V1.
//!
//! These codecs expose only the three-message Request -> Payment ->
//! Acknowledgement flow. A qualified provider must atomically retain the exact
//! request/payment bytes and the rollback-resistant inbox receipt before it
//! reports staging success. Shape validation here never grants monetary
//! authority and the stock provider remains unavailable.

use iroha_data_model::kagemusha::{
    KAGEMUSHA_INBOX_STAGING_METADATA_MAX_BYTES_V1, KAGEMUSHA_PAYMENT_MAX_BYTES_V1,
    KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1, KAGEMUSHA_WIRE_VERSION_V1, KagemushaInboxReceiptV1,
    KagemushaPaymentRequestV1, KagemushaPaymentV1,
};
use norito::{
    DecodeLimits, NoritoDeserialize, NoritoSerialize,
    codec::{Decode, Encode},
};

const VERSION: u16 = KAGEMUSHA_WIRE_VERSION_V1;
const STAGE: u8 = 2;
const RECOVER_STAGED: u8 = 3;
const PAGE: u8 = 4;
const COMMAND_MAX: usize = 16 * 1024;
const REPLY_MAX: usize = 64 * 1024;
const PAGE_COUNT_MAX: u16 = 4;

/// Qualified state required after public body validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MissingAuthorityV1 {
    /// Authenticated rollback-resistant inbox and duplicate/conflict index.
    QualifiedReceiverJournal,
}

/// Closed receiver contract failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ReceiverErrorV1 {
    /// The operation is not in this receiver inventory.
    UnsupportedOperation,
    /// Empty, oversized, or resource-amplifying input.
    Size,
    /// Malformed or non-canonical Norito.
    CanonicalEncoding,
    /// An outer selector or direct exchange binding mismatched.
    Binding,
    /// A request, payment, or receipt failed public validation.
    PublicShape,
    /// A qualified provider is not installed.
    Unavailable(MissingAuthorityV1),
}
type Result<T> = std::result::Result<T, ReceiverErrorV1>;

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.stage-inbound-payment-command")]
struct StagePayloadV1 {
    version: u16,
    operation: u8,
    canonical_request: Vec<u8>,
    canonical_payment: Vec<u8>,
    staging_metadata: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.recover-staged-inbound-payment-command")]
struct RecoverStagedPayloadV1 {
    version: u16,
    operation: u8,
    credit_id: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.recover-inbound-inbox-page-command")]
struct PagePayloadV1 {
    version: u16,
    operation: u8,
    snapshot_revision: Option<u128>,
    after_credit_id: Option<[u8; 32]>,
    maximum_entries: u16,
}

/// Shape-checked direct receiver command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum ReceiverCommandV1 {
    /// Irreversibly stage one request-bound payment and exact transport bytes.
    Stage {
        request: KagemushaPaymentRequestV1,
        payment: KagemushaPaymentV1,
        canonical_request: Vec<u8>,
        canonical_payment: Vec<u8>,
        staging_metadata: Vec<u8>,
    },
    /// Recover the byte-identical staged record selected by credit ID.
    RecoverStaged { credit_id: [u8; 32] },
    /// Recover a bounded page at one stable inbox revision.
    Page {
        snapshot_revision: Option<u128>,
        after_credit_id: Option<[u8; 32]>,
        maximum_entries: u16,
    },
}

impl ReceiverCommandV1 {
    fn operation(&self) -> u8 {
        match self {
            Self::Stage { .. } => STAGE,
            Self::RecoverStaged { .. } => RECOVER_STAGED,
            Self::Page { .. } => PAGE,
        }
    }
}

/// Durable public projection returned by staging and recovery.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.staged-inbound-payment-record")]
struct StagedRecordV1 {
    canonical_request: Vec<u8>,
    canonical_payment: Vec<u8>,
    staging_metadata: Vec<u8>,
    inbox_receipt: KagemushaInboxReceiptV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.staged-inbound-payment-reply")]
struct StagedReplyV1 {
    version: u16,
    operation: u8,
    inbox_revision: u128,
    record: StagedRecordV1,
}

#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode)]
#[norito(schema_name = "iroha.kagemusha.device.v1.inbound-inbox-page-reply")]
struct PageReplyV1 {
    version: u16,
    operation: u8,
    snapshot_revision: u128,
    records: Vec<StagedRecordV1>,
    next_cursor: Option<[u8; 32]>,
}

fn bound(bytes: &[u8], maximum: usize) -> Result<()> {
    if bytes.is_empty() || bytes.len() > maximum {
        Err(ReceiverErrorV1::Size)
    } else {
        Ok(())
    }
}

fn nonzero(value: &[u8; 32]) -> Result<()> {
    if value == &[0; 32] {
        Err(ReceiverErrorV1::Binding)
    } else {
        Ok(())
    }
}

fn header(version: u16, operation: u8, expected: u8) -> Result<()> {
    if version == VERSION && operation == expected {
        Ok(())
    } else {
        Err(ReceiverErrorV1::Binding)
    }
}

fn exact<T>(bytes: &[u8], maximum: usize) -> Result<T>
where
    T: NoritoSerialize,
    for<'de> T: NoritoDeserialize<'de>,
{
    bound(bytes, maximum)?;
    norito::decode_canonical_with_limits(
        bytes,
        DecodeLimits::new(maximum, maximum, maximum * 4, maximum * 8, 32),
    )
    .map_err(|_| ReceiverErrorV1::CanonicalEncoding)
}

fn encode<T: NoritoSerialize>(value: &T, maximum: usize) -> Result<Vec<u8>> {
    let bytes = norito::encode_canonical(value).map_err(|_| ReceiverErrorV1::CanonicalEncoding)?;
    bound(&bytes, maximum)?;
    Ok(bytes)
}

fn decode_exchange(
    request_bytes: &[u8],
    payment_bytes: &[u8],
) -> Result<(KagemushaPaymentRequestV1, KagemushaPaymentV1)> {
    bound(request_bytes, KAGEMUSHA_PAYMENT_REQUEST_MAX_BYTES_V1)?;
    bound(payment_bytes, KAGEMUSHA_PAYMENT_MAX_BYTES_V1)?;
    let request = KagemushaPaymentRequestV1::decode_canonical_exact(request_bytes)
        .map_err(|_| ReceiverErrorV1::PublicShape)?;
    let payment = KagemushaPaymentV1::decode_canonical_shape_exact_against(payment_bytes, &request)
        .map_err(|_| ReceiverErrorV1::PublicShape)?;
    Ok((request, payment))
}

fn validate_record(record: &StagedRecordV1) -> Result<[u8; 32]> {
    if record.staging_metadata.len()
        > usize::try_from(KAGEMUSHA_INBOX_STAGING_METADATA_MAX_BYTES_V1)
            .map_err(|_| ReceiverErrorV1::Size)?
    {
        return Err(ReceiverErrorV1::Size);
    }
    let (_, payment) = decode_exchange(&record.canonical_request, &record.canonical_payment)?;
    if record.inbox_receipt.version != VERSION
        || record.inbox_receipt.credit_id != payment.output.credit_id
        || record.inbox_receipt.receipt_commitment == [0; 32]
    {
        return Err(ReceiverErrorV1::Binding);
    }
    Ok(payment.output.credit_id)
}

/// Decode a direct receiver command and bind it to the outer request ID.
pub(super) fn decode_receiver_command_v1(
    operation: u8,
    request_id: [u8; 32],
    bytes: &[u8],
) -> Result<ReceiverCommandV1> {
    nonzero(&request_id)?;
    match operation {
        STAGE => {
            let value: StagePayloadV1 = exact(bytes, COMMAND_MAX)?;
            header(value.version, value.operation, operation)?;
            if value.staging_metadata.len()
                > usize::try_from(KAGEMUSHA_INBOX_STAGING_METADATA_MAX_BYTES_V1)
                    .map_err(|_| ReceiverErrorV1::Size)?
            {
                return Err(ReceiverErrorV1::Size);
            }
            let (request, payment) =
                decode_exchange(&value.canonical_request, &value.canonical_payment)?;
            if payment.output.credit_id != request_id {
                return Err(ReceiverErrorV1::Binding);
            }
            Ok(ReceiverCommandV1::Stage {
                request,
                payment,
                canonical_request: value.canonical_request,
                canonical_payment: value.canonical_payment,
                staging_metadata: value.staging_metadata,
            })
        }
        RECOVER_STAGED => {
            let value: RecoverStagedPayloadV1 = exact(bytes, COMMAND_MAX)?;
            header(value.version, value.operation, operation)?;
            nonzero(&value.credit_id)?;
            if value.credit_id != request_id {
                return Err(ReceiverErrorV1::Binding);
            }
            Ok(ReceiverCommandV1::RecoverStaged {
                credit_id: value.credit_id,
            })
        }
        PAGE => {
            let value: PagePayloadV1 = exact(bytes, COMMAND_MAX)?;
            header(value.version, value.operation, operation)?;
            if !(1..=PAGE_COUNT_MAX).contains(&value.maximum_entries) {
                return Err(ReceiverErrorV1::Binding);
            }
            if let Some(after) = value.after_credit_id {
                nonzero(&after)?;
                if value.snapshot_revision.is_none() {
                    return Err(ReceiverErrorV1::Binding);
                }
            }
            Ok(ReceiverCommandV1::Page {
                snapshot_revision: value.snapshot_revision,
                after_credit_id: value.after_credit_id,
                maximum_entries: value.maximum_entries,
            })
        }
        _ => Err(ReceiverErrorV1::UnsupportedOperation),
    }
}

fn validate_receiver_reply_v1(command: &ReceiverCommandV1, bytes: &[u8]) -> Result<()> {
    match command {
        ReceiverCommandV1::Stage {
            canonical_request,
            canonical_payment,
            staging_metadata,
            payment,
            ..
        } => {
            let reply: StagedReplyV1 = exact(bytes, REPLY_MAX)?;
            header(reply.version, reply.operation, command.operation())?;
            if reply.inbox_revision == 0
                || reply.record.canonical_request != *canonical_request
                || reply.record.canonical_payment != *canonical_payment
                || reply.record.staging_metadata != *staging_metadata
                || validate_record(&reply.record)? != payment.output.credit_id
            {
                return Err(ReceiverErrorV1::Binding);
            }
            Ok(())
        }
        ReceiverCommandV1::RecoverStaged { credit_id } => {
            let reply: StagedReplyV1 = exact(bytes, REPLY_MAX)?;
            header(reply.version, reply.operation, command.operation())?;
            if reply.inbox_revision == 0 || validate_record(&reply.record)? != *credit_id {
                return Err(ReceiverErrorV1::Binding);
            }
            Ok(())
        }
        ReceiverCommandV1::Page {
            snapshot_revision,
            after_credit_id,
            maximum_entries,
        } => {
            let reply: PageReplyV1 = exact(bytes, REPLY_MAX)?;
            header(reply.version, reply.operation, command.operation())?;
            if snapshot_revision.is_some_and(|value| value != reply.snapshot_revision)
                || reply.records.len() > usize::from(*maximum_entries)
            {
                return Err(ReceiverErrorV1::Binding);
            }
            let mut previous = *after_credit_id;
            for record in &reply.records {
                let credit_id = validate_record(record)?;
                if previous.is_some_and(|value| value >= credit_id) {
                    return Err(ReceiverErrorV1::Binding);
                }
                previous = Some(credit_id);
            }
            if let Some(cursor) = reply.next_cursor {
                if reply.records.len() != usize::from(*maximum_entries) || previous != Some(cursor)
                {
                    return Err(ReceiverErrorV1::Binding);
                }
            }
            Ok(())
        }
    }
}

mod engine_seal {
    pub(super) trait Sealed {}
}

trait ReceiverEngineV1: engine_seal::Sealed {
    fn execute(&mut self, command: &ReceiverCommandV1) -> Result<Vec<u8>>;
}

struct UnavailableReceiverEngineV1;
impl engine_seal::Sealed for UnavailableReceiverEngineV1 {}
impl ReceiverEngineV1 for UnavailableReceiverEngineV1 {
    fn execute(&mut self, _: &ReceiverCommandV1) -> Result<Vec<u8>> {
        Err(ReceiverErrorV1::Unavailable(
            MissingAuthorityV1::QualifiedReceiverJournal,
        ))
    }
}

fn dispatch<E: ReceiverEngineV1>(
    engine: &mut E,
    request_id: [u8; 32],
    operation: u8,
    bytes: &[u8],
) -> Result<Vec<u8>> {
    let command = decode_receiver_command_v1(operation, request_id, bytes)?;
    let response = engine.execute(&command)?;
    validate_receiver_reply_v1(&command, &response)?;
    Ok(response)
}

/// Strict stock entry point: decode the complete command, then fail unavailable.
pub(super) fn dispatch_unavailable_receiver_v1(
    request_id: [u8; 32],
    operation: u8,
    bytes: &[u8],
) -> Result<Vec<u8>> {
    dispatch(
        &mut UnavailableReceiverEngineV1,
        request_id,
        operation,
        bytes,
    )
}

/// Construct simple recovery command bodies for outer-frame tests.
#[cfg(test)]
pub(super) fn canonical_command_body_for_tests(operation: u8) -> Option<Vec<u8>> {
    match operation {
        RECOVER_STAGED => encode(
            &RecoverStagedPayloadV1 {
                version: VERSION,
                operation,
                credit_id: [7; 32],
            },
            COMMAND_MAX,
        )
        .ok(),
        PAGE => encode(
            &PagePayloadV1 {
                version: VERSION,
                operation,
                snapshot_revision: None,
                after_credit_id: None,
                maximum_entries: PAGE_COUNT_MAX,
            },
            COMMAND_MAX,
        )
        .ok(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_operation_codes_are_rejected() {
        let bytes = norito::encode_canonical(&RecoverStagedPayloadV1 {
            version: VERSION,
            operation: RECOVER_STAGED,
            credit_id: [7; 32],
        })
        .unwrap();
        assert_eq!(
            decode_receiver_command_v1(0, [7; 32], &bytes),
            Err(ReceiverErrorV1::UnsupportedOperation),
        );
    }

    #[test]
    fn recovery_selector_is_bound_to_outer_credit_id() {
        let bytes = canonical_command_body_for_tests(RECOVER_STAGED).unwrap();
        assert!(decode_receiver_command_v1(RECOVER_STAGED, [7; 32], &bytes).is_ok());
        assert_eq!(
            decode_receiver_command_v1(RECOVER_STAGED, [8; 32], &bytes),
            Err(ReceiverErrorV1::Binding),
        );
    }
}
