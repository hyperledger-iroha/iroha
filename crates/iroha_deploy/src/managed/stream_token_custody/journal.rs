//! Bounded immutable original intents, with UTC signing limits independent of recovery I/O.

use super::*;
use iroha_data_model::{asset::AssetDefinitionId, transaction::FeePaymentIntent};
use iroha_primitives::numeric::Quantity;
use norito::{Decode, Encode};
use sorafs_manifest::signer::{
    custody::{
        SignerCustodyAnchorV1, SignerCustodyEnrollmentContextV1,
        verify_signer_custody_enrollment_v1,
    },
    custody_control::SignerCustodyControlStateV1,
};
use std::collections::BTreeMap;

const MAX_ORIGINAL_BYTES: usize = MAX_CHECKPOINT_BYTES + 128 * 1024;

#[derive(Clone, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::stream_token_custody::Terms")]
pub(super) struct Terms {
    pub requested_deadline_unix_ms: u64,
    pub signing_deadline_unix_ms: u64,
    fee_payment: FeePaymentIntent,
    max_total_fees: BTreeMap<AssetDefinitionId, Quantity>,
}

impl Terms {
    pub fn new(deadline_unix_ms: u64, options: &BoundedTransactionOptions) -> Result<Self> {
        validate_options(options)?;
        require_deadline(options.deadline)?;
        let now = now_ms()?;
        if deadline_unix_ms <= now || deadline_unix_ms == u64::MAX {
            return Err(invalid(
                "original custody UTC authorization is expired or unbounded",
            ));
        }
        let remaining = options.deadline.saturating_duration_since(Instant::now());
        let io_end = now
            .checked_add(
                u64::try_from(remaining.as_millis())
                    .map_err(|_| invalid("custody deadline exceeds bound"))?,
            )
            .ok_or_else(|| invalid("custody deadline overflow"))?;
        if io_end <= now {
            return Err(invalid("custody signing budget elapsed"));
        }
        Ok(Self {
            requested_deadline_unix_ms: deadline_unix_ms,
            signing_deadline_unix_ms: deadline_unix_ms.min(io_end),
            fee_payment: options.fee_payment.clone(),
            max_total_fees: options.max_total_fees.clone(),
        })
    }
    pub fn options(&self, deadline: Instant) -> BoundedTransactionOptions {
        BoundedTransactionOptions {
            fee_payment: self.fee_payment.clone(),
            max_total_fees: self.max_total_fees.clone(),
            deadline,
        }
    }
    pub fn signing_deadline(&self, deadline: Instant) -> Result<Instant> {
        require_deadline(deadline)?;
        let remaining = self
            .signing_deadline_unix_ms
            .checked_sub(now_ms()?)
            .filter(|value| *value > 0)
            .ok_or_else(|| invalid("original custody signing interval expired"))?;
        Ok(deadline.min(
            Instant::now()
                .checked_add(Duration::from_millis(remaining))
                .ok_or_else(|| invalid("custody monotonic deadline overflow"))?,
        ))
    }
    fn matches(&self, deadline_unix_ms: u64, options: &BoundedTransactionOptions) -> Result<()> {
        validate_options(options)?;
        if self.requested_deadline_unix_ms != deadline_unix_ms
            || self.fee_payment != options.fee_payment
            || self.max_total_fees != options.max_total_fees
        {
            return Err(invalid("original custody UTC or fee authorization changed"));
        }
        Ok(())
    }
    fn validate(&self) -> Result<()> {
        if self.requested_deadline_unix_ms == 0
            || self.requested_deadline_unix_ms == u64::MAX
            || self.signing_deadline_unix_ms == 0
            || self.signing_deadline_unix_ms > self.requested_deadline_unix_ms
        {
            return Err(invalid("invalid original custody signing authorization"));
        }
        validate_fees(&self.fee_payment, &self.max_total_fees)
    }
}

