//! Native E2-E6 orchestration over one exclusive retained hardware/provider owner.

use super::carrier::*;
use crate::kagemusha_wallet_advance_v1::*;
use iroha_crypto::{Algorithm, Signature};
use iroha_data_model::{account::AccountId, kagemusha::*};
use rand::rand_core::TryRngCore as _;
use sha2::{Digest as _, Sha256};

/// Operator-owned native provisioning. Foreign enrollment calls cannot choose these policies.
#[derive(Clone)]
pub struct EnrollmentConfigV1 {
    /// Independently authenticated deployment scheme, including Enrollment certificate root.
    pub scheme: KagemushaWalletSchemeV1,
    /// Exact approved app-policy preimage.
    pub app: KagemushaWalletAppPolicyV1,
    /// Exact approved enrollment-policy preimage, not a client liveness assertion.
    pub policy: KagemushaWalletEnrollmentPolicyV1,
    /// Exact independently approved root DER; a self-consistent pin is not approval.
    pub attestation_root_der: Vec<u8>,
}
/// Refusal without converting unavailable custody to absence or issuer rejection.
#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum Error {
    /// Exact native provider error, including uncertain publication.
    #[error(transparent)]
    Provider(#[from] KagemushaWalletProviderErrorV1),
    /// Original/account/policy/credential authentication failed.
    #[error("enrollment original: {0}")]
    Original(&'static str),
    /// No pending account/evidence challenge or wrong lifecycle phase.
    #[error("enrollment phase")]
    Phase,
}
fn original<T>(value: Result<T, KagemushaWalletValidationErrorV1>) -> Result<T, Error> {
    value.map_err(|_| Error::Original("protocol original"))
}
fn decode<T>(bytes: &[u8], maximum: usize) -> Result<T, Error>
where
    T: norito::NoritoSerialize,
    for<'de> T: norito::NoritoDeserialize<'de>,
{
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(Error::Original("frame bound"));
    }
    norito::decode_canonical_with_limits(bytes, norito::canonical_decode_limits(maximum))
        .map_err(|_| Error::Original("canonical frame"))
}
fn authorize(account: &AccountId, message: &[u8; 32], signature: &[u8]) -> Result<(), Error> {
    let key = account
        .try_signatory()
        .filter(|key| key.algorithm() == Algorithm::Ed25519)
        .ok_or(Error::Original("existing account"))?;
    if signature.len() != 64 {
        return Err(Error::Original("account signature bound"));
    }
    Signature::from_bytes(signature)
        .verify(key, message)
        .map_err(|_| Error::Original("account signature"))
}
#[derive(Clone)]
struct Scope {
    challenge: KagemushaWalletEnrollmentChallengeV1,
    account: AccountId,
    asset: KagemushaWalletAssetScopeV1,
}
struct PendingAccount {
    scope: Scope,
    message: [u8; 32],
}

/// Native source-selected enrollment progress. It does not assert issuer approval.
#[derive(Debug, Clone)]
pub enum EnrollmentProgressV1 {
    /// Durable generation-zero marker; originals identify the exact key to attest.
    Evidence {
        /// Opaque random native slot for the platform evidence exporter.
        slot: [u8; 32],
        /// Exact generation-zero marker, not an App-supplied head.
        marker: KagemushaWalletMarkerV1,
        /// Exact challenge digest supplied during key generation.
        challenge_digest: [u8; 32],
    },
    /// Reconciliation did not establish an enrolled marker. No key is regenerated here.
    Pending,
    /// This exact attempt is terminal; a new issuer challenge is required.
    Abandoned,
    /// E7 already selected or completed; recover through strict original wallet open.
    BootstrapSelected,
}
/// E5 returns either the exact selected request or an account signature challenge for new originals.
#[derive(Debug, Clone)]
pub enum RequestPreparationV1 {
    /// Existing request wins over newly supplied evidence, including after uncertain delivery.
    Retained(Vec<u8>),
    /// Exact E5 message over challenge, generated key, policies, account/asset and evidence.
    AccountChallenge([u8; 32]),
}

