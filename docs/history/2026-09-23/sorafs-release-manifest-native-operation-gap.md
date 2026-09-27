# Release-manifest native operation boundary

Inspection on the existing `optimizations` checkout found that the role-13
release-manifest **private operation** is already implemented in
`crates/irohad/src/signer_operation/release_manifest.rs` and its private
`ceremony.rs`. The service pins reviewed manifest bytes, exact role/purpose and
an original audit predecessor. It uses the shared opaque key-operation
coordinator for four ordered signatures, stages a complete private Norito
receipt before the completion CAS, rechecks authoritative completion and
custody before release, and recovers without another key operation. Its
adversarial source tests use injected state and key providers; they do not
authenticate deployed finality or confer runtime authority.

The missing F04 work is the **production native authority and dispatch**.
The new internal role-13 DataModel contract in
`crates/iroha_data_model/src/sorafs/release_manifest_authority.rs` supplies
bounded canonical Norito action and Check claims, exact reviewed-request/intent
and original-operation phase preflight, and an independent role-13 namespace.
It is separate from the existing private four-signature service. Decoding or
preflighting those claims cannot authenticate execution, custody, finality,
challenge freshness or completion time. At this audit point the new module had
no ISI, native State owner, query, daemon state source, or public JSON/schema
surface. The subsequent closed ISI slice below adds one strict JSON/schema
representation of the existing Manifest request and role-13 claims; it does
not add native State authority.

`SignerOperationStateSourceV1` still needs a fresh
same-finalized-snapshot custody/audit predecessor, durable Reserve and Complete
CAS with retained replay tombstones, and phase-specific Current Checks. Current
Core authority provides StreamToken custody without its ordinary operation
journal, and separate role-14 custody/operation and role-15 account-custody
histories with same-State Check consumers. At the initial audit, the closed
shared custody-history purpose adapters and `signer_check` admitted the two
final-promotion roles, not role 13. The bounded custody slice below adds a
distinct role-13 adapter and raw committed-height readback; `signer_check`
still has no role-13 finalized Check consumer. Reusing role-14 records would
mix independent purposes.
The generic external software signer deliberately rejects ReleaseManifest in
`valid_software_signer_handle`; enabling its raw payload service would bypass
the existing purpose-specific four-signature receipt and authoritative
completion protocol. `scripts/release_manifest_signing.py` accepts a raw
external Ed25519 signature and verifies it with the native CLI, but that
signature alone does not prove a completed role-13 signer operation. No HSM is
required; the conditional owner-only software credential key adapter exists,
but it is not assembled with a production role-13 state source or dispatch.

The smallest complete native cut therefore crosses Manifest/DataModel/Core
State/Kura and daemon ownership: introduce role-13 custody/permission and
operation records, ordered execution and exact Check, implement the configured
state source against those finalized rows, then connect the existing private
service and software credential adapter to the canonical sign/recover command.
The release wrapper and receipt consumer must switch atomically to that
purpose-aware operation. The DataModel change does not enable dispatch:
turning on role 13 within the current generic service would create an unsafe
second signing path.

The adjacent script suites
`release_manifest_signing_test.py`, `generate_release_manifest_test.py`, and
`generate_sorafs_cli_release_manifest_test.py` passed 102/102. These exercise
manifest generation, raw external signing and native verification plumbing,
not production role-13 custody, finalized operation authority, restart
reconciliation or promotion. The focused command
`scripts/cargo_fast.sh --stable-local-metadata --incremental -- test -p iroha_data_model release_manifest_authority --lib`
passed 4/4 before the final coherent foreign-purpose request substitution
assertion was added. The root integration owner reran that exact selector after
the assertion was added: 4/4 passed. F04 and release promotion remain open.

## Exact native integration seam (23 September audit)

