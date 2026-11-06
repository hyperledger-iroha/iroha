//! Isolated launch controls for reserved protocol descriptors.

use super::*;
use std::{fs::File, io::Read as _, os::fd::AsFd as _, sync::Barrier};

const SLOTS: [RawFd; 3] = [198, 201, 202];
const CASE: &str = "IROHA_RESERVED_FD_TEST_CASE";

fn own_test(case: &str) -> crate::ProcessCommand {
    let module = module_path!().split_once("::").unwrap().1;
    let mut cmd = crate::ProcessCommand::new(std::env::current_exe().unwrap()).unwrap();
    cmd.args([
        "--exact",
        &format!("{module}::reserved_descriptor_subprocess"),
        "--ignored",
        "--nocapture",
    ])
    .env(CASE, case);
    cmd
}

fn isolated(case: &str) {
    let result = own_test(case).output().unwrap();
    assert!(
        result.status.success(),
        "case={case}: {} {}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

fn fd_path(fd: RawFd) -> std::path::PathBuf {
    format!("/dev/fd/{fd}").into()
}

fn collision_rollback() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("canary");
    std::fs::write(&path, b"occupied-original").unwrap();
    for target in SLOTS {
        let occupied = fd::fcntl_dupfd_cloexec(File::open(&path).unwrap(), target).unwrap();
        assert_eq!(occupied.as_raw_fd(), target);
        assert_eq!(
            ReservedProcessDescriptors::new(&SLOTS)
                .unwrap_err()
                .raw_os_error(),
            Some(fd::Errno::BUSY.raw_os_error())
        );
        assert!(
            fd::fcntl_getfd(&occupied)
                .unwrap()
                .contains(fd::FdFlags::CLOEXEC)
        );
        let mut bytes = Vec::new();
        File::from(occupied).read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"occupied-original");
        let reclaimed = ReservedProcessDescriptors::new(&SLOTS).unwrap();
        assert_eq!(reclaimed.slots.len(), 3);
    }
}

fn read_transferred_originals() {
    let tag = std::env::var("IROHA_RESERVED_FD_TEST_TAG").unwrap();
    for target in SLOTS {
        let source = File::open(fd_path(target)).unwrap();
        // Opening this path proves the exact target exists. It belongs to this
        // isolated child for the entire inspection; the probe never closes it.
        #[allow(
            unsafe_code,
            reason = "inspect the live independently inherited test descriptor"
        )]
        let borrowed = unsafe { BorrowedFd::borrow_raw(target) };
        assert!(
            !fd::fcntl_getfd(borrowed)
                .unwrap()
                .contains(fd::FdFlags::CLOEXEC)
        );
        assert!(
            !fs::fcntl_getfl(borrowed)
                .unwrap()
                .intersects(fs::OFlags::WRONLY | fs::OFlags::RDWR)
        );
        let mut bytes = String::new();
        (&source).read_to_string(&mut bytes).unwrap();
        assert_eq!(bytes, format!("{tag}/{target}"));
    }
}

fn concurrent_children() {
    let owner = Arc::new(ReservedProcessDescriptors::new(&SLOTS).unwrap());
    let gate = Barrier::new(4);
    std::thread::scope(|scope| {
        for index in 0..4 {
            let owner = Arc::clone(&owner);
            let gate = &gate;
            scope.spawn(move || {
                let root = tempfile::tempdir().unwrap();
                let files: Vec<_> = SLOTS
                    .iter()
                    .map(|target| {
                        let path = root.path().join(format!("input-{target}"));
                        std::fs::write(&path, format!("{index}/{target}")).unwrap();
                        File::open(path).unwrap()
                    })
                    .collect();
                let mut cmd = own_test("read");
                cmd.env("IROHA_RESERVED_FD_TEST_TAG", index.to_string());
                let map: Vec<_> = files
                    .iter()
                    .zip(SLOTS)
                    .map(|(f, t)| (f.as_fd(), t))
                    .collect();
                cmd.inherit_reserved_descriptors(&owner, &map).unwrap();
                drop(files);
                gate.wait();
                let child = cmd.output().unwrap();
                assert!(
                    child.status.success(),
                    "{} {}",
                    String::from_utf8_lossy(&child.stdout),
                    String::from_utf8_lossy(&child.stderr)
                );
            });
        }
    });
    for slot in &owner.slots {
        assert!(
            fd::fcntl_getfd(slot)
                .unwrap()
                .contains(fd::FdFlags::CLOEXEC)
        );
        let mut byte = [0; 1];
        assert_eq!(fd::read(slot, &mut byte).unwrap(), 0);
    }
}

