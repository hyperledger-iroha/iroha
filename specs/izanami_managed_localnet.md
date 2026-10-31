# Izanami on managed localnets: load, validator operations and Degraded

Status: draft design for owner review. Stage S1 (typed control requests and per-validator
status, §11) is implemented in source and has no native qualification; everything else is
unimplemented. Section 12 lists the decisions the later stages need.

Source paths are relative to `crates/iroha_deploy/src/managed/`, except paths that start
with `crates/`, `mochi/`, `scripts/` or `specs/`, and `src/…` paths in §8, which are
relative to `crates/izanami/`.

## 1. Problem

Mochi had an Izanami-backed Chaos Lab with six presets until commit `005146273a`
(2026-09-30) moved Mochi onto `iroha_deploy::managed` and deleted it, together with
Mochi's own per-peer supervisor (`mochi/mochi-core/src/chaos.rs` and
`mochi/mochi-ui-egui/src/chaos_view.rs` in `005146273a^`). The shared supervisor cannot
host that feature:

- The worker accepts five actions: `status`, `attachment_start`, `attachment_status`,
  `down` and `startup_expired` (`runtime.rs:232-285`).
  None addresses one validator.
- Any validator exit fails the worker and stops all four
  (`runtime.rs:287-293`, `runtime.rs:668-692`). A committee that tolerates `f = 1`
  cannot demonstrate it.
- Izanami starts its own network through `iroha_test_network` and reports through log
  lines (`target: "izanami::summary"`). It cannot drive a Kagami or Mochi localnet, and
  `scripts/run_izanami_*` scrape its logs.

## 2. Goals and non-goals

Goals:

- **G1 Degraded.** A managed global localnet that loses up to `f` validators keeps
  committing and reports which validators are out and why.
- **G2 Validator operations.** Stop, kill and start one validator through the worker
  that owns it, and wipe-and-resync validator 3, from Kagami, Mochi and Izanami.
- **G3 Load.** One workload engine, extracted from Izanami and safe to ship, drives a
  managed localnet at a seeded, bounded rate and returns a canonical report.
- **G4 Drills.** Load plus one scheduled validator operation, with a verdict: did the
  chain keep committing, how long was the longest stall, how long did rejoin take.
- **G5 One implementation.** Kagami, Mochi and the Izanami CLI share the engine and the
  validator operations. Izanami gains a managed target that runs from the installed
  bundle without a source checkout.

Non-goals:

- Consensus fault injection: no latency, partition, packet loss, Byzantine behavior,
  clock skew or safety-record deletion. AGENTS.md injects consensus faults only in the
  deterministic simulator, never through node configuration, and `specs/sumeragi.md`
  §13.3 already covers crashed leaders (F1), crash-restart at every boundary (F13), late
  joiners (F17) and record deletion (F24). Stop, kill and start here are operational
  actions of the owning supervisor, in the same class as `localnet down`: they inject
  nothing into a node, change no configuration and verify no protocol rule.
- Node configuration overlays, protocol toggles, consensus bypasses or empty blocks.
- Any change to startup readiness. Ready still requires all four validators, the
  readiness transaction Applied on each, and the complete mesh
  (`runtime/readiness.rs:401-466`).
- Private data space roots, multi-host networks and committees other than four.
- Release, latency (DX7) or consensus qualification. Drills are developer tools.

## 3. Constraints in the current code

