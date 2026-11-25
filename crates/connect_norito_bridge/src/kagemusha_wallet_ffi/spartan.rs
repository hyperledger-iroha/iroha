//! Authenticated experimental backend on the installed runtime's exclusive provider.
use super::*;
use iroha_core_zk::kagemusha_spartan_native::{FutureRuntime, native};

#[cfg(any(test, target_os = "ios"))]
mod apple_confirmation;
#[cfg(feature = "spartan-device-fixture")]
pub mod device_fixture;
mod direct_time;
mod settlement;
pub use settlement::{
    connect_norito_kagemusha_spartan_activation_settlement_v1,
    connect_norito_kagemusha_spartan_epoch_v1,
    connect_norito_kagemusha_spartan_unload_settlement_v1,
};
mod session_state;
use session_state::{InstalledOriginals, ReviewCache};

/// The exact canonical asset original and Enrollment certificate of a signed selection.
/// Only an installed owner constructs it, from its own authenticated selection.
#[derive(Clone, Copy)]
pub(crate) struct SelectedEnrollment<'a> {
    pub(super) asset: &'a [u8],
    pub(super) issuer: &'a iroha_data_model::kagemusha::KagemushaWalletSignerCertificateV1,
}

pub(crate) enum Action<'a> {
    Install {
        bundle: &'a [u8],
        manifest: &'a [u8],
        signature: &'a [u8; 64],
    },
    /// Buffer-free open of the programs an owner authenticated from its own installed
    /// carrier. It carries no original, so it can select or install nothing.
    OpenInstalled,
    Prepare {
        slot: [u8; 32],
        witness: &'a [u8],
        head: &'a [u8],
    },
    InstallBlacklist {
        slot: [u8; 32],
        original: &'a [u8],
        certificates: &'a [u8],
    },
    IssueOpenRequest {
        slot: [u8; 32],
        id: [u8; 32],
        amount: u128,
        fee: &'a [u8],
        issuer: &'a [u8],
    },
    UnloadSettlement {
        slot: [u8; 32],
        operation: [u8; 32],
        signed: &'a [u8],
        proof: &'a [u8],
    },
    ActivationSettlement {
        slot: [u8; 32],
        signed: &'a [u8],
        proof: &'a [u8],
    },
    Epoch {
        slot: [u8; 32],
        expected: u64,
        boundary: &'a [u8],
    },
    DirectTime {
        slot: [u8; 32],
        selector: u32,
        token: u64,
        first: &'a [u8],
        second: &'a [u8],
    },
    BeginAccount([&'a [u8]; 4]),
    /// Account admission from one complete E6 delivery and the existing account original.
    /// A foreign boundary always leaves `selected` empty: only the signed selected owner
    /// supplies its canonical asset original and Enrollment certificate.
    BeginAccountResult {
        result: &'a [u8],
        account: &'a [u8],
        selected: Option<SelectedEnrollment<'a>>,
    },
    AuthorizeAccount(&'a [u8]),
    Confirm {
        token: u64,
        message: &'a [u8],
        der: &'a [u8],
    },
    #[cfg(target_os = "ios")]
    ApplePrompt {
        token: u64,
        message: &'a [u8; 32],
        update: apple_confirmation::Update<'a>,
    },
    Execute(u64),
    Output {
        slot: [u8; 32],
        operation: [u8; 32],
        part: u32,
    },
    VerifyPayment(&'a [u8]),
    CreditStatus {
        slot: [u8; 32],
        selector: u32,
        operation: [u8; 32],
        request: &'a [u8],
        status: &'a [u8],
    },

    Discard(u64),
}
impl From<native::Error> for Failure {
    fn from(error: native::Error) -> Self {
        Self::code(match error {
            native::Error::ResourceLimit => RESOURCE,
            native::Error::NotEnoughMemory { .. } => NOT_ENOUGH_MEMORY,
            native::Error::Unavailable(_) => UNAVAILABLE,
            native::Error::Rejected(_) => PROOF_REJECTED,
            native::Error::AuthorityMismatch(_) | native::Error::Malformed(_) => INVALID,
        })
    }
}
pub(super) trait FutureSession: Send {
    fn call(&mut self, action: Action<'_>) -> Result<Response>;
}
struct Review {
    slot: advance::KagemushaWalletSlotIdV1,
    capsule: native::RecoveryCapsule,
    digest: [u8; 32],
    bytes: Vec<u8>,
    last_confirmation: Option<([u8; 32], Vec<u8>)>,
}
pub(super) struct Session<P, S> {
    runtime: FutureRuntime<advance::KagemushaWalletStdFsV1, P, S>,
    installed: InstalledOriginals,
    reviews: ReviewCache<Review>,
    times: review::Tokens<state::DirectTimeExchangeV1>,
    #[cfg(target_os = "ios")]
    apple_prompt: Option<apple_confirmation::Prompt>,
}
impl<P: advance::KagemushaWalletPlatformV1 + 'static, S> Session<P, S> {
    pub(super) fn new(
        runtime: FutureRuntime<advance::KagemushaWalletStdFsV1, P, S>,
        bundle: &[u8],
        manifest: &[u8],
        signature: &[u8; 64],
    ) -> Self {
        let installed =
            InstalledOriginals::authenticated(*runtime.parameter_id(), bundle, manifest, signature);
        Self {
            runtime,
            installed,
            reviews: ReviewCache::default(),
            times: review::Tokens::default(),
            #[cfg(target_os = "ios")]
            apple_prompt: None,
        }
    }
}
fn response(kind: i32, bytes: Vec<u8>) -> Response {
    Response {
        kind,
        bytes,
        ..Response::default()
    }
}
impl<P: advance::KagemushaWalletPlatformV1 + 'static, S: Send> FutureSession for Session<P, S> {
    fn call(&mut self, action: Action<'_>) -> Result<Response> {
        match action {
            Action::Install {
                bundle,
                manifest,
                signature,
            } => self.installed.retry(bundle, manifest, signature),
            // Only the owner that authenticated its own installed carrier answers the
            // buffer-free open, and only before its session exists. A session, however its
            // programs were installed, never answers it.
            Action::OpenInstalled => Err(Failure::code(CONFLICT)),
            Action::Prepare {
                slot,
                witness,
                head,
            } => {
                let slot = advance::KagemushaWalletSlotIdV1(slot);
                let prepared = self.runtime.prepare(&slot, witness, head)?;
                if let Some((token, old)) = self
                    .reviews
                    .find(|r| r.slot == slot && &r.digest == prepared.capsule_digest())
                {
                    return Ok(Response {
                        kind: 82,
                        sequence: u128::from(token),
                        bytes: old.bytes.clone(),
                        ..Response::default()
                    });
                }
                let bytes = prepared.review_bytes()?;
                let token = self.reviews.insert(Review {
                    slot,
                    capsule: prepared.capsule().clone(),
                    digest: *prepared.capsule_digest(),
                    bytes: bytes.clone(),
                    last_confirmation: None,
                })?;
                Ok(Response {
                    kind: 82,
                    sequence: u128::from(token),
                    bytes,
                    ..Response::default()
                })
            }
            Action::CreditStatus {
                slot,
                selector,
                operation,
                request,
                status,
            } => {
                let slot = advance::KagemushaWalletSlotIdV1(slot);
                match selector {
                    1 if status.is_empty() => {
                        let id = request.try_into().map_err(|_| Failure::code(INVALID))?;
                        Ok(response(
                            104,
                            self.runtime.credit_status_peer(&slot, id, &operation)?,
                        ))
                    }
                    3 if !status.is_empty() => {
                        let id = request.try_into().map_err(|_| Failure::code(INVALID))?;
                        let proof = self
                            .runtime
                            .credit_status_for_payment(&slot, id, &operation, status)?;
                        Ok(response(104, proof))
                    }
                    2 if !status.is_empty() => {
                        let proof = self
                            .runtime
                            .verify_sent_credit_status(&slot, &operation, request, status)?;
                        Ok(response(
                            105,
                            vec![u8::from(proof == native::VerifiedCreditStatus::Credited)],
                        ))
                    }
                    _ => Err(Failure::code(INVALID)),
                }
            }
            Action::InstallBlacklist {
                slot,
                original,
                certificates,
            } => {
                let digest = self.runtime.install_local_blacklist(
                    &advance::KagemushaWalletSlotIdV1(slot),
                    original,
                    certificates,
                )?;
                Ok(response(95, digest.to_vec()))
            }
            Action::UnloadSettlement {
                slot,
                operation,
                signed,
                proof,
            } => {
                let result = self.runtime.unload_settlement(
                    &advance::KagemushaWalletSlotIdV1(slot),
                    &operation,
                    signed,
                    proof,
                )?;
                Ok(response(102, settlement::outcome_bytes(&result)))
            }
            Action::ActivationSettlement {
                slot,
                signed,
                proof,
            } => {
                let slot = advance::KagemushaWalletSlotIdV1(slot);
                let result = self.runtime.activation_settlement(&slot, signed, proof)?;
                Ok(response(108, settlement::outcome_bytes(&result)))
            }
            Action::Epoch {
                slot,
                expected,
                boundary,
            } => {
                let slot = advance::KagemushaWalletSlotIdV1(slot);
                let progress = if boundary.is_empty() && expected == 0 {
                    self.runtime.commit_epoch_progress(&slot)?
                } else {
                    self.runtime
                        .sync_epoch_boundary(&slot, expected, boundary)?
                };
                let mut bytes = Vec::with_capacity(24);
                for word in [
                    progress.epoch,
                    progress.first_height,
                    progress.boundary_height,
                ] {
                    bytes.extend_from_slice(&word.to_le_bytes());
                }
                Ok(response(103, bytes))
            }
            Action::IssueOpenRequest {
                slot,
                id,
                amount,
                fee,
                issuer,
            } => Ok(response(
                96,
                self.runtime.issue_open_request(
                    &advance::KagemushaWalletSlotIdV1(slot),
                    id,
                    amount,
                    fee,
                    issuer,
                )?,
            )),
            Action::DirectTime {
                slot,
                selector,
                token,
                first,
                second,
            } => self.direct_time(
                advance::KagemushaWalletSlotIdV1(slot),
                selector,
                token,
                first,
                second,
            ),
            Action::BeginAccount([credential, certificate, account, asset]) => {
                let challenge =
                    self.runtime
                        .begin_account(credential, certificate, account, asset)?;
                Ok(response(15, challenge.to_vec()))
            }
            Action::BeginAccountResult {
                result,
                account,
                selected,
            } => {
                // Without a signed selection there is no asset or issuer to verify against;
                // none is taken from the caller, and nothing is retained.
                let selected = selected.ok_or(Failure::code(INVALID))?;
                let challenge = self.runtime.begin_account_result(
                    result,
                    account,
                    selected.asset,
                    selected.issuer,
                )?;
                Ok(response(15, challenge.to_vec()))
            }
            Action::AuthorizeAccount(signature) => {
                let slot = self.runtime.authorize_account(signature)?;
                let mut bytes = self.runtime.parameter_id().to_vec();
                bytes.extend_from_slice(&slot);
                let restricted = self
                    .runtime
                    .retained_outputs_only(&advance::KagemushaWalletSlotIdV1(slot))?;
                Ok(response(if restricted { 87 } else { 85 }, bytes))
            }
            #[cfg(target_os = "ios")]
            Action::ApplePrompt {
                token,
                message,
                update,
            } => self.apple_prompt(token, message, update),
            Action::Confirm {
                token,
                message,
                der,
            } => {
                let review = self.reviews.active(token)?;
                if message.is_empty() && der == [2] && matches!(review.capsule.kind(), 3 | 6) {
                    let scale = self
                        .runtime
                        .reviewed_asset_scale(&review.slot, &review.capsule)?;
                    let mut asset = review.capsule.asset().to_vec();
                    asset.push(scale);
                    return Ok(response(94, asset));
                }
                #[cfg(target_os = "ios")]
                if !(der.is_empty() || (message.is_empty() && (der == [1] || der == [2]))) {
                    return Err(Failure::code(INVALID));
                }
                #[cfg(target_os = "ios")]
                if message.is_empty() && der == [1] {
                    self.runtime
                        .prepare_apple_debit_confirmation_key(&review.slot, &review.capsule)?;
                    return Ok(response(93, Vec::new()));
                }
                if message.is_empty() && der.is_empty() {
                    #[cfg(target_os = "ios")]
                    if self.apple_prompt.is_some() {
                        return Err(Failure::unavailable(
                            UNAVAILABLE,
                            advance::KagemushaWalletUnavailableV1::Busy,
                        ));
                    }
                    let message = self
                        .runtime
                        .begin_debit_confirmation(&review.slot, &review.capsule)?;
                    review.last_confirmation = None;
                    return Ok(response(88, message.to_vec()));
                }
                let message: &[u8; 32] = message.try_into().map_err(|_| Failure::code(INVALID))?;
                if der.is_empty() {
                    if review
                        .last_confirmation
                        .as_ref()
                        .is_some_and(|(held, _)| held == message)
                    {
                        review.last_confirmation = None;
                    }
                    #[cfg(target_os = "ios")]
                    if self
                        .apple_prompt
                        .as_ref()
                        .is_some_and(|p| p.matches(token, message))
                    {
                        self.apple_prompt = None;
                    }
                    self.runtime.cancel_debit_confirmation(
                        &review.slot,
                        review.capsule.operation_id(),
                        message,
                    );
                    return Ok(response(84, Vec::new()));
                }
                if !(8..=72).contains(&der.len()) {
                    return Err(Failure::code(INVALID));
                }
                // Cache only exact acknowledged evidence. A repeated FFI delivery never
                // creates fresh authority or invokes the signer again.
                if !review
                    .last_confirmation
                    .as_ref()
                    .is_some_and(|(held, signed)| held == message && signed == der)
                {
                    self.runtime.retain_signed_debit_confirmation(
                        &review.slot,
                        &review.capsule,
                        message,
                        der,
                    )?;
                    review.last_confirmation = Some((*message, der.to_vec()));
                }
                Ok(response(89, Vec::new()))
            }
            Action::Execute(token) => {
                let runtime = &mut self.runtime;
                self.reviews.execute(token, |review| {
                    let prepared = runtime.resume(&review.capsule)?;
                    let result = runtime.advance(&review.slot, prepared)?;
                    use advance::KagemushaWalletAdvanceOutcomeV1 as O;
                    Ok(match result {
                        O::Released { retained, .. } => response(1, retained.record.encode()?),
                        O::Pending { .. } => response(2, Vec::new()),
                        O::NotPerformed(reason) => Response {
                            kind: 3,
                            detail: match reason {
                                advance::KagemushaWalletNotPerformedV1::StaleHead => 0,
                                advance::KagemushaWalletNotPerformedV1::CapacityWait => 1,
                                advance::KagemushaWalletNotPerformedV1::Invalid { .. } => 2,
                            },
                            ..Response::default()
                        },
                        O::Archived(_) => response(4, Vec::new()),
                        O::DeliveryDataLoss { .. } => response(5, Vec::new()),
                    })
                })
            }
            Action::Output {
                slot,
                operation,
                part,
            } => {
                if part == 5 && operation == [0; 32] {
                    let bytes = self
                        .runtime
                        .issue_activation(&advance::KagemushaWalletSlotIdV1(slot))?;
                    return Ok(response(107, bytes));
                }
                if part == 4 && operation == [0; 32] {
                    let ordinal = self
                        .runtime
                        .next_load_ordinal(&advance::KagemushaWalletSlotIdV1(slot))?;
                    return Ok(response(106, ordinal.to_le_bytes().to_vec()));
                }
                if part == 3 && operation == [0; 32] {
                    let bytes = self
                        .runtime
                        .balance_snapshot(&advance::KagemushaWalletSlotIdV1(slot))?;
                    return Ok(response(101, bytes));
                }
                if part > 3 {
                    return Err(Failure::code(INVALID));
                }
                use advance::KagemushaWalletLookupV1 as L;
                Ok(
                    match self
                        .runtime
                        .lookup(&advance::KagemushaWalletSlotIdV1(slot), &operation)?
                    {
                        L::Retained(retained) => response(1, retained.record.output_part(part)?),
                        L::SelectedUnsigned { .. } => response(2, Vec::new()),
                        L::Unknown => response(0, Vec::new()),
                        L::Archived(_) => response(4, Vec::new()),
                        L::DeliveryDataLoss => response(5, Vec::new()),
                    },
                )
            }
            Action::VerifyPayment(bytes) => {
                let payment = self.runtime.verify_payment(bytes)?;
                let mut projection = Vec::with_capacity(514);
                projection.extend_from_slice(&payment.digest_bytes());
                projection.extend_from_slice(&payment.request().body);
                projection.extend_from_slice(&payment.request().signature);
                Ok(response(83, projection))
            }
            Action::Discard(token) => {
                #[cfg(target_os = "ios")]
                if self.apple_prompt.as_ref().is_some_and(|p| p.token == token) {
                    self.apple_prompt = None;
                }
                // Release volatile preparation only; durable custody stays with the provider.
                if let Some(removed) = self.reviews.discard(token)
                    && self
                        .reviews
                        .find(|other| {
                            other.slot == removed.slot
                                && other.capsule.operation_id() == removed.capsule.operation_id()
                        })
                        .is_none()
                {
                    self.runtime
                        .discard_debit_confirmation(&removed.slot, removed.capsule.operation_id());
                    self.runtime
                        .abandon_send_capture(&removed.slot, removed.capsule.operation_id())?;
                }
                Ok(response(84, Vec::new()))
            }
        }
    }
}
pub(crate) fn call(handle: u64, action: Action<'_>) -> Result<Response> {
    let owner = open::runtime(handle)?;
    let mut guard = owner
        .admission
        .lock()
        .map_err(|_| Failure::code(INTERNAL))?;
    owner.closing.require_open()?;
    if owner
        .finished
        .lock()
        .map_err(|_| Failure::code(INTERNAL))?
        .is_some()
    {
        return Err(Failure::code(CONFLICT));
    }
    let admission = guard.as_deref_mut().ok_or(Failure::code(CLOSED))?;
    let mut result = custody_transaction(|| admission.spartan(action))?;
    if matches!(result.kind, 15 | 81 | 85 | 87) {
        result.sequence = u128::from(handle);
    }
    Ok(result)
}
unsafe fn fixed32(pointer: *const u8) -> Result<[u8; 32]> {
    unsafe { exports::input(pointer, 32, 32) }?
        .try_into()
        .map_err(|_| Failure::code(INVALID))
}
/// The one install entry has exactly two argument forms. All three originals absent is the
/// buffer-free open of the programs the owner authenticated from its installed carrier. All
/// three present is a raw offer, which an installed owner only compares. Every partial
/// tuple, and a signature that is not 64 bytes, is refused before any owner is looked up.
pub(crate) fn install_action<'a>(
    bundle: &'a [u8],
    manifest: &'a [u8],
    signature: &'a [u8],
) -> Result<Action<'a>> {
    match [bundle, manifest, signature].map(<[u8]>::is_empty) {
        [true, true, true] => Ok(Action::OpenInstalled),
        [false, false, false] => Ok(Action::Install {
            bundle,
            manifest,
            signature: signature.try_into().map_err(|_| Failure::code(INVALID))?,
        }),
        _ => Err(Failure::code(INVALID)),
    }
}
/// Open the programs this owner authenticated from its installed carrier, or compare a raw
/// offer with them. All three originals absent (null, zero length) is the buffer-free open:
/// only an owner of the signed Spartan profile answers 81 with its 32-byte parameter
/// identity, and every other owner refuses and stays unchanged. All three present are
/// compared with the installed originals; outside isolated fixture builds they select and
/// install nothing. Every partial tuple is `INVALID`.
/// # Safety
/// Inputs address their stated lengths; `out` is writable and aligned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_install_v1(
    handle: u64,
    bundle: *const u8,
    bundle_len: usize,
    manifest: *const u8,
    manifest_len: usize,
    signature: *const u8,
    signature_len: usize,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        exports::output(out, || {
            // An absent original is a null pointer with zero length. A pointer never
            // accompanies a zero length, so no offered buffer can pose as the absent form.
            if [
                (bundle, bundle_len),
                (manifest, manifest_len),
                (signature, signature_len),
            ]
            .iter()
            .any(|(pointer, length)| *length == 0 && !pointer.is_null())
            {
                return Err(Failure::code(INVALID));
            }
            call(
                handle,
                install_action(
                    exports::input(bundle, bundle_len, native::MAX_PROGRAM_BUNDLE_BYTES)?,
                    exports::input(manifest, manifest_len, 16384)?,
                    exports::input(signature, signature_len, 64)?,
                )?,
            )
        })
    }
}
// Argument-form and ownership boundary only. No fixture here holds programs, a carrier, a
// provider or a proof verdict, so none of these calls can reach an installed session.
#[cfg(test)]
mod install_form_tests {
    use super::*;

