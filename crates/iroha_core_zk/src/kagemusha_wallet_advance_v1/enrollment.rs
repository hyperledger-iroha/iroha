//! Enrollment custody (spec §§2.2, 3.2, 4.2 step 3; G2 design rev 2 §6 E1-E8).
//!
//! An enrollment creates one slot per attempt and payment key:
//!
//! - **E2** the NOREPLACE capability probe (an unsupported filesystem refuses enrollment; there
//!   is no weaker fallback), the ballast, the slot directories, the iOS `NONE` anchor and the
//!   create-new intent holding the issuer challenge, all durable;
//! - **E3** the payment key is generated after definitive `Absent`, or once on a freshly
//!   sampled slot through the Native-owned fresh-generation grant on platforms which cannot
//!   establish absence. A loaded intent never grants the latter authority. Both paths bind
//!   the issuer challenge digest (Android attestation) and the hardware profile in the intent.
//!   A key found
//!   before generation, or reported as already present, abandons the slot: a key without a
//!   marker is a used incarnation and is never initialized (spec §§3.2, 4.2 step 3); the
//!   caller starts again with a new slot and alias;
//! - **E4** the generation-0 enrollment marker, carrying the slot's anchor kind from the
//!   intent, is made durable with the payment key before the credential request exists, and
//!   the iOS anchor is raised to it;
//! - **E5/E6** the credential request is retained create-new before it is sent, so every retry
//!   sends identical bytes, and the issued credential is stored create-new;
//! - **E7** Bootstrap is an ordinary `Advance` from the enrollment marker (exactly once: the
//!   generation-1 marker is created with NOREPLACE and competes with abandonment).
//!
//! An interrupted enrollment resumes the same slot: an intent whose key is still definitively
//! absent continues while the challenge is live and its policy permits resumption; an
//! interrupted fresh-only intent never generates again. A durable enrollment marker continues
//! at E5. Older intent formats are retained but refused; no implicit migration or reset occurs.
// TODO(G2-S): E8 activation uses the retained Bootstrap completion record as its request; the
// state owner archives the activation response.

use iroha_data_model::kagemusha::{
    KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1, KagemushaWalletEnrollmentChallengeV1,
    KagemushaWalletMarkerV1,
};

use super::{
    KagemushaWalletProviderErrorV1,
    advance::KagemushaWalletAdvanceCapsuleV1,
    anchor::{
        kagemusha_wallet_create_anchor_v1, kagemusha_wallet_raise_anchor_v1,
        kagemusha_wallet_require_anchor_policy_v1,
    },
    completion::KagemushaWalletCompletionFrameV1,
    decode_envelope_v1, encode_envelope_v1,
    layout::{
        KAGEMUSHA_WALLET_ABANDONED_NAME_V1, KAGEMUSHA_WALLET_ENROLLMENT_NAME_V1,
        KAGEMUSHA_WALLET_INTENT_NAME_V1, KAGEMUSHA_WALLET_KEY_GENERATION_ATTEMPT_NAME_V1,
        KagemushaWalletCustodyDirV1, KagemushaWalletEntryNameV1, KagemushaWalletSlotIdV1,
        kagemusha_wallet_credential_name_v1, kagemusha_wallet_fixed_name_v1,
        kagemusha_wallet_prepare_slot_dirs_v1, kagemusha_wallet_probe_noreplace_v1,
        kagemusha_wallet_require_published_v1, kagemusha_wallet_slot_dir_v1,
    },
    marker::{
        KagemushaWalletMarkerPublicationV1, KagemushaWalletMarkerRecordV1,
        kagemusha_wallet_publish_marker_v1,
    },
    platform::{
        KagemushaWalletAnchorPolicyV1, KagemushaWalletFsV1, KagemushaWalletKeyGenerationPolicyV1,
        KagemushaWalletKeyGenerationRequestV1, KagemushaWalletKeyGenerationV1,
        KagemushaWalletKeyProfileV1, KagemushaWalletNotPublishedV1, KagemushaWalletPlatformV1,
        KagemushaWalletProbeV1, KagemushaWalletPublishOutcomeV1, KagemushaWalletReadV1,
        KagemushaWalletUnavailableV1, kagemusha_wallet_boot_stamp_v1,
    },
    provider::{KagemushaWalletProviderV1, KagemushaWalletSlotStatusV1},
    reconcile::KagemushaWalletSlotAbandonReasonV1,
    store::KagemushaWalletDurableStoreV1,
};

mod dates_policy;

/// Exact original Core E1 dates; DATA retained before the original generation grant.
/// Core authenticates the ticket and enforces trusted time independently. These values
/// do not assert a hardware clock, a periodic lease or issuer authority.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core::zk::kagemusha_wallet_advance_v1::EnrollmentDatesV1")]
pub struct KagemushaWalletEnrollmentDatesV1 {
    /// Original Core issuance time; never refreshed on retry.
    pub issued_at_ms: u64,
    /// Original Core exclusive expiration bound.
    pub expires_at_ms: u64,
}
impl KagemushaWalletEnrollmentDatesV1 {
    /// Whether the original dates satisfy the current Core finite-ticket contract.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        dates_policy::valid(self.issued_at_ms, self.expires_at_ms)
    }
}

/// Version of the enrollment-phase records.
pub const KAGEMUSHA_WALLET_ENROLLMENT_FILE_VERSION_V1: u16 = 1;
/// Sole first-release intent codec including the exact creation policy.
pub const KAGEMUSHA_WALLET_INTENT_FILE_VERSION_V1: u16 = 1;
/// Maximum encoded intent or abandoned-slot record.
pub const KAGEMUSHA_WALLET_INTENT_MAX_BYTES_V1: usize = 1_024;
/// Maximum retained credential request.
pub const KAGEMUSHA_WALLET_ENROLLMENT_REQUEST_MAX_BYTES_V1: usize = 524_288;
/// Envelope overhead of the request and credential records.
const RECORD_OVERHEAD_BYTES: usize = 512;