The lowest authoritative owner is Core's finalized `State`, not the private
daemon journal. The role-14 implementation is a concrete model of the required
join: `crates/iroha_core/src/query/final_promotion_authority.rs` reads a
purpose-owned custody record, operation head, immutable revision, original
admission and per-ID slot at one committed height;
`crates/iroha_core/src/smartcontracts/isi/sorafs_final_promotion_authority.rs`
publishes its control and operation writes atomically; and
`crates/iroha_core/src/query/final_promotion_authority/observation.rs` consumes
an exact signed, successfully applied Check with Kura/QC finality through the
closed `query/signer_check.rs` owner. The role-15
`query/final_promotion_account_custody.rs` path independently governs the
ordinary transaction key; its custody or permission cannot serve role 13.

The source changes that must move together for a *native*, still non-signing
first cut are:

1. Extend the role-13 DTO in
   `crates/iroha_data_model/src/sorafs/release_manifest_authority.rs` with
   purpose-owned custody/operation transition records, actual execution
   coordinates, bounded strict JSON/schema, and an exact instruction wrapper
   in `crates/iroha_data_model/src/isi/sorafs/release_manifest_authority.rs`.
   Register one V1 wire ID through `isi/sorafs.rs`, `isi/mod.rs`, and
   `isi/registry/wire_ids.rs`, with codec, unknown-field, foreign-role and
   direct-box tests. The closed ISI slice below supplies the one canonical
   JSON/schema representation for the existing Manifest
   `SignerReleaseManifestRequestV1` and role-13 DTOs.
2. Add distinct Manage, Operate and Check permissions under
   `crates/iroha_executor_data_model/src/permission.rs`. They must be scoped to
   the deployment and not reuse any role-14/15 grant. Tests must reject
   cross-purpose conversion and an observer equal to the protected signer or
   operator.
3. Add a sealed role-13 custody adapter in
   `crates/iroha_core/src/query/signer_custody_history/purpose.rs`, its own
   query/read and operation-index owner (parallel to
   `query/final_promotion_authority.rs`), and a native instruction executor
   registered in `smartcontracts/isi/mod.rs`. Configure/Enroll/Revoke must
   preserve generation uniqueness and invalidate an active slot atomically.
   Reserve must compare the same committed custody and audit predecessor,
   create a permanent first-use ID index, and issue one bounded fence. Complete
   must compare the original request, reservation, operator, staged signature
   digest and audit successor, derive completion time from actual execution,
   and retain a timely immutable result. Expire/invalidated IDs remain spent.
   Capacity refusal must fail closed before any partial write.
4. Extend the closed `crates/iroha_core/src/query/signer_check.rs` purpose set
   and add a role-13 Check consumer. An independent observer must sign the
   exact single-Check transaction. The consumer must prove that same envelope
   executed successfully in the retained applied State with authenticated
   Kura/QC lineage, then recheck role-13 grants, current custody, original
   operation row, independently retained floor and both ends of a qualified
   time interval in that same cut. A submitted transaction, decoded claim,
   isolated signature or private receipt cannot substitute for this proof.
   This uses the existing Kura finality mechanism; it does not require a new
   Kura row format.

Only after that cut can `crates/irohad/src/signer_operation/release_manifest.rs`
receive a production `SignerOperationStateSourceV1`. The adapter must implement
its `observe_signing_state`, `reserve`, each Before/AfterProvider and
BeforeCommit observation, `commit`, and both AfterCommit/BeforeRelease
observations from the exact role-13 native rows. It must hold the original
reviewed manifest bytes, custody and audit predecessor; qualify an independent
observer account, fee budget, clock and monotonic finalized floor; submit or
reconcile only each original signed Check; and never retry a protected key
operation after an ambiguous result. The existing four-signature service then
remains the sole producer of the role payload, audit, provenance and response
signatures, with a staged private receipt before Complete. The conditional
owner-only software credential provider can supply opaque Ed25519 operations
only through that service. `external_software_signer/protocol.rs` must continue
rejecting role 13's raw-payload handle. The raw external-signature wrapper and
native CLI receipt consumer need an atomic cutover to the completed purpose
receipt; neither currently supplies native completion authority.

The next meaningful authority slice crosses that Core owner; a daemon adapter
against claims alone would necessarily trust a callback or local receipt as
finality. The remaining native cut needs adversarial tests for replay,
concurrent reservation, stale predecessor, forged completion time, revocation,
crash/restart at every durable transition, failed Check execution and
four-validator finality. F04 remains open until the daemon source and
purpose-specific command/readback are connected and qualified.

