//! Filesystem authentication and immutable durable record regressions.
use super::*;
use std::os::unix::fs::{PermissionsExt as _, symlink};

#[test]
fn journal_roundtrip_is_immutable_and_exclusively_locked() -> Result<()> {
    let root = tempfile::tempdir()?;
    let path = root.path().join("journal");
    let journal = Journal::open(&path, true)?;
    journal.put_exact("plan.json", &vec!["original"])?;
    journal.put_exact("plan.json", &vec!["original"])?;
    assert!(
        journal
            .put_exact("plan.json", &vec!["substitution"])
            .is_err()
    );
    assert_eq!(journal.read::<Vec<String>>("plan.json")?, vec!["original"]);
    assert!(Journal::open(&path, false).is_err());
    drop(journal);
    assert!(Journal::open(&path, false)?.exists("plan.json")?);
    assert!(validate_name("../elsewhere").is_err());
    assert!(validate_name("..").is_err());
    Ok(())
}

#[test]
fn journal_rejects_symlink_hardlink_and_nonprivate_records() -> Result<()> {
    let root = tempfile::tempdir()?;
    let path = root.path().join("journal");
    let journal = Journal::open(&path, true)?;
    journal.put_exact("original.json", &"evidence")?;
    symlink(path.join("original.json"), path.join("symlink.json"))?;
    assert!(journal.read::<String>("symlink.json").is_err());
    std::fs::hard_link(path.join("original.json"), path.join("hardlink.json"))?;
    assert!(journal.read::<String>("original.json").is_err());
    assert!(journal.read::<String>("hardlink.json").is_err());
    std::fs::remove_file(path.join("hardlink.json"))?;
    std::fs::set_permissions(
        path.join("original.json"),
        std::fs::Permissions::from_mode(0o644),
    )?;
    assert!(journal.read::<String>("original.json").is_err());
    symlink(&path, root.path().join("linked-journal"))?;
    assert!(Journal::open(&root.path().join("linked-journal"), false).is_err());
    drop(journal);
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    assert!(Journal::open(&path, false).is_err());
    Ok(())
}

#[test]
fn journal_rejects_truncated_and_oversized_evidence() -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let root = tempfile::tempdir()?;
    let path = root.path().join("journal");
    let journal = Journal::open(&path, true)?;
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(path.join("partial.json"))?;
    file.write_all(b"{\"partial\":")?;
    file.sync_all()?;
    assert!(journal.read::<norito::json::Value>("partial.json").is_err());
    assert!(journal.put_exact("partial.json", &"replacement").is_err());
    file.set_len(MAX_RECORD_BYTES + 1)?;
    assert!(journal.exists("partial.json").is_err());
    Ok(())
}