/// One live, move-only authorization for a newly sampled enrollment slot.
///
/// Only the Native enrollment owner constructs this after its original create-new intent
/// and generation-attempt writes both report durable publication. It is not deserializable
/// or cloneable and cannot be reconstructed from a journal, caller DTO or alias. The borrow
/// keeps the exact platform owner alive until this authorization is consumed.
///
/// This is creation authority, not an absence verdict or a hardware anti-rollback claim.
pub struct KagemushaWalletFreshGenerationV1<'a> {
    owner: &'a dyn KagemushaWalletPlatformV1,
    slot: KagemushaWalletSlotIdV1,
    request: KagemushaWalletKeyGenerationRequestV1,
}

impl<'a> KagemushaWalletFreshGenerationV1<'a> {
    /// Called only from the original enrollment call after both create-new publications.
    fn new(
        owner: &'a dyn KagemushaWalletPlatformV1,
        slot: KagemushaWalletSlotIdV1,
        request: KagemushaWalletKeyGenerationRequestV1,
    ) -> Self {
        Self {
            owner,
            slot,
            request,
        }
    }

    /// Consume the grant on the exact platform owner which received it. Adapters must call
    /// this before crossing FFI, and must never retry or replace an occupied alias.
    ///
    /// # Errors
    /// Returns unavailable for another owner; the rejected grant is still consumed.
    pub fn consume(
        self,
        owner: &dyn KagemushaWalletPlatformV1,
    ) -> Result<
        (
            KagemushaWalletSlotIdV1,
            KagemushaWalletKeyGenerationRequestV1,
        ),
        KagemushaWalletUnavailableV1,
    > {
        if !std::ptr::addr_eq(self.owner, owner) {
            return Err(KagemushaWalletUnavailableV1::Platform(0));
        }
        Ok((self.slot, self.request))
    }
}

/// Durable enrollment intent of one slot (E2; create-new only).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core::zk::kagemusha_wallet_advance_v1::IntentV1")]
pub struct KagemushaWalletIntentV1 {
    /// Version; exactly [`KAGEMUSHA_WALLET_INTENT_FILE_VERSION_V1`].
    pub version: u16,
    /// Slot.
    pub slot: [u8; 32],
    /// Issuer challenge the enrollment marker binds; its digest is the key-attestation
    /// challenge.
    pub challenge: KagemushaWalletEnrollmentChallengeV1,
    /// Anchor kind ([`KagemushaWalletAnchorPolicyV1::tag`]): 0 none (Android), 1 keychain (iOS).
    pub anchor_kind: u8,
    /// Hardware key profile ([`KagemushaWalletKeyProfileV1::tag`]) of the issuer's enrollment
    /// policy; a resumed enrollment generates under the same profile. The key handle (alias or
    /// keychain account) is derived from the slot.
    pub profile: u8,
    /// Creation policy chosen at the original enrollment, never recomputed on resume.
    pub generation_policy: u8,
    /// Exact original Core dates published in this intent before generation.
    pub dates: KagemushaWalletEnrollmentDatesV1,
}

impl KagemushaWalletIntentV1 {
    /// Creation policy retained across process restarts and OS upgrades.
    ///
    /// # Errors
    /// `UnavailableCustodyData("intent")` for an unknown tag.
    pub fn key_generation_policy(
        &self,
    ) -> Result<KagemushaWalletKeyGenerationPolicyV1, KagemushaWalletProviderErrorV1> {
        KagemushaWalletKeyGenerationPolicyV1::from_tag(self.generation_policy)
            .ok_or(KagemushaWalletProviderErrorV1::UnavailableCustodyData { object: "intent" })
    }

    /// Anchor kind of the slot.
    ///
    /// # Errors
    ///
    /// `UnavailableCustodyData("intent")` for an unknown tag.
    pub fn anchor(&self) -> Result<KagemushaWalletAnchorPolicyV1, KagemushaWalletProviderErrorV1> {
        KagemushaWalletAnchorPolicyV1::from_tag(self.anchor_kind)
            .ok_or(KagemushaWalletProviderErrorV1::UnavailableCustodyData { object: "intent" })
    }

    /// Hardware key profile of the enrollment.
    ///
    /// # Errors
    ///
    /// `UnavailableCustodyData("intent")` for an unknown tag.
    pub fn key_profile(
        &self,
    ) -> Result<KagemushaWalletKeyProfileV1, KagemushaWalletProviderErrorV1> {
        KagemushaWalletKeyProfileV1::from_tag(self.profile)
            .ok_or(KagemushaWalletProviderErrorV1::UnavailableCustodyData { object: "intent" })
    }
}

/// Durable notice that the original fresh enrollment call consumed its generation attempt.
/// This is recovery data, never grant authority, absence proof, or an anti-rollback anchor.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core::zk::kagemusha_wallet_advance_v1::KeyGenerationAttemptV1")]
pub(super) struct KagemushaWalletKeyGenerationAttemptV1 {
    version: u16,
    slot: [u8; 32],
    challenge_digest: [u8; 32],
    profile: u8,
}

/// Abandoned-slot record (create-new only): the slot and its key are never used again.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_core::zk::kagemusha_wallet_advance_v1::AbandonedSlotV1")]
pub struct KagemushaWalletAbandonedSlotV1 {
    /// Version; exactly [`KAGEMUSHA_WALLET_ENROLLMENT_FILE_VERSION_V1`].
    pub version: u16,
    /// Slot.
    pub slot: [u8; 32],
    /// [`KagemushaWalletSlotAbandonReasonV1`] tag.
    pub reason: u8,
}

/// Retained credential request (E5; create-new only).
#[derive(Debug, Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_wallet_advance_v1::EnrollmentRecordV1")]
pub struct KagemushaWalletEnrollmentRecordV1 {
    /// Version; exactly [`KAGEMUSHA_WALLET_ENROLLMENT_FILE_VERSION_V1`].
    pub version: u16,
    /// Slot.
    pub slot: [u8; 32],
    /// G1 digest of the generation-0 marker the request follows.
    pub enrollment_marker_digest: [u8; 32],
    /// Exact request bytes; every retry sends them unchanged.
    pub request: Vec<u8>,
}

/// Stored credential (E6; create-new only).
#[derive(Debug, Clone, PartialEq, Eq, norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core::zk::kagemusha_wallet_advance_v1::CredentialRecordV1")]
pub struct KagemushaWalletCredentialRecordV1 {
    /// Version; exactly [`KAGEMUSHA_WALLET_ENROLLMENT_FILE_VERSION_V1`].
    pub version: u16,
    /// Slot.
    pub slot: [u8; 32],
    /// Credential index (0 at enrollment, then renewals).
    pub index: u32,
    /// Exact canonical credential bytes.
    pub credential: Vec<u8>,
}