| # | Fact | Source |
| - | --- | --- |
| C1 | Validators 0–2 host providers 0–2 inside the same `iroha3d` process; validator 0 also hosts the publication service. Validator 3 hosts no provider. | `generated_service_runtime.rs:90-125`, `runtime/owned.rs:278-302` |
| C2 | The generated client context targets validator 0's Torii. Deploy, call, view and the build registry use it, and deploys confirm through that one endpoint. | `crates/iroha_deploy/src/localnet/config.rs:687`, `crates/iroha_contract_deploy/src/lib.rs:1085`, `crates/iroha_contract_deploy/src/call.rs:1240` |
| C3 | `PeerProcesses::gateway` requires four children; start and stop act on the whole committee. On Windows every stop is `kill`. | `runtime/owned.rs:283`, `runtime/owned.rs:218-275`, `runtime/owned.rs:354-408`, `runtime/owned.rs:375-376` |
| C4 | A service observation lasts `DISCOVERY_FRESHNESS` = 30 s and an expired one stops the network. Discovery reads fall back across peers, but each provider's gateway must be a running child. | `build_registry.rs:24`, `runtime.rs:203-214`, `runtime.rs:427-449`, `native_operation.rs:560-578` |
| C5 | Custody renewal starts at the enrollment midpoint, withdraws Ready and may restart all four validators. Enrollment validity is at most one day. | `runtime.rs:479-541`, `runtime/renewal.rs:26-70`, `service_policies.rs:216` |
| C6 | The first launch of each validator passes `--sumeragi-assert-fresh-key` and records `peer{i}.launch` = `1` before exec. Later launches never assert fresh keys. | `runtime.rs:611-658` |
| C7 | `storage/peer{i}` holds Kura and its snapshot directory. `state/peer{i}` holds `sumeragi-records`, `sumeragi-installation.log`, provider archives and caches. Provider archives require State height = Kura height, and a height-zero start requires an empty archive namespace. | `crates/iroha_deploy/src/localnet/config.rs:53-66`, `crates/iroha_deploy/src/localnet/config.rs:330-375`, `crates/irohad/src/sorafs_reputation_finalized_query.rs:880-893`, `crates/irohad/src/sorafs_provider_ingest_finalized_query.rs:270-283` |
| C8 | Readiness already reads each peer separately through `status(peer)` and `wait_applied(peer, hash)`. | `runtime/readiness.rs:161-173`, `runtime/readiness.rs:401-415` |
| C9 | Mochi shows a validator count only, because the status carries no per-validator data. Its `ready()` gate and stream connection require `Ready`. | `mochi/mochi-ui-egui/src/gui/widgets.rs:216-217`, `mochi/mochi-ui-egui/src/gui.rs:878`, `mochi/mochi-ui-egui/src/gui.rs:1330` |

Consequences:

- Validator 0 is not stopped on purpose in this design (C1, C2).
- Validators 1–2 can be stopped only after service readiness is separated from network
  liveness (C4). Until then only validator 3 is eligible.
- Wipe-and-resync is limited to validator 3 and never deletes Sumeragi records or the
  installation log (C6, C7).
- Renewal waits while the network is `degraded` (C5).

## 4. Status model

### 4.1 Phases

`ManagedPhase` (`mod.rs:316-361`) gains `degraded`:

| Phase | Meaning |
| --- | --- |
| `stopped` | No worker owns the generation. Unchanged. |
| `starting` | Startup, activation or renewal is in progress. All four validators are required. Unchanged. |
| `ready` | All four validators run and are caught up, the mesh is complete, and services are current or not configured. |
| `degraded` | Readiness was proved; at least `q = n - f` validators run; and at least one validator is out or catching up, or services have not yet recovered from a provider validator's outage. The ledger accepts transactions. |
| `failed` | Startup failed, or fewer than `q` validators run. Unchanged cleanup. |

`f = (n - 1) / 3` and `q = n - f` come from the committee size; nothing hard-codes 3.

Transitions:

- `ready → degraded`: an admitted stop, kill or wipe; or an unrequested exit that leaves
  at least `q` running with nothing else out.
- `degraded → ready`: every validator is `running` with its rejoin proof (§6.3), and
  services are current or not configured.
- `degraded → failed`: another exit leaves fewer than `q` running. The existing closed
  cleanup stops everything.
- `starting`: unchanged. Any exit fails the worker, because startup and renewal need
  all four.
- `down` from any phase: unchanged.
- `up` on `degraded`: starts every out validator and waits for `ready` (§7.1).

### 4.2 Per-validator status

`ManagedStatus` (`mod.rs`) gains `validators`, one entry per committee member while a
worker runs and empty otherwise. A failed session keeps only the entries whose
unrequested exit caused the failure. `running_peers` keeps its cleanup meaning.

S1 implements the subset below. `Running` means the owned process is alive; network
readiness stays in the phase. Exits fail the worker exactly as before, and the failure
text names the validator by its log (`a supervised validator exited; inspect its retained
log peer2.log`), because Kagami and logs number validators 0–3 while Mochi shows V1–V4
(decision D4).

```rust
pub struct ManagedValidatorStatus {
    /// 0..=3, the same index as `localnet logs --peer` and `peer{i}.log`.
    pub index: u8,
    pub state: ManagedValidatorState,
}

pub enum ManagedValidatorState {
    Running,
    Exited { code: Option<i32>, signal: Option<i32> },
}
```

S2 adds `roles` (`client_endpoint`, `provider`, `publication`) and the states
`CatchingUp { since_unix_ms }`, `Stopping { requested }`,
`Stopped { requested, restart_at_unix_ms }` and `Wiping`, plus `at_unix_ms` on `Exited`.

Values are closed and never contain child output, like `progress::Failure`
(`runtime/progress.rs`). Frontends read log tails through the existing `logs --peer`
path.

### 4.3 Services status

