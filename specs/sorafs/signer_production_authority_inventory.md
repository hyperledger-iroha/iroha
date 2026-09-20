# SoraFS production signer authority inventory

This is the implementation inventory for G02.1, inspected on 2026-09-13 in the
final-promotion candidate. It records existing source boundaries and missing
production code. Configuration declarations and test signatures do not prove
that a device, authority or deployment exists. The outer receipt contract is
specified in [final-promotion receipt V1](final_promotion_receipt_v1.md).

## Missing implementations

The [daemon coordinator](../../crates/irohad/src/signer_operation.rs) depends on
`SignerKeyOperationProviderV1` and `SignerOperationStateSourceV1`. Their only
current implementors are the coordinator and stream-token test fixtures.
No production implementation yet resolves the configured software signer and carries
the coordinator's ordered, fenced operations through the runtime provider.
No production state source authenticates and atomically updates the required
custody, audit predecessor, reservation and completed-operation authority.

Repository Rust/Python/shell/TOML and Cargo-manifest searches found no PKCS#11
operation implementation, cryptoki/YubiHSM client, or cloud KMS SDK dependency.
The `hsm`, `kms` and `pkcs11` schemes in custody/config validation admit opaque
credential-free handles only. A handle does not select or implement a vendor
protocol and cannot attest non-exportability.

Final promotion also has no `iroha_config` runtime configuration, broker slot,
deployment assembly or signing command. Its generic daemon service, private
receipt journal and offline verification command are implemented. The
independently signed evidence schema has no production observer that derives and
signs its observations from real finalized custody and operation records.

## Existing configuration and transport

The closest complete public configuration is
`sorafs.storage.stream_tokens.signer`, owned by
[user admission](../../crates/iroha_config/src/parameters/user/stream_token_signer.rs)
and [actual parameters](../../crates/iroha_config/src/parameters/actual/stream_token_signer.rs).
It binds runtime/key handles, signer service/administrator/key/policy, independent
attester trust and independent observer routing/trust with validity and age bounds.
Its default is absent; enabled issuance requires all leaves. Chain/network and
provider identity come from the node/storage context.

