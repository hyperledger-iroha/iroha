// Descriptor-bound platform policy and ACL regression cases.

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn fsetxattr(
        fd: c_int,
        name: *const c_char,
        value: *const c_void,
        size: usize,
        flags: c_int,
    ) -> c_int;
    fn fremovexattr(fd: c_int, name: *const c_char) -> c_int;
}
#[cfg(target_os = "linux")]
fn install_linux_default_acl(handle: &fs::File) -> CString {
    fn push_acl_entry(bytes: &mut Vec<u8>, tag: u16, permissions: u16, id: u32) {
        bytes.extend_from_slice(&tag.to_le_bytes());
        bytes.extend_from_slice(&permissions.to_le_bytes());
        bytes.extend_from_slice(&id.to_le_bytes());
    }
    let name = CString::new("system.posix_acl_default").expect("ACL xattr name");
    let mut acl = 2_u32.to_le_bytes().to_vec();
    let undefined_id = u32::MAX;
    push_acl_entry(&mut acl, 0x01, 0o7, undefined_id);
    push_acl_entry(&mut acl, 0x02, 0o7, 65_534);
    push_acl_entry(&mut acl, 0x04, 0o0, undefined_id);
    push_acl_entry(&mut acl, 0x10, 0o7, undefined_id);
    push_acl_entry(&mut acl, 0x20, 0o0, undefined_id);
    // SAFETY: the descriptor and NUL-terminated name are valid and the
    // ACL buffer follows Linux's fixed little-endian POSIX ACL xattr ABI.
    let installed = unsafe {
        fsetxattr(
            handle.as_raw_fd(),
            name.as_ptr(),
            acl.as_ptr().cast(),
            acl.len(),
            0,
        )
    };
    assert_eq!(
        installed,
        0,
        "install descriptor-bound POSIX default ACL: {}",
        io::Error::last_os_error()
    );
    name
}
#[cfg(target_os = "linux")]
fn remove_linux_default_acl(handle: &fs::File, name: &CString) {
    // SAFETY: the retained descriptor and NUL-terminated xattr name remain
    // valid for this cleanup call.
    assert_eq!(
        unsafe { fremovexattr(handle.as_raw_fd(), name.as_ptr()) },
        0
    );
}
#[test]
fn windows_dacl_qualification_source_contract_is_handle_bound() {
    let source = [
        include_str!("../governance_rooted_fs.rs"),
        include_str!("two_slot_store.rs"),
    ]
    .concat();
    assert!(source.contains("#[link_name = \"GetSecurityInfo\"]"));
    assert!(source.contains("#[link_name = \"GetSecurityDescriptorControl\"]"));
    assert!(source.contains("#[link_name = \"LocalFree\"]"));
    assert!(source.contains("handle.as_raw_handle(),"));
    let pathname_api = ["GetNamed", "SecurityInfo"].concat();
    assert!(!source.contains(&pathname_api));
}
#[test]
fn windows_atomic_replacement_source_contract_is_non_destructive() {
    let source = [
        include_str!("../governance_rooted_fs.rs"),
        include_str!("two_slot_store.rs"),
    ]
    .concat();
    assert!(source.contains("(*info).replace_or_flags = 0;"));
    assert!(source.contains("without replacement: {error}"));
    assert!(source.contains("Windows governance existing-target replacement is disabled"));
    assert!(source.contains("metadata.number_of_links() != Some(1)"));
    let destructive_match = ["matches!(&expected, ExpectedFile::", "Identity(_))"].concat();
    assert!(!source.contains(&destructive_match));
}
#[test]
fn linux_acl_stability_contract_rejects_equal_length_churn() {
    let mut snapshots = std::collections::VecDeque::from([
        b"user.a\0".to_vec(),
        b"user.b\0".to_vec(),
        b"user.a\0".to_vec(),
        b"user.b\0".to_vec(),
        b"user.a\0".to_vec(),
        b"user.b\0".to_vec(),
    ]);
    let error = super::super::stable_linux_acl_attribute_names(
        std::path::Path::new("synthetic-linux-directory"),
        || {
            Ok(Some(
                snapshots
                    .pop_front()
                    .expect("bounded stability reader call"),
            ))
        },
    )
    .expect_err("equal-length ACL-name substitution must fail closed");
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    assert!(
        snapshots.is_empty(),
        "both snapshots in every retry are read"
    );
}
#[cfg(windows)]
#[test]
fn rooted_directory_pins_initial_windows_owner_sid() {
    let temp = tempdir().expect("tempdir");
    let mut root = test_root(temp.path());
    root.owner_sid[0] ^= 1;
    assert_eq!(
        root.verify()
            .expect_err("substituted pinned owner SID must fail closed")
            .kind(),
        io::ErrorKind::PermissionDenied
    );
}
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn retained_directory_acl_policy_accepts_plain_directory() {
    let temp = tempdir().expect("tempdir");
    let handle = fs::File::open(temp.path()).expect("open plain directory");
    super::super::validate_retained_directory_acl(&handle, temp.path())
        .expect("plain descriptor has no ACL mutation grant");
}
#[cfg(target_os = "macos")]
fn change_macos_acl(path: &std::path::Path, operation: &str, acl: Option<&str>) {
    let mut command = Command::new("chmod");
    command.arg(operation);
    if let Some(acl) = acl {
        command.arg(acl);
    }
    let status = command
        .arg(path)
        .status()
        .expect("execute macOS chmod ACL operation");
    assert!(status.success(), "macOS chmod ACL operation must succeed");
}
#[cfg(target_os = "macos")]
#[test]
fn retained_directory_acl_policy_rejects_mutation_allow_entry() {
    let temp = tempdir().expect("tempdir");
    change_macos_acl(temp.path(), "+a", Some("everyone allow add_file"));
    let handle = fs::File::open(temp.path()).expect("open ACL directory");
    let result = super::super::validate_retained_directory_acl(&handle, temp.path());
    change_macos_acl(temp.path(), "-RN", None);
    let error = result.expect_err("ACL add-file grant must fail closed");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
}
#[cfg(target_os = "macos")]
#[test]
fn retained_directory_acl_policy_accepts_deny_only_entry() {
    let temp = tempdir().expect("tempdir");
    change_macos_acl(temp.path(), "+a", Some("everyone deny delete"));
    let handle = fs::File::open(temp.path()).expect("open deny-ACL directory");
    let result = super::super::validate_retained_directory_acl(&handle, temp.path());
    change_macos_acl(temp.path(), "-RN", None);
    result.expect("deny-only ACL must not grant mutation authority");
}
#[cfg(target_os = "linux")]
#[test]
fn retained_directory_acl_policy_rejects_posix_default_acl() {
    let temp = tempdir().expect("tempdir");
    let handle = fs::File::open(temp.path()).expect("open ACL directory");
    let name = install_linux_default_acl(&handle);
    let result = super::super::validate_retained_directory_acl(&handle, temp.path());
    remove_linux_default_acl(&handle, &name);
    let error = result.expect_err("POSIX ACL attribute must fail closed");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
}
#[cfg(target_os = "linux")]
#[test]
fn rooted_descendant_rejects_post_capture_acl_mutation() {
    let temp = tempdir().expect("tempdir");
    fs::create_dir(temp.path().join("child")).expect("create child");
    let root = test_root(temp.path());
    let child = root
        .open_directory(OsStr::new("child"))
        .expect("retain child");
    let name = install_linux_default_acl(&child.handle);
    let result = child.verify();
    remove_linux_default_acl(&child.handle, &name);
    assert_eq!(
        result
            .expect_err("post-capture descendant ACL must fail closed")
            .kind(),
        io::ErrorKind::PermissionDenied
    );
}
#[cfg(target_os = "macos")]
#[test]
fn rooted_descendant_rejects_post_capture_acl_mutation() {
    let temp = tempdir().expect("tempdir");
    let child_path = temp.path().join("child");
    fs::create_dir(&child_path).expect("create child");
    let root = test_root(temp.path());
    let child = root
        .open_directory(OsStr::new("child"))
        .expect("retain child");
    change_macos_acl(&child_path, "+a", Some("everyone allow add_file"));
    let result = child.verify();
    change_macos_acl(&child_path, "-RN", None);
    assert_eq!(
        result
            .expect_err("post-capture descendant ACL must fail closed")
            .kind(),
        io::ErrorKind::PermissionDenied
    );
}
