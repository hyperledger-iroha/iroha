//! Bounded local dispatch custody shared by the closed generated bootstrap purposes.
//!
//! This module cannot inspect, retire, quote, sign or submit a wallet operation. Concrete owners
//! supply canonical wallet inspections and retirement receipts. Local records never prove native
//! state, successful execution or current authority. The stable native intent is stored once.

use super::{Terms, encode, invalid, read_optional, require_empty};
use crate::managed::{ManagedBootstrapFailure, Result};
use iroha_crypto::Hash;
use iroha_data_model::sorafs::capacity::ProviderId;
use iroha_fs::{PrivateDirectory, PublishMode};
use iroha_wallet::operations::{
    NativePreparationPhase, RetiredNativeRequest, VerifiedNativePreparation,
};
use std::ffi::OsStr;

#[path = "attempts/selection.rs"]
mod selection;
pub(in crate::managed) use selection::{generated, initial};

pub(in crate::managed) const MAX_ATTEMPTS: usize = 64;
pub(in crate::managed) const MAX_RECORD_BYTES: usize = 16 * 1024;

/// Fixed paid purposes; a local attempt number is never a native custody sequence or revision.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_deploy::managed::native_operation::attempts::Purpose")]
pub(in crate::managed) enum Purpose {
    ReservePolicy,
    CustodyConfigure(ProviderId),
    CustodyEnroll(ProviderId),
    ReserveAccount(ProviderId),
    FundingRequest(ProviderId),
    FundingApproval(ProviderId),
    FundingCredit(ProviderId),
    FundingCapacity(ProviderId),
    ProviderIngest(ProviderId),
    Gateway(ProviderId),
    Reputation,
    /// Existing explicit renewal authorization stays separate from bootstrap replacement.
    CustodyRenewal {
        provider: ProviderId,
        sequence: u64,
    },
}

#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::native_operation::attempts::Origin")]
pub(in crate::managed) enum Origin {
    /// An existing public purpose API authorizes only this one original attempt.
    Explicit,
    /// A retained record binds an epoch; only a live parent capability can authorize its use.
    Generated {
        ordinal: u8,
        epoch: [u8; 32],
        parent_intent: [u8; 32],
    },
}

#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::native_operation::attempts::Authorization")]
struct Authorization {
    ordinal: u8,
    purpose: Purpose,
    semantic: [u8; 32],
    previous: Option<[u8; 32]>,
    origin: Origin,
    terms: Terms,
}

