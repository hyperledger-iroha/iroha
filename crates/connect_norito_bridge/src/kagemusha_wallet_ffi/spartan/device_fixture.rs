//! Isolated device validation of the real registry, Session and C exports.
//!
//! Construction transfers an already authenticated FutureRuntime unchanged.
//! It covers exact Install retry, not the initial legacy NativeWalletRuntime
//! installation transfer. No public roots, provider mocks or proof verdicts are
//! accepted here. This module is absent from production bridge builds.

use super::{Action, FutureSession, Session};
use crate::kagemusha_wallet_ffi as ffi;
use iroha_core_zk::kagemusha_spartan_native::FutureRuntime;
use std::sync::{Arc, Mutex};

type Result<T> = std::result::Result<T, String>;

struct Admission<P, S>(Session<P, S>);
impl<P: ffi::advance::KagemushaWalletPlatformV1 + 'static, S: Send> ffi::open::Admission
    for Admission<P, S>
{
    fn spartan(&mut self, action: Action<'_>) -> ffi::Result<ffi::Response> {
        self.0.call(action)
    }
    fn begin(&mut self, _: [&[u8]; 4]) -> ffi::Result<Vec<u8>> {
        Err(ffi::Failure::code(ffi::INVALID))
    }
    fn finish(&mut self, _: &[u8]) -> ffi::Result<(Box<dyn ffi::Wallet>, ffi::state::Scheduler)> {
        Err(ffi::Failure::code(ffi::INVALID))
    }
    fn cancel(&mut self) -> ffi::Result<()> {
        Err(ffi::Failure::code(ffi::INVALID))
    }
}

#[derive(Debug, PartialEq, Eq)]
/// Exact successful result copied from one production C entry.
pub struct Reply {
    /// Native result status, unchanged from the C output.
    pub status: i32,
    /// Native result detail, unchanged from the C output.
    pub detail: u32,
    /// Owned copy of the native result; its C allocation has been released.
    pub bytes: Vec<u8>,
    token: u64,
}

#[derive(Debug)]
/// Native review and the process-local token that owns it.
pub struct Prepared {
    /// Token required to execute or discard this exact review.
    pub token: u64,
    /// Canonical review bytes returned by Native.
    pub review: Vec<u8>,
}

/// Actual OS observations of this process, sampled at the call boundary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProcessMetrics {
    /// Process-lifetime peak resident set size in bytes, not a per-operation delta.
    pub peak_rss_bytes: Option<u64>,
    /// Current total OS thread count, not the Rayon worker count or CPU count.
    pub process_threads: Option<u32>,
}

/// Read process metrics without substituting estimates for unavailable OS data.
pub fn process_metrics() -> ProcessMetrics {
    ProcessMetrics {
        peak_rss_bytes: peak_rss_bytes(),
        process_threads: process_threads(),
    }
}

