//! One finite authorization minted by a newly started managed worker, never by recovery.
//!
//! Files bind original intent and unsigned replacement custody. Only the non-decodable live
//! value carries the original process-local clock; stored epochs cannot renew signing permission.

use super::Original;
use crate::managed::{
    ManagedBootstrapFailure, PreparedLocalnet, Result,
    native_operation::{
        Terms,
        attempts::{self, Origin, Purpose},
        encode, invalid, now_ms, require_deadline,
    },
    service_authority::ServiceAuthority,
    service_policies::GeneratedServicePolicies,
};
use iroha_crypto::Hash;
use iroha_data_model::sorafs::capacity::ProviderId;
use iroha_fs::PrivateDirectory;
use std::{
    ffi::OsString,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};

const MAX_EPOCHS: usize = 64;

#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::service_bootstrap::authorization::Epoch")]
struct Epoch {
    ordinal: u8,
    previous: Option<[u8; 32]>,
    parent_intent: [u8; 32],
    issued_at_unix_ms: u64,
    terms: Terms,
}
#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::service_bootstrap::authorization::Replacement")]
struct Replacement {
    epoch: [u8; 32],
    purpose: Purpose,
    previous_attempt: [u8; 32],
}

/// Exact retained epoch plus original live clock. No Clone, decoder or caller constructor.
pub(in crate::managed) struct GeneratedBootstrapAuthorization {
    prepared: PreparedLocalnet,
    original: Original,
    original_digest: [u8; 32],
    directory: PrivateDirectory,
    epoch: Epoch,
    deadline: Instant,
    cancelled: Arc<AtomicBool>,
}

/// A concrete owner must match its exact generated policy and native semantic selection as well
/// as this scope. This value is not a proof of current permission or finalized execution.
pub(in crate::managed) struct BootstrapChildAuthorization<'a> {
    parent: &'a GeneratedBootstrapAuthorization,
    purpose: Purpose,
}

pub(in crate::managed) struct FundingAuthorization<'a> {
    parent: &'a GeneratedBootstrapAuthorization,
    provider: ProviderId,
}

