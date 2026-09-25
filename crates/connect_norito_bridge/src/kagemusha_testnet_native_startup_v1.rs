//! Signed-checkpoint activation of a natively provisioned Experimental mobile host.
//!
//! C/JNI transports only a bounded signed bootstrap. Independent deployment policy, proof
//! artifacts, private storage and freshness remain Rust-owned. Activation retains the actual
//! host for the process lifetime; it does not open the production Core coordinator or prepare
//! a private mint. No reset is possible after a partially completed installation.

use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{Mutex, OnceLock},
};

use iroha_core::zk::kagemusha_v1_recursion::{
    KagemushaRecursiveVerifierProfileV1, KagemushaVerifiedFinalityChainV1,
};
use iroha_data_model::{NetworkId, kagemusha::KagemushaReleaseAuthorityPolicyV1};

use crate::{
    ERR_BUFFER_TOO_SMALL, ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1, ERR_KAGEMUSHA_V1,
    KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1, KagemushaMobileBootstrapPinsV1,
    KagemushaMobileBootstrapReplayPinV1, KagemushaMobileBootstrapScopeV1,
    KagemushaTestnetDurableObservationModeV1, KagemushaTestnetNativeMintInstallV1,
    KagemushaTestnetNativeMobileHostV1, KagemushaVerifiedMobileBootstrapV1,
    verify_kagemusha_mobile_bootstrap_v1,
};

/// Exact startup ABI and maximum signed-bootstrap archive size.
pub const KAGEMUSHA_TESTNET_NATIVE_STARTUP_CONTRACT_V1: [u32; 2] =
    [1, KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1 as u32];

/// Immutable native provisioning for one Experimental mobile installation.
///
/// A native application owner must obtain these values independently of operation responses.
/// This structure and its private paths are never decoded from C/JNI input. The release loader
/// still authenticates every archive and proof artifact before constructing the actual host.
pub struct KagemushaTestnetNativeStartupContextV1 {
    /// Independently pinned threshold authority policy.
    pub authority_policy: KagemushaReleaseAuthorityPolicyV1,
    /// Independently selected deployment network.
    pub network_id: NetworkId,
    /// Independently selected asset and reserve scope.
    pub scope: KagemushaMobileBootstrapScopeV1,
    /// Independently selected release identity.
    pub release_id: [u8; 32],
    /// Independently selected release-attestation digest.
    pub release_attestation_digest: [u8; 32],
    /// Signed release manifest archive.
    pub manifest_archive: Vec<u8>,
    /// Release validation receipt archive.
    pub validation_receipt_archive: Vec<u8>,
    /// Signed release attestation archive.
    pub release_attestation_archive: Vec<u8>,
    /// Release-authenticated native proof layout.
    pub profile: KagemushaRecursiveVerifierProfileV1,
    /// Native content-addressed proof artifact directory.
    pub artifact_root: PathBuf,
    /// Native private mint and observation journal path.
    pub mint_journal_path: PathBuf,
    /// Native private counted-value journal path.
    pub value_ledger_path: PathBuf,
    /// Trusted create/recover choice applied to both journals.
    pub mode: KagemushaTestnetDurableObservationModeV1,
    /// Independent authenticated finality anchors required to recover mint history.
    pub independent_anchors: BTreeMap<[u8; 32], KagemushaVerifiedFinalityChainV1>,
}

/// Fresh independent native time and sequence state for the provisioned context.
#[derive(Clone, Copy, Debug)]
pub struct KagemushaTestnetNativeStartupFreshnessV1 {
    /// Trusted current Unix milliseconds; a handset wall clock alone is insufficient.
    pub trusted_now_ms: u64,
    /// Independently retained nonzero sequence floor.
    pub minimum_sequence: u64,
    /// Exact previously accepted checkpoint, when present.
    pub previous: Option<KagemushaMobileBootstrapReplayPinV1>,
}

/// Native freshness ownership; no method or callback is exposed through C/JNI.
///
/// Implementations must independently authenticate time and retain the accepted sequence
/// against the deployment's rollback threat. Success must mean durable persistence; the
/// next read must expose that exact pin. This interface itself grants no hardware guarantee.
pub trait KagemushaTestnetNativeStartupFreshnessProviderV1: Send + Sync {
    /// Read current time and replay state for the independently provisioned context.
    ///
    /// # Errors
    /// Returns an error when independent freshness cannot be established.
    fn read_freshness(&self) -> Result<KagemushaTestnetNativeStartupFreshnessV1, String>;

