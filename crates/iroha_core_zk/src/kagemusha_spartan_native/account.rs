//! Account admission for future typed custody. Account decoding and signatures use
//! the canonical intake helpers; only the selected proof frame is backend-specific.
use super::*;
use crate::kagemusha_wallet_advance_v1::{
    KagemushaWalletLookupV1 as Lookup, KagemushaWalletMarkerRecordV1,
    KagemushaWalletProbeV1 as Probe, KagemushaWalletSlotIdV1 as Slot,
    KagemushaWalletSlotStatusV1 as Status, OutputSourceV1,
};
use crate::kagemusha_wallet_enrollment_v1::{
    RequestV1, ResultV1, SPARTAN_CREDENTIAL_BYTES_V1, spartan_signer_certificate_original_v1,
};
use crate::kagemusha_wallet_intake_v1 as intake;
use iroha_data_model::{
    account::AccountId,
    kagemusha::{
        KagemushaDevicePublicKeyV1, KagemushaWalletCertificateSetV1,
        KagemushaWalletSignerCertificateV1, KagemushaWalletSignerRoleV1,
    },
};
use rand::rand_core::TryRng as _;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Access {
    Monetary,
    HistoricalOutput,
}

/// A complete E6 delivery offered for account admission, with the Enrollment certificate
/// the signed selected owner supplies. The bytes are the exact offered original.
struct EnrollmentResult<'a> {
    original: &'a [u8],
    result: &'a ResultV1,
    issuer: &'a KagemushaWalletSignerCertificateV1,
}

pub(super) struct AccountAdmission {
    originals: [Vec<u8>; 4],
    // The exact E6 original this admission began from, or `None` for split originals. A
    // retry must present the same form and the same bytes.
    enrollment_result: Option<Vec<u8>>,
    credential: native::Credential,
    account: AccountId,
    slot: Slot,
    marker_file_digest: [u8; 32],
    // Zero means nonce generation did not complete. The selected source remains pinned.
    challenge: [u8; 32],
    authorized: bool,
    access: Access,
}
fn rejected(field: &'static str) -> ProviderError {
    ProviderError::Invalid { field }
}
fn original_error(error: intake::Error) -> ProviderError {
    use crate::kagemusha_wallet_advance_v1::KagemushaWalletDecodeResourceV1 as Resource;
    match error {
        intake::Error::DecodeResource(original) => {
            ProviderError::DecodeResource(match original.kind() {
                norito::core::DecodeLocalRefusalKind::Allocator => Resource::Allocator,
                norito::core::DecodeLocalRefusalKind::EnclosingLimit => Resource::EnclosingLimit,
            })
        }
        intake::Error::Provider(error) => error,
        _ => rejected("future original account admission"),
    }
}
fn same_credential(a: &native::Credential, b: &native::Credential) -> bool {
    a.body == b.body && a.signature == b.signature
}