    type Original = (*const u8, usize);

    fn install(handle: u64, originals: [Original; 3]) -> i32 {
        let [
            (bundle, bundle_len),
            (manifest, manifest_len),
            (signature, signature_len),
        ] = originals;
        let mut out = WalletResult::default();
        let status = unsafe {
            connect_norito_kagemusha_spartan_install_v1(
                handle,
                bundle,
                bundle_len,
                manifest,
                manifest_len,
                signature,
                signature_len,
                &mut out,
            )
        };
        assert_eq!(status, out.status);
        assert!(out.bytes.is_null() && out.length == 0);
        status
    }

    #[test]
    fn install_entry_has_exactly_the_absent_and_the_complete_argument_forms() {
        let signature = [5; 64];
        assert!(matches!(
            install_action(&[], &[], &[]),
            Ok(Action::OpenInstalled)
        ));
        assert!(matches!(
            install_action(&[1], &[2], &signature),
            Ok(Action::Install {
                bundle: [1],
                manifest: [2],
                signature: held,
            }) if *held == signature
        ));
        let refused: [(&[u8], &[u8], &[u8]); 8] = [
            (&[1], &[], &[]),
            (&[], &[2], &[]),
            (&[], &[], &signature),
            (&[1], &[2], &[]),
            (&[1], &[], &signature),
            (&[], &[2], &signature),
            (&[1], &[2], &signature[..63]),
            (&[1], &[2], &[5; 65]),
        ];
        for (bundle, manifest, signature) in refused {
            assert_eq!(
                install_action(bundle, manifest, signature).err(),
                Some(Failure::code(INVALID))
            );
        }
    }

