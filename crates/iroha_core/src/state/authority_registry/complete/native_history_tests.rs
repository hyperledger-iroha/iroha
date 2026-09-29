//! Exact classification of the opaque native execution history owner.

use super::*;

#[test]
fn native_execution_history_requires_its_exact_field_source_and_owner() {
    let actual = find_identity(STATE_FIELDS, "state.native_execution_tip")
        .expect("native execution owner is exhaustively classified");
    assert_eq!(actual.disclosure, Disclosure::CommitmentOnly);
    let Role::History {
        source,
        authentication,
    } = actual.role
    else {
        panic!("native execution identity must retain its independent history owner");
    };
    assert_eq!(check_history_field(actual, source, authentication), Ok(()));

    const VALID: Field = Field::new(
        "state.native_execution_tip",
        Role::History {
            source: NATIVE_EXECUTION_HISTORY_SOURCE,
            authentication: NATIVE_EXECUTION_HISTORY_AUTHENTICATION,
        },
    );
    // A header history descriptor cannot stand in for authenticated execution R.
    for (source, authentication) in [
        (
            BLOCK_HISTORY_SOURCE,
            NATIVE_EXECUTION_HISTORY_AUTHENTICATION,
        ),
        (
            NATIVE_EXECUTION_HISTORY_SOURCE,
            BLOCK_HISTORY_AUTHENTICATION,
        ),
        (
            "decoded snapshot claim",
            NATIVE_EXECUTION_HISTORY_AUTHENTICATION,
        ),
        (
            NATIVE_EXECUTION_HISTORY_SOURCE,
            "unverified stored certificate",
        ),
    ] {
        assert_eq!(
            check_history_field(&VALID, source, authentication),
            Err(CompleteInventoryError::HistoryDescriptorMismatch(VALID.id))
        );
    }
    for (source, authentication) in [
        ("", NATIVE_EXECUTION_HISTORY_AUTHENTICATION),
        (NATIVE_EXECUTION_HISTORY_SOURCE, ""),
    ] {
        assert_eq!(
            check_history_field(&VALID, source, authentication),
            Err(CompleteInventoryError::IncompleteDescriptor(VALID.id))
        );
    }

    const FORGED_FIELD: Field = Field::new("test.native_execution_tip", VALID.role);
    assert_eq!(
        check_history_field(
            &FORGED_FIELD,
            NATIVE_EXECUTION_HISTORY_SOURCE,
            NATIVE_EXECUTION_HISTORY_AUTHENTICATION,
        ),
        Err(CompleteInventoryError::HistoryDescriptorMismatch(
            FORGED_FIELD.id
        ))
    );
    let header = find_identity(STATE_FIELDS, "state.block_hashes").expect("header history");
    assert_eq!(
        check_history_field(
            header,
            NATIVE_EXECUTION_HISTORY_SOURCE,
            NATIVE_EXECUTION_HISTORY_AUTHENTICATION,
        ),
        Err(CompleteInventoryError::HistoryDescriptorMismatch(header.id))
    );
}