    /// Durably retain an already verified checkpoint without regressing prior state.
    ///
    /// # Errors
    /// Any storage uncertainty must return an error; activation then requires restart.
    fn retain_verified_bootstrap(
        &self,
        pin: KagemushaMobileBootstrapReplayPinV1,
    ) -> Result<(), String>;
}

enum StartupState<H> {
    Cold,
    Active {
        pin: KagemushaMobileBootstrapReplayPinV1,
        host: H,
    },
    Poisoned,
}

struct StartupSession {
    context: KagemushaTestnetNativeStartupContextV1,
    freshness: Box<dyn KagemushaTestnetNativeStartupFreshnessProviderV1>,
    state: StartupState<KagemushaTestnetNativeMobileHostV1>,
}

struct ProvisionedStartupSession {
    process_id: u32,
    session: Mutex<StartupSession>,
}

static STARTUP_SESSION: OnceLock<ProvisionedStartupSession> = OnceLock::new();

/// Provision one immutable native startup context before the app calls activation.
///
/// This is Rust-only and install-once. Provisioning does not install an observation owner,
/// open a journal, grant monetary authority, or declare a device hardware-qualified.
///
/// # Errors
/// Rejects a second provisioner, even if activation has not yet succeeded.
pub fn install_kagemusha_testnet_native_startup_context_v1(
    context: KagemushaTestnetNativeStartupContextV1,
    freshness: Box<dyn KagemushaTestnetNativeStartupFreshnessProviderV1>,
) -> Result<(), String> {
    install_context(&STARTUP_SESSION, context, freshness)
}

fn install_context(
    slot: &OnceLock<ProvisionedStartupSession>,
    context: KagemushaTestnetNativeStartupContextV1,
    freshness: Box<dyn KagemushaTestnetNativeStartupFreshnessProviderV1>,
) -> Result<(), String> {
    slot.set(ProvisionedStartupSession {
        process_id: std::process::id(),
        session: Mutex::new(StartupSession {
            context,
            freshness,
            state: StartupState::Cold,
        }),
    })
    .map_err(|_| "testnet native startup context is already provisioned".to_owned())
}

fn require_process_owner(process_id: u32) -> Result<(), String> {
    if process_id != std::process::id() {
        return Err(
            "testnet native startup cannot use a fork-inherited context or host".to_owned(),
        );
    }
    Ok(())
}

impl KagemushaTestnetNativeStartupContextV1 {
    fn verify(
        &self,
        archive: &[u8],
        freshness: KagemushaTestnetNativeStartupFreshnessV1,
    ) -> Result<KagemushaVerifiedMobileBootstrapV1, String> {
        verify_kagemusha_mobile_bootstrap_v1(
            archive,
            KagemushaMobileBootstrapPinsV1 {
                authority_policy: &self.authority_policy,
                network_id: self.network_id,
                scope: self.scope,
                release_id: self.release_id,
                release_attestation_digest: self.release_attestation_digest,
                minimum_sequence: freshness.minimum_sequence,
                previous: freshness.previous,
                trusted_now_ms: freshness.trusted_now_ms,
            },
        )
    }

    fn install_host(
        &self,
        bootstrap: &KagemushaVerifiedMobileBootstrapV1,
    ) -> Result<KagemushaTestnetNativeMobileHostV1, String> {
        KagemushaTestnetNativeMobileHostV1::install(
            KagemushaTestnetNativeMintInstallV1 {
                manifest_archive: &self.manifest_archive,
                validation_receipt_archive: &self.validation_receipt_archive,
                release_attestation_archive: &self.release_attestation_archive,
                bootstrap,
                profile: self.profile.clone(),
                artifact_root: &self.artifact_root,
                journal_path: &self.mint_journal_path,
                mode: self.mode,
                independent_anchors: &self.independent_anchors,
            },
            &self.value_ledger_path,
            self.mode,
        )
    }
}