    #[test]
    fn neither_form_switches_an_owner_without_carrier_authenticated_programs() {
        let drops = Arc::new(Mutex::new(Vec::new()));
        let owner = open::installation_data::owner(&drops);
        let handle = open::register_owned_runtime(registry(), &owner).unwrap();
        let bytes = [7_u8; 64];
        let absent: Original = (std::ptr::null(), 0);
        let present = |length: usize| -> Original { (bytes.as_ptr(), length) };
        // Nothing was authenticated from a carrier on this owner: there is nothing to open,
        // and a complete raw offer installs nothing either.
        assert_eq!(install(handle, [absent; 3]), ARTIFACTS_UNAVAILABLE);
        assert_eq!(
            install(handle, [present(1), present(1), present(64)]),
            ARTIFACTS_UNAVAILABLE
        );
        // A pointer with zero length is not the absent form, and no partial tuple is either.
        for originals in [
            [present(0), absent, absent],
            [absent, present(0), absent],
            [absent, absent, present(0)],
            [present(0); 3],
            [present(1), absent, absent],
            [absent, present(1), absent],
            [absent, absent, present(64)],
            [present(1), present(1), absent],
            [present(1), present(1), present(63)],
        ] {
            assert_eq!(install(handle, originals), INVALID);
        }
        // Every refusal left the same owner registered with its whole admission.
        assert!(owner.admission.lock().unwrap().is_some());
        assert!(drops.lock().unwrap().is_empty());
        close(handle).unwrap();
        assert_eq!(*drops.lock().unwrap(), vec![1, 2]);
        assert_eq!(install(handle, [absent; 3]), ARTIFACTS_UNAVAILABLE);
    }
}
/// Freeze a checked operation and return its native review token and projection.
/// # Safety
/// `slot` addresses 32 bytes; other inputs address stated lengths; `out` is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_prepare_v1(
    handle: u64,
    slot: *const u8,
    witness: *const u8,
    witness_len: usize,
    head: *const u8,
    head_len: usize,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        exports::output(out, || {
            call(
                handle,
                Action::Prepare {
                    slot: fixed32(slot)?,
                    witness: exports::input(witness, witness_len, native::MAX_RECOVERY_BYTES)?,
                    head: exports::input(head, head_len, native::MAX_OUTPUT_BYTES)?,
                },
            )
        })
    }
}
/// Execute or resume the exact reviewed capsule through hardware custody.
/// # Safety
/// `out` is writable and aligned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_execute_v1(
    handle: u64,
    token: u64,
    out: *mut WalletResult,
) -> i32 {
    unsafe { exports::output(out, || call(handle, Action::Execute(token))) }
}
/// Return the retained head (0), Payment (1), completion (2), or Unload claim (3).
/// With a zero operation ID, part 3 reads the authenticated current balance snapshot.
/// # Safety
/// `slot` and `operation` address 32 bytes each; `out` is writable and aligned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_output_v1(
    handle: u64,
    slot: *const u8,
    operation: *const u8,
    part: u32,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        exports::output(out, || {
            call(
                handle,
                Action::Output {
                    slot: fixed32(slot)?,
                    operation: fixed32(operation)?,
                    part,
                },
            )
        })
    }
}
/// Verify complete Payment and return its authenticated request projection.
/// # Safety
/// Input addresses its stated length; `out` is writable and aligned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_verify_payment_v1(
    handle: u64,
    bytes: *const u8,
    length: usize,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        exports::output(out, || {
            call(
                handle,
                Action::VerifyPayment(exports::input(bytes, length, native::MAX_OUTPUT_BYTES)?),
            )
        })
    }
}
/// Discard volatile preparation without deleting selected or released custody.
/// # Safety
/// `out` is writable and aligned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_discard_v1(
    handle: u64,
    token: u64,
    out: *mut WalletResult,
) -> i32 {
    unsafe { exports::output(out, || call(handle, Action::Discard(token))) }
}

