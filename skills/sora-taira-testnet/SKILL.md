---
name: sora-taira-testnet
description: "Work against the SORA Taira testnet through its deployed Torii MCP endpoint, or operate the repository's disposable four-validator Taira devnet. Use for live account, asset, alias, contract, governance, Musubi, transaction, endpoint-health, and local deployment workflows."
---

# SORA Taira Testnet

Use current compiled Iroha surfaces and native Torii MCP. Do not reconstruct
API contracts in shell scripts.

## Select the workflow

- For a disposable local four-validator testnet, use
  `python3 scripts/taira_devnet.py up`.
- For live public Taira reads, prefer the curated `iroha.*` MCP tools at
  `https://taira.sora.org/v1/mcp`.
- For public endpoint diagnostics, use the same-revision compiled
  `iroha taira doctor --public-root https://taira.sora.org --json`.
- For a public write canary, require explicit user authorization and use
  `iroha taira write-canary` with runtime-only credentials.

Stay read-only until the user explicitly asks to mutate live state. Treat
`authority`, `private_key`, bearer tokens, onboarding tokens, and forwarded
authorization headers as runtime-only secrets; never persist them in tracked
files or committed documentation.

## Disposable deployment

Before the first run on each native AArch64 Linux validator, install direct,
root-owned, single-link executables at `/usr/bin/qemu-system-aarch64`,
`/usr/bin/setpriv`, `/usr/bin/ldd`, `/usr/bin/bwrap`, `/usr/bin/nsenter`, and
`/usr/bin/socat`. Then create the fixed parent and package the immutable runtime
from the `optimizations` checkout:

```bash
sudo install -d -o root -g root -m 0755 /opt/iroha
sudo -- python3 scripts/ci/package_inrou_runtime_v1.py
```

The packager publishes only the previously absent
`/opt/iroha/inrou-runtime-v1/{root,manifest.sha256}` and never replaces it. Its
CLI exposes only canonical absolute `--qemu`, `--setpriv`, and `--ldd` source
overrides; use the defaults for Taira AArch64. The host must also provide
root-custodied `/usr/bin/qemu-img`, a supported standard-path `iptables`, KVM
API 12, and unified cgroup-v2 `cpu`, `io`, `memory`, and `pids` controllers.
The daemon's bounded startup probe verifies the remaining namespace, QEMU
user-network listener/private connector, QMP, firewall, and cgroup posture. It
does not start or verify the workload loopback bridge.

Run from the repository root:

```bash
python3 scripts/taira_devnet.py up
python3 scripts/taira_devnet.py check
python3 scripts/taira_devnet.py down
```

`up` builds the current Kagami, daemon, and CLI and replaces one marked
owner-only directory under `/var/lib/iroha-taira-devnet/` by default. It
generates exactly four fresh-key NPoS validators on the canonical Taira chain,
binds them to loopback, validates all four configs with the current daemon,
starts them, requires health/readiness,
submits a signed ping, waits for its typed `Applied` status, requires four-peer
height convergence, and performs a semantic MCP initialize/tools-list smoke.

Treat `up` as the startup-boundary qualification command. It requires
Linux/AArch64, uid 0, KVM API version 12, and the four canonical locked Inrou
identities. Each daemon runs an artifact-free probe of the production machine
type and host CPU under KVM, private namespaces, cgroup limits, anonymous QMP,
QEMU user networking, the private loopback connector, and the owner firewall.
This does not prove guest boot, a workload, placements, or the public route;
request the real canary workspace for those checks. `up` builds the fixed
`local-release` binaries for the exact native target, records the `optimizations`
HEAD plus a pre/post tracked-diff and non-ignored-untracked worktree observation,
hashes the selected executables, and verifies live validator and CLI build
identities. The report calls this `source_observation` and sets
`cargo_source_consumption` to `not_proven`; ignored files, Cargo configuration,
external build-script inputs, toolchains, and caches are outside its scope.
`check` is read-only health/convergence inspection; it does not re-establish
KVM, source/binary, signed-write, or Inrou-route qualification.

When `--inrou-canary-dir` is used, keep the workspace outside the repository
and disjoint from both `--dir` and the qualification Cargo target. Every
ancestor must be direct, root-owned, and non-writable by group/other. The
launcher pins each input identity and digest, revalidates it before cohort
replacement, snapshots it through no-follow descriptors, and reports
`inrou_canary_input_content_sha256`; the compiled stager consumes only that
owner-only snapshot.

Use `python3 scripts/taira_devnet.py up --full-doctor` only when the broad public
product-route surface is part of the test. A minimal throwaway chain must not
be rejected merely because an unrelated optional application route is absent.

This optional local diagnostic does not qualify a public ingress. Run the
same-revision `iroha taira doctor` directly against the public ingress under
test for that purpose.

The generated bundle contains private keys and tokens. Do not print, move,
archive, or commit it. On failure the command attempts bounded teardown and
leaves bounded peer logs for diagnosis; if ownership or termination cannot be
proved, it warns instead of claiming cleanup. `down` retains those logs; the
next `up` replaces the bundle.

For the offline public-reset inventory handoff, use the single canonical
read-only command:

```bash
python3 scripts/taira_public_reset.py preflight --inventory /private/runtime/inventory.json
```

`confirm` validates an explicit inventory-bound acknowledgement. `apply` is a
fail-closed placeholder and always refuses because this repository has no
authenticated public deployment executor.

## Public MCP endpoint

The default public Torii root is `https://taira.sora.org`; its native MCP
endpoint is `https://taira.sora.org/v1/mcp`. If the operator supplies another
Torii root, use that exact deployment instead.

