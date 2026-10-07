//! Live issuer permit admission and immutable client/attempt/slot selection.
use super::*;
use crate::kagemusha_wallet_enrollment_v1::prekey::{
    GenerationAuthorizationV1, PreKeyDispatchV1, require_elapsed,
};
use rand::rand_core::TryRngCore as _;

pub(super) enum Pending {
    Dispatch {
        dispatch: Box<PreKeyDispatchV1>,
        started: KagemushaWalletMonotonicReadingV1,
    },
    Account {
        dispatch: Box<PreKeyDispatchV1>,
        started: KagemushaWalletMonotonicReadingV1,
        permit: KagemushaEnrollmentPermitV1,
        original: Vec<u8>,
        message: [u8; 32],
    },
}
#[derive(norito::Encode, norito::Decode, norito::NoritoSchema)]
#[norito_schema(name = "iroha_core_zk::kagemusha_wallet_enrollment_v1::PreKeySlotV1")]
struct SlotSelection {
    version: u16,
    client_nonce: [u8; 32],
    permit_digest: [u8; 32],
    slot: [u8; 32],
    generation_policy: u8,
}
fn nonce() -> Result<[u8; 32], Error> {
    let mut value = [0; 32];
    while value == [0; 32] {
        rand::rngs::OsRng
            .try_fill_bytes(&mut value)
            .map_err(|_| Error::Original("native entropy unavailable"))?;
    }
    Ok(value)
}
fn scope(role: KagemushaEnrollmentPermitScopeRoleV1, bytes: &[u8]) -> Result<[u8; 32], Error> {
    original(kagemusha_enrollment_permit_scope_digest_v1(role, bytes))
}
impl EnrollmentConfigV1 {
    pub(super) fn verify_prekey(&self) -> Result<(), Error> {
        if self.installation.scheme_id != self.scheme.scheme_id()
            || self.installation.manifest_digest == [0; 32]
        {
            return Err(Error::Original("pre-key installed scope"));
        }
        original(
            self.enrollment_certificate
                .verify_role(&self.scheme, KagemushaWalletSignerRoleV1::Enrollment),
        )?;
        self.scopes().map(|_| ())
    }
    fn scopes(&self) -> Result<[[u8; 32]; 4], Error> {
        use KagemushaEnrollmentPermitScopeRoleV1 as R;
        Ok([
            scope(R::Release, &self.release)?,
            scope(R::ServiceOrigin, &self.service_origin)?,
            scope(R::Fi, &self.fi)?,
            scope(R::Actor, &self.actor)?,
        ])
    }
    fn dispatch(
        &self,
        request_id: [u8; 32],
        account: AccountId,
        asset: KagemushaWalletAssetScopeV1,
        client_nonce: [u8; 32],
        native_dispatch_nonce: [u8; 32],
        previous_permit: Option<Vec<u8>>,
    ) -> Result<PreKeyDispatchV1, Error> {
        let [
            release_digest,
            service_origin_digest,
            fi_digest,
            actor_digest,
        ] = self.scopes()?;
        let platform = match self.policy.platform {
            KagemushaWalletEnrollmentPlatformV1::Android { .. } => {
                KagemushaEnrollmentPermitPlatformV1::Android
            }
            KagemushaWalletEnrollmentPlatformV1::Apple { .. } => {
                KagemushaEnrollmentPermitPlatformV1::Apple
            }
        };
        Ok(PreKeyDispatchV1 {
            version: 1,
            request_id,
            platform,
            purpose: if previous_permit.is_some() {
                KagemushaEnrollmentPermitPurposeV1::Resume
            } else {
                KagemushaEnrollmentPermitPurposeV1::Fresh
            },
            client_nonce,
            native_dispatch_nonce,
            manifest_digest: self.installation.manifest_digest,
            release_digest,
            service_origin_digest,
            fi_digest,
            actor_digest,
            scheme: self.scheme,
            app: self.app.clone(),
            policy: self.policy,
            enrollment_certificate: self.enrollment_certificate,
            account,
            asset,
            previous_permit,
        })
    }
}
impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1> EnrollmentOwnerV1<F, P> {
    /// Native provisioning identity; foreign enrollment DATA cannot change it.
    #[must_use]
    pub fn installation(&self) -> crate::kagemusha_wallet_artifacts_v1::InstallationV1 {
        self.config.installation
    }

    /// Retain native client identity before issuer E1 and sample a fresh live dispatch.
    /// Repeated request IDs preserve exact originals/client nonce across uncertainty/restart.
    pub fn begin(
        &mut self,
        request_id: &[u8],
        account: &[u8],
        asset: &[u8],
    ) -> Result<Vec<u8>, Error> {
        self.pending = None;
        self.request = None;
        self.selected = None;
        let id: [u8; 32] = request_id
            .try_into()
            .map_err(|_| Error::Original("pre-key request identity"))?;
        if id == [0; 32] {
            return Err(Error::Original("pre-key request identity"));
        }
        let account: AccountId = decode(account, 4096)?;
        let asset: KagemushaWalletAssetScopeV1 = decode(asset, 1024)?;
        let existing = self.provider.prekey_read(&id, PreKeyRecordV1::Client)?;
        let accepted = self.provider.prekey_read(&id, PreKeyRecordV1::Accepted)?;
        let slot = self.provider.prekey_read(&id, PreKeyRecordV1::Slot)?;
        if existing.is_none() && (accepted.is_some() || slot.is_some()) {
            return Err(Error::Original("selected pre-key client lost"));
        }
        if accepted.is_none() && slot.is_some() {
            return Err(Error::Original("selected pre-key permit lost"));
        }
        let client_nonce = match &existing {
            Some(bytes) => {
                let original = PreKeyDispatchV1::decode(bytes).map_err(Error::Original)?;
                if original.purpose != KagemushaEnrollmentPermitPurposeV1::Fresh
                    || original.previous_permit.is_some()
                {
                    return Err(Error::Original("pre-key initial dispatch"));
                }
                original.client_nonce
            }
            None => nonce()?,
        };
        let mut dispatch =
            self.config
                .dispatch(id, account, asset, client_nonce, nonce()?, None)?;
        let proposed = dispatch.encode().map_err(Error::Original)?;
        let retained = match self
            .provider
            .prekey_retain(&id, PreKeyRecordV1::Client, &proposed)?
        {
            PreKeyPublicationV1::Published => proposed,
            PreKeyPublicationV1::Existing(bytes) => bytes,
        };
        let initial = PreKeyDispatchV1::decode(&retained).map_err(Error::Original)?;
        if initial.stable_selection().map_err(Error::Original)?
            != dispatch.stable_selection().map_err(Error::Original)?
        {
            return Err(Error::Original("pre-key request changed originals"));
        }
        dispatch.previous_permit = accepted;
        if dispatch.previous_permit.is_some() {
            dispatch.purpose = KagemushaEnrollmentPermitPurposeV1::Resume;
        }
        dispatch.validate().map_err(Error::Original)?;
        if let (Some(original), Some(slot)) = (&dispatch.previous_permit, slot) {
            let selected = self.decode_slot(&dispatch, original, &slot)?;
            let permit = KagemushaEnrollmentPermitV1::decode_canonical(
                original,
                &self.config.scheme,
                &self.config.enrollment_certificate,
            )
            .map_err(|_| Error::Original("pre-key permit"))?;
            self.selected = Some((
                Scope {
                    challenge: permit.body.challenge,
                    account: dispatch.account.clone(),
                    asset: dispatch.asset.clone(),
                },
                KagemushaWalletSlotIdV1(selected.slot),
            ));
        } else {
            self.selected = None;
        }
        let started = self.provider.monotonic_reading()?;
        let bytes = dispatch.encode().map_err(Error::Original)?;
        self.pending = Some(Pending::Dispatch {
            dispatch: Box::new(dispatch),
            started,
        });
        Ok(bytes)
    }

    /// Admit an issuer-signed permit for this live dispatch, then expose the exact existing-
    /// account signing challenge. No decoded permit or persisted clock becomes live authority.
    pub fn accept_permit(&mut self, bytes: &[u8]) -> Result<[u8; 32], Error> {
        if let Some(Pending::Account {
            original, message, ..
        }) = &self.pending
        {
            if original == bytes {
                return Ok(*message);
            }
        }
        let Some(Pending::Dispatch { dispatch, started }) = self.pending.take() else {
            return Err(Error::Phase);
        };
        let permit = original(KagemushaEnrollmentPermitV1::decode_canonical(
            bytes,
            &self.config.scheme,
            &self.config.enrollment_certificate,
        ))?;
        dispatch
            .require_permit_selection(&permit)
            .map_err(Error::Original)?;
        if permit.body.native_dispatch_nonce != dispatch.native_dispatch_nonce
            || permit.body.purpose != dispatch.purpose
        {
            return Err(Error::Original("pre-key live dispatch"));
        }
        require_elapsed(
            &started,
            &self.provider.monotonic_reading()?,
            permit.body.observed_at_ms,
            permit.body.expires_at_ms,
        )
        .map_err(Error::Original)?;
        let selected = match &dispatch.previous_permit {
            Some(original) => original.as_slice(),
            None => bytes,
        };
        let retained = match self.provider.prekey_retain(
            &dispatch.request_id,
            PreKeyRecordV1::Accepted,
            selected,
        )? {
            PreKeyPublicationV1::Published => selected.to_vec(),
            PreKeyPublicationV1::Existing(original) => original,
        };
        let first = original(KagemushaEnrollmentPermitV1::decode_canonical(
            &retained,
            &self.config.scheme,
            &self.config.enrollment_certificate,
        ))?;
        dispatch
            .require_permit_selection(&first)
            .map_err(Error::Original)?;
        if first.body.attempt_id != permit.body.attempt_id
            || first.body.challenge != permit.body.challenge
            || first.body.created_at_ms != permit.body.created_at_ms
            || first.body.expires_at_ms != permit.body.expires_at_ms
        {
            return Err(Error::Original("pre-key original attempt changed"));
        }
        let transcript = norito::encode_canonical(&(
            dispatch.stable_selection().map_err(Error::Original)?,
            bytes.to_vec(),
        ))
        .map_err(|_| Error::Original("pre-key account transcript"))?;
        let message = kagemusha_wallet_provider_digest_v1("enrollment-local-account", &transcript);
        self.pending = Some(Pending::Account {
            dispatch,
            started,
            permit,
            original: bytes.to_vec(),
            message,
        });
        Ok(message)
    }
    fn decode_slot(
        &self,
        dispatch: &PreKeyDispatchV1,
        permit: &[u8],
        bytes: &[u8],
    ) -> Result<SlotSelection, Error> {
        let slot: SlotSelection = decode(bytes, 1024)?;
        if slot.version != 1
            || slot.client_nonce != dispatch.client_nonce
            || slot.slot == [0; 32]
            || slot.permit_digest
                != kagemusha_wallet_provider_digest_v1("enrollment-pre-key-permit", permit)
            || KagemushaWalletKeyGenerationPolicyV1::from_tag(slot.generation_policy).is_none()
        {
            return Err(Error::Original("pre-key selected slot"));
        }
        Ok(slot)
    }
    /// Consume exact account authorization and recheck the permit at actual key generation.
    pub fn authorize(&mut self, signature: &[u8]) -> Result<EnrollmentProgressV1, Error> {
        let Some(Pending::Account {
            dispatch,
            started,
            permit,
            message,
            ..
        }) = self.pending.take()
        else {
            return Err(Error::Phase);
        };
        authorize(&dispatch.account, &message, signature)?;
        require_elapsed(
            &started,
            &self.provider.monotonic_reading()?,
            permit.body.observed_at_ms,
            permit.body.expires_at_ms,
        )
        .map_err(Error::Original)?;
        let original = self
            .provider
            .prekey_read(&dispatch.request_id, PreKeyRecordV1::Accepted)?
            .ok_or(Error::Original("selected pre-key permit lost"))?;
        let first = KagemushaEnrollmentPermitV1::decode_canonical(
            &original,
            &self.config.scheme,
            &self.config.enrollment_certificate,
        )
        .map_err(|_| Error::Original("selected pre-key permit"))?;
        dispatch
            .require_permit_selection(&first)
            .map_err(Error::Original)?;
        if first.body.purpose != KagemushaEnrollmentPermitPurposeV1::Fresh
            || first.body.attempt_id != permit.body.attempt_id
            || first.body.challenge != permit.body.challenge
            || first.body.created_at_ms != permit.body.created_at_ms
            || first.body.expires_at_ms != permit.body.expires_at_ms
        {
            return Err(Error::Original("pre-key original attempt changed"));
        }
        let existing = self
            .provider
            .prekey_read(&dispatch.request_id, PreKeyRecordV1::Slot)?;
        let (slot, fresh) = match existing {
            Some(bytes) => (self.decode_slot(&dispatch, &original, &bytes)?, false),
            None => {
                let candidate = SlotSelection {
                    version: 1,
                    client_nonce: dispatch.client_nonce,
                    permit_digest: kagemusha_wallet_provider_digest_v1(
                        "enrollment-pre-key-permit",
                        &original,
                    ),
                    slot: KagemushaWalletSlotIdV1::generate()
                        .map_err(KagemushaWalletProviderErrorV1::Unavailable)?
                        .0,
                    generation_policy: self.provider.enrollment_generation_policy()?.tag(),
                };
                let bytes = norito::encode_canonical(&candidate)
                    .map_err(|_| Error::Original("pre-key slot encoding"))?;
                match self.provider.prekey_retain(
                    &dispatch.request_id,
                    PreKeyRecordV1::Slot,
                    &bytes,
                )? {
                    PreKeyPublicationV1::Published => (candidate, true),
                    PreKeyPublicationV1::Existing(bytes) => {
                        (self.decode_slot(&dispatch, &original, &bytes)?, false)
                    }
                }
            }
        };
        let profile = match self.config.policy.platform {
            KagemushaWalletEnrollmentPlatformV1::Apple { .. }
            | KagemushaWalletEnrollmentPlatformV1::Android {
                hardware: KagemushaWalletAndroidHardwareV1::StrongBox,
                ..
            } => KagemushaWalletKeyProfileV1::SecureElement,
            KagemushaWalletEnrollmentPlatformV1::Android {
                hardware: KagemushaWalletAndroidHardwareV1::TeeOrStrongBox,
                ..
            } => KagemushaWalletKeyProfileV1::SecureElementOrTee,
            KagemushaWalletEnrollmentPlatformV1::Android {
                hardware: KagemushaWalletAndroidHardwareV1::Tee,
                ..
            } => KagemushaWalletKeyProfileV1::AndroidTee,
        };
        let selected = KagemushaWalletSlotIdV1(slot.slot);
        self.selected = Some((
            Scope {
                challenge: permit.body.challenge,
                account: dispatch.account.clone(),
                asset: dispatch.asset.clone(),
            },
            selected,
        ));
        self.request = None;
        let authorization = GenerationAuthorizationV1::new(
            self.provider.prekey_root_identity(),
            &permit,
            profile,
            selected,
            KagemushaWalletKeyGenerationPolicyV1::from_tag(slot.generation_policy)
                .ok_or(Error::Original("pre-key creation policy"))?,
            fresh,
            started,
        );
        authorization.check(&self.provider)?;
        if fresh {
            self.provider.begin_enrollment_checked(authorization)?;
            return self.progress();
        }
        match self.provider.status(&selected)? {
            KagemushaWalletSlotStatusV1::Empty => {
                self.provider.begin_enrollment_checked(authorization)?;
            }
            KagemushaWalletSlotStatusV1::IntentOnly => {
                self.provider.resume_enrollment_checked(authorization)?;
            }
            _ => {}
        }
        self.progress()
    }
}