#[cfg(unix)]
fn peak_rss_bytes() -> Option<u64> {
    let mut usage = std::mem::MaybeUninit::<libc::rusage>::zeroed();
    // SAFETY: writable storage for one rusage; RUSAGE_SELF samples this process.
    if unsafe { libc::getrusage(libc::RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
        return None;
    }
    // SAFETY: the successful call above initialized the result.
    let peak = u64::try_from(unsafe { usage.assume_init() }.ru_maxrss).ok()?;
    if cfg!(target_vendor = "apple") {
        Some(peak)
    } else {
        peak.checked_mul(1024)
    }
}

#[cfg(not(unix))]
fn peak_rss_bytes() -> Option<u64> {
    None
}

#[cfg(any(target_os = "linux", target_os = "android", test))]
fn status_threads(status: &str) -> Option<u32> {
    status
        .lines()
        .find_map(|line| {
            line.strip_prefix("Threads:")
                .and_then(|value| value.trim().parse().ok())
        })
        .filter(|count| *count != 0)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn process_threads() -> Option<u32> {
    status_threads(&std::fs::read_to_string("/proc/self/status").ok()?)
}

#[cfg(target_vendor = "apple")]
fn process_threads() -> Option<u32> {
    unsafe extern "C" {
        fn mach_port_deallocate(
            task: libc::mach_port_t,
            name: libc::mach_port_t,
        ) -> libc::kern_return_t;
    }
    struct Threads {
        task: libc::mach_port_t,
        ports: libc::thread_act_array_t,
        count: libc::mach_msg_type_number_t,
    }
    impl Drop for Threads {
        fn drop(&mut self) {
            if self.ports.is_null() {
                return;
            }
            // SAFETY: successful task_threads returns count owned send rights
            // in a VM allocation. Release every right, then the allocation.
            unsafe {
                for index in 0..self.count as usize {
                    let _ = mach_port_deallocate(self.task, *self.ports.add(index));
                }
                let size = self.count as libc::vm_size_t
                    * std::mem::size_of::<libc::thread_act_t>() as libc::vm_size_t;
                let _ = libc::vm_deallocate(self.task, self.ports as libc::vm_address_t, size);
            }
        }
    }
    // SAFETY: the port names this process; task_threads writes both outputs.
    let mut threads = Threads {
        task: unsafe { libc::mach_task_self() },
        ports: std::ptr::null_mut(),
        count: 0,
    };
    if unsafe { libc::task_threads(threads.task, &mut threads.ports, &mut threads.count) }
        != libc::KERN_SUCCESS
    {
        // Failed Mach RPCs have not transferred an array or send rights.
        threads.ports = std::ptr::null_mut();
        return None;
    }
    if threads.ports.is_null() || threads.count == 0 {
        return None;
    }
    Some(threads.count)
}

#[cfg(not(any(target_vendor = "apple", target_os = "linux", target_os = "android")))]
fn process_threads() -> Option<u32> {
    None
}

/// Sole handle to the actual provider, released through production Close.
pub struct SessionAdapter {
    handle: u64,
    parameters: [u8; 32],
    closed: bool,
    retained_outputs_only: bool,
    calls: [usize; 8],
}

struct OutputAllocation(*mut u8);
impl Drop for OutputAllocation {
    fn drop(&mut self) {
        crate::connect_norito_free(self.0);
    }
}

fn call(f: impl FnOnce(*mut ffi::WalletResult) -> i32) -> Result<Reply> {
    let mut out = ffi::WalletResult::default();
    let status = f(&mut out);
    let _allocation = OutputAllocation(out.bytes);
    // Copy then release through the same allocator contract as Swift/JNI. The
    // only producer of this pointer is a real production C export below.
    let bytes = if out.length == 0 {
        Vec::new()
    } else if out.bytes.is_null() {
        return Err("C export returned a null nonempty result".into());
    } else {
        let mut copy = Vec::new();
        copy.try_reserve_exact(out.length)
            .map_err(|_| "cannot copy the C result; native allocation released")?;
        copy.extend_from_slice(unsafe { std::slice::from_raw_parts(out.bytes, out.length) });
        copy
    };
    if status != 0 || out.status < 0 {
        return Err(format!(
            "C export refusal: return={status}, status={}, reason={}, platform={}",
            out.status, out.reason, out.platform_code
        ));
    }
    if out.sequence_high != 0 {
        return Err("unexpected high C token word".into());
    }
    Ok(Reply {
        status: out.status,
        detail: out.detail,
        bytes,
        token: out.sequence_low,
    })
}

impl SessionAdapter {
    /// Transfer a genuine runtime to the C registry and verify exact Install retry.
    ///
    /// Returns an error if registration or an installation/token boundary check fails.
    pub fn new<P, S>(
        runtime: FutureRuntime<ffi::advance::KagemushaWalletStdFsV1, P, S>,
        bundle: &[u8],
        manifest: &[u8],
        signature: &[u8; 64],
    ) -> Result<Self>
    where
        P: ffi::advance::KagemushaWalletPlatformV1 + 'static,
        S: Send + 'static,
    {
        let parameters = *runtime.parameter_id();
        let owner = Arc::new(ffi::open::RuntimeOwner {
            closing: Arc::new(ffi::closing::CloseState::default()),
            admission: Mutex::new(Some(Box::new(Admission(Session::new(
                runtime, bundle, manifest, signature,
            ))))),
            finished: Mutex::new(None),
        });
        let handle = ffi::open::register_owned_runtime(ffi::registry(), &owner)
            .map_err(|e| format!("register actual future owner: {e:?}"))?;
        let mut this = Self {
            handle,
            parameters,
            closed: false,
            retained_outputs_only: false,
            calls: [0; 8],
        };
        let first = this.install(bundle, manifest, signature)?;
        let retry = this.install(bundle, manifest, signature)?;
        if first != retry
            || first.status != 81
            || first.bytes != parameters
            || first.token != handle
        {
            return Err("exact Install retry changed native parameter owner".into());
        }
        let mut changed = *signature;
        changed[0] ^= 1;
        if this.install(bundle, manifest, &changed).is_ok() {
            return Err("changed installation originals were admitted".into());
        }
        if this.execute(u64::MAX).is_ok() {
            return Err("invalid review token was admitted".into());
        }
        Ok(this)
    }

    fn install(&mut self, bundle: &[u8], manifest: &[u8], signature: &[u8; 64]) -> Result<Reply> {
        self.calls[0] += 1;
        call(|out| unsafe {
            ffi::connect_norito_kagemusha_spartan_install_v1(
                self.handle,
                bundle.as_ptr(),
                bundle.len(),
                manifest.as_ptr(),
                manifest.len(),
                signature.as_ptr(),
                64,
                out,
            )
        })
    }

    /// Begin account admission through C with credential, certificates, account and asset originals.
    pub fn begin_account(&mut self, originals: [&[u8]; 4]) -> Result<[u8; 32]> {
        self.calls[1] += 1;
        let [credential, certificates, account, asset] = originals;
        let request = ffi::WalletOpenRequest {
            credential: credential.as_ptr(),
            credential_length: credential.len(),
            certificates: certificates.as_ptr(),
            certificates_length: certificates.len(),
            account: account.as_ptr(),
            account_length: account.len(),
            asset: asset.as_ptr(),
            asset_length: asset.len(),
        };
        let reply = call(|out| unsafe {
            ffi::connect_norito_kagemusha_spartan_begin_account_v1(self.handle, &request, out)
        })?;
        if reply.status != 15 || reply.token != self.handle {
            return Err("wrong account challenge response".into());
        }
        reply
            .bytes
            .try_into()
            .map_err(|_| "account challenge must be32 bytes".into())
    }

    /// Authorize the current account challenge and return its authenticated wallet identity.
    pub fn authorize_account(&mut self, signature: &[u8]) -> Result<[u8; 32]> {
        self.calls[2] += 1;
        let reply = call(|out| unsafe {
            ffi::connect_norito_kagemusha_spartan_authorize_account_v1(
                self.handle,
                signature.as_ptr(),
                signature.len(),
                out,
            )
        })?;
        if !matches!(reply.status, 85 | 87)
            || reply.token != self.handle
            || reply.bytes.len() != 64
            || reply.bytes[..32] != self.parameters
        {
            return Err("wrong authenticated account parameter identity".into());
        }
        self.retained_outputs_only = reply.status == 87;
        Ok(reply.bytes[32..].try_into().unwrap())
    }

    /// Read the access mode returned by the most recent successful C account authorization.
    pub fn retained_outputs_only(&self) -> bool {
        self.retained_outputs_only
    }

    /// Read the genuine received-credit acknowledgement through the retail C ABI.
    pub fn credit_status(
        &mut self,
        slot: [u8; 32],
        request: [u8; 32],
        receive: [u8; 32],
    ) -> Result<Vec<u8>> {
        self.read_receiver_credit_status(slot, request, receive, 1, &[])
    }

    /// Read a receiver acknowledgement bound by the C owner to the exact carrier Payment.
    pub fn credit_status_for_payment(
        &mut self,
        slot: [u8; 32],
        request: [u8; 32],
        receive: [u8; 32],
        payment: &[u8],
    ) -> Result<Vec<u8>> {
        self.read_receiver_credit_status(slot, request, receive, 3, payment)
    }

    fn read_receiver_credit_status(
        &mut self,
        slot: [u8; 32],
        request: [u8; 32],
        receive: [u8; 32],
        selector: u32,
        payment: &[u8],
    ) -> Result<Vec<u8>> {
        let reply = call(|out| unsafe {
            ffi::connect_norito_kagemusha_spartan_credit_status_v1(
                self.handle,
                slot.as_ptr(),
                selector,
                receive.as_ptr(),
                request.as_ptr(),
                request.len(),
                payment.as_ptr(),
                payment.len(),
                out,
            )
        })?;
        if reply.status != 104 || reply.token != 0 || !(85..=10_084).contains(&reply.bytes.len()) {
            return Err("wrong C receipt response".into());
        }
        Ok(reply.bytes)
    }
    /// Verify an independently produced peer acknowledgement against the local Send.
    pub fn verify_credit_status(
        &mut self,
        slot: [u8; 32],
        send: [u8; 32],
        open: &[u8],
        status: &[u8],
    ) -> Result<bool> {
        let reply = call(|out| unsafe {
            ffi::connect_norito_kagemusha_spartan_credit_status_v1(
                self.handle,
                slot.as_ptr(),
                2,
                send.as_ptr(),
                open.as_ptr(),
                open.len(),
                status.as_ptr(),
                status.len(),
                out,
            )
        })?;
        match (reply.status, reply.token, reply.bytes.as_slice()) {
            (105, 0, [1]) => Ok(true),
            (105, 0, [0]) => Ok(false),
            _ => Err("wrong C receipt verification response".into()),
        }
    }

    /// Issue or retry an open Request using the source-bound hardware key through the C entry.
    pub fn open_request(
        &mut self,
        slot: &[u8; 32],
        id: &[u8; 32],
        amount: u128,
    ) -> Result<Vec<u8>> {
        let reply = call(|out| unsafe {
            ffi::connect_norito_kagemusha_spartan_issue_open_request_v1(
                self.handle,
                slot.as_ptr(),
                id.as_ptr(),
                amount.to_le_bytes().as_ptr(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                out,
            )
        })?;
        if reply.status != 96 || reply.token != 0 || reply.bytes.is_empty() {
            return Err("invalid C open Request result".into());
        }
        Ok(reply.bytes)
    }

    /// Install and read back the scheme-authenticated local list through the stock C entry.
    pub fn install_blacklist(
        &mut self,
        slot: &[u8; 32],
        original: &[u8],
        certificates: &[u8],
    ) -> Result<[u8; 32]> {
        let reply = call(|out| unsafe {
            ffi::connect_norito_kagemusha_spartan_install_blacklist_v1(
                self.handle,
                slot.as_ptr(),
                original.as_ptr(),
                original.len(),
                certificates.as_ptr(),
                certificates.len(),
                out,
            )
        })?;
        if reply.status != 95 || reply.token != 0 || reply.bytes == [0; 32] {
            return Err("invalid C local blacklist acknowledgment".into());
        }
        reply
            .bytes
            .try_into()
            .map_err(|_| "invalid C local blacklist digest length".into())
    }

    /// Prepare the exact witness and current head through the production C entry.
    pub fn prepare(&mut self, slot: &[u8; 32], witness: &[u8], head: &[u8]) -> Result<Prepared> {
        self.calls[3] += 1;
        let reply = call(|out| unsafe {
            ffi::connect_norito_kagemusha_spartan_prepare_v1(
                self.handle,
                slot.as_ptr(),
                witness.as_ptr(),
                witness.len(),
                head.as_ptr(),
                head.len(),
                out,
            )
        })?;
        let valid_projection = matches!(
            (reply.bytes.get(..3), reply.bytes.len()),
            (Some([1, 0, 1..=4]), 291) | (Some([2, 0, 7]), 243)
        );
        if reply.status != 82 || reply.token == 0 || !valid_projection {
            return Err("wrong native review response".into());
        }
        Ok(Prepared {
            token: reply.token,
            review: reply.bytes,
        })
    }

    /// Obtain the asset digest of this exact prepared review for iOS presentation.
    #[cfg(target_os = "ios")]
    pub fn apple_debit_asset(&mut self, token: u64) -> Result<[u8; 33]> {
        let selector = [2u8];
        let reply = call(|out| unsafe {
            ffi::connect_norito_kagemusha_spartan_confirm_v1(
                self.handle,
                token,
                std::ptr::null(),
                0,
                selector.as_ptr(),
                1,
                out,
            )
        })?;
        if reply.status != 94
            || reply.token != 0
            || !reply.bytes.get(32).is_some_and(|scale| *scale <= 28)
        {
            return Err("missing Native review asset".into());
        }
        reply
            .bytes
            .as_slice()
            .try_into()
            .map_err(|_| "wrong Native asset width".into())
    }

    /// Exercise the same fresh Secure Enclave prompt used by the physical iOS SDK.
    #[cfg(target_os = "ios")]
    pub fn confirm_apple_debit(&mut self, token: u64) -> Result<()> {
        let send = |message: &[u8], signature: &[u8]| {
            call(|out| unsafe {
                ffi::connect_norito_kagemusha_spartan_confirm_v1(
                    self.handle,
                    token,
                    message.as_ptr(),
                    message.len(),
                    signature.as_ptr(),
                    signature.len(),
                    out,
                )
            })
        };
        let provisioned = send(&[], &[1])?;
        if provisioned.status != 93 || !provisioned.bytes.is_empty() {
            return Err("wrong Native iOS debit key provisioning response".into());
        }
        let challenge = send(&[], &[])?;
        if challenge.status != 88 || challenge.bytes.len() != 32 {
            return Err("wrong Native iOS debit challenge".into());
        }
        eprintln!("SPARTAN_CONFIRMATION_REQUIRED");
        let retained = match send(&challenge.bytes, &[0]) {
            Ok(result) => result,
            Err(error) => {
                let _ = send(&challenge.bytes, &[]);
                return Err(error);
            }
        };
        if retained.status != 89 || !retained.bytes.is_empty() {
            return Err("missing verified Native iOS user-presence record".into());
        }
        eprintln!("SPARTAN_CONFIRMATION_GRANTED");
        Ok(())
    }

    /// Execute or retry the Native-owned review token through C.
    pub fn execute(&mut self, token: u64) -> Result<Reply> {
        self.calls[4] += 1;
        call(|out| unsafe {
            ffi::connect_norito_kagemusha_spartan_execute_v1(self.handle, token, out)
        })
    }

    /// Retrieve an exact retained output part through C.
    pub fn output(&mut self, slot: &[u8; 32], operation: &[u8; 32], part: u32) -> Result<Reply> {
        self.calls[5] += 1;
        call(|out| unsafe {
            ffi::connect_norito_kagemusha_spartan_output_v1(
                self.handle,
                slot.as_ptr(),
                operation.as_ptr(),
                part,
                out,
            )
        })
    }

    /// Read the wallet's next Load ordinal from its released head through C.
    pub fn next_load_ordinal(&mut self, slot: &[u8; 32]) -> Result<u128> {
        let reply = self.output(slot, &[0; 32], 4)?;
        if reply.status != 106 || reply.detail != 0 || reply.token != 0 {
            return Err("wrong C next Load ordinal status".into());
        }
        let bytes = reply
            .bytes
            .try_into()
            .map_err(|_| "wrong C next Load ordinal length")?;
        Ok(u128::from_le_bytes(bytes))
    }

    /// Issue or recover the exact independently signed Bootstrap activation over C.
    pub fn activation(&mut self, slot: &[u8; 32]) -> Result<Vec<u8>> {
        let reply = self.output(slot, &[0; 32], 5)?;
        if reply.status != 107
            || reply.detail != 0
            || reply.token != 0
            || !(129..=32_896).contains(&reply.bytes.len())
        {
            return Err("wrong C activation response".into());
        }
        Ok(reply.bytes)
    }

    /// Verify a complete Payment through C and return its canonical public projection.
    pub fn verify_payment(&mut self, payment: &[u8]) -> Result<Vec<u8>> {
        self.calls[6] += 1;
        let reply = call(|out| unsafe {
            ffi::connect_norito_kagemusha_spartan_verify_payment_v1(
                self.handle,
                payment.as_ptr(),
                payment.len(),
                out,
            )
        })?;
        if reply.status != 83 || reply.token != 0 || reply.bytes.len() != 514 {
            return Err("wrong verified Payment projection".into());
        }
        Ok(reply.bytes)
    }

    /// Idempotently discard an unused review token through C.
    pub fn discard(&mut self, token: u64) -> Result<()> {
        self.calls[7] += 1;
        let reply = call(|out| unsafe {
            ffi::connect_norito_kagemusha_spartan_discard_v1(self.handle, token, out)
        })?;
        if reply.status != 84 || reply.token != 0 || !reply.bytes.is_empty() {
            return Err("wrong discard response".into());
        }
        Ok(())
    }

    /// Confirm the actual fixture invoked every production Spartan C entry.
    pub fn require_all_entries(&self) -> Result<()> {
        if self.calls.contains(&0) {
            return Err(format!("unexercised Spartan C entry: {:?}", self.calls));
        }
        Ok(())
    }

    /// Close the actual C owner and verify that subsequent calls are refused.
    pub fn close(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        let status = ffi::connect_norito_kagemusha_wallet_close_v1(self.handle);
        if status != 0 {
            return Err(format!("actual C owner close: {status}"));
        }
        self.closed = true;
        // Discard of an unknown token succeeds on any open owner. Using it
        // here distinguishes closure from merely rejecting an invalid token.
        if self.discard(u64::MAX).is_ok() {
            return Err("closed C owner accepted a call".into());
        }
        Ok(())
    }
}

impl Drop for SessionAdapter {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn process_metrics_report_actual_os_values_and_parse_threads_strictly() {
        assert_eq!(status_threads("Name:\tfixture\nThreads:\t17\n"), Some(17));
        assert_eq!(status_threads("Threads:\t0\n"), None);
        assert_eq!(status_threads("Threads:\t17 extra\n"), None);
        assert_eq!(status_threads("Threads:\t-1\n"), None);
        let metrics = process_metrics();
        #[cfg(unix)]
        assert!(metrics.peak_rss_bytes.is_some_and(|bytes| bytes > 0));
        #[cfg(any(target_vendor = "apple", target_os = "linux", target_os = "android"))]
        assert!(metrics.process_threads.is_some_and(|threads| threads > 0));
    }
    #[test]
    fn production_c_export_refuses_unowned_handle_without_output() {
        assert!(
            call(|out| unsafe {
                ffi::connect_norito_kagemusha_spartan_execute_v1(u64::MAX, u64::MAX, out)
            })
            .is_err()
        );
        assert!(
            call(|out| unsafe {
                ffi::connect_norito_kagemusha_spartan_install_v1(
                    u64::MAX,
                    std::ptr::null(),
                    0,
                    std::ptr::null(),
                    0,
                    std::ptr::null(),
                    0,
                    out,
                )
            })
            .is_err()
        );
    }
}
