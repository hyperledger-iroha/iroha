//! Linux UAPI originals and ring ordering; only the calling supervisor is opened.
#![allow(
    unsafe_code,
    reason = "narrow owned perf syscall, mapping, atomic ring, and waitid boundary"
)]

use super::{
    Failure, Result,
    ledger::{MAX_RECORDS, Record},
};
use std::{
    fs::File,
    io::Read as _,
    os::fd::{AsRawFd as _, FromRawFd as _, IntoRawFd as _},
    ptr::NonNull,
    sync::atomic::{AtomicU64, Ordering, fence},
};

pub(super) const MAX_CPUS: usize = 256;
const RING_PAGES: usize = 8;
const DATA_HEAD: usize = 1024;
const DATA_TAIL: usize = 1032;

// Only references to the atomic fields escape the raw page pointer; the kernel
// may modify the other mapped metadata independently.
#[repr(C)]
struct MetadataPage {
    reserved: [u8; 1024],
    data_head: AtomicU64,
    data_tail: AtomicU64,
    data_offset: u64,
    data_size: u64,
}
const _: () = assert!(std::mem::offset_of!(MetadataPage, data_head) == DATA_HEAD);
const _: () = assert!(std::mem::offset_of!(MetadataPage, data_tail) == DATA_TAIL);
const _: () = assert!(std::mem::offset_of!(MetadataPage, data_offset) == 1040);
const _: () = assert!(std::mem::offset_of!(MetadataPage, data_size) == 1048);

// The used UAPI prefix is PERF_ATTR_SIZE_VER3. New trailing fields remain absent,
// never inferred from the build host's C headers or an ad hoc runtime library.
#[repr(C)]
#[derive(Default)]
struct Attr {
    kind: u32,
    size: u32,
    config: u64,
    period: u64,
    sample_type: u64,
    read_format: u64,
    flags: u64,
    wakeup: u32,
    bp_type: u32,
    config1: u64,
    config2: u64,
    branch_sample_type: u64,
    sample_regs_user: u64,
    sample_stack_user: u32,
    clockid: i32,
}
const _: () = assert!(std::mem::size_of::<Attr>() == 96);
const _: () = assert!(std::mem::offset_of!(Attr, flags) == 40);
const _: () = assert!(std::mem::offset_of!(Attr, clockid) == 92);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Topology {
    pub possible: Vec<u32>,
    pub online: Vec<u32>,
}

pub(super) fn cpu_set(text: &str) -> Result<Vec<u32>> {
    if text.is_empty() || text.len() > 4096 {
        return Err(Failure::new("cpu_inventory"));
    }
    let mut result = Vec::with_capacity(MAX_CPUS);
    for range in text.trim().split(',') {
        let parts: Vec<&str> = range.split('-').collect();
        if !(1..=2).contains(&parts.len())
            || parts
                .iter()
                .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
        {
            return Err(Failure::new("cpu_inventory"));
        }
        let first = parts[0]
            .parse::<u32>()
            .map_err(|_| Failure::new("cpu_inventory"))?;
        let last = parts[parts.len() - 1]
            .parse::<u32>()
            .map_err(|_| Failure::new("cpu_inventory"))?;
        if first > last || usize::try_from(last).map_or(true, |last| last >= MAX_CPUS) {
            return Err(Failure::new("cpu_bound"));
        }
        for cpu in first..=last {
            if result.contains(&cpu) {
                return Err(Failure::new("cpu_inventory"));
            }
            result.push(cpu);
        }
    }
    if result.is_empty() {
        return Err(Failure::new("cpu_inventory"));
    }
    result.sort_unstable();
    Ok(result)
}

fn cpu_file(name: &str) -> Result<Vec<u32>> {
    let mut text = String::new();
    File::open(format!("/sys/devices/system/cpu/{name}"))
        .map_err(Failure::io)?
        .take(4097)
        .read_to_string(&mut text)
        .map_err(Failure::io)?;
    cpu_set(&text)
}

