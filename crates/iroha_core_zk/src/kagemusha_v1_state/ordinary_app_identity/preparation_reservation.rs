//! Original native financial witness and nonce custody before the first issuer HTTP request.
//! This private WAL is software custody, not a hardware monotonicity or monetary capability.

use super::super::{KagemushaOrdinaryNativeClockOwnerV1, KagemushaOrdinaryNativeTimeIntervalV1};
use super::super::{PrivateJournal, PrivateJournalFormat};
#[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
use super::journal::continuous_clock::Reading;
use super::{
    Custody, KagemushaOrdinaryGovernedPolicyOriginalsV1, KagemushaOrdinaryIdentityErrorV1,
    KagemushaPreparedOrdinaryAppEnrollmentV1, Rejected, Result,
};
use iroha_data_model::kagemusha::*;
use rand_core_06::{OsRng, RngCore as _};
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
use zeroize::{Zeroize as _, Zeroizing};

#[path = "preparation_reservation/integrity_refresh.rs"]
mod integrity_refresh;
pub use integrity_refresh::KagemushaOrdinaryIntegrityRefreshOwnerV1;

#[path = "preparation_reservation/current_control.rs"]
mod current_control;
pub(crate) use current_control::KagemushaCapturedOrdinaryFinancialControlDecisionV1;
pub use current_control::{
    KagemushaAuthenticatedOrdinaryCurrentFinancialControlLoanV1,
    KagemushaOrdinaryCurrentFinancialControlOwnerV1,
};

#[path = "preparation_reservation/lineage_cas.rs"]
mod lineage_cas;
pub use lineage_cas::KagemushaAuthenticatedOrdinaryLineageAccountSigningV1;
pub use lineage_cas::{
    KAGEMUSHA_ORDINARY_RECEIVED_COMMIT_ORIGINAL_MAX_BYTES_V1,
    KagemushaAuthenticatedOrdinaryReceivedLineageCommitAssertionV1,
    KagemushaOrdinaryReceivedLineageCommitOriginalV1,
};
pub(crate) use lineage_cas::{
    KagemushaAuthenticatedOrdinaryIncomingCommitReceiptV1,
    KagemushaAuthenticatedOrdinaryIncomingReservationReceiptV1,
    KagemushaAuthenticatedOrdinaryLineageAnchorReceiptV1,
    KagemushaAuthenticatedOrdinaryLineageCommitReceiptV1,
    KagemushaAuthenticatedOrdinaryLineageReservationReceiptV1, KagemushaOrdinaryLineageCasOwnerV1,
};

#[path = "preparation_reservation/finalized_mint_source.rs"]
mod finalized_mint_source;
pub use finalized_mint_source::KagemushaAuthenticatedOrdinaryFinalizedMintSourceV1;

const FORMAT: PrivateJournalFormat = PrivateJournalFormat {
    filename: "ordinary-preparation.norito.wal",
    magic: b"KGMCINI1",
    hash_domain: b"iroha:kagemusha:v1:ordinary-preparation-reservation\0",
    maximum_payload_bytes: 256 * 1024,
};

/// Immutable native-selected account and governed originals, before any response exists.
/// Rust provisioning supplies these from independently authenticated installed custody.
/// No C/JNI field, account cache or offered preparation can construct this selection.
pub struct KagemushaOrdinaryPreparationSelectedOriginalsV1 {
    owner: KagemushaRetailEnrollmentOwnerV1,
    world_schema_hash: Option<iroha_crypto::Hash>,
    governed: KagemushaOrdinaryGovernedPolicyOriginalsV1,
    issuer: KagemushaRetailEnrollmentIssuerPolicyV1,
    ordinary: Arc<KagemushaOrdinaryRetailIdentityPolicyOriginalsV1>,
    core_authorization_key_reference: [u8; 32],
    clock_selection_digest: Option<[u8; 32]>,
    clock: SelectedClock,
}
enum SelectedClock {
    Native(Arc<Mutex<KagemushaOrdinaryNativeClockOwnerV1>>),
    #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
    Fixture(Box<FixtureClock>),
}
#[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
struct FixtureClock {
    reference_ms: u64,
    reading: Reading,
}

impl KagemushaOrdinaryPreparationSelectedOriginalsV1 {
    /// Bind actual native scope and threshold-authenticated governed originals.
    /// This validates their crypto/policy relationship; the installing Rust owner must retain
    /// independent issuer/account/runtime custody. No application ABI accepts these arguments.
    /// # Errors
    /// Rejects another scope, ungoverned profile, key role or trusted interval.
    #[allow(clippy::too_many_arguments)]
    #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
    pub fn from_selected_originals(
        owner: KagemushaRetailEnrollmentOwnerV1,
        release: Arc<KagemushaAuthenticatedReleaseV1>,
        issuer: KagemushaRetailEnrollmentIssuerPolicyV1,
        ordinary: Arc<KagemushaOrdinaryRetailIdentityPolicyOriginalsV1>,
        trust: KagemushaOrdinaryAppTrustPolicyV1,
        authority: KagemushaAppAttestationAuthorityPolicyV1,
        profile_id: [u8; 32],
        original_core_public_key: &KagemushaDevicePublicKeyV1,
        trusted_native_reference_ms: u64,
    ) -> Result<Self> {
        let trust_original = norito::encode_canonical(&trust).map_err(|_| Rejected)?;
        let authority_original = authority
            .canonical_digest_preimage_v1()
            .map_err(|_| Rejected)?;
        let governed = KagemushaOrdinaryGovernedPolicyOriginalsV1::authenticate(
            release,
            profile_id,
            &trust_original,
            &authority_original.bytes,
        )?;
        Self::from_governed_originals(
            owner,
            governed,
            issuer,
            ordinary,
            original_core_public_key,
            trusted_native_reference_ms,
        )
    }