`sorafs.storage.native_transaction_signers` binds proof-outcome, repair, reserve
and orderbook transaction authorities. That configuration does not establish
signer custody or a role-14 completed operation. Its qualified transaction facade
now pins the independently supplied node network and configured account, and accepts
one direct instruction from the canonical 16-type role inventory. Cross-role,
foreign/genesis-network, wrapped/batched and attached inputs fail before provider
probes or signing. Broker and software paths use the same pure role predicate;
the duplicate software-only allowlist is removed. The facade also checks the exact
returned payload and signature. Its revision/policy
qualification and software-capable handles do not prove non-exportability. The
final-promotion account now has distinct role15 native custody and a Current Check
with its own control CAS, observer Check permission and target Operate permission.
Observer must differ from the enrolled target. The daemon now has the immutable
prepared-account owner and native Check/signature continuation described in the
[account contract](final_promotion_account_custody_v1.md#remaining-production-integration).
Its current-candidate Rust tests, configured software adapter and qualified
observer/clock/floor/spending/submission assembly remain unfinished. Role14 cannot
sign its own Reserve transaction because it already requires a finalized reservation.
The source now includes exact observer-Check signing that retains the original Core
challenge and deadline, separately pins fee approval and checks payload/signature
substitution. The account workflow constructs it from its retained bindings. This
owner returns only the existing pending Check and supplies no configured service,
spending reservation, qualified clock, durable floor or native submission adapter.
The role/network hardening and broker transport corrections pass a locked nine-library
build, 661 runtime tests and 970 release/CI contracts. The short private socket
fixtures and bounded shared I/O retain every earlier test and add regressions for
deep directories, buffered responses after peer close, blocked writes, expired
empty I/O and accepted socket modes. The initial 633-pass/22-failure and subsequent
642-pass/14-failure and 660-pass/1-failure runs remain recorded. The successful run keeps 8,147 captured
inputs, eighteen controls and binaries unchanged, with exact CI selection and
38 sentinels. Taira and external-software signer credential loaders import software secrets;
that is allowed, but their metadata does not implement the purpose-specific
fencing, authorization and durable operation contract.

The [broker launcher](../../crates/irohad/src/runtime_provider_broker/launcher.rs)
accepts a public catalog and a deployment-owned backend registry. Credentials
remain in concrete adapters; catalog metadata itself grants no custody authority.
The [stream-token broker clients](../../crates/irohad/src/runtime_provider_broker/stream_token_signer_client.rs)
transport sign/recover and independently challenged observer requests. Server
qualification checks their routes, while Torii authenticates custody evidence.
These are real transport implementations, not vendor key or observer backends.

The [stream-token service transport](../../crates/irohad/src/signer_operation/stream_token/transport.rs)
returns complete receipts and preserves ambiguous operations for recovery, but
its service still depends on the two missing coordinator implementations.
The broker clients cannot be used as four-purpose raw signing providers or
retagged as final-promotion authority.

## Native custody and finality

The [native data model](../../crates/iroha_data_model/src/sorafs/stream_token_custody.rs),
[instruction](../../crates/iroha_data_model/src/isi/sorafs/stream_token_custody.rs),
[Core mutation](../../crates/iroha_core/src/smartcontracts/isi/sorafs_stream_token_custody.rs)
and [Core query](../../crates/iroha_core/src/query/stream_token_custody.rs)
implement provider-scoped StreamToken Configure/Enroll/Revoke transitions.
They enforce exact revision/digest CAS, governed provider permissions, key
generation and irreversible revocation, verified enrollment, and bounded
immutable execution records with indexed historical reads. Execution height,
ordinal, time and transaction authority come from native execution.

That history explicitly excludes per-token operation, audit, reservation and
completion state. Its scope requires `StreamToken { provider_id }`; a deployment
label cannot stand in for a provider ID, and a stream-token record cannot supply
final-promotion custody or an absent operation journal.

The [Torii finality owner](../../crates/iroha_torii/src/sorafs/token/signer_finality.rs)
uses the same Core `State` as Torii. It checks native custody at exact heights,
committed block hashes, durable Kura block hashes and revision-4 finality
artifacts. Its production constructor does not accept a caller-provided history
implementation. Exact block/hash, durable certificate and network checks now
share the Core `query::signer_finality` owner. This demonstrates an existing local finality boundary to reuse;
a valid block hash/QC without the corresponding native operation record cannot
prove a reservation or completion, or current revocations under partition.
Sumeragi V2 uses signed logical block time: ordinary ingress bounds transaction
time against NTS, while consensus checks transactions against block time and
does not enforce validator wall clocks. An old future-dated QC can later enter
an age window on fresh startup; neither header age nor a new observation timestamp
establishes current authority. The implementation now adds a purpose-native `Check`
ordered through ordinary consensus, binding a fresh unpredictable one-use
challenge, exact phase/subject, independent floor and committee continuity.
Its consumer must authenticate the exact signed entrypoint, successful aligned
execution result and executed wire/finality at H, then authenticate ancestry to
the current applied cut J >= H and recheck native state and account/role Operate
permissions together at J. Use a separate transaction account key and bounded monotonic phase
time; no historical receipt refresh or new quorum-tip message protocol is implied.
The Check/native consumer passes 38 focused Core tests within the latest
[285-test runtime checkpoint](v1_closure_ledger.md#native-deployment-authority-and-schema-checkpoint).
Its actual submission path and independent runtime trust inputs remain unqualified.
Bounded adversarial models are design evidence, not protocol qualification. Both Torii and the daemon need
the shared Core boundary described in the
[native authority contract](final_promotion_native_authority_v1.md#current-authority-prerequisite).

The new [native role-14 authority contract](final_promotion_native_authority_v1.md)
uses a stable deployment scope and separate custody/operation histories. Its
DataModel/ISI, scoped permissions, Core execution and bounded same-State readers
are implemented and covered by the scoped native checkpoint. This does not yet supply a production
`SignerOperationStateSourceV1` or a production signer adapter. The ordinary state
source interface is separate from enrollment and audited-control capabilities,
so native emergency revocation need not pretend to supply an old-key audit. The shared Manifest
policy/control schemas are now generic `signer::custody_control` types, while
native StreamToken reads and writes retain explicit role/provider enforcement.
Both native owners use the same pure policy-configuration transition for immutable
scope, monotonic signer/attester generations and retained revocation/lineage.
The role-14 snapshot also rejects independently replayed control/operation prefixes.

The [private receipt journal](../../crates/irohad/src/signer_operation/journal.rs)
already provides bounded immutable staging, no-follow directory lineage and an
exclusive directory lease. It stores exact public receipts before completion.
It is not the independently governed custody/operation-state authority. Assembly
now has a private read-only capability over the same exclusive journal lease;
every pinned receipt also retains that lease. The producer retains staging authority. Both consumers
must pin the same independently reviewed statement bytes and expected coordinates;
commitment hashes alone cannot validate its four staged signatures. A nonblocking
service gate covers each complete sign/recover call, including final receipt checks;
a panic leaves the service unavailable. The public Core native request-digest
helper preserves the sole bounded hashing implementation for reconciliation.
All six reader, four lifecycle and three digest tests pass within the fresh
281-test Core/Torii/daemon/SCCP follow-up. The subsequent interval API removes
scalar-clock acceptance and checks both finite UTC endpoints against one native
snapshot. Its four endpoint regressions pass in the 285-test runtime checkpoint.
Real clock/floor/account-signer/source integration remains unfinished.

Clock health and durable files are separate from qualified time and rollback
authority. Core's peer-offset NTS health is not an independently bounded UTC source;
its confidence value measures dispersion, not a certified UTC error. A local
fsynced floor is not proof against restored old state. The production factory must
pin the independent clock and monotonic floor authority, reject fallback/rollback,
preserve Check's original earliest observation bound, and resample both eligibility endpoints after
floor persistence before using custody or a reservation. The separate transaction
signer must authorize the exact network/deployment/action and unsigned payload
before provider I/O; a role label or account-only check is insufficient.

## Smallest real implementation sequence

1. **Native authority validation.** Governed role-14 custody and durable
   deployment-scoped operation records with same-State readers are implemented.
   The focused native/permission/finality/corruption checkpoint passes; complete
   full workspace and actual four-validator qualification. Manifest owns
   the shared typed custody/operation commitments; DataModel/ISI and executor
   permission/registry owners expose the canonical native mutation contract;
   Core owns transactional enforcement and same-State queries. Reuse existing
   primitives without relabeling StreamToken records or adding a competing
   authoritative service database.
2. **State-source adapter.** In `irohad::signer_operation`, connect actual native
   transactions and finalized reads to `SignerOperationStateSourceV1`.
   Signing snapshots must bind custody and the audit head from the same
   authoritative snapshot. Reserve/commit compare the full custody identity,
   request, predecessor and fence; failed/expired operations keep replay
   tombstones. Persist exact timely original completions. Keep operation rows
   outside the custody control-state digest and make terminal custody/audit
   transitions atomic. Use the implemented consensus-ordered purpose-native `Check`
   and its shared Core executed-result consumer; keep historical Kura checks separate.
   Pin the immutable reviewed request in the source before constructing the
   coordinator: binding-only observation methods cannot supply that request scope.
3. **One real software signer adapter.** Implement actual runtime operations and identity
   verification behind `SignerKeyOperationProviderV1`, including exact Ed25519
   bytes, four ordered purposes, operation fencing and failure without unsafe retry. Public
   routing, key and independent authority policy come from `iroha_config`;
   credential acquisition remains encapsulated in the deployment adapter.
4. **Assembly and observer.** Construct the provider, state source and existing
   private journal in one production sign/recover command. Produce independently
   trusted state observations from actual finalized native rows. Exercise
   rotation/revocation, outage, ambiguous completion, expiry, restart recovery,
   changed custody and journal/state races against those implementations.

TODO: Finish full/deployed step 1 qualification and implement and qualify steps 2–4. These are
source and validation tasks, not missing credentials. No additional aliases, software profiles or metadata-only readiness
claims are permitted for this first release.

## Deployment prerequisites after implementation

Provision the configured runtime signer and enrollment, independently
administered observer authority, reviewed policies and native
governance grants, authenticated finalized chain access, and runtime credentials.
None of those private/runtime inputs belong in repository files. Their absence
can block deployed qualification after the implementations exist; supplying
them today would not fill the code gaps above.

This work does not complete the foundational, topology, resilience and lane
inventory signer-proof chain, the 17-lane gate, independent cosign provenance,
the full validation matrix or the 24-hour production soak. Those retain their
own acceptance evidence in the [goals](v1_implementation_goals.md) and
[closure ledger](v1_closure_ledger.md).
