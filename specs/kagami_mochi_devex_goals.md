# Kagami and Mochi developer experience

Status: implementation in progress. This is the first-release acceptance contract;
the commands and guarantees below are targets until their current-candidate
end-to-end checks pass. No backward-compatible command aliases, old wire layouts,
or parallel orchestration implementations are required.

## User contract

```sh
kagami localnet up
kagami dataspace up acme --network taira
kagami contract deploy hello.ko
```

These commands require no supplied TOML, prompts, source checkout, Cargo, Python,
shell scripts, or container runtime. Configuration and credentials are generated
in private managed storage outside projects. Kagami is the CLI frontend; Mochi
is the desktop frontend to the same services.

The default localnet has four validators and a funded deployment-capable account.
An attached private dataspace has four local validators with owner-only application
data, works behind NAT, and publishes only authenticated commitments and certificates
to its parent. Restricted FullReplica lanes do not satisfy that privacy contract.
Contract deployment accepts `.ko`, `.to`, and existing Musubi packages. Without a
selected environment it starts the default localnet in the same invocation.

## Implementation goals

| ID | Owner | Required outcome | Completion evidence |
| --- | --- | --- | --- |
| DX1 | Native filesystem / SDK / daemon | One private filesystem boundary on Unix and native Windows; handle-bound reads, exclusive custody, durable journals and publication. | Native macOS, Linux and Windows tests for permissions, links/reparse points, path replacement, crashes and concurrent owners. |
| DX2 | `iroha_deploy` / Kagami / Mochi | Shared persistent local runtime, generated identity/configuration, authenticated local IPC, managed workspace contexts, `up/status/logs/down/reset`. | Four real peers start without inputs; repeated up and down/up retain identity, funds and contract state; only reset creates a new ledger. |
| DX3 | Musubi / contract deployment | Shared source/artifact/package deployment API and thin Kagami/Mochi callers with automatic aliases and exact journal recovery. | All three inputs deploy and execute; concurrent and ambiguous attempts preserve original signed transactions and charges. |
| DX4 | Core / Torii / data model | Independent private dataspace State, execution, storage and artifact scope using native Sumeragi and IVM. | Four-node private execution; cross-dataspace authorization tests; no private canary data in parent storage, routes, events, logs or transport. |
| DX5 | Deployment / Taira / finality | Governed self-service committee admission, testnet funding, signed network metadata/checkpoint bootstrap, outbound parent anchoring. | Fresh-wallet registration and attachment behind blocked inbound ports; exact instance, quorum, epoch, funding and replay verification. |
| DX6 | Mochi / release | Complete matching native runtime bundles and desktop actions over shared APIs. | Clean-install flows on macOS/Linux ARM64 and x86-64 and Windows x86-64, with no build tools on PATH. |
| DX7 | CI / runtime | Startup and deployment latency with truthful readiness. | Twenty-run p95: local startup <=30 s, remote attachment <=60 s, small contract deploy on a ready environment <=30 s. |

DX7 uses preinstalled release binaries, 8 CPU cores, 32 GiB RAM and SSD storage;
remote measurements include metadata/proof retrieval, a healthy funded parent
network and RTT <=100 ms. Timeouts preserve resumable state and never count as
readiness. There are no empty blocks or consensus bypasses to meet these targets.
Generated validators begin outbound dialing without a fixed initial wait and use
the normal authenticated connection and retry behavior.
Startup may submit its one signed readiness transaction after all four validators
have applied genesis, while their authenticated peer links finish forming. Ready
requires that exact transaction to be state-resolved Applied on each peer through
explicit local status reads, followed by the complete four-validator mesh, within
the original deadline. A global fanout response does not prove an individual peer's
application. Failed startup retains its specific stage even when cleanup stops all
validators; an already Applied readiness transaction does not make that attempt Ready.

