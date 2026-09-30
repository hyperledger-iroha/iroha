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

`iroha_deploy::bootstrap` now authenticates one canonical native checkpoint
against an independently installed Ed25519 release authority. Signed metadata
binds the network label, genesis identity, chain label, reset generation,
publication serial, HTTPS roots, checkpoint digest/height/block and a validity
interval no longer than one day. Private exclusive custody retains the release
watermark across localnet resets; expiry does not discard rollback protection.
Same-generation identity changes, clock rollback, regressed releases and
same-height equivocation fail closed. Runtime finality progress remains a
separate retained checkpoint and must never regress to a release checkpoint.

This verifier does not supply an official Taira trust key or published release
artifact. Their authenticated release installation and publication, parent
transport, fresh committee observation and dataspace admission are still required
before the remote command can be exposed. Key rotation requires an authenticated
migration of retained release custody, not a silent fresh store.

Use focused unit and native filesystem suites while developing; use real
four-global-plus-four-private node coverage for attachment and privacy. Every new
safety/liveness rule requires a deterministic simulator regression and named
mutation. Run applicable codec/ABI checks and release checks on one candidate.
Component passes alone do not close any whole-network acceptance goal.

Update source-coupled specifications as behavior lands. Public guides belong in
the optional sibling `iroha-docs` repository. Keep `status.md` about current
health and `roadmap.md` about remaining outcomes; retain routine test receipts in
the change report rather than appending development history here.