/// The sole provider owner for enrollment; construction is a trusted native startup operation.
pub struct EnrollmentOwnerV1<F: KagemushaWalletFsV1, P> {
    provider: KagemushaWalletProviderV1<F, P>,
    config: EnrollmentConfigV1,
    pending: Option<PendingAccount>,
    selected: Option<(Scope, KagemushaWalletSlotIdV1)>,
    request: Option<RequestBodyV1>,
}
impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1> EnrollmentOwnerV1<F, P> {
    /// Bind existing exclusive custody to independently provisioned deployment originals.
    /// On refusal, the same provider remains owned by the caller.
    pub fn new(
        provider: KagemushaWalletProviderV1<F, P>,
        config: EnrollmentConfigV1,
    ) -> Result<Self, (KagemushaWalletProviderV1<F, P>, Error)> {
        let checked = (|| {
            original(config.scheme.validate())?;
            original(config.policy.validate_for_app(&config.app))?;
            let pinned_root = match config.policy.platform {
                KagemushaWalletEnrollmentPlatformV1::Android {
                    attestation_root_sha256,
                    ..
                }
                | KagemushaWalletEnrollmentPlatformV1::Apple {
                    attestation_root_sha256,
                } => attestation_root_sha256,
            };
            if config.attestation_root_der.is_empty()
                || config.attestation_root_der.len() > 16_384
                || <[u8; 32]>::from(Sha256::digest(&config.attestation_root_der)) != pinned_root
            {
                return Err(Error::Original("approved root original"));
            }
            if config.scheme.scheme_id() != *provider.scheme_id()
                || config.policy.scheme_id != *provider.scheme_id()
            {
                return Err(Error::Original("native deployment scheme"));
            }
            Ok(())
        })();
        if let Err(error) = checked {
            return Err((provider, error));
        }
        Ok(Self {
            provider,
            config,
            pending: None,
            selected: None,
            request: None,
        })
    }
    /// Release this opaque enrollment owner to the native artifact loader/open path.
    /// No account admission or Bootstrap authority transfers with this method.
    pub fn into_provider(self) -> KagemushaWalletProviderV1<F, P> {
        self.provider
    }

    /// Transfer the same exclusive provider to a native loader, restoring every private
    /// enrollment selection if that loader returns custody with an error. This grants no
    /// account or proof admission; the destination must perform its complete own checks.
    pub fn try_handoff<T, E>(
        self,
        load: impl FnOnce(
            KagemushaWalletProviderV1<F, P>,
        ) -> Result<T, (KagemushaWalletProviderV1<F, P>, E)>,
    ) -> Result<T, (Self, E)> {
        let Self {
            provider,
            config,
            pending,
            selected,
            request,
        } = self;
        match load(provider) {
            Ok(value) => Ok(value),
            Err((provider, error)) => Err((
                Self {
                    provider,
                    config,
                    pending,
                    selected,
                    request,
                },
                error,
            )),
        }
    }

    /// Sample a local account challenge before touching hardware. This is distinct from issuer E5 authorization.
    pub fn begin(
        &mut self,
        challenge: &[u8],
        account: &[u8],
        asset: &[u8],
    ) -> Result<[u8; 32], Error> {
        let challenge = decode(challenge, 1024)?;
        original(
            self.config
                .policy
                .verify_challenge(&self.config.app, &challenge),
        )?;
        let account: AccountId = decode(account, 4096)?;
        let asset: KagemushaWalletAssetScopeV1 = decode(asset, 1024)?;
        original(asset.validate())?;
        if account
            .try_signatory()
            .is_none_or(|key| key.algorithm() != Algorithm::Ed25519)
            || original(kagemusha_wallet_account_digest_v1(&account))? != challenge.account_digest
            || asset.asset_digest() != challenge.asset_digest
        {
            return Err(Error::Original("account/asset binding"));
        }
        let mut nonce = [0; 32];
        while nonce == [0; 32] {
            rand::rngs::OsRng
                .try_fill_bytes(&mut nonce)
                .map_err(|_| Error::Original("native entropy unavailable"))?;
        }
        let scope = Scope {
            challenge,
            account,
            asset,
        };
        let mut transcript = norito::encode_canonical(&(
            scope.challenge,
            scope.account.clone(),
            scope.asset.clone(),
        ))
        .map_err(|_| Error::Original("local account transcript"))?;
        transcript.extend_from_slice(&nonce);
        let message = kagemusha_wallet_provider_digest_v1("enrollment-local-account", &transcript);
        self.pending = Some(PendingAccount { scope, message });
        Ok(message)
    }
    /// Consume local account authorization, recovering the same exact challenge slot before considering creation.
    pub fn authorize(&mut self, signature: &[u8]) -> Result<EnrollmentProgressV1, Error> {
        let pending = self.pending.take().ok_or(Error::Phase)?;
        authorize(&pending.scope.account, &pending.message, signature)?;
        let mut selected = None;
        for slot in self.provider.slots()? {
            if self
                .provider
                .read_intent(&slot)?
                .is_some_and(|intent| intent.challenge == pending.scope.challenge)
            {
                if selected.replace(slot).is_some() {
                    return Err(Error::Original("multiple challenge slots"));
                }
            }
        }
        let slot = match selected {
            Some(slot) => slot,
            None => {
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
                match self
                    .provider
                    .begin_enrollment(&pending.scope.challenge, profile)?
                {
                    KagemushaWalletEnrollmentStepV1::Enrolled { slot, .. }
                    | KagemushaWalletEnrollmentStepV1::Pending { slot }
                    | KagemushaWalletEnrollmentStepV1::SlotAbandoned { slot } => slot,
                }
            }
        };
        self.selected = Some((pending.scope, slot));
        self.request = None;
        self.progress()
    }
    /// Recover source status without foreign liveness, client time or key-regeneration authority.
    pub fn progress(&mut self) -> Result<EnrollmentProgressV1, Error> {
        let (scope, slot) = self.selected.as_ref().ok_or(Error::Phase)?;
        Ok(match self.provider.status(slot)? {
            KagemushaWalletSlotStatusV1::Enrollment(marker) => EnrollmentProgressV1::Evidence {
                slot: slot.0,
                marker: marker.marker().clone(),
                challenge_digest: scope.challenge.challenge_digest(),
            },
            KagemushaWalletSlotStatusV1::IntentOnly => EnrollmentProgressV1::Pending,
            KagemushaWalletSlotStatusV1::Pending(_) | KagemushaWalletSlotStatusV1::Released(_) => {
                EnrollmentProgressV1::BootstrapSelected
            }
            KagemushaWalletSlotStatusV1::SlotAbandoned
            | KagemushaWalletSlotStatusV1::Terminal(_) => EnrollmentProgressV1::Abandoned,
            KagemushaWalletSlotStatusV1::Empty => {
                return Err(Error::Original("selected enrollment disappeared"));
            }
        })
    }
    fn selected_marker(&mut self) -> Result<KagemushaWalletMarkerV1, Error> {
        let (scope, slot) = self.selected.as_ref().ok_or(Error::Phase)?;
        let status = self.provider.status(slot)?;
        let marker = match status {
            KagemushaWalletSlotStatusV1::Enrollment(marker)
            | KagemushaWalletSlotStatusV1::Pending(marker)
            | KagemushaWalletSlotStatusV1::Released(marker) => marker,
            _ => return Err(Error::Phase),
        };
        let intent = self
            .provider
            .read_intent(slot)?
            .ok_or(Error::Original("selected enrollment intent"))?;
        if intent.challenge != scope.challenge
            || marker.marker().scheme_id != scope.challenge.scheme_id
            || marker.marker().asset_digest != scope.challenge.asset_digest
            || marker.marker().wallet_id != scope.challenge.wallet_id(marker.payment_key())
        {
            return Err(Error::Original("selected enrollment marker"));
        }
        match self.provider.probe_payment_key(slot)? {
            KagemushaWalletProbeV1::Present(key) if key == *marker.payment_key() => {}
            KagemushaWalletProbeV1::Present(_) | KagemushaWalletProbeV1::Absent => {
                return Err(KagemushaWalletProviderErrorV1::KeyLost.into());
            }
            KagemushaWalletProbeV1::Unavailable(reason) => {
                return Err(KagemushaWalletProviderErrorV1::Unavailable(reason).into());
            }
        }
        original(KagemushaWalletMarkerV1::enrollment(
            &scope.challenge,
            *marker.payment_key(),
        ))
    }
    fn retained(&mut self) -> Result<Option<Vec<u8>>, Error> {
        let marker = self.selected_marker()?;
        let (scope, slot) = self.selected.as_ref().ok_or(Error::Phase)?;
        // Provider reconciles and durably adopts any uncertain E5 record before retransmission.
        let Some(record) = self.provider.enrollment_record(slot)? else {
            if self.provider.credential(slot, 0)?.is_some() {
                return Err(Error::Original("selected enrollment request lost"));
            }
            return Ok(None);
        };
        let request = RequestV1::decode(&record.request).map_err(Error::Original)?;
        if request.body.marker != marker
            || record.enrollment_marker_digest != original(marker.marker_digest())?
            || request.body.challenge != scope.challenge
            || request.body.account != scope.account
            || request.body.asset != scope.asset
            || request.body.app != self.config.app
            || request.body.policy != self.config.policy
        {
            return Err(Error::Original("retained enrollment request binding"));
        }
        Ok(Some(
            self.provider
                .retain_enrollment_request(slot, &record.request)?,
        ))
    }
    /// Select exact E5 evidence before exposing its existing-account challenge. Retained requests always win.
    pub fn prepare_request(&mut self, evidence: &[u8]) -> Result<RequestPreparationV1, Error> {
        if let Some(bytes) = self.retained()? {
            return Ok(RequestPreparationV1::Retained(bytes));
        }
        let EnrollmentProgressV1::Evidence { marker, .. } = self.progress()? else {
            return Err(Error::Phase);
        };
        PlatformEvidenceV1::decode(evidence, &self.config.policy).map_err(Error::Original)?;
        let (scope, _) = self.selected.as_ref().ok_or(Error::Phase)?;
        let body = RequestBodyV1 {
            version: 1,
            challenge: scope.challenge,
            marker,
            app: self.config.app.clone(),
            policy: self.config.policy,
            account: scope.account.clone(),
            asset: scope.asset.clone(),
            evidence: evidence.to_vec(),
        };
        let message = body.account_challenge().map_err(Error::Original)?;
        self.request = Some(body);
        Ok(RequestPreparationV1::AccountChallenge(message))
    }
    /// Retain the exact account-authorized E5 request durably before returning network bytes.
    pub fn retain_request(&mut self, signature: &[u8]) -> Result<Vec<u8>, Error> {
        if let Some(bytes) = self.retained()? {
            return Ok(bytes);
        }
        let body = self.request.as_ref().ok_or(Error::Phase)?;
        let signature: [u8; 64] = signature
            .try_into()
            .map_err(|_| Error::Original("E5 account signature bound"))?;
        let request = RequestV1 {
            body: body.clone(),
            account_signature: signature,
        };
        let bytes = request.encode().map_err(Error::Original)?;
        let (_, slot) = self.selected.as_ref().ok_or(Error::Phase)?;
        let exact = self.provider.retain_enrollment_request(slot, &bytes)?;
        if exact != bytes {
            return Err(Error::Original("concurrent enrollment request"));
        }
        self.request = None;
        Ok(exact)
    }
    /// Authenticate and durably retain E6 before selecting credential zero.
    /// Existing exact result bytes win on every retry, including after uncertain delivery.
    pub fn accept_credential(&mut self, supplied: &[u8]) -> Result<Vec<u8>, Error> {
        let request_bytes = self.retained()?.ok_or(Error::Phase)?;
        let request = RequestV1::decode(&request_bytes).map_err(Error::Original)?;
        let (_, slot) = self.selected.as_ref().ok_or(Error::Phase)?;
        let slot = *slot;
        let key = kagemusha_wallet_provider_digest_v1("enrollment-issuer-result", &request_bytes);
        let retained = self
            .provider
            .with_archive(&slot, |archive| archive.read_record(&key, RESULT_MAX_BYTES))?;
        if retained.is_none() && self.provider.credential(&slot, 0)?.is_some() {
            return Err(Error::Original("selected enrollment result lost"));
        }
        let bytes = retained.as_deref().unwrap_or(supplied);
        let result = ResultV1::decode(bytes).map_err(Error::Original)?;
        self.verify_result(&request, &result)?;
        self.provider
            .with_archive(&slot, |archive| archive.write_record(&key, bytes))?;
        self.provider
            .store_credential(&slot, 0, &result.credential)?;
        Ok(bytes.to_vec())
    }
    /// Exact retained originals for the separate account-admission boundary. This verifies
    /// and adopts E6 again; it grants no account or Bootstrap authority.
    pub fn open_originals(&mut self) -> Result<[Vec<u8>; 4], Error> {
        let selected = self.accept_credential(&[])?;
        let result = ResultV1::decode(&selected).map_err(Error::Original)?;
        let request =
            RequestV1::decode(&self.retained()?.ok_or(Error::Phase)?).map_err(Error::Original)?;
        Ok([
            result.credential,
            result.certificates,
            norito::encode_canonical(&request.body.account)
                .map_err(|_| Error::Original("account encoding"))?,
            norito::encode_canonical(&request.body.asset)
                .map_err(|_| Error::Original("asset encoding"))?,
        ])
    }

    fn verify_result(&self, request: &RequestV1, result: &ResultV1) -> Result<(), Error> {
        let credential_value = original(KagemushaWalletCredentialV1::decode_canonical(
            &result.credential,
            self.provider.scheme_id(),
        ))?;
        let set: KagemushaWalletCertificateSetV1 =
            decode(&result.certificates, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1)?;
        let issuer = original(set.certificate(
            &credential_value.body.issuer_certificate,
            KagemushaWalletSignerRoleV1::Enrollment,
        ))?;
        original(credential_value.verify_enrollment(
            &self.config.scheme,
            issuer,
            &request.body.challenge,
            &request.body.marker.payment_key,
        ))?;
        let c = &credential_value.body;
        let kind_matches = matches!(
            (self.config.policy.platform, c.evidence_kind),
            (
                KagemushaWalletEnrollmentPlatformV1::Apple { .. },
                KagemushaWalletEvidenceKindV1::AppleAppAttest
            ) | (
                KagemushaWalletEnrollmentPlatformV1::Android {
                    hardware: KagemushaWalletAndroidHardwareV1::Tee,
                    ..
                },
                KagemushaWalletEvidenceKindV1::AndroidKeyMintTee
            ) | (
                KagemushaWalletEnrollmentPlatformV1::Android {
                    hardware: KagemushaWalletAndroidHardwareV1::StrongBox,
                    ..
                },
                KagemushaWalletEvidenceKindV1::AndroidKeyMintStrongBox
            ) | (
                KagemushaWalletEnrollmentPlatformV1::Android {
                    hardware: KagemushaWalletAndroidHardwareV1::TeeOrStrongBox,
                    ..
                },
                KagemushaWalletEvidenceKindV1::AndroidKeyMintTee
                    | KagemushaWalletEvidenceKindV1::AndroidKeyMintStrongBox
            )
        );
        if !kind_matches
            || c.regulatory_policy != self.config.policy.regulatory_policy
            || c.lease_expires_at_ms
                != original(self.config.policy.lease_expires_at(c.issued_at_ms))?
            || c.enrollment_evidence.digest
                != result
                    .evidence
                    .digest(&request.body, c.evidence_kind)
                    .map_err(Error::Original)?
        {
            return Err(Error::Original("issuer enrollment policy/evidence"));
        }
        Ok(())
    }
}