    /// Join retained release-authenticated policy originals to independent native custody.
    ///
    /// Policy admission supplies neither account/runtime ownership, issuer authority, a native
    /// Core key nor trusted time. The Rust provisioner must supply each original separately.
    /// No application ABI accepts these inputs or constructs a selected owner.
    /// # Errors
    /// Rejects another scope, key role, issuer/runtime or trusted interval.
    #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
    pub fn from_governed_originals(
        owner: KagemushaRetailEnrollmentOwnerV1,
        governed: KagemushaOrdinaryGovernedPolicyOriginalsV1,
        issuer: KagemushaRetailEnrollmentIssuerPolicyV1,
        ordinary: Arc<KagemushaOrdinaryRetailIdentityPolicyOriginalsV1>,
        original_core_public_key: &KagemushaDevicePublicKeyV1,
        trusted_native_reference_ms: u64,
    ) -> Result<Self> {
        original_core_public_key.validate().map_err(|_| Rejected)?;
        let this = Self {
            owner,
            world_schema_hash: None,
            governed,
            issuer,
            ordinary,
            core_authorization_key_reference: kagemusha_core_authorization_key_reference_v1(
                original_core_public_key,
            ),
            clock_selection_digest: None,
            clock: SelectedClock::Fixture(Box::new(FixtureClock {
                reference_ms: trusted_native_reference_ms,
                reading: Reading::now()?,
            })),
        };
        this.recheck_at_trusted_time(trusted_native_reference_ms)?;
        Ok(this)
    }
    /// Join the actual current Native signed-clock owner with the release-authenticated policy
    /// originals and the independently retained account/runtime/issuer/Core-key custody.
    /// No copied caller timestamp, clock callback or managed frame constructs this selection.
    /// # Errors
    /// Refuses wrong account/runtime/network, expired actual clock or another governed scope/key.
    pub fn from_governed_originals_with_native_clock(
        owner: KagemushaRetailEnrollmentOwnerV1,
        governed: KagemushaOrdinaryGovernedPolicyOriginalsV1,
        issuer: KagemushaRetailEnrollmentIssuerPolicyV1,
        ordinary: Arc<KagemushaOrdinaryRetailIdentityPolicyOriginalsV1>,
        original_core_public_key: &KagemushaDevicePublicKeyV1,
        clock: Arc<Mutex<KagemushaOrdinaryNativeClockOwnerV1>>,
        installed_clock_selection_digest: [u8; 32],
        installed_world_schema_hash: iroha_crypto::Hash,
    ) -> Result<Self> {
        original_core_public_key.validate().map_err(|_| Rejected)?;
        {
            let actual = clock.lock().map_err(|_| Custody)?;
            if actual.network_id().map_err(|_| Custody)? != owner.runtime.network_id
                || installed_clock_selection_digest == [0; 32]
                || actual.installed_selection_digest().map_err(|_| Custody)?
                    != installed_clock_selection_digest
            {
                return Err(Rejected);
            }
        }
        let this = Self {
            owner,
            world_schema_hash: Some(installed_world_schema_hash),
            governed,
            issuer,
            ordinary,
            core_authorization_key_reference: kagemusha_core_authorization_key_reference_v1(
                original_core_public_key,
            ),
            clock_selection_digest: Some(installed_clock_selection_digest),
            clock: SelectedClock::Native(clock),
        };
        this.trusted_time_ms()?;
        Ok(this)
    }
    /// Borrow the current actual interval from the retained signed-clock owner and recheck
    /// selected issuer/profile activation at its lower endpoint and expiry at its upper endpoint.
    /// # Errors
    /// Rejects stale originals, changed clock selection, future policy, expiry or unknown durability.
    pub fn trusted_time_interval(&self) -> Result<KagemushaOrdinaryNativeTimeIntervalV1> {
        let interval = match &self.clock {
            SelectedClock::Native(clock) => {
                let mut actual = clock.lock().map_err(|_| Custody)?;
                if Some(actual.installed_selection_digest().map_err(|_| Custody)?)
                    != self.clock_selection_digest
                {
                    return Err(Rejected);
                }
                actual.current_native_time_interval().map_err(|_| Custody)?
            }
            #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
            SelectedClock::Fixture(fixture) => {
                KagemushaOrdinaryNativeTimeIntervalV1::fixture_point(
                    fixture
                        .reference_ms
                        .checked_add(Reading::now()?.elapsed_ms(fixture.reading)?)
                        .ok_or(Custody)?,
                )
            }
        };
        interval.check_both(|now| self.recheck_at_trusted_time(now))?;
        Ok(interval)
    }
    /// Lower bound for immutable Native-issued timestamps only. This is not a universal current
    /// point: current original validity must use `trusted_time_interval().check_both(...)`.
    /// # Errors
    /// Rejects the same selected-current interval failures.
    pub fn trusted_time_ms(&self) -> Result<u64> {
        self.trusted_time_interval()
            .map(|interval| interval.lower_ms())
    }
    pub(super) fn time_for_native_or_fixture(&self, offered_fixture_time: u64) -> Result<u64> {
        #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
        if matches!(&self.clock, SelectedClock::Fixture(_)) {
            self.recheck_at_trusted_time(offered_fixture_time)?;
            return Ok(offered_fixture_time);
        }
        let _ = offered_fixture_time;
        self.trusted_time_ms()
    }
    /// Read the same independently selected stable enrollment ID; this creates no reservation.
    /// # Errors
    /// Rejects stale native original custody or an invalid canonical owner.
    pub fn enrollment_id(&self) -> Result<[u8; 32]> {
        self.trusted_time_ms()?;
        self.owner.enrollment_id().map_err(|_| Rejected)
    }
    /// Exact governed optional Integrity original digest, never selected from a token or response.
    /// # Errors
    /// Rejects stale native selected originals.
    pub fn integrity_policy_digest(&self) -> Result<Option<[u8; 32]>> {
        self.trusted_time_ms()?;
        Ok(self
            .governed
            .trust()
            .play_integrity_policy
            .as_ref()
            .map(|p| p.policy_digest))
    }
    pub(super) fn core_authorization_key_reference(&self) -> Result<[u8; 32]> {
        self.trusted_time_ms()?;
        Ok(self.core_authorization_key_reference)
    }
    pub(super) fn preparation_issuer_key(&self) -> Result<&iroha_crypto::PublicKey> {
        self.trusted_time_ms()?;
        Ok(&self
            .ordinary
            .identity_policy()
            .policy()
            .enrollment_issuer_key)
    }

    // Data projection for the separate issuer-side durable C owner. Current FI customer/request
    // admission and protected signer custody remain the installing Native parent's responsibility.
    pub(super) fn issuer_challenge_for_carrier(
        &self,
        carrier: &KagemushaOrdinaryPreparationCarrierV1,
        server_nonce: [u8; 32],
        issued_at_ms: u64,
        expires_at_ms: u64,
    ) -> Result<KagemushaOrdinaryAppEnrollmentChallengeV1> {
        self.trusted_time_ms()?;
        let enabled = self
            .governed
            .release()
            .enabled_profile(self.governed.profile_id())
            .ok_or(Rejected)?;
        if carrier.account_i105
            != self
                .owner
                .account_id
                .canonical_i105()
                .map_err(|_| Rejected)?
            || carrier.release_id != self.governed.release().release_id()
            || carrier.hardware_profile_id != self.governed.profile_id()
            || carrier.lane_id != self.owner.lane_id
            || issued_at_ms
                < self
                    .ordinary
                    .identity_policy()
                    .policy()
                    .profile
                    .valid_from_ms
            || expires_at_ms
                > self
                    .ordinary
                    .identity_policy()
                    .policy()
                    .profile
                    .expires_at_ms
            || issued_at_ms < enabled.hardware_profile.valid_from_ms
            || expires_at_ms > enabled.hardware_profile.expires_at_ms
        {
            return Err(Rejected);
        }
        let challenge = KagemushaOrdinaryAppEnrollmentChallengeV1 {
            version: 1,
            platform_class: enabled.hardware_profile.platform_class,
            enrollment_id: self.owner.enrollment_id().map_err(|_| Rejected)?,
            client_nonce: carrier.client_nonce,
            server_nonce,
            account_binding: kagemusha_ordinary_app_account_binding_v1(&self.owner.account_id),
            network_id: *self.owner.runtime.network_id.as_bytes(),
            lane_id: self.owner.lane_id,
            release_id: self.governed.release().release_id(),
            hardware_profile_id: self.governed.profile_id(),
            suite_id: enabled.suite_id,
            trust_policy_digest: self
                .governed
                .trust()
                .canonical_digest()
                .map_err(|_| Rejected)?,
            app_authority_policy_digest: self
                .governed
                .authority()
                .canonical_digest()
                .map_err(|_| Rejected)?,
            financial_authority_commitment: carrier.financial_authority_commitment,
            issuer_policy_digest: self
                .ordinary
                .issuer_policy()
                .policy()
                .canonical_digest()
                .map_err(|_| Rejected)?,
            policy_epoch: enabled.policy_epoch,
            hardware_epoch: self
                .ordinary
                .issuer_policy()
                .policy()
                .planned_hardware_epoch,
            issued_at_ms,
            expires_at_ms,
        };
        challenge.canonical_signing_bytes().map_err(|_| Rejected)?;
        Ok(challenge)
    }

    pub(super) fn preparation_expiry_at(&self, issued: u64) -> Result<u64> {
        self.trusted_time_ms()?;
        let enabled = self
            .governed
            .release()
            .enabled_profile(self.governed.profile_id())
            .ok_or(Rejected)?;
        issued
            .checked_add(
                self.ordinary
                    .issuer_policy()
                    .policy()
                    .maximum_pending_lifetime_ms,
            )
            .map(|end| {
                end.min(
                    self.ordinary
                        .identity_policy()
                        .policy()
                        .profile
                        .expires_at_ms,
                )
                .min(self.issuer.expires_at_ms)
                .min(enabled.hardware_profile.expires_at_ms)
            })
            .ok_or(Rejected)
    }

    // Bind the complete retained pre-key original to these independently selected originals.
    // This grants neither clock installation nor financial/issuer custody. A current interval
    // alone cannot lend authority from another valid account, release, profile or issuer.
    pub(super) fn require_prepared_original_scope(
        &self,
        prepared: &KagemushaPreparedOrdinaryAppEnrollmentV1,
    ) -> Result<()> {
        if prepared.ordinary != self.ordinary
            || prepared.issuer != self.issuer
            || prepared.owner != self.owner
            || prepared.release.release_id() != self.governed.release().release_id()
            || prepared.release.network_id() != self.governed.release().network_id()
        {
            return Err(Rejected);
        }
        let original = &prepared.preparation;
        let c = &original.challenge;
        let carrier = KagemushaOrdinaryPreparationCarrierV1 {
            account_i105: prepared
                .owner
                .account_id
                .canonical_i105()
                .map_err(|_| Rejected)?,
            client_nonce: c.client_nonce,
            release_id: c.release_id,
            hardware_profile_id: c.hardware_profile_id,
            lane_id: c.lane_id,
            financial_authority_commitment: c.financial_authority_commitment,
        };
        let expected = self.issuer_challenge_for_carrier(
            &carrier,
            c.server_nonce,
            c.issued_at_ms,
            c.expires_at_ms,
        )?;
        if expected != *c {
            return Err(Rejected);
        }
        original
            .authenticate(
                self.preparation_issuer_key()?,
                c,
                prepared.authenticated_at_ms,
            )
            .map_err(|_| Rejected)?;
        Ok(())
    }

