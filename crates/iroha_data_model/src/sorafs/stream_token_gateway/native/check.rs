//! Bounded challenged gateway readback claims, distinct from verified serving authority.

use crate::{
    DeriveJsonDeserialize, DeriveJsonSerialize,
    account::AccountId,
    block::consensus::HeightContextId,
    sorafs::{
        reputation::StreamTokenValidationStatusV1,
        stream_token_gateway::{
            STREAM_TOKEN_GATEWAY_RECONCILE_MAX_ITEMS_V1,
            StreamTokenGatewayAdmissionDeliveryStateV1 as Delivery,
            StreamTokenGatewayAdmissionErrorV1 as Error,
            StreamTokenGatewayAdmissionQualificationV1 as Qualification,
            StreamTokenGatewayAdmissionReadbackV1 as Readback,
            StreamTokenGatewayAdmissionRecordV1 as Record,
            StreamTokenGatewayAdmissionResultV1 as AdmissionResult,
        },
    },
};
use iroha_crypto::Hash;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Maximum complete canonical pending commitment frame, including qualification and limit.
///
/// The 1,024-item ceiling independently bounds work before frame sizing. This aggregate ceiling
/// prevents oversized readback allocation; the signed Check carries only its 32-byte commitment.
pub const STREAM_TOKEN_GATEWAY_MAX_PENDING_READBACK_BYTES_V1: usize = 1024 * 1024;
const PENDING_DOMAIN: &[u8] = b"iroha.sorafs.stream-token.gateway-pending-readback.v1\0";

/// Independently captured finalized block and exact signed-RS16 height context.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_gateway::native::StreamTokenGatewayFinalityFloorV1"
)]
#[norito(deny_unknown_fields)]
pub struct StreamTokenGatewayFinalityFloorV1 {
    /// Positive finalized height, strictly before the challenged Check executes.
    pub height: u64,
    /// Exact block hash at that height.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub block_hash: [u8; 32],
    /// Exact committed header/result context at that height.
    pub context_id: HeightContextId,
}
impl StreamTokenGatewayFinalityFloorV1 {
    /// Validate non-inert coordinates; only the native reader can authenticate their lineage.
    ///
    /// # Errors
    /// Rejects a zero height, block hash or height-context commitment, including the marked
    /// representation of an empty prehash.
    pub fn validate(self) -> Result<(), Error> {
        if self.height == 0
            || self.block_hash == [0; 32]
            || *self.context_id.0.as_ref() == [0; 32]
            || Hash::from(self.context_id.0) == Hash::prehashed([0; 32])
        {
            return Err(Error::BindingMismatch);
        }
        Ok(())
    }
}

/// Closed assertions for one exact native gateway Check.
///
/// Admission authenticates historical recovery material. Only Serving asks the native owner
/// to establish live eligibility for the independently retained physical HTTP serving attempt.
/// No decoded subject or successful shape check establishes either fact.
// Bounded inline gateway claims retain their canonical V1 layout.
#[derive(
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_gateway::native::StreamTokenGatewayCheckSubjectV1"
)]
#[norito(
    tag = "subject",
    content = "value",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum StreamTokenGatewayCheckSubjectV1 {
    /// Authenticate the exact current policy binding and current operator/observer authority.
    /// Disabled or expired admission intervals do not prevent recovery qualification.
    #[codec(index = 0)]
    Qualification,
    /// Authenticate the immutable original admission and its current delivery state.
    /// An accepted historical record does not imply a live lease or permission to serve.
    #[codec(index = 1)]
    Admission {
        /// Commitment to the complete independently retained original request and serving attempt.
        #[norito(json = "crate::json_helpers::fixed_bytes")]
        request_digest: [u8; 32],
        /// Exact claimed retained result; Core compares its original request and policy history.
        result: AdmissionResult,
    },
    /// Establish final live eligibility after the exact callback record was acknowledged.
    #[codec(index = 2)]
    Serving {
        /// Same complete original request commitment; never derived from a replacement worker.
        #[norito(json = "crate::json_helpers::fixed_bytes")]
        request_digest: [u8; 32],
        /// Exact accepted, acknowledged result whose original lease must still be live.
        result: AdmissionResult,
    },
    /// Authenticate the complete oldest pending prefix, including its exact empty case.
    #[codec(index = 3)]
    Pending {
        /// Exact requested prefix bound, from one through 1,024 records.
        max_items: u32,
        /// Commitment from [`stream_token_gateway_pending_readback_digest_v1`].
        #[norito(json = "crate::json_helpers::fixed_bytes")]
        readback_digest: [u8; 32],
    },
    /// Confirm that the contiguous acknowledgement head covers this exact retained record.
    #[codec(index = 4)]
    Acknowledged {
        /// Complete original record, including its historical policy and serving attempt.
        record: Record,
    },
    /// Confirm an authenticated release or expiry terminal for this original accepted lease.
    #[codec(index = 5)]
    Released {
        /// Complete original accepted record; a new lease or serving attempt cannot substitute.
        record: Record,
    },
}

