//! Image-local protection before either native encoder reads private signer inputs.
//!
//! These kernel observations confer no Native issuer installation, signing role,
//! policy, financial authority or credential admission. All caller/FD/model gates
//! remain independently required. The disposable probe only targets its own parent.

use std::io;

fn unavailable() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "private signer process protection unavailable",
    )
}

/// Observe protection in this executable image before private request/seed intake.
pub(super) fn protect() -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        use rustix::process::{
            DumpableBehavior, Resource, Rlimit, dumpable_behavior, getrlimit,
            set_dumpable_behavior, setrlimit,
        };
        let zero = Rlimit {
            current: Some(0),
            maximum: Some(0),
        };
        setrlimit(Resource::Core, zero)?;
        if getrlimit(Resource::Core) != zero {
            return Err(unavailable());
        }
        set_dumpable_behavior(DumpableBehavior::NotDumpable)?;
        if dumpable_behavior()? != DumpableBehavior::NotDumpable {
            return Err(unavailable());
        }
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    {
        darwin::protect()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        Err(unavailable())
    }
}

// Exact process-local Darwin syscalls need FFI; this allowance is confined to
// the private protection module, with bounded buffers and one owned child.
#[cfg(target_os = "macos")]
#[allow(unsafe_code)]
mod darwin {
    use super::{io, unavailable};
    use std::{
        ffi::c_void,
        mem::size_of,
        ptr,
        time::{Duration, Instant},
    };

    const MAX_FDS: usize = 1024;
    const PACKET_LEN: usize = 24;
    const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
    const PT_DENY_ATTACH: i32 = 31;
    const PT_ATTACHEXC: i32 = 14;
    const PT_DETACH: i32 = 11;
    const SIGCHLD: i32 = 20;
    const SIGSEGV: i32 = 11;
    const SIGKILL: i32 = 9;
    const WNOHANG: i32 = 1;
    const F_GETFD: i32 = 1;
    const F_SETFD: i32 = 2;
    const F_GETFL: i32 = 3;
    const F_SETFL: i32 = 4;
    const FD_CLOEXEC: i32 = 1;
    const O_NONBLOCK: i32 = 4;