impl GeneratedBootstrapAuthorization {
    #[cfg(test)]
    pub(in crate::managed) fn test_child(
        &self,
        purpose: Purpose,
    ) -> Result<BootstrapChildAuthorization<'_>> {
        self.child(purpose)
    }
    #[cfg(test)]
    pub(in crate::managed) fn test_terms(&self) -> &Terms {
        &self.epoch.terms
    }
    #[cfg(test)]
    pub(in crate::managed) fn test_ordinal(&self) -> u8 {
        self.epoch.ordinal
    }

    pub(super) fn issue(
        authority: &ServiceAuthority,
        original: Original,
        deadline: Instant,
        cancelled: Arc<AtomicBool>,
    ) -> Result<Self> {
        require_active(&cancelled)?;
        require_deadline(deadline)?;
        authority.validate_profile()?;
        original.validate(authority)?;
        let original_digest = original.digest()?;
        let directory = authority.directory.open_child("initial")?;
        let epoch = {
            let expires = profile_expiry(authority, &original)?;
            let issued_at_unix_ms = now_ms()?;
            let remaining = u64::try_from(
                deadline
                    .saturating_duration_since(Instant::now())
                    .as_millis(),
            )
            .map_err(|_| invalid("bootstrap startup interval exceeds bound"))?;
            let requested = issued_at_unix_ms
                .checked_add(remaining)
                .ok_or_else(|| invalid("bootstrap authorization UTC overflow"))?
                .min(expires.saturating_sub(1));
            if requested <= issued_at_unix_ms {
                return Err(ManagedBootstrapFailure::ProfileExpired.into());
            }
            let terms = Terms::new(requested, &original.fees.options(deadline))?;
            require_active(&cancelled)?;
            let root = directory.ensure_child("epochs")?;
            let retained = read_epochs(&root, &original)?;
            if retained.len() >= MAX_EPOCHS {
                return Err(ManagedBootstrapFailure::EpochLimit.into());
            }
            let epoch = Epoch {
                ordinal: u8::try_from(retained.len() + 1)
                    .map_err(|_| invalid("bootstrap ordinal overflow"))?,
                previous: retained.last().map(digest).transpose()?,
                parent_intent: original_digest,
                issued_at_unix_ms,
                terms,
            };
            require_active(&cancelled)?;
            attempts::write_record(&root, &format!("{:04}.nrt", epoch.ordinal), &epoch)?;
            let after = read_epochs(&root, &original)?;
            if after.last() != Some(&epoch) || after.len() != retained.len() + 1 {
                return Err(invalid("new bootstrap epoch changed during publication"));
            }
            epoch
        };
        Ok(Self {
            prepared: authority.prepared.clone(),
            original,
            original_digest,
            directory,
            epoch,
            deadline,
            cancelled,
        })
    }

    pub(super) fn validate(
        &self,
        authority: &ServiceAuthority,
        deadline: Instant,
    ) -> Result<Instant> {
        authority.validate_profile()?;
        if self.prepared != authority.prepared {
            return Err(invalid(
                "bootstrap capability belongs to another generated profile",
            ));
        }
        self.check(deadline)
    }
    fn check(&self, deadline: Instant) -> Result<Instant> {
        require_active(&self.cancelled)?;
        let deadline = deadline.min(self.deadline);
        if deadline <= Instant::now() {
            return Err(ManagedBootstrapFailure::AuthorizationExpired.into());
        }
        self.directory.revalidate()?;
        let current = self
            .directory
            .read("original.nrt", super::MAX_ORIGINAL_BYTES)?;
        if *Hash::new(current.as_slice()).as_ref() != self.original_digest
            || current.as_slice() != encode(&self.original, super::MAX_ORIGINAL_BYTES)?.as_slice()
        {
            return Err(invalid(
                "bootstrap original intent changed after authorization",
            ));
        }
        let epoch = &self.epoch;
        if now_ms()? >= epoch.terms.signing_deadline_unix_ms {
            return Err(ManagedBootstrapFailure::AuthorizationExpired.into());
        }
        let retained = read_epochs(&self.directory.open_child("epochs")?, &self.original)?;
        if retained.last() != Some(epoch) {
            return Err(invalid(
                "bootstrap capability no longer selects the current retained epoch",
            ));
        }
        epoch
            .terms
            .signing_deadline(deadline)
            .map_err(|_| ManagedBootstrapFailure::AuthorizationExpired.into())
    }
    pub(super) fn child(&self, purpose: Purpose) -> Result<BootstrapChildAuthorization<'_>> {
        validate_purpose(&self.original, purpose)?;
        Ok(BootstrapChildAuthorization {
            parent: self,
            purpose,
        })
    }
    pub(super) fn funding(&self, provider: ProviderId) -> Result<FundingAuthorization<'_>> {
        self.original.policies.provider(provider)?;
        Ok(FundingAuthorization {
            parent: self,
            provider,
        })
    }
}

