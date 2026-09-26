# Release-manifest role-13 finalized source audit

2026-09-24, `optimizations`. This is a source-backed F04 blocker record after
the [software service assembly](sorafs-release-manifest-software-service-assembly.md).
It does not authorize production signing or promotion. Authenticated software
custody remains acceptable; no HSM is required and no compatibility path is
introduced.

The canonical role-13 `MutateSorafsReleaseManifestAuthority` instruction and
deployment-scoped permissions exist, but Core's
[`Execute` handler](../../../crates/iroha_core/src/smartcontracts/isi/sorafs_release_manifest_authority.rs)
returns `Err(CLOSED)` for every Configure, Enroll, Revoke, Reserve, Complete,
Expire and Check action, including actors with exact permissions. Its
[`every_role13_action_remains_closed_even_with_exact_permission` test](../../../crates/iroha_core/src/smartcontracts/isi/sorafs_release_manifest_authority/tests.rs)
asserts that disposition. Core's
[`read_release_manifest_custody_at_v1`](../../../crates/iroha_core/src/query/release_manifest_authority.rs)
returns a raw same-State custody-history row and block hash. Its contract
explicitly disclaims finality, current signer eligibility, permissions and
completed release operations; it exposes no operation/audit head or executed
Check proof. That read cannot implement even the first
`SignerOperationStateSourceV1::observe_signing_state` response, much less the
exclusive reservation, completion and two fresh release observations.

The daemon's [`SignerOperationStateSourceV1`](../../../crates/irohad/src/signer_operation.rs)
requires a same-snapshot custody/audit read, fresh active observations, durable
exclusive Reserve, phase-specific reserved observations, atomic timely Complete
and phase-specific finalized completed reads. Workspace search found only three
implementations, all under `irohad/src/signer_operation/tests`; there is no
production implementation or startup call site for
`SignerReleaseManifestServiceV1::from_software_supervisor_credential`.
The separate generic external-software-signer
[`valid_software_signer_handle`](../../../crates/irohad/src/external_software_signer/protocol.rs)
rejects role 13, and the
[`ExternalSoftwareSignerNativeBackendsV1` adapter](../../../crates/irohad/src/external_software_signer/adapter.rs)
also refuses it. Neither a local private receipt journal nor an injected test
source may be relabeled as finalized authority. There is no safe startup switch
or configured production dispatch to enable from these APIs.

The next production cut must first implement the role-13 native custody and
operation transitions with a replay-tombstone journal, exact same-State
custody/audit/operation readers, and successful executed Check proof tied to
State/Kura/QC finality. The daemon can then implement all six state-source
methods using independently governed role permissions, approved signed
transactions, exact executed-result readback, qualified time and monotonic
floor, durable reservation/completion and ambiguous-operation reconciliation.
Only after that source is qualified should configuration and startup assemble
the existing purpose-specific software-key service, validate its four ordered
receipts against completed operation state, and admit release-manifest signing.
This audit made no runtime code change and ran no new Cargo test; the preceding
software-service cut passed its 2/2 focused daemon tests, which used injected
state and did not close these gates.