    fn recheck_at_trusted_time(&self, now: u64) -> Result<()> {
        self.issuer.validate().map_err(|_| Rejected)?;
        self.governed.recheck()?;
        self.ordinary
            .require_owner_data(self.governed.release(), &self.owner, now)
            .map_err(|_| Rejected)?;
        if self.ordinary.retail_policy() != &self.issuer
            || self.ordinary.identity_policy().policy().trust != *self.governed.trust()
            || self.ordinary.identity_policy().policy().app_authority()
                != *self.governed.authority()
        {
            return Err(Rejected);
        }
        let enabled = self
            .governed
            .release()
            .enabled_profile(self.governed.profile_id())
            .ok_or(Rejected)?;
        if self.owner.runtime != self.issuer.runtime
            || self.owner.runtime.network_id != self.governed.release().network_id()
            || self.owner.lane_id == [0; 32]
            || self.owner.enrollment_id().map_err(|_| Rejected)? == [0; 32]
            || now == 0
            || now < self.issuer.valid_from_ms
            || now >= self.issuer.expires_at_ms
            || now < enabled.hardware_profile.valid_from_ms
            || now >= enabled.hardware_profile.expires_at_ms
        {
            return Err(Rejected);
        }
        Ok(())
    }
}

/// Exact public prepare carrier derived from an already fsynced private native reservation.
/// These fields are correlation data; they expose no financial secret or authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KagemushaOrdinaryPreparationCarrierV1 {
    /// Exact selected account in canonical I105 form.
    pub account_i105: String,
    /// Native client nonce, fixed before HTTP.
    pub client_nonce: [u8; 32],
    /// Actual threshold-authenticated release.
    pub release_id: [u8; 32],
    /// Actual selected ordinary profile.
    pub hardware_profile_id: [u8; 32],
    /// Original selected native financial lane.
    pub lane_id: [u8; 32],
    /// Commitment to the separate native financial secret, never the app key scalar.
    pub financial_authority_commitment: [u8; 32],
}

#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::ordinary_app_identity::PreparationReservationRecordV1")]
enum Record {
    Initialize {
        ticket: u64,
        enrollment_id: [u8; 32],
        account_i105: String,
        release_id: [u8; 32],
        profile_id: [u8; 32],
        lane_id: [u8; 32],
        issuer_policy_digest: [u8; 32],
        trust_policy_digest: [u8; 32],
        app_authority_policy_digest: [u8; 32],
        core_authorization_key_reference: [u8; 32],
        client_nonce: [u8; 32],
        financial_secret: [u8; 32],
        financial_authority_commitment: [u8; 32],
        hardware_epoch: u64,
        issued_at_ms: u64,
        expires_at_ms: u64,
    },
    Preparation {
        signed_original: Vec<u8>,
        admitted_at_ms: u64,
    },
    EnrollmentComplete {
        certificate_original: Vec<u8>,
        possession_original: Vec<u8>,
        authenticated_at_ms: u64,
        captured_at_ms: u64,
    },
}
impl Drop for Record {
    fn drop(&mut self) {
        if let Self::Initialize {
            financial_secret, ..
        } = self
        {
            financial_secret.zeroize();
        }
    }
}
fn encode(record: &Record) -> Result<Zeroizing<Vec<u8>>> {
    let bytes = norito::encode_canonical(record).map_err(|_| Rejected)?;
    if bytes.len() > FORMAT.maximum_payload_bytes as usize {
        return Err(Rejected);
    }
    Ok(Zeroizing::new(bytes))
}
fn decode(raw: &[u8]) -> Result<Record> {
    if raw.len() > FORMAT.maximum_payload_bytes as usize {
        return Err(Rejected);
    }
    let record: Record =
        norito::decode_canonical_with_limits(raw, norito::canonical_decode_limits(raw.len()))
            .map_err(|_| Rejected)?;
    if encode(&record)?.as_slice() != raw {
        return Err(Rejected);
    }
    Ok(record)
}