impl BootstrapChildAuthorization<'_> {
    pub(in crate::managed) fn bind_account(
        &self,
        account: iroha_wallet::operations::AccountService,
    ) -> Result<iroha_wallet::operations::AccountService> {
        account
            .with_cancellation(Arc::clone(&self.parent.cancelled))
            .map_err(|_| invalid("bootstrap wallet cancellation binding changed"))
    }
    pub(in crate::managed) fn fees(&self) -> &crate::managed::native_operation::Fees {
        &self.parent.original.fees
    }
    pub(in crate::managed) fn check(&self, purpose: Purpose, deadline: Instant) -> Result<Instant> {
        if purpose != self.purpose {
            return Err(invalid("bootstrap dispatch changed its authorized purpose"));
        }
        self.parent.check(deadline)
    }
    pub(in crate::managed) fn validate(
        &self,
        authority: &ServiceAuthority,
        expected: Purpose,
        deadline: Instant,
    ) -> Result<Instant> {
        authority.validate_profile()?;
        if self.purpose != expected || self.parent.prepared != authority.prepared {
            return Err(invalid(
                "bootstrap child capability changed purpose or profile",
            ));
        }
        match provider(expected) {
            Some(provider) if authority.provider_id()? != provider => {
                return Err(invalid(
                    "bootstrap child capability selected another provider",
                ));
            }
            None if authority.provider_id().is_ok() => {
                return Err(invalid(
                    "network bootstrap capability used in provider scope",
                ));
            }
            _ => {}
        }
        self.parent.check(deadline)
    }
    pub(in crate::managed) fn policies(&self) -> &GeneratedServicePolicies {
        &self.parent.original.policies
    }
    pub(in crate::managed) fn terms(
        &self,
        deadline: Instant,
        exclusive_ceiling: Option<u64>,
    ) -> Result<Terms> {
        self.parent.check(deadline)?;
        let mut terms = self.parent.epoch.terms.clone();
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
    pub(in crate::managed) fn origin(&self) -> Result<Origin> {
        let epoch = &self.parent.epoch;
        Ok(Origin::Generated {
            ordinal: epoch.ordinal,
            epoch: digest(epoch)?,
            parent_intent: self.parent.original_digest,
        })
    }
    /// One aggregate replacement claim, durably retained before retiring any wallet request.
    pub(in crate::managed) fn claim_replacement(
        &self,
        previous_attempt: [u8; 32],
        deadline: Instant,
    ) -> Result<()> {
        self.parent.check(deadline)?;
        if previous_attempt == [0; 32] {
            return Err(invalid(
                "unsigned replacement lacks its exact original attempt",
            ));
        }
        let epoch = &self.parent.epoch;
        let root = self.parent.directory.open_child("epochs")?;
        let name = format!("{:04}-replacement.nrt", epoch.ordinal);
        let selected = Replacement {
            epoch: digest(epoch)?,
            purpose: self.purpose,
            previous_attempt,
        };
        if let Some(old) = attempts::read_record::<Replacement>(&root, &name)? {
            if old != selected {
                return Err(ManagedBootstrapFailure::ReplacementLimit.into());
            }
        } else {
            self.parent.check(deadline)?;
            attempts::write_record(&root, &name, &selected)?;
        }
        self.parent.check(deadline)?;
        Ok(())
    }
}
impl FundingAuthorization<'_> {
    pub(in crate::managed) fn validate(
        &self,
        authority: &ServiceAuthority,
        deadline: Instant,
    ) -> Result<Instant> {
        authority.validate_profile()?;
        if self.parent.prepared != authority.prepared || authority.provider_id()? != self.provider {
            return Err(invalid(
                "funding authorization selected another profile or provider",
            ));
        }
        self.parent.check(deadline)
    }
    pub(in crate::managed) fn fees(&self) -> &crate::managed::native_operation::Fees {
        &self.parent.original.fees
    }
    pub(in crate::managed) fn policies(&self) -> &GeneratedServicePolicies {
        &self.parent.original.policies
    }
    pub(in crate::managed) fn child(
        &self,
        purpose: Purpose,
    ) -> Result<BootstrapChildAuthorization<'_>> {
        if !matches!(purpose, Purpose::FundingRequest(p) | Purpose::FundingApproval(p) | Purpose::FundingCredit(p) | Purpose::FundingCapacity(p) if p == self.provider)
        {
            return Err(invalid(
                "funding authorization cannot grant another native purpose",
            ));
        }
        Ok(BootstrapChildAuthorization {
            parent: self.parent,
            purpose,
        })
    }
}

