//! Native direct-output controls compiled into both shared command targets.

use std::{
    fs,
    io::{BufWriter, Write},
    path::Path,
    process::Command,
};

use super::open_output_file;

fn grant_read(path: &Path) {
    let result = Command::new("icacls.exe")
        .arg(path)
        .args(["/grant", "*S-1-1-0:(R)"])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "native fixture public-read grant failed"
    );
}

#[test]
fn native_command_output_preserves_direct_and_buffered_overwrite_semantics() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("new-parent/output");
    let mut first = open_output_file(&path, "test output").unwrap();
    first.write_all(b"first longer content").unwrap();
    first.flush().unwrap();
    drop(first);
    assert_eq!(
        iroha_fs::read_private(&path, 64).unwrap().as_slice(),
        b"first longer content"
    );
    grant_read(&path);
    let original = iroha_fs::RetainedFile::open_regular(&path)
        .unwrap()
        .identity()
        .unwrap();
    let mut buffered = BufWriter::new(open_output_file(&path, "test output").unwrap());
    buffered.write_all(b"short\0").unwrap();
    buffered.flush().unwrap();
    drop(buffered);
    assert_eq!(fs::read(&path).unwrap(), b"short\0");
    assert_eq!(
        iroha_fs::RetainedFile::open_regular(&path)
            .unwrap()
            .identity()
            .unwrap(),
        original
    );
    assert!(iroha_fs::RetainedFile::open_private(&path).is_err());
}

#[test]
fn native_command_output_refuses_linked_original_before_any_truncation() {
    let temporary = tempfile::tempdir().unwrap();
    let path = temporary.path().join("output");
    let mut first = open_output_file(&path, "test output").unwrap();
    first.write_all(b"keep both names").unwrap();
    drop(first);
    let alias = temporary.path().join("alias");
    fs::hard_link(&path, &alias).unwrap();
    assert!(open_output_file(&path, "test output").is_err());
    assert_eq!(fs::read(&path).unwrap(), b"keep both names");
    assert_eq!(fs::read(alias).unwrap(), b"keep both names");
}

#[test]
fn native_command_output_refuses_nonlocal_namespaces() {
    for path in [
        r"\\server\share\output",
        r"\\?\UNC\server\share\output",
        r"\\.\C:\output",
    ] {
        assert!(open_output_file(Path::new(path), "test output").is_err());
    }
    let temporary = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(temporary.path()).unwrap();
    let path = root.join("local-verbatim-output");
    let mut file = open_output_file(&path, "test output").unwrap();
    file.write_all(b"local").unwrap();
    drop(file);
    assert_eq!(
        iroha_fs::read_private(path, 16).unwrap().as_slice(),
        b"local"
    );
}