/// Largest first original of the account-begin entry for the form its other originals
/// select: the complete E6 delivery when certificate and asset are both absent, otherwise the
/// exact Spartan credential. Presence alone selects the form; no byte of an original does.
pub(crate) fn begin_account_first_bound(certificate_len: usize, asset_len: usize) -> usize {
    if certificate_len == 0 && asset_len == 0 {
        iroha_core_zk::kagemusha_wallet_enrollment_v1::RESULT_MAX_BYTES
    } else {
        iroha_core_zk::kagemusha_wallet_enrollment_v1::SPARTAN_CREDENTIAL_BYTES_V1
    }
}
/// The one account-begin entry has exactly two argument forms. All four originals present
/// are the split credential, signer certificate, account and asset originals. A first
/// original and an account with certificate and asset both absent are one complete E6
/// delivery plus the existing account: the signed selected owner then supplies its own asset
/// and Enrollment certificate. Every other tuple is refused before any owner is looked up.
pub(crate) fn begin_account_action(originals: [&[u8]; 4]) -> Result<Action<'_>> {
    let [result, _, account, _] = originals;
    match originals.map(<[u8]>::is_empty) {
        [false, false, false, false] => Ok(Action::BeginAccount(originals)),
        [false, true, false, true] => Ok(Action::BeginAccountResult {
            result,
            account,
            selected: None,
        }),
        _ => Err(Failure::code(INVALID)),
    }
}

