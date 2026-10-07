//! Native enrollment custody followed by complete source-qualified runtime installation.
use super::*;
use iroha_core_zk::kagemusha_wallet_enrollment_v1 as native;

/// Enrollment actions contain originals only; no policy, time, liveness or hardware verdict.
pub(crate) enum Action<'a> {
    Begin([&'a [u8]; 3]),
    Authorize(&'a [u8]),
    Progress,
    Prepare(native::PlatformEvidenceV1),
    RetainRequest(&'a [u8]),
    AcceptResult(&'a [u8]),
    Load,
    BeginOpen,
}
impl From<native::Error> for Failure {
    fn from(error: native::Error) -> Self {
        match error {
            native::Error::Provider(error) => error.into(),
            native::Error::Original(_) => Self::code(INVALID),
            native::Error::Phase => Self::code(CONFLICT),
        }
    }
}
/// Native initialization retains the exclusive enrollment owner and real deployment material.
/// No foreign call can construct this configuration or select the original source store.
pub struct NativeEnrollmentRuntime<
    P: advance::KagemushaWalletPlatformV1,
    S: OriginalSourceV1 + Send,
> {
    owner: Option<native::EnrollmentOwnerV1<advance::KagemushaWalletStdFsV1, P>>,
    installed: Option<open::Runtime<P, S>>,
    config: state::NativeInstallationConfigV1,
    verifier_pack: Vec<u8>,
    inventory: Vec<u8>,
    originals: Option<S>,
    open_originals: Option<[Vec<u8>; 4]>,
}
impl<P: advance::KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send>
    NativeEnrollmentRuntime<P, S>
{
    /// Retain native deployment inputs before E2. Artifact qualification remains mandatory at handoff.
    #[must_use]
    pub fn new(
        owner: native::EnrollmentOwnerV1<advance::KagemushaWalletStdFsV1, P>,
        config: state::NativeInstallationConfigV1,
        verifier_pack: Vec<u8>,
        inventory: Vec<u8>,
        originals: S,
    ) -> Self {
        Self {
            owner: Some(owner),
            installed: None,
            config,
            verifier_pack,
            inventory,
            originals: Some(originals),
            open_originals: None,
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
fn progress(value: native::EnrollmentProgressV1) -> Response {
    match value {
        native::EnrollmentProgressV1::Evidence {
            slot,
            marker,
            challenge_digest,
        } => {
            // Fixed FFI projection, not a wire carrier or an authority verdict.
            let mut bytes = Vec::with_capacity(161);
            bytes.extend_from_slice(&slot);
            bytes.extend_from_slice(marker.payment_key.as_sec1_bytes());
            bytes.extend_from_slice(&challenge_digest);
            bytes.extend_from_slice(&kagemusha_wallet_enrollment_key_binding_v1(
                &challenge_digest,
                &marker.payment_key,
            ));
            response(19, bytes)
        }
        native::EnrollmentProgressV1::Pending => response(20, Vec::new()),
        native::EnrollmentProgressV1::Abandoned => response(21, Vec::new()),
        native::EnrollmentProgressV1::BootstrapSelected => response(22, Vec::new()),
    }
}
impl<P: advance::KagemushaWalletPlatformV1 + 'static, S: OriginalSourceV1 + Send + 'static>
    open::Admission for NativeEnrollmentRuntime<P, S>
{
    fn enrollment(&mut self, action: Action<'_>) -> Result<Response> {
        if let Some(runtime) = self.installed.as_mut() {
            return match action {
                Action::Load => Ok(response(26, Vec::new())),
                Action::BeginOpen => {
                    let originals = self
                        .open_originals
                        .as_ref()
                        .ok_or(Failure::code(INTERNAL))?;
                    Ok(response(
                        15,
                        runtime.begin(originals.each_ref().map(Vec::as_slice))?,
                    ))
                }
                _ => Err(Failure::code(CONFLICT)),
            };
        }
        if matches!(action, Action::Load) {
            // A selected verified E6 is required before provider transfer. Missing originals
            // are not reconstructed from a supplied result during this action.
            let selected_originals = self
                .owner
                .as_mut()
                .ok_or(Failure::code(CLOSED))?
                .open_originals()?;
            let owner = self.owner.take().ok_or(Failure::code(CLOSED))?;
            let Some(originals) = self.originals.take() else {
                self.owner = Some(owner);
                return Err(Failure::code(INTERNAL));
            };
            let loaded = owner.try_handoff(|provider| {
                state::NativeWalletRuntimeV1::load(
                    &self.config,
                    provider,
                    &self.verifier_pack,
                    &self.inventory,
                    originals,
                )
                .map_err(|error| {
                    let (provider, originals, error) = error.into_parts();
                    (provider, (originals, error))
                })
            });
            match loaded {
                Ok(runtime) => {
                    self.open_originals = Some(selected_originals);
                    self.installed = Some(open::Runtime(Some(open::Phase::Ready(runtime))));
                    Ok(response(26, Vec::new()))
                }
                Err((owner, (originals, error))) => {
                    self.owner = Some(owner);
                    self.originals = Some(originals);
                    Err(error.into())
                }
            }
        } else {
            let owner = self.owner.as_mut().ok_or(Failure::code(CLOSED))?;
            match action {
                Action::Begin([challenge, account, asset]) => Ok(response(
                    18,
                    owner.begin(challenge, account, asset)?.to_vec(),
                )),
                Action::Authorize(signature) => Ok(progress(owner.authorize(signature)?)),
                Action::Progress => Ok(progress(owner.progress()?)),
                Action::Prepare(evidence) => {
                    let bytes =
                        norito::encode_canonical(&evidence).map_err(|_| Failure::code(INVALID))?;
                    match owner.prepare_request(&bytes)? {
                        native::RequestPreparationV1::Retained(bytes) => Ok(response(24, bytes)),
                        native::RequestPreparationV1::AccountChallenge(message) => {
                            Ok(response(23, message.to_vec()))
                        }
                    }
                }
                Action::RetainRequest(signature) => {
                    Ok(response(24, owner.retain_request(signature)?))
                }
                Action::AcceptResult(bytes) => Ok(response(25, owner.accept_credential(bytes)?)),
                Action::BeginOpen => Err(Failure::code(ARTIFACTS_UNAVAILABLE)),
                Action::Load => unreachable!("handled before borrowing owner"),
            }
        }
    }
    fn begin(&mut self, originals: [&[u8]; 4]) -> Result<Vec<u8>> {
        self.installed
            .as_mut()
            .ok_or(Failure::code(ARTIFACTS_UNAVAILABLE))?
            .begin(originals)
    }
    fn finish(&mut self, signature: &[u8]) -> Result<(Box<dyn Wallet>, state::Scheduler)> {
        self.installed
            .as_mut()
            .ok_or(Failure::code(ARTIFACTS_UNAVAILABLE))?
            .finish(signature)
    }
    fn cancel(&mut self) -> Result<()> {
        self.installed
            .as_mut()
            .ok_or(Failure::code(ARTIFACTS_UNAVAILABLE))?
            .cancel()
    }
}
/// Register native enrollment startup. Every registration failure returns the exact owner.
/// # Errors
/// Registry capacity or poison; no provider, key or original source is discarded on failure.
pub fn retain_native_enrollment<P, S>(
    runtime: NativeEnrollmentRuntime<P, S>,
) -> std::result::Result<u64, (NativeEnrollmentRuntime<P, S>, Failure)>
where
    P: advance::KagemushaWalletPlatformV1 + 'static,
    S: OriginalSourceV1 + Send + 'static,
{
    let mut registry = match registry().lock() {
        Ok(value) => value,
        Err(_) => return Err((runtime, Failure::code(INTERNAL))),
    };
    if registry.owners.len() + registry.runtimes.len() >= MAX_OWNERS {
        return Err((runtime, Failure::code(RESOURCE)));
    }
    let Some(id) = registry
        .next
        .checked_add(1)
        .filter(|id| *id <= i64::MAX as u64)
    else {
        return Err((runtime, Failure::code(RESOURCE)));
    };
    registry.next = id;
    registry.runtimes.insert(
        id,
        Arc::new(open::RuntimeOwner {
            admission: Mutex::new(Some(Box::new(runtime))),
            finished: Mutex::new(None),
        }),
    );
    Ok(id)
}
pub(crate) fn call(id: u64, action: Action<'_>) -> Result<Response> {
    let owner = open::runtime(id)?;
    let mut admission = owner
        .admission
        .lock()
        .map_err(|_| Failure::code(INTERNAL))?;
    if owner
        .finished
        .lock()
        .map_err(|_| Failure::code(INTERNAL))?
        .is_some()
    {
        return Err(Failure::code(CONFLICT));
    }
    let mut reply = admission
        .as_deref_mut()
        .ok_or(Failure::code(CLOSED))?
        .enrollment(action)?;
    reply.sequence = u128::from(id);
    Ok(reply)
}

pub(crate) fn bounds(selector: u32) -> Result<[usize; 3]> {
    Ok(match selector {
        0 => [1024, 4096, 1024],
        1 | 5 => [64, 0, 0],
        2 | 7 | 8 => [0; 3],
        3 => [65_536, 0, 0],
        4 => [32, 65_536, 4096],
        6 => [native::RESULT_MAX_BYTES, 0, 0],
        _ => return Err(Failure::code(INVALID)),
    })
}
pub(crate) fn request<'a>(
    selector: u32,
    originals: [&'a [u8]; 3],
    chain: &[&[u8]],
) -> Result<Action<'a>> {
    let limits = bounds(selector)?;
    if originals
        .iter()
        .zip(limits)
        .any(|(bytes, bound)| bytes.len() > bound)
        || (selector != 3 && !chain.is_empty())
        || chain.len() > 8
        || chain
            .iter()
            .any(|item| item.is_empty() || item.len() > 16_384)
    {
        return Err(Failure::code(INVALID));
    }
    let [first, second, third] = originals;
    Ok(match selector {
        0 if !first.is_empty() && !second.is_empty() && !third.is_empty() => {
            Action::Begin(originals)
        }
        1 => Action::Authorize(first),
        2 => Action::Progress,
        3 if !first.is_empty() && chain.len() >= 2 => {
            Action::Prepare(native::PlatformEvidenceV1::Android {
                certificates: chain.iter().map(|item| item.to_vec()).collect(),
                play_integrity_token: first.to_vec(),
            })
        }
        4 if first.len() == 32 && !second.is_empty() && !third.is_empty() => {
            Action::Prepare(native::PlatformEvidenceV1::Apple {
                key_id: first.try_into().map_err(|_| Failure::code(INVALID))?,
                attestation: second.to_vec(),
                key_binding_assertion: third.to_vec(),
            })
        }
        5 => Action::RetainRequest(first),
        6 if !first.is_empty() => Action::AcceptResult(first),
        7 => Action::Load,
        8 => Action::BeginOpen,
        _ => return Err(Failure::code(INVALID)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_enrollment_bounds_are_per_role_and_unused_fields_are_rejected() {
        for selector in 0..=8 {
            let limits = bounds(selector).unwrap();
            for slot in 0..3 {
                let long = vec![0; limits[slot] + 1];
                let mut originals = [&[][..]; 3];
                originals[slot] = &long;
                assert!(request(selector, originals, &[]).is_err());
            }
        }
        assert!(request(9, [&[]; 3], &[]).is_err());
        assert!(request(2, [&[]; 3], &[b"foreign chain"]).is_err());
        assert!(request(3, [b"token", &[], &[]], &[b"leaf"]).is_err());
        assert!(request(3, [b"token", &[], &[]], &[b"leaf", b"ca"]).is_ok());
        assert!(request(4, [&[1; 32], b"attestation", b"assertion"], &[]).is_ok());
        assert!(request(4, [&[1; 32], b"attestation", &vec![0; 4097]], &[]).is_err());
    }
    #[test]
    fn enrollment_ffi_layout_has_no_hidden_original_or_authority_fields() {
        use std::mem::{align_of, offset_of, size_of};
        let pointer = size_of::<usize>();
        let first = (size_of::<u32>() + pointer - 1) & !(pointer - 1);
        assert_eq!(offset_of!(WalletEnrollmentRequest, selector), 0);
        assert_eq!(offset_of!(WalletEnrollmentRequest, first), first);
        assert_eq!(
            offset_of!(WalletEnrollmentRequest, first_length),
            first + pointer
        );
        assert_eq!(
            offset_of!(WalletEnrollmentRequest, second),
            first + 2 * pointer
        );
        assert_eq!(
            offset_of!(WalletEnrollmentRequest, second_length),
            first + 3 * pointer
        );
        assert_eq!(
            offset_of!(WalletEnrollmentRequest, third),
            first + 4 * pointer
        );
        assert_eq!(
            offset_of!(WalletEnrollmentRequest, third_length),
            first + 5 * pointer
        );
        assert_eq!(
            offset_of!(WalletEnrollmentRequest, certificates),
            first + 6 * pointer
        );
        assert_eq!(
            offset_of!(WalletEnrollmentRequest, certificate_count),
            first + 7 * pointer
        );
        assert_eq!(size_of::<WalletEnrollmentRequest>(), first + 8 * pointer);
        assert_eq!(align_of::<WalletEnrollmentRequest>(), pointer);
        assert_eq!(offset_of!(WalletEnrollmentItem, bytes), 0);
        assert_eq!(offset_of!(WalletEnrollmentItem, length), pointer);
        assert_eq!(size_of::<WalletEnrollmentItem>(), 2 * pointer);
    }
    #[test]
    fn c_enrollment_rejects_lengths_before_dereferencing_and_initializes_outputs() {
        let mut input = WalletEnrollmentRequest {
            selector: 3,
            first: std::ptr::null(),
            first_length: 65_537,
            second: std::ptr::null(),
            second_length: 0,
            third: std::ptr::null(),
            third_length: 0,
            certificates: std::ptr::null(),
            certificate_count: 9,
        };
        let mut output = WalletResult::default();
        assert_eq!(
            unsafe { connect_norito_kagemusha_wallet_enrollment_v1(u64::MAX, &input, &mut output) },
            INVALID
        );
        assert_eq!(output.status, INVALID);
        assert!(output.bytes.is_null());
        assert_eq!(output.length, 0);
        input.selector = 2;
        input.first_length = 0;
        input.certificate_count = 0;
        assert_eq!(
            unsafe { connect_norito_kagemusha_wallet_enrollment_v1(u64::MAX, &input, &mut output) },
            ARTIFACTS_UNAVAILABLE
        );
        assert_eq!(output.status, ARTIFACTS_UNAVAILABLE);
    }
}
