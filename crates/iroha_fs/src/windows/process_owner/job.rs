//! Original Windows Job ownership; this conveys no executable or release qualification.

use std::{
    io,
    os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, OwnedHandle},
    ptr, thread,
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::HANDLE,
    System::JobObjects::{
        CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
        QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
    },
};

#[derive(Debug)]
pub(super) struct Job {
    handle: OwnedHandle,
}

impl Job {
    pub(super) fn new() -> io::Result<Self> {
        // SAFETY: no name or inheritable security attributes; success returns one new original.
        let handle = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if handle.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: this successful creation transfers the only owned handle here.
        let job = Self {
            handle: unsafe { OwnedHandle::from_raw_handle(handle) },
        };
        // SAFETY: the documented native limits structure accepts zero reserved fields.
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = unsafe { std::mem::zeroed() };
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        // SAFETY: the original Job and exact initialized native structure remain alive.
        if unsafe {
            SetInformationJobObject(
                job.raw(),
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                u32::try_from(size_of_val(&limits)).unwrap(),
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        Ok(job)
    }

    pub(super) fn raw(&self) -> HANDLE {
        self.handle.as_raw_handle()
    }
    pub(super) fn handle(&self) -> BorrowedHandle<'_> {
        self.handle.as_handle()
    }

    pub(super) fn active_processes(&self) -> io::Result<u32> {
        // SAFETY: native query fills this fixed-size output on successful return.
        let mut accounting: JOBOBJECT_BASIC_ACCOUNTING_INFORMATION = unsafe { std::mem::zeroed() };
        let mut returned = 0;
        let length = u32::try_from(size_of_val(&accounting)).unwrap();
        // SAFETY: output and returned-size storage cover exactly their documented extents.
        if unsafe {
            QueryInformationJobObject(
                self.raw(),
                JobObjectBasicAccountingInformation,
                (&raw mut accounting).cast(),
                length,
                &raw mut returned,
            )
        } == 0
        {
            return Err(io::Error::last_os_error());
        }
        if returned != length {
            return Err(io::Error::other("unexpected native Job accounting extent"));
        }
        Ok(accounting.ActiveProcesses)
    }

    pub(super) fn terminate(&self, code: u32) -> io::Result<()> {
        // SAFETY: this command acts only on the retained original Job.
        if unsafe { TerminateJobObject(self.raw(), code) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    pub(super) fn wait_empty(&self) -> io::Result<()> {
        while self.active_processes()? != 0 {
            thread::sleep(Duration::from_millis(20));
        }
        Ok(())
    }
}