impl<F: KagemushaWalletFsV1, P: KagemushaWalletPlatformV1 + 'static, S> FutureRuntime<F, P, S> {
    /// Authenticate original credential/certificate/account/asset frames and derive one
    /// fresh challenge from the actual enrolled slot and current selected marker.
    /// Ordinary refusal retains any already issued challenge and selected source.
    ///
    /// # Errors
    /// Rejects foreign originals, unproven custody, an ambiguous slot or unavailable key.
    pub fn begin_account(
        &mut self,
        credential: &[u8],
        certificate: &[u8],
        account: &[u8],
        asset: &[u8],
    ) -> Result<[u8; 32], ProviderError> {
        self.begin_account_from([credential, certificate, account, asset], None)
    }

    /// Begin the same account admission from one complete E6 delivery and the existing
    /// account original. `asset` and `selected_issuer` come from the signed selected owner,
    /// never from the caller of a foreign boundary. The delivered certificate set must be
    /// canonical and hold exactly that issuer; its raw `body.transcript() || signature`
    /// original and the exact 596-byte credential then enter the same admission as
    /// [`Self::begin_account`]. Before anything is retained, the result is verified against
    /// the retained account-authorized E5 of the enrolled slot (challenge, evidence,
    /// enrollment and wallet identity, lease and signer), its retained intent dates and the
    /// positive hardware key. The offered bytes must then be the exact durable E6 winner
    /// that enrollment retained for that E5, read back under the same verified source marker
    /// without reconciling, writing or signing anything; a selected head and a
    /// retained-output-only source are answered as they are. A retry returns the retained
    /// challenge only for the exact same E6, account and asset originals. The split form of
    /// [`Self::begin_account`] stays a distinct admission and reads no winner.
    ///
    /// # Errors
    /// Rejects everything [`Self::begin_account`] rejects, a malformed or foreign E6, a
    /// certificate set without the selected issuer, an E6 that does not bind the retained
    /// E5 or is not byte-identical to its durable winner, and a retry whose originals differ
    /// from the retained admission. A winner that is lost or cannot be read is unavailable
    /// custody data or storage, never absence that admits the offered bytes.
    pub fn begin_account_result(
        &mut self,
        result: &[u8],
        account: &[u8],
        asset: &[u8],
        selected_issuer: &KagemushaWalletSignerCertificateV1,
    ) -> Result<[u8; 32], ProviderError> {
        let decoded = ResultV1::decode(result)
            .map_err(|_| rejected("future account enrollment result"))?;
        if decoded.credential.len() != SPARTAN_CREDENTIAL_BYTES_V1 {
            return Err(rejected("future account enrollment credential"));
        }
        selected_issuer
            .verify_role(&self.scheme, KagemushaWalletSignerRoleV1::Enrollment)
            .map_err(|_| rejected("future account selected issuer"))?;
        // The complete delivered set must contain the selected issuer itself. Only that
        // certificate's raw original is emitted; no other member of the set is consulted.
        let set: KagemushaWalletCertificateSetV1 = norito::decode_canonical_with_limits(
            &decoded.certificates,
            norito::canonical_decode_limits(decoded.certificates.len()),
        )
        .map_err(|_| rejected("future account enrollment certificate set"))?;
        set.validate()
            .map_err(|_| rejected("future account enrollment certificate set"))?;
        if set
            .certificate(
                &selected_issuer.certificate_digest(),
                KagemushaWalletSignerRoleV1::Enrollment,
            )
            .ok()
            != Some(selected_issuer)
        {
            return Err(rejected("future account enrollment issuer"));
        }
        let certificate = spartan_signer_certificate_original_v1(selected_issuer);
        self.begin_account_from(
            [
                decoded.credential.as_slice(),
                certificate.as_slice(),
                account,
                asset,
            ],
            Some(EnrollmentResult {
                original: result,
                result: &decoded,
                issuer: selected_issuer,
            }),
        )
    }

    fn begin_account_from(
        &mut self,
        originals: [&[u8]; 4],
        enrollment: Option<EnrollmentResult<'_>>,
    ) -> Result<[u8; 32], ProviderError> {
        let [credential, certificate, account, asset] = originals;
        if credential.len() != 596
            || certificate.len() != 172
            || account.is_empty()
            || account.len() > intake::ACCOUNT_ORIGINAL_MAX_BYTES_V1
            || asset.is_empty()
            || asset.len() > intake::ASSET_SCOPE_ORIGINAL_MAX_BYTES_V1
        {
            return Err(rejected("future account original bounds"));
        }
        if let Some(held) = &self.account {
            if held
                .originals
                .iter()
                .zip(originals)
                .any(|(a, b)| a.as_slice() != b)
                || held.enrollment_result.as_deref()
                    != enrollment.as_ref().map(|offered| offered.original)
            {
                return Err(rejected("retained future account originals changed"));
            }
            if held.challenge != [0; 32] {
                return Ok(held.challenge);
            }
        } else {
            let credential = self
                .engine
                .authenticate_credential_originals(credential, certificate, &self.parameters)
                .map_err(provider_error)?;
            let account = intake::account(account, credential.body[98..130].try_into().unwrap())
                .map_err(original_error)?;
            intake::asset_scope(asset, credential.body[34..66].try_into().unwrap())
                .map_err(original_error)?;
            let (slot, marker, access) = self.select_account_source(&credential)?;
            let marker_file_digest = *marker.marker_file_digest();
            // The hardware key, intent, marker and retained credential are proven above.
            // A complete E6 must also bind the retained E5 and be the exact durable winner
            // of that enrollment, under this same verified marker, before this admission is
            // stored and before any challenge exists.
            if let Some(offered) = &enrollment {
                self.require_enrollment_result(
                    &slot,
                    &marker,
                    &credential,
                    offered,
                    &account,
                    asset,
                )?;
            }
            self.account = Some(AccountAdmission {
                originals: originals.map(<[u8]>::to_vec),
                enrollment_result: enrollment.as_ref().map(|offered| offered.original.to_vec()),
                credential,
                account,
                slot,
                marker_file_digest,
                challenge: [0; 32],
                authorized: false,
                access,
            });
        }
        let held = self
            .account
            .as_ref()
            .ok_or_else(|| rejected("missing future account attempt"))?;
        let slot = held.slot;
        let credential = held.credential.clone();
        let (marker, access) = self.account_source(&slot, &credential)?;
        let held = self
            .account
            .as_mut()
            .ok_or_else(|| rejected("missing future account attempt"))?;
        if marker.marker_file_digest() != &held.marker_file_digest || access != held.access {
            return Err(rejected("future account source changed"));
        }
        let mut nonce = [0; 32];
        rand::rngs::SysRng
            .try_fill_bytes(&mut nonce)
            .map_err(|_| ProviderError::Unavailable(KagemushaWalletUnavailableV1::Platform(0)))?;
        if nonce == [0; 32] {
            return Err(rejected("future account nonce"));
        }
        let open = intake::open_message(
            &nonce,
            self.parameters.manifest_digest(),
            &held.slot,
            &held.marker_file_digest,
            std::array::from_fn(|i| held.originals[i].as_slice()),
        );
        held.challenge = match held.access {
            Access::Monetary => open
                .try_into()
                .map_err(|_| rejected("future account challenge"))?,
            Access::HistoricalOutput => {
                crate::kagemusha_wallet_advance_v1::kagemusha_wallet_provider_digest_v1(
                    "spartan-historical-output-account",
                    &open,
                )
            }
        };
        Ok(held.challenge)
    }

    /// Authorize the same existing Ed25519 account and recheck the exact source
    /// and positive hardware key. A proven loss of both Released completion copies
    /// permits only ownerless exact historical output reads on the pinned marker.
    ///
    /// # Errors
    /// Refusal preserves the original challenge, source pin and provider owner.
    pub fn authorize_account(&mut self, signature: &[u8]) -> Result<[u8; 32], ProviderError> {
        let held = self
            .account
            .as_ref()
            .ok_or_else(|| rejected("future account begin required"))?;
        if held.challenge == [0; 32] {
            return Err(rejected("future account challenge missing"));
        }
        intake::authorize_account(&held.account, &held.challenge, signature)
            .map_err(original_error)?;
        if held.authorized {
            return Ok(held.slot.0);
        }
        let credential = held.credential.clone();
        let slot = held.slot;
        let digest = held.marker_file_digest;
        let expected_access = held.access;
        let (marker, access) = self.account_source(&slot, &credential)?;
        if marker.marker_file_digest() != &digest || access != expected_access {
            return Err(rejected("future account source changed"));
        }
        self.account
            .as_mut()
            .ok_or_else(|| rejected("future account attempt lost"))?
            .authorized = true;
        Ok(slot.0)
    }

    fn admitted_account(&self, slot: &Slot) -> Result<&AccountAdmission, ProviderError> {
        self.account
            .as_ref()
            .filter(|held| held.authorized && &held.slot == slot)
            .ok_or_else(|| rejected("future account authorization required"))
    }

    pub(super) fn require_account(
        &self,
        slot: &Slot,
        credential: Option<&native::Credential>,
    ) -> Result<(), ProviderError> {
        let held = self.admitted_account(slot)?;
        if held.access != Access::Monetary {
            return Err(rejected("future account restricted to retained outputs"));
        }
        if credential.is_some_and(|credential| !same_credential(credential, &held.credential)) {
            return Err(rejected("future account credential changed"));
        }
        Ok(())
    }
    /// Whether this authenticated slot permits only retained output reads.
    ///
    /// # Errors
    /// Requires a completed account challenge for the same slot.
    pub fn retained_outputs_only(&self, slot: &Slot) -> Result<bool, ProviderError> {
        Ok(self.lookup_marker(slot)?.is_some())
    }

    pub(super) fn lookup_marker(&self, slot: &Slot) -> Result<Option<[u8; 32]>, ProviderError> {
        let held = self.admitted_account(slot)?;
        Ok((held.access == Access::HistoricalOutput).then_some(held.marker_file_digest))
    }

    /// Decimal precision authenticated by the enrolled credential's complete canonical asset scope.
    ///
    /// # Errors
    /// Refuses a foreign or historical-only account, wrong asset, malformed original or scale.
    pub fn reviewed_asset_scale(
        &self,
        slot: &Slot,
        capsule: &RecoveryCapsule,
    ) -> Result<u8, ProviderError> {
        self.require_account_wallet(capsule.wallet())?;
        self.account_asset_scale(slot, capsule.asset())
    }

    pub(super) fn account_asset_scale(&self, slot: &Slot, asset: &[u8; 32]) -> Result<u8, ProviderError> {
        let held = self.admitted_account(slot)?;
        let scope =
            intake::asset_scope(&held.originals[3], asset).map_err(original_error)?;
        u8::try_from(scope.scale).map_err(|_| rejected("future reviewed asset scale"))
    }
    pub(super) fn authorized_wallet_id(&self, slot: &Slot) -> Result<[u8; 32], ProviderError> {
        let held = self.admitted_account(slot)?;
        Ok(held.credential.body[66..98]
            .try_into()
            .expect("fixed credential field"))
    }
    pub(super) fn admitted_asset_digest(&self, slot: &Slot) -> Result<[u8; 32], ProviderError> {
        Ok(self.admitted_account(slot)?.credential.body[34..66]
            .try_into()
            .expect("fixed credential asset field"))
    }
    pub(super) fn payout_account(&self, slot: &Slot) -> Result<AccountId, ProviderError> {
        self.require_account(slot, None)?;
        Ok(self.admitted_account(slot)?.account.clone())
    }
    pub(super) fn bootstrap_operation_id(&self, slot: &Slot) -> Result<[u8; 32], ProviderError> {
        self.require_account(slot, None)?;
        Ok(self.admitted_account(slot)?.credential.bootstrap_operation_id())
    }
    pub(super) fn request_account_originals(
        &self,
        slot: &Slot,
    ) -> Result<[Vec<u8>; 3], ProviderError> {
        self.require_account(slot, None)?;
        let held = self.admitted_account(slot)?;
        Ok(std::array::from_fn(|i| held.originals[i].clone()))
    }
    /// Exact account, asset scope and Enrollment certificate used to verify the admitted Bootstrap.
    pub(super) fn activation_companions(&self, slot: &Slot) -> Result<[Vec<u8>; 3], ProviderError> {
        self.require_account(slot, None)?;
        let held = self.admitted_account(slot)?;
        Ok([
            held.originals[2].clone(),
            held.originals[3].clone(),
            held.originals[1].clone(),
        ])
    }
    pub(super) fn admitted_wallet_slot(&self, wallet: &[u8; 32]) -> Result<Slot, ProviderError> {
        self.account.as_ref()
            .filter(|held| held.authorized && held.access == Access::Monetary && held.credential.body[66..98] == *wallet)
            .map(|held| held.slot.clone())
            .ok_or_else(|| rejected("future account wallet authorization required"))
    }
    pub(super) fn require_account_wallet(&self, wallet: &[u8; 32]) -> Result<(), ProviderError> {
        self.admitted_wallet_slot(wallet).map(|_| ())
    }

    /// Verify a complete E6 against the retained account-authorized E5 of `slot` with the
    /// shared enrollment verifier, then require the offered bytes to be the exact durable
    /// winner that enrollment retained. `credential` is already authenticated under the
    /// admitted parameters and the selected issuer, and `marker` is the source marker
    /// `account_source` just verified.
    ///
    /// Everything here is a read: nothing is reconciled, adopted, written or signed to make
    /// the comparison, so a selected head stays pending and a retained-output-only source
    /// (its Released completion lost) is answered under that same marker. A lost winner is
    /// unavailable custody data, never permission to admit the offered bytes; a legitimate
    /// original is byte-identical to its winner and is admitted exactly as before.
    fn require_enrollment_result(
        &self,
        slot: &Slot,
        marker: &KagemushaWalletMarkerRecordV1,
        credential: &native::Credential,
        offered: &EnrollmentResult<'_>,
        account: &AccountId,
        asset: &[u8],
    ) -> Result<(), ProviderError> {
        let fields = credential.enrollment_fields().map_err(provider_error)?;
        let record = self
            .provider
            .enrollment_record(slot)?
            .ok_or_else(|| rejected("future account enrollment request"))?;
        let request = RequestV1::decode(&record.request)
            .map_err(|_| rejected("future account retained enrollment request"))?;
        let intent = self
            .provider
            .read_intent(slot)?
            .ok_or_else(|| rejected("future account enrollment intent"))?;
        let window = intent.dates.issued_at_ms..intent.dates.expires_at_ms;
        if request.body.challenge != intent.challenge
            || request.body.account != *account
            || norito::encode_canonical(&request.body.asset).ok().as_deref() != Some(asset)
            || !window.contains(&fields.issued_at_ms)
            || !window.contains(&fields.enrollment_evidence.time_ms)
        {
            return Err(rejected("future account enrollment request binding"));
        }
        offered
            .result
            .verify_initial_fields(&self.scheme, offered.issuer, &request, &fields)
            .map_err(|_| rejected("future account enrollment result"))?;
        // The authenticated retained E5 alone names the winner record. A different valid
        // encoding of the same credential (for example a larger certificate set) is not the
        // selected original.
        let winner = self
            .provider
            .enrollment_result_at_output_marker(slot, marker, &record.request)?
            .ok_or(ProviderError::UnavailableCustodyData {
                object: "selected enrollment result",
            })?;
        if winner.as_slice() != offered.original {
            return Err(rejected("future account enrollment result original"));
        }
        Ok(())
    }

    fn select_account_source(
        &mut self,
        credential: &native::Credential,
    ) -> Result<(Slot, KagemushaWalletMarkerRecordV1, Access), ProviderError> {
        let mut selected = None;
        for slot in self.provider.slots()? {
            if let Some(intent) = self.provider.read_intent_for_enrollment_search(&slot)?
                && intent.challenge.scheme_id != *self.provider.scheme_id()
            {
                continue;
            }
            let status = self.provider.output_source(&slot)?;
            if status
                .marker()
                .is_some_and(|m| m.marker().wallet_id == credential.body[66..98])
            {
                if selected.is_some() {
                    return Err(rejected("ambiguous future account custody"));
                }
                let (marker, access) = self.account_source(&slot, credential)?;
                selected = Some((slot, marker, access));
            }
        }
        selected.ok_or_else(|| rejected("future account has no enrolled custody"))
    }

    fn account_source(
        &mut self,
        slot: &Slot,
        credential: &native::Credential,
    ) -> Result<(KagemushaWalletMarkerRecordV1, Access), ProviderError> {
        let (record, access) = match self.provider.output_source(slot)? {
            OutputSourceV1::Reconciled(
                Status::Enrollment(record) | Status::Pending(record) | Status::Released(record),
            ) => (record, Access::Monetary),
            OutputSourceV1::ReleasedWithoutCompletion(record) => (record, Access::HistoricalOutput),
            OutputSourceV1::Reconciled(Status::Terminal(_)) => return Err(ProviderError::Terminal),
            _ => return Err(rejected("future account enrolled marker")),
        };
        let body = &credential.body;
        let key = KagemushaDevicePublicKeyV1::try_from(&body[130..195])
            .map_err(|_| rejected("future account payment key"))?;
        let intent = self
            .provider
            .read_intent(slot)?
            .ok_or_else(|| rejected("future account enrollment intent"))?;
        self.provider.require_enrollment_intent_originals(
            slot,
            &intent,
            Some(self.parameters.manifest_digest()),
        )?;
        let challenge = &intent.challenge;
        challenge
            .validate()
            .map_err(|_| rejected("future account enrollment challenge"))?;
        let marker = record.marker();
        if challenge.scheme_id != body[2..34]
            || challenge.asset_digest != body[34..66]
            || challenge.account_digest != body[98..130]
            || challenge.app_policy != body[404..436]
            || challenge.enrollment_id(&key) != body[448..480]
            || challenge.wallet_id(&key) != body[66..98]
            || marker.scheme_id != body[2..34]
            || marker.asset_digest != body[34..66]
            || marker.wallet_id != body[66..98]
            || marker.payment_key != key
        {
            return Err(rejected("future account retained owner key intent"));
        }
        let enrollment = self
            .provider
            .enrollment_record(slot)?
            .ok_or_else(|| rejected("future account enrollment request"))?;
        let original_marker = KagemushaWalletMarkerV1::enrollment(challenge, key)
            .map_err(|_| rejected("future account enrollment marker"))?;
        if enrollment.request.is_empty()
            || enrollment.enrollment_marker_digest
                != original_marker
                    .marker_digest()
                    .map_err(|_| rejected("future account enrollment marker digest"))?
        {
            return Err(rejected("future account retained enrollment"));
        }
        match self.provider.probe_payment_key(slot)? {
            Probe::Present(actual) if actual == key => {}
            Probe::Unavailable(reason) => return Err(ProviderError::Unavailable(reason)),
            _ => return Err(ProviderError::KeyLost),
        }
        if record.head().is_some() {
            let capsule = self.provider.capsule_at_output_marker(slot, &record)?;
            capsule.require_marker(record.marker())?;
            let lookup = match access {
                Access::Monetary => self.provider.lookup(slot, capsule.operation_id())?,
                Access::HistoricalOutput => self.provider.lookup_after_completion_loss(
                    slot,
                    record.marker_file_digest(),
                    capsule.operation_id(),
                )?,
            };
            let prepared = match (lookup, access) {
                (Lookup::Retained(retained), _) => {
                    let (prepared, completed) = self.verify_retained_release(slot, &capsule, &retained)?;
                    if !same_credential(completed.head().credential(), credential) {
                        return Err(rejected("future completed source credential"));
                    }
                    prepared
                }
                (Lookup::SelectedUnsigned { .. }, Access::Monetary)
                | (Lookup::DeliveryDataLoss, Access::HistoricalOutput) => {
                    Arc::new(
                        self.engine_for_capsule(slot, &capsule)?
                            .resume(&capsule, &self.parameters)
                            .map_err(provider_error)?,
                    )
                }
                _ => return Err(rejected("future selected source unavailable")),
            };
            if !same_credential(prepared.credential(), credential) {
                return Err(rejected("future selected source credential"));
            }
        } else {
            let original = [credential.body.as_slice(), credential.signature.as_slice()].concat();
            if body[488..492] != [0; 4]
                || self.provider.credential(slot, 0)?.as_deref() != Some(original.as_slice())
            {
                return Err(rejected("future retained enrollment credential"));
            }
        }
        if self
            .provider
            .output_source(slot)?
            .marker()
            .map(|m| m.marker_file_digest())
            != Some(record.marker_file_digest())
        {
            return Err(rejected("future account source changed"));
        }
        Ok((record, access))
    }
}