If MCP returns `404`, report that native Torii MCP is not enabled. If the MCP
endpoint or public root returns `502` or `503`, classify it as ingress or
upstream deployment degradation before investigating a signer or payload.

Prefer curated `iroha.*` tools over raw route wrappers. Use each tool's current
`inputSchema`; rediscover tools when the server reports a changed tool set.

The first-release public contract is exact:

- chain id: `fc56984b-2be7-431d-840e-21514d1883f0`
- native/fee XOR asset definition: `6TEAJqbb8oEPmLncoNiMRbLEK6tw`
- public XOR alias: `xor#universal`
- XOR numeric scale: `9`

Reject the retired `iroha3-taira` chain alias and legacy `<name>#<domain>`
asset-definition literals in Taira signing inputs.

## Public-node triage

Before long or stateful writes, sample:

- `iroha.health`
- `iroha.blocks.list`
- `iroha.transactions.status` or `iroha.transactions.wait`
- `GET https://taira.sora.org/status`

Proceed only when blocks advance, the queue is not saturated, and the relevant
dataspace backlog is not climbing.

Interpret common failures narrowly:

- `route_unavailable` with healthy reads usually means public ingress cannot
  reach an authoritative peer for the write lane.
- `Transaction expired` usually indicates slow/stalled finality or queue
  saturation. Report `blocks`, `commit_qc_height`, `highest_qc_height`,
  `queue_size`, `tx_queue_depth`, `tx_queue_saturated`,
  `teu_dataspace_backlog`, and `view_change_causes.last_cause` when available.
- A transaction-status `404` means only that the queried node cannot currently
  see that hash; it does not prove commit, rejection, or network-wide loss.
- `Failed to find asset` during a signed canary usually means the signer is not
  funded for the fee asset.
- `403` after a chain reset is usually a signer existence or permission issue.

Do not infer validator-set size from `/status.peers`; that is the queried
node's current remote-peer count. Do not claim a whole-network outage from one
public node without validator/operator evidence.

## Public CLI diagnostics

Copy `configs/soranexus/taira/taira-canary-client.example.toml` to an ignored,
owner-only runtime path and replace its placeholders. Then run:

```bash
target/local-release/iroha -c /private/runtime/client.toml \
  taira doctor --public-root https://taira.sora.org --json
```

For an explicitly authorized write canary:

```bash
target/local-release/iroha -c /private/runtime/client.toml \
  --fee-payer authority \
  taira write-canary \
  --public-root https://taira.sora.org \
  --onboarding-token-file /private/runtime/onboarding.token \
  --write-config /private/runtime/canary-client.toml \
  --json
```

The canary CLI owns onboarding, faucet funding, blocking submission, and
receipt verification. Do not replace it with a parallel Python implementation.

Before other live writes, verify the account exists, holds a positive fee
asset balance, and has the exact required permission. After any reset, treat
cached or previously funded signers as suspect until rechecked.

## MCP transaction and package workflows

For a pre-signed transaction envelope, prefer
`iroha.transactions.submit_and_wait`:

```json
{
  "body_base64": "<base64-encoded versioned SignedTransaction>",
  "hash": "<64-character lowercase transaction hash>",
  "status_accept": "application/json",
  "terminal_statuses": ["Applied"],
  "timeout_ms": 120000
}
```

Compute the expected hash locally and require the submit receipt hash, returned
hash fields, and final `Applied` status to match it. `Rejected`, `Expired`, a
timeout, or a successful submit without terminal `Applied` is not success. Do
not pass more than one envelope encoding.

For Musubi reads, prefer:

- `iroha.musubi.queries.exact_package`
- `iroha.musubi.queries.exact_release`
- `iroha.musubi.queries.resolver_index`
- `iroha.musubi.queries.versions`
- `iroha.musubi.queries.maintainers`
- `iroha.musubi.queries.archive_locations`
- `iroha.musubi.queries.alias`
- `iroha.musubi.queries.alias_history`
- `iroha.musubi.queries.ordered_prefix`

Pass the structural V1 package/release object required by `inputSchema`.
Human-facing `namespace/package` text is not a substitute for typed
`home_dataspace`, `scope`, and `name` fields.

Musubi instruction helpers return unsigned Norito-framed instructions. They do
not accept signing material. Assemble and sign locally, then submit with
`iroha.transactions.submit_and_wait`. Common helpers are:

- `iroha.musubi.instructions.release_publish`
- `iroha.musubi.instructions.release_yank_set`
- `iroha.musubi.instructions.alias_register`
- `iroha.musubi.instructions.release_digest_assert`

## SoraFS and application reads

For SoraFS/app-api diagnosis, sample both:

- `GET /v1/sorafs/capacity/state`
- repeated `GET /v1/app-api/cid/<cid>`

A stable app-api `404` with zero capacity declarations suggests missing provider
publication. Mixed `200`/`404` responses suggest inconsistent upstream manifest
visibility, not a bad CID.

## Response handling and safety

1. Treat a routed read as incomplete when the
   `x-iroha-fanout-routes-*` headers show any failed, denied, unavailable, or
   not-found route, even when HTTP status is 2xx. Do not reconcile or zero
   cached wallet state from an incomplete read.
2. Report transaction hashes and terminal status for successful writes.
3. Surface server errors and classify them as authentication, validation,
   missing-tool exposure, endpoint availability, or chain-health failures.
4. Do not invent operator credentials or direct validator hostnames.
5. Do not assume operator-only routes are exposed publicly.
6. Persist a newly generated signer only when the user explicitly requests it,
   and then only in an ignored owner-only runtime file.