```rust
pub enum ManagedServicesStatus {
    /// Standard profile or private root.
    NotConfigured,
    Starting,
    Current,
    Unavailable { providers: Vec<u8>, cause: ManagedServicesCause },
}

pub enum ManagedServicesCause {
    ValidatorOut,
    ObservationExpired,
}
```

The only behavior change (decision D1): today a lapsed observation stops the network
(C4). Under this design, a lapse **for a provider whose validator the worker already
records as out** sets `Unavailable { cause: ValidatorOut }` and leaves the ledger
running. Package publication and build-registry reads then refuse with "package
services are unavailable while validator N is out". Every other lapse keeps today's
fail-closed behavior. The relaxation is limited to an outage the supervisor itself
caused or observed.

## 5. Control protocol

### 5.1 Typed request and reply

`ControlRequest { token, action: String }` (`mod.rs:418-423`) becomes a closed enum.
Unknown actions currently get no reply (`runtime.rs:284`); the typed protocol
answers every admitted request, either with a result or with an explicit refusal.

```rust
struct ControlRequest {
    token: String,
    action: ControlAction,
}

#[norito(tag = "kind", content = "value", rename_all = "snake_case", deny_unknown_fields)]
enum ControlAction {
    Status,
    Down,
    StartupExpired,
    AttachmentStart,
    AttachmentStatus,
    Validator { index: u8, operation: ValidatorOperation },
}

enum ValidatorOperation {
    /// `restart_after_ms` bounds the outage; the worker restarts the validator itself.
    Stop { mode: ManagedStopMode, restart_after_ms: u64 },
    Start,
    WipeAndResync,
}

enum ManagedStopMode {
    /// SIGTERM, five-second grace, then kill. Windows uses kill, as `down` does today.
    Graceful,
    Kill,
}

enum ControlReply {
    Status(ManagedStatus),
    Dataspace(ManagedDataspaceStatus),
    Refused(ControlRefusal),
}
```

`ControlRefusal` is the closed set of codes in §5.2. Authentication is unchanged: every
request carries the worker token from the owner-private `worker.json`.

This is a first-release wire change between frontends and the worker. Both come from
the same pinned installation, so no compatibility path is needed.

S1 implements only the typed request: `ControlRequest.action` is the closed
`ControlAction` enum without the `Validator` variant, and replies keep their current
types (`ManagedStatus`, or `ManagedDataspaceStatus` for attachment status). A malformed or
unknown request still decodes to nothing and gets no reply. `ControlReply` and
`Refused` land with S2, whose validator operations are their first users.

### 5.2 Admission

The worker decides under its own state. Frontend checks are advisory.

| Refusal | When |
| --- | --- |
| `unsupported_root` | The generation is a private root. |
| `not_running` | Phase is `stopped` or `failed`. |
| `busy` | Phase is `starting` (startup, activation or renewal), or another validator transition is in progress. |
| `client_endpoint` | Stop, kill or wipe of validator 0. |
| `wipe_scope` | Wipe of any validator except 3. |
| `services_split_pending` | Stop or kill of validators 1–2 before §4.3 lands. |
| `quorum` | The operation would leave fewer than `q` validators running, or another validator is already out. |
| `already_running`, `already_out` | Start of a running validator; stop of one that is out. |
| `outage_bound` | `restart_after_ms` outside 10 s..=30 min (decision D3). |
| `renewal_due` | A custody renewal falls due before `now + restart_after + 120 s + 5 min`. The 120 s is the renewal turn maximum (`runtime/renewal.rs:26`). |

`start` is accepted for any index, including 0, so an unrequested exit of validator 0
can be repaired.

### 5.3 Frontend API

```rust
impl ManagedStore {
    /// Apply one validator operation to the captured generation and observe it until the
    /// transition reaches its target state or `wait` elapses.
    pub fn validator_captured(
        &self,
        expected: &PreparedLocalnet,
        index: u8,
        operation: ValidatorOperation,
        wait: Duration,
    ) -> Result<ManagedStatus>;
}
```

It follows `down_captured` (`store.rs:645-734`): take `operation.lock`, compare
the captured generation, send one request, then observe status. The lock is released
when the call returns; the worker owns the rest of the outage. A refusal becomes
`Error::ValidatorRefused(ControlRefusal)`, and its message names the next step, for
example: "validator 0 serves the default client endpoint and cannot be stopped; choose
validator 1, 2 or 3".

## 6. Worker changes

### 6.1 Fixed validator slots

`PeerProcesses.children: Vec<Arc<Mutex<Child>>>` (`runtime/owned.rs:69-73`) becomes four
fixed slots:

