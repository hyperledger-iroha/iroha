//! Native sparse-file setup for the CLI's metadata-only Windows test fixtures.

use std::{ffi::c_void, fs::File, io, os::windows::io::AsRawHandle};

const FSCTL_SET_SPARSE: u32 = 0x0009_00c4;

// This test-only boundary keeps all native calls on a borrowed original File.
// No raw handle, unchecked pathname, or unsafe API escapes the module.
#[allow(unsafe_code)]
#[link(name = "kernel32")]
unsafe extern "system" {
    fn DeviceIoControl(
        handle: *mut c_void,
        code: u32,
        input: *mut c_void,
        input_size: u32,
        output: *mut c_void,
        output_size: u32,
        returned: *mut u32,
        overlapped: *mut c_void,
    ) -> i32;
    fn GetFileInformationByHandleEx(
        handle: *mut c_void,
        class: i32,
        buffer: *mut c_void,
        size: u32,
    ) -> i32;
}

/// Mark the retained original sparse before extending its logical length.
pub fn mark_sparse(file: &File) -> io::Result<()> {
    let mut returned = 0;
    // SAFETY: this synchronous no-payload control uses the live borrowed file
    // handle. A null input sets the sparse flag; no user buffer is dereferenced.
    #[allow(unsafe_code)]
    let ok = unsafe {
        DeviceIoControl(
            file.as_raw_handle(),
            FSCTL_SET_SPARSE,
            std::ptr::null_mut(),
            0,
            std::ptr::null_mut(),
            0,
            &raw mut returned,
            std::ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        fs::OpenOptions,
        io::{Read as _, Seek as _, SeekFrom},
        mem::size_of,
    };

    use super::*;

    #[repr(C)]
    #[derive(Default)]
    struct StandardInfo {
        allocation: i64,
        end_of_file: i64,
        links: u32,
        delete_pending: u8,
        directory: u8,
    }

    #[repr(C)]
    #[derive(Default)]
    struct AttributeInfo {
        attributes: u32,
        reparse_tag: u32,
    }

    fn allocation_and_sparse(file: &File) -> io::Result<(u64, bool)> {
        let mut standard = StandardInfo::default();
        let mut attribute = AttributeInfo::default();
        // SAFETY: both calls write into initialized, correctly sized native C
        // structures while the original file handle stays borrowed and live.
        #[allow(unsafe_code)]
        let standard_ok = unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle(),
                1,
                (&raw mut standard).cast(),
                u32::try_from(size_of::<StandardInfo>()).expect("native size fits DWORD"),
            )
        };
        if standard_ok == 0 {
            return Err(io::Error::last_os_error());
        }
        #[allow(unsafe_code)]
        let attribute_ok = unsafe {
            GetFileInformationByHandleEx(
                file.as_raw_handle(),
                9,
                (&raw mut attribute).cast(),
                u32::try_from(size_of::<AttributeInfo>()).expect("native size fits DWORD"),
            )
        };
        if attribute_ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok((
            u64::try_from(standard.allocation)
                .map_err(|_| io::Error::other("negative native allocation"))?,
            attribute.attributes & 0x200 != 0,
        ))
    }

    #[test]
    fn original_handle_sparse_sizing_preserves_zero_extent_without_allocation() {
        const LENGTH: u64 = 8 * 1024 * 1024;
        let directory = tempfile::tempdir().expect("owned native fixture directory");
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(directory.path().join("sparse.bin"))
            .expect("new original native file");
        assert!(
            !allocation_and_sparse(&file)
                .expect("initial original info")
                .1
        );
        mark_sparse(&file).expect("mark original sparse before sizing");
        file.set_len(LENGTH).expect("extend sparse original");
        assert_eq!(file.metadata().expect("original metadata").len(), LENGTH);
        let (allocated, sparse) = allocation_and_sparse(&file).expect("sized original info");
        assert!(sparse);
        assert_eq!(
            allocated, 0,
            "metadata-only sizing must allocate no data extent"
        );
        for offset in [0, LENGTH - 16] {
            file.seek(SeekFrom::Start(offset)).expect("seek original");
            let mut bytes = [0xff; 16];
            file.read_exact(&mut bytes)
                .expect("read original zero extent");
            assert_eq!(bytes, [0; 16]);
        }
        drop(file);
    }
}
