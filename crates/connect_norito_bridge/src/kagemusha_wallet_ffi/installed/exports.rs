//! One original-only C install entry point. Existing open/finish/cancel/close own custody.

use super::*;

/// Exact public originals selected by the authenticated application release.
/// Empty financial trio is unavailable only after the mandatory signed base is verified.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WalletRuntimeOriginals {
    /// Whole signed application manifest, at most 8 MiB.
    pub app_manifest: *const u8,
    /// Exact whole application manifest extent.
    pub app_manifest_length: usize,
    /// Whole fixed v6 signature envelope, at most 2048 bytes.
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
/// Authenticate installation originals under immutable Native build trust and retain the
/// actual platform/provider in the existing Native runtime registry. Supplies no loose IDs.
/// All seven installation originals are mandatory; absent inputs return INVALID with no handle.
/// Success performs no key generation, account admission, payment signing or monetary step.
/// # Safety
/// Request and callbacks are readable and correctly aligned; declared original buffers
/// remain initialized during this call. `out_runtime` is writable, aligned output. Callback
/// context is retained/released exactly by Native and satisfies PlatformCallbacks lifetime.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn connect_norito_kagemusha_wallet_install_runtime_v1(
    request: *const WalletRuntimeOriginals,
    callbacks: *const PlatformCallbacks,
    out_runtime: *mut u64,
) -> i32 {
    if out_runtime.is_null() {
        return INVALID;
    }
    // SAFETY: admitted caller output is initialized before any fallible work.
    unsafe {
        out_runtime.write(0);
    }
    let result = run(|| {
        // SAFETY: readable aligned original request is supplied by the caller.
        let request = unsafe { request.as_ref() }.ok_or(Failure::code(INVALID))?;
        let prepared = PreparedInstallation::load(unsafe { request.originals()? })?;
        // Runtime authentication/complete qualification precedes any platform upcall.
        let callbacks = *unsafe { callbacks.as_ref() }.ok_or(Failure::code(INVALID))?;
        let platform = unsafe { CallbackPlatform::new(callbacks)? };
        let root = platform
            .custody_root()
            .map_err(|error| Failure::unavailable(UNAVAILABLE, error))?;
        prepared.register(platform, root, false)
    });
    match result {
        Ok(handle) => {
            unsafe {
                out_runtime.write(handle);
            }
            0
        }
        Err(error) => error.status,
    }
}