fn validate_options(options: &BoundedTransactionOptions) -> Result<()> {
    validate_fees(&options.fee_payment, &options.max_total_fees)
}
fn validate_fees(
    fee_payment: &FeePaymentIntent,
    max_total_fees: &BTreeMap<AssetDefinitionId, Quantity>,
) -> Result<()> {
    if max_total_fees.len() > 16
        || fee_payment.charge_limits().len() > 16
        || !matches!(fee_payment, FeePaymentIntent::Authority(_))
        || max_total_fees.values().any(Quantity::is_zero)
    {
        return Err(invalid(
            "custody fees require bounded authority-paid maxima",
        ));
    }
    fee_payment
        .validate()
        .map_err(|_| invalid("invalid custody fee authorization"))
}

#[derive(Clone, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::stream_token_custody::Action")]
pub(super) enum Action {
    Configure(SignerCustodyPolicyV1),
    Enroll {
        anchor: SignerCustodyAnchorV1,
        observed_at_unix_ms: u64,
        interval: ManagedCustodyEnrollmentInterval,
        enrollment: Vec<u8>,
    },
}

#[derive(Clone, Encode, Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::stream_token_custody::Original")]
pub(super) struct Original {
    pub selection: StreamTokenCustodySelection,
    pub action: Action,
    pub terms: Terms,
    pub checkpoint: Vec<u8>,
}

pub(super) enum Request {
    Configure(StreamTokenCustodyConfigureRequest),
    Enroll(StreamTokenCustodyEnrollRequest),
}

impl Original {
    pub fn request(&self, deadline: Instant) -> Request {
        match &self.action {
            Action::Configure(policy) => Request::Configure(StreamTokenCustodyConfigureRequest {
                selection: self.selection.clone(),
                policy: policy.clone(),
                deadline_unix_ms: self.terms.signing_deadline_unix_ms,
                options: self.terms.options(deadline),
            }),
            Action::Enroll {
                anchor,
                observed_at_unix_ms,
                interval,
                enrollment,
            } => Request::Enroll(StreamTokenCustodyEnrollRequest {
                selection: self.selection.clone(),
                anchor: *anchor,
                anchor_observed_at_unix_ms: *observed_at_unix_ms,
                issued_at_unix_ms: interval.issued_at_unix_ms,
                expires_at_unix_ms: interval.expires_at_unix_ms,
                enrollment: enrollment.clone(),
                deadline_unix_ms: self.terms.signing_deadline_unix_ms,
                options: self.terms.options(deadline),
            }),
        }
    }
    pub fn validate(&self) -> Result<()> {
        self.terms.validate()?;
        if self.checkpoint.is_empty()
            || self.checkpoint.len() > MAX_CHECKPOINT_BYTES
            || norito::canonical_frame_len(&self.selection)
                .map_err(|_| invalid("invalid custody selection"))?
                > 64 * 1024
        {
            return Err(invalid("original custody intent exceeds bound"));
        }
        if let Action::Enroll {
            enrollment,
            interval,
            observed_at_unix_ms,
            anchor,
        } = &self.action
        {
            if enrollment.is_empty()
                || enrollment.len() > 16 * 1024
                || interval.deadline_unix_ms != self.terms.requested_deadline_unix_ms
            {
                return Err(invalid(
                    "original enrollment differs from immutable request",
                ));
            }
            validate_interval(*interval, *observed_at_unix_ms)?;
            let predecessor = self
                .selection
                .current
                .as_ref()
                .ok_or_else(|| invalid("original enrollment lacks its selected predecessor"))?;
            if predecessor.control_state.len() > 16 * 1024 {
                return Err(invalid("original custody control exceeds its bound"));
            }
            let control: SignerCustodyControlStateV1 = norito::decode_canonical_with_limits(
                &predecessor.control_state,
                norito::DecodeLimits::new(4096, 16 * 1024, 16 * 1024, 1024 * 1024, 32),
            )
            .map_err(|_| invalid("invalid original custody control"))?;
            let verified = verify_signer_custody_enrollment_v1(
                enrollment,
                &self.selection.binding,
                &control.policy.custody_trust(),
                &SignerCustodyEnrollmentContextV1 {
                    now_unix_ms: *observed_at_unix_ms,
                    anchor_observed_at_unix_ms: *observed_at_unix_ms,
                    current_anchor: *anchor,
                    next_sequence: control.next_sequence,
                    predecessor_digest: control.predecessor_digest,
                    signer_revoked: control.signer_revoked,
                    attester_revoked: control.attester_revoked,
                },
            )
            .map_err(|_| {
                invalid("enrollment exceeds its original governed custody authorization")
            })?;
            if verified.statement().issued_at_unix_ms != interval.issued_at_unix_ms
                || verified.statement().expires_at_unix_ms != interval.expires_at_unix_ms
            {
                return Err(invalid(
                    "enrollment statement differs from original requested interval",
                ));
            }
        } else if let Action::Configure(policy) = &self.action {
            encode(policy, 16 * 1024)?;
        }
        Ok(())
    }
    pub fn matches_configuration(
        &self,
        policy: &SignerCustodyPolicyV1,
        deadline: u64,
        options: &BoundedTransactionOptions,
    ) -> Result<()> {
        self.validate()?;
        if !matches!(&self.action, Action::Configure(saved) if saved == policy) {
            return Err(invalid("initial custody configuration cannot be replaced"));
        }
        self.terms.matches(deadline, options)
    }
    pub fn matches_enrollment(
        &self,
        interval: ManagedCustodyEnrollmentInterval,
        options: &BoundedTransactionOptions,
    ) -> Result<()> {
        self.validate()?;
        if !matches!(&self.action, Action::Enroll { interval: saved, .. } if *saved == interval) {
            return Err(invalid("initial custody enrollment cannot be renewed"));
        }
        self.terms.matches(interval.deadline_unix_ms, options)
    }
}