pub(super) fn topology() -> Result<Topology> {
    let possible = cpu_file("possible")?;
    let present = cpu_file("present")?;
    let online = cpu_file("online")?;
    if !online.iter().all(|cpu| present.contains(cpu))
        || !present.iter().all(|cpu| possible.contains(cpu))
    {
        return Err(Failure::new("cpu_inventory"));
    }
    Ok(Topology { possible, online })
}

pub(super) fn caller() -> Result<u32> {
    // SAFETY: gettid has no pointer arguments or side effects on another task.
    let tid = unsafe { libc::syscall(libc::SYS_gettid) };
    u32::try_from(tid).map_err(|_| Failure::new("caller_identity"))
}

pub(super) fn single_main() -> Result<u32> {
    let pid = std::process::id();
    if caller()? != pid {
        return Err(Failure::new("not_main_thread"));
    }
    let mut entries = std::fs::read_dir("/proc/self/task").map_err(Failure::io)?;
    let first = entries
        .next()
        .ok_or(Failure::new("task_inventory"))?
        .map_err(Failure::io)?;
    if first.file_name() != pid.to_string().as_str() || entries.next().is_some() {
        return Err(Failure::new("not_single_threaded"));
    }
    Ok(pid)
}

pub(super) fn monotonic_ns() -> Result<u64> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // SAFETY: initialized writable timespec is live and CLOCK_MONOTONIC is fixed.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &raw mut time) } != 0 {
        return Err(Failure::last_io());
    }
    u64::try_from(time.tv_sec)
        .ok()
        .and_then(|sec| sec.checked_mul(1_000_000_000))
        .and_then(|sec| {
            u64::try_from(time.tv_nsec)
                .ok()
                .and_then(|ns| sec.checked_add(ns))
        })
        .ok_or(Failure::new("clock_extent"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, norito::json::JsonSerialize)]
pub(super) struct Counter {
    pub cpu: u32,
    pub lost: u64,
}

pub(super) struct Ring {
    file: Option<File>,
    mapping: NonNull<MetadataPage>,
    mapping_len: usize,
    unmapped: bool,
    offset: usize,
    size: usize,
    tail: u64,
    pub cpu: u32,
}
// SAFETY: ownership is unique, the mapping is accessed only through the locked
// observer, and kernel sharing uses the documented perf atomic head/tail protocol.
unsafe impl Send for Ring {}