The `xtask mochi-latency` diagnostic collector records twenty fresh-state startup
attempts and separate ready `.ko`, distinct `.to` and local Musubi package cases;
its exact-twenty aggregator retains failed attempts and refuses to compute a
successful p95 from incomplete samples. `xtask mochi-latency-remote` invokes the
exact disposable eight-validator test twenty times, retaining separate attachment
and ready private-input durations plus whole-run exit/cleanup. These collectors are
implemented in source; no twenty-run collector campaign has yet been executed for
this candidate. Current signed-release campaigns on the reference host, production
remote attachment samples and RTT qualification remain outstanding. The native CI
workflow likewise does not claim reference-host latency qualification. See
[the measurement boundary](mochi_bundle.md#diagnostic-latency-samples).

## Ownership and protocol decisions

- `iroha_deploy` owns environment/context state, genesis/config rendering, local
  supervision and dataspace provisioning. Move the useful Kagami and Mochi
  implementations into this owner; it must not depend on either frontend.
- Musubi exposes the canonical typed compiler/deployment adapter. It accepts an
  immutable SDK configuration and alias scope, and calls `iroha_contract_deploy`.
  It does not depend on the environment engine.
- The supervisor owns live process handles and one environment lock. Frontends
  attach using owner-authenticated Unix sockets or Windows named pipes. They do
  not signal processes identified only by stored numeric PIDs.
- Managed generations and exact operation journals survive `down`; `reset` is
  explicit and local-only. Project context selection lives outside the project.
- Native dataspace instances have independent World/State, queues, Kura, body and
  artifact stores, events, archives and safety records. IVM ABI V1, canonical
  Norito, exact quorums and signed RS16 availability remain mandatory.
- All private peer and Torii traffic stays on loopback. Parent traffic contains
  public registration and certified root proofs only. No private relay fallback
  or global replay of private bodies is allowed.
- Artifact instructions, storage, routing, queries and receipts carry explicit
  dataspace scope. There is no hash-only global fallback for private artifacts.
- Fresh parent verification may use an independently release-signed native
  checkpoint. Metadata must bind genesis/network identity and enforce freshness
  and rollback protection; a queried peer cannot select its own trust root.
- Self-service admission grants only the governed dataspace capabilities. Exact
  rent, fees and original validator staking custody are required. Automatic
  budgets use dedicated managed test wallets and verified faucet allowances.
- Private finality and parent anchoring are distinct facts. Receipts report both;
  an anchoring timeout cannot undo already finalized private execution.
- Distributed private hosting, general cross-dataspace contract calls/AMX,
  Minamoto mutation and hardware-backed offline monetary guarantees are outside
  this developer workflow.

## Qualification and documentation

Installed-release fixtures exercise both the local four-validator workflow and a
disposable four-parent/four-private attachment. They check all three contract inputs,
exact repeat receipts, localnet restart recovery, funded parent provisioning, paid SNS
ownership, private execution and independently authenticated parent anchoring.
The combined flows require a fresh run against the current implementation; component
tests do not establish their completion. The attachment fixture also checks
that its private canary, complete artifact bytes and listener token are absent
from captured parent requests and retained parent files. This bounded regression
does not establish universal secrecy, official Taira readiness, native desktop
interaction, signed multi-platform release qualification or the DX7 latency targets.

Signed genesis context now selects a global or private dataspace root. A private root binds
the exact parent network and full 64-bit dataspace identifier; its own genesis and chain label
derive a native Dataspace instance with root index zero. There is no node-local scope toggle
or missing-field fallback. Routing, execution-time instruction authorization, artifact scope,
parent registration and whole-network privacy still require qualification together before
the managed private-dataspace command can be declared ready.

Immutable artifacts use `ContractArtifactId { dataspace_id, code_hash }` throughout
storage, native instructions, queries and access hints. Equal bytecode hashes in
different dataspaces do not share uploads, manifests or read authority. REST artifact
resources include both identifiers and return their exact network binding; there is
no hash-only lookup fallback. Private roots require an owner-held listener API token
on every Torii route, including otherwise public gateway resources. Generated private
node and client contexts retain that credential automatically, while account signatures
and revocable ledger permissions still authorize individual reads. These source
boundaries remain subject to the combined private-network qualification above.

Private genesis now commits `PrivateRootFeePolicy` under `private_root_fees_v1`:
an exact dataspace-restricted currency and decimal transaction rates with positive
base and gas charges. Private execution derives its fee schedule from that committed
policy and burns the payer's exact private balance. Node-local currency/rate settings
cannot replace it; the global XOR economy remains separate. Private sponsors require
a separately implemented dataspace-local vault owner and are currently rejected.
The generated configuration mirrors the signed policy for configuration identity;
runtime startup and paid private execution still require combined qualification.

The parent now retains a canonical `PrivateDataspaceRegistry` and native
`RegisterPrivateDataspace` / `AnchorPrivateDataspace` transitions. Registration
requires a committed global root, an active SNS dataspace lease, its exact owner
and ownership generation, the parent's genesis identity, and the chain-governed
`private_dataspace_admission_v1` quota policy. Missing policy disables new
registrations. An external child cannot claim an existing physical parent
dataspace or lane; later physical catalog/lane policies cannot claim a registered
child either. Exact registration retries retain an advanced cursor. Lease expiry,
suspension or ownership replacement stops further anchoring without resetting the
child's registered genesis or committee.

Compact anchors retain the exact permissioned four-validator child context and
accept only contiguous genuine three-vote CommitQCs. Child genesis bodies,
transactions and artifacts never enter these records. Parent ordinary-write
receipts bind the public record to an exact certified execution result; a client
must verify them against independently authenticated parent finality. The bounded
`/v1/private-dataspaces/{dataspace_id}/records/{height}/proof` route and SDK reader
use the original certified archive write, never current mutable state. These model
and executor boundaries still need their complete runtime, funding,
crash/restart and privacy qualification before the remote command is declared ready.

`iroha_deploy::bootstrap` now authenticates one canonical native checkpoint
against an independently installed Ed25519 release authority. Signed metadata
binds the network label, genesis identity, chain label, reset generation,
publication serial, HTTPS roots, checkpoint digest/height/block and a validity
interval no longer than one day. Private exclusive custody retains the release
watermark across localnet resets; expiry does not discard rollback protection.
Same-generation identity changes, clock rollback, regressed releases and
same-height equivocation fail closed. Runtime finality progress remains a
separate retained checkpoint in `ParentFinalityStore`. Reopening preserves newer
verified progress; reset generations require separate contexts. Verification
updates publish durably before exposing a readiness report. Uncertain publication
requires reopening custody before another update, so retries cannot overwrite
newer disk state with an older in-memory prefix.
An observation that exhausts its bounded catch-up page retains its verified prefix
and continues to report `CatchingUp`; it cannot claim fresh quorum readiness. Other
observation failures retain the preceding checkpoint. The release watermark records
an explicit uninitialized state so a failed first download remains retryable without
treating a missing custody record as a fresh trust decision.
Initial release, finality, provisioning and attachment directories publish their complete
records and lock files atomically. Prepared wallet journals likewise appear only after the
original operation is durable; a crash may leave an unpublished private staging sibling,
but cannot expose a lock-only operation at its final path.
Managed environments likewise publish keys, signed genesis, final-path configurations
and their sole generation manifest together. Only an unpublished stage may be discarded
under exclusive operation ownership. A published incomplete generation fails closed;
retrying cannot silently replace its identity.
The original foreground attachment deadline is published with the exact generation
binding before a supervisor can start parent work. Automatic startup must not select
the shorter background relay budget while that foreground activation is being written.
Expired or rejected parent transactions remain bound to their original journals and
are reported as terminal failures; retry cannot renew their signed lifetime.

The shared deployment report captures the original execution network and root scope
before dispatch. It preserves the canonical receipt and journal, and binds any separate
historical parent observation to the original child and parent networks. Changed local
selection or an unavailable parent observation cannot overturn that original Applied
result. Transaction-status scope on a private root does not imply public-parent inclusion.

The release requires an original committed global root and also binds the native
World schema, account-address profile, BLS member endpoint hints and
an optional exact faucet authority/currency/allowance with finite fee and namespace
rent caps. Hints never grant committee membership. Native public checkpoint reads
use the independently installed HTTPS location, one absolute deadline, a finite
response bound and no wallet credentials or redirects. The installation artifact
has exact named profiles; a missing Taira profile cannot select a response-supplied
key. Automatic parent SDK contexts use only the signed endpoint mapping and reject
child listener credentials. Optional build-registry roots explicitly select parent
discovery; a cold fetch authenticates the current provider policy and native token
signer against a newly observed parent decision. A private child context cannot
select its own registry or forward its listener credentials.

The composed cold-package workflow remains unqualified: it needs genuine governed
provider admission and token custody, normal TLS, native DNS timeout/rebinding and
revocation coverage, and the complete HTTP/TLS, JSON, CAR and cache 64 MiB peak-RSS
gate. Local-package smoke and isolated transport or allocation tests do not
establish that combined result.
Its harness must exercise the native gateway owner with atomic quotas, leases and
durable callbacks through the complete daemon service graph. The SDK's bounded
provider-discovery reader authenticates current token custody against an independently selected signer
binding and certified native State, including configured-but-unenrolled controls.
That evidence supplies enrollment inputs; current signer eligibility and download
admission still require their separate checks.
The first release removes external broker gateway authority; bare gateway DTOs and
test providers cannot supply the native deployment authority.

The wallet owns a closed initial reserve-policy operation: revision one without a
predecessor, exact asset and role bindings, and original signed fee/deadline terms.
Its journal permits once-only dispatch and read-only recovery of that same request.
Caller-selected fields do not prove policy absence or governance permission.
The independent reserve-policy reader binds the selected Global decision and native
schema to the manager's direct governance permission, selected account and asset keys,
and exact policy and activation provenance. Its account-authenticated Torii route
works before service activation and shares the native snapshot and bounded transport
owners. Authenticated singleton absence does not establish namespace emptiness or
initial activation eligibility. Current-candidate native validation, that separate
eligibility prerequisite, managed collateral/credit/capacity provisioning and durable
service activation remain required for unattended cold-package setup.

The native gateway owner is separate from token-signing custody.
Gateway requests, quota inputs and receipts now have one canonical data-model owner;
each admission retains its original policy qualification so changed lease limits do
not reinterpret pending callbacks or exact replays. A gateway-generated serving-attempt
identity separates one HTTP worker's internal retries from a second worker replaying
the same caller nonce. Governed policy commitments bind
the network-derived gateway identity, limits, validity and independent operator and
observer controller keys. Core's bounded transition engine keeps permanent request
and token identities separate from live quota and expiry indexes. Its clocks come
from native execution, and delayed admission cannot renew the original lease.
The governed native instruction derives its execution from the exact directly signed
transaction. Genesis seeds a policy administrator; configured gateway scopes then
receive explicit operator permissions. Atomic protected World publication retains
original policies, admissions and mutation commitments through rotation and draining.
Six purpose-specific native Checks authenticate current qualification, historical admission,
current Serving, the exact pending prefix, acknowledgement and lease release. Historical
admission grants no new serving authority. Final Serving requires the original physical
attempt, acknowledged callback and unexpired original lease, plus current policy and
permissions. An opaque verifier authenticates the original signed executions and fresh Check
against one certified State cut. It revalidates before a short publication fence enclosing
the final synchronous admission handoff; subsequent HTTP transport is outside that fence.

Each native admission atomically retains a source-owned reputation delivery intent or an
explicit exclusion. Counted intents bind the original admission, source-time recorder policy
and complete unsigned append payload. Fresh certified Checks authorize the daemon to sign that
exact payload with its independent recorder credential; local files, queue acceptance and
process counters grant no delivery authority. Only committed delivery, expiry or an explicit
current-policy-manager cancellation closes a counted callback. Serving an accepted request
requires delivered reputation and acknowledgement. The former generic local stream-token
producer is removed; the separate PoR producer does not authorize native token delivery.

The daemon uses independent configured operator, observer and recorder software credentials
and rejects injected gateway providers. Startup performs live qualification; per-operation local identity
pins cannot replace each purpose-specific current proof. One absolute operation deadline,
default 30 seconds and configurable within 1..=60,000 milliseconds, starts before the worker
queue and covers admission, callback reconciliation, acknowledgement and final Serving.
The bounded native expiry action never renews leases. Permanent acknowledgement and terminal
provenance distinguishes exact replay without local rollback authority. Disabling admission
in governed policy still permits authenticated callback and lease draining.

Token-signing receipt custody uses one journal over `iroha_fs` on Unix and Windows.
Exact ancestor and lock identities remain retained through the last receipt reader;
a bounded writer is consumed into an opaque sealed reader before receipt use.
Inventory, pinned bytes and rereads share finite configured resource admission.
This source portability removes the token issuer's Unix-only startup gate; it does
not qualify native Windows custody, the complete service graph or installed bundles.
Native protocol State continues to own signing permission, revocation and replay;
local journal contents cannot authorize recovery after a protocol-state rollback.

Combined candidate validation, original-deadline runtime completion and multi-replica
recovery remain open. Component coverage of Check, provider and Capture owners does not
qualify the full native service graph or cold-package fetch. Signed native releases and
reference-host twenty-run p95 measurements remain separate DX6/DX7 outcomes; the 30-second
admission budget is a configured limit, not measured latency evidence.

This verifier does not supply an official Taira trust key or published release
artifact. Its HTTP adapter binds explicit SDK contexts to the release-approved
HTTPS roots, overlaps up to eight independent committee reads, and retains one
operation deadline plus bounded per-peer tip retries. Every successful read still
requires independent contiguous-chain and exact-committee verification. Their
authenticated release installation/publication, actual fresh committee observation
and dataspace provisioning remain required for the remote command's qualification.
Key rotation requires an authenticated
migration of retained release custody, not a silent fresh store.

Use focused unit and native filesystem suites while developing; use real
four-global-plus-four-private node coverage for attachment and privacy. Every new
safety/liveness rule requires a deterministic simulator regression and named
mutation. Run applicable codec/ABI checks and release checks on one candidate.
Component passes alone do not close any whole-network acceptance goal.

`iroha_deploy::attachment` retains the selected child genesis and exact parent SNS
ownership generation, then advances its durable anchoring cursor only from original
parent ordinary-write proofs and independently verified finality. A prepared child
certificate or successful transaction submission does not advance that cursor.
Exact anchored replays require no new transaction. Restart revalidates retained
proofs, and incomplete custody or uncertain publication fails closed. The separate
wallet journal owns quoted signed transactions and the marker preceding their sole
dispatch. `AttachmentStore::advance_parent` now combines fresh parent observation,
approved endpoint/signer checks, exact quoted wallet preparation/recovery, original
carrier verification and durable receipt publication. It retains a pre-dispatch
checkpoint so later tip progress cannot prevent authenticating the original
carrier. Each replay turn verifies at most sixteen successors, persists its progress,
and joins the original carrier to an independently observed parent decision. Completion
publishes the receipt and clears pending work in one atomic update. Once an Applied
carrier is retained, recovery does not depend on a transaction-status endpoint keeping
that observation indefinitely.

The shared `relay_once` operation recovers pending work before polling a new child
certificate. Its local SDK source is restricted to an exact owner-token loopback
context and compares the registration with retained signed-genesis execution.
Only a contiguous native-verified compact successor can become a parent transaction;
local successor observations and confirmed parent receipts remain separate.

`provisioning::RemoteProvisioning` now coordinates the original signed private genesis,
separate parent owner wallet, fresh parent quorum, canonical faucet and bounded paid SNS
requests. Its exact public funding policy and operation journals survive local reset.
Reopening rejects a different child, parent generation, owner or funding authorization.
Once a namespace envelope exists, retry preserves that exact request; before signing,
an unsigned quote may refresh within the original release's allowance. A fresh parent
observation after namespace completion selects the certified native World cut used to
authenticate the active lease owner and generation. Committed parent execution rechecks
that generation when applying registration; only its verified receipt establishes
anchoring. Provisioning and relay turns share absolute deadlines. The native supervisor,
Kagami command and Mochi profile picker call these shared owners; their combined
real-network qualification remains required before `dataspace up` can meet its
one-command contract. Installed profile upgrades may raise the retained rollback floor
without replacing the private ledger. Lower floors and substituted trust keys or
checkpoint locations fail closed.

Update source-coupled specifications as behavior lands. Public guides belong in
the optional sibling `iroha-docs` repository. Keep `status.md` about current
health and `roadmap.md` about remaining outcomes; retain routine test receipts in
the change report rather than appending development history here.