/// Actual descriptor-owned original reservation. No Clone, decoder or public secret intake.
/// Fresh construction uses the native RNG, and existing recovery never generates replacements.
pub struct KagemushaOrdinaryPreparationReservationV1 {
    selected: Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1>,
    journal: PrivateJournal,
    initialize: Zeroizing<Vec<u8>>,
    ticket: u64,
    carrier: KagemushaOrdinaryPreparationCarrierV1,
    secret: Zeroizing<[u8; 32]>,
    hardware_epoch: u64,
    original_issued_at_ms: u64,
    original_expires_at_ms: u64,
    #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
    reference_ms: u64,
    #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
    reference_clock: Reading,
    preparation: Option<(KagemushaSignedOrdinaryAppEnrollmentChallengeV1, u64)>,
    completed: Option<Zeroizing<Vec<u8>>>,
}
impl KagemushaOrdinaryPreparationReservationV1 {
    pub(super) fn selected_originals(
        &self,
    ) -> Result<&Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1>> {
        self.recheck_originals()?;
        Ok(&self.selected)
    }
    /// Reserve exactly one native secret/client nonce and fsync them before exposing a carrier.
    /// `root` and `selected` are native provisioning originals, never mobile path/response fields.
    /// # Errors
    /// Rejects existing/uncertain storage, invalid selected originals or RNG failure.
    pub fn create(
        root: &Path,
        selected: Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1>,
        now: u64,
    ) -> Result<Self> {
        let now = selected.time_for_native_or_fixture(now)?;
        #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
        let clock = Reading::now()?;
        let mut secret = Zeroizing::new([0; 32]);
        let mut client_nonce = [0; 32];
        let mut ticket_bytes = [0; 8];
        OsRng.try_fill_bytes(secret.as_mut()).map_err(|_| Custody)?;
        OsRng
            .try_fill_bytes(&mut client_nonce)
            .map_err(|_| Custody)?;
        OsRng
            .try_fill_bytes(&mut ticket_bytes)
            .map_err(|_| Custody)?;
        let ticket = u64::from_le_bytes(ticket_bytes);
        if *secret == [0; 32] || client_nonce == [0; 32] || ticket == 0 {
            return Err(Custody);
        }
        let commitment = crate::kagemusha_v1_recursion::device_authority_commitment_v1(*secret);
        let enrollment = selected.owner.enrollment_id().map_err(|_| Rejected)?;
        let carrier = KagemushaOrdinaryPreparationCarrierV1 {
            account_i105: selected
                .owner
                .account_id
                .canonical_i105()
                .map_err(|_| Rejected)?,
            client_nonce,
            release_id: selected.governed.release().release_id(),
            hardware_profile_id: selected.governed.profile_id(),
            lane_id: selected.owner.lane_id,
            financial_authority_commitment: commitment,
        };
        let expires = selected.preparation_expiry_at(now)?;
        let initialize = encode(&Record::Initialize {
            ticket,
            enrollment_id: enrollment,
            account_i105: carrier.account_i105.clone(),
            release_id: carrier.release_id,
            profile_id: carrier.hardware_profile_id,
            lane_id: carrier.lane_id,
            issuer_policy_digest: selected
                .ordinary
                .issuer_policy()
                .policy()
                .canonical_digest()
                .map_err(|_| Rejected)?,
            trust_policy_digest: selected
                .governed
                .trust()
                .canonical_digest()
                .map_err(|_| Rejected)?,
            app_authority_policy_digest: selected
                .governed
                .authority()
                .canonical_digest()
                .map_err(|_| Rejected)?,
            core_authorization_key_reference: selected.core_authorization_key_reference,
            client_nonce,
            financial_secret: *secret,
            financial_authority_commitment: commitment,
            hardware_epoch: selected
                .ordinary
                .issuer_policy()
                .policy()
                .planned_hardware_epoch,
            issued_at_ms: now,
            expires_at_ms: expires,
        })?;
        let mut journal = PrivateJournal::create_new(
            &root.join(format!("{}-preparation", hex::encode(enrollment))),
            FORMAT,
        )
        .map_err(|_| Custody)?;
        journal.append(&initialize).map_err(|_| Custody)?;
        let this = Self {
            selected,
            journal,
            initialize,
            ticket,
            carrier,
            secret,
            hardware_epoch: 1,
            original_issued_at_ms: now,
            original_expires_at_ms: expires,
            #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
            reference_ms: now,
            #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
            reference_clock: clock,
            preparation: None,
            completed: None,
        };
        this.recheck()?;
        Ok(this)
    }
    /// Reopen the exact existing native reservation under independently current originals/time.
    /// Missing, empty, torn, replaced or foreign storage never becomes a fresh reservation.
    /// # Errors
    /// Rejects any original secret/nonce/scope/sequence/interval substitution.
    pub fn open_existing(
        root: &Path,
        selected: Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1>,
        now: u64,
    ) -> Result<Self> {
        let this = Self::open_originals(root, selected, now)?;
        this.recheck()?;
        Ok(this)
    }
    /// Reopen only a previously retained signed C under current native Selected originals.
    /// The actual WAL admission time authenticates C; no nonce/secret/preparation is generated.
    /// # Errors
    /// Rejects absent signed preparation, foreign policy/secret/descriptor or corrupt completion.
    pub fn open_retained_originals(
        root: &Path,
        selected: Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1>,
        now: u64,
    ) -> Result<Self> {
        let this = Self::open_originals(root, selected, now)?;
        this.preparation.as_ref().ok_or(Custody)?;
        this.recheck_originals()?;
        Ok(this)
    }
    fn open_originals(
        root: &Path,
        selected: Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1>,
        now: u64,
    ) -> Result<Self> {
        let now = selected.time_for_native_or_fixture(now)?;
        let enrollment = selected.owner.enrollment_id().map_err(|_| Rejected)?;
        let mut journal = PrivateJournal::open_existing(
            &root.join(format!("{}-preparation", hex::encode(enrollment))),
            FORMAT,
        )
        .map_err(|_| Custody)?;
        let (_, raw) = journal.replay_next().map_err(|_| Custody)?.ok_or(Custody)?;
        let initialize = Zeroizing::new(raw);
        let record = decode(&initialize)?;
        let Record::Initialize {
            ticket,
            enrollment_id,
            account_i105,
            release_id,
            profile_id,
            lane_id,
            issuer_policy_digest,
            trust_policy_digest,
            app_authority_policy_digest,
            core_authorization_key_reference,
            client_nonce,
            financial_secret,
            financial_authority_commitment,
            hardware_epoch,
            issued_at_ms,
            expires_at_ms,
        } = &record
        else {
            return Err(Custody);
        };
        if *ticket == 0
            || *enrollment_id != enrollment
            || *account_i105
                != selected
                    .owner
                    .account_id
                    .canonical_i105()
                    .map_err(|_| Rejected)?
            || *release_id != selected.governed.release().release_id()
            || *profile_id != selected.governed.profile_id()
            || *lane_id != selected.owner.lane_id
            || *issuer_policy_digest
                != selected
                    .ordinary
                    .issuer_policy()
                    .policy()
                    .canonical_digest()
                    .map_err(|_| Rejected)?
            || *trust_policy_digest
                != selected
                    .governed
                    .trust()
                    .canonical_digest()
                    .map_err(|_| Rejected)?
            || *app_authority_policy_digest
                != selected
                    .governed
                    .authority()
                    .canonical_digest()
                    .map_err(|_| Rejected)?
            || *core_authorization_key_reference != selected.core_authorization_key_reference
            || *client_nonce == [0; 32]
            || *financial_secret == [0; 32]
            || *hardware_epoch
                != selected
                    .ordinary
                    .issuer_policy()
                    .policy()
                    .planned_hardware_epoch
            || *financial_authority_commitment
                != crate::kagemusha_v1_recursion::device_authority_commitment_v1(*financial_secret)
            || now < *issued_at_ms
            || *expires_at_ms <= *issued_at_ms
            || *expires_at_ms - *issued_at_ms
                > selected
                    .ordinary
                    .issuer_policy()
                    .policy()
                    .maximum_pending_lifetime_ms
            || *expires_at_ms
                > selected
                    .ordinary
                    .identity_policy()
                    .policy()
                    .profile
                    .expires_at_ms
        {
            return Err(Rejected);
        }
        let carrier = KagemushaOrdinaryPreparationCarrierV1 {
            account_i105: account_i105.clone(),
            client_nonce: *client_nonce,
            release_id: *release_id,
            hardware_profile_id: *profile_id,
            lane_id: *lane_id,
            financial_authority_commitment: *financial_authority_commitment,
        };
        let ticket = *ticket;
        let secret = Zeroizing::new(*financial_secret);
        let hardware_epoch = *hardware_epoch;
        let original_issued_at_ms = *issued_at_ms;
        let original_expires_at_ms = *expires_at_ms;
        let preparation = match journal.replay_next().map_err(|_| Custody)? {
            None => None,
            Some((_, raw)) => {
                let second = decode(&raw)?;
                let Record::Preparation {
                    signed_original,
                    admitted_at_ms,
                } = &second
                else {
                    return Err(Custody);
                };
                Some((
                    KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(
                        signed_original,
                    )
                    .map_err(|_| Rejected)?,
                    *admitted_at_ms,
                ))
            }
        };
        let completed = match journal.replay_next().map_err(|_| Custody)? {
            None => None,
            Some((_, raw)) => {
                if preparation.is_none()
                    || !matches!(decode(&raw)?, Record::EnrollmentComplete { .. })
                {
                    return Err(Custody);
                }
                Some(Zeroizing::new(raw))
            }
        };
        if journal.replay_next().map_err(|_| Custody)?.is_some() {
            return Err(Custody);
        }
        let this = Self {
            selected,
            journal,
            initialize,
            ticket,
            carrier,
            secret,
            hardware_epoch,
            original_issued_at_ms,
            original_expires_at_ms,
            #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
            reference_ms: now,
            #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
            reference_clock: Reading::now()?,
            preparation,
            completed,
        };
        this.recheck_originals()?;
        Ok(this)
    }
    /// Exact original ticket, only while the descriptor and original interval remain current.
    /// # Errors
    /// Rejects completed, expired or uncertain original custody.
    pub fn ticket(&self) -> Result<u64> {
        self.recheck()?;
        Ok(self.ticket)
    }
    /// Read the original fsynced six-field public prepare carrier without new randomness.
    /// # Errors
    /// Rejects completed, expired or uncertain original custody.
    pub fn carrier(&self) -> Result<&KagemushaOrdinaryPreparationCarrierV1> {
        self.recheck()?;
        Ok(&self.carrier)
    }
    fn interval(&self) -> Result<KagemushaOrdinaryNativeTimeIntervalV1> {
        #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
        if matches!(&self.selected.clock, SelectedClock::Fixture(_)) {
            let now = self
                .reference_ms
                .checked_add(Reading::now()?.elapsed_ms(self.reference_clock)?)
                .ok_or(Custody)?;
            self.selected.recheck_at_trusted_time(now)?;
            return Ok(KagemushaOrdinaryNativeTimeIntervalV1::fixture_point(now));
        }
        self.selected.trusted_time_interval()
    }
    fn now(&self) -> Result<u64> {
        self.interval().map(|interval| interval.lower_ms())
    }
    /// Recheck exact held Init/Preparation frames and suspend-inclusive original interval.
    /// # Errors
    /// Rejects path/inode/prefix drift, policy/secret mismatch or original expiry.
    pub fn recheck(&self) -> Result<()> {
        self.recheck_originals()?;
        let interval = self.interval()?;
        if self.completed.is_some() || interval.upper_ms() >= self.original_expires_at_ms {
            return Err(Custody);
        }
        if let Some((p, _)) = &self.preparation {
            interval.check_both(|now| self.prepared_from_original(p, now).map(|_| ()))?;
        }
        Ok(())
    }
    fn recheck_originals(&self) -> Result<()> {
        let interval = self.interval()?;
        let now = interval.lower_ms();
        if now < self.original_issued_at_ms {
            return Err(Custody);
        }
        self.recheck_retained_original_custody()
    }
    // Exact immutable proving custody only; no current clock or live validity is borrowed here.
    // Every live recheck above separately samples/validates the genuine current interval.
    fn recheck_retained_original_custody(&self) -> Result<()> {
        self.selected.governed.recheck()?;
        if *self.secret == [0; 32]
            || self.carrier.financial_authority_commitment
                != crate::kagemusha_v1_recursion::device_authority_commitment_v1(*self.secret)
        {
            return Err(Custody);
        }
        let preparation = self
            .preparation
            .as_ref()
            .map(|(p, t)| {
                encode(&Record::Preparation {
                    signed_original: p.to_transport_bytes().map_err(|_| Rejected)?,
                    admitted_at_ms: *t,
                })
            })
            .transpose()?;
        let mut count = 0;
        self.journal
            .scan_complete(|sequence, bytes| {
                let expected = match sequence {
                    0 => Some(self.initialize.as_slice()),
                    1 => preparation.as_ref().map(|b| b.as_slice()),
                    2 => self.completed.as_ref().map(|b| b.as_slice()),
                    _ => None,
                };
                if expected != Some(bytes) {
                    return Err(super::super::PrivateJournalError::Corrupt);
                }
                count += 1;
                Ok(())
            })
            .map_err(|_| Custody)?;
        if count != 1 + usize::from(preparation.is_some()) + usize::from(self.completed.is_some()) {
            return Err(Custody);
        }
        if let Some((p, admitted_at)) = &self.preparation {
            if *admitted_at < self.original_issued_at_ms
                || *admitted_at >= self.original_expires_at_ms
            {
                return Err(Custody);
            }
            self.prepared_from_original(p, *admitted_at)?;
        }
        Ok(())
    }
    fn prepared_from_original(
        &self,
        p: &KagemushaSignedOrdinaryAppEnrollmentChallengeV1,
        now: u64,
    ) -> Result<KagemushaPreparedOrdinaryAppEnrollmentV1> {
        let c = &p.challenge;
        if c.issued_at_ms < self.original_issued_at_ms || c.expires_at_ms <= c.issued_at_ms {
            return Err(Rejected);
        }
        KagemushaPreparedOrdinaryAppEnrollmentV1::authenticate_pre_key(
            p.clone(),
            self.selected.owner.clone(),
            self.selected.governed.release().clone(),
            self.selected.governed.trust().clone(),
            self.selected.governed.authority().clone(),
            self.selected.issuer.clone(),
            Arc::clone(&self.selected.ordinary),
            self.selected.governed.profile_id(),
            self.carrier.client_nonce,
            self.carrier.financial_authority_commitment,
            self.hardware_epoch,
            now,
        )
    }
    /// Authenticate and fsync exactly one signed C before exposing platform generation inputs.
    /// An identical retry reuses the retained C; another C or nonce never replaces it.
    /// # Errors
    /// Rejects missing native reservation, foreign signature/scope or uncertain append.
    pub fn retain_preparation(&mut self, raw: &[u8]) -> Result<()> {
        self.recheck()?;
        let offered = KagemushaSignedOrdinaryAppEnrollmentChallengeV1::from_transport_bytes(raw)
            .map_err(|_| Rejected)?;
        self.prepared_from_original(&offered, self.now()?)?;
        if let Some((old, _)) = &self.preparation {
            if old != &offered {
                return Err(Rejected);
            }
            return self.recheck();
        }
        let time = self.now()?;
        self.journal
            .append(&encode(&Record::Preparation {
                signed_original: raw.to_vec(),
                admitted_at_ms: time,
            })?)
            .map_err(|_| Custody)?;
        self.preparation = Some((offered, time));
        self.recheck()
    }
    /// Borrow the exact signed preparation only after its durable original admission.
    /// # Errors
    /// Rejects absent, completed or expired preparation custody.
    pub fn original_preparation(&self) -> Result<&KagemushaSignedOrdinaryAppEnrollmentChallengeV1> {
        self.recheck()?;
        Ok(&self.preparation.as_ref().ok_or(Rejected)?.0)
    }
    /// Derive the genuine pre-key owner from this same retained C/reservation, without replacements.
    /// # Errors
    /// Rejects absent, completed or expired preparation custody.
    pub fn prepared_owner(&self) -> Result<KagemushaPreparedOrdinaryAppEnrollmentV1> {
        self.recheck()?;
        let owner = self.retained_prepared_owner()?;
        self.interval()?
            .check_both(|now| owner.recheck_at_trusted_time(now))?;
        Ok(owner)
    }
    /// Read the original admitted C holder for completed platform-original recovery only.
    /// Its fresh signing/generation methods still require C's original short interval.
    /// # Errors
    /// Rejects absent signed C or changed current selected policy/private original custody.
    pub fn retained_prepared_owner(&self) -> Result<KagemushaPreparedOrdinaryAppEnrollmentV1> {
        self.recheck_originals()?;
        let (p, admitted_at) = self.preparation.as_ref().ok_or(Custody)?;
        let owner = self.prepared_from_original(p, *admitted_at)?;
        self.interval()?
            .check_both(|now| owner.recheck_retained_originals_at_trusted_time(now))?;
        Ok(owner)
    }
    /// Consume original financial custody only after genuine complete FI/wallet/platform admission.
    /// No signed credential alone or raw application archive can create the resulting owner.
    /// # Errors
    /// Rejects another enrollment, expired preparation or an uncertain durable completion.
    pub fn complete_enrollment(
        self,
        enrollment: Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>,
    ) -> Result<KagemushaOrdinaryEnrolledFinancialOwnerV1> {
        self.complete_enrollment_or_retain(enrollment)
            .map_err(|(_, error)| error)
    }
    /// Complete this original or retain its actual custody for exact recovery on failure.
    /// A completed WAL is matched to the identical FI original without another append.
    /// Poisoned storage remains held and must be reopened from its surviving original prefix.
    /// # Errors
    /// Returns the unchanged financial witness holder with the refusal; no new reservation is made.
    pub fn complete_enrollment_or_retain(
        mut self,
        enrollment: Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>,
    ) -> std::result::Result<
        KagemushaOrdinaryEnrolledFinancialOwnerV1,
        (Self, KagemushaOrdinaryIdentityErrorV1),
    > {
        let result = (|| {
            self.recheck_originals()?;
            let now = self.now()?;
            self.interval()?
                .check_both(|point| enrollment.recheck_at_trusted_time(point))
                .map_err(|_| Rejected)?;
            self.require_enrollment(&enrollment)?;
            if let Some(raw) = &self.completed {
                let completed = decode(raw)?;
                let Record::EnrollmentComplete {
                    certificate_original,
                    possession_original,
                    authenticated_at_ms,
                    captured_at_ms,
                } = &completed
                else {
                    return Err(Custody);
                };
                if certificate_original.as_slice()
                    != enrollment
                        .certificate()
                        .canonical_bytes()
                        .map_err(|_| Rejected)?
                    || possession_original.as_slice() != enrollment.possession().original()
                    || *authenticated_at_ms != enrollment.authenticated_at_ms()
                    || *captured_at_ms > now
                {
                    return Err(Custody);
                }
            } else {
                self.recheck()?;
                let completed = encode(&Record::EnrollmentComplete {
                    certificate_original: enrollment
                        .certificate()
                        .canonical_bytes()
                        .map_err(|_| Rejected)?,
                    possession_original: enrollment.possession().original().to_vec(),
                    authenticated_at_ms: enrollment.authenticated_at_ms(),
                    captured_at_ms: now,
                })?;
                self.journal.append(&completed).map_err(|_| Custody)?;
                self.completed = Some(completed);
            }
            self.recheck_originals()
        })();
        if let Err(error) = result {
            return Err((self, error));
        }
        let this = KagemushaOrdinaryEnrolledFinancialOwnerV1 {
            reservation: self,
            enrollment,
            integrity_lease: None,
        };
        if let Err(error) = this.recheck() {
            return Err((this.reservation, error));
        }
        Ok(this)
    }
    fn require_enrollment(
        &self,
        enrollment: &KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1,
    ) -> Result<()> {
        let certificate = enrollment.certificate();
        let app = enrollment.app_credential().subject();
        if certificate.subject.owner != self.selected.owner
            || certificate
                .subject
                .issuance
                .core_authorization_key_reference
                != self.selected.core_authorization_key_reference
            || enrollment.possession().challenge().preparation
                != self.preparation.as_ref().ok_or(Rejected)?.0
            || app.financial_authority_commitment != self.carrier.financial_authority_commitment
            || app.hardware_epoch != self.hardware_epoch
            || enrollment.authenticated_at_ms() < self.original_issued_at_ms
            || enrollment.authenticated_at_ms() >= self.original_expires_at_ms
        {
            return Err(Rejected);
        }
        Ok(())
    }
}