/// Begin actual account admission. Native derives the custody slot and head.
///
/// The request has exactly two argument forms. With all four originals present they are
/// the exact credential (596 bytes), signer certificate (172 bytes), canonical account and
/// asset scope. With `certificates` and `asset` both absent (null, zero length),
/// `credential` carries one complete E6 enrollment result and `account` the existing
/// account original: the signed selected owner supplies its own canonical asset and
/// Enrollment certificate, verifies that the delivered certificate set holds that issuer and
/// that the result binds the retained E5, hardware key and evidence, and only then retains
/// the admission. Both forms answer 15 with the fresh Ed25519 challenge; an exact retry
/// returns the retained challenge. Every other tuple is `INVALID`.
/// # Safety
/// Inputs address stated lengths and `out` is writable and aligned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_begin_account_v1(
    handle: u64,
    request: *const WalletOpenRequest,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        exports::output(out, || {
            let request = request.as_ref().ok_or(Failure::code(INVALID))?;
            // An absent original is a null pointer with zero length. A pointer never
            // accompanies a zero length, so no offered buffer can pose as the absent form.
            if [
                (request.credential, request.credential_length),
                (request.certificates, request.certificates_length),
                (request.account, request.account_length),
                (request.asset, request.asset_length),
            ]
            .iter()
            .any(|(pointer, length)| *length == 0 && !pointer.is_null())
            {
                return Err(Failure::code(INVALID));
            }
            call(
                handle,
                begin_account_action([
                    exports::input(
                        request.credential,
                        request.credential_length,
                        begin_account_first_bound(
                            request.certificates_length,
                            request.asset_length,
                        ),
                    )?,
                    exports::input(request.certificates, request.certificates_length, 172)?,
                    exports::input(request.account, request.account_length, open::BOUNDS[2])?,
                    exports::input(request.asset, request.asset_length, open::BOUNDS[3])?,
                ])?,
            )
        })
    }
}
// Argument-form boundary only. No fixture here holds programs, custody, an enrollment or a
// credential, so none of these calls can reach an account admission.
#[cfg(test)]
mod begin_account_form_tests {
    use super::*;

