// Actual compiler observations for existing owners in sorafs_manifest::token.
// This is a private include fragment; owning suites retain all payload controls.

/// Verify every captured concrete identity in this ownership scope.
#[test]
fn captured_concrete_owner_identities() {
    crate::captured_owner_identity_support::check_both::<self::StreamTokenBodyV1>(
        "sorafs_manifest::token::StreamTokenBodyV1",
        "sorafs_manifest::token::StreamTokenBodyV1",
        "a1e6e22ba539bb503843f81d46541a8f",
        "a1e6e22ba539bb503843f81d46541a8f",
    );
    crate::captured_owner_identity_support::check_both::<self::StreamTokenV1>(
        "sorafs_manifest::token::StreamTokenV1",
        "sorafs_manifest::token::StreamTokenV1",
        "a254db644158964f37cbe88baf9eb442",
        "a254db644158964f37cbe88baf9eb442",
    );
}