## Canonical closed ISI and permission slice

The role-13 action now has one registered
`MutateSorafsReleaseManifestAuthority` instruction/wire ID and three distinct
deployment-scoped Manage, Operate and independent Check permissions. Core's
role-13 handler checks bounded canonical instruction/action frames, the exact
release-manifest purpose, registered accounts and grants, including a distinct
Check observer and registered authorized operator. Its dispatch disposition is
`Closed` and every action returns an execution error, even when correctly
authorized. Thus an instruction can be decoded and policy-inspected but cannot
reserve, complete, mutate custody or authorize signing. The validation-fee
classifier has no asset-effect admission for this closed instruction. The raw
external software signer remains closed to role 13.

The instruction and underlying Manifest request now have the same strict
JSON/`IntoSchema` model as their canonical Norito types; no alternate wire
layout was invented. The action uses one purpose-owned tuple
`ReleaseManifestRevocationV1` because the canonical schema does not admit named
enum fields; no first-release compatibility variant was retained. The new
codec/permission/Core tests cover canonical wire identity,
foreign frames and grants, deployment/boundary rejection, observer/operator
separation and closed execution for every action. Their Cargo selectors are
`iroha_data_model release_manifest_authority`,
`iroha_executor_data_model release_manifest_permission`, and
`iroha_core sorafs_release_manifest_authority`. The focused Manifest request
JSON/Norito test passed 1/1; the DataModel role-13 selector passed 6/6 and its
production library check passed; the executor permission selector passed 2/2;
the Core closed-admission selector passed 3/3. These are local source and
simulated-State tests, not finalized role-13 operation qualification. The next
implementation cut is the role-13 purpose-owned operation rows,
authoritative control publication and finalized Check consumer described
above. No HSM access is required.

## Role-13 custody record and raw committed-height slice

The role-13 DataModel now has one bounded canonical
`ReleaseManifestExecutionV1` and `ReleaseManifestCustodyRecordV1` schema with
its own immutable record domain and finite revision limits. Core's sealed
`ManifestPurpose` uses the existing bounded custody-history machinery with a
separate namespace, role and purpose. That machinery derives execution
coordinates from the applying transaction, prepares immutable revision,
height and key-first-use indexes without publishing partial writes, and
rejects rollback or reuse of prior key generations. The new
`read_release_manifest_custody_at_v1` reads one selected committed-height
snapshot, checks the role-13 chain/network/deployment binding and retained
history, and binds the control digest to the selected block hash.

This is a **raw committed State reader**, not a QC-finalized Check or signing
authority. The public role-13 `Execute` handler and instruction disposition
remain Closed for Configure, Enroll, Revoke, Reserve, Complete, Expire and
Check. The internal Core tests stage and commit purpose-owned control rows
only in simulated State, with an exact manager permission check before
preparation; they cover foreign-purpose refusal, unpublished preparation,
committed readback, stale compare-and-swap, revocation history and unavailable
heights. The DataModel test covers the single canonical Norito/strict JSON
record shape and its independent domain. Focused selectors are
`iroha_data_model role13_custody_record` and
`iroha_core sorafs_release_manifest_authority::tests::custody`. The integration
owner ran both against the combined source: the DataModel record selector
passed 1/1 and the Core custody selector passed 3/3 on a fresh test binary.
`rustfmt --edition 2024` on the owned Rust files, `git diff --check` and
`scripts/check_no_legacy_codec.sh` passed. The repository-wide source-file
budget checker reported 239 other existing or parallel findings and none in
the role-13 files; it is not a passing release gate.

Production still needs an authorized State owner that publishes these prepared
custody rows and invalidates active operation slots atomically, immutable
Reserve/Complete/Expire operation and audit journals, exact successful Check
execution authenticated against Kura/QC finality, and the daemon state source
joined to the existing four-signature private service. This slice does not
enable raw role-13 software signing, synthetic finality or promotion.