/// Genuine unchanged financial witness plus verified enrollment, retained for native publication.
/// This is still not a State proof, Guard or spend lease. Root publication consumes it separately.
pub struct KagemushaOrdinaryEnrolledFinancialOwnerV1 {
    reservation: KagemushaOrdinaryPreparationReservationV1,
    enrollment: Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>,
    integrity_lease: Option<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
}
impl KagemushaOrdinaryEnrolledFinancialOwnerV1 {
    /// Borrow the actual verified complete FI enrollment.
    pub fn enrollment(&self) -> &Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1> {
        &self.enrollment
    }
    /// Reopen only the completed exact three-frame custody under a genuine re-admitted enrollment.
    /// The original preparation is verified at its retained admission time, never renewed.
    /// # Errors
    /// Rejects missing completion, foreign proof/certificate, changed policy or expired current lease.
    pub fn open_existing(
        root: &Path,
        selected: Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1>,
        enrollment: Arc<KagemushaVerifiedOrdinaryRetailEnrollmentCertificateV1>,
        integrity_lease: Option<Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>>,
        now: u64,
    ) -> Result<Self> {
        let reservation =
            KagemushaOrdinaryPreparationReservationV1::open_originals(root, selected, now)?;
        let this = Self {
            reservation,
            enrollment,
            integrity_lease,
        };
        this.recheck()?;
        Ok(this)
    }
    /// Check unchanged complete custody and the current FI/credential or separate Integrity lease.
    /// The short enrollment preparation deadline grants no current financial authority.
    /// # Errors
    /// Rejects original WAL drift, another enrolled token or current interval/lease expiry.
    pub fn recheck(&self) -> Result<()> {
        self.reservation.recheck_originals()?;
        self.reservation.require_enrollment(&self.enrollment)?;
        let completion = decode(self.reservation.completed.as_ref().ok_or(Custody)?)?;
        let Record::EnrollmentComplete {
            certificate_original,
            possession_original,
            authenticated_at_ms,
            captured_at_ms,
        } = &completion
        else {
            return Err(Custody);
        };
        if *certificate_original
            != self
                .enrollment
                .certificate()
                .canonical_bytes()
                .map_err(|_| Rejected)?
            || *possession_original != self.enrollment.possession().original()
            || *authenticated_at_ms != self.enrollment.authenticated_at_ms()
            || *captured_at_ms < *authenticated_at_ms
            || *captured_at_ms >= self.reservation.original_expires_at_ms
        {
            return Err(Custody);
        }
        self.reservation
            .interval()?
            .check_both(|now| match &self.integrity_lease {
                Some(lease) => self.enrollment.recheck_with_integrity_lease(lease, now),
                None => self.enrollment.recheck_at_trusted_time(now),
            })
            .map_err(|_| Custody)
    }
    /// Private immutable proving custody from the completed original Native reservation.
    /// No current clock or FI grant is lent; all live effects must separately call `recheck`.
    pub(crate) fn recheck_historical_proof_custody(&self) -> Result<()> {
        self.reservation.recheck_retained_original_custody()?;
        self.reservation.require_enrollment(&self.enrollment)?;
        let completion = decode(self.reservation.completed.as_ref().ok_or(Custody)?)?;
        let Record::EnrollmentComplete {
            certificate_original,
            possession_original,
            authenticated_at_ms,
            captured_at_ms,
        } = &completion
        else {
            return Err(Custody);
        };
        if *certificate_original
            != self
                .enrollment
                .certificate()
                .canonical_bytes()
                .map_err(|_| Rejected)?
            || *possession_original != self.enrollment.possession().original()
            || *authenticated_at_ms != self.enrollment.authenticated_at_ms()
            || *captured_at_ms < *authenticated_at_ms
            || *captured_at_ms >= self.reservation.original_expires_at_ms
        {
            return Err(Custody);
        }
        // Original admission is retained in the exact completed WAL, never supplied by a caller.
        self.enrollment
            .recheck_at_trusted_time(*authenticated_at_ms)
            .map_err(|_| Custody)?;
        self.enrollment
            .possession()
            .recheck_at_trusted_time(*authenticated_at_ms)
            .map_err(|_| Custody)?;
        if let Some(lease) = &self.integrity_lease {
            self.enrollment
                .recheck_with_integrity_lease(lease, lease.authenticated_at_ms())
                .map_err(|_| Custody)?;
        }
        self.reservation.recheck_retained_original_custody()
    }
    /// Private commitment projection from the genuine held financial seed; no seed is exposed.
    pub(crate) fn historical_financial_authority_commitment(&self) -> Result<[u8; 32]> {
        self.recheck_historical_proof_custody()?;
        let commitment =
            crate::kagemusha_v1_recursion::device_authority_commitment_v1(*self.reservation.secret);
        if commitment
            != self
                .enrollment
                .app_credential()
                .subject()
                .financial_authority_commitment
        {
            return Err(Rejected);
        }
        self.recheck_historical_proof_custody()?;
        Ok(commitment)
    }
    /// Compare authenticated release identities without creating or lending another release.
    pub(crate) fn recheck_historical_release(
        &self,
        release: &Arc<KagemushaAuthenticatedReleaseV1>,
    ) -> Result<()> {
        self.recheck_historical_proof_custody()?;
        let actual = self.reservation.selected.governed.release();
        if actual.release_id() != release.release_id()
            || actual.manifest_digest() != release.manifest_digest()
            || actual.vk_set_digest() != release.vk_set_digest()
            || actual.hardware_policy_digest() != release.hardware_policy_digest()
            || actual.provider_policy_root() != release.provider_policy_root()
            || actual.network_id() != release.network_id()
        {
            return Err(Rejected);
        }
        Ok(())
    }

