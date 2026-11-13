//! Adverse admission checks against the actual installed runtime and custody.
//! No proof verdict, account authority or selected marker is substituted here.
use super::*;

pub(super) struct Admitted {
    pub(super) slot: advance::KagemushaWalletSlotIdV1,
    challenge: [u8; 32],
    signature: Vec<u8>,
}

pub(super) fn authorize<F: advance::KagemushaWalletFsV1, P: KagemushaWalletPlatformV1 + 'static>(
    runtime: &mut FutureRuntime<F, P, ()>,
    originals: [&[u8]; 4],
    sign: impl Fn(&[u8]) -> Vec<u8>,
    previous: Option<&Admitted>,
) -> Result<Admitted> {
    let [credential, certificate, account, asset] = originals;
    let challenge = runtime
        .begin_account(credential, certificate, account, asset)
        .map_err(fail)?;
    if let Some(previous) = previous {
        if challenge == previous.challenge {
            return Err("cold-start account challenge was reused".into());
        }
        if runtime.authorize_account(&previous.signature).is_ok() {
            return Err("old account signature authorized a restarted runtime".into());
        }
    }
    let signature = sign(&challenge);
    if signature.len() != 64 {
        return Err("fixture account signature length".into());
    }
    let mut invalid_signature = signature.clone();
    invalid_signature[0] ^= 1;
    if runtime.authorize_account(&invalid_signature).is_ok() {
        return Err("invalid account signature admitted monetary custody".into());
    }
    let mut changed_credential = credential.to_vec();
    let Some(first) = changed_credential.first_mut() else {
        return Err("fixture credential missing".into());
    };
    *first ^= 1;
    if runtime
        .begin_account(&changed_credential, certificate, account, asset)
        .is_ok()
    {
        return Err("changed credential replaced retained admission originals".into());
    }
    let retried = runtime
        .begin_account(credential, certificate, account, asset)
        .map_err(fail)?;
    if retried != challenge {
        return Err("refused account attempt replaced the issued challenge".into());
    }
    let slot =
        advance::KagemushaWalletSlotIdV1(runtime.authorize_account(&signature).map_err(fail)?);
    if previous.is_some_and(|previous| previous.slot != slot) {
        return Err("cold-start account admission selected another custody slot".into());
    }
    if runtime.authorize_account(&signature).map_err(fail)? != slot.0 {
        return Err("lost account authorization reply changed the custody owner".into());
    }
    Ok(Admitted {
        slot,
        challenge,
        signature,
    })
}
