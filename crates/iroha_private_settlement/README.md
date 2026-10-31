# `iroha_private_settlement`

Internal native preparation for confidential atomic private settlement. This
crate owns the genuine wallet/prover/sealer orchestration, authenticated context
acquisition, encrypted prepared artifacts and preparation recovery. It depends
on the base `iroha` SDK and the native privacy engine; the SDK's prepared
settlement transport and durable submission workflow remain in `iroha::client`.

Construct `OneCallPrivateSettlementOperation::new(account, request, storage,
artifacts, wallet, verifier)` with an owned `iroha::client::AccountClient`, a
`OneCallPrivateSettlementRequest`, durable workflow storage,
`FilePrivateSettlementArtifacts`, a `PrivateSettlementWallet` and an independently
pinned `SumeragiFinalityVerifier`. Use
`OneCallPrivateSettlementOperation::with_source(account, request, storage,
artifacts, source, wallet, verifier)` when the caller supplies the authenticated
context transport. Both constructors preserve the same native preparation and
verification requirements.

The caller retains exact private business terms, original wallet reservations,
genesis/schema pins and the encrypted artifact key. The selected context authenticates
private-payment limits at the same World cut. Preparation checks the intent, padding
and signed retention through the latest permitted admission height before wallet
effects, and checks the actual proof, complete
capsule and auditor threshold before publishing an artifact. Its carrier ceiling is
the smaller of the caller's allowance and the authenticated policy limit. Selected context is durable
before wallet work. One caller-key authenticated finality checkpoint retains only
the newest bounded prefix for that pending request and round. Every verified
advance is durable before another read, so cold retries with the original genesis
pins progress beyond the per-call read budget. The same exclusive preparation
lease covers reads, checkpoint publication and wallet work; corrupt or foreign
custody fails without restarting from genesis. Checkpoint bytes count toward the
existing artifact limits. This is pending-payment recovery, with no historical
State restoration or alternate checkpoint decoder. Prepared settlement reuses this
caller-key file owner through `FilePrivateSettlementArtifacts::finality()`. Its
single atomic record has fixed prefix, registration and terminal-outcome slots bound
to the exact prepared workflow and root. One-call handoff reloads the original
artifacts and limits that record to the remaining aggregate artifact allowance;
it does not obtain another full budget. `recover` reopens and
independently verifies the retained context and proofs without opening wallet
policies, reserving inputs, proving, signing or sending transactions. `prepare`
explicitly continues the same original preparation and returns its validated
`PrivateSettlementRequest` without uploading material, requesting committee
certification, signing or submitting settlement carriers. `run` uses that same
preparation path and continues to settlement without renewing its per-call read
allowance. `prepared_request` remains a read-only reopen and returns `None` for
an incomplete original. `max_finality_proof_reads = 0` permits reopening authenticated
originals and a retained verified terminal outcome without another finality HTTP
read; any required new proof refuses before HTTP or wallet work. A prepared request is not a settlement receipt and does
not release wallet inputs. Original artifact digesting, transcript checks and
final bundle assembly run on the canonical native worker while retaining the
same exclusive round lease through cancellation and thread-local cleanup.
Prepared materials feed
`AccountClient::private_settlement` with the SDK's existing
`PrivateSettlementRunOptions` and typed settlement results.

`FilePrivateSettlementWallet::open(path, key, maximum_total_bytes)` provides
software custody for caller-supplied original authorizations. Explicit
`stage_authorized` stores only authenticated encryption of the native witness
codec and restricted openings, reserves active inputs across rounds, and never
generates keys or chooses replacement notes. The same wallet supplies every
original leg. Both `run` and `recover` invoke its required `reconcile_verified`
only after independently verified successful Finalize or Abort execution, on the
original bounded worker and round lease. `VerifiedPrivateSettlementOutcome`
distinguishes settlement from abortion. One atomic publication consumes all
original live witnesses after settlement or releases their pending reservations
after an authenticated Abort. Rejection, observed expiry and local cancellation
never release notes. Keys, pending ciphertexts and terminal markers share the
caller's finite store allowance.

`FilePrivateSettlementWallet::release_verified_abort` accepts only an opaque
`VerifiedPrivateSettlementAbort` issued by native successful execution verification
against the exact original manifest. It releases all matching pending legs in one
atomic publication and retains minimal aborted-intent markers across restart.
Those inputs can then be selected only through a new explicit authorization and
its independently authenticated context. Consumed inputs cannot be released.
Run and recovery observe an Abort before resuming committee phases, including
when it is the final block and Prepare never completed. A public status or receipt
DTO supplies only a locator; native successful execution must authenticate the
complete original manifest before local release. The immutable terminal slot
preserves its kind and original identity across later rejected carriers.

If local reconciliation fails after a verified terminal outcome,
`ReconciliationPending` retains that outcome, original optional carrier progress
and typed local cause. Reopening retries the same local cleanup without creating
another payment or discarding the authenticated result.

Prepared-artifact reads enforce the caller's wire-size cap before decoding. Their
cumulative decode allocation budget follows Norito's canonical policy for the
authenticated frame length, capped at 128 MiB; narrower enclosing budgets still
apply. A local read refusal does not replace the retained original.

The current full-World context transport requires `CanReadAllLedgerData`.
Selected data space grants do not authorize its unrelated row identities and
low-entropy values; scoped authenticated context proofs remain unfinished.
Whole-network one-call qualification remains open. The fresh-process recovery
and real-network harness is
[`atomic_private_settlement_onecall`](../../integration_tests/tests/nexus/atomic_private_settlement_onecall.rs).

