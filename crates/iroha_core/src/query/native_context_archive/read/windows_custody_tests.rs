//! Windows ACL custody checks for completed native archive reads.

use super::*;

#[test]
fn completed_native_read_rechecks_original_private_custody_at_receiver_finish() {
    let original = vec![0x42; 5000];
    let budget = AllocationBudget::new(original.len());
    let (directory, mut read) = fixture(&original, budget.clone());
    let bytes = complete(&mut read);
    let pointer = bytes.as_slice().as_ptr();
    read.recheck_namespace().unwrap();
    assert!(
        std::process::Command::new("icacls.exe")
            .arg(canonical_path(directory.path()))
            .args(["/grant", "*S-1-1-0:R"])
            .status()
            .unwrap()
            .success()
    );
    assert!(read.recheck_namespace().is_err());
    assert_eq!(bytes.as_slice(), original);
    assert_eq!(bytes.as_slice().as_ptr(), pointer);
    assert_eq!(budget.reserved_bytes(), original.len());
    drop(bytes);
    assert_eq!(budget.reserved_bytes(), 0);
}
