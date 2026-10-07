//! Original E1 progression over the installed runtime's sole actual custody provider.
use super::*;
use advance::KagemushaWalletEnrollmentStepV1 as Step;

mod account_frame;
mod exports;
#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows"
))]
mod jni;
mod request;
pub use exports::*;

pub(super) const BOUNDS: [usize; 5] = [
    1024,
    1024,
    4096,
    advance::KAGEMUSHA_WALLET_ENROLLMENT_REQUEST_MAX_BYTES_V1,
    KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
];
pub(super) struct Input<'a> {
    pub selector: u32,
    pub challenge: &'a [u8],
    pub policy: &'a [u8],
    pub account: &'a [u8],
    pub original: &'a [u8],
    pub certificates: &'a [u8],
    pub issued_at_ms: u64,
    pub expires_at_ms: u64,
}
impl Input<'_> {
    pub(super) fn dates(&self) -> advance::KagemushaWalletEnrollmentDatesV1 {
        advance::KagemushaWalletEnrollmentDatesV1 {
            issued_at_ms: self.issued_at_ms,
            expires_at_ms: self.expires_at_ms,
        }
    }
    pub(super) fn validate(&self) -> Result<()> {
        let inputs = [
            self.challenge,
            self.policy,
            self.account,
            self.original,
            self.certificates,
        ];
        if inputs.iter().zip(BOUNDS).any(|(v, bound)| v.len() > bound)
            || [self.challenge, self.policy, self.account]
                .iter()
                .any(|v| v.is_empty())
            || !self.dates().is_valid()
            || self.selector > 3
            || (if self.selector < 2 {
                !self.original.is_empty()
            } else {
                self.original.is_empty()
            })
            || (if self.selector == 3 {
                self.certificates.is_empty()
                    || self.original.len() > KAGEMUSHA_WALLET_CREDENTIAL_MAX_BYTES_V1
            } else {
                !self.certificates.is_empty()
            })
        {
            return Err(Failure::code(INVALID));
        }
        Ok(())
    }
}
pub(super) struct Scope {
    pub challenge: KagemushaWalletEnrollmentChallengeV1,
    pub profile: advance::KagemushaWalletKeyProfileV1,
    pub policy: KagemushaWalletEnrollmentPolicyV1,
    pub network: [u8; 32],
    pub account_key: iroha_crypto::PublicKey,
    pub android: bool,
}
impl Scope {
    fn account_frame(
        &self,
        key: &iroha_data_model::kagemusha::KagemushaDevicePublicKeyV1,
        input: &Input<'_>,
    ) -> Result<Vec<u8>> {
        let (_, owner) = self
            .account_key
            .try_to_bytes()
            .map_err(|_| Failure::code(INVALID))?;
        let owner: [u8; 32] = owner.try_into().map_err(|_| Failure::code(INVALID))?;
        account_frame::frame(
            &self.network,
            &self.challenge.transcript(),
            &owner,
            key.as_sec1_bytes(),
            input.issued_at_ms,
            input.expires_at_ms,
        )
        .ok_or(Failure::code(INVALID))
    }
}
#[derive(Default)]
pub(super) struct Response {
    pub kind: i32,
    pub payment_key: Vec<u8>,
    pub account_frame: Vec<u8>,
    pub binding: Vec<u8>,
    pub chain: Vec<Vec<u8>>,
    pub bytes: Vec<u8>,
}
pub(super) fn execute<P: advance::KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send>(
    runtime: &mut state::NativeWalletRuntimeV1<advance::KagemushaWalletStdFsV1, P, S>,
    input: Input<'_>,
    scope: Scope,
) -> Result<Response> {
    input.validate()?;
    let c = &scope.challenge;
    if input.selector < 2 {
        let step = if input.selector == 0 {
            runtime.begin_enrollment(c, scope.profile, input.dates())?
        } else {
            runtime.resume_enrollment(c, scope.profile, input.dates())?
        };
        return match step {
            Step::Enrolled { marker, .. } => {
                let operation = runtime.enrollment_operation(c, scope.profile, input.dates())?;
                let key = runtime.enrollment_payment_key(&operation)?;
                if key != *marker.payment_key() {
                    return Err(Failure::code(CONFLICT));
                }
                let chain = if scope.android {
                    runtime.enrollment_attestation_chain(&operation)?
                } else {
                    vec![]
                };
                let bytes = norito::encode_canonical(marker.marker())
                    .map_err(|_| Failure::code(INVALID))?;
                if bytes.len() > KAGEMUSHA_WALLET_MARKER_MAX_BYTES_V1 {
                    return Err(Failure::code(INVALID));
                }
                Ok(Response {
                    kind: 0,
                    payment_key: key.as_sec1_bytes().to_vec(),
                    account_frame: scope.account_frame(&key, &input)?,
                    binding:
                        iroha_data_model::kagemusha::kagemusha_wallet_enrollment_key_binding_v1(
                            &c.challenge_digest(),
                            &key,
                        )
                        .to_vec(),
                    chain,
                    bytes,
                })
            }
            Step::Pending { .. } => Ok(Response {
                kind: 1,
                ..Response::default()
            }),
            Step::SlotAbandoned { .. } => Ok(Response {
                kind: 2,
                ..Response::default()
            }),
        };
    }
    let operation = runtime.enrollment_operation(c, scope.profile, input.dates())?;
    match input.selector {
        2 => {
            let bytes = if let Some(retained) = runtime.enrollment_request(&operation)? {
                retained
            } else {
                let key = runtime.enrollment_payment_key(&operation)?;
                let frame = scope.account_frame(&key, &input)?;
                let chain = if scope.android {
                    runtime.enrollment_attestation_chain(&operation)?
                } else {
                    vec![]
                };
                request::verify(input.original, &scope, &key, &frame, &chain)?;
                runtime.retain_enrollment_request(&operation, input.original)?
            };
            Ok(Response {
                kind: 3,
                bytes,
                ..Response::default()
            })
        }
        3 => {
            runtime.store_enrollment_credential(
                &operation,
                &scope.policy,
                input.original,
                input.certificates,
            )?;
            Ok(Response {
                kind: 4,
                bytes: input.original.to_vec(),
                ..Response::default()
            })
        }
        _ => Err(Failure::code(INVALID)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_roles_and_finite_dates_refuse_unused_or_missing_data() {
        let one = [1];
        for selector in 0..4 {
            let mut input = Input {
                selector,
                challenge: &one,
                policy: &one,
                account: &one,
                original: if selector >= 2 { &one } else { &[] },
                certificates: if selector == 3 { &one } else { &[] },
                issued_at_ms: 1,
                expires_at_ms: 2,
            };
            assert!(input.validate().is_ok());
            input.selector = 4;
            assert_eq!(input.validate().unwrap_err().status, INVALID);
            input.selector = selector;
            input.expires_at_ms = 600_002;
            assert_eq!(input.validate().unwrap_err().status, INVALID);
        }
    }
}