/// Validate all retained epoch records even when complete native history needs no new capability.
pub(super) fn validate_inventory(directory: &PrivateDirectory, original: &Original) -> Result<()> {
    let names = directory.entries(2)?;
    if names
        .iter()
        .any(|name| name != "original.nrt" && name != "epochs")
    {
        return Err(invalid(
            "bootstrap intent contains unknown retained material",
        ));
    }
    match directory.open_child("epochs") {
        Ok(root) => {
            read_epochs(&root, original)?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    directory.revalidate()?;
    Ok(())
}

fn read_epochs(root: &PrivateDirectory, original: &Original) -> Result<Vec<Epoch>> {
    let names = root.entries(MAX_EPOCHS * 2)?;
    let mut expected = Vec::<OsString>::new();
    let mut epochs: Vec<Epoch> = Vec::new();
    let parent_intent = original.digest()?;
    for index in 1..=MAX_EPOCHS {
        let name = format!("{index:04}.nrt");
        let Some(epoch): Option<Epoch> = attempts::read_record(root, &name)? else {
            break;
        };
        epoch.terms.validate()?;
        if usize::from(epoch.ordinal) != index
            || epoch.parent_intent != parent_intent
            || epoch.terms.fees != original.fees
            || epoch.issued_at_unix_ms == 0
            || epoch.issued_at_unix_ms >= epoch.terms.signing_deadline_unix_ms
            || epoch.previous != epochs.last().map(digest).transpose()?
        {
            return Err(invalid(
                "bootstrap epoch changed original intent, fee terms or lineage",
            ));
        }
        expected.push(name.into());
        let claim_name = format!("{index:04}-replacement.nrt");
        if let Some(claim) = attempts::read_record::<Replacement>(root, &claim_name)? {
            validate_purpose(original, claim.purpose)?;
            if claim.epoch != digest(&epoch)? || claim.previous_attempt == [0; 32] {
                return Err(invalid("bootstrap unsigned replacement claim changed"));
            }
            expected.push(claim_name.into());
        }
        epochs.push(epoch);
    }
    expected.sort();
    if names != expected || root.entries(MAX_EPOCHS * 2)? != names {
        return Err(invalid(
            "bootstrap epoch inventory changed or contains gaps or unknown material",
        ));
    }
    Ok(epochs)
}
fn validate_purpose(original: &Original, purpose: Purpose) -> Result<()> {
    if matches!(purpose, Purpose::CustodyRenewal { .. }) {
        return Err(invalid(
            "initial bootstrap cannot authorize custody renewal",
        ));
    }
    if let Some(provider) = provider(purpose) {
        original.policies.provider(provider)?;
    }
    Ok(())
}
fn provider(purpose: Purpose) -> Option<ProviderId> {
    match purpose {
        Purpose::ReservePolicy | Purpose::Reputation => None,
        Purpose::CustodyConfigure(provider)
        | Purpose::CustodyEnroll(provider)
        | Purpose::ReserveAccount(provider)
        | Purpose::FundingRequest(provider)
        | Purpose::FundingApproval(provider)
        | Purpose::FundingCredit(provider)
        | Purpose::FundingCapacity(provider)
        | Purpose::ProviderIngest(provider)
        | Purpose::Gateway(provider)
        | Purpose::CustodyRenewal { provider, .. } => Some(provider),
    }
}
fn profile_expiry(authority: &ServiceAuthority, original: &Original) -> Result<u64> {
    let plans = authority
        .prepared
        .provider_service_plans()?
        .ok_or_else(|| invalid("original provider plans absent"))?;
    let mut end = u64::MAX;
    let now = now_ms()?;
    for plan in plans {
        let selected = original.policies.provider(plan.provider_id())?;
        let admission = plan.admission_material();
        let compliance = authority
            .prepared
            .gateway_compliance_plan(plan.provider_id())?
            .ok_or_else(|| invalid("original compliance plan absent"))?;
        let start = admission
            .issued_at
            .checked_mul(1_000)
            .ok_or_else(|| invalid("original provider interval overflow"))?
            .max(selected.custody.active_from_unix_ms)
            .max(
                compliance
                    .issued_at_unix()
                    .checked_mul(1_000)
                    .ok_or_else(|| invalid("original compliance interval overflow"))?,
            );
        let provider_end = admission
            .retention_epoch
            .checked_mul(1_000)
            .ok_or_else(|| invalid("original provider interval overflow"))?
            .min(selected.custody.active_until_unix_ms)
            .min(
                compliance
                    .expires_at_unix()
                    .checked_mul(1_000)
                    .ok_or_else(|| invalid("original compliance interval overflow"))?,
            );
        if now < start || now >= provider_end {
            return Err(ManagedBootstrapFailure::ProfileExpired.into());
        }
        end = end.min(provider_end);
    }
    Ok(end)
}
fn digest<T: norito::NoritoSerialize>(value: &T) -> Result<[u8; 32]> {
    attempts::semantic_digest(value, attempts::MAX_RECORD_BYTES)
}

/// Cancellation is only a refusal signal, never a source of epoch or native authority.
pub(super) fn require_active(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Acquire) {
        Err(ManagedBootstrapFailure::Cancelled.into())
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "authorization/tests.rs"]
mod tests;
