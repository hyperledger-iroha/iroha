//! One bounded epoch, replacement-claim and live-clock owner for closed generated dispatches.
//!
//! Retained records bind custody only. Only a newly issued non-decodable Lease carries a live
//! deadline and cancellation; reading these records never renews transaction authorization.
use super::{
    Fees, Terms,
    attempts::{self, BodyReplacementTarget, Origin, Purpose},
    invalid, now_ms, require_deadline,
};
use crate::managed::{ManagedBootstrapFailure, Result};
use iroha_crypto::Hash;
use iroha_data_model::sorafs::capacity::ProviderId;
use iroha_fs::PrivateDirectory;
use iroha_wallet::operations::AccountService;
use std::{
    ffi::OsString,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

pub(in crate::managed) const MAX_EPOCHS: usize = 64;

#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::native_operation::authorization::Epoch")]
pub(in crate::managed) struct Epoch {
    pub(in crate::managed) ordinal: u8,
    pub(in crate::managed) previous: Option<[u8; 32]>,
    pub(in crate::managed) parent_intent: [u8; 32],
    issued_at_unix_ms: u64,
    pub(in crate::managed) terms: Terms,
}
#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::native_operation::authorization::Replacement")]
struct Replacement {
    epoch: [u8; 32],
    purpose: Purpose,
    target: ReplacementTarget,
}
#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::native_operation::authorization::ReplacementTarget")]
enum ReplacementTarget {
    Dispatch {
        previous_attempt: [u8; 32],
    },
    EnrollmentBody {
        outer_intent: [u8; 32],
        previous_body: [u8; 32],
        successor_selection: [u8; 32],
    },
}
impl ReplacementTarget {
    fn validate(&self, purpose: Purpose) -> Result<()> {
        let valid = match self {
            Self::Dispatch { previous_attempt } => *previous_attempt != [0; 32],
            Self::EnrollmentBody {
                outer_intent,
                previous_body,
                successor_selection,
            } => {
                matches!(
                    purpose,
                    Purpose::CustodyEnroll(_) | Purpose::CustodyRenewal { .. }
                ) && *outer_intent != [0; 32]
                    && *previous_body != [0; 32]
                    && *successor_selection != [0; 32]
                    && previous_body != successor_selection
            }
        };
        if valid {
            Ok(())
        } else {
            Err(invalid(
                "unsigned replacement target differs from its closed purpose",
            ))
        }
    }
}

/// Closed local scope. It cannot supply a native policy, current state or successful carrier.
#[derive(Clone, Copy)]
pub(in crate::managed) enum Scope {
    Bootstrap([ProviderId; 3]),
    Renewal { provider: ProviderId, sequence: u64 },
}
impl Scope {
    pub(in crate::managed) fn check(self, purpose: Purpose) -> Result<()> {
        let allowed = match (self, purpose) {
            (
                Self::Renewal { provider, sequence },
                Purpose::CustodyRenewal {
                    provider: selected,
                    sequence: next,
                },
            ) => provider == selected && sequence == next && (2..=64).contains(&sequence),
            (Self::Bootstrap(_), Purpose::CustodyRenewal { .. }) => false,
            (Self::Bootstrap(_), Purpose::ReservePolicy | Purpose::Reputation) => true,
            (
                Self::Bootstrap(providers),
                Purpose::CustodyConfigure(p)
                | Purpose::CustodyEnroll(p)
                | Purpose::ReserveAccount(p)
                | Purpose::FundingRequest(p)
                | Purpose::FundingApproval(p)
                | Purpose::FundingCredit(p)
                | Purpose::FundingCapacity(p)
                | Purpose::ProviderIngest(p)
                | Purpose::Gateway(p),
            ) => providers.contains(&p),
            _ => false,
        };
        if allowed {
            Ok(())
        } else {
            Err(invalid(
                "generated authorization changed its closed purpose",
            ))
        }
    }
}

/// Held original anchor and newly issued live clocks; no Clone or decode implementation.
pub(in crate::managed) struct Lease {
    pub(in crate::managed) directory: PrivateDirectory,
    original: Vec<u8>,
    original_digest: [u8; 32],
    scope: Scope,
    pub(in crate::managed) epoch: Epoch,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}
impl Lease {
    pub(in crate::managed) fn issue(
        directory: PrivateDirectory,
        original: Vec<u8>,
        fees: &Fees,
        scope: Scope,
        expires: u64,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Self> {
        require_active(&cancelled)?;
        require_deadline(deadline)?;
        fees.validate()?;
        if original.is_empty() || original.len() > super::MAX_CHECKPOINT_BYTES + 128 * 1024 {
            return Err(invalid(
                "generated authorization original exceeds its byte bound",
            ));
        }
        let original_digest = *Hash::new(&original).as_ref();
        if directory.read("original.nrt", original.len())?.as_slice() != original.as_slice() {
            return Err(invalid("generated authorization original changed"));
        }
        let issued_at_unix_ms = now_ms()?;
        let remaining = u64::try_from(
            deadline
                .saturating_duration_since(Instant::now())
                .as_millis(),
        )
        .map_err(|_| invalid("generated authorization interval exceeds bound"))?;
        let requested = issued_at_unix_ms
            .checked_add(remaining)
            .ok_or_else(|| invalid("generated authorization UTC overflow"))?
            .min(expires.saturating_sub(1));
        if requested <= issued_at_unix_ms {
            return Err(ManagedBootstrapFailure::ProfileExpired.into());
        }
        let terms = Terms::new(requested, &fees.options(deadline))?;
        require_active(&cancelled)?;
        let root = directory.ensure_child("epochs")?;
        let retained = read_epochs(&root, original_digest, fees, scope)?;
        if retained.len() >= MAX_EPOCHS {
            return Err(ManagedBootstrapFailure::EpochLimit.into());
        }
        let epoch = Epoch {
            ordinal: u8::try_from(retained.len() + 1)
                .map_err(|_| invalid("generated epoch ordinal overflow"))?,
            previous: retained.last().map(digest).transpose()?,
            parent_intent: original_digest,
            issued_at_unix_ms,
            terms,
        };
        require_active(&cancelled)?;
        attempts::write_record(&root, &format!("{:04}.nrt", epoch.ordinal), &epoch)?;
        let after = read_epochs(&root, original_digest, fees, scope)?;
        if after.last() != Some(&epoch) || after.len() != retained.len() + 1 {
            return Err(invalid("new generated epoch changed during publication"));
        }
        Ok(Self {
            directory,
            original,
            original_digest,
            scope,
            epoch,
            deadline,
            cancelled,
        })
    }
    pub(in crate::managed) fn check(&self, deadline: Instant) -> Result<Instant> {
        require_active(&self.cancelled)?;
        let deadline = deadline.min(self.deadline);
        if deadline <= Instant::now() {
            return Err(ManagedBootstrapFailure::AuthorizationExpired.into());
        }
        self.directory.revalidate()?;
        if self
            .directory
            .read("original.nrt", self.original.len())?
            .as_slice()
            != self.original.as_slice()
        {
            return Err(invalid("generated original changed after authorization"));
        }
        if now_ms()? >= self.epoch.terms.signing_deadline_unix_ms {
            return Err(ManagedBootstrapFailure::AuthorizationExpired.into());
        }
        let retained = read_epochs(
            &self.directory.open_child("epochs")?,
            self.original_digest,
            &self.epoch.terms.fees,
            self.scope,
        )?;
        if retained.last() != Some(&self.epoch) {
            return Err(invalid(
                "generated capability no longer selects its retained epoch",
            ));
        }
        self.epoch
            .terms
            .signing_deadline(deadline)
            .map_err(|_| ManagedBootstrapFailure::AuthorizationExpired.into())
    }
    fn origin(&self) -> Result<Origin> {
        Ok(Origin::Generated {
            ordinal: self.epoch.ordinal,
            epoch: digest(&self.epoch)?,
            parent_intent: self.original_digest,
        })
    }
    fn claim(&self, purpose: Purpose, target: ReplacementTarget, deadline: Instant) -> Result<()> {
        self.check(deadline)?;
        self.scope.check(purpose)?;
        target.validate(purpose)?;
        let root = self.directory.open_child("epochs")?;
        let name = format!("{:04}-replacement.nrt", self.epoch.ordinal);
        let selected = Replacement {
            epoch: digest(&self.epoch)?,
            purpose,
            target,
        };
        if let Some(old) = attempts::read_record::<Replacement>(&root, &name)? {
            if old != selected {
                return Err(ManagedBootstrapFailure::ReplacementLimit.into());
            }
        } else {
            self.check(deadline)?;
            attempts::write_record(&root, &name, &selected)?;
        }
        self.check(deadline)?;
        Ok(())
    }
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for crate::managed::service_bootstrap::authorization::BootstrapChildAuthorization<'_> {}
    impl Sealed for crate::managed::stream_token_custody::renewal::GeneratedRenewalAuthorization {}
}
/// Only the two closed issuers above may feed the sole generated attempt transition.
pub(in crate::managed) trait DispatchAuthorization: sealed::Sealed {
    fn lease(&self) -> &Lease;
    fn purpose(&self) -> Purpose;
    fn check(&self, purpose: Purpose, deadline: Instant) -> Result<Instant> {
        if purpose != self.purpose() {
            return Err(invalid("generated dispatch changed its authorized purpose"));
        }
        self.lease().scope.check(purpose)?;
        self.lease().check(deadline)
    }
    fn fees(&self) -> &Fees {
        &self.lease().epoch.terms.fees
    }
    fn origin(&self) -> Result<Origin> {
        self.lease().origin()
    }
    fn bind_account(&self, account: AccountService) -> Result<AccountService> {
        account
            .with_cancellation(Arc::clone(&self.lease().cancelled))
            .map_err(|_| invalid("generated wallet cancellation binding changed"))
    }
    fn terms(&self, deadline: Instant, exclusive_ceiling: Option<u64>) -> Result<Terms> {
        self.check(self.purpose(), deadline)?;
        let mut terms = self.lease().epoch.terms.clone();
        if let Some(ceiling) = exclusive_ceiling {
            let end = ceiling
                .checked_sub(1)
                .ok_or(ManagedBootstrapFailure::EnrollmentExpired)?;
            terms.requested_deadline_unix_ms = terms.requested_deadline_unix_ms.min(end);
            terms.signing_deadline_unix_ms = terms.signing_deadline_unix_ms.min(end);
        }
        terms.validate()?;
        if now_ms()? >= terms.signing_deadline_unix_ms {
            return Err(ManagedBootstrapFailure::AuthorizationExpired.into());
        }
        Ok(terms)
    }
    fn claim_replacement(&self, previous_attempt: [u8; 32], deadline: Instant) -> Result<()> {
        self.check(self.purpose(), deadline)?;
        self.lease().claim(
            self.purpose(),
            ReplacementTarget::Dispatch { previous_attempt },
            deadline,
        )
    }
    fn claim_body_replacement(
        &self,
        target: &dyn BodyReplacementTarget,
        deadline: Instant,
    ) -> Result<()> {
        self.check(target.purpose(), deadline)?;
        target.validate_target()?;
        if target.fees() != self.fees() {
            return Err(invalid(
                "body replacement changed original fee authorization",
            ));
        }
        self.lease().claim(
            target.purpose(),
            ReplacementTarget::EnrollmentBody {
                outer_intent: target.outer_intent(),
                previous_body: target.predecessor_body(),
                successor_selection: target.successor_selection(),
            },
            deadline,
        )
    }
}

pub(in crate::managed) fn validate_retained(
    directory: &PrivateDirectory,
    original_digest: [u8; 32],
    fees: &Fees,
    scope: Scope,
) -> Result<()> {
    match directory.open_child("epochs") {
        Ok(root) => {
            read_epochs(&root, original_digest, fees, scope)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    directory.revalidate()?;
    Ok(())
}
pub(in crate::managed) fn validate_references<'a>(
    directory: &PrivateDirectory,
    original_digest: [u8; 32],
    fees: &Fees,
    scope: Scope,
    origins: impl Iterator<Item = &'a Origin>,
) -> Result<()> {
    let records = match directory.open_child("epochs") {
        Ok(root) => read_epochs(&root, original_digest, fees, scope)?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    for origin in origins {
        if let Origin::Generated {
            ordinal,
            epoch,
            parent_intent,
        } = origin
        {
            let selected = records
                .get(
                    usize::from(*ordinal)
                        .checked_sub(1)
                        .ok_or_else(|| invalid("generated epoch ordinal is zero"))?,
                )
                .ok_or_else(|| {
                    invalid("retained dispatch lost its original authorization epoch")
                })?;
            if *parent_intent != original_digest || *epoch != digest(selected)? {
                return Err(invalid(
                    "retained dispatch changed its original authorization epoch",
                ));
            }
        }
    }
    Ok(())
}

fn read_epochs(
    root: &PrivateDirectory,
    parent_intent: [u8; 32],
    fees: &Fees,
    scope: Scope,
) -> Result<Vec<Epoch>> {
    let names = root.entries(MAX_EPOCHS * 2)?;
    let mut expected = Vec::<OsString>::new();
    let mut epochs: Vec<Epoch> = Vec::new();
    for index in 1..=MAX_EPOCHS {
        let name = format!("{index:04}.nrt");
        let Some(epoch): Option<Epoch> = attempts::read_record(root, &name)? else {
            break;
        };
        epoch.terms.validate()?;
        if usize::from(epoch.ordinal) != index
            || epoch.parent_intent != parent_intent
            || epoch.terms.fees != *fees
            || epoch.issued_at_unix_ms == 0
            || epoch.issued_at_unix_ms >= epoch.terms.signing_deadline_unix_ms
            || epoch.previous != epochs.last().map(digest).transpose()?
        {
            return Err(invalid(
                "generated epoch changed original intent, fees or lineage",
            ));
        }
        expected.push(name.into());
        let claim_name = format!("{index:04}-replacement.nrt");
        if let Some(claim) = attempts::read_record::<Replacement>(root, &claim_name)? {
            scope.check(claim.purpose)?;
            claim.target.validate(claim.purpose)?;
            if claim.epoch != digest(&epoch)? {
                return Err(invalid("generated unsigned replacement claim changed"));
            }
            expected.push(claim_name.into());
        }
        epochs.push(epoch);
    }
    expected.sort();
    if names != expected || root.entries(MAX_EPOCHS * 2)? != names {
        return Err(invalid(
            "generated epoch inventory changed or contains gaps or unknown material",
        ));
    }
    Ok(epochs)
}
pub(in crate::managed) fn digest<T: norito::NoritoSerialize>(value: &T) -> Result<[u8; 32]> {
    attempts::semantic_digest(value, attempts::MAX_RECORD_BYTES)
}
pub(in crate::managed) fn require_active(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Acquire) {
        Err(ManagedBootstrapFailure::Cancelled.into())
    } else {
        Ok(())
    }
}