    // Installed Darwin sys/resource.h and sys/{signal,proc_info}.h originals.
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Limit {
        current: u64,
        maximum: u64,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct Action {
        handler: usize,
        mask: u32,
        flags: i32,
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct TaskInfo {
        times: [u64; 6],
        counters: [i32; 12],
    }
    #[repr(C)]
    #[derive(Clone, Copy, Default)]
    struct FdInfo {
        descriptor: i32,
        kind: u32,
    }

    unsafe extern "C" {
        fn getpid() -> i32;
        fn getppid() -> i32;
        fn getuid() -> u32;
        fn geteuid() -> u32;
        fn getgid() -> u32;
        fn getegid() -> u32;
        fn getrlimit(resource: i32, limit: *mut Limit) -> i32;
        fn setrlimit(resource: i32, limit: *const Limit) -> i32;
        fn sigaction(signal: i32, action: *const Action, old: *mut Action) -> i32;
        fn sigprocmask(how: i32, mask: *const u32, old: *mut u32) -> i32;
        fn proc_pidinfo(pid: i32, flavor: i32, arg: u64, buffer: *mut c_void, size: i32) -> i32;
        fn ptrace(request: i32, pid: i32, address: *mut c_void, data: i32) -> i32;
        fn pipe(descriptors: *mut i32) -> i32;
        fn fcntl(descriptor: i32, command: i32, ...) -> i32;
        fn fork() -> i32;
        fn close(descriptor: i32) -> i32;
        fn read(descriptor: i32, buffer: *mut c_void, length: usize) -> isize;
        fn write(descriptor: i32, buffer: *const c_void, length: usize) -> isize;
        fn waitpid(pid: i32, status: *mut i32, options: i32) -> i32;
        fn kill(pid: i32, signal: i32) -> i32;
        fn _exit(status: i32) -> !;
        fn mach_task_self() -> u32;
        fn mach_port_deallocate(task: u32, port: u32) -> i32;
        fn task_for_pid(task: u32, pid: i32, port: *mut u32) -> i32;
        fn task_read_for_pid(task: u32, pid: i32, port: *mut u32) -> i32;
        fn task_inspect_for_pid(task: u32, pid: i32, port: *mut u32) -> i32;
    }

    fn zero_core() -> io::Result<()> {
        let zero = Limit::default();
        // SAFETY: fixed resource constant; initialized exact-layout limit pointer.
        if unsafe { setrlimit(4, &zero) } != 0 || !core_is_zero() {
            return Err(unavailable());
        }
        Ok(())
    }

    fn core_is_zero() -> bool {
        let mut value = Limit::default();
        // SAFETY: writable initialized exact-layout resource limit.
        unsafe { getrlimit(4, &mut value) == 0 && value.current == 0 && value.maximum == 0 }
    }

    fn single_thread() -> bool {
        let mut value = TaskInfo::default();
        // SAFETY: PROC_PIDTASKINFO=4, fixed 96-byte writable initialized record.
        size_of::<TaskInfo>() == 96
            && unsafe {
                proc_pidinfo(getpid(), 4, 0, ptr::from_mut(&mut value).cast(), 96) == 96
                    && value.counters[9] == 1
            }
    }

    fn descriptor_inventory(values: &mut [FdInfo; MAX_FDS]) -> Option<usize> {
        // SAFETY: PROC_PIDLISTFDS=1 fills only this fixed 8192-byte live buffer.
        let bytes = unsafe {
            proc_pidinfo(
                getpid(),
                1,
                0,
                values.as_mut_ptr().cast(),
                size_of::<[FdInfo; MAX_FDS]>() as i32,
            )
        };
        if bytes <= 0
            || bytes as usize >= size_of::<[FdInfo; MAX_FDS]>()
            || bytes as usize % size_of::<FdInfo>() != 0
        {
            return None;
        }
        let count = bytes as usize / size_of::<FdInfo>();
        for (index, value) in values[..count].iter().enumerate() {
            if value.descriptor < 0
                || values[..index]
                    .iter()
                    .any(|old| old.descriptor == value.descriptor)
            {
                return None;
            }
        }
        Some(count)
    }

    // Only a fixed public packet and kernel operations run after the verified
    // single-thread fork. No allocation, private descriptor read, stdout or key
    // parsing is performed by this child. Every inherited FD is closed first.
    fn child_probe(target: i32, uid: u32, output: i32) -> ! {
        let mut descriptors = [FdInfo::default(); MAX_FDS];
        let Some(count) = descriptor_inventory(&mut descriptors) else {
            unsafe { _exit(80) }
        };
        for item in &descriptors[..count] {
            if item.descriptor != output {
                // SAFETY: each actual own FD appears once; this child owns closure.
                if unsafe { close(item.descriptor) } != 0 {
                    unsafe { _exit(81) }
                }
            }
        }
        let Some(count) = descriptor_inventory(&mut descriptors) else {
            unsafe { _exit(82) }
        };
        // SAFETY: these read only process-local scalar kernel identities.
        if count != 1
            || descriptors[0].descriptor != output
            || !core_is_zero()
            || unsafe { getppid() != target || getuid() != uid || geteuid() != uid }
        {
            unsafe { _exit(83) }
        }
        // Rust may install a stack-overflow SIGSEGV handler that consumes the
        // one-shot deny-attach signal. Normalize ONLY this disposable child to
        // the default/unblocked disposition, and observe it before probing.
        // The parent's handlers and mask are never changed.
        let default = Action::default();
        let mut observed = Action::default();
        let segv_mask = 1u32 << (SIGSEGV - 1);
        let mut observed_mask = 0u32;
        // SAFETY: exact-layout initialized records; all operations are child-local.
        if unsafe { sigaction(SIGSEGV, &default, ptr::null_mut()) } != 0
            || unsafe { sigaction(SIGSEGV, ptr::null(), &mut observed) } != 0
            || observed.handler != 0
            || unsafe { sigprocmask(2, &segv_mask, ptr::null_mut()) } != 0
            || unsafe { sigprocmask(3, ptr::null(), &mut observed_mask) } != 0
            || observed_mask & segv_mask != 0
        {
            unsafe { _exit(91) }
        }
        let mut controls = 0u32;
        let mut denials = 0u32;
        // SAFETY: mach_task_self returns only this child's actual task send right.
        let task = unsafe { mach_task_self() };
        let apis: [unsafe extern "C" fn(u32, i32, *mut u32) -> i32; 3] =
            [task_for_pid, task_read_for_pid, task_inspect_for_pid];
        for (index, api) in apis.iter().enumerate() {
            let mut own = 0u32;
            // SAFETY: actual own task/PID and a writable fixed port slot.
            let own_result = unsafe { api(task, getpid(), &mut own) };
            if own_result == 0 && own != 0 {
                controls |= 1 << index;
            }
            if own != 0 && unsafe { mach_port_deallocate(task, own) } != 0 {
                unsafe { _exit(84) }
            }
            let mut offered = 0u32;
            // SAFETY: the only target is the actual live parent captured before fork.
            let result = unsafe { api(task, target, &mut offered) };
            if result != 0 && offered == 0 {
                denials |= 1 << index;
            }
            if offered != 0 && unsafe { mach_port_deallocate(task, offered) } != 0 {
                unsafe { _exit(85) }
            }
        }
        let Some(count) = descriptor_inventory(&mut descriptors) else {
            unsafe { _exit(86) }
        };
        if count != 1 || descriptors[0].descriptor != output {
            unsafe { _exit(87) }
        }
        let mut packet = [0u8; PACKET_LEN];
        packet[..4].copy_from_slice(b"RPV1");
        // SAFETY: getpid reads this child's own scalar identity.
        for (slot, value) in [
            target as u32,
            uid,
            unsafe { getpid() } as u32,
            controls,
            denials,
        ]
        .iter()
        .enumerate()
        {
            packet[4 + slot * 4..8 + slot * 4].copy_from_slice(&value.to_le_bytes());
        }
        // SAFETY: fixed public initialized packet; output is the child's sole FD.
        if unsafe { write(output, packet.as_ptr().cast(), packet.len()) } != packet.len() as isize {
            unsafe { _exit(88) }
        }
        if unsafe { close(output) } != 0 || controls != 7 || denials != 7 {
            unsafe { _exit(89) }
        }
        // PT_DENY_ATTACH must kill this owned caller with SIGSEGV/no core. Generic
        // errno is not accepted. Unexpected successful attach is immediately
        // detached so the unprotected public negative fixture can resume/refuse.
        if unsafe { ptrace(PT_ATTACHEXC, target, ptr::null_mut(), 0) } == 0 {
            unsafe {
                ptrace(PT_DETACH, target, 1usize as *mut c_void, 0);
            }
        }
        unsafe { _exit(90) }
    }

    struct OwnedProbe {
        child: i32,
        read_fd: i32,
        write_fd: i32,
        reaped: bool,
    }
    impl Drop for OwnedProbe {
        fn drop(&mut self) {
            // SAFETY: positive PID is only our still-unreaped child; no offered
            // PID/group/other task is signalled. Its owned PID cannot be reused.
            if self.child > 0 && !self.reaped {
                unsafe {
                    kill(self.child, SIGKILL);
                    let mut status = 0;
                    while waitpid(self.child, &mut status, 0) < 0 {
                        if io::Error::last_os_error().kind() != io::ErrorKind::Interrupted {
                            break;
                        }
                    }
                }
            }
            if self.read_fd >= 0 {
                unsafe {
                    close(self.read_fd);
                }
            }
            if self.write_fd >= 0 {
                unsafe {
                    close(self.write_fd);
                }
            }
        }
    }

    fn valid_result(packet: &[u8], status: i32, target: i32, uid: u32, child: i32) -> bool {
        if packet.len() != PACKET_LEN || &packet[..4] != b"RPV1" {
            return false;
        }
        let mut fields = [0u32; 5];
        for (slot, field) in fields.iter_mut().enumerate() {
            *field = u32::from_le_bytes(
                packet[4 + slot * 4..8 + slot * 4]
                    .try_into()
                    .expect("checked public width"),
            );
        }
        fields == [target as u32, uid, child as u32, 7, 7]
            && status & 0x7f == SIGSEGV
            && status & 0x80 == 0
    }

    pub(super) fn rejection_probe() -> io::Result<()> {
        // SAFETY: read only actual scalar identity; caller never supplies target.
        let target = unsafe { getpid() };
        let uid = unsafe { getuid() };
        let mut descriptors = [-1; 2];
        // SAFETY: two initialized writable integer slots for a new owned pipe.
        if unsafe { pipe(descriptors.as_mut_ptr()) } != 0 {
            return Err(unavailable());
        }
        let mut probe = OwnedProbe {
            child: -1,
            read_fd: descriptors[0],
            write_fd: descriptors[1],
            reaped: false,
        };
        // SAFETY: operations affect only the two freshly owned pipe descriptors.
        unsafe {
            let flags = fcntl(probe.read_fd, F_GETFL);
            if flags < 0
                || fcntl(probe.read_fd, F_SETFL, flags | O_NONBLOCK) != 0
                || fcntl(probe.read_fd, F_SETFD, FD_CLOEXEC) != 0
                || fcntl(probe.write_fd, F_SETFD, FD_CLOEXEC) != 0
                || fcntl(probe.read_fd, F_GETFD) & FD_CLOEXEC == 0
                || fcntl(probe.write_fd, F_GETFD) & FD_CLOEXEC == 0
            {
                return Err(unavailable());
            }
        }
        if !single_thread() {
            return Err(unavailable());
        }
        // SAFETY: exact native single-thread observation; child runs only bounded
        // stack/kernel operations and _exit, never inherited private Rust state.
        let child = unsafe { fork() };
        if child < 0 {
            return Err(unavailable());
        }
        if child == 0 {
            child_probe(target, uid, probe.write_fd);
        }
        probe.child = child;
        // SAFETY: parent closes only its fresh write pipe; child owns its copy.
        if unsafe { close(probe.write_fd) } != 0 {
            return Err(unavailable());
        }
        probe.write_fd = -1;
        let deadline = Instant::now() + PROBE_TIMEOUT;
        let mut packet = [0u8; PACKET_LEN + 1];
        let mut used = 0usize;
        let mut status = 0;
        loop {
            // SAFETY: initialized live packet suffix; no private input reads.
            let count = unsafe {
                read(
                    probe.read_fd,
                    packet[used..].as_mut_ptr().cast(),
                    packet.len() - used,
                )
            };
            if count > 0 {
                used += count as usize;
            } else if count < 0
                && !matches!(
                    io::Error::last_os_error().kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                )
            {
                return Err(unavailable());
            }
            if used > PACKET_LEN {
                return Err(unavailable());
            }
            // SAFETY: exact child from fork, initialized status, nonblocking wait.
            let observed = unsafe { waitpid(child, &mut status, WNOHANG) };
            if observed == child {
                probe.reaped = true;
                // Child is dead and has closed its only writer: final bytes are
                // already available, so retain even if its exit preceded read.
                let count = unsafe {
                    read(
                        probe.read_fd,
                        packet[used..].as_mut_ptr().cast(),
                        packet.len() - used,
                    )
                };
                if count > 0 {
                    used += count as usize;
                }
                break;
            }
            if observed < 0 || Instant::now() >= deadline {
                return Err(unavailable());
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if !valid_result(&packet[..used], status, target, uid, child)
            || !core_is_zero()
            || !single_thread()
        {
            return Err(unavailable());
        }
        Ok(())
    }

    pub(super) fn protect() -> io::Result<()> {
        // SAFETY: read only current identities; root/set-ID processes are refused.
        if unsafe { getuid() == 0 || getuid() != geteuid() || getgid() != getegid() } {
            return Err(unavailable());
        }
        zero_core()?;
        let mut action = Action::default();
        // SAFETY: query only exact-layout action; no handler/mask is modified.
        if size_of::<Action>() != 16
            || unsafe { sigaction(SIGCHLD, ptr::null(), &mut action) } != 0
            || action.handler != 0
            || action.flags & 0x20 != 0
            || !single_thread()
        {
            return Err(unavailable());
        }
        // SAFETY: PT_DENY_ATTACH acts only on this image; required after exec.
        if unsafe { ptrace(PT_DENY_ATTACH, 0, ptr::null_mut(), 0) } != 0 {
            return Err(unavailable());
        }
        rejection_probe()
    }

    #[cfg(test)]
    pub(super) fn zero_core_for_test() -> io::Result<()> {
        zero_core()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn public_probe_packet_requires_exact_target_controls_and_signal_without_core() {
            let mut packet = [0u8; PACKET_LEN];
            packet[..4].copy_from_slice(b"RPV1");
            for (slot, value) in [17u32, 501, 19, 7, 7].iter().enumerate() {
                packet[4 + slot * 4..8 + slot * 4].copy_from_slice(&value.to_le_bytes());
            }
            assert!(valid_result(&packet, SIGSEGV, 17, 501, 19));
            for status in [0, 90 << 8, SIGKILL, SIGSEGV | 0x80] {
                assert!(!valid_result(&packet, status, 17, 501, 19));
            }
            for slot in 0..5 {
                let mut changed = packet;
                changed[4 + slot * 4] ^= 1;
                assert!(!valid_result(&changed, SIGSEGV, 17, 501, 19));
            }
            assert!(!valid_result(
                &packet[..PACKET_LEN - 1],
                SIGSEGV,
                17,
                501,
                19
            ));
        }
    }
}

/// Disposable unprotected host fixture hook; never built in an encoder image.
#[cfg(all(test, target_os = "macos"))]
pub(super) fn unprotected_probe_for_test() -> io::Result<()> {
    darwin::zero_core_for_test()?;
    darwin::rejection_probe()
}