/// Whether the issuer challenge of an interrupted enrollment is still usable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg(test)]
pub enum KagemushaWalletChallengeLivenessV1 {
    /// The challenge is live; enrollment may continue on the same slot.
    Live,
    /// The challenge expired; the slot is abandoned.
    Expired,
}

/// Result of an enrollment step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KagemushaWalletEnrollmentStepV1 {
    /// The generation-0 marker is durable; continue with the credential request (E5).
    Enrolled {
        /// Slot.
        slot: KagemushaWalletSlotIdV1,
        /// Enrollment marker.
        marker: Box<KagemushaWalletMarkerRecordV1>,
    },
    /// The slot was abandoned; start again with a new slot and alias.
    SlotAbandoned {
        /// Slot.
        slot: KagemushaWalletSlotIdV1,
    },
    /// A write's outcome is unknown; resume this slot after reconciliation.
    Pending {
        /// Slot.
        slot: KagemushaWalletSlotIdV1,
    },
}

/// Result of a create-new write that adopts an earlier attempt.
enum WriteOnceV1 {
    /// The name now durably holds the caller's bytes.
    Written,
    /// The name durably holds other bytes, which win.
    Existing(Vec<u8>),
}

/// Write `bytes` create-new; an existing file is adopted (rewritten to a fresh inode so an
/// earlier uncertain write becomes durable) and returned when it differs.
fn write_once<F: KagemushaWalletFsV1>(
    store: &KagemushaWalletDurableStoreV1<F>,
    dir: &KagemushaWalletCustodyDirV1,
    name: &KagemushaWalletEntryNameV1,
    bytes: &[u8],
    max: usize,
) -> Result<WriteOnceV1, KagemushaWalletProviderErrorV1> {
    match store.write_new(dir, name, bytes) {
        KagemushaWalletPublishOutcomeV1::NotPublished(
            KagemushaWalletNotPublishedV1::DestinationExists,
        ) => {
            let existing = match store.read(dir, name, max) {
                KagemushaWalletReadV1::Present(existing) => existing,
                KagemushaWalletReadV1::Unavailable(reason) => {
                    return Err(KagemushaWalletProviderErrorV1::Unavailable(reason));
                }
                KagemushaWalletReadV1::Absent => {
                    return Err(KagemushaWalletProviderErrorV1::Unavailable(
                        KagemushaWalletUnavailableV1::Busy,
                    ));
                }
                KagemushaWalletReadV1::Oversized => {
                    return Err(KagemushaWalletProviderErrorV1::UnavailableCustodyData {
                        object: "enrollment record",
                    });
                }
            };
            kagemusha_wallet_require_published_v1(store.rewrite_same(dir, name, &existing))?;
            Ok(if existing == bytes {
                WriteOnceV1::Written
            } else {
                WriteOnceV1::Existing(existing)
            })
        }
        outcome => kagemusha_wallet_require_published_v1(outcome).map(|()| WriteOnceV1::Written),
    }
}

/// Read and decode one enrollment-phase record; `None` only when definitively absent.
fn read_record<F, T>(
    store: &KagemushaWalletDurableStoreV1<F>,
    dir: &KagemushaWalletCustodyDirV1,
    name: &KagemushaWalletEntryNameV1,
    max: usize,
    object: &'static str,
) -> Result<Option<T>, KagemushaWalletProviderErrorV1>
where
    F: KagemushaWalletFsV1,
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    match store.read(dir, name, max) {
        KagemushaWalletReadV1::Present(bytes) => decode_envelope_v1(&bytes, max)
            .map(Some)
            .map_err(|_| KagemushaWalletProviderErrorV1::UnavailableCustodyData { object }),
        KagemushaWalletReadV1::Absent => Ok(None),
        KagemushaWalletReadV1::Oversized => {
            Err(KagemushaWalletProviderErrorV1::UnavailableCustodyData { object })
        }
        KagemushaWalletReadV1::Unavailable(reason) => {
            Err(KagemushaWalletProviderErrorV1::Unavailable(reason))
        }
    }
}

fn request_file_max() -> usize {
    KAGEMUSHA_WALLET_ENROLLMENT_REQUEST_MAX_BYTES_V1.saturating_add(RECORD_OVERHEAD_BYTES)
}

fn credential_file_max() -> usize {
    KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1.saturating_add(RECORD_OVERHEAD_BYTES)
}

