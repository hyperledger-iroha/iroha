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
challenge freshness or completion time. The new module has no ISI, native State
owner, query, daemon state source, or public JSON/schema surface. Its underlying
`SignerReleaseManifestRequestV1` has a canonical Norito codec but currently no
JSON/`IntoSchema` derives, so production API exposure must establish one strict
shared representation rather than an alternate layout.

`SignerOperationStateSourceV1` still needs a fresh
same-finalized-snapshot custody/audit predecessor, durable Reserve and Complete
CAS with retained replay tombstones, and phase-specific Current Checks. Current
Core authority provides StreamToken custody without its ordinary operation
journal, and separate role-14 custody/operation and role-15 account-custody
histories with same-State Check consumers. The closed shared
custody-history purpose adapters and `signer_check` admit the two final-promotion
roles, not role 13. Reusing role 14 records would mix independent purposes.
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
   direct-box tests. The existing Manifest
   `SignerReleaseManifestRequestV1` lacks JSON/`IntoSchema`; the same canonical
   representation must be established there before the wrapper is public.
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

The instruction is Norito-only at this stage: the underlying Manifest request
still lacks strict JSON/`IntoSchema`, so no second JSON representation was
invented. The new codec/permission/Core tests cover canonical wire identity,
foreign frames and grants, deployment/boundary rejection, observer/operator
separation and closed execution for every action. Their Cargo selectors are
`iroha_data_model release_manifest_authority`,
`iroha_executor_data_model release_manifest_permission`, and
`iroha_core sorafs_release_manifest_authority`; they are pending while the
shared Cargo slot is occupied by BFV validation. The next implementation cut
is the role-13 purpose-owned control/operation rows and finalized Check
consumer described above. No HSM access is required.