/// An observation timestamp is useful only after the concrete custody owner authenticates the
/// original anchor and current state. No constructor here supplies that native proof.
#[derive(Clone, Copy, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::native_operation::attempts::Observation")]
pub(in crate::managed) struct Observation {
    pub(in crate::managed) enrollment_observed_at_unix_ms: Option<u64>,
}
impl Observation {
    pub(in crate::managed) const fn ordinary() -> Self {
        Self {
            enrollment_observed_at_unix_ms: None,
        }
    }
    fn validate(&self, authorization: &Authorization) -> Result<()> {
        let enrollment = matches!(
            authorization.purpose,
            Purpose::CustodyEnroll(_) | Purpose::CustodyRenewal { .. }
        );
        if enrollment != self.enrollment_observed_at_unix_ms.is_some()
            || self.enrollment_observed_at_unix_ms.is_some_and(|time| {
                time == 0 || time >= authorization.terms.signing_deadline_unix_ms
            })
        {
            return Err(invalid(
                "dispatch observation differs from its purpose or original bounds",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::native_operation::attempts::RetirementKind")]
enum RetirementKind {
    Missing,
    Request { request_sha256: String },
}

#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::native_operation::attempts::Retirement")]
struct Retirement {
    authorization: [u8; 32],
    successor: [u8; 32],
    kind: RetirementKind,
}

#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::native_operation::attempts::Commit")]
struct Commit {
    authorization: [u8; 32],
    observation: [u8; 32],
    request_sha256: String,
    previous_retirement: Option<[u8; 32]>,
}

/// The semantic operation retains this commitment outside the attempts subtree before any
/// wallet request is created. Lost committed custody cannot become a new empty authorization.
#[derive(Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_deploy::managed::native_operation::attempts::Dispatch")]
struct Dispatch {
    purpose: Purpose,
    semantic: [u8; 32],
    first: [u8; 32],
}

/// A locally validated attempt, not a native proof. Its concrete owner verifies the exact wallet.
pub(in crate::managed) struct Attempt {
    directory: PrivateDirectory,
    authorization: Authorization,
    observation: Option<Observation>,
    commit: Option<Commit>,
    retirement: Option<Retirement>,
}
impl Attempt {
    pub(in crate::managed) fn directory(&self) -> &PrivateDirectory {
        &self.directory
    }
    pub(in crate::managed) fn terms(&self) -> &Terms {
        &self.authorization.terms
    }
    pub(in crate::managed) fn origin(&self) -> &Origin {
        &self.authorization.origin
    }
    pub(in crate::managed) fn digest(&self) -> Result<[u8; 32]> {
        digest(&self.authorization)
    }
    pub(in crate::managed) fn ordinal(&self) -> u8 {
        self.authorization.ordinal
    }
    pub(in crate::managed) fn observation(&self) -> Result<Observation> {
        self.observation
            .ok_or_else(|| ManagedBootstrapFailure::TransitionPending.into())
    }
    pub(in crate::managed) fn is_committed(&self) -> bool {
        self.commit.is_some()
    }
    pub(in crate::managed) fn wallet_path(&self) -> std::path::PathBuf {
        self.directory.path().join("transaction")
    }
    pub(in crate::managed) fn require_selected(&self) -> Result<()> {
        if self.commit.is_none() || self.retirement.is_some() {
            return Err(ManagedBootstrapFailure::TransitionPending.into());
        }
        self.verify_authorization()?;
        if read_record::<Commit>(&self.directory, "committed.nrt")? != self.commit
            || read_record::<Retirement>(&self.directory, "retired.nrt")? != self.retirement
            || read_record::<Observation>(&self.directory, "observation.nrt")? != self.observation
        {
            return Err(invalid("selected dispatch custody changed"));
        }
        Ok(())
    }
    fn verify_authorization(&self) -> Result<()> {
        self.directory.revalidate()?;
        if read_record::<Authorization>(&self.directory, "authorization.nrt")?.as_ref()
            != Some(&self.authorization)
        {
            return Err(invalid(
                "original dispatch authorization was lost or changed",
            ));
        }
        Ok(())
    }
    fn retain_observation(&self, observation: Observation) -> Result<()> {
        self.verify_authorization()?;
        observation.validate(&self.authorization)?;
        write_record(&self.directory, "observation.nrt", &observation)
    }
}

/// Read-only bounded metadata. No arbitrary filename can select a native head or grant signing.
pub(in crate::managed) struct History {
    operation: PrivateDirectory,
    dispatch: Option<Dispatch>,
    root: Option<PrivateDirectory>,
    attempts: Vec<Attempt>,
    empty_tail: bool,
    purpose: Purpose,
    semantic: [u8; 32],
}

/// Typed native intent plus its one selected paid attempt. This is an in-memory view, not a
/// second serialized layout or a constructor for authenticated transaction history.
pub(in crate::managed) struct Selected<I> {
    intent: I,
    history: History,
    pub(in crate::managed) terms: Terms,
}
impl<I> std::ops::Deref for Selected<I> {
    type Target = I;
    fn deref(&self) -> &I {
        &self.intent
    }
}
impl<I> Selected<I> {
    pub(in crate::managed) fn from_history(intent: I, history: History) -> Result<Self> {
        let terms = history
            .selected()?
            .ok_or(ManagedBootstrapFailure::TransitionPending)?
            .terms()
            .clone();
        Ok(Self {
            intent,
            history,
            terms,
        })
    }
    pub(in crate::managed) fn attempt(&self) -> &Attempt {
        // Construction requires a selected committed last attempt; the private history never
        // mutates. Callers still revalidate filesystem custody around their native operations.
        self.history
            .attempts
            .last()
            .expect("selected dispatch history is nonempty")
    }
    pub(in crate::managed) fn directory(&self) -> &PrivateDirectory {
        self.attempt().directory()
    }
    pub(in crate::managed) fn observation(&self) -> Result<Observation> {
        self.attempt().observation()
    }
    pub(in crate::managed) fn verify_wallets(
        &self,
        inspect: impl FnMut(&I, &Attempt) -> Result<VerifiedNativePreparation>,
    ) -> Result<()> {
        let mut inspect = inspect;
        self.history
            .verify_wallets(|attempt| inspect(&self.intent, attempt))
    }
}
impl History {
    pub(in crate::managed) fn read(
        operation: &PrivateDirectory,
        purpose: Purpose,
        semantic: [u8; 32],
    ) -> Result<Self> {
        operation.revalidate()?;
        let names = operation.entries(3)?;
        if names
            .iter()
            .any(|name| name != "original.nrt" && name != "dispatch.nrt" && name != "attempts")
        {
            return Err(invalid("dispatch operation contains unknown material"));
        }
        let intent = read_optional(
            operation,
            "original.nrt",
            super::MAX_CHECKPOINT_BYTES + 3 * 1024 * 1024,
        )?
        .ok_or_else(|| invalid("dispatch lost its original semantic intent"))?;
        if *Hash::new(&intent).as_ref() != semantic {
            return Err(invalid("dispatch semantic original was changed"));
        }
        let retained_operation = PrivateDirectory::open_exact(operation.path())?;
        if retained_operation.identity()? != operation.identity()? {
            return Err(invalid("dispatch operation custody changed"));
        }
        let dispatch: Option<Dispatch> = read_record(operation, "dispatch.nrt")?;
        let root = match operation.open_child("attempts") {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if dispatch.is_some() {
                    return Err(invalid("retained dispatch lost its attempts custody"));
                }
                return Ok(Self {
                    operation: retained_operation,
                    dispatch,
                    root: None,
                    attempts: Vec::new(),
                    empty_tail: false,
                    purpose,
                    semantic,
                });
            }
            Err(error) => return Err(error.into()),
        };
        let names = root.entries(MAX_ATTEMPTS)?;
        let mut attempts: Vec<Attempt> = Vec::with_capacity(names.len());
        let mut empty_tail = false;
        for (index, name) in names.iter().enumerate() {
            if name != OsStr::new(&format!("{:04}", index + 1)) {
                return Err(invalid(
                    "dispatch attempt inventory has a gap or foreign name",
                ));
            }
            let directory = root.open_child(name)?;
            let inventory = directory.entries(7)?;
            if inventory.iter().any(|name| {
                ![
                    "authorization.nrt",
                    "observation.nrt",
                    "committed.nrt",
                    "retired.nrt",
                    "transaction",
                    "replay.nrt",
                    "carrier.nrt",
                ]
                .iter()
                .any(|allowed| name == OsStr::new(allowed))
            }) {
                return Err(invalid("dispatch attempt contains unknown material"));
            }
            let Some(authorization): Option<Authorization> =
                read_record(&directory, "authorization.nrt")?
            else {
                if index + 1 != names.len() || !inventory.is_empty() {
                    return Err(invalid(
                        "dispatch material exists without its authorization",
                    ));
                }
                empty_tail = true;
                continue;
            };
            authorization.terms.validate()?;
            validate_origin(&authorization.origin)?;
            let previous = attempts.last().map(Attempt::digest).transpose()?;
            if usize::from(authorization.ordinal) != index + 1
                || authorization.purpose != purpose
                || authorization.semantic != semantic
                || authorization.previous != previous
            {
                return Err(invalid(
                    "dispatch authorization changed its original purpose, intent or predecessor",
                ));
            }
            if let Some(prior) = attempts.last() {
                validate_successor(&prior.authorization, &authorization)?;
            }
            let observation: Option<Observation> = read_record(&directory, "observation.nrt")?;
            if let Some(observation) = observation {
                observation.validate(&authorization)?;
            }
            let commit: Option<Commit> = read_record(&directory, "committed.nrt")?;
            let retirement: Option<Retirement> = read_record(&directory, "retired.nrt")?;
            let attempt = Attempt {
                directory,
                authorization,
                observation,
                commit,
                retirement,
            };
            if let Some(commit) = &attempt.commit {
                let observed = attempt
                    .observation
                    .ok_or_else(|| invalid("committed dispatch lost its original observation"))?;
                let prior_retirement = attempts
                    .last()
                    .map(|prior| {
                        prior
                            .retirement
                            .as_ref()
                            .ok_or_else(|| invalid("dispatch predecessor was not retired"))
                    })
                    .transpose()?;
                if !valid_sha256(&commit.request_sha256)
                    || commit.authorization != attempt.digest()?
                    || commit.observation != digest(&observed)?
                    || commit.previous_retirement != prior_retirement.map(digest).transpose()?
                {
                    return Err(invalid("dispatch commit differs from original custody"));
                }
            } else if inventory.iter().any(|name| {
                ["replay.nrt", "carrier.nrt"]
                    .iter()
                    .any(|later| name == OsStr::new(later))
            }) {
                return Err(invalid(
                    "uncommitted dispatch contains native execution material",
                ));
            }
            if let Some(retirement) = &attempt.retirement {
                if retirement.authorization != attempt.digest()?
                    || retirement.successor == [0; 32]
                    || matches!(&retirement.kind, RetirementKind::Request { request_sha256 } if !valid_sha256(request_sha256))
                    || inventory.iter().any(|name| {
                        ["replay.nrt", "carrier.nrt"]
                            .iter()
                            .any(|later| name == OsStr::new(later))
                    })
                {
                    return Err(invalid(
                        "retired dispatch contains changed or execution material",
                    ));
                }
            }
            if let Some(prior) = attempts.last() {
                if let Some(retirement) = &prior.retirement {
                    if retirement.successor != attempt.digest()? {
                        return Err(invalid("unsigned retirement selected another successor"));
                    }
                } else if index + 1 != names.len() || attempt.commit.is_some() {
                    return Err(invalid(
                        "dispatch chain has an unretired interior predecessor",
                    ));
                }
            }
            attempts.push(attempt);
        }
        if let Some(last) = attempts.last() {
            if last.retirement.is_some() {
                return Err(invalid("unsigned retirement lost its reserved successor"));
            }
        }
        if root.entries(MAX_ATTEMPTS)? != names {
            return Err(invalid("dispatch inventory changed during inspection"));
        }
        match &dispatch {
            Some(dispatch) => {
                if dispatch.purpose != purpose
                    || dispatch.semantic != semantic
                    || attempts.first().map(Attempt::digest).transpose()? != Some(dispatch.first)
                {
                    return Err(invalid(
                        "dispatch root differs from its first retained authorization",
                    ));
                }
            }
            None => {
                if attempts.len() > 1
                    || attempts.first().is_some_and(|first| {
                        first.observation.is_some()
                            || first.commit.is_some()
                            || first.retirement.is_some()
                    })
                    || attempts
                        .first()
                        .map(|first| first.directory.entries(7))
                        .transpose()?
                        .is_some_and(|names| names.len() != 1)
                {
                    return Err(invalid(
                        "dispatch material lost its original root commitment",
                    ));
                }
            }
        }
        operation.revalidate()?;
        Ok(Self {
            operation: retained_operation,
            dispatch,
            root: Some(root),
            attempts,
            empty_tail,
            purpose,
            semantic,
        })
    }

    /// Inspect every retained wallet through the concrete canonical purpose owner. The closure
    /// cannot substitute a decoded DTO for the opaque wallet result.
    pub(in crate::managed) fn verify_wallets(
        &self,
        mut inspect: impl FnMut(&Attempt) -> Result<VerifiedNativePreparation>,
    ) -> Result<()> {
        self.require_current()?;
        for (index, attempt) in self.attempts.iter().enumerate() {
            if attempt.observation.is_none() {
                if attempt
                    .directory
                    .entries(7)?
                    .iter()
                    .any(|name| name == "transaction")
                {
                    return Err(invalid(
                        "dispatch wallet exists without a selected observation",
                    ));
                }
                if let Some(retirement) = &attempt.retirement {
                    if retirement.kind != RetirementKind::Missing {
                        return Err(invalid("unobserved dispatch has a request retirement"));
                    }
                }
                continue;
            }
            let preparation = inspect(attempt)?;
            if self.empty_tail
                && index + 1 == self.attempts.len()
                && matches!(
                    preparation.phase(),
                    NativePreparationPhase::PayloadRetained | NativePreparationPhase::Signed
                )
            {
                return Err(invalid(
                    "unfinished successor directory cannot replace paid custody",
                ));
            }
            if let Some(commit) = &attempt.commit {
                if preparation.request_sha256() != Some(commit.request_sha256.as_str()) {
                    return Err(invalid(
                        "committed dispatch lost or changed its original wallet request",
                    ));
                }
            } else if matches!(
                preparation.phase(),
                NativePreparationPhase::PayloadRetained | NativePreparationPhase::Signed
            ) {
                return Err(invalid(
                    "uncommitted dispatch contains a paid payload or signature",
                ));
            }
            match (&attempt.retirement, preparation.phase()) {
                (
                    Some(Retirement {
                        kind: RetirementKind::Missing,
                        ..
                    }),
                    NativePreparationPhase::Missing,
                ) => {}
                (
                    Some(Retirement {
                        kind: RetirementKind::Request { request_sha256 },
                        ..
                    }),
                    NativePreparationPhase::Retired,
                ) if preparation.request_sha256() == Some(request_sha256.as_str()) => {}
                (Some(_), _) => {
                    return Err(invalid(
                        "retired dispatch wallet no longer proves its exact unsigned retirement",
                    ));
                }
                (None, NativePreparationPhase::Retired)
                    if index + 1 == self.attempts.len() - 1
                        && self
                            .attempts
                            .last()
                            .is_some_and(|next| !next.is_committed()) => {}
                (None, NativePreparationPhase::Retired) => {
                    return Err(invalid("retired wallet has no exact reserved successor"));
                }
                (None, _) if index + 1 == self.attempts.len() => {}
                (None, NativePreparationPhase::Missing | NativePreparationPhase::RequestOnly)
                    if index + 2 == self.attempts.len()
                        && self
                            .attempts
                            .last()
                            .is_some_and(|next| !next.is_committed()) => {}
                (None, _) => {
                    return Err(invalid(
                        "a reserved successor cannot replace retained paid material",
                    ));
                }
            }
        }
        self.require_current()?;
        Ok(())
    }

    fn require_current(&self) -> Result<()> {
        self.operation.revalidate()?;
        let current = Self::read(&self.operation, self.purpose, self.semantic)?;
        if self.dispatch != current.dispatch
            || self.empty_tail != current.empty_tail
            || self.attempts.len() != current.attempts.len()
            || self
                .root
                .as_ref()
                .map(PrivateDirectory::identity)
                .transpose()?
                != current
                    .root
                    .as_ref()
                    .map(PrivateDirectory::identity)
                    .transpose()?
        {
            return Err(invalid(
                "dispatch inventory changed during native operation",
            ));
        }
        for (before, after) in self.attempts.iter().zip(&current.attempts) {
            if before.directory.identity()? != after.directory.identity()?
                || before.authorization != after.authorization
                || before.observation != after.observation
                || before.commit != after.commit
                || before.retirement != after.retirement
            {
                return Err(invalid(
                    "retained dispatch metadata changed during native operation",
                ));
            }
        }
        Ok(())
    }

    fn retain_root(&self, operation: &PrivateDirectory) -> Result<()> {
        self.require_current()?;
        let first = self
            .attempts
            .first()
            .ok_or_else(|| invalid("dispatch root has no first authorization"))?;
        write_record(
            operation,
            "dispatch.nrt",
            &Dispatch {
                purpose: self.purpose,
                semantic: self.semantic,
                first: first.digest()?,
            },
        )
    }

    /// Compare every retained authorization before wallet inspection or native HTTP.
    pub(in crate::managed) fn require_fees(&self, fees: &super::Fees) -> Result<()> {
        fees.validate()?;
        if self
            .attempts
            .iter()
            .any(|attempt| &attempt.terms().fees != fees)
        {
            return Err(invalid(
                "retained dispatch differs from the full original fee authorization",
            ));
        }
        Ok(())
    }

    pub(in crate::managed) fn selected(&self) -> Result<Option<&Attempt>> {
        if self.empty_tail {
            return Err(ManagedBootstrapFailure::TransitionPending.into());
        }
        let Some(last) = self.attempts.last() else {
            return Ok(None);
        };
        last.require_selected()?;
        Ok(Some(last))
    }
    pub(in crate::managed) fn last(&self) -> Option<&Attempt> {
        self.attempts.last()
    }
    pub(in crate::managed) fn predecessor(&self) -> Option<&Attempt> {
        self.attempts
            .len()
            .checked_sub(2)
            .and_then(|index| self.attempts.get(index))
    }

    /// Called only after the concrete owner validates its live authorization. This reserves
    /// exact terms; it neither retires its predecessor nor grants dispatch permission.
    pub(in crate::managed) fn reserve(
        &self,
        operation: &PrivateDirectory,
        origin: Origin,
        terms: Terms,
    ) -> Result<Attempt> {
        self.require_current()?;
        terms.validate()?;
        validate_origin(&origin)?;
        let index = self.attempts.len() + 1;
        if index > MAX_ATTEMPTS {
            return Err(ManagedBootstrapFailure::EpochLimit.into());
        }
        let authorization = Authorization {
            ordinal: u8::try_from(index).map_err(|_| invalid("dispatch ordinal overflow"))?,
            purpose: self.purpose,
            semantic: self.semantic,
            previous: self.attempts.last().map(Attempt::digest).transpose()?,
            origin,
            terms,
        };
        if let Some(prior) = self.attempts.last() {
            validate_successor(&prior.authorization, &authorization)?;
        }
        if operation.identity()? != self.operation.identity()? {
            return Err(invalid("dispatch selected another operation custody"));
        }
        let root = match &self.root {
            Some(old) => {
                let root = operation.open_child("attempts")?;
                if old.identity()? != root.identity()? {
                    return Err(invalid("dispatch attempt custody changed"));
                }
                root
            }
            None => operation.create_child("attempts")?,
        };
        let directory = if self.empty_tail {
            root.open_child(format!("{index:04}"))?
        } else {
            root.create_child(format!("{index:04}"))?
        };
        require_empty(&directory)?;
        write_record(&directory, "authorization.nrt", &authorization)?;
        let first = self
            .attempts
            .first()
            .map(Attempt::digest)
            .transpose()?
            .unwrap_or(digest(&authorization)?);
        write_record(
            operation,
            "dispatch.nrt",
            &Dispatch {
                purpose: self.purpose,
                semantic: self.semantic,
                first,
            },
        )?;
        Ok(Attempt {
            directory,
            authorization,
            observation: None,
            commit: None,
            retirement: None,
        })
    }
}

/// Retain exact no-wallet retirement under the purpose lock; no decoded marker supplies proof.
pub(in crate::managed) fn retire_missing(previous: &Attempt, successor: &Attempt) -> Result<()> {
    if previous.is_committed() {
        return Err(invalid(
            "a committed dispatch cannot infer unsigned absence from a missing wallet",
        ));
    }
    let names = previous.directory.entries(7)?;
    if names.iter().any(|name| {
        ![
            "authorization.nrt",
            "observation.nrt",
            "committed.nrt",
            "retired.nrt",
        ]
        .iter()
        .any(|allowed| name == OsStr::new(allowed))
    }) {
        return Err(invalid(
            "missing-wallet retirement refuses later or unknown material",
        ));
    }
    write_retirement(previous, successor, RetirementKind::Missing)
}

/// Consume only the canonical purpose owner's opaque exact RequestOnly retirement receipt.
pub(in crate::managed) fn retire_request(
    previous: &Attempt,
    successor: &Attempt,
    receipt: &RetiredNativeRequest,
) -> Result<()> {
    if receipt.journal_path() != previous.wallet_path() || !valid_sha256(receipt.request_sha256()) {
        return Err(invalid(
            "wallet retirement receipt selected another original request",
        ));
    }
    write_retirement(
        previous,
        successor,
        RetirementKind::Request {
            request_sha256: receipt.request_sha256().to_owned(),
        },
    )
}

fn write_retirement(previous: &Attempt, successor: &Attempt, kind: RetirementKind) -> Result<()> {
    previous.verify_authorization()?;
    successor.verify_authorization()?;
    if let (Some(commit), RetirementKind::Request { request_sha256 }) = (&previous.commit, &kind) {
        if commit.request_sha256 != *request_sha256 {
            return Err(invalid("retirement changed the committed wallet request"));
        }
    }
    if successor.authorization.previous != Some(previous.digest()?)
        || successor.authorization.purpose != previous.authorization.purpose
        || successor.authorization.semantic != previous.authorization.semantic
    {
        return Err(invalid(
            "unsigned retirement has another native intent or predecessor",
        ));
    }
    if read_optional(
        &previous.directory,
        "carrier.nrt",
        super::MAX_CHECKPOINT_BYTES,
    )?
    .is_some()
        || read_optional(
            &previous.directory,
            "replay.nrt",
            super::MAX_CHECKPOINT_BYTES,
        )?
        .is_some()
    {
        return Err(invalid("native carrier custody cannot be retired"));
    }
    write_record(
        &previous.directory,
        "retired.nrt",
        &Retirement {
            authorization: previous.digest()?,
            successor: successor.digest()?,
            kind,
        },
    )
}

/// The caller has just authenticated the concrete native prerequisite. Commit its exact selected
/// observation, canonical RequestOnly commitment and predecessor retirement before payload
/// preparation or signing can begin.
pub(in crate::managed) fn commit(
    attempt: &Attempt,
    previous: Option<&Attempt>,
    observation: Observation,
    preparation: &VerifiedNativePreparation,
) -> Result<()> {
    attempt.verify_authorization()?;
    if let Some(previous) = previous {
        previous.verify_authorization()?;
    }
    if preparation.phase() != NativePreparationPhase::RequestOnly {
        return Err(invalid(
            "dispatch commit requires the exact retained unsigned wallet request",
        ));
    }
    let request_sha256 = preparation
        .request_sha256()
        .ok_or_else(|| invalid("retained wallet request has no canonical commitment"))?;
    if !valid_sha256(request_sha256) {
        return Err(invalid("invalid original wallet request commitment"));
    }
    observation.validate(&attempt.authorization)?;
    let authorization_digest = attempt.digest()?;
    let retirement: Option<Retirement> = previous
        .map(|prior| {
            read_record(&prior.directory, "retired.nrt")
                .and_then(|value| value.ok_or_else(|| invalid("predecessor retirement absent")))
        })
        .transpose()?;
    if attempt.authorization.previous != previous.map(Attempt::digest).transpose()?
        || retirement
            .as_ref()
            .is_some_and(|value| value.successor != authorization_digest)
    {
        return Err(invalid("dispatch commit has another predecessor"));
    }
    if read_record::<Observation>(&attempt.directory, "observation.nrt")? != Some(observation) {
        return Err(invalid("dispatch commit lost its selected observation"));
    }
    write_record(
        &attempt.directory,
        "committed.nrt",
        &Commit {
            authorization: attempt.digest()?,
            observation: digest(&observation)?,
            request_sha256: request_sha256.to_owned(),
            previous_retirement: retirement.as_ref().map(digest).transpose()?,
        },
    )
}

pub(in crate::managed) fn semantic_digest<T: norito::NoritoSerialize>(
    value: &T,
    maximum: usize,
) -> Result<[u8; 32]> {
    Ok(*Hash::new(encode(value, maximum)?).as_ref())
}
fn digest<T: norito::NoritoSerialize>(value: &T) -> Result<[u8; 32]> {
    semantic_digest(value, MAX_RECORD_BYTES)
}
fn valid_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
fn validate_origin(origin: &Origin) -> Result<()> {
    if let Origin::Generated {
        ordinal,
        epoch,
        parent_intent,
    } = origin
    {
        if *ordinal == 0
            || usize::from(*ordinal) > MAX_ATTEMPTS
            || *epoch == [0; 32]
            || *parent_intent == [0; 32]
        {
            return Err(invalid(
                "dispatch refers to an invalid generated authorization epoch",
            ));
        }
    }
    Ok(())
}

fn validate_successor(previous: &Authorization, successor: &Authorization) -> Result<()> {
    if previous.terms.fees != successor.terms.fees || matches!(successor.origin, Origin::Explicit) {
        return Err(invalid(
            "unsigned successor changed fixed fees or lacks a generated epoch",
        ));
    }
    if let (
        Origin::Generated {
            ordinal: before,
            parent_intent: original,
            ..
        },
        Origin::Generated {
            ordinal: after,
            parent_intent: current,
            ..
        },
    ) = (&previous.origin, &successor.origin)
    {
        if after <= before || original != current {
            return Err(invalid(
                "unsigned successor did not advance the same parent authorization history",
            ));
        }
    }
    Ok(())
}
pub(in crate::managed) fn read_record<
    T: norito::NoritoSerialize + for<'a> norito::NoritoDeserialize<'a>,
>(
    directory: &PrivateDirectory,
    name: &str,
) -> Result<Option<T>> {
    let Some(bytes) = read_optional(directory, name, MAX_RECORD_BYTES)? else {
        return Ok(None);
    };
    norito::decode_canonical_with_limits(
        &bytes,
        norito::DecodeLimits::new(
            MAX_RECORD_BYTES,
            MAX_RECORD_BYTES,
            MAX_RECORD_BYTES,
            MAX_RECORD_BYTES * 8,
            32,
        ),
    )
    .map(Some)
    .map_err(|_| invalid("invalid canonical dispatch custody record"))
}
pub(in crate::managed) fn write_record<T: norito::NoritoSerialize>(
    directory: &PrivateDirectory,
    name: &str,
    value: &T,
) -> Result<()> {
    let bytes = encode(value, MAX_RECORD_BYTES)?;
    if let Some(retained) = read_optional(directory, name, MAX_RECORD_BYTES)? {
        if retained != bytes {
            return Err(invalid("immutable dispatch custody record changed"));
        }
        return Ok(());
    }
    directory.write_atomic(name, &bytes, PublishMode::CreateNew)?;
    Ok(())
}

#[cfg(test)]
#[path = "attempts/tests.rs"]
mod tests;
