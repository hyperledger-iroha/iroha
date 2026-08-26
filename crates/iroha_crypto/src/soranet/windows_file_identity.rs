//! Stable Win32 identity checks for direct file and directory handles.

use std::{
    fs::{self, File},
    io,
    mem::MaybeUninit,
    os::windows::{fs::OpenOptionsExt as _, io::AsRawHandle as _},
    path::Path,
};

const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x0000_0010;
const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
const FILE_FLAG_BACKUP_SEMANTICS: u32 = 0x0200_0000;
const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
const FILE_SHARE_READ_WRITE_DELETE: u32 = 0x0000_0001 | 0x0000_0002 | 0x0000_0004;

#[repr(C)]
#[derive(Clone, Copy)]
struct FileTime {
    _low: u32,
    _high: u32,
}

#[repr(C)]
struct ByHandleFileInformation {
    file_attributes: u32,
    _creation_time: FileTime,
    _last_access_time: FileTime,
    _last_write_time: FileTime,
    volume_serial_number: u32,
    _file_size_high: u32,
    _file_size_low: u32,
    _number_of_links: u32,
    file_index_high: u32,
    file_index_low: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIdentity {
    volume_serial_number: u32,
    file_index: u64,
    file_attributes: u32,
}

impl FileIdentity {
    const fn is_direct_file(self) -> bool {
        self.file_attributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT) == 0
    }

    const fn is_direct_directory(self) -> bool {
        self.file_attributes & FILE_ATTRIBUTE_DIRECTORY != 0
            && self.file_attributes & FILE_ATTRIBUTE_REPARSE_POINT == 0
    }

    const fn same_file(self, other: Self) -> bool {
        self.volume_serial_number == other.volume_serial_number
            && self.file_index == other.file_index
    }
}

#[link(name = "kernel32")]
#[allow(unsafe_code)]
unsafe extern "system" {
    #[link_name = "GetFileInformationByHandle"]
    fn get_file_information_by_handle(
        file: *mut std::ffi::c_void,
        information: *mut ByHandleFileInformation,
    ) -> i32;
}

#[allow(unsafe_code)]
fn identity(file: &File) -> io::Result<FileIdentity> {
    let mut information = MaybeUninit::<ByHandleFileInformation>::uninit();
    // SAFETY: `file` owns a valid kernel handle for the duration of the call,
    // and `information` has the exact writable Win32 ABI layout expected by
    // `GetFileInformationByHandle`.
    let succeeded =
        unsafe { get_file_information_by_handle(file.as_raw_handle(), information.as_mut_ptr()) };
    if succeeded == 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: Win32 initializes every field when the call succeeds.
    let information = unsafe { information.assume_init() };
    Ok(FileIdentity {
        volume_serial_number: information.volume_serial_number,
        file_index: u64::from(information.file_index_high) << 32
            | u64::from(information.file_index_low),
        file_attributes: information.file_attributes,
    })
}

pub(super) fn open_direct_file(path: &Path) -> io::Result<File> {
    let mut options = fs::OpenOptions::new();
    options
        .read(true)
        .share_mode(FILE_SHARE_READ_WRITE_DELETE)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    options.open(path)
}

pub(super) fn open_direct_directory(path: &Path) -> io::Result<File> {
    let mut options = fs::OpenOptions::new();
    options
        .access_mode(0)
        .share_mode(FILE_SHARE_READ_WRITE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
    options.open(path)
}

pub(super) fn path_identifies_file(path: &Path, opened: &File) -> io::Result<bool> {
    let named = open_direct_file(path)?;
    let named_identity = identity(&named)?;
    let opened_identity = identity(opened)?;
    Ok(named_identity.is_direct_file()
        && opened_identity.is_direct_file()
        && named_identity.same_file(opened_identity))
}

pub(super) fn path_identifies_directory(path: &Path, opened: &File) -> io::Result<bool> {
    let named = open_direct_directory(path)?;
    let named_identity = identity(&named)?;
    let opened_identity = identity(opened)?;
    Ok(named_identity.is_direct_directory()
        && opened_identity.is_direct_directory()
        && named_identity.same_file(opened_identity))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_handle_identity_distinguishes_equal_files() {
        let directory = tempfile::tempdir().expect("create fixture directory");
        let first_path = directory.path().join("first.bin");
        let second_path = directory.path().join("second.bin");
        fs::write(&first_path, b"same bytes").expect("write first fixture");
        fs::write(&second_path, b"same bytes").expect("write second fixture");

        let first = open_direct_file(&first_path).expect("open first fixture");
        let first_again = open_direct_file(&first_path).expect("reopen first fixture");
        let second = open_direct_file(&second_path).expect("open second fixture");
        assert!(
            identity(&first)
                .unwrap()
                .same_file(identity(&first_again).unwrap())
        );
        assert!(
            !identity(&first)
                .unwrap()
                .same_file(identity(&second).unwrap())
        );

        assert!(path_identifies_file(&first_path, &first).unwrap());
        assert!(!path_identifies_file(&second_path, &first).unwrap());
    }

    #[test]
    fn stable_handle_identity_distinguishes_directories() {
        let directory = tempfile::tempdir().expect("create fixture directory");
        let other_path = directory.path().join("other");
        fs::create_dir(&other_path).expect("create other directory");

        let root = open_direct_directory(directory.path()).expect("open fixture root");
        let root_again = open_direct_directory(directory.path()).expect("reopen fixture root");
        let other = open_direct_directory(&other_path).expect("open other directory");
        assert!(
            identity(&root)
                .unwrap()
                .same_file(identity(&root_again).unwrap())
        );
        assert!(
            !identity(&root)
                .unwrap()
                .same_file(identity(&other).unwrap())
        );
        assert!(path_identifies_directory(directory.path(), &root).unwrap());
        assert!(!path_identifies_directory(&other_path, &root).unwrap());
    }
}