    /// Current same-owner Native interval; immutable historical C/E/FI times are not renewed.
    pub(crate) fn trusted_time_interval(&self) -> Result<KagemushaOrdinaryNativeTimeIntervalV1> {
        self.recheck()?;
        let interval = self.reservation.interval()?;
        interval
            .check_both(|now| match &self.integrity_lease {
                Some(lease) => self.enrollment.recheck_with_integrity_lease(lease, now),
                None => self.enrollment.recheck_at_trusted_time(now),
            })
            .map_err(|_| Custody)?;
        self.reservation.recheck_originals()?;
        Ok(interval)
    }
    /// Project the immutable original nonce/full signed-observation digest and actual sampled
    /// lower/upper clock bounds retained by this same Native financial owner. This data carrier
    /// supplies no new clock, FI, approval or monetary authority; callers cannot offer its fields.
    /// # Errors
    /// Refuses substituted/expired financial or clock custody, a fixture clock, changed original
    /// selection or either projected bound outside the same FI/C/current Integrity validity.
    pub(crate) fn current_cash_clock_context(
        &self,
    ) -> Result<iroha_data_model::kagemusha::KagemushaOrdinaryCashClockContextV1> {
        self.recheck()?;
        let selected = &self.reservation.selected;
        let context = match &selected.clock {
            SelectedClock::Native(clock) => {
                let mut clock = clock.lock().map_err(|_| Custody)?;
                if Some(clock.installed_selection_digest().map_err(|_| Custody)?)
                    != selected.clock_selection_digest
                {
                    return Err(Rejected);
                }
                clock.current_cash_clock_context().map_err(|_| Custody)?
            }
            #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
            SelectedClock::Fixture(_) => return Err(Rejected),
        };
        context.validate_shape().map_err(|_| Rejected)?;
        for point in [context.lower_at_ms, context.upper_at_ms] {
            selected.recheck_at_trusted_time(point)?;
            match &self.integrity_lease {
                Some(lease) => self.enrollment.recheck_with_integrity_lease(lease, point),
                None => self.enrollment.recheck_at_trusted_time(point),
            }
            .map_err(|_| Custody)?;
        }
        self.reservation.recheck_originals()?;
        self.recheck()?;
        Ok(context)
    }

    /// Private complete signed clock originals from the authentic same-owner WAL, selected by
    /// a context retained in actual Native cash custody. Historical originals lend no live clock.
    pub(crate) fn retained_cash_clock_originals(
        &self,
        context: &KagemushaOrdinaryCashClockContextV1,
    ) -> Result<super::super::KagemushaRetainedOrdinaryNativeClockOriginalsV1> {
        self.recheck_historical_proof_custody()?;
        let selected = &self.reservation.selected;
        let loan = match &selected.clock {
            SelectedClock::Native(clock) => {
                let clock = clock.lock().map_err(|_| Custody)?;
                if Some(clock.installed_selection_digest().map_err(|_| Custody)?)
                    != selected.clock_selection_digest
                {
                    return Err(Rejected);
                }
                clock
                    .retained_cash_clock_originals(context)
                    .map_err(|_| Custody)?
            }
            #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
            SelectedClock::Fixture(_) => return Err(Rejected),
        };
        self.recheck_historical_proof_custody()?;
        self.recheck_retained_cash_clock_originals(&loan)?;
        Ok(loan)
    }
    /// Authenticate an already private loan against the same actual clock and financial custody.
    /// This check never refreshes FI, Integrity or the sender's elapsed clock interval.
    pub(crate) fn recheck_retained_cash_clock_originals(
        &self,
        loan: &super::super::KagemushaRetainedOrdinaryNativeClockOriginalsV1,
    ) -> Result<()> {
        self.recheck_historical_proof_custody()?;
        match &self.reservation.selected.clock {
            SelectedClock::Native(clock) => {
                let clock = clock.lock().map_err(|_| Custody)?;
                if Some(clock.installed_selection_digest().map_err(|_| Custody)?)
                    != self.reservation.selected.clock_selection_digest
                {
                    return Err(Rejected);
                }
                loan.recheck(&clock).map_err(|_| Custody)?;
            }
            #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
            SelectedClock::Fixture(_) => return Err(Rejected),
        }
        self.recheck_historical_proof_custody()
    }

    /// Authenticate the same private retained clock original under actual installed custody.
    /// This lends signatures/finality for historical proving, without a current elapsed grant.
    pub(crate) fn verified_retained_cash_clock_originals(
        &self,
        context: &KagemushaOrdinaryCashClockContextV1,
    ) -> Result<super::super::KagemushaVerifiedOrdinaryNativeSignedClockOriginalV1> {
        let loan = self.retained_cash_clock_originals(context)?;
        self.recheck_retained_cash_clock_originals(&loan)?;
        let verified = match &self.reservation.selected.clock {
            SelectedClock::Native(clock) => {
                let clock = clock.lock().map_err(|_| Custody)?;
                if Some(clock.installed_selection_digest().map_err(|_| Custody)?)
                    != self.reservation.selected.clock_selection_digest
                {
                    return Err(Rejected);
                }
                loan.verified_original(&clock).map_err(|_| Custody)?
            }
            #[cfg(any(test, feature = "test-utils", feature = "kagemusha-real-proof-harness"))]
            SelectedClock::Fixture(_) => return Err(Rejected),
        };
        self.recheck_retained_cash_clock_originals(&loan)?;
        self.recheck_historical_proof_custody()?;
        Ok(verified)
    }

