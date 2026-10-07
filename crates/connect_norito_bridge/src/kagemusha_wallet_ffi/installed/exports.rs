//! Original-only installation attempt, consuming registration and explicit close acknowledgment.

use super::*;

/// Exact public originals selected by the authenticated application release.
/// Four base originals are mandatory; the financial trio must be wholly present or absent.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WalletRuntimeOriginals {
    /// Whole signed application manifest, at most 8 MiB.
    pub app_manifest: *const u8,
    /// Exact whole application manifest extent.
    pub app_manifest_length: usize,
    /// Whole authority-selected signature envelope, at most 2048 bytes.
    pub envelope: *const u8,
    /// Exact signature-envelope extent.
    pub envelope_length: usize,
    /// Whole signed-selected Core runtime original, at most 128 KiB.
    pub wallet_runtime: *const u8,
    /// Exact Core runtime extent.
    pub wallet_runtime_length: usize,
    /// Genuine original complete signed verifier pack.
    pub verifier_pack: *const u8,
    /// Exact verifier pack extent.
    pub verifier_pack_length: usize,
    /// Genuine original complete signed producer inventory.
    pub producer_inventory: *const u8,
    /// Exact producer inventory extent.
    pub producer_inventory_length: usize,
    /// Exact independently signed-selected native genesis block, at most 64 MiB.
    pub signed_genesis: *const u8,
    /// Exact genesis block extent.
    pub signed_genesis_length: usize,
    /// Actual private no-follow financial metadata/original root as UTF-8 DATA.
    pub originals_root: *const u8,
    /// Exact root extent, at most 4096 bytes; no caller-selected role paths.
    pub originals_root_length: usize,
}
unsafe fn bytes<'a>(pointer: *const u8, length: usize, bound: usize) -> Result<&'a [u8]> {
    if length > bound || (pointer.is_null() && length != 0) {
        return Err(Failure::code(INVALID));
    }
    if length == 0 {
        return Ok(&[]);
    }
    // SAFETY: the caller supplies exactly declared initialized bytes, bounded above.
    Ok(unsafe { std::slice::from_raw_parts(pointer, length) })
}
impl WalletRuntimeOriginals {
    unsafe fn originals(&self) -> Result<RuntimeOriginals<'_>> {
        // SAFETY: the export contract supplies all readable original slices. Bounds
        // are checked before each pointer is converted, and decoding allocates later.
        unsafe {
            Ok(RuntimeOriginals {
                app_manifest: bytes(
                    self.app_manifest,
                    self.app_manifest_length,
                    APP_MANIFEST_MAX,
                )?,
                envelope: bytes(self.envelope, self.envelope_length, ENVELOPE_MAX)?,
                wallet_runtime: bytes(
                    self.wallet_runtime,
                    self.wallet_runtime_length,
                    WALLET_RUNTIME_MAX,
                )?,
                verifier_pack: bytes(
                    self.verifier_pack,
                    self.verifier_pack_length,
                    VERIFIER_PACK_MAX_BYTES_V1,
                )?,
                producer_inventory: bytes(
                    self.producer_inventory,
                    self.producer_inventory_length,
                    CATALOG_MAX_BYTES_V1,
                )?,
                signed_genesis: bytes(
                    self.signed_genesis,
                    self.signed_genesis_length,
                    GENESIS_MAX,
                )?,
                originals_root: bytes(self.originals_root, self.originals_root_length, ROOT_MAX)?,
            })
        }
    }
}
/// Authenticate the four base originals under immutable Native build trust.
/// BPNG's signed null financial selection with an absent trio returns -4 before
/// any platform upcall, retaining no attempt or runtime. Full signed selections
/// require all three financial inputs and genuine financial graph qualification.
/// Complete success retains the runtime/provider with its authenticated app binding.
/// This creates no registry ID/reservation, key generation or account/monetary authority.
/// # Safety
/// Readable aligned request/callbacks and initialized declared buffers are required.
/// `out_attempt` is writable/aligned and initially owns no other attempt. The returned
/// opaque pointer must be owned once and serialized until consuming registration/close.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_installation_begin_v1(
    request: *const WalletRuntimeOriginals,
    callbacks: *const PlatformCallbacks,
    out_attempt: *mut *mut WalletInstallationAttempt,
) -> i32 {
    if out_attempt.is_null() {
        return INVALID;
    }
    // SAFETY: caller provides one fresh writable output.
    unsafe {
        out_attempt.write(std::ptr::null_mut());
    }
    let result = run(|| {
        // SAFETY: admitted C caller supplies aligned original request and callback table.
        let request = unsafe { request.as_ref() }.ok_or(Failure::code(INVALID))?;
        let prepared = PreparedInstallation::load(unsafe { request.originals()? })?;
        let callbacks = *unsafe { callbacks.as_ref() }.ok_or(Failure::code(INVALID))?;
        let platform = unsafe { CallbackPlatform::new(callbacks)? };
        let root = platform
            .custody_root()
            .map_err(|error| Failure::unavailable(UNAVAILABLE, error))?;
        let owner = prepared.runtime(platform, root, false)?;
        Ok(Box::new(WalletInstallationAttempt::new(owner)))
    });
    match result {
        Ok(attempt) => {
            unsafe {
                out_attempt.write(Box::into_raw(attempt));
            }
            0
        }
        Err(error) => error.status,
    }
}
/// Register the SAME loaded attempt in the single existing Native registry.
/// Ordinary refusal leaves `*attempt` unchanged. Zero transfers all custody/binding
/// once to `out_runtime` and clears/frees the attempt. No reloading or platform upcall.
/// # Safety
/// `attempt` owns exactly the live unique pointer returned by installation_begin.
/// Calls and close are serialized. Writable outputs are aligned and do not overlap.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_installation_register_v1(
    attempt: *mut *mut WalletInstallationAttempt,
    out_runtime: *mut u64,
) -> i32 {
    if out_runtime.is_null() {
        return INVALID;
    }
    unsafe {
        out_runtime.write(0);
    }
    let result = run(|| {
        let slot = unsafe { attempt.as_mut() }.ok_or(Failure::code(INVALID))?;
        // SAFETY: the unique live Native allocation is exclusively borrowed for this call.
        let owner = unsafe { (*slot).as_mut() }.ok_or(Failure::code(INVALID))?;
        let handle = owner.register(registry())?;
        // SAFETY: unique live Native allocation, now transferred to the actual registry.
        unsafe {
            drop(Box::from_raw(*slot));
            out_runtime.write(handle);
        }
        *slot = std::ptr::null_mut();
        Ok(())
    });
    result.map_or_else(|error| error.status, |()| 0)
}
/// Irrevocably fence registration and join the SAME unadmitted custody owner.
/// Only zero clears/frees the opaque allocation. A normal refusal retains the exact
/// owner for close retry; no result implies key deletion or replacement permission.
/// # Safety
/// Exact unique live attempt from installation_begin; serialized with registration.
/// The pointer-to-pointer is writable/aligned. Never use the consumed pointer again.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_installation_close_v1(
    attempt: *mut *mut WalletInstallationAttempt,
) -> i32 {
    let result = run(|| {
        let slot = unsafe { attempt.as_mut() }.ok_or(Failure::code(INVALID))?;
        // SAFETY: the unique live Native allocation is exclusively borrowed for this call.
        let owner = unsafe { (*slot).as_mut() }.ok_or(Failure::code(INVALID))?;
        owner.close()?;
        // SAFETY: same unique allocation, freed only after the existing join acknowledged zero.
        unsafe {
            drop(Box::from_raw(*slot));
        }
        *slot = std::ptr::null_mut();
        Ok(())
    });
    result.map_or_else(|error| error.status, |()| 0)
}