/// Exact fresh observer challenge and its independently bound gateway assertion.
///
/// The surrounding request supplies the network, gateway and current policy CAS. Core verifies
/// registered current permissions and policy membership; the purpose-owned observation consumer
/// verifies signed successful execution, historical bindings, finality and the original deadline.
#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Encode,
    Decode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
    norito::NoritoSchema,
)]
#[norito_schema(
    name = "iroha_data_model::sorafs::stream_token_gateway::native::StreamTokenGatewayCheckV1"
)]
#[norito(deny_unknown_fields)]
pub struct StreamTokenGatewayCheckV1 {
    /// Nonzero OS-random challenge issued once within the original observation deadline.
    #[norito(json = "crate::json_helpers::fixed_bytes")]
    pub challenge: [u8; 32],
    /// Independently configured current gateway operator; may differ from historical operators.
    pub expected_operator: AccountId,
    /// Independent current policy observer that signs the exact sole Check instruction.
    pub expected_observer: AccountId,
    /// Independently captured finalized context preceding this Check.
    pub floor: StreamTokenGatewayFinalityFloorV1,
    /// One exact bounded assertion; variants cannot substitute for each other's authority.
    pub subject: StreamTokenGatewayCheckSubjectV1,
}

fn record_binding(
    record: &Record,
    gateway: [u8; 32],
    revision: u64,
    digest: [u8; 32],
) -> Result<(), Error> {
    record.validate_shape(record.admitted_under)?;
    if record.admitted_under.gateway_id != gateway
        || record.admitted_under.revision > revision
        || (record.admitted_under.revision == revision
            && record.admitted_under.policy_digest != digest)
    {
        return Err(Error::BindingMismatch);
    }
    Ok(())
}

fn result_binding(
    result: &AdmissionResult,
    gateway: [u8; 32],
    revision: u64,
    digest: [u8; 32],
) -> Result<(), Error> {
    record_binding(&result.record, gateway, revision, digest)?;
    let sequence = result.record.outcome.binding.gateway_sequence;
    match result.delivery_state {
        Delivery::Pending {
            predecessor_sequence,
        } if predecessor_sequence.checked_add(1) == Some(sequence) => Ok(()),
        Delivery::AcknowledgedExactReplay {
            acknowledged_through_sequence,
        } if acknowledged_through_sequence >= sequence => Ok(()),
        _ => Err(Error::BindingMismatch),
    }
}

