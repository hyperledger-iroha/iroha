# SoraFS council-signed admission lineage wire

2026-09-24, `optimizations`. This bounded F05 cut changes the sole Norito V1
provider-admission wire layout. A council-signed admission envelope now commits
to a nonzero network, council-policy identity/revision/digest, admission-event
revision, and exact expected current event digest. Initial admissions have
revision one and no predecessor. A renewal has the next revision and its signed
expected digest must equal both the outer renewal predecessor and the active
record's envelope digest. A revocation signs the same policy binding, its next
transition revision, and the exact current event digest. Verification refuses
policy substitution, predecessor substitution, revision gaps, foreign networks,
and loading a successor envelope as a fresh initial record. The retired
pre-lineage envelope and revocation layouts have no decoder fallback.

The council policy fields are signed **claims**, not a governed-policy read.
`ProviderAdmissionCouncilPolicy` still accepts locally supplied keys and quorum;
the directory-backed record remains provisional. Production authorization
requires a Parliament-enacted council policy in finalized State/Kura, an exact
policy id/revision/digest projection, consensus-owned admission-event head and
revocation tombstone, and one authenticated reader binding the signed event to
that finalized policy and head. The reader must reject a fork, stale revision,
revoked head, or missing retained ancestry across restart. The current CLI can
construct an initial admission and a first renewal against an explicit initial
predecessor; it cannot prove a longer chain or revive a successor envelope from
an isolated file. No HTTPS grant, service deployment, or F05 release promotion
is qualified by this wire change. Authenticated software signing suffices;
there is no HSM prerequisite.

The direct canonical fixture generator was run twice into separate directories
and both 19-file outputs were byte-identical before updating the checked-in
`fixtures/sorafs_manifest/provider_admission` set. The separate xtask
provider-alpha generator was also run twice byte-identically. Its ten-file
output was left separate because its default destination collides with the
direct generator's exact 19-file directory contract; joining or retiring those
generator surfaces remains an interface cleanup. Focused manifest admission,
CAR CLI, and direct generator tests pass, including signed predecessor
substitution and retired-layout refusal. The repository's retired-codec guard
passes. This evidence is local wire qualification, not distributed authority or
restart evidence.
