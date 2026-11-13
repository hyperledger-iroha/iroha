//! Synchronous fixture seam over the actual retained Apple enrollment journal.
use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use connect_norito_bridge::kagemusha_wallet_ffi::{CallbackPlatform, PlatformCallbacks};
use iroha_data_model::kagemusha::{
    KagemushaWalletAppIdentityV1, KagemushaWalletEnrollmentPlatformV1,
};
use sha2::{Digest as _, Sha256};
use std::ffi::c_void;

type Collect =
    unsafe extern "C" fn(*mut c_void, *const u8, *const u8, *const u8, *mut c_void) -> i32;
type Confirm = unsafe extern "C" fn(*mut c_void, *const u8, usize) -> i32;
pub(super) struct Callbacks {
    context: *mut c_void,
    collect: Collect,
    confirm: Confirm,
}

impl Confirmation for Callbacks {
    fn confirm(&self, review: &[u8]) -> Result<()> {
        let result = unsafe { (self.confirm)(self.context, review.as_ptr(), review.len()) };
        if result == 0 {
            Ok(())
        } else {
            Err(format!("hardware Send confirmation refused: {result}"))
        }
    }
}

pub(super) fn configure(f: &mut advance::engineering_fixture::Fixture) -> Result<()> {
    // Actual independently selected public Apple CA, never supplied by the app callback.
    let root = include_bytes!("../../../../../certs/apple_app_attestation_root.der").to_vec();
    f.config.app.identity = KagemushaWalletAppIdentityV1::Apple {
        app_id: "YLWWUD25VZ.jp.co.soramitsu.kagemusha.spartanfixture".into(),
    };
    f.config.policy.app_policy = f.config.app.policy_digest().map_err(fail)?;
    f.config.policy.platform = KagemushaWalletEnrollmentPlatformV1::Apple {
        attestation_root_sha256: Sha256::digest(&root).into(),
    };
    f.config.attestation_root_der = Some(root);
    f.challenge.app_policy = f.config.policy.app_policy;
    f.challenge.enrollment_policy = f.config.policy.policy_digest().map_err(fail)?;
    Ok(())
}

trait JournalOps {
    fn call(&mut self, operation: u32, stage: u8, input: &[u8]) -> Result<Vec<u8>>;
}
impl<F: advance::KagemushaWalletFsV1, P: KagemushaWalletPlatformV1> JournalOps
    for EnrollmentOwnerV1<F, P>
{
    fn call(&mut self, operation: u32, stage: u8, input: &[u8]) -> Result<Vec<u8>> {
        match operation {
            0 if input.is_empty() => {
                let mut output = Vec::new();
                for value in self.apple_collection_originals().map_err(fail)? {
                    output.extend_from_slice(&(value.len() as u32).to_be_bytes());
                    output.extend_from_slice(&value);
                }
                Ok(output)
            }
            1 if input.is_empty() => self
                .begin_apple_effect(stage)
                .map(|()| Vec::new())
                .map_err(fail),
            2 => self
                .retain_apple_effect(stage, input)
                .map(|()| Vec::new())
                .map_err(fail),
            3 if input.is_empty() => self
                .complete_apple_collection()
                .map(|()| Vec::new())
                .map_err(fail),
            _ => Err("invalid fixture journal operation".into()),
        }
    }
}
struct Journal<'a>(&'a mut dyn JournalOps);