impl Ring {
    pub fn open_current(cpu: u32) -> Result<Self> {
        let owner = caller()?;
        let attr = Attr {
            kind: 1,
            size: 96,
            config: 9,
            sample_type: (1 << 1) | (1 << 2) | (1 << 7),
            read_format: (1 << 0) | (1 << 1) | (1 << 4),
            flags: (1 << 1)
                | (1 << 5)
                | (1 << 6)
                | (1 << 9)
                | (1 << 13)
                | (1 << 18)
                | (1 << 24)
                | (1 << 25),
            clockid: libc::CLOCK_MONOTONIC,
            ..Attr::default()
        };
        // SAFETY: the initialized UAPI prefix is live; only this current thread is
        // selected. FD_CLOEXEC and inherited task scope exclude system-wide events.
        let raw = unsafe {
            libc::syscall(
                libc::SYS_perf_event_open,
                &raw const attr,
                owner,
                cpu,
                -1,
                8,
            )
        };
        let fd = i32::try_from(raw).map_err(|_| Failure::new("event_descriptor"))?;
        if fd < 0 {
            return Err(Failure::last_io());
        }
        // SAFETY: successful syscall returned a new exclusively owned descriptor.
        let file = unsafe { File::from_raw_fd(fd) };
        // SAFETY: sysconf has no pointers and reads the current page-size constant.
        let page = usize::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) })
            .map_err(|_| Failure::new("page_size"))?;
        if page < 4096 || !page.is_power_of_two() {
            return Err(Failure::new("page_size"));
        }
        let size = page
            .checked_mul(RING_PAGES)
            .ok_or(Failure::new("ring_extent"))?;
        let mapping_len = page.checked_add(size).ok_or(Failure::new("ring_extent"))?;
        // SAFETY: shared non-overwrite perf mapping is requested for this original
        // descriptor only; a successful mapping is retained until explicit Drop.
        let raw = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                mapping_len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                file.as_raw_fd(),
                0,
            )
        };
        if raw == libc::MAP_FAILED {
            return Err(Failure::last_io());
        }
        let Some(mapping) = NonNull::new(raw.cast::<MetadataPage>()) else {
            // SAFETY: a successful address-zero mapping still has this exact extent.
            let _ = unsafe { libc::munmap(raw, mapping_len) };
            return Err(Failure::new("ring_mapping"));
        };
        let ring = Self {
            file: Some(file),
            mapping,
            mapping_len,
            unmapped: false,
            offset: page,
            size,
            tail: 0,
            cpu,
        };
        // SAFETY: both fixed UAPI u64 fields fit in the first mapped page and are
        // initialized by perf before mmap returns. They are read, never modified.
        let (offset, actual_size) = unsafe {
            (
                std::ptr::read_volatile(&raw const (*ring.mapping.as_ptr()).data_offset),
                std::ptr::read_volatile(&raw const (*ring.mapping.as_ptr()).data_size),
            )
        };
        if offset != page as u64 || actual_size != size as u64 {
            return Err(Failure::new("ring_geometry"));
        }
        Ok(ring)
    }

    fn copy(&self, position: u64, output: &mut [u8]) -> Result<()> {
        let start = usize::try_from(position & (self.size as u64 - 1))
            .map_err(|_| Failure::new("ring_offset"))?;
        let first = output.len().min(self.size - start);
        // SAFETY: output is disjoint owned stack memory, and each source extent
        // lies inside the original forward ring after acquired head validation.
        unsafe {
            std::ptr::copy_nonoverlapping(
                self.mapping.as_ptr().cast::<u8>().add(self.offset + start),
                output.as_mut_ptr(),
                first,
            );
            std::ptr::copy_nonoverlapping(
                self.mapping.as_ptr().cast::<u8>().add(self.offset),
                output.as_mut_ptr().add(first),
                output.len() - first,
            );
        }
        Ok(())
    }

    pub fn drain(&mut self, records: &mut Vec<Record>) -> Result<()> {
        if self.unmapped {
            return Err(Failure::new("closed_mapping"));
        }
        // SAFETY: page alignment, fixed UAPI offset, and live original mapping
        // provide an aligned shared u64; acquire pairs with kernel publication.
        let head = unsafe { (*self.mapping.as_ptr()).data_head.load(Ordering::Acquire) };
        if head < self.tail || head - self.tail > self.size as u64 {
            return Err(Failure::new("ring_overwrite"));
        }
        let mut position = self.tail;
        while position < head {
            if head - position < 8 {
                return Err(Failure::new("record_extent"));
            }
            let mut raw = [0_u8; 128];
            self.copy(position, &mut raw[..8])?;
            let length = usize::from(u16::from_ne_bytes([raw[6], raw[7]]));
            if !(8..=raw.len()).contains(&length)
                || !length.is_multiple_of(8)
                || length as u64 > head - position
            {
                return Err(Failure::new("record_extent"));
            }
            self.copy(position, &mut raw[..length])?;
            if records.len() >= MAX_RECORDS {
                return Err(Failure::new("record_bound"));
            }
            records.push(Record::parse(&raw[..length], self.cpu)?);
            position += length as u64;
        }
        // The perf ABI requires all record reads before the data_tail store.
        fence(Ordering::SeqCst);
        // SAFETY: the retained aligned UAPI field is the sole userspace tail;
        // the observer mutex serializes every consumer operation on this ring.
        unsafe {
            (*self.mapping.as_ptr())
                .data_tail
                .store(head, Ordering::Relaxed);
        }
        self.tail = head;
        Ok(())
    }

    pub fn counter(&mut self) -> Result<Counter> {
        let mut raw = [0_u8; 32];
        if self
            .file
            .as_mut()
            .ok_or(Failure::new("closed_event"))?
            .read(&mut raw)
            .map_err(Failure::io)?
            != raw.len()
        {
            return Err(Failure::new("counter_extent"));
        }
        let lost = u64::from_ne_bytes(
            raw[24..32]
                .try_into()
                .map_err(|_| Failure::new("counter_extent"))?,
        );
        Ok(Counter {
            cpu: self.cpu,
            lost,
        })
    }

    pub fn disable(&self) -> Result<()> {
        // SAFETY: ioctl only disables this original owned event. It does not
        // signal, attach to, reap or modify any child process.
        if unsafe {
            libc::ioctl(
                self.file
                    .as_ref()
                    .ok_or(Failure::new("closed_event"))?
                    .as_raw_fd(),
                libc::_IO(u32::from(b'$'), 1),
                0,
            )
        } != 0
        {
            return Err(Failure::last_io());
        }
        Ok(())
    }

    pub fn close(&mut self) -> Result<()> {
        if !self.unmapped {
            // SAFETY: this is the original complete mapping and its sole owner.
            if unsafe { libc::munmap(self.mapping.as_ptr().cast(), self.mapping_len) } != 0 {
                return Err(Failure::last_io());
            }
            self.unmapped = true;
        }
        if let Some(file) = self.file.take() {
            let fd = file.into_raw_fd();
            // SAFETY: this is the exclusively owned original descriptor. Linux
            // releases it even on close error; never retry a potentially reused fd.
            if unsafe { libc::close(fd) } != 0 {
                return Err(Failure::last_io());
            }
        }
        Ok(())
    }
}