impl<F, P, C, R> KagemushaWalletProviderV1<F, P, C, R>
where
    F: KagemushaWalletFsV1,
    P: KagemushaWalletPlatformV1,
    C: KagemushaWalletAdvanceCapsuleV1,
    R: KagemushaWalletCompletionFrameV1,
{
    /// Locate this exact issuer challenge before starting or retrying enrollment.
    /// An existing durable intent always resumes its original slot; it never grants a new
    /// fresh-only generation attempt. Unknown reads and duplicate intents refuse creation.
    /// # Errors
    /// Invalid challenge/profile binding, ambiguous retained intent, or provider unavailability.
    #[cfg(test)]
    pub(crate) fn test_begin_or_resume_enrollment(
        &mut self,
        challenge: &KagemushaWalletEnrollmentChallengeV1,
        profile: KagemushaWalletKeyProfileV1,
        dates: KagemushaWalletEnrollmentDatesV1,
    ) -> Result<KagemushaWalletEnrollmentStepV1, KagemushaWalletProviderErrorV1> {
        if !dates.is_valid() {
            return Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "enrollment.dates",
            });
        }
        challenge
            .validate()
            .map_err(|_| KagemushaWalletProviderErrorV1::Invalid { field: "challenge" })?;
        if challenge.scheme_id != self.scheme_id {
            return Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "challenge.scheme_id",
            });
        }
        if let Some(slot) = self.enrollment_slot(challenge, profile, dates)? {
            self.test_resume_enrollment(&slot, KagemushaWalletChallengeLivenessV1::Live)
        } else {
            self.test_begin_enrollment(challenge, profile, dates)
        }
    }

    /// Locate a unique durable original intent; never create a slot or generation grant.
    /// # Errors
    /// Unknown storage, changed original dates/profile, or an ambiguous intent.
    pub(crate) fn enrollment_slot(
        &mut self,
        challenge: &KagemushaWalletEnrollmentChallengeV1,
        profile: KagemushaWalletKeyProfileV1,
        dates: KagemushaWalletEnrollmentDatesV1,
    ) -> Result<Option<KagemushaWalletSlotIdV1>, KagemushaWalletProviderErrorV1> {
        if !dates.is_valid()
            || challenge.validate().is_err()
            || challenge.scheme_id != self.scheme_id
        {
            return Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "enrollment.originals",
            });
        }
        let mut selected = None;
        for slot in self.slots()? {
            let Some(intent) = self.read_intent(&slot)? else {
                // A missing intent is not permission to replace a surviving key or wallet.
                // Reconcile first, preserving unknown key/storage answers on older Android.
                match self.status(&slot)? {
                    KagemushaWalletSlotStatusV1::Empty
                    | KagemushaWalletSlotStatusV1::SlotAbandoned => continue,
                    _ => {
                        return Err(KagemushaWalletProviderErrorV1::UnavailableCustodyData {
                            object: "enrollment intent",
                        });
                    }
                }
            };

            if intent.challenge != *challenge {
                continue;
            }
            if intent.profile != profile.tag()
                || intent.dates != dates
                || selected.replace(slot).is_some()
            {
                return Err(KagemushaWalletProviderErrorV1::Invalid {
                    field: "enrollment.retained_intent",
                });
            }
        }
        Ok(selected)
    }

    /// Begin an enrollment under `challenge` (E2-E4) in a fresh slot, generating the payment
    /// key under the issuer's hardware key `profile`.
    ///
    /// # Errors
    ///
    /// `Invalid` for a challenge of another scheme; `NoReplaceUnsupported` when the custody
    /// filesystem lacks create-new rename (enrollment refused); `NoSpace` when the ballast
    /// or the capability probe does not fit; `Unavailable` when storage, the key store or the
    /// keychain gives no answer (on iOS also when no device passcode is set). An unknown write
    /// outcome after the intent exists is `Ok(Pending)`.
    pub(crate) fn begin_enrollment_checked(
        &mut self,
        authorization: crate::kagemusha_wallet_enrollment_v1::GenerationAuthorizationV1,
    ) -> Result<KagemushaWalletEnrollmentStepV1, KagemushaWalletProviderErrorV1> {
        authorization.check(self)?;
        let (challenge, profile, slot, generation_policy, fresh) = authorization.selection();
        if !fresh && generation_policy == KagemushaWalletKeyGenerationPolicyV1::FreshEnrollmentOnly
        {
            return Ok(KagemushaWalletEnrollmentStepV1::Pending { slot });
        }
        let dates = authorization.dates();
        if self.enrollment_slot(&challenge, profile, dates)?.is_some() {
            return Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "pre-key retained intent already exists",
            });
        }
        self.begin_enrollment_inner(
            &challenge,
            profile,
            dates,
            slot,
            generation_policy,
            |provider| authorization.check(provider),
        )
    }

    // Explicit provider simulator fixture. Production callers require the verified permit above.
    #[cfg(test)]
    pub(crate) fn test_begin_enrollment(
        &mut self,
        challenge: &KagemushaWalletEnrollmentChallengeV1,
        profile: KagemushaWalletKeyProfileV1,
        dates: KagemushaWalletEnrollmentDatesV1,
    ) -> Result<KagemushaWalletEnrollmentStepV1, KagemushaWalletProviderErrorV1> {
        let slot = KagemushaWalletSlotIdV1::generate()
            .map_err(KagemushaWalletProviderErrorV1::Unavailable)?;
        let policy = self
            .platform
            .key_generation_policy()
            .map_err(KagemushaWalletProviderErrorV1::Unavailable)?;
        self.begin_enrollment_inner(challenge, profile, dates, slot, policy, |_| Ok(()))
    }

    fn begin_enrollment_inner(
        &mut self,
        challenge: &KagemushaWalletEnrollmentChallengeV1,
        profile: KagemushaWalletKeyProfileV1,
        dates: KagemushaWalletEnrollmentDatesV1,
        slot: KagemushaWalletSlotIdV1,
        generation_policy: KagemushaWalletKeyGenerationPolicyV1,
        check: impl Fn(&Self) -> Result<(), KagemushaWalletProviderErrorV1>,
    ) -> Result<KagemushaWalletEnrollmentStepV1, KagemushaWalletProviderErrorV1> {
        if !dates.is_valid() {
            return Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "enrollment.dates",
            });
        }
        challenge
            .validate()
            .map_err(|_| KagemushaWalletProviderErrorV1::Invalid { field: "challenge" })?;
        if challenge.scheme_id != self.scheme_id {
            return Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "challenge.scheme_id",
            });
        }
        self.platform
            .storage_state()
            .map_err(KagemushaWalletProviderErrorV1::Unavailable)?;
        // E2.
        kagemusha_wallet_probe_noreplace_v1(&self.store)?;
        if !self.ballast_present()? && !self.write_ballast()? {
            return Err(KagemushaWalletProviderErrorV1::NoSpace);
        }
        if self
            .platform
            .key_generation_policy()
            .map_err(KagemushaWalletProviderErrorV1::Unavailable)?
            != generation_policy
        {
            return Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "pre-key creation policy changed",
            });
        }
        check(self)?;
        kagemusha_wallet_prepare_slot_dirs_v1(&self.store, &slot)?;
        kagemusha_wallet_create_anchor_v1(&self.platform, &slot)?;
        // The slot's anchor kind is fixed here, once; every marker of the slot carries it.
        let intent = KagemushaWalletIntentV1 {
            version: KAGEMUSHA_WALLET_INTENT_FILE_VERSION_V1,
            slot: slot.0,
            challenge: *challenge,
            anchor_kind: self.platform.anchor_policy().tag(),
            profile: profile.tag(),
            generation_policy: generation_policy.tag(),
            dates,
        };
        let bytes = encode_envelope_v1(&intent, KAGEMUSHA_WALLET_INTENT_MAX_BYTES_V1)?;
        match self.store.write_new(
            &kagemusha_wallet_slot_dir_v1(&slot),
            &kagemusha_wallet_fixed_name_v1(KAGEMUSHA_WALLET_INTENT_NAME_V1),
            &bytes,
        ) {
            KagemushaWalletPublishOutcomeV1::Published => {}
            KagemushaWalletPublishOutcomeV1::Uncertain(_) => {
                return Ok(KagemushaWalletEnrollmentStepV1::Pending { slot });
            }
            outcome => kagemusha_wallet_require_published_v1(outcome)?,
        }
        if generation_policy == KagemushaWalletKeyGenerationPolicyV1::FreshEnrollmentOnly {
            // This branch is reachable only from the original CSPRNG slot and original
            // write_new(Published) above. Neither resume nor a loaded attempt can enter it.
            self.require_storage()?;
            kagemusha_wallet_require_anchor_policy_v1(&self.platform, intent.anchor()?)?;
            let request = KagemushaWalletKeyGenerationRequestV1 {
                challenge_digest: intent.challenge.challenge_digest(),
                profile,
            };
            let attempt = KagemushaWalletKeyGenerationAttemptV1 {
                version: KAGEMUSHA_WALLET_ENROLLMENT_FILE_VERSION_V1,
                slot: slot.0,
                challenge_digest: request.challenge_digest,
                profile: profile.tag(),
            };
            let bytes = encode_envelope_v1(&attempt, KAGEMUSHA_WALLET_INTENT_MAX_BYTES_V1)?;
            match self.store.write_new(
                &kagemusha_wallet_slot_dir_v1(&slot),
                &kagemusha_wallet_fixed_name_v1(KAGEMUSHA_WALLET_KEY_GENERATION_ATTEMPT_NAME_V1),
                &bytes,
            ) {
                KagemushaWalletPublishOutcomeV1::Published => {}
                KagemushaWalletPublishOutcomeV1::Uncertain(_) => {
                    return Ok(KagemushaWalletEnrollmentStepV1::Pending { slot });
                }
                outcome => kagemusha_wallet_require_published_v1(outcome)?,
            }
            self.require_storage()?;
            check(self)?;
            let grant = KagemushaWalletFreshGenerationV1::new(&self.platform, slot, request);
            let generated = self.platform.key_generate_fresh(grant);
            // A storage error after generation leaves this attempt consumed and uncertain.
            self.require_storage()?;
            return self.finish_enrollment(&slot, &intent, generated);
        }
        self.continue_enrollment(&slot, &intent, check)
    }

    pub(crate) fn resume_enrollment_checked(
        &mut self,
        authorization: crate::kagemusha_wallet_enrollment_v1::GenerationAuthorizationV1,
    ) -> Result<KagemushaWalletEnrollmentStepV1, KagemushaWalletProviderErrorV1> {
        authorization.check(self)?;
        let (challenge, profile, slot, generation_policy, _) = authorization.selection();
        let intent = self.read_intent(&slot)?.ok_or(
            KagemushaWalletProviderErrorV1::UnavailableCustodyData {
                object: "pre-key selected intent",
            },
        )?;
        if intent.challenge != challenge
            || intent.key_profile()? != profile
            || intent.key_generation_policy()? != generation_policy
            || intent.dates != authorization.dates()
        {
            return Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "pre-key retained intent",
            });
        }
        self.resume_enrollment_inner(&slot, false, |provider| authorization.check(provider))
    }

    /// Resume an interrupted enrollment of `slot` (design E2a-E4b).
    ///
    /// # Errors
    ///
    /// `Invalid` for a slot without an intent or whose enrollment already advanced past the
    /// enrollment marker, and the reconcile and enrollment errors.
    #[cfg(test)]
    pub(crate) fn test_resume_enrollment(
        &mut self,
        slot: &KagemushaWalletSlotIdV1,
        liveness: KagemushaWalletChallengeLivenessV1,
    ) -> Result<KagemushaWalletEnrollmentStepV1, KagemushaWalletProviderErrorV1> {
        self.resume_enrollment_inner(
            slot,
            liveness == KagemushaWalletChallengeLivenessV1::Expired,
            |_| Ok(()),
        )
    }

    fn resume_enrollment_inner(
        &mut self,
        slot: &KagemushaWalletSlotIdV1,
        expired: bool,
        check: impl Fn(&Self) -> Result<(), KagemushaWalletProviderErrorV1>,
    ) -> Result<KagemushaWalletEnrollmentStepV1, KagemushaWalletProviderErrorV1> {
        match self.reconcile_slot(slot, None)? {
            KagemushaWalletSlotStatusV1::IntentOnly => {
                if expired {
                    self.abandon_slot(slot, KagemushaWalletSlotAbandonReasonV1::ChallengeExpired)?;
                    return Ok(KagemushaWalletEnrollmentStepV1::SlotAbandoned { slot: *slot });
                }
                let intent =
                    self.read_intent(slot)?
                        .ok_or(KagemushaWalletProviderErrorV1::Unavailable(
                            KagemushaWalletUnavailableV1::Busy,
                        ))?;
                if intent.key_generation_policy()?
                    == KagemushaWalletKeyGenerationPolicyV1::FreshEnrollmentOnly
                {
                    // A fresh generation grant cannot be recreated, even when an OS upgrade
                    // can now report definitive absence. The caller may explicitly request
                    // another enrollment with a new slot; no key or funded marker is reset.
                    return Ok(KagemushaWalletEnrollmentStepV1::Pending { slot: *slot });
                }
                // The intent may come from an uncertain write of this boot: make it durable
                // before a key is generated under it.
                let bytes = encode_envelope_v1(&intent, KAGEMUSHA_WALLET_INTENT_MAX_BYTES_V1)?;
                let adopted = kagemusha_wallet_require_published_v1(self.store.rewrite_same(
                    &kagemusha_wallet_slot_dir_v1(slot),
                    &kagemusha_wallet_fixed_name_v1(KAGEMUSHA_WALLET_INTENT_NAME_V1),
                    &bytes,
                ));
                self.guard(slot, adopted)?;
                self.continue_enrollment(slot, &intent, check)
            }
            KagemushaWalletSlotStatusV1::SlotAbandoned => {
                Ok(KagemushaWalletEnrollmentStepV1::SlotAbandoned { slot: *slot })
            }
            KagemushaWalletSlotStatusV1::Enrollment(marker) => {
                Ok(KagemushaWalletEnrollmentStepV1::Enrolled {
                    slot: *slot,
                    marker: Box::new(marker),
                })
            }
            KagemushaWalletSlotStatusV1::Empty => Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "slot.no_intent",
            }),
            KagemushaWalletSlotStatusV1::Pending(_)
            | KagemushaWalletSlotStatusV1::Released(_)
            | KagemushaWalletSlotStatusV1::Terminal(_) => {
                Err(KagemushaWalletProviderErrorV1::Invalid {
                    field: "enrollment.finished",
                })
            }
        }
    }

    /// E3-E4 on a slot whose intent is durable and which has no marker and is not abandoned.
    fn continue_enrollment(
        &mut self,
        slot: &KagemushaWalletSlotIdV1,
        intent: &KagemushaWalletIntentV1,
        check: impl Fn(&Self) -> Result<(), KagemushaWalletProviderErrorV1>,
    ) -> Result<KagemushaWalletEnrollmentStepV1, KagemushaWalletProviderErrorV1> {
        if intent.key_generation_policy()?
            != KagemushaWalletKeyGenerationPolicyV1::DefinitiveAbsence
        {
            return Ok(KagemushaWalletEnrollmentStepV1::Pending { slot: *slot });
        }
        let anchor = intent.anchor()?;
        let request = KagemushaWalletKeyGenerationRequestV1 {
            challenge_digest: intent.challenge.challenge_digest(),
            profile: intent.key_profile()?,
        };
        // The platform must still keep the anchor kind this slot was created with.
        kagemusha_wallet_require_anchor_policy_v1(&self.platform, anchor)?;
        // E3: generate only after a definitive absence; never reuse a key without a marker.
        match self.probe_key(slot)? {
            KagemushaWalletProbeV1::Absent => {}
            KagemushaWalletProbeV1::Present(_) => {
                self.abandon_slot(slot, KagemushaWalletSlotAbandonReasonV1::KeyWithoutMarker)?;
                return Ok(KagemushaWalletEnrollmentStepV1::SlotAbandoned { slot: *slot });
            }
            KagemushaWalletProbeV1::Unavailable(reason) => {
                return Err(KagemushaWalletProviderErrorV1::Unavailable(reason));
            }
        }
        check(self)?;
        let generated = self.platform.key_generate(slot, &request);
        self.finish_enrollment(slot, intent, generated)
    }

    /// E4 after exactly one platform generation result. This never generates or retries.
    fn finish_enrollment(
        &mut self,
        slot: &KagemushaWalletSlotIdV1,
        intent: &KagemushaWalletIntentV1,
        generated: KagemushaWalletKeyGenerationV1,
    ) -> Result<KagemushaWalletEnrollmentStepV1, KagemushaWalletProviderErrorV1> {
        let anchor = intent.anchor()?;
        let payment_key = match generated {
            KagemushaWalletKeyGenerationV1::Generated(payment_key) => payment_key,
            KagemushaWalletKeyGenerationV1::AlreadyPresent => {
                self.abandon_slot(slot, KagemushaWalletSlotAbandonReasonV1::KeyWithoutMarker)?;
                return Ok(KagemushaWalletEnrollmentStepV1::SlotAbandoned { slot: *slot });
            }
            KagemushaWalletKeyGenerationV1::Unavailable(reason) => {
                return Err(KagemushaWalletProviderErrorV1::Unavailable(reason));
            }
        };
        // E4.
        let marker =
            KagemushaWalletMarkerV1::enrollment(&intent.challenge, payment_key).map_err(|_| {
                KagemushaWalletProviderErrorV1::Invalid {
                    field: "enrollment.marker",
                }
            })?;
        let record = KagemushaWalletMarkerRecordV1::new(
            *slot,
            marker,
            anchor,
            None,
            kagemusha_wallet_boot_stamp_v1(&self.boot()),
        )?;
        let published = kagemusha_wallet_publish_marker_v1(&self.store, record);
        let durable = match self.guard(slot, published) {
            Ok(KagemushaWalletMarkerPublicationV1::Durable(durable)) => durable,
            Ok(KagemushaWalletMarkerPublicationV1::GenerationTaken) => {
                self.poison(slot);
                return Err(KagemushaWalletProviderErrorV1::Unavailable(
                    KagemushaWalletUnavailableV1::Busy,
                ));
            }
            Err(KagemushaWalletProviderErrorV1::Uncertain(_)) => {
                return Ok(KagemushaWalletEnrollmentStepV1::Pending { slot: *slot });
            }
            Err(error) => return Err(error),
        };
        match kagemusha_wallet_raise_anchor_v1(&self.platform, durable.record()) {
            Ok(()) => {}
            Err(KagemushaWalletProviderErrorV1::Uncertain(_)) => {
                self.poison(slot);
                return Ok(KagemushaWalletEnrollmentStepV1::Pending { slot: *slot });
            }
            Err(error) => {
                self.poison(slot);
                return Err(error);
            }
        }
        let record = durable.record().clone();
        self.remember(
            slot,
            durable,
            KagemushaWalletSlotStatusV1::Enrollment(record.clone()),
        );
        Ok(KagemushaWalletEnrollmentStepV1::Enrolled {
            slot: *slot,
            marker: Box::new(record),
        })
    }

    /// Retain the credential request of `slot` before it is sent (E5) and return the exact
    /// bytes to send: an earlier retained request wins over `request`.
    ///
    /// # Errors
    ///
    /// `Invalid` for an empty or oversized request or when no request is retained and the
    /// enrollment marker is not current; the reconcile, read and write errors otherwise.
    pub fn retain_enrollment_request(
        &mut self,
        slot: &KagemushaWalletSlotIdV1,
        request: &[u8],
    ) -> Result<Vec<u8>, KagemushaWalletProviderErrorV1> {
        if request.is_empty() || request.len() > KAGEMUSHA_WALLET_ENROLLMENT_REQUEST_MAX_BYTES_V1 {
            return Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "enrollment.request",
            });
        }
        let status = self.reconcile_slot(slot, None)?;
        if let Some(existing) = self.adopt_enrollment_record(slot)? {
            return Ok(existing.request);
        }
        let KagemushaWalletSlotStatusV1::Enrollment(marker) = status else {
            return Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "enrollment.phase",
            });
        };
        let record = KagemushaWalletEnrollmentRecordV1 {
            version: KAGEMUSHA_WALLET_ENROLLMENT_FILE_VERSION_V1,
            slot: slot.0,
            enrollment_marker_digest: *marker.marker_digest(),
            request: request.to_vec(),
        };
        let bytes = encode_envelope_v1(&record, request_file_max())?;
        let written = write_once(
            &self.store,
            &kagemusha_wallet_slot_dir_v1(slot),
            &kagemusha_wallet_fixed_name_v1(KAGEMUSHA_WALLET_ENROLLMENT_NAME_V1),
            &bytes,
            request_file_max(),
        );
        match self.guard(slot, written)? {
            WriteOnceV1::Written => Ok(record.request),
            WriteOnceV1::Existing(existing) => decode_envelope_v1::<
                KagemushaWalletEnrollmentRecordV1,
            >(&existing, request_file_max())
            .map(|existing| existing.request)
            .map_err(|_| KagemushaWalletProviderErrorV1::UnavailableCustodyData {
                object: "enrollment record",
            }),
        }
    }

    /// Recover an earlier original request through the same guarded durable adoption
    /// path used before its first send. A readable uncertain publication is not enough.
    ///
    /// # Errors
    /// The original read, reconciliation, guarded rewrite and publication errors.
    pub fn recover_enrollment_request(
        &mut self,
        slot: &KagemushaWalletSlotIdV1,
    ) -> Result<Option<Vec<u8>>, KagemushaWalletProviderErrorV1> {
        let Some(record) = self.enrollment_record(slot)? else {
            return Ok(None);
        };
        self.retain_enrollment_request(slot, &record.request)
            .map(Some)
    }

    /// Retained credential request of `slot`, read without side effects.
    ///
    /// # Errors
    ///
    /// `Unavailable` on a read error and `UnavailableCustodyData` for an invalid record.
    pub fn enrollment_record(
        &self,
        slot: &KagemushaWalletSlotIdV1,
    ) -> Result<Option<KagemushaWalletEnrollmentRecordV1>, KagemushaWalletProviderErrorV1> {
        let record = read_record::<F, KagemushaWalletEnrollmentRecordV1>(
            &self.store,
            &kagemusha_wallet_slot_dir_v1(slot),
            &kagemusha_wallet_fixed_name_v1(KAGEMUSHA_WALLET_ENROLLMENT_NAME_V1),
            request_file_max(),
            "enrollment record",
        )?;
        match record {
            Some(record)
                if record.version != KAGEMUSHA_WALLET_ENROLLMENT_FILE_VERSION_V1
                    || record.slot != slot.0 =>
            {
                Err(KagemushaWalletProviderErrorV1::UnavailableCustodyData {
                    object: "enrollment record",
                })
            }
            record => Ok(record),
        }
    }

    /// Retained credential request of `slot`, made durable before it is (re)sent: a record
    /// from an earlier uncertain write is rewritten to a fresh inode under the slot guard, so
    /// an unknown outcome poisons the slot.
    fn adopt_enrollment_record(
        &mut self,
        slot: &KagemushaWalletSlotIdV1,
    ) -> Result<Option<KagemushaWalletEnrollmentRecordV1>, KagemushaWalletProviderErrorV1> {
        let Some(record) = self.enrollment_record(slot)? else {
            return Ok(None);
        };
        let bytes = encode_envelope_v1(&record, request_file_max())?;
        let adopted = kagemusha_wallet_require_published_v1(self.store.rewrite_same(
            &kagemusha_wallet_slot_dir_v1(slot),
            &kagemusha_wallet_fixed_name_v1(KAGEMUSHA_WALLET_ENROLLMENT_NAME_V1),
            &bytes,
        ));
        self.guard(slot, adopted)?;
        Ok(Some(record))
    }

    /// Store credential `index` of `slot` (E6). Storing identical bytes again succeeds.
    ///
    /// # Errors
    ///
    /// `Invalid` for empty or oversized bytes, before the request is retained, for a terminal
    /// or unenrolled slot, and for bytes that differ from the stored credential; the reconcile,
    /// read and write errors otherwise.
    pub fn store_credential(
        &mut self,
        slot: &KagemushaWalletSlotIdV1,
        index: u32,
        credential: &[u8],
    ) -> Result<(), KagemushaWalletProviderErrorV1> {
        if credential.is_empty() || credential.len() > KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1 {
            return Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "credential",
            });
        }
        match self.reconcile_slot(slot, None)? {
            KagemushaWalletSlotStatusV1::Enrollment(_)
            | KagemushaWalletSlotStatusV1::Pending(_)
            | KagemushaWalletSlotStatusV1::Released(_) => {}
            KagemushaWalletSlotStatusV1::Terminal(_) => {
                return Err(KagemushaWalletProviderErrorV1::Terminal);
            }
            _ => {
                return Err(KagemushaWalletProviderErrorV1::Invalid {
                    field: "slot.not_enrolled",
                });
            }
        }
        if self.adopt_enrollment_record(slot)?.is_none() {
            return Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "credential.before_request",
            });
        }
        let record = KagemushaWalletCredentialRecordV1 {
            version: KAGEMUSHA_WALLET_ENROLLMENT_FILE_VERSION_V1,
            slot: slot.0,
            index,
            credential: credential.to_vec(),
        };
        let bytes = encode_envelope_v1(&record, credential_file_max())?;
        let written = write_once(
            &self.store,
            &kagemusha_wallet_slot_dir_v1(slot),
            &kagemusha_wallet_credential_name_v1(index),
            &bytes,
            credential_file_max(),
        );
        match self.guard(slot, written)? {
            WriteOnceV1::Written => Ok(()),
            WriteOnceV1::Existing(_) => Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "credential.differs",
            }),
        }
    }

    /// Stored credential `index` of `slot`.
    ///
    /// # Errors
    ///
    /// `Unavailable` on a read error and `UnavailableCustodyData` for an invalid record.
    pub fn credential(
        &self,
        slot: &KagemushaWalletSlotIdV1,
        index: u32,
    ) -> Result<Option<Vec<u8>>, KagemushaWalletProviderErrorV1> {
        let record = read_record::<F, KagemushaWalletCredentialRecordV1>(
            &self.store,
            &kagemusha_wallet_slot_dir_v1(slot),
            &kagemusha_wallet_credential_name_v1(index),
            credential_file_max(),
            "credential",
        )?;
        match record {
            Some(record)
                if record.version == KAGEMUSHA_WALLET_ENROLLMENT_FILE_VERSION_V1
                    && record.slot == slot.0
                    && record.index == index =>
            {
                Ok(Some(record.credential))
            }
            Some(_) => Err(KagemushaWalletProviderErrorV1::UnavailableCustodyData {
                object: "credential",
            }),
            None => Ok(None),
        }
    }

    /// Read the actual enrolled key's leaf-first DER chain without exporting its custody.
    /// The Core evidence verifier, rather than this DATA read, authenticates the certificates.
    /// # Errors
    /// A non-enrollment marker, changed key, unavailable platform or invalid chain extent.
    pub fn enrollment_attestation_chain(
        &mut self,
        slot: &KagemushaWalletSlotIdV1,
    ) -> Result<Vec<Vec<u8>>, KagemushaWalletProviderErrorV1> {
        let KagemushaWalletSlotStatusV1::Enrollment(marker) = self.status(slot)? else {
            return Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "enrollment.marker",
            });
        };
        match self.platform.key_probe(slot) {
            KagemushaWalletProbeV1::Present(key) if key == *marker.payment_key() => {}
            KagemushaWalletProbeV1::Unavailable(reason) => {
                return Err(KagemushaWalletProviderErrorV1::Unavailable(reason));
            }
            _ => return Err(KagemushaWalletProviderErrorV1::KeyLost),
        }
        let chain = self
            .platform
            .key_attestation_chain(slot)
            .into_result()?
            .ok_or(KagemushaWalletProviderErrorV1::KeyLost)?;
        if !(2..=8).contains(&chain.len())
            || chain.iter().any(|der| der.is_empty() || der.len() > 16_384)
        {
            return Err(KagemushaWalletProviderErrorV1::Invalid {
                field: "enrollment.chain",
            });
        }
        Ok(chain)
    }

    /// Durable intent of `slot`.
    ///
    /// # Errors
    ///
    /// `Unavailable` on a read error and `UnavailableCustodyData` for an invalid intent.
    pub fn read_intent(
        &self,
        slot: &KagemushaWalletSlotIdV1,
    ) -> Result<Option<KagemushaWalletIntentV1>, KagemushaWalletProviderErrorV1> {
        let intent = read_record::<F, KagemushaWalletIntentV1>(
            &self.store,
            &kagemusha_wallet_slot_dir_v1(slot),
            &kagemusha_wallet_fixed_name_v1(KAGEMUSHA_WALLET_INTENT_NAME_V1),
            KAGEMUSHA_WALLET_INTENT_MAX_BYTES_V1,
            "intent",
        )?;
        match intent {
            Some(intent)
                if intent.version == KAGEMUSHA_WALLET_INTENT_FILE_VERSION_V1
                    && intent.slot == slot.0
                    && intent.anchor().is_ok()
                    && intent.key_profile().is_ok()
                    && intent.key_generation_policy().is_ok()
                    && intent.dates.is_valid()
                    && intent.challenge.scheme_id == self.scheme_id
                    && intent.challenge.validate().is_ok() =>
            {
                Ok(Some(intent))
            }
            Some(_) => {
                Err(KagemushaWalletProviderErrorV1::UnavailableCustodyData { object: "intent" })
            }
            None => Ok(None),
        }
    }

    /// Validate a recognized attempt record against its intent. It grants no authority.
    pub(super) fn validate_generation_attempt(
        &self,
        slot: &KagemushaWalletSlotIdV1,
        intent: &KagemushaWalletIntentV1,
    ) -> Result<(), KagemushaWalletProviderErrorV1> {
        let attempt = read_record::<F, KagemushaWalletKeyGenerationAttemptV1>(
            &self.store,
            &kagemusha_wallet_slot_dir_v1(slot),
            &kagemusha_wallet_fixed_name_v1(KAGEMUSHA_WALLET_KEY_GENERATION_ATTEMPT_NAME_V1),
            KAGEMUSHA_WALLET_INTENT_MAX_BYTES_V1,
            "key_generation_attempt",
        )?;
        match attempt {
            Some(attempt)
                if attempt.version == KAGEMUSHA_WALLET_ENROLLMENT_FILE_VERSION_V1
                    && attempt.slot == slot.0
                    && attempt.challenge_digest == intent.challenge.challenge_digest()
                    && attempt.profile == intent.profile
                    && intent.key_generation_policy()?
                        == KagemushaWalletKeyGenerationPolicyV1::FreshEnrollmentOnly =>
            {
                Ok(())
            }
            _ => Err(KagemushaWalletProviderErrorV1::UnavailableCustodyData {
                object: "key_generation_attempt",
            }),
        }
    }

    /// Record that `slot` is never used again (create-new; an existing record is kept).
    ///
    /// Storage must be available now: the decision rests on the slot showing no marker.
    pub(super) fn abandon_slot(
        &mut self,
        slot: &KagemushaWalletSlotIdV1,
        reason: KagemushaWalletSlotAbandonReasonV1,
    ) -> Result<(), KagemushaWalletProviderErrorV1> {
        self.require_storage()?;
        let record = KagemushaWalletAbandonedSlotV1 {
            version: KAGEMUSHA_WALLET_ENROLLMENT_FILE_VERSION_V1,
            slot: slot.0,
            reason: reason.tag(),
        };
        let bytes = encode_envelope_v1(&record, KAGEMUSHA_WALLET_INTENT_MAX_BYTES_V1)?;
        let dir = kagemusha_wallet_slot_dir_v1(slot);
        let name = kagemusha_wallet_fixed_name_v1(KAGEMUSHA_WALLET_ABANDONED_NAME_V1);
        let written = match self.store.write_new(&dir, &name, &bytes) {
            // An earlier abandonment, whatever its reason, stands: make it durable.
            KagemushaWalletPublishOutcomeV1::NotPublished(
                KagemushaWalletNotPublishedV1::DestinationExists,
            ) => self
                .store
                .sync_file(&dir, &name)
                .and_then(|()| self.store.sync_dir(&dir))
                .map_err(KagemushaWalletProviderErrorV1::Unavailable),
            outcome => kagemusha_wallet_require_published_v1(outcome),
        };
        self.guard(slot, written)
    }
}

#[cfg(test)]
#[path = "enrollment_tests.rs"]
mod tests;