pub(super) fn encode<T: norito::NoritoSerialize>(value: &T, maximum: usize) -> Result<Vec<u8>> {
    if norito::canonical_frame_len(value).map_err(|_| invalid("cannot size custody record"))?
        > maximum
    {
        return Err(invalid("custody record exceeds byte bound"));
    }
    norito::encode_canonical(value).map_err(|_| invalid("cannot encode custody record"))
}
pub(super) fn read_optional(
    directory: &PrivateDirectory,
    name: &str,
    maximum: usize,
) -> Result<Option<Vec<u8>>> {
    match directory.read(name, maximum) {
        Ok(bytes) => Ok(Some(bytes.to_vec())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            directory.revalidate()?;
            Ok(None)
        }
        Err(error) => Err(error.into()),
    }
}
pub(super) fn read_original(directory: &PrivateDirectory) -> Result<Option<Original>> {
    let Some(bytes) = read_optional(directory, "original.nrt", MAX_ORIGINAL_BYTES)? else {
        return Ok(None);
    };
    let original: Original = norito::decode_canonical_with_limits(
        &bytes,
        norito::DecodeLimits::new(
            4096,
            MAX_ORIGINAL_BYTES,
            MAX_ORIGINAL_BYTES,
            96 * 1024 * 1024,
            40,
        ),
    )
    .map_err(|_| invalid("invalid bounded original custody intent"))?;
    original.validate()?;
    Ok(Some(original))
}
pub(super) fn required_original(directory: &PrivateDirectory) -> Result<Original> {
    read_original(directory)?.ok_or_else(|| invalid("original custody request is absent"))
}
pub(super) fn publish_original(directory: &PrivateDirectory, original: &Original) -> Result<()> {
    original.validate()?;
    directory.write_atomic(
        "original.nrt",
        &encode(original, MAX_ORIGINAL_BYTES)?,
        PublishMode::CreateNew,
    )?;
    Ok(())
}
pub(super) fn require_empty(directory: &PrivateDirectory) -> Result<()> {
    if !directory.entries(0)?.is_empty() {
        return Err(invalid("custody journal has no original request"));
    }
    Ok(())
}