pub(super) fn collect<F: advance::KagemushaWalletFsV1, P: KagemushaWalletPlatformV1>(
    callbacks: &Callbacks,
    owner: &mut EnrollmentOwnerV1<F, P>,
    slot: &[u8; 32],
    payment_key: &[u8; 65],
    challenge: &[u8; 32],
) -> Result<Vec<u8>> {
    let mut journal = Journal(owner);
    // Callback runs synchronously. The journal pointer cannot outlive this exclusive borrow.
    let result = unsafe {
        (callbacks.collect)(
            callbacks.context,
            slot.as_ptr(),
            payment_key.as_ptr(),
            challenge.as_ptr(),
            (&mut journal as *mut Journal<'_>).cast(),
        )
    };
    if result != 0 {
        return Err(format!("actual Apple evidence callback refused: {result}"));
    }
    owner.complete_apple_collection().map_err(fail)?;
    let [key, attestation, key_binding_assertion] =
        owner.apple_collection_originals().map_err(fail)?;
    let key_id = STANDARD
        .decode(key)
        .map_err(fail)?
        .try_into()
        .map_err(|_| "App Attest key length")?;
    norito::encode_canonical(&PlatformEvidenceV1::Apple {
        key_id,
        attestation,
        key_binding_assertion,
    })
    .map_err(fail)
}

unsafe fn bytes<'a>(pointer: *const u8, length: usize, maximum: usize) -> Result<&'a [u8]> {
    if length > maximum || (pointer.is_null() && length != 0) {
        return Err("fixture buffer bounds".into());
    }
    Ok(if length == 0 {
        &[]
    } else {
        unsafe { std::slice::from_raw_parts(pointer, length) }
    })
}
unsafe fn report(
    result: Result<Vec<u8>>,
    output: *mut u8,
    capacity: usize,
    output_len: *mut usize,
) -> i32 {
    if output_len.is_null() || (output.is_null() && capacity != 0) {
        return -1;
    }
    unsafe {
        *output_len = 0;
    }
    let (status, data) = match result {
        Ok(value) => (0, value),
        Err(error) => (-1, error.into_bytes()),
    };
    if data.len() > capacity {
        return -2;
    }
    if !data.is_empty() {
        unsafe {
            std::ptr::copy_nonoverlapping(data.as_ptr(), output, data.len());
        }
    }
    unsafe {
        *output_len = data.len();
    }
    status
}

/// Borrowed only during the synchronous Apple collection callback; no global registry.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spartan_fixture_apple_journal_v1(
    journal: *mut c_void,
    operation: u32,
    stage: u8,
    input: *const u8,
    input_len: usize,
    output: *mut u8,
    capacity: usize,
    output_len: *mut usize,
) -> i32 {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if journal.is_null() {
            return Err("missing fixture journal".into());
        }
        let input = unsafe { bytes(input, input_len, 65_536)? };
        let journal = unsafe { &mut *journal.cast::<Journal<'_>>() };
        journal.0.call(operation, stage, input)
    }))
    .unwrap_or_else(|_| Err("fixture journal panic".into()));
    unsafe { report(result, output, capacity, output_len) }
}

/// Actual native fixture; all hardware calls use the canonical retained platform.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn spartan_fixture_run_apple_v1(
    platform: *const PlatformCallbacks,
    root: *const u8,
    root_len: usize,
    artifacts: *const u8,
    artifacts_len: usize,
    context: *mut c_void,
    collect: Option<Collect>,
    confirm: Option<Confirm>,
    output: *mut u8,
    capacity: usize,
    output_len: *mut usize,
) -> i32 {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if platform.is_null() {
            return Err("missing actual Apple platform".into());
        }
        let root = std::str::from_utf8(unsafe { bytes(root, root_len, 4096)? }).map_err(fail)?;
        let artifacts =
            std::str::from_utf8(unsafe { bytes(artifacts, artifacts_len, 4096)? }).map_err(fail)?;
        if !Path::new(root).is_absolute() || !Path::new(artifacts).is_absolute() {
            return Err("fixture paths must be absolute".into());
        }
        let callbacks = Callbacks {
            context,
            collect: collect.ok_or("missing Apple collection callback")?,
            confirm: confirm.ok_or("missing hardware confirmation callback")?,
        };
        let platform = unsafe { CallbackPlatform::new(*platform) }
            .map_err(|_| "invalid Apple platform callbacks")?;
        exercise(
            Path::new(root),
            Path::new(artifacts),
            Arc::new(platform),
            Some(&callbacks),
            Some(&callbacks),
        )
        .map(String::into_bytes)
    }))
    .unwrap_or_else(|_| Err("fixture runtime panic".into()));
    unsafe { report(result, output, capacity, output_len) }
}
