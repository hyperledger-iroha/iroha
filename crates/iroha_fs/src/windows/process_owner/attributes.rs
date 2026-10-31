//! Bounded lifetime of creation attributes and their original handle arrays.

use std::{io, mem, ptr};
use windows_sys::Win32::{
    Foundation::{ERROR_INSUFFICIENT_BUFFER, HANDLE},
    System::Threading::{
        DeleteProcThreadAttributeList, InitializeProcThreadAttributeList,
        LPPROC_THREAD_ATTRIBUTE_LIST, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
        PROC_THREAD_ATTRIBUTE_JOB_LIST, UpdateProcThreadAttribute,
    },
};

#[repr(C, align(16))]
#[derive(Clone)]
struct Block {
    _bytes: [u8; 16],
}

pub(super) struct Attributes<'a> {
    storage: Box<[Block]>,
    initialized: bool,
    // Attribute values must live until DeleteProcThreadAttributeList, not just the syscall.
    handles: &'a [HANDLE],
    jobs: &'a [HANDLE],
}

impl<'a> Attributes<'a> {
    pub(super) fn new(handles: &'a [HANDLE], jobs: &'a [HANDLE]) -> io::Result<Self> {
        let mut bytes = 0;
        // SAFETY: this is the documented sizing call; no output list is accessed.
        let sized =
            unsafe { InitializeProcThreadAttributeList(ptr::null_mut(), 2, 0, &raw mut bytes) };
        let error = io::Error::last_os_error();
        if sized != 0
            || error.raw_os_error() != Some(i32::try_from(ERROR_INSUFFICIENT_BUFFER).unwrap())
            || bytes == 0
            || bytes > 1024 * 1024
        {
            return Err(io::Error::other(format!(
                "invalid native attribute extent: {error}"
            )));
        }
        // Match native Win64 allocation alignment without inspecting the opaque list.
        let mut value = Self {
            storage: vec![Block { _bytes: [0; 16] }; bytes.div_ceil(mem::size_of::<Block>())]
                .into_boxed_slice(),
            initialized: false,
            handles,
            jobs,
        };
        // SAFETY: allocated aligned storage covers the exact measured list extent.
        if unsafe { InitializeProcThreadAttributeList(value.raw(), 2, 0, &raw mut bytes) } == 0 {
            return Err(io::Error::last_os_error());
        }
        value.initialized = true;
        for (attribute, entries) in [
            (PROC_THREAD_ATTRIBUTE_HANDLE_LIST, value.handles),
            (PROC_THREAD_ATTRIBUTE_JOB_LIST, value.jobs),
        ] {
            // SAFETY: both borrowed arrays and every original handle outlive this list.
            if unsafe {
                UpdateProcThreadAttribute(
                    value.raw(),
                    0,
                    attribute as usize,
                    entries.as_ptr().cast(),
                    mem::size_of_val(entries),
                    ptr::null_mut(),
                    ptr::null(),
                )
            } == 0
            {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(value)
    }

    pub(super) fn raw(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.storage.as_mut_ptr().cast()
    }
}

impl Drop for Attributes<'_> {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: this initialized list still owns its storage and borrowed values.
            unsafe { DeleteProcThreadAttributeList(self.raw()) };
        }
    }
}