```rust
struct ValidatorSlot {
    index: u8,
    process: Option<Arc<Mutex<Child>>>,
    state: SlotState,
}
```

- `gateway(provider)` checks only that provider's slot; the four-children requirement
  (`runtime/owned.rs:283`) goes.
- Whole-committee `start` and `stop` keep their current behavior for startup, renewal
  restarts and `down`.
- `start_one(index)` reuses the retained daemon program, the current `GeneratedLaunch`
  revision that the other validators run (exact config path and BLAKE3; a renewal
  restart replaces it), the inherited ownership handle on stdin and
  the appended `peer{i}.log` (`runtime/owned.rs:247-270`). It requires `peer{i}.launch` =
  `1` and never passes `--sumeragi-assert-fresh-key`. It revalidates the retained daemon
  pin first, so a rebuilt `iroha3d` refuses the restart, exactly as `up` does
  (`store.rs:403-407`).
- `stop_one(index, mode)` signals only the owned handle in that slot. No numeric PID is
  stored or signalled.
- Transitions never block the accept loop. Admission is synchronous; the grace period,
  wipe and rejoin proof run as loop steps or in a dedicated background slot (like
  `refresh`), so status requests keep their 2 s budget (`transport.rs:12`).

### 6.2 Exit handling

`check_owned_validator_exit` (`runtime.rs:668-692`) becomes per slot:

- Phase `starting`: unchanged; the worker fails.
- Requested stop, kill or wipe: record `stopped` or `wiping`. This is not a failure.
- Unrequested exit with at least `q` still running and nothing else out: record
  `exited` with code and signal, set phase `degraded`, and set services `unavailable`
  if the validator hosts a provider.
- Otherwise: the existing `fail_worker` path.

Unrequested exits are not restarted automatically (decision D2); a crash is a signal
for the developer. Requested stops restart at `restart_at_unix_ms`.

### 6.3 Rejoin proof

A restarted validator is `catching_up` until three checks pass:

1. Its local status responds with `peers >= n - 1`, the per-peer form of `wait_status`
   with mesh (`runtime/readiness.rs:401-415`).
2. Its committed height `h_i` is at least `h_r`, the highest height the running
   validators reported when the restart finished.
3. Its block hash at `h_r` equals the hash a running validator reports at `h_r`.

The proof submits no transaction: no fee, no block, and an idle chain stays idle. It
has a 120 s bound. When that elapses the slot stays `catching_up` with a closed
`rejoin_slow` note, and the network does not fail. When all four validators are
`running` and services are current again, the phase returns to `ready`.

### 6.4 Wipe-and-resync (validator 3 only)

1. Stop validator 3 (graceful, then kill after the grace period).
2. Remove `storage/peer3`, which holds Kura and its snapshot directory, plus every
   State-derived cache whose startup check binds it to the Kura height. The list comes
   from a wipe manifest built from validator 3's generated configuration. Each store is
   classified as *derived: remove*, *durable: keep* or *unknown: refuse*, and a test
   pins the classification.
3. Never remove `state/peer3/sumeragi-records`, `state/peer3/sumeragi-installation.log`,
   keys, the configuration, `peer3.launch` or logs.
4. Start without asserting fresh keys, then run the rejoin proof from height zero.

The wipe is refused if the manifest finds a nonempty store it cannot classify.
Validators 0–2 are refused because their provider archives must match the Kura height
(C7).

### 6.5 Renewal and maintenance while degraded

- Renewal turns start only from `ready` (`runtime.rs:479`). A renewal that falls due
  during `degraded` waits. The `renewal_due` admission check keeps planned outages
  clear of renewals.
- If an unrequested outage outlasts an enrollment, recovery is the same as for a
  network that was stopped past its enrollment expiry: the next owned turn advances a
  successor body within the existing 64-body and `ReplacementLimit` bounds
  (`specs/kagami_mochi_devex_goals.md`, the sentences "If that body has expired…" and
  "Completing an already-reserved successor that expired during downtime…").
- Maintenance refresh skips providers whose validator is out, and resumes on the first
  turn after that validator is `running`.

## 7. Frontends

### 7.1 Kagami

```sh
kagami localnet validator stop <INDEX> [NAME] [--kill] [--for <DURATION>] [--json]
kagami localnet validator start <INDEX> [NAME] [--json]
kagami localnet validator wipe <INDEX> [NAME] [--json]
kagami localnet load [NAME] [--tps <N>] [--duration <DURATION>] [--accounts <N>] \
    [--seed <N>] [--budget <AMOUNT>] [--include-invalid] [--report <PATH>] [--json]
kagami localnet drill <PRESET> [NAME] [--validator <INDEX>] [--seed <N>] \
    [--report <PATH>] [--json]
```

