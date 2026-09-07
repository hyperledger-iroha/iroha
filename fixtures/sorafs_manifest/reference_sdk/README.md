# Shared reference-validator fixtures

The three `pop_membership_*` payloads exercise the native PoP **structural
validator** used by Kotlin, Java-source consumers, Swift, JNI, FFI and
`sorafs-validate pop`. The current V1 wire includes a nonzero
`presentation_binding_digest`; the negatives omit that field under the actual
current schema or set it to zero. Each payload has an exact
`ValidationOutcomeV1` JSON result in the signed fixture inventory.

The transcript bytes and verifier fingerprints are deliberate structural test
data. A successful result does not verify Halo2, authorize a recipient, bind an
account, or consume a nullifier. Those guarantees require the authenticated
native prover/verifier and ledger tests.

Regenerate with
`cargo run -p sorafs_manifest --features dev-tools --bin generate_por_fixtures -- --write`;
use `--check` to compare the complete managed fixture set without publishing.
The generator owns all six files and the signed test-only inventory. Native
generation asserts that the missing-field payload fails inside the current
schema rather than at schema identity checking.