fn invalid_maps_and_duplicate_install() {
    let owner = Arc::new(ReservedProcessDescriptors::new(&SLOTS).unwrap());
    let source = File::open("/dev/null").unwrap();
    let source = source.as_fd();
    let mut cmd = own_test("must-not-exec");
    for invalid in [
        vec![(source, 198), (source, 197)],
        vec![(source, 198), (source, 198)],
    ] {
        assert_eq!(
            cmd.inherit_reserved_descriptors(&owner, &invalid)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }
    cmd.inherit_reserved_descriptors(&owner, &[(source, 201)])
        .unwrap();
    // A second child callback cannot overwrite even a previously transferred /dev/null.
    cmd.inherit_reserved_descriptors(&owner, &[(source, 198), (source, 201)])
        .unwrap();
    assert_eq!(
        cmd.spawn().unwrap_err().raw_os_error(),
        Some(fd::Errno::BUSY.raw_os_error())
    );
    for slot in &owner.slots {
        assert!(
            fd::fcntl_getfd(slot)
                .unwrap()
                .contains(fd::FdFlags::CLOEXEC)
        );
    }
}

static TURNOVER_POOL: SharedProcessDescriptorReservations =
    SharedProcessDescriptorReservations::new(&SLOTS);
struct RetirementHooks {
    pause_once: std::sync::atomic::AtomicBool,
    dropping: std::sync::mpsc::Sender<()>,
    waiting: std::sync::mpsc::Sender<std::thread::ThreadId>,
    resume: Mutex<std::sync::mpsc::Receiver<()>>,
}
static HOOKS: std::sync::OnceLock<RetirementHooks> = std::sync::OnceLock::new();
const CONTROL_BOUND: std::time::Duration = std::time::Duration::from_secs(10);
pub(super) fn before_slot_retirement(pool: &SharedProcessDescriptorReservations) {
    if std::ptr::eq(pool, &TURNOVER_POOL)
        && let Some(hooks) = HOOKS.get()
        && hooks
            .pause_once
            .swap(false, std::sync::atomic::Ordering::SeqCst)
    {
        hooks.dropping.send(()).unwrap();
        hooks
            .resume
            .lock()
            .unwrap()
            .recv_timeout(CONTROL_BOUND)
            .unwrap();
    }
}
pub(super) fn at_retirement_wait(pool: &SharedProcessDescriptorReservations) {
    if std::ptr::eq(pool, &TURNOVER_POOL)
        && let Some(hooks) = HOOKS.get()
    {
        hooks.waiting.send(std::thread::current().id()).unwrap();
    }
}
fn synchronized_turnover() {
    use std::sync::mpsc::channel;
    let (dropping, entered) = channel();
    let (waiting, waiters) = channel();
    let (resume, release) = channel();
    assert!(
        HOOKS
            .set(RetirementHooks {
                pause_once: true.into(),
                dropping,
                waiting,
                resume: Mutex::new(release)
            })
            .is_ok()
    );
    let first = TURNOVER_POOL.acquire().unwrap();
    assert!(Arc::ptr_eq(
        &first,
        &TURNOVER_POOL.active().unwrap().unwrap()
    ));
    let previous = Arc::downgrade(&first);
    let mut command = own_test("must-not-exec");
    command
        .inherit_reserved_descriptors(&first, &[(File::open("/dev/null").unwrap().as_fd(), 198)])
        .unwrap();
    drop(first);
    std::thread::scope(|s| {
        let last = s.spawn(move || drop(command));
        entered.recv_timeout(CONTROL_BOUND).unwrap();
        // Reproduce the exact old Weak-only acquisition branch while the actual
        // final command's destructor still owns all three descriptors.
        assert!(previous.upgrade().is_none());
        assert_eq!(
            ReservedProcessDescriptors::new(&SLOTS)
                .unwrap_err()
                .raw_os_error(),
            Some(fd::Errno::BUSY.raw_os_error())
        );
        let next = s.spawn(|| TURNOVER_POOL.acquire().unwrap());
        let observing = s.spawn(|| TURNOVER_POOL.active().unwrap());
        let mut seen = std::collections::HashSet::new();
        while seen.len() < 2 {
            seen.insert(waiters.recv_timeout(CONTROL_BOUND).unwrap());
        }
        assert!(seen.contains(&next.thread().id()) && seen.contains(&observing.thread().id()));
        resume.send(()).unwrap();
        last.join().unwrap();
        let next = next.join().unwrap();
        if let Some(observed) = observing.join().unwrap() {
            assert!(Arc::ptr_eq(&next, &observed));
        }
        assert!(!previous.ptr_eq(&Arc::downgrade(&next)));
    });
    assert!(TURNOVER_POOL.active().unwrap().is_none());
    let canary = fd::fcntl_dupfd_cloexec(File::open("/dev/null").unwrap(), 198).unwrap();
    assert_eq!(canary.as_raw_fd(), 198);
    assert_eq!(
        TURNOVER_POOL.acquire().unwrap_err().raw_os_error(),
        Some(fd::Errno::BUSY.raw_os_error())
    );
    assert!(TURNOVER_POOL.active().unwrap().is_none());
}

fn exec_error_under_pressure() {
    let owner = Arc::new(ReservedProcessDescriptors::new(&[128]).unwrap());
    let source = File::open("/dev/null").unwrap();
    let root = tempfile::tempdir().unwrap();
    let mut cmd = crate::ProcessCommand::new(root.path().join("missing-executable")).unwrap();
    cmd.stdin(crate::ProcessStdio::Inherit)
        .stdout(crate::ProcessStdio::Inherit)
        .stderr(crate::ProcessStdio::Inherit);
    cmd.inherit_reserved_descriptors(&owner, &[(source.as_fd(), 128)])
        .unwrap();
    let mut pressure = Vec::new();
    loop {
        let next = fd::fcntl_dupfd_cloexec(&source, 3).unwrap();
        if next.as_raw_fd() == 127 {
            drop(next);
            break;
        }
        assert!(next.as_raw_fd() < 127);
        pressure.push(next);
    }
    assert_eq!(
        cmd.spawn().unwrap_err().raw_os_error(),
        Some(fd::Errno::NOENT.raw_os_error())
    );
    assert!(
        fd::fcntl_getfd(&owner.slots[0])
            .unwrap()
            .contains(fd::FdFlags::CLOEXEC)
    );
}

#[test]
fn reserved_slots_never_overwrite_and_roll_back_partial_acquisition() {
    isolated("collision");
}
#[test]
fn parallel_children_receive_distinct_originals_while_parent_slots_stay_inert() {
    isolated("parallel");
}
#[test]
fn reserved_slots_preserve_exec_error_under_descriptor_pressure() {
    isolated("exec-error");
}

#[test]
fn mapping_rejects_unreserved_duplicate_and_previously_installed_targets() {
    isolated("invalid-map");
}

#[test]
fn final_command_drop_and_fresh_acquisition_share_a_retirement_barrier() {
    isolated("turnover");
}

#[test]
fn invalid_reserved_sets_are_rejected_before_opening_any_slot() {
    for invalid in [
        vec![],
        vec![2],
        vec![-1],
        vec![4096],
        vec![17, 17],
        (3..68).collect(),
    ] {
        assert_eq!(
            ReservedProcessDescriptors::new(&invalid)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }
}

#[test]
#[ignore = "isolated subprocess body for fixed descriptor controls"]
fn reserved_descriptor_subprocess() {
    match std::env::var(CASE).unwrap().as_str() {
        "collision" => collision_rollback(),
        "parallel" => concurrent_children(),
        "invalid-map" => invalid_maps_and_duplicate_install(),
        "turnover" => synchronized_turnover(),
        "read" => read_transferred_originals(),
        "exec-error" => exec_error_under_pressure(),
        other => panic!("unknown case {other}"),
    }
}