    /// Lower bound for immutable event timestamps; current validity must check both endpoints.
    pub(crate) fn trusted_time_ms(&self) -> Result<u64> {
        self.trusted_time_interval()
            .map(|interval| interval.lower_ms())
    }
    pub(crate) fn trusted_time_for_integrity_refresh(
        &self,
        lease: &KagemushaVerifiedPlayIntegrityRefreshLeaseV1,
    ) -> Result<u64> {
        self.reservation.recheck_originals()?;
        self.reservation.require_enrollment(&self.enrollment)?;
        let now = self.reservation.now()?;
        self.reservation
            .interval()?
            .check_both(|point| self.enrollment.recheck_with_integrity_lease(lease, point))
            .map_err(|_| Custody)?;
        self.reservation.recheck_originals()?;
        Ok(now)
    }
    pub(crate) fn retained_integrity_lease(
        &self,
    ) -> Option<&Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>> {
        self.integrity_lease.as_ref()
    }
    // The Root publication calls this only after the same Arc has been durably retained in its
    // actual logical journal. No public/client API can select a lease on this financial owner.
    pub(crate) fn select_verified_integrity_lease(
        &mut self,
        lease: Arc<KagemushaVerifiedPlayIntegrityRefreshLeaseV1>,
    ) -> Result<()> {
        self.reservation.recheck_originals()?;
        self.reservation.require_enrollment(&self.enrollment)?;
        self.reservation
            .interval()?
            .check_both(|now| self.enrollment.recheck_with_integrity_lease(&lease, now))
            .map_err(|_| Custody)?;
        self.integrity_lease = Some(lease);
        self.recheck()
    }
    pub(crate) fn financial_secret(&self) -> Result<&[u8; 32]> {
        self.recheck()?;
        Ok(&self.reservation.secret)
    }
    pub(crate) fn bootstrap_state_nonce_commitment(&self) -> Result<[u8; 32]> {
        use sha2::{Digest as _, Sha256};
        self.recheck()?;
        let mut hash = Sha256::new();
        hash.update(b"iroha:kagemusha:v1:ordinary-bootstrap-state-nonce\0");
        hash.update(self.reservation.secret.as_slice());
        hash.update(self.reservation.carrier.client_nonce);
        hash.update(self.enrollment.certificate().subject.enrollment_id);
        let commitment = hash.finalize().into();
        self.recheck()?;
        Ok(commitment)
    }
    pub(crate) fn with_borrowed_financial_secret(
        &self,
        selection: &crate::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
        consume: &mut dyn for<'secret> FnMut(&'secret [u8; 32]) -> Result<()>,
    ) -> Result<()> {
        self.recheck()?;
        if !std::ptr::eq(selection.enrollment(), self.enrollment.as_ref()) {
            return Err(Rejected);
        }
        self.reservation
            .interval()?
            .check_both(|now| selection.recheck_at_trusted_time(now))
            .map_err(|_| Custody)?;
        let result = consume(&self.reservation.secret);
        self.recheck()?;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, KeyPair, Signature, SignatureOf};
    use iroha_data_model::testing::ordinary_app_enrollment::KagemushaOrdinaryRetailEnrollmentFixtureV1 as Fixture;
    use p256::ecdsa::{Signature as P256Signature, SigningKey, signature::Signer as _};
    use sha2::{Digest as _, Sha256};

    fn core_key() -> KagemushaDevicePublicKeyV1 {
        let key = SigningKey::from_bytes((&[9; 32]).into()).unwrap();
        KagemushaDevicePublicKeyV1::from_sec1_bytes(
            key.verifying_key().to_encoded_point(false).as_bytes(),
        )
        .unwrap()
    }
    pub(super) fn selected(
        f: &Fixture,
        now: u64,
    ) -> Arc<KagemushaOrdinaryPreparationSelectedOriginalsV1> {
        Arc::new(
            KagemushaOrdinaryPreparationSelectedOriginalsV1::from_selected_originals(
                f.selection.owner.clone(),
                f.release.clone(),
                f.issuer_policy.clone(),
                Arc::clone(&f.ordinary_policy),
                f.trust.clone(),
                f.app_authority.clone(),
                f.selection.preparation.challenge.hardware_profile_id,
                &core_key(),
                now,
            )
            .unwrap(),
        )
    }
    fn governed(f: &Fixture) -> KagemushaOrdinaryGovernedPolicyOriginalsV1 {
        KagemushaOrdinaryGovernedPolicyOriginalsV1::authenticate(
            f.release.clone(),
            f.selection.preparation.challenge.hardware_profile_id,
            &norito::encode_canonical(&f.trust).unwrap(),
            &f.app_authority
                .canonical_digest_preimage_v1()
                .unwrap()
                .bytes,
        )
        .unwrap()
    }

    #[test]
    fn selected_retains_checked_originals_and_independent_key_identity() {
        let f = Fixture::android_with_integrity();
        let old_input_path = selected(&f, 300);
        let key = core_key();
        let new_input_path =
            KagemushaOrdinaryPreparationSelectedOriginalsV1::from_governed_originals(
                f.selection.owner.clone(),
                governed(&f),
                f.issuer_policy.clone(),
                Arc::clone(&f.ordinary_policy),
                &key,
                300,
            )
            .unwrap();
        assert_eq!(
            new_input_path.enrollment_id().unwrap(),
            old_input_path.enrollment_id().unwrap()
        );
        assert_eq!(
            new_input_path.integrity_policy_digest().unwrap(),
            old_input_path.integrity_policy_digest().unwrap()
        );
        assert_eq!(
            new_input_path.core_authorization_key_reference().unwrap(),
            kagemusha_core_authorization_key_reference_v1(&key)
        );
        assert_eq!(
            new_input_path.governed.original_trust_policy_bytes(),
            norito::encode_canonical(&f.trust).unwrap()
        );
        assert_eq!(
            new_input_path.governed.original_app_authority_bytes(),
            f.app_authority
                .canonical_digest_preimage_v1()
                .unwrap()
                .bytes
        );
    }

    #[test]
    fn governed_policy_admission_cannot_supply_runtime_lane_or_trusted_time() {
        let f = Fixture::new(true);
        for changed in 0..5 {
            let mut owner = f.selection.owner.clone();
            let mut issuer = f.issuer_policy.clone();
            let mut now = 300;
            match changed {
                0 => owner.runtime.scale += 1,
                1 => issuer.runtime.scale += 1,
                2 => owner.lane_id = [0; 32],
                3 => now = 0,
                _ => now = issuer.expires_at_ms,
            }
            assert!(
                KagemushaOrdinaryPreparationSelectedOriginalsV1::from_governed_originals(
                    owner,
                    governed(&f),
                    issuer,
                    Arc::clone(&f.ordinary_policy),
                    &core_key(),
                    now,
                )
                .is_err()
            );
        }
    }

    #[test]
    fn typed_selected_inputs_use_the_same_release_bound_policy_admission() {
        let f = Fixture::new(true);
        let mut trust = f.trust.clone();
        trust.maximum_credential_lifetime_ms -= 1;
        trust.validate().unwrap();
        assert!(
            KagemushaOrdinaryPreparationSelectedOriginalsV1::from_selected_originals(
                f.selection.owner.clone(),
                f.release.clone(),
                f.issuer_policy.clone(),
                Arc::clone(&f.ordinary_policy),
                trust,
                f.app_authority.clone(),
                f.selection.preparation.challenge.hardware_profile_id,
                &core_key(),
                300,
            )
            .is_err()
        );
    }

    pub(super) fn bind(mut f: Fixture, carrier: &KagemushaOrdinaryPreparationCarrierV1) -> Fixture {
        let issuer = KeyPair::from_seed(vec![61; 32], Algorithm::Ed25519);
        let core_issuer = KeyPair::from_seed(vec![63; 32], Algorithm::Ed25519);
        let fi_issuer = KeyPair::from_seed(vec![64; 32], Algorithm::Ed25519);
        let wallet = KeyPair::from_seed(vec![62; 32], Algorithm::Ed25519);
        let platform = SigningKey::from_bytes((&[7; 32]).into()).unwrap();
        f.selection.preparation.challenge.client_nonce = carrier.client_nonce;
        f.selection
            .preparation
            .challenge
            .financial_authority_commitment = carrier.financial_authority_commitment;
        f.selection.preparation.challenge.issued_at_ms = 300;
        let c = f.selection.preparation.challenge;
        f.selection.preparation.signature = Signature::try_new(
            core_issuer.private_key(),
            &c.canonical_signing_bytes().unwrap(),
        )
        .unwrap();
        let e = kagemusha_ordinary_app_enrollment_possession_message_v1(
            &c,
            &f.selection.issuance.credential.subject.app_public_key,
            Sha256::digest(&f.proof.raw_attestation).into(),
        )
        .unwrap();
        let signature: P256Signature = platform.sign(&e);
        let der = signature.to_der().as_bytes().to_vec();
        let a = &mut f.selection.issuance.credential.subject;
        a.client_nonce = c.client_nonce;
        a.financial_authority_commitment = c.financial_authority_commitment;
        a.enrollment_challenge_digest = c.attestation_challenge().unwrap();
        a.platform_evidence_digest =
            kagemusha_ordinary_app_enrollment_evidence_digest_v1(&f.proof.raw_attestation, &der)
                .unwrap();
        a.issued_at_ms = 400;
        let a = *a;
        f.selection.issuance.credential.signature =
            Signature::try_new(issuer.private_key(), &a.canonical_signing_bytes().unwrap())
                .unwrap();
        f.selection.issuance.credential.circuit_admission = iroha_data_model::testing::ordinary_app_enrollment::ordinary_test_issuer_admission_v1(
            iroha_data_model::kagemusha::KagemushaOrdinaryAppCredentialV1::circuit_admission_subject_for(&a, &f.selection.issuance.credential.signature).unwrap());
        f.selection.issuance.core_authorization_key_reference =
            kagemusha_core_authorization_key_reference_v1(&core_key());
        f.challenge.preparation = f.selection.preparation.clone();
        f.challenge.issuance = f.selection.issuance.clone();
        f.challenge.issued_at_ms = 450;
        f.proof.challenge = f.challenge.clone();
        f.proof.account_signature = SignatureOf::try_new(
            wallet.private_key(),
            &f.challenge.account_signing_payload().unwrap(),
        )
        .unwrap();
        f.proof.app_possession =
            KagemushaAppOperationApprovalEvidenceV1::AndroidKeystore { signature_der: der };
        let app = f
            .selection
            .issuance
            .credential
            .authenticate(
                f.ordinary_policy.identity_policy(),
                &f.checked_preparation().unwrap(),
                &a.app_public_key,
                600,
            )
            .unwrap();
        let possession = f
            .proof
            .authenticate(
                &f.challenge,
                &f.selection,
                &f.issuer_policy,
                &f.release,
                &app,
                None,
                600,
            )
            .unwrap();
        f.certificate.subject.issuance = f.selection.issuance.clone();
        f.certificate.subject.challenge_evidence_digest = possession.evidence_digest();
        f.certificate.subject.ordinary_app_credential_digest = app.digest();
        f.certificate.subject.issued_at_ms = 600;
        f.certificate.signature = SignatureOf::try_new(
            fi_issuer.private_key(),
            &f.certificate.subject.approval_payload().unwrap(),
        )
        .unwrap();
        f.verify(600).unwrap();
        f
    }
    #[test]
    fn ordinary_preparation_wal_recovers_same_secret_nonce_and_rejects_replacement() {
        let f = Fixture::new(false);
        let original = selected(&f, 300);
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut held =
            KagemushaOrdinaryPreparationReservationV1::create(&root, original.clone(), 300)
                .unwrap();
        let carrier = held.carrier().unwrap().clone();
        let ticket = held.ticket().unwrap();
        let secret = *held.secret;
        assert_eq!(
            carrier.financial_authority_commitment,
            crate::kagemusha_v1_recursion::device_authority_commitment_v1(secret)
        );
        let f = bind(f, &carrier);
        let raw = f.selection.preparation.to_transport_bytes().unwrap();
        held.retain_preparation(&raw).unwrap();
        held.retain_preparation(&raw).unwrap();
        let mut foreign = f.selection.preparation.clone();
        foreign.challenge.hardware_epoch += 1;
        assert!(
            held.retain_preparation(&foreign.to_transport_bytes().unwrap())
                .is_err()
        );
        drop(held);
        assert!(
            KagemushaOrdinaryPreparationReservationV1::create(&root, original.clone(), 300)
                .is_err()
        );
        let held =
            KagemushaOrdinaryPreparationReservationV1::open_existing(&root, original.clone(), 600)
                .unwrap();
        assert_eq!(held.ticket().unwrap(), ticket);
        assert_eq!(held.carrier().unwrap(), &carrier);
        assert_eq!(*held.secret, secret);
        assert_eq!(
            held.original_preparation().unwrap(),
            &f.selection.preparation
        );
        drop(held);
        assert!(
            KagemushaOrdinaryPreparationReservationV1::open_existing(&root, original, 2500)
                .is_err()
        );
    }
    #[test]
    fn ordinary_completed_custody_survives_preparation_expiry_without_new_witness() {
        let f = Fixture::new(false);
        let original = selected(&f, 300);
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let mut held =
            KagemushaOrdinaryPreparationReservationV1::create(&root, original.clone(), 300)
                .unwrap();
        let f = bind(f, held.carrier().unwrap());
        held.retain_preparation(&f.selection.preparation.to_transport_bytes().unwrap())
            .unwrap();
        let secret = *held.secret;
        // This test advances the private native trusted reference, never a mobile frame field.
        held.reference_ms = 600;
        held.reference_clock = Reading::now().unwrap();
        let enrolled = Arc::new(f.verify(600).unwrap());
        let completed = held.complete_enrollment(enrolled.clone()).unwrap();
        assert_eq!(*completed.financial_secret().unwrap(), secret);
        let state_nonce = completed.bootstrap_state_nonce_commitment().unwrap();
        assert_ne!(state_nonce, [0; 32]);
        let certificate_original = enrolled.certificate().canonical_bytes().unwrap();
        let wal_path = root
            .join(format!(
                "{}-preparation",
                hex::encode(original.owner.enrollment_id().unwrap())
            ))
            .join(FORMAT.filename);
        let wal_before = Zeroizing::new(std::fs::read(&wal_path).unwrap());
        assert!(completed.reservation.completed.is_some());
        let completed_prefix = completed.reservation.journal.recovery_prefix().unwrap();
        assert_eq!(completed_prefix.sequence, 3);
        let retry_at_ms = completed.reservation.now().unwrap();
        drop(completed);
        let retained = KagemushaOrdinaryPreparationReservationV1::open_retained_originals(
            &root,
            original.clone(),
            retry_at_ms,
        )
        .unwrap();
        assert!(retained.completed.is_some());
        assert_eq!(
            retained.journal.recovery_prefix().unwrap(),
            completed_prefix
        );
        let completed = retained
            .complete_enrollment_or_retain(enrolled.clone())
            .unwrap_or_else(|(_, error)| panic!("same enrollment retry refused: {error:?}"));
        assert_eq!(*completed.financial_secret().unwrap(), secret);
        assert_eq!(
            completed.bootstrap_state_nonce_commitment().unwrap(),
            state_nonce
        );
        assert!(Arc::ptr_eq(completed.enrollment(), &enrolled));
        assert_eq!(
            completed
                .enrollment()
                .certificate()
                .canonical_bytes()
                .unwrap(),
            certificate_original
        );
        assert_eq!(
            completed.reservation.journal.recovery_prefix().unwrap(),
            completed_prefix,
        );
        let wal_after = Zeroizing::new(std::fs::read(&wal_path).unwrap());
        assert!(
            wal_before.as_slice() == wal_after.as_slice(),
            "same enrollment retry appended or replaced the WAL"
        );
        drop(completed);
        assert!(
            KagemushaOrdinaryPreparationReservationV1::open_existing(&root, original.clone(), 2500)
                .is_err()
        );
        let completed_wal = Zeroizing::new(std::fs::read(&wal_path).unwrap());
        let foreign = Arc::new(Fixture::new(false).verify(300).unwrap());
        foreign.recheck_at_trusted_time(2500).unwrap();
        assert_ne!(
            foreign.certificate().canonical_bytes().unwrap(),
            enrolled.certificate().canonical_bytes().unwrap()
        );
        let held = KagemushaOrdinaryPreparationReservationV1::open_retained_originals(
            &root,
            original.clone(),
            2500,
        )
        .unwrap();
        let held = match held.complete_enrollment_or_retain(foreign) {
            Err((held, _)) => held,
            Ok(_) => panic!("foreign enrollment replaced completed reservation custody"),
        };
        assert_eq!(
            Zeroizing::new(std::fs::read(&wal_path).unwrap()).as_slice(),
            completed_wal.as_slice()
        );
        let completed = match held.complete_enrollment_or_retain(enrolled.clone()) {
            Ok(completed) => completed,
            Err((_, error)) => panic!("original completed enrollment refused recovery: {error:?}"),
        };
        assert_eq!(*completed.financial_secret().unwrap(), secret);
        assert_eq!(
            Zeroizing::new(std::fs::read(&wal_path).unwrap()).as_slice(),
            completed_wal.as_slice()
        );
        drop(completed);
        let completed = KagemushaOrdinaryEnrolledFinancialOwnerV1::open_existing(
            &root,
            original.clone(),
            enrolled,
            None,
            2500,
        )
        .unwrap();
        assert_eq!(*completed.financial_secret().unwrap(), secret);
        assert_eq!(
            completed.bootstrap_state_nonce_commitment().unwrap(),
            state_nonce
        );
        drop(completed);
        let foreign = Arc::new(Fixture::new(false).verify(300).unwrap());
        assert!(
            KagemushaOrdinaryEnrolledFinancialOwnerV1::open_existing(
                &root, original, foreign, None, 2500
            )
            .is_err()
        );
    }
}