fn activate<H>(
    context: &KagemushaTestnetNativeStartupContextV1,
    freshness: &dyn KagemushaTestnetNativeStartupFreshnessProviderV1,
    state: &mut StartupState<H>,
    archive: &[u8],
    install: impl FnOnce(&KagemushaVerifiedMobileBootstrapV1) -> Result<H, String>,
) -> Result<(), String> {
    if matches!(state, StartupState::Poisoned) {
        return Err(
            "testnet native startup requires process restart after uncertain installation"
                .to_owned(),
        );
    }
    let verified = context.verify(archive, freshness.read_freshness()?)?;
    let pin = verified.replay_pin();
    if let StartupState::Active { pin: previous, .. } = state {
        return if *previous == pin {
            verified.require_unexpired()
        } else {
            Err("testnet native startup cannot replace the active checkpoint".to_owned())
        };
    }
    // All failures and panics from the first possible durable mutation onward are terminal.
    // A success callback alone cannot install a host: signed verification and the concrete
    // release/artifact/journal loader must also succeed before publishing Active.
    *state = StartupState::Poisoned;
    verified.require_unexpired()?;
    freshness.retain_verified_bootstrap(pin)?;
    let retained = freshness.read_freshness()?;
    if retained.previous != Some(pin) {
        return Err(
            "testnet native startup freshness owner did not retain the verified checkpoint"
                .to_owned(),
        );
    }
    let verified = context.verify(archive, retained)?;
    let host = install(&verified)?;
    verified.require_unexpired()?;
    *state = StartupState::Active { pin, host };
    Ok(())
}

/// Exclusively use the actual activated native host without exporting its private state.
///
/// The callback is Rust-only and runs under the startup ownership mutex. It must not reenter
/// this accessor or activation. A panic poisons the session, requiring process restart.
///
/// # Errors
/// Rejects missing provisioning, incomplete/failed activation, or a poisoned ownership lock.
pub fn with_kagemusha_testnet_native_mobile_host_v1<R>(
    use_host: impl FnOnce(&KagemushaTestnetNativeMobileHostV1) -> Result<R, String>,
) -> Result<R, String> {
    let provisioned = STARTUP_SESSION
        .get()
        .ok_or_else(|| "testnet native startup context is unavailable".to_owned())?;
    // Check before touching a mutex potentially inherited while locked at fork.
    require_process_owner(provisioned.process_id)?;
    let session = provisioned
        .session
        .lock()
        .map_err(|_| "testnet native startup ownership lock is poisoned".to_owned())?;
    match &session.state {
        StartupState::Active { host, .. } => use_host(host),
        StartupState::Cold | StartupState::Poisoned => {
            Err("testnet native host is not active".to_owned())
        }
    }
}

/// Write the exact two-word startup contract and return its word count.
///
/// # Safety
/// `output` must point to writable, aligned storage for `capacity` `u32` words.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_testnet_native_startup_contract_v1(
    output: *mut u32,
    capacity: usize,
) -> i32 {
    if output.is_null() || !(output as usize).is_multiple_of(std::mem::align_of::<u32>()) {
        return ERR_KAGEMUSHA_V1;
    }
    if capacity < KAGEMUSHA_TESTNET_NATIVE_STARTUP_CONTRACT_V1.len() {
        return ERR_BUFFER_TOO_SMALL;
    }
    unsafe {
        std::ptr::copy_nonoverlapping(
            KAGEMUSHA_TESTNET_NATIVE_STARTUP_CONTRACT_V1.as_ptr(),
            output,
            2,
        )
    };
    2
}

/// Activate a preprovisioned Experimental native host with a signed bootstrap only.
///
/// Returns zero on activation or a freshly reverified exact-checkpoint retry, unavailable
/// when no Rust context is provisioned, and rejected on every other failure. It accepts no
/// root keys, storage paths, private openings, or operation-response trust anchors.
///
/// # Safety
/// `bootstrap` must remain readable and unchanged for `bootstrap_len` bytes during the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_testnet_native_startup_activate_v1(
    bootstrap: *const u8,
    bootstrap_len: usize,
) -> i32 {
    if bootstrap.is_null()
        || bootstrap_len == 0
        || bootstrap_len > KAGEMUSHA_MOBILE_BOOTSTRAP_MAX_BYTES_V1
        || (bootstrap as usize).checked_add(bootstrap_len).is_none()
    {
        return ERR_KAGEMUSHA_V1;
    }
    let Some(slot) = STARTUP_SESSION.get() else {
        return ERR_KAGEMUSHA_DEVICE_UNAVAILABLE_V1;
    };
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let archive = unsafe { std::slice::from_raw_parts(bootstrap, bootstrap_len) }.to_vec();
        require_process_owner(slot.process_id).map_err(|_| ())?;
        let mut session = slot.session.lock().map_err(|_| ())?;
        let StartupSession {
            context,
            freshness,
            state,
            ..
        } = &mut *session;
        activate(context, freshness.as_ref(), state, &archive, |verified| {
            context.install_host(verified)
        })
        .map_err(|_| ())
    }))
    .map_or(ERR_KAGEMUSHA_V1, |result| {
        result.map_or(ERR_KAGEMUSHA_V1, |()| 0)
    })
}

#[cfg(test)]
#[path = "kagemusha_testnet_native_startup_v1_tests.rs"]
mod tests;