    #[test]
    fn begin_account_entry_has_exactly_the_split_and_the_complete_result_forms() {
        let credential = [1_u8; 596];
        let certificate = [2_u8; 172];
        assert!(matches!(
            begin_account_action([&credential, &certificate, b"account", b"asset"]),
            Ok(Action::BeginAccount([held, _, b"account", b"asset"])) if held == credential
        ));
        // A complete E6 with the account alone: no asset, issuer or selection is offered.
        assert!(matches!(
            begin_account_action([b"complete result", &[], b"account", &[]]),
            Ok(Action::BeginAccountResult {
                result: b"complete result",
                account: b"account",
                selected: None,
            })
        ));
        let refused: [[&[u8]; 4]; 8] = [
            [&[], &[], &[], &[]],
            [b"result", &[], &[], &[]],
            [&[], &[], b"account", &[]],
            [b"result", &certificate, b"account", &[]],
            [b"result", &[], b"account", b"asset"],
            [&[], &certificate, b"account", b"asset"],
            [&credential, &certificate, &[], b"asset"],
            [&credential, &certificate, b"account", &[]],
        ];
        for originals in refused {
            assert_eq!(
                begin_account_action(originals).err(),
                Some(Failure::code(INVALID))
            );
        }
    }

    #[test]
    fn only_the_complete_result_form_admits_more_than_the_exact_credential_length() {
        use iroha_core_zk::kagemusha_wallet_enrollment_v1::RESULT_MAX_BYTES;
        assert_eq!(begin_account_first_bound(0, 0), RESULT_MAX_BYTES);
        for (certificate, asset) in [(172, 1), (172, 0), (0, 1), (1, 1)] {
            assert_eq!(begin_account_first_bound(certificate, asset), 596);
        }
    }