impl StreamTokenGatewayCheckV1 {
    /// Validate bounded subject relations to the surrounding current-policy request.
    pub(super) fn validate_binding(
        &self,
        gateway: [u8; 32],
        revision: u64,
        digest: [u8; 32],
    ) -> Result<(), Error> {
        use StreamTokenGatewayCheckSubjectV1 as Subject;

        self.floor.validate()?;
        let operators = std::collections::BTreeSet::from([self.expected_operator.clone()]);
        let observers = std::collections::BTreeSet::from([self.expected_observer.clone()]);
        if self.challenge == [0; 32]
            || self.expected_operator == self.expected_observer
            || !super::controller_keys(&operators).is_disjoint(&super::controller_keys(&observers))
        {
            return Err(Error::BindingMismatch);
        }
        match self.subject {
            Subject::Qualification => Ok(()),
            Subject::Admission {
                request_digest,
                result,
            }
            | Subject::Serving {
                request_digest,
                result,
            } => {
                if request_digest == [0; 32]
                    || (matches!(self.subject, Subject::Serving { .. })
                        && (result.record.outcome.status
                            != StreamTokenValidationStatusV1::Accepted
                            || !matches!(
                                result.delivery_state,
                                Delivery::AcknowledgedExactReplay { .. }
                            )))
                {
                    return Err(Error::BindingMismatch);
                }
                result_binding(&result, gateway, revision, digest)
            }
            Subject::Pending {
                max_items,
                readback_digest,
            } => {
                if !(1..=STREAM_TOKEN_GATEWAY_RECONCILE_MAX_ITEMS_V1).contains(&max_items)
                    || readback_digest == [0; 32]
                {
                    return Err(Error::InvalidRequest);
                }
                Ok(())
            }
            Subject::Acknowledged { record } | Subject::Released { record } => {
                if matches!(self.subject, Subject::Released { .. })
                    && record.outcome.status != StreamTokenValidationStatusV1::Accepted
                {
                    return Err(Error::BindingMismatch);
                }
                record_binding(&record, gateway, revision, digest)
            }
        }
    }
}

/// Commit the exact complete requested pending prefix, including empty readback and policy.
///
/// The preimage is the fixed purpose domain followed by the complete canonical Norito frame of
/// `(qualification, max_items, readback)`. No decoded commitment authenticates source rows; Core
/// reconstructs this value from its exact World prefix and the observer verifies successful Check
/// execution. Item and aggregate bounds apply before allocating the canonical frame.
///
/// # Errors
/// Rejects invalid qualification, an oversized, omitted, reordered or substituted prefix, and an
/// unencodable or oversized complete commitment frame.
pub fn stream_token_gateway_pending_readback_digest_v1(
    qualification: Qualification,
    max_items: u32,
    readback: &Readback,
) -> Result<[u8; 32], Error> {
    if !(1..=STREAM_TOKEN_GATEWAY_RECONCILE_MAX_ITEMS_V1).contains(&max_items)
        || readback.records.len() > max_items as usize
    {
        return Err(Error::InvalidRequest);
    }
    // Bound the borrowed readback before cloning its fixed-layout records into the exact
    // owned tuple. References have a distinct Norito identity and are not this commitment.
    if norito::canonical_frame_len(readback).map_err(|_| Error::InvalidRequest)?
        > STREAM_TOKEN_GATEWAY_MAX_PENDING_READBACK_BYTES_V1
    {
        return Err(Error::InvalidRequest);
    }
    readback.validate(max_items, qualification)?;
    let material = (qualification, max_items, readback.clone());
    if norito::canonical_frame_len(&material).map_err(|_| Error::InvalidRequest)?
        > STREAM_TOKEN_GATEWAY_MAX_PENDING_READBACK_BYTES_V1
    {
        return Err(Error::InvalidRequest);
    }
    let frame = norito::encode_canonical(&material).map_err(|_| Error::InvalidRequest)?;
    let mut bytes = Vec::with_capacity(PENDING_DOMAIN.len() + frame.len());
    bytes.extend_from_slice(PENDING_DOMAIN);
    bytes.extend_from_slice(&frame);
    Ok(*Hash::new(bytes).as_ref())
}
