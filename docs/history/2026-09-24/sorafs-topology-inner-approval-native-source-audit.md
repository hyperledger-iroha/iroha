# SoraFS topology inner-approval source audit (2026-09-24)

Scope: `check_sorafs_production_promotion_bundle.py` on the existing
`optimizations` checkout. This is a source and negative-test record, not a
production approval or a signed release artifact.

The current promotion checker independently pins the inner trust document and
replays the topology summary plus its Ed25519 detached envelope against that
trust. It compares the authenticated projection with the positive aggregate and
rejects appended authority fields. The envelope is the configuration-only
`sorafs.l1.deployment_qualification.signed_envelope.v1` layout. Its exact fields
include review time, qualification and manifest digests, deployment and ordered
validator binding, signer coordinates, and a detached signature. They do not
include role-16 custody state, the original operation, a completed native
result, or a finalized challenged Check. The topology summary itself explicitly
has `promotion_eligible: false` and recognizes no live evidence.

The separate role-16 receipt in `sorafs_manifest::signer::topology` is currently
a consistency contract. Its checker accepts supplied current/completed rows,
while the generic software signer rejects role 16 and no purpose-owned native
state, execution, and Check producer exists. Neither the detached envelope nor
a copied outer/other-role receipt can establish current signer authorization or
completed execution. The promotion check therefore retains both the topology
native-authority blocker and the unconditional inner-chain release blocker.
There is no HSM prerequisite; authenticated software custody still requires
the missing authoritative source.

Adversarial tests now inject purported completion, role-16 receipt, native
Check, and current authorization digests into the old envelope, and inject
purported role-16 receipt, Check, and authorization digests into the independently
pinned trust document. Even after the replay input or trust digest is updated,
the exact schemas reject the claims and final promotion stays blocked. These
tests do not constitute a positive native-operation test.

The one-V1 production cut must connect topology-owned custody and operation
state, exact successful input/result/output and revision-4 finality, a fresh
challenge-bearing Check, and the configured software signer before enabling
role 16 or promotion. It must replace the detached envelope in its producers,
aggregate checker, promotion checker, fixtures, and SDK/CLI consumers together;
the old envelope must not remain as a fallback decoder or second acceptance
path. Resilience, lane-inventory, and foundational approvals still require their
own current authorization and completed-operation verification.

Validation: nine focused promotion-checker tests passed, including the new
adversarial cases. The complete promotion-checker file passed (206 tests). This
does not close native authority, independent audit, or deployment gates.