    #[test]
    fn c_begin_account_refuses_partial_and_disguised_forms_before_any_owner() {
        let bytes = [7_u8; 596];
        let present = |length: usize| -> (*const u8, usize) { (bytes.as_ptr(), length) };
        let absent: (*const u8, usize) = (std::ptr::null(), 0);
        let begin = |originals: [(*const u8, usize); 4]| {
            let [credential, certificates, account, asset] = originals;
            let request = WalletOpenRequest {
                credential: credential.0,
                credential_length: credential.1,
                certificates: certificates.0,
                certificates_length: certificates.1,
                account: account.0,
                account_length: account.1,
                asset: asset.0,
                asset_length: asset.1,
            };
            let mut out = WalletResult::default();
            let status = unsafe {
                connect_norito_kagemusha_spartan_begin_account_v1(u64::MAX, &request, &mut out)
            };
            assert_eq!(status, out.status);
            assert!(out.bytes.is_null() && out.length == 0);
            status
        };
        // Both complete forms pass the boundary and then find no such owner.
        assert_eq!(
            begin([present(596), present(172), present(8), present(8)]),
            ARTIFACTS_UNAVAILABLE
        );
        assert_eq!(
            begin([present(64), absent, present(8), absent]),
            ARTIFACTS_UNAVAILABLE
        );
        for originals in [
            // A pointer with zero length is not the absent form.
            [present(64), present(0), present(8), absent],
            [present(64), absent, present(8), present(0)],
            // Partial tuples.
            [present(64), present(172), present(8), absent],
            [present(64), absent, present(8), present(8)],
            [present(64), absent, absent, absent],
            [absent, absent, present(8), absent],
            // The split form never admits a first original beyond the exact credential.
            [(bytes.as_ptr(), 597), present(172), present(8), present(8)],
        ] {
            assert_eq!(begin(originals), INVALID);
        }
    }
}

/// Complete the same retained account challenge; no caller acceptance flag.
/// # Safety
/// Signature addresses its stated length; `out` is writable and aligned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_authorize_account_v1(
    handle: u64,
    signature: *const u8,
    length: usize,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        exports::output(out, || {
            call(
                handle,
                Action::AuthorizeAccount(exports::input(signature, length, 4096)?),
            )
        })
    }
}