impl Drop for Ring {
    fn drop(&mut self) {
        // SAFETY: this is the original complete mmap extent and its sole owner.
        // Drop is reached before spawn refusal or after retained terminal handling.
        if !self.unmapped {
            let _ = unsafe { libc::munmap(self.mapping.as_ptr().cast(), self.mapping_len) };
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, norito::json::JsonSerialize)]
pub(super) struct Terminal {
    pub pid: u32,
    pub code: i32,
    pub status: i32,
}

/// Observe only a previously selected original direct child; never reap it.
pub(super) fn terminal(root: u32) -> Result<Option<Terminal>> {
    // SAFETY: zero is the UAPI no-result sentinel and siginfo_t permits zero init.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    // SAFETY: root is admitted by observed direct FORK, and live initialized
    // siginfo is retained for this nonreaping wait of our own original child.
    if unsafe {
        libc::waitid(
            libc::P_PID,
            root,
            &raw mut info,
            libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
        )
    } != 0
    {
        return Err(Failure::last_io());
    }
    // SAFETY: waitid successfully initialized the corresponding child fields.
    let pid = unsafe { info.si_pid() };
    if pid == 0 {
        return Ok(None);
    }
    if u32::try_from(pid).ok() != Some(root)
        || !matches!(
            info.si_code,
            libc::CLD_EXITED | libc::CLD_KILLED | libc::CLD_DUMPED
        )
    {
        return Err(Failure::new("terminal_identity"));
    }
    // SAFETY: the successful CLD_* result selects the initialized status field.
    Ok(Some(Terminal {
        pid: root,
        code: info.si_code,
        status: unsafe { info.si_status() },
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn possible_cpu_parser_is_bounded_exact_and_rejects_overlap() {
        assert_eq!(cpu_set("0-3,7\n").unwrap(), [0, 1, 2, 3, 7]);
        for bad in [
            "",
            "0-1,1",
            "3-2",
            "256",
            "-1",
            "0-99999999999",
            "0,",
            "0--1",
        ] {
            assert!(cpu_set(bad).is_err(), "{bad}");
        }
    }
    #[test]
    fn uapi_prefix_and_ring_fields_match_documented_kernel_offsets() {
        assert_eq!(std::mem::size_of::<Attr>(), 96);
        assert_eq!(std::mem::offset_of!(Attr, read_format), 32);
        assert_eq!(std::mem::offset_of!(Attr, flags), 40);
        assert_eq!(std::mem::offset_of!(Attr, clockid), 92);
        assert_eq!((DATA_HEAD, DATA_TAIL), (1024, 1032));
    }
}