- `--for` defaults to 5 min and is capped at 30 min. `wipe` accepts only validator 3
  in this design and refuses others with `wipe_scope`.
- While validator 0 is out, commands that use the context endpoint (deploy, call, view,
  package publish) fail fast with "validator 0 serves the default client endpoint and is
  out; run `kagami localnet validator start 0`".
- `localnet status` prints one row per validator and one services line; `--json` adds
  `validators` and `services`.
- `localnet up` on a `degraded` network starts every out validator and waits for
  `ready` within `--timeout`.
- Exit status: `status` succeeds for `degraded`; `up` succeeds only for `ready`.
- Every refusal prints its reason and the next command to run.

### 7.2 Mochi

- **Status.** The phase pill and mascot get a `degraded` color and caption. A
  per-validator strip replaces the count-only dots (C9).
- **Validator actions.** Each validator offers Stop, Kill and Start, plus Wipe on
  validator 3. Every action shows its equivalent Kagami command.
- **Observer.** Out validators are disabled in the observer picker. If the observed
  validator goes out, Mochi switches to the lowest running one and says so.
- **Banners.** "Validator 2 stopped by you · committing with 3 of 4 · restarts in 4:12 ·
  Start now". For an unrequested exit: "Validator 2 exited (code 101) · View log ·
  Start".
- **Gates.** `ready()`, `connect_streams` and `require_ready_localnet` accept
  `degraded` for ledger actions. Header Start means "bring every validator back"; Stop
  is enabled for `degraded`. Package Publish is disabled, with the services reason,
  while services are unavailable.
