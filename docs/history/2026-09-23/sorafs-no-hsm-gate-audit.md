# SoraFS V1 no-HSM release-gate audit

This audit covers the current `optimizations` checkout's active SoraFS
production checkers, release workflows, signer configuration and V1 contracts.
It does not use a sibling checkout or treat historical fixture claims as current
deployment evidence.

No active SoraFS release script or workflow requires an HSM, PKCS#11 device,
non-exportable key, or hardware-origin attestation. The release-manifest
authentication path requires an external authorized signer and an exact signed
response; the role-specific promotion paths additionally require their own
finalized custody and completed-operation proofs. A software-backed signer may
satisfy those rules. Opaque `hsm`, `kms` and `pkcs11` handle schemes remain
optional routes with the same authority contract; their names prove no key
origin. An `hsm` replacement for a typed `external_signer` field is rejected
because it changes the schema, not because software custody is disallowed.

Two hardware-labelled evidence fields have different meanings and are not key
custody gates. Gateway-load `hardware_profile` identifies the measured host
configuration; PDP `hardware_determinism_reviewed` records cross-machine proof
behavior. Neither attests signer key storage. The retired `require_hsm` consensus
configuration is an unknown field.

The older closure-ledger checkpoint that listed `non-exportability` among 23
custody tests was an obsolete key-storage assertion, superseded by the current
provider-independent contract. Current public signer binding, authorization,
revocation and finalized-operation verification cannot establish whether a
private key can be exported. The later sentence that a software service was not
"hardware-qualified" does not define a release gate. Those historical local
tests do not qualify a production signer or authorize promotion.

The actual open gate is native purpose-specific Reserve/Complete/Check authority
with independent finalized-state readback for each inner approval, followed by
genuine signed topology, inventory, resilience and foundational evidence. The
promotion checker remains fail-closed until those proofs and the full deployed
qualification pass. No HSM procurement is a prerequisite.

Focused current-checkout validation: L1 deployment topology 48/48, L1
resilience 26/26, release-automation software/backend-claim selectors 5/5,
and reference-SDK release-manifest provider/response selectors 5/5 passed.
`git diff --check` passed. These local tests verify the existing contract;
they do not produce any of the missing promotion evidence.