/// Begin or finish user-presence evidence for one owner-local Send/Unload review.
/// Empty inputs begin. A 32-byte message plus empty signature cancels. On iOS a single
/// zero requests the Native system prompt. Android supplies its platform-confirmed DER.
/// # Safety
/// Inputs address their stated lengths and the result is writable and aligned.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_confirm_v1(
    handle: u64,
    token: u64,
    message: *const u8,
    message_len: usize,
    der: *const u8,
    der_len: usize,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        exports::output(out, || {
            let message = exports::input(message, message_len, 32)?;
            let der = exports::input(der, der_len, 72)?;
            #[cfg(target_os = "ios")]
            if !(der.is_empty() || (message.is_empty() && (der == [1] || der == [2]))) {
                // An iPhone obtains the signature itself with a fresh Native-owned LAContext.
                if der != [0] {
                    return Err(Failure::code(INVALID));
                }
                // Copy caller-owned memory before releasing the wallet lock for system UI.
                let challenge: [u8; 32] = message.try_into().map_err(|_| Failure::code(INVALID))?;
                return apple_confirmation::confirm(handle, token, &challenge);
            }
            call(
                handle,
                Action::Confirm {
                    token,
                    message,
                    der,
                },
            )
        })
    }
}

pub(crate) fn credit_status_input_limits(selector: u32) -> Result<(usize, usize)> {
    match selector {
        1 => Ok((32, 0)),
        2 => Ok((12_182, 10_084)),
        3 => Ok((32, 12_182)),
        _ => Err(Failure::code(INVALID)),
    }
}

#[cfg(test)]
mod credit_status_form_tests {
    use super::*;

    #[test]
    fn archive_payer_and_carrier_have_distinct_bounded_inputs() {
        assert_eq!(credit_status_input_limits(1).unwrap(), (32, 0));
        assert_eq!(credit_status_input_limits(2).unwrap(), (12_182, 10_084));
        assert_eq!(credit_status_input_limits(3).unwrap(), (32, 12_182));
        for selector in [0, 4, u32::MAX] {
            assert!(credit_status_input_limits(selector).is_err());
        }
    }
}

/// Construct or verify a portable receipt from the matching local released transition.
/// # Safety
/// Fixed inputs each address 32 bytes, buffers address their stated lengths, and out is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_credit_status_v1(
    handle: u64,
    slot: *const u8,
    selector: u32,
    operation: *const u8,
    request: *const u8,
    request_len: usize,
    status: *const u8,
    status_len: usize,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        exports::output(out, || {
            let (request_max, status_max) = credit_status_input_limits(selector)?;
            call(
                handle,
                Action::CreditStatus {
                    slot: fixed32(slot)?,
                    selector,
                    operation: fixed32(operation)?,
                    request: exports::input(request, request_len, request_max)?,
                    status: exports::input(status, status_len, status_max)?,
                },
            )
        })
    }
}

/// Issue a receiver Request with the admitted wallet's hardware key, or retry its exact bytes.
/// # Safety
/// The slot and id address 32 readable bytes each, the amount addresses 16 little-endian
/// bytes and each fee original addresses its declared length. The result is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_issue_open_request_v1(
    handle: u64,
    slot: *const u8,
    request_id: *const u8,
    amount: *const u8,
    fee: *const u8,
    fee_len: usize,
    issuer: *const u8,
    issuer_len: usize,
    out: *mut WalletResult,
) -> i32 {
    unsafe {
        exports::output(out, || {
            call(
                handle,
                Action::IssueOpenRequest {
                    slot: fixed32(slot)?,
                    id: fixed32(request_id)?,
                    amount: u128::from_le_bytes(
                        exports::input(amount, 16, 16)?
                            .try_into()
                            .map_err(|_| Failure::code(INVALID))?,
                    ),
                    fee: exports::input(fee, fee_len, 255)?,
                    issuer: exports::input(issuer, issuer_len, 172)?,
                },
            )
        })
    }
}

/// Authenticate a complete regulatory blacklist and install its exact original in local custody.
/// # Safety
/// Slot addresses 32 readable bytes. Other inputs address their stated lengths; out is writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_spartan_install_blacklist_v1(
    handle: u64,
    slot: *const u8,
    original: *const u8,
    original_len: usize,
    certificates: *const u8,
    certificates_len: usize,
    out: *mut WalletResult,
) -> i32 {
    use iroha_data_model::kagemusha::{
        KAGEMUSHA_WALLET_BLACKLIST_MAX_BYTES_V1, KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
    };
    unsafe {
        exports::output(out, || {
            call(
                handle,
                Action::InstallBlacklist {
                    slot: fixed32(slot)?,
                    original: exports::input(
                        original,
                        original_len,
                        KAGEMUSHA_WALLET_BLACKLIST_MAX_BYTES_V1,
                    )?,
                    certificates: exports::input(
                        certificates,
                        certificates_len,
                        KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
                    )?,
                },
            )
        })
    }
}