- **Lab view** (new):
  - *Load panel.* Inputs: rate, duration, session accounts, seed (random, always shown
    so a run can be repeated), fee budget, and an "include invalid transactions"
    toggle. Start shows a one-line preflight ("registers 8 session accounts, funds each
    with X, spends at most Y") and needs explicit confirmation, like every other Mochi
    dispatch. That one confirmation approves the whole session within its budget, the
    same bounded authorization as Kagami's `--max-fee`. While it runs: submitted/s, committed/s, confirmation p50/p95, rejected
    count, and the height of each validator, which makes a lagging validator visible.
    Stop drains in-flight work within the drain bound. The report card offers Copy
    JSON, Save, Run again with the same seed, and the equivalent `kagami localnet load`
    command.
  - *Drill panel.* Preset, validator and seed; a timeline of the scheduled operation,
    outage and rejoin; and the verdict (§9).

Validator numbering (decision D4): use 0–3 everywhere. Logs are `peer0.log`…`peer3.log`
and `--peer` takes 0–3, while Mochi currently labels validators V1–V4.

### 7.3 Izanami CLI

- `izanami --context <NAME> [--workspace <DIR>] [--state <DIR>]` targets a managed
  localnet through `ManagedStore` instead of starting a test network. It spawns no
  peers, so `--allow-net` stays a test-network gate only.
- Fault mapping on a managed target:

  | Izanami fault | Today (`crates/izanami/src/faults.rs`) | Managed target |
  | --- | --- | --- |
  | `crash_restart` | Graceful shutdown (SIGTERM, 5 s, SIGQUIT, SIGKILL), 1–4 s pause, restart | `Stop { mode: Graceful }`, then `Start`. `Kill` is new and used by drills. |
  | `wipe_storage` | Removes the test peer's `./storage` tree; Sumeragi records are siblings and survive | `WipeAndResync`, validator 3 only (§6.4) |
  | `spam_invalid_transactions` | Mints an unknown asset to `ALICE_ID` | The engine's invalid recipes, signed by session accounts |
  | `cpu_stress` | Spin threads in the Izanami process | Unchanged |
  | `disk_saturation` | Writes a 4–8 MiB file inside the peer's Kura directory | Writes to a temporary directory on the same volume, never inside a managed validator directory |
  | `network_latency`, `network_partition` | Restart the peer with altered gossip or trusted-peer configuration | Refused (decision D6) |

- Both modes write the same canonical report (`--report <PATH>`), and
  `scripts/run_izanami_*` read it instead of scraping logs.

## 8. Shared workload engine

### 8.1 Crate boundary

A new crate, `crates/izanami_workload` (decision D5), holds the engine. Kagami,
mochi-core and the Izanami CLI depend on it. It must not depend on
`iroha_test_network`, which resolves binaries through `IROHA_TEST_*` variables and may
run `cargo build`; the shipped Kagami and Mochi must never do either. It depends on the
`iroha` client, the data model, `iroha_crypto`, Norito, `rand`, `tokio`, and
`iroha_deploy` for the managed target.

Moves out of `crates/izanami`:

- Recipes and plan state from `src/instructions.rs` (`RecipeKind` and plan production,
  lines 760-799 and 1066-1107). Session state needs a public constructor from an
  existing signer. Today the only producer is `prepare_state`, which emits genesis
  instructions (lines 289-688 and 866-984).
- Pacing and submission from `src/chaos.rs`: submitter loops, the in-flight semaphore,
  the backlog cap, endpoint preference and failover (lines 1148-1362, 3719-3781 and
  4059-4063).
- Metrics and progress math from `src/chaos.rs` (lines 4262-4700, 4759-4816 and
  6976-7286).

Stays in `crates/izanami`: the test-network runner and genesis preparation
(`src/chaos.rs:2257-2513`, `2858-2931`), the configuration-overlay faults, the TUI and
settings persistence. The contract-deploy recipes stay too, because
`src/smart_contracts.rs` locates artifacts through `iroha_test_network::repo_root()`.

Changes made while moving:

- **Endpoints.** `IngressEndpoint { peer: NetworkPeer }` (`src/chaos.rs:1129-1132`)
  becomes an endpoint type with index, Torii URL, client factory, height and status.
- **Status sampling.** Leader and status sampling use an Alice client and
  `Signatory::Peer` on `/v1/sumeragi/status` (`src/chaos.rs:2750-2786`, `4382-4421`).
  The managed target uses the generated operator signing context that Mochi already
  holds (`mochi/mochi-core/src/developer.rs:560-573`), or skips leader sampling.
- **Fees.** Every hot path signs `FeePaymentIntent::authority(Vec::new(), None)`
  (`src/chaos.rs:3604`, `6232`, `6301`). Admission requires a signed limit for every
  non-zero charge (`crates/iroha_core/src/executor.rs:2974-2991`, `4692-4697`). The
  generated localnet charges 0.001 XOR per instruction plus gas
  (`crates/iroha_config/src/parameters/defaults.rs:3670-3685`,
  `crates/iroha_deploy/src/localnet/genesis.rs:299-307`). The engine therefore signs a
  bounded limit for each transaction shape, either from a fee quote
  (`crates/iroha/src/client.rs:15473`) or from the committed fee policy, as the readiness
  transaction does (`runtime/readiness/fees.rs`). Mochi's composer signs the same empty
  intent (`mochi/mochi-core/src/compose.rs:523-527`), so the limit helper should be
  shared, after a native check of the composer.

### 8.2 Target

```rust
pub trait LoadTarget: Send + Sync {
    /// Chain and network identity used for signing and report labels.
    fn chain(&self) -> ChainIdentity;
    /// Torii endpoints of running validators, in a stable order.
    fn endpoints(&self) -> Vec<Endpoint>;
    /// The account that pays session setup and funding.
    fn funder(&self) -> &iroha::client::Client;
    /// Validator operations, used by drills only.
    fn validators(&self) -> Option<&dyn ValidatorControl>;
}
```

- `ManagedTarget::capture(store, &PreparedLocalnet)` builds the target from the
  generated client configuration and the four validator Torii URLs, and implements
  `ValidatorControl` through `ManagedStore::validator_captured` (§5.3).
- It refuses private roots and any endpoint that is not loopback, so a load session can
  never reach Taira or Minamoto by mistake.
- Izanami's test network implements the same traits for its own mode.

### 8.3 Session accounts and budget

- **Setup.** Register `K` session accounts with keys generated in memory and never
  written to disk, label each with metadata `izanami.session = <id>`, and fund each
  from the developer account with its share of the budget in the fee asset.
- **Budget.** `--budget` bounds total fee spend. The default, shown in the preflight, is
  rate × duration × quoted fee × 1.5. The session stops with `budget_exhausted` before
  signing a transaction whose fee limit exceeds what remains.
- **Teardown.** Stop submitting, drain in-flight work within the drain bound (Izanami's
  `shutdown_drain_timeout`, 15 s by default), sweep the remaining fee asset back to the
  developer account, then drop the keys. Session accounts stay registered; Mochi's
  Accounts view hides `izanami.session` accounts by default.

### 8.4 Recipes for managed targets

Izanami's own runs rely on genesis it writes: a `chaosnet` domain, two asset
definitions, a treasury and at least 64 users with large balances, and treasury grants
(`src/instructions.rs:517-647`). Its stable profile is `TransferAsset` only
(`src/instructions.rs:802`). A managed localnet has none of that state. Roles also
require `CanManageRoles`, which only genesis can grant, and registering a domain needs
an SNS lease. The managed profiles are therefore:

| Profile | Recipes |
| --- | --- |
| `stable` (default) | Fee-asset transfers between session accounts; account metadata set and remove, each signed by the account itself. |
| `extended` | Adds asset-definition registration, mint, burn, NFT and asset-metadata recipes, only for definitions the developer account owns or is granted. A permission probe at session start enables them, and the report lists what ran. Self-owned trigger recipes need no grant (`crates/iroha_core/src/executor.rs:8748-8760`). |
| `--include-invalid` | Izanami's expect-failure recipes that need no genesis state (duplicate account, missing-trigger execution, unknown asset definition), signed by session accounts and counted as expected rejections. |

Excluded on managed targets: roles, domains, Space Directory manifests, replication
orders, DvP and contract deploys. Contract deploys already have their own path through
Musubi and `iroha_contract_deploy`.

### 8.5 Pacing, submission and confirmation

- Izanami's submitter loops, offsets, in-flight semaphore and backlog cap are kept.
- **Endpoints.** Only running validators. Each submitter prefers one endpoint and fails
  over up to three times, as today (`src/chaos.rs:1148-1161`, `1250-1362`). When the
  target reports a validator out, its endpoint leaves the set, and transactions in
  flight there count as `in_flight_at_outage`.
- **Confirmation.** Izanami polls the global transaction status until it reads
  `Applied` from state (`src/chaos.rs:6621-6773`) and records no per-transaction
  confirmation latency; its submit latency includes retries
  (`src/chaos.rs:6879-6884`). The engine instead
  matches hashes on the canonical block and event stream, as Mochi does, which gives a
  latency per transaction. Transactions not seen before their timeout fall back to one
  status read.
- **Prebuilt buffer.** Kept for high rates (`src/chaos.rs:3497-3511`); fee limits are
  set before signing.
- **Bounds on managed targets** (decision D9): rate up to 200 tx/s, duration up to
  30 min, up to 64 session accounts.

### 8.6 Reproducibility

`--seed` fixes the recipe choice of each submitter and the drill schedule. It does not
fix keys, nonces, interleaving or block layout: keys are random
(`src/instructions.rs:300`, `310`), the client nonce comes from the system RNG
(`crates/iroha/src/client.rs:15264`), and submitters share mutable state. "Run again
with the same seed" therefore repeats the schedule, not the exact transactions. Izanami
does not log an entropy-derived seed today (`src/chaos.rs:4210-4218`); the engine always
records the seed it used.

### 8.7 Report

Schema `iroha.izanami.load-report.v1`: canonical Norito JSON with
`deny_unknown_fields`, named like xtask's `iroha.devex.latency-sample.v1`.

| Group | Fields |
| --- | --- |
| Identity | `schema`, session id, context name, chain and network ids, engine version, seed, profile, enabled recipes, rate, duration, accounts, budget. No keys or private paths. |
| Outcome | `verdict` (`pass`, `fail`, `stopped`) and `stop_reason` (`completed`, `cancelled`, `budget_exhausted`, `target_failed`, `quorum_lost`). |
| Counts | `offered`, `submitted`, `accepted`, `applied`, `rejected` by closed reason class, `expected_rejections`, `unconfirmed`, `in_flight_at_outage`. |
| Latency | Confirmation p50, p95, p99 and max (submission to Applied); ingress p50 and p95. |
| Chain | Start and end height of each validator, blocks committed, block interval p50 and p95, longest stall. |
| Fees | Total signed limit, total charged, amount swept back. |
| Drill | The §9 verdict fields, when a drill ran. |

Kagami prints a short summary, `--json` prints the report, and `--report <PATH>` writes
it. Izanami writes the same schema, with its test-network-only fields in an optional
`test_network` object, and `scripts/run_izanami_*` read the report instead of matching
"izanami run complete" in logs (`scripts/run_izanami_liveness_matrix.py:78-80`).

## 9. Drills

A drill runs load, applies one validator operation at a seeded offset, waits for rejoin
and returns a verdict.

| Preset | Operation | Default validator | Passes when |
| --- | --- | --- | --- |
| `crash-restart` | Kill, start after 20 s | 3 | Blocks commit during the outage, rejoin finishes within 120 s, and only transactions submitted to the stopped validator lose confirmation. |
| `stop-and-hold` | Graceful stop for the chosen duration | 3 | As above, over a longer outage. |
| `provider-outage` (after S3) | Kill validator 1 or 2 for 30 s | 1 | The ledger keeps committing; services go `unavailable`, then `current` after rejoin. |
| `wipe-and-resync` | Wipe validator 3 | 3 | Validator 3 resyncs from height zero and passes the rejoin proof. |
| `bad-actor` | Burst of invalid transactions | n/a | Every invalid transaction is rejected at admission, and valid load keeps its p95 confirmation within the threshold. |

Verdict fields: `committed_during_outage`, `longest_stall_ms`, `rejoin_ms`, confirmation
p50/p95 before, during and after the outage, rejected and failed counts, and each
validator's final height. A failing drill reports the seed, so the same schedule can be
replayed, and links to the matching simulator scenario in `specs/sumeragi.md` §13.3 for
protocol-level investigation.

Compared with the removed Chaos Lab: Peer bounce becomes `crash-restart` or
`stop-and-hold`, Wipe and rejoin becomes `wipe-and-resync`, and Bad actor becomes
`bad-actor`. Slow gossip and Partition one are not carried over, because they are
consensus faults (§2). Resource pressure stays in Izanami's host-local faults.

## 10. Testing

- **Unit.** Codec round trips for `ControlAction`, `ControlReply` and the new status
  types; every admission refusal; slot state transitions; rejoin proof with a fake
  backend, following the readiness tests (`runtime/readiness.rs`, `mod tests`); services withdrawal
  on a provider outage; renewal deferral; the wipe manifest classification.
- **Native, in `iroha_deploy`.** Stop and start validator 3 under load; kill validator 1
  and observe services `unavailable`, then `current`; wipe validator 3 and resync; a
  second exit during `degraded` ends `failed` with zero running peers; `down` during
  `degraded`.
- **Engine.** The same recipes against `iroha_test_network` (Izanami's own mode) and
  against a managed localnet.
- **Kagami.** Argument parsing, refusal messages, and human and JSON output for the
  `localnet validator`, `load` and `drill` commands.
- **Mochi.** Headless previews of the strip, banners and Lab view; gates in `degraded`;
  automatic observer switch.
- **Simulator.** No Sumeragi rule changes, so no new §13.4 mutation is required.

## 11. Delivery stages

| Stage | Scope | Depends on |
| --- | --- | --- |
| S1 | Implemented in source: typed control requests, `Running`/`Exited` per-validator status, Kagami status rows and per-validator Mochi dots. Lifecycle behavior is unchanged. Typed replies move to S2. | none |
| S2 | `degraded` phase, unrequested-exit tolerance, and stop, kill and start for validator 3. Kagami `localnet validator`; Mochi validator strip. | S1 |
| S3 | Services status split; validators 1–2; renewal deferral. | S2, D1 |
| S4 | `izanami_workload` crate; `kagami localnet load`; Mochi load panel; `izanami --context`. | S1, D5 |
| S5 | Drills, wipe-and-resync for validator 3, and scripts that read reports. | S2–S4 |
| S6 | Documentation listed in §13. | each stage |

S4 depends only on S1, so load can ship before drills.

## 12. Decisions needed

- **D1.** Allow a provider outage the worker caused or observed to withdraw service
  readiness instead of stopping the network (§4.3)? This needs confirmation that no
  service path relies on the worker stopping a node to fence stale custody. The native
  protocol State already owns signing permission and revocation.
- **D2.** No automatic restart after an unrequested exit (proposed), or restart once
  with backoff?
- **D3.** Outage bound for requested stops: default 5 min, maximum 30 min (proposed).
- **D4.** Validator numbering 0–3 everywhere (proposed), including Mochi.
- **D5.** New crate `crates/izanami_workload` (manifest and lock change), rather than
  feature-gating Izanami's test-network runner.
- **D6.** Izanami's configuration-overlay faults (`network_latency`,
  `network_partition`): keep them for test-network mode only, or retire them in favor
  of the simulator?
- **D7.** Allow validator 0 once the client configuration carries every validator
  endpoint with failover? Not in this draft.
- **D8.** Private data space roots use the `Standard` profile with no providers, so all
  four validators are ledger-only. Add them after S3?
- **D9.** Load bounds on managed targets: 200 tx/s, 30 min and 64 session accounts
  (proposed).

## 13. Documentation to update as stages land

- `specs/mochi_architecture_plan.md` and `mochi/README.md`: both say validator selection
  "provides no per-peer lifecycle".
- `specs/kagami_mochi_devex_goals.md` DX2: the lifecycle command list.
- `specs/testing.md`, Izanami section.
- `crates/izanami/src/lib.rs` crate docs, `docs/repository_map.md` and
  `ci/rust_lanes.toml` for the new crate.
- The public chaos-testing guide in `iroha-docs`: managed target, report format, and
  removal of the retired `--fault-enable-network-packet-loss` flag.
- `status.md` and `roadmap.md` only when current health or outstanding outcomes change.
