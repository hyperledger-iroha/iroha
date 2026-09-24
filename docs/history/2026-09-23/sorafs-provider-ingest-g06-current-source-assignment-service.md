# SoraFS G06 current source-assignment read service

2026-09-23, `optimizations`. The daemon now exposes a read-only resolver-facing
projection from its existing [current-assignment reader](../../../crates/irohad/src/sorafs_provider_ingest_finalized_query.rs).
The [qualified observation](../../../crates/irohad/src/sorafs_provider_ingest_finalized_query/current_source_assignment.rs)
returns the exact genesis network, visible committed State/Kura archive head,
finalized timestamp, destination provider-state root, assignment revision,
selected source, sorted source inventory, and a canonical payload-free source
request. The request is reconstructed from the archive-derived inventory only
after it exactly matches the worker-supplied authorization and optional Musubi
transport binding. A second read compares the complete observation, including
the head, for use-boundary rechecks. The worker's private claim factory, grant
issuer, token signer, and completion signer remain outside this interface.

The [archive-revision test](../../../crates/irohad/src/sorafs_provider_ingest_finalized_query/current_source_assignment_tests.rs)
checks exact request/head/source projection, changed revision, rotated source,
cross-network substitution, and an unchanged assignment at a later head. The
missing-head test checks that the resolver-facing read fails unavailable and
zero revision fails rejected. The fresh combined `irohad` library test binary's
`current_assignment` selector passed all three focused cases, 3/3. The
`iroha3d` binary target built separately but contains zero unit tests; that
build is not evidence for these cases. The final candidate
still needs a daemon test with a live committed State/Kura head and restart, plus
four-validator provider-ingest qualification.

This read service is not a grant authority. A lower-height immutable admission
cursor still lacks authenticated ancestry to the current head. Current council
admission, advert and revocation lineage, separately governed origin and token
issuer keys, TLS DER roots, bounded grant issuance, and before/after/EOF joins
remain missing. The configured directory and signed-advert cache cannot supply
finalized revocation authority. G06 and SoraFS promotion remain open. No HSM
prerequisite or first-release compatibility path was introduced.