The software wallet's supplied byte allowance bounds its complete plaintext index, including
inner encrypted originals and consumed/aborted-intent markers. Physical custody reserves one active
AEAD envelope and one native atomic staging envelope (each at most the allowance plus 28
bytes), plus an empty lock. Under that original lock, each writer validates the full three-file
inventory and removes at most one bounded interrupted native staging file before publication;
unknown files, excess stages, oversized extents, and replaced locks fail closed.

`NativePrivateSettlementHost` is the current typed Rust session owner. Supply an
existing Tokio runtime and explicit finite session, original-request, aggregate
artifact and prepared-result allowances. Call `admit` before copying boundary
inputs or constructing the original operation, then hand it to the admission
token's `open`. `start` accepts one Prepare, Run, Prepared or Recover command;
`wait` observes its exact ticket and `take` transfers its one result. Original
and prepared canonical accounting includes private terms, repeated manifests and
endpoint strings. Run and Recover reserve the protocol-derived terminal-result
allowance before admission. These are retained-value bounds, not RSS limits or
accounting for arbitrary custom provider error allocations.

`admission.open_source(originals, signed_genesis_wire)` constructs the concrete
native operation under that same supervisor. `NativePrivateSettlementOriginals`
owns the original `AccountClient`, typed request, independently selected
`SignedGenesisPinsV1` and World schema pin, plus `NativePrivateSettlementStores`.
The latter selects an absolute owner-private root, caller-retained zeroizing key,
and wallet capacity. The opener authenticates the exact signed genesis and checks
account, sponsor, network, chain, schema binding and all local allowances before
opening files. Certified World evidence still verifies the independently selected
schema before preparation or recovery can use wallet material. No remote response
supplies trust pins.

The root contains `workflows`, `artifacts` and `wallet`. The complete original
request, genesis, native pins and path share the host's original-value allowance;
the artifact and wallet capacities share its custody allowance. Opening creates
no journal, prepared artifact, reservation or transaction. Provision original
authorizations separately with the existing software wallet's `stage_authorized`
operation; `NativePrivateSettlementStores::wallet_directory` names the same wallet.
Cold reopen never stages witnesses. The existing `open(operation)` path remains
available for typed Rust callers with their own source, wallet and durable storage
owners. Both paths use one supervisor, command scope and native worker lifecycle.

Dropping a waiter never cancels an accepted command. `close` immediately marks the
session Closing, refuses further commands, then drains accepted work and destroys
unclaimed results. `shutdown` does the same for the host. The canonical native
worker's unique command scope tracks actual capture, result and thread-local
cleanup separately from the asynchronous command task; even a command panic must
join and drain before closure. Keep the supplied runtime alive until shutdown
completes. A prepared result or session close never releases reserved inputs.
`Verified { outcome, progress }` retains the canonical settled-or-aborted outcome;
pending local reconciliation retains the same verified outcome and original cause.

`admission.open_files(descriptor_bytes)` accepts the closed Norito-JSON layout
`iroha.private-settlement.files.v1` under that same admission before its first copy.
Its only fields are `layout`, `client_file`, `request_file`, `pins_file`, `genesis_file`,
`custody_key_file`, `custody_root` and `maximum_wallet_bytes`. Paths must be absolute;
unknown and duplicate fields are refused. The complete descriptor is limited to
`NATIVE_PRIVATE_SETTLEMENT_DESCRIPTOR_MAX_BYTES`. Native source reads enforce both
the individual format's limit and the remaining original-input allowance before
allocation. The same cumulative allowance includes every file referenced by the
no-environment client TOML parser. Signing-key references retain native private-file
custody and their existing 4096-byte parser ceiling. Custody keys are exactly 32 raw
bytes. Source files belong to the current native owner; they are never logged or
copied into the repository or workflow journal.

The restricted request file uses layout `iroha.private-settlement.request.v1` with
`intent`, `terms`, `schema`, `minimum_context_time_ms`, `endpoints`, `padding`,
`retention_height`, `max_carrier_bytes`, `max_artifact_bytes` and
`max_total_artifact_bytes`. Each closed terms row has `asset`, `payer`, `recipient`,
`amount`, `reimbursement` and byte-array `memo`. Model values retain their existing
Norito-JSON representation; this file contains business data and no witness.
The independently selected pins file uses layout `iroha.private-settlement.pins.v1`
with `chain_id`, `network_id`, `genesis_hash`, `signed_genesis_sha256` (32 bytes),
`genesis_public_key`, complete ordered public-key `roster`, `mode` and `world_schema`.
The opener never derives those pins from the request, genesis or a remote source.

`host.stage_files(session, descriptor_bytes)` is explicit. Its closed layout
`iroha.private-settlement.stage.v1` has `ordinal`, `reservation_file` and
`opening_file`. The first file is the existing canonical opaque reservation frame
from `encode_atomic_private_settlement_reservation_v1`; the second is the canonical
`(PrivateSettlementPoolGovernanceV1, PrivateSettlementAuditPolicyV1)` tuple.
Staging authenticates the exact original intent, leg, controller signatures,
independent business terms and governed openings before the software wallet's one
atomic publication. Its input budget also counts the retained original. Only an
opener-issued original-request/wallet capability can stage; a typed custom owner
cannot obtain it by downcast. Reopen and Recover never stage. Discarding a stage
waiter or closing a session drains the accepted original command and does not
release reserved notes.

The Node N-API adapter in `iroha_js_host` projects these owners as opaque host,
ticket and outcome objects on its existing runtime. It has no imported handle or
verified-receipt decoder. Prepared material and public receipt byte getters expose
bounded canonical data, while native terminal authority remains in the issued
object. Each data export acquires a bounded lease from its original host before
queuing serialization; host shutdown refuses new exports and waits through actual
native result and thread-local cleanup, including canceled export waiters.
C/JNI delivery and installed/genuine-network qualification remain separate
open checks; workspace source and component tests alone do not establish them.
