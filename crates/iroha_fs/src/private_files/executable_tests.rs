//! Creator-only executable sealing preserves identity without admitting receipt access.

use super::*;
use std::io::Write as _;

#[test]
fn executable_sealing_preserves_original_bounded_bytes_and_moves_without_reopening() {
    let temporary = tempfile::tempdir().unwrap();
    let source = OwnerDirectory::open_or_create(temporary.path().join("source")).unwrap();
    let destination = OwnerDirectory::open_or_create(temporary.path().join("destination")).unwrap();
    let mut pending = source.create_retained_private("native", 6).unwrap();
    pending.write_all(b"native").unwrap();
    assert!(pending.write_all(b"overflow").is_err());
    let original = pending.seal_executable().unwrap();
    let identity = original.identity().unwrap();
    let mut moved = original.move_no_replace(&destination, "module").unwrap();
    assert_eq!(moved.identity().unwrap(), identity);
    moved.require_source_absent().unwrap();
    moved.rewind().unwrap();
    let mut bytes = Vec::new();
    moved.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"native");
    drop(moved);
    let private = PrivateDirectory::open(destination.path()).unwrap();
    assert!(private.open_retained_read_only("module", 6).is_err());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(
            std::fs::metadata(destination.path().join("module"))
                .unwrap()
                .permissions()
                .mode()
                & 0o7777,
            0o500
        );
    }
}

#[cfg(unix)]
#[test]
fn executable_access_change_is_detected_and_cannot_be_recovered_as_receipt() {
    use std::os::unix::fs::PermissionsExt as _;
    let temporary = tempfile::tempdir().unwrap();
    let source = PrivateDirectory::open_or_create(temporary.path().join("source")).unwrap();
    let mut pending = source.create_retained_private("native", 1).unwrap();
    pending.write_all(b"x").unwrap();
    let sealed = pending.seal_executable().unwrap();
    std::fs::set_permissions(
        source.path().join("native"),
        std::fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    assert!(sealed.revalidate().is_err());
    assert!(source.open_retained_read_only("native", 1).is_err());
}

#[cfg(windows)]
#[test]
#[allow(
    unsafe_code,
    reason = "load and invoke the test's freshly compiled constant-only DLL"
)]
fn executable_publication_loads_real_native_code_while_receipt_seal_refuses_loading() {
    use std::{os::windows::ffi::OsStrExt as _, process::Command};
    use windows_sys::Win32::{
        Foundation::FreeLibrary,
        System::LibraryLoader::{GetProcAddress, LoadLibraryW},
    };

    fn load(path: &Path) -> io::Result<u32> {
        let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: the terminated path names the constant-only DLL compiled below.
        let library = unsafe { LoadLibraryW(path.as_ptr()) };
        if library.is_null() {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: the live module exports this exact zero-argument C function.
        let address = unsafe { GetProcAddress(library, c"custody_loader_probe".as_ptr().cast()) }
            .expect("compiled DLL must export its probe");
        // SAFETY: the source below defines the exported symbol with this exact signature.
        let probe: unsafe extern "C" fn() -> u32 = unsafe { std::mem::transmute(address) };
        // SAFETY: the module remains loaded and the function only returns a constant.
        let result = unsafe { probe() };
        // SAFETY: this test owns the one successful LoadLibrary reference.
        assert_ne!(unsafe { FreeLibrary(library) }, 0);
        Ok(result)
    }

    let temporary = tempfile::tempdir().unwrap();
    let source_code = temporary.path().join("probe.rs");
    std::fs::write(&source_code, b"//! Genuine native loader control.\n#[unsafe(no_mangle)]\npub extern \"C\" fn custody_loader_probe() -> u32 { 42 }\n").unwrap();
    let compiled = temporary.path().join("compiled.dll");
    let output = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args([
            "--edition=2024",
            "--crate-type=cdylib",
            "--crate-name=custody_loader_probe",
            "-C",
            "debuginfo=0",
        ])
        .arg(&source_code)
        .arg("-o")
        .arg(&compiled)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(load(&compiled).unwrap(), 42);
    let bytes = std::fs::read(&compiled).unwrap();
    let staging = OwnerDirectory::open_or_create(temporary.path().join("staging")).unwrap();
    let destination = OwnerDirectory::open_or_create(temporary.path().join("installed")).unwrap();

    let mut pending = staging
        .create_retained_private("receipt", bytes.len())
        .unwrap();
    pending.write_all(&bytes).unwrap();
    let receipt = pending.seal_read_only().unwrap().into_movable().unwrap();
    let receipt = receipt
        .move_no_replace(&destination, "receipt.dll")
        .unwrap();
    drop(receipt);
    assert_eq!(
        load(&destination.path().join("receipt.dll"))
            .unwrap_err()
            .raw_os_error(),
        Some(5)
    );

    let mut pending = staging
        .create_retained_private("executable", bytes.len())
        .unwrap();
    pending.write_all(&bytes).unwrap();
    let executable = pending.seal_executable().unwrap();
    let executable = executable
        .move_no_replace(&destination, "executable.dll")
        .unwrap();
    executable.require_source_absent().unwrap();
    drop(executable);
    assert_eq!(
        load(&destination.path().join("executable.dll")).unwrap(),
        42
    );
}
