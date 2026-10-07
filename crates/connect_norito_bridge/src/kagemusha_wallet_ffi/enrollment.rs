//! Finite original-only enrollment over the installed runtime's exclusive provider.
use super::*;
use advance::{KagemushaWalletEnrollmentStepV1 as Step, KagemushaWalletSlotIdV1 as Slot};

mod exports;
#[cfg(any(target_os = "android", target_os = "linux", target_os = "macos"))]
mod jni;
pub use exports::*;

pub(super) const BOUNDS: [usize; 6] = [
    32,
    1024,
    1024,
    4096,
    advance::KAGEMUSHA_WALLET_ENROLLMENT_REQUEST_MAX_BYTES_V1,
    KAGEMUSHA_WALLET_MESSAGE_MAX_BYTES_V1,
];
pub(super) struct Input<'a> {
    pub selector: u32,
    pub slot: &'a [u8],
    pub challenge: &'a [u8],
    pub policy: &'a [u8],
    pub account: &'a [u8],
    pub original: &'a [u8],
    pub certificates: &'a [u8],
}
impl Input<'_> {
    pub(super) fn validate(&self) -> Result<()> {
        let inputs = [
            self.slot,
            self.challenge,
            self.policy,
            self.account,
            self.original,
            self.certificates,
        ];
        if inputs
            .iter()
            .zip(BOUNDS)
            .any(|(bytes, bound)| bytes.len() > bound)
            || [self.challenge, self.policy, self.account]
                .iter()
                .any(|bytes| bytes.is_empty())
            || self.selector > 3
            || (if self.selector == 0 {
                !self.slot.is_empty()
            } else {
                self.slot.len() != 32 || self.slot.iter().all(|v| *v == 0)
            })
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
    pub regulatory: KagemushaWalletRegulatoryPolicyV1,
}
#[derive(Default)]
pub(super) struct Response {
    pub kind: i32,
    pub slot: [u8; 32],
    pub payment_key: Vec<u8>,
    pub bytes: Vec<u8>,
}
fn step(value: Step) -> Result<Response> {
    Ok(match value {
        Step::Enrolled { slot, marker } => {
            let bytes =
                norito::encode_canonical(marker.marker()).map_err(|_| Failure::code(INVALID))?;
            if bytes.len() > KAGEMUSHA_WALLET_MARKER_MAX_BYTES_V1 {
                return Err(Failure::code(INVALID));
            }
            Response {
                kind: 0,
                slot: slot.0,
                payment_key: marker.payment_key().as_sec1_bytes().to_vec(),
                bytes,
            }
        }
        Step::Pending { slot } => Response {
            kind: 1,
            slot: slot.0,
            ..Response::default()
        },
        Step::SlotAbandoned { slot } => Response {
            kind: 2,
            slot: slot.0,
            ..Response::default()
        },
    })
}
pub(super) fn execute<P: advance::KagemushaWalletPlatformV1, S: OriginalSourceV1 + Send>(
    runtime: &mut state::NativeWalletRuntimeV1<advance::KagemushaWalletStdFsV1, P, S>,
    input: Input<'_>,
    scope: Scope,
) -> Result<Response> {
    input.validate()?;
    let c = &scope.challenge;
    if input.selector == 0 {
        return step(runtime.begin_enrollment(c, scope.profile)?);
    }
    let slot = Slot(input.slot.try_into().map_err(|_| Failure::code(INVALID))?);
    match input.selector {
        1 => step(runtime.resume_enrollment(&slot, c, scope.profile)?),
        2 => Ok(Response {
            kind: 3,
            slot: slot.0,
            bytes: runtime.retain_enrollment_request(&slot, c, scope.profile, input.original)?,
            ..Response::default()
        }),
        3 => {
            runtime.store_enrollment_credential(
                &slot,
                c,
                scope.profile,
                &scope.regulatory,
                input.original,
                input.certificates,
            )?;
            Ok(Response {
                kind: 4,
                slot: slot.0,
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
    fn finite_enrollment_request_roles_reject_unused_or_missing_originals() {
        let one = [1];
        let slot = [1; 32];
        for selector in 0..4 {
            let mut input = Input {
                selector,
                slot: if selector == 0 { &[] } else { &slot },
                challenge: &one,
                policy: &one,
                account: &one,
                original: if selector >= 2 { &one } else { &[] },
                certificates: if selector == 3 { &one } else { &[] },
            };
            assert!(input.validate().is_ok());
            input.selector = 4;
            assert_eq!(input.validate().unwrap_err().status, INVALID);
        }
        let input = Input {
            selector: 0,
            slot: &slot,
            challenge: &one,
            policy: &one,
            account: &one,
            original: &[],
            certificates: &[],
        };
        assert_eq!(input.validate().unwrap_err().status, INVALID);
    }
}
