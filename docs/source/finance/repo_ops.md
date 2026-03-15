---
title: Repo Operations & Evidence Guide
summary: Governance, lifecycle, and audit requirements for repo/reverse-repo flows (roadmap F1).
---

# Repo Operations & Evidence Guide (Roadmap F1)

The repo program unlocks bilateral and tri-party financing with deterministic
Norito instructions, CLI/SDK helpers, and ISO 20022 parity. This note captures
the operational contract required to satisfy roadmap milestone **F1 — repo
lifecycle documentation & tooling**. It complements the workflow-oriented
[`repo_runbook.md`](./repo_runbook.md) by articulating:

- lifecycle surfaces across CLI/SDKs/runtime (`crates/iroha_cli/src/main.rs:3821`,
  `python/iroha_python/iroha_python_rs/src/lib.rs:2216`,
  `crates/iroha_core/src/smartcontracts/isi/repo.rs:1`);
- deterministic proof/evidence capture (`integration_tests/tests/repo.rs:1`);
- tri-party custody & collateral substitution behaviour; and
- governance expectations (dual-control, audit trails, rollback playbooks).

## 1. Scope & Acceptance Criteria

Roadmap item F1 remains gated on four themes; this document enumerates the
required artefacts and links to the code/tests that already satisfy them:

| Requirement | Evidence |
|-------------|----------|
| Deterministic settlement proofs covering repo → reverse repo → substitution | `integration_tests/tests/repo.rs` captures end-to-end flows, duplicate-ID guards, margin cadence checks, and collateral substitution success/failure cases. The suite runs as part of `cargo test --workspace`. The deterministic lifecycle digest harness at `crates/iroha_core/src/smartcontracts/isi/repo.rs` (`repo_deterministic_lifecycle_proof_matches_fixture`) snapshots initiation → margin → substitution frames so auditors can diff the canonical payloads. |
| Tri-party coverage | Runtime enforces custodian-aware flows: `RepoAgreement::custodian` + `RepoAccountRole::Custodian` events (`crates/iroha_data_model/src/repo.rs:74`, `crates/iroha_data_model/src/events/data/events.rs:742`). |
| Collateral substitution tests | Reverse-leg invariants reject under-collateralised substitutions (`crates/iroha_core/src/smartcontracts/isi/repo.rs:417`) and integration tests assert the ledger clears correctly after a substitution roundtrip (`integration_tests/tests/repo.rs:261`). |
| Margin-call cadence & participant enforcement | `integration_tests/tests/repo.rs::repo_margin_call_enforces_cadence_and_participant_rules` exercises `RepoMarginCallIsi`, proving cadence-aligned scheduling, rejection of premature calls, and participant-only authorisation. |
| Governance-approved runbooks | This guide and `repo_runbook.md` provide CLI/SDK procedures, fraud/rollback steps, and evidence capture instructions for audits. |

## 2. Lifecycle Surfaces

### 2.1 CLI & Norito builders

- `iroha app repo initiate|unwind|margin|margin-call` wrap `RepoIsi`,
  `ReverseRepoIsi`, and `RepoMarginCallIsi`
  (`crates/iroha_cli/src/main.rs:3821`). Each subcommand supports `--input` /
  `--output` so desks can stage instruction payloads for dual approval before
  submission. Custodian routing is expressed via `--custodian`.
- `repo query list|get` uses `FindRepoAgreements` to snapshot agreements and can
  be redirected into JSON artefacts for evidence bundles.
- The CLI smoke tests under `crates/iroha_cli/tests/cli_smoke.rs:2637` ensure
  the emit-to-file path stays stable for auditors.

### 2.2 SDKs & automation hooks

- Python bindings expose `RepoAgreementRecord`, `RepoCashLeg`,
  `RepoCollateralLeg`, and convenience builders
  (`python/iroha_python/iroha_python_rs/src/lib.rs:2216`) so automation can
  assemble transactions and evaluate `next_margin_check_after` locally.
- JS/Swift helpers reuse the same Norito layouts via
  `javascript/iroha_js/src/instructionBuilders.js` and
  `IrohaSwift/Sources/IrohaSwift/ConfidentialEncryptedPayload.swift` for memo
  handling; SDKs should refer to this doc when threading repo governance knobs.

### 2.3 Ledger events & telemetry

Every lifecycle action emits `AccountEvent::Repo(...)` records containing
`RepoAccountEvent::{Initiated,Settled,MarginCalled}` payloads scoped to the
participant role (`crates/iroha_data_model/src/events/data/events.rs:742`). Push
those events into your SIEM/log aggregator to obtain a tamper-evident audit log
for desk actions, margin calls, and custodian notifications.

### 2.4 Configuration propagation & verification

Nodes ingest repo governance knobs from the `[settlement.repo]` stanza in
`iroha_config` (`crates/iroha_config/src/parameters/user.rs:4071`). Treat that
snippet as part of the governance evidence contract—stage it in version control
alongside the repo packet and hash it before pushing the change through your
automation or ConfigMap. A minimal profile looks like:

```toml
[settlement.repo]
default_haircut_bps = 1500
margin_frequency_secs = 86400
eligible_collateral = ["bond#wonderland", "note#wonderland"]

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["note#wonderland", "bill#wonderland"]
```

Operational checklist:

1. Commit the snippet above (or your production variant) into the config repo
   that feeds `irohad` and record its SHA-256 inside the governance packet so
   reviewers can diff the bytes you plan to deploy.
2. Roll the change across the fleet (systemd unit, Kubernetes ConfigMap, etc.)
   and restart each node. Immediately after rollout, capture the Torii
   configuration snapshot for provenance:

   ```bash
   curl -sS "${TORII_URL}/v1/configuration" \
     -H "Authorization: Bearer ${TOKEN}" | jq .
   ```

   `ToriiClient.get_configuration()` is available in the Python SDK for the same
   purpose when automation needs typed evidence.【python/iroha_python/src/iroha_python/client.py:5791】
3. Prove that the runtime now enforces the requested cadence/haircut by querying
   `FindRepoAgreements` (or `iroha app repo margin --agreement-id ...`) and
   inspecting the embedded `RepoGovernance` values. Store the JSON responses
   under `artifacts/finance/repo/<agreement>/agreements_after.json`; those values
   are derived from `[settlement.repo]`, so they act as a secondary witness when
   Torii’s `/v1/configuration` snapshot is insufficient.
4. Keep both artefacts—the TOML snippet and the Torii/CLI snapshots—in the
   evidence bundle before filing a governance request. Auditors must be able to
   replay the snippet, verify its hash, and correlate it with the runtime view.

This workflow ensures repo desks never rely on ad-hoc environment variables, the
configuration path stays deterministic, and every governance ticket carries the
same set of `iroha_config` proofs expected in roadmap F1.

### 2.5 Deterministic proof harness

The unit test `repo_deterministic_lifecycle_proof_matches_fixture` (see
`crates/iroha_core/src/smartcontracts/isi/repo.rs`) serializes each stage of the
repo lifecycle into a Norito JSON frame, compares it to the canonical fixture at
`crates/iroha_core/tests/fixtures/repo_lifecycle_proof.json`, and hashes the
bundle (fixture digest tracked at
`crates/iroha_core/tests/fixtures/repo_lifecycle_proof.digest`). Run it locally via:

```bash
cargo test -p iroha_core \
  -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
```

This test now runs as part of the default `cargo test -p iroha_core` suite, so CI
guards the snapshot automatically. Whenever repo semantics or fixtures change,
refresh both the JSON and digest with:

```bash
scripts/regen_repo_proof_fixture.sh
```

The helper uses the pinned `rust-toolchain.toml` channel, rewrites the fixtures
under `crates/iroha_core/tests/fixtures/`, and reruns the deterministic harness
so the checked-in snapshot/digest stay in sync with the runtime behaviour
auditors will replay.

### 2.4 Torii API surfaces

- `GET /v1/repo/agreements` returns the active agreements with optional pagination, filtering
  (`filter={...}`), sorting, and address-formatting parameters. Use this for quick audits or
  dashboards when the raw JSON payloads are sufficient.
- `POST /v1/repo/agreements/query` accepts the structured query envelope (pagination, sort,
  `FilterExpr`, `fetch_size`) so downstream services can page through the ledger deterministically.
- The JavaScript SDK now exposes `listRepoAgreements`, `queryRepoAgreements`, and the iterator
  helpers so browser/Node.js tooling receives the same typed DTOs as Rust/Python.

### 2.4 Configuration defaults

Nodes read `[settlement.repo]` into
`iroha_config::parameters::actual::Repo` during start-up; any repo instruction
that leaves a parameter at zero is normalised against those defaults before it
is recorded on-chain.【crates/iroha_core/src/smartcontracts/isi/repo.rs:40】 This
lets governance raise (or lower) baseline policy without touching every SDK
call-site, provided the policy change is fully documented.

- `default_haircut_bps` – fallback haircut when `RepoGovernance::haircut_bps()`
  equals zero. The runtime clamps it to the hard 10 000 bps ceiling to keep
  configs sane.【crates/iroha_core/src/smartcontracts/isi/repo.rs:44】
- `margin_frequency_secs` – cadence for `RepoMarginCallIsi`. Zeroed requests
  inherit this value, so shortening the cadence forces desks to margin more
  frequently by default.【crates/iroha_core/src/smartcontracts/isi/repo.rs:49】
- `eligible_collateral` – optional allow-list of `AssetDefinitionId`s. When the
  list is non-empty `RepoIsi` rejects any pledge outside the set, preventing
  accidental onboarding of unvetted bonds.【crates/iroha_core/src/smartcontracts/isi/repo.rs:57】
- `collateral_substitution_matrix` – map of original collateral →
  permitted substitutes. `ReverseRepoIsi` only accepts a substitution when the
  matrix contains the recorded definition as a key and the replacement in its
  value array; otherwise the unwind fails, proving that governance approved the
  ladder.【crates/iroha_core/src/smartcontracts/isi/repo.rs:74】

These knobs live under `[settlement.repo]` in the node configuration and are
parsed via `iroha_config::parameters::user::Repo`, so they should be captured in
every governance evidence bundle.【crates/iroha_config/src/parameters/user.rs:3956】

```toml
[settlement.repo]
default_haircut_bps = 1750
margin_frequency_secs = 43200
eligible_collateral = ["bond#wonderland", "note#wonderland"]

[settlement.repo.collateral_substitution_matrix]
"bond#wonderland" = ["note#wonderland", "bill#wonderland"]
```

**Change-management checklist**

1. Stage the proposed TOML snippet (including substitution matrix deltas), hash
   it with SHA-256, and attach both the snippet and hash to the governance
   ticket so reviewers can reproduce the bytes verbatim.
2. Reference the snippet inside the proposal/referendum (for example via the
   `--notes` field on the governance CLI) and collect the required approvals
   for F1. Keep the signed approval packet with the snippet attached.
3. Roll the change across the fleet: update `[settlement.repo]`, restart each
   node, then capture a `GET /v1/configuration` snapshot (or
   `ToriiClient.getConfiguration`) proving the applied values per peer.
4. Re-run `integration_tests/tests/repo.rs` plus
   `repo_deterministic_lifecycle_proof_matches_fixture` and store the logs next
   to the config diff so auditors can see that the new defaults preserve
   determinism.

Without a matrix entry the runtime rejects substitutions that change the asset
definition, even if the general `eligible_collateral` list permits it; commit
config snapshots alongside repo evidence so auditors can reproduce the exact
policy enforced when a repo was booked.

### 2.5 Configuration evidence & drift detection

The Norito/`iroha_config` plumbing now exposes the resolved repo policy under
`iroha_config::parameters::actual::Repo`, so governance packets must prove the
applied values per peer—not just the proposed TOML. Capture the resolved
configuration and its digest after every rollout:

1. Fetch the configuration from each peer (`GET /v1/configuration` or
   `ToriiClient.getConfiguration`) and isolate the repo stanza:

   ```bash
   curl -s http://<torii-host>/v1/configuration \
     | jq -cS '.settlement.repo' \
     > artifacts/finance/repo/<agreement-id>/config/repo_config_actual.json
   ```

2. Hash the canonical JSON and record it in the evidence manifest. When the
   fleet is healthy the hash should match across peers because `actual`
   combines defaults with the staged `[settlement.repo]` snippet:

   ```bash
   shasum -a 256 artifacts/finance/repo/<agreement-id>/config/repo_config_actual.json
   ```

3. Attach the JSON + hash to the governance packet and mirror the entry in the
   manifest uploaded to the governance DAG. If any peer reports a divergent
   digest, halt the rollout and reconcile the config/state drift before
   proceeding.

### 2.6 Governance approvals & evidence pack

Roadmap F1 closes only when repo desks feed a deterministic packet into the
governance DAG, so every change (new haircut, custodian policy, or collateral
matrix) must ship the same artefacts before a vote is scheduled.【docs/source/governance_playbook.md:1】

**Intake packet**

1. **Tracking template** – copy
   `docs/examples/finance/repo_governance_packet_template.md` into your evidence
   directory (for example
   `artifacts/finance/repo/<agreement-id>/packet.md`) and fill the metadata
   block before you start hashing artefacts. The template keeps the governance
   council’s cadence deterministic by listing file paths, SHA-256 digests, and
   reviewer acknowledgements in one place.
2. **Instruction payloads** – stage the initiation, unwind, and margin-call
   instructions with `iroha app repo ... --output` so dual-control approvers review
   byte-identical payloads. Hash each file and store it under
   `artifacts/finance/repo/<agreement-id>/` next to the desk’s evidence bundle
   referenced elsewhere in this note.【crates/iroha_cli/src/main.rs:3821】
3. **Configuration diff** – include the exact `[settlement.repo]` TOML snippet
   (defaults plus substitution matrix) and its SHA-256. This proves which
   `iroha_config` knobs will be active once the vote passes and mirrors the
   runtime fields that normalise repo instructions at admission time.【crates/iroha_config/src/parameters/user.rs:3956】
4. **Deterministic tests** – attach the latest
   `integration_tests/tests/repo.rs` log and the output from
   `repo_deterministic_lifecycle_proof_matches_fixture` so reviewers see the
   lifecycle proof hash that corresponds to the staged instructions.【integration_tests/tests/repo.rs:1】【crates/iroha_core/src/smartcontracts/isi/repo.rs:1450】
5. **Event/telemetry snapshot** – export the recent `AccountEvent::Repo(*)`
   stream for the desks in scope plus any dashboards/metrics the council needs
   to judge risk (for example, margin drift). This gives auditors the same
   tamper-evident log they would reconstruct from Torii later.【crates/iroha_data_model/src/events/data/events.rs:742】

**Approval & logging**

- Reference the artefact hashes inside the governance ticket or referendum and
  link to the staged packet so the council can follow the standard ceremony
  outlined in the governance playbook without chasing ad-hoc paths.【docs/source/governance_playbook.md:8】
- Capture which dual-control signers reviewed the staged instruction files and
  store their acknowledgements next to the hashes; this is the on-chain proof
  that repo desks satisfied the “two-person rule” even though the runtime also
  enforces participant-only execution.
- When the council publishes the Governance Approval Record (GAR), mirror the
  signed minutes inside the evidence directory so future substitutions or
  haircut updates can cite the exact decision packet instead of restating the
  rationale.

**Post-approval rollouts**

1. Apply the approved `[settlement.repo]` config and restart each node (or roll
   it via your automation). Immediately call `GET /v1/configuration` and archive
   the response per node so the governance bundle shows which peers accepted the
   change.【crates/iroha_torii/src/lib.rs:3225】
2. Re-run the deterministic repo tests and attach the fresh logs plus build
   metadata (git commit, toolchain) so auditors can reproduce the settlement
   proof after the rollout.
3. Update the governance tracker with the evidence archive path, hashes, and
   observer contact so later repo desks can inherit the same process instead of
   re-deriving the checklist.

**Governance DAG publication (required)**

1. Tar the evidence directory (config snippet, instruction payloads, proof logs,
   GAR/minutes) and hand it to the governance DAG pipeline as a
   `GovernancePayloadKind::PolicyUpdate` payload with annotations for
   `agreement_id`, `iso_week`, and the proposed haircut/margin values; the
   pipeline spec and CLI surfaces live in
   `docs/source/sorafs_governance_dag_plan.md`.
2. After the publisher updates the IPNS head, record the block CID and head CID
   in the governance tracker and in the GAR so anyone can fetch the immutable
   packet later. `sorafs governance dag head` and `sorafs governance dag list`
   let you confirm the node was pinned before the vote opens.
3. Store the CAR file or block payload next to the repo evidence archive so
   auditors can reconcile the on-chain governance decision with the exact
   off-chain packet that was approved.

### 2.7 Lifecycle snapshot refresh

Whenever repo semantics change (rates, settlement maths, custody logic, or
default config), refresh the deterministic lifecycle snapshot so governance can
cite the new digest without reverse engineering the proof harness.

1. Refresh the fixtures under the pinned toolchain:

   ```bash
   scripts/regen_repo_proof_fixture.sh --toolchain <toolchain> \
     --bundle-dir artifacts/finance/repo/<agreement>
   ```

   The helper stages outputs in a temp directory, updates the tracked fixtures
   at `crates/iroha_core/tests/fixtures/repo_lifecycle_proof.{json,digest}`,
   reruns the proof test for verification, and (when `--bundle-dir` is set)
   drops `repo_proof_snapshot.json` and `repo_proof_digest.txt` into the bundle
   directory for auditors.
2. To export artefacts without touching the tracked fixtures (e.g., dry-run
   evidence), set the env helpers directly:

   ```bash
   REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<agreement>/repo_proof_snapshot.json \
   REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<agreement>/repo_proof_digest.txt \
   cargo test -p iroha_core \
     -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
   ```

   `REPO_PROOF_SNAPSHOT_OUT` receives the prettified Norito JSON from the proof
   harness while `REPO_PROOF_DIGEST_OUT` stores the uppercase hex digest (with a
   trailing newline for convenience). The helper refuses to overwrite files when
   the parent directory does not exist, so build the `artifacts/...` tree first.
3. Attach both exported files to the agreement bundle (see §3) and regenerate
   the manifest via `scripts/repo_evidence_manifest.py` so the governance packet
   references the refreshed proof artefacts explicitly. The in-repo fixtures
   remain the source of truth for CI.

### 2.8 Interest accrual & maturity governance

**Deterministic interest math.** `RepoIsi` and `ReverseRepoIsi` derive the cash
owed at unwind time from the ACT/360 helper
`compute_accrued_interest()`【crates/iroha_core/src/smartcontracts/isi/repo.rs:100】
and the guard inside `expected_cash_settlement()` that rejects repayment legs
which return less than *principal + interest*.【crates/iroha_core/src/smartcontracts/isi/repo.rs:132】
The helper normalises `rate_bps` into a four-decimal fraction, multiplies it by
`elapsed_ms / (360 * 24h)` using 18 decimal places, and finally rounds to the
scale declared by the cash leg’s `NumericSpec`. To keep the governance packet
reproducible, capture the four values that feed the helper:

1. `cash_leg.quantity` (principal),
2. `rate_bps`,
3. `initiated_timestamp_ms`, and
4. the unwind timestamp you intend to use (for planned GL entries this is
   usually `maturity_timestamp_ms`, but emergency unwinds record the actual
   `ReverseRepoIsi::settlement_timestamp_ms`).

Store the tuple alongside the staged unwind instruction and attach a short proof
snippet such as:

```python
from decimal import Decimal
ACT_360_YEAR_MS = 24 * 60 * 60 * 1000 * 360

principal = Decimal("1000")
rate_bps = Decimal("1500")  # 150 bps
elapsed_ms = Decimal(maturity_ms - initiated_ms)
interest = principal * (rate_bps / Decimal(10_000)) * (elapsed_ms / Decimal(ACT_360_YEAR_MS))
expected_cash = principal + interest.quantize(Decimal("0.01"))
```

The rounded `expected_cash` must match the `quantity` encoded in the reverse
repo instruction. Keep the script output (or calculator worksheet) in
`artifacts/finance/repo/<agreement>/interest.json` so auditors can recompute the
figure without interpreting your trading spreadsheet. The integration suite
already enforces the same invariant
(`repo_roundtrip_transfers_balances_and_clears_agreement`), but ops evidence
should cite the exact values that will be unwound.【integration_tests/tests/repo.rs:1】

**Margin & accrual cadence.** Every agreement exposes the cadence helpers
`RepoAgreement::next_margin_check_after()` and the cached
`last_margin_check_timestamp_ms`, enabling desks to prove that margin sweeps
were scheduled according to policy even before they submit a `RepoMarginCallIsi`
transaction.【crates/iroha_data_model/src/repo.rs:113】【crates/iroha_core/src/smartcontracts/isi/repo.rs:557】
Each margin call must include three artefacts in the evidence bundle:

1. `repo margin-call --agreement <id>` JSON output (or the equivalent SDK
   payload), which records the agreement id, the block timestamp used for the
   check, and the authority that triggered it.【crates/iroha_cli/src/main.rs:3821】
2. A snapshot of the agreement (`repo query get --agreement-id <id>`) taken
   immediately before the call so reviewers can confirm the cadence was due
   (compare `current_timestamp_ms` with `next_margin_check_after()`).
3. The `AccountEvent::Repo::MarginCalled` SSE/NDJSON feed emitted to each role
   (initiator, counterparty, and optionally custodian) because the runtime
   duplicates the event for every participant.【crates/iroha_data_model/src/events/data/events.rs:742】

CI already exercises these rules via
`repo_margin_call_enforces_cadence_and_participant_rules`, which rejects calls
that arrive early or from unauthorised accounts.【integration_tests/tests/repo.rs:395】
Repeating that provenance in the evidence archive is what closes the roadmap F1
documentation gap: governance reviewers can see the same timestamps that the
runtime relied on, together with the deterministic proof hash captured in §2.7
and the manifest discussed in §3.2.

### 2.8 Tri-party custody approvals & monitoring

Roadmap **F1** also calls out tri-party repos where collateral is parked with a
custodian rather than the counterparty. The runtime enforces the custodian path
by persisting `RepoAgreement::custodian`, routing pledged assets into the
custodian’s account during initiation, and emitting
`RepoAccountRole::Custodian` events for every lifecycle step so auditors can see
who held collateral at each timestamp.【crates/iroha_data_model/src/repo.rs:74】【crates/iroha_core/src/smartcontracts/isi/repo.rs:252】【integration_tests/tests/repo.rs:951】
In addition to the bilateral evidence listed above, every tri-party repo must
capture the artefacts below before the governance packet is considered complete.

**Additional intake requirements**

1. **Custodian acknowledgement.** Desks must store a signed acknowledgement from
   each custodian confirming the repo identifier, custody window, routing
   account, and settlement SLAs. Attach the signed document
   (`artifacts/finance/repo/<agreement>/custodian_ack_<custodian>.md`)
   and reference it in the governance packet so reviewers can see that the
   third party agreed to the same bytes the initiator/counterparty approved.
2. **Custody ledger snapshot.** Initiation moves collateral into the custodian
   account and unwind returns it to the initiator; capture the relevant
   `FindAssets` output for the custodian before and after each leg so auditors
   can confirm the balances match the staged instructions.【crates/iroha_core/src/smartcontracts/isi/repo.rs:252】【crates/iroha_core/src/smartcontracts/isi/repo.rs:1641】
3. **Event receipts.** Mirror the `RepoAccountEvent` stream for all roles and
   store the custodian payload alongside the initiator/counterparty records.
   The runtime emits separate events for each role in
   `RepoAccountRole::{Initiator,Counterparty,Custodian}`, so attaching the raw
   SSE feed proves that all three parties saw the same timestamps and
   settlement amounts.【crates/iroha_data_model/src/events/data/events.rs:742】【integration_tests/tests/repo.rs:1508】
4. **Custodian readiness checklist.** When the repo references operational
   shims (for example, escrow reconciliations or standing instructions), record
   the automation contact and the command used to rehearse the workflow (such
   as `iroha app repo initiate --custodian ... --dry-run`) so reviewers can reach
   the custodian operators during drills.

| Evidence | Command / Path | Purpose |
|----------|----------------|---------|
| Custodian acknowledgement (`custodian_ack_<custodian>.md`) | Link to the signed note referenced in `docs/examples/finance/repo_governance_packet_template.md` (use `docs/examples/finance/repo_custodian_ack_template.md` as the seed). | Shows the third party accepted the repo id, custody SLA, and settlement channel before assets move. |
| Custody asset snapshot | `iroha json --query FindAssets '{ "id": "...#<custodian>" }' > artifacts/.../assets/custodian_<ts>.json` | Proves collateral left/returned exactly as `RepoIsi` encoded it. |
| Custodian `RepoAccountEvent` feed | `torii-events --account <custodian> --event-type repo > artifacts/.../events/custodian.ndjson` | Captures the `RepoAccountRole::Custodian` payloads the runtime emitted for initiation, margin calls, and unwind. |
| Custody drill log | `artifacts/.../governance/drills/<timestamp>-custodian.log` | Documents dry runs where the custodian exercised their rollback or settlement scripts. |

Reusing the same hashing workflow (`scripts/repo_evidence_manifest.py`) for the
custodian acknowledgement, asset snapshots, and event feeds keeps tri-party
packets reproducible. When multiple custodians participate in a book, create
subdirectories per custodian so the manifest highlights which files belong to
each party; the governance ticket should reference each manifest hash and the
matching acknowledgement file. The integration tests that cover
`repo_initiation_with_custodian_routes_collateral` and
`reverse_repo_with_custodian_emits_events_for_all_parties` already enforce the
runtime behaviour—mirroring their artefacts inside the evidence bundle is what
lets roadmap **F1** ship GA-ready documentation for the tri-party scenario.【integration_tests/tests/repo.rs:951】【integration_tests/tests/repo.rs:1508】

### 2.9 Post-approval configuration snapshots

Once governance approves a change and the `[settlement.repo]` stanza lands on
the cluster, capture an authenticated configuration snapshot from every peer so
auditors can prove the approved values are live. Torii exposes the
`/v1/configuration` route for this purpose and all SDKs surface helpers such as
`ToriiClient.getConfiguration`, so the capture workflow works for desk scripts,
CI, or manual operator runs.【crates/iroha_torii/src/lib.rs:3225】【javascript/iroha_js/src/toriiClient.js:2115】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:4681】

1. Call `GET /v1/configuration` (or the SDK helper) per peer immediately after
   the roll-out. Persist the full JSON under
   `artifacts/finance/repo/<agreement>/config/peers/<peer-id>.json` and record
   the block height/cluster timestamp in `config/config_snapshot_index.md`.
   ```bash
   mkdir -p artifacts/finance/repo/<slug>/config/peers
   curl -fsSL https://peer01.example/v1/configuration \
     | jq '.' \
     > artifacts/finance/repo/<slug>/config/peers/peer01.json
   ```
2. Hash every snapshot (`sha256sum config/peers/*.json`) and log the digest next
   to the peer id in the governance packet template. This proves which peers
   ingested the policy and which commit/toolchain produced the snapshot.
3. Compare the `.settlement.repo` block in each snapshot with the staged
   `[settlement.repo]` TOML snippet; record any drift and rerun
   `repo query get --agreement-id <id> --pretty` so the evidence bundle shows
   both the runtime configuration and the normalised `RepoGovernance` values
   stored with the agreement.【crates/iroha_cli/src/main.rs:3821】
4. Attach the snapshot files and summary index to the evidence manifest (see
   §3.2) so the governance record links the approved change to the actual peer
   configuration bytes. The governance template was updated to include this
   table, so every future repo packet carries the same proof.

Capturing these snapshots closes the `iroha_config` documentation gap called out
in the roadmap: reviewers can now diff the staged TOML against the bytes every
peer reports, and auditors can re-run the comparison whenever a repo change is
under investigation.

## 3. Deterministic Evidence Workflow

1. **Record the instruction provenance**
   - Generate the repo/unwind payload via `iroha app repo ... --output`.
   - Store the `InstructionBox` JSON under
     `artifacts/finance/repo/<agreement-id>/initiation.json`.
2. **Capture ledger state**
   - Run `iroha app repo query list --pretty > artifacts/.../agreements.json` before
     and after settlement to prove balances cleared.
   - Optionally query `FindAssets` via `iroha json` or SDK helpers to archive
     the asset balances touched in the repo leg.
3. **Persist event streams**
   - Subscribe to `AccountEvent::Repo` over Torii SSE or Pull and attach the
     emitted JSON to the evidence directory. This satisfies the tamper-evident
     logging clause because the events are signed by the peers that observed
     each change.
4. **Run deterministic tests**
   - CI already runs `integration_tests/tests/repo.rs`; for manual sign-off,
     execute `cargo test -p integration_tests repo::` and archive the log plus
     `target/debug/deps/repo-*` JUnit output.
5. **Serialize governance & config**
   - Check in (or attach) the `[settlement.repo]` config used for the period,
     including haircut/eligible lists. This allows audit replays to match the
     runtime-normalised governance recorded in `RepoAgreement`.

### 3.1 Evidence bundle layout

Store every artefact called out in this section beneath a single agreement
directory so governance can archive or hash one tree. The recommended layout is:

```
artifacts/finance/repo/<agreement-id>/
├── agreements_before.json
├── agreements_after.json
├── initiation.json
├── unwind.json
├── margin/
│   └── 2026-04-30.json
├── events/
│   └── repo-events.ndjson
├── config/
│   ├── settlement_repo.toml
│   └── peers/
│       ├── peer01.json
│       └── peer02.json
├── repo_proof_snapshot.json
├── repo_proof_digest.txt
└── tests/
    └── repo_lifecycle.log
```

- `agreements_before/after.json` capture `repo query list` output so auditors can
  prove the ledger cleared the agreement.
- `initiation.json`, `unwind.json`, and `margin/*.json` are the exact Norito
  payloads staged with `iroha app repo ... --output`.
- `events/repo-events.ndjson` replays the `AccountEvent::Repo(*)` stream while
  `tests/repo_lifecycle.log` preserves the `cargo test` evidence.
- `repo_proof_snapshot.json` and `repo_proof_digest.txt` come from the snapshot
  refresh procedure in §2.7 and let reviewers recompute the lifecycle hash
  without re-running the harness.
- `config/settlement_repo.toml` contains the `[settlement.repo]` snippet
  (haircuts, substitution matrix) that was active when the repo executed.
- `config/peers/*.json` captures `/v1/configuration` snapshots for each peer,
  closing the loop between the staged TOML and the runtime values peers report
  over Torii.

### 3.2 Hash manifest generation

Attach a deterministic manifest to every bundle so reviewers can verify hashes
without unpacking the archive. The helper at `scripts/repo_evidence_manifest.py`
walks the agreement directory, records `size`, `sha256`, `blake2b`, and the last
modified timestamp for each file, and writes a JSON summary:

```bash
python3 scripts/repo_evidence_manifest.py \
  --root artifacts/finance/repo/wonderland-2026q1 \
  --agreement-id repo#wonderland \
  --output artifacts/finance/repo/wonderland-2026q1/manifest.json \
  --exclude 'scratch/*'
```

The generator sorts paths lexicographically, skips the output file when it lives
inside the same directory, and emits totals that governance can copy directly
into the change ticket. When `--output` is omitted the manifest prints to
stdout, which is convenient for quick diffs during desk reviews.
Use `--exclude <glob>` to omit scratch material (for example, `--exclude 'scratch/*' --exclude '*.tmp'`)
without moving files out of the bundle; the glob pattern always applies to the
path relative to `--root`.

Example manifest (truncated for brevity):

```json
{
  "agreement_id": "repo#wonderland",
  "generated_at": "2026-04-30T11:58:43Z",
  "root": "/var/tmp/repo/wonderland-2026q1",
  "file_count": 5,
  "total_bytes": 1898,
  "files": [
    {
      "path": "agreements_after.json",
      "size": 512,
      "sha256": "6b6ca81b00d0d889272142ce1e6456872dd6b01ce77fcd1905f7374fc7c110cc",
      "blake2b": "5f0c7f03d15cd2a69a120f85df2a4a4a219a716e1f2ec5852a9eb4cdb443cbfe3c1e8cd02b3b7dbfb89ab51a1067f4107be9eab7d5b46a957c07994eb60bb070",
      "modified_at": "2026-04-30T11:42:01Z"
    },
    {
      "path": "initiation.json",
      "size": 274,
      "sha256": "7a1a0ec8c8c5d43485c3fee2455f996191f0e17a9a7d6b25fc47df0ba8de91e7",
      "blake2b": "ce72691b4e26605f2e8a6486d2b43a3c2b472493efd824ab93683a1c1d77e4cff40f5a8d99d138651b93bcd1b1cb5aa855f2c49b5f345d8fac41f5b221859621",
      "modified_at": "2026-04-30T11:39:55Z"
    }
  ]
}
```

Include the manifest next to the evidence bundle and reference its SHA-256 hash
in the governance proposal so desks, operators, and auditors share the same
ground truth.

### 3.3 Governance change log & rollback drills

The finance council expects every repo request, haircut tweak, or substitution
matrix change to arrive with a reproducible governance packet that can be linked
directly from the referendum minutes.【docs/source/governance_playbook.md:1】

1. **Build the governance packet**
   - Copy the evidence bundle for the agreement into
     `artifacts/finance/repo/<agreement-id>/governance/`.
   - Add `gar.json` (council approval record), `referendum.md` (who approved
     and which hashes they reviewed), and `rollback_playbook.md`
     summarising the reversal procedure from `repo_runbook.md`
     §§4–5.【docs/source/finance/repo_runbook.md:1】
   - Capture the deterministic manifest hash from §3.2 in
     `hashes.txt` so reviewers can confirm the payloads they see in Torii match
     the staged bytes.
2. **Reference the packet in the referendum**
   - When running `iroha app governance referendum submit` (or the equivalent SDK
     helper) include the manifest hash from `hashes.txt` in the `--notes`
     payload so the GAR points back to the immutable packet.
   - File the same hash inside the governance tracker or ticketing system so
     audit traces do not rely on screenshotting dashboards.
3. **Document drills and rollbacks**
   - After the referendum passes, update `ops/drill-log.md` with the repo
     agreement id, deployed config hash, GAR id, and operator contact so the
     quarterly drill record includes finance actions.【ops/drill-log.md:1】
   - If a rollback drill executes, attach the signed
     `rollback_playbook.md` and the CLI output from `iroha app repo unwind` under
     `governance/drills/<timestamp>.log` and inform the council using the same
     steps described in the governance playbook.

Example layout:

```
artifacts/finance/repo/<agreement-id>/governance/
├── gar.json
├── hashes.txt
├── referendum.md
├── rollback_playbook.md
└── drills/
    └── 2026-05-12T09-00Z.log
```

Keeping the GAR, referendum, and drill artefacts alongside the lifecycle
evidence guarantees that every repo change satisfies the roadmap F1 governance
bar without requiring bespoke ticket spelunking later on.

### 3.4 Lifecycle governance checklist

Roadmap **F1** calls out governance coverage for initiation, accrual/margin, and
tri-party unwinds. The table below consolidates the approvals, deterministic
artefacts, and test references per lifecycle step so finance desks can cite a
single checklist when assembling a packet.

| Lifecycle step | Required approvals & tickets | Deterministic artefacts & commands | Linked regression coverage |
|----------------|------------------------------|------------------------------------|----------------------------|
| **Initiation (bilateral or tri-party)** | Dual-control sign-off recorded via `docs/examples/finance/repo_governance_packet_template.md`, governance ticket with the `[settlement.repo]` diff and GAR ID, custodian acknowledgement when `--custodian` is set. | Stage the instruction via<br>`iroha --config client.toml --output repo initiate ...`.<br>Emit the lifecycle proof snapshot (`REPO_PROOF_*` env vars) plus the bundle manifest from `scripts/repo_evidence_manifest.py`.<br>Attach the latest `FindRepoAgreements` JSON and `[settlement.repo]` snippet (haircut, eligible list, substitution matrix). | `integration_tests/tests/repo.rs::repo_roundtrip_transfers_balances_and_clears_agreement` (bilateral) and `integration_tests/tests/repo.rs::repo_roundtrip_with_custodian_routes_collateral` (tri-party) prove the runtime matches the staged payloads. |
| **Margin call accrual cadence** | Desk lead + risk manager approve the cadence window documented in the governance packet; ticket references the scheduled `RepoMarginCallIsi`. | Capture `iroha app repo margin --agreement-id` output before calling `iroha app repo margin-call`, hash the resulting JSON, and archive the `RepoAccountEvent::MarginCalled` SSE payload in the evidence bundle.<br>Store the CLI log next to the deterministic proof hash. | `integration_tests/tests/repo.rs::repo_margin_call_enforces_cadence_and_participant_rules` guarantees the runtime rejects premature calls and non-participant submissions. |
| **Collateral substitution & maturity unwind** | Governance change record cites the required `collateral_substitution_matrix` entries and haircut policy; council minutes list the substitution pair SHA-256 hash. | Stage the unwind leg with `iroha app repo unwind --output ... --settlement-timestamp-ms <planned>` so both the ACT/360 calculation (§2.8) and substitution payload are reproducible.<br>Include the `[settlement.repo]` TOML snippet, substitution manifest, and the resulting `RepoAccountEvent::Settled` payload in the artefact bundle. | The substitution round-trip inside `integration_tests/tests/repo.rs::repo_roundtrip_transfers_balances_and_clears_agreement` exercises insufficient-versus-approved substitution flows while keeping the agreement id constant. |
| **Emergency unwind / rollback drill** | Incident commander + finance council approve the rollback as described in `docs/source/finance/repo_runbook.md` (sections 4–5) and capture the entry in `ops/drill-log.md`. | Execute `iroha app repo unwind` using the staged rollback payload, append CLI logs + GAR reference to `governance/drills/<timestamp>.log`, and rerun both `repo_deterministic_lifecycle_proof_matches_fixture` and the `scripts/repo_evidence_manifest.py` helper to prove determinism before/after the drill. | The happy-path unwind is covered by `integration_tests/tests/repo.rs::repo_roundtrip_transfers_balances_and_clears_agreement`; following the drill steps keeps the governance artefacts aligned with the runtime guarantees exercised by that test. |

**Desk timeline.**

1. Copy the intake template, fill the metadata block (agreement id, GAR ticket,
   custodian, configuration hash), and create the evidence directory.
2. Stage every instruction (`initiate`, `margin-call`, `unwind`, substitution) in
   `--output` mode, hash the JSON, and log the approvals beside each hash.
3. Emit the lifecycle proof snapshot and manifest immediately after staging so
   governance reviewers can recompute the digest with the same repo fixtures.
4. Mirror `RepoAccountEvent::*` SSE payloads for the affected accounts and drop
   the exported NDJSON in `artifacts/finance/repo/<agreement-id>/events.ndjson`
   before filing the packet.
5. Once the vote passes, update `hashes.txt` with the GAR identifier,
   configuration hash, and manifest checksum so the council can trace the rollout
   without re-running local scripts.

### 3.5 Governance packet quickstart

Roadmap F1 reviewers asked for a concise checklist they can reference while
assembling an evidence bundle. Follow the sequence below whenever a repo request
or policy change is heading to governance:

1. **Export the lifecycle proof artefacts.**
   ```bash
   mkdir -p artifacts/finance/repo/<slug>
   REPO_PROOF_SNAPSHOT_OUT=artifacts/finance/repo/<slug>/repo_proof_snapshot.json \
   REPO_PROOF_DIGEST_OUT=artifacts/finance/repo/<slug>/repo_proof_digest.txt \
   cargo test -p iroha_core \
     -- --exact smartcontracts::isi::repo::tests::repo_deterministic_lifecycle_proof_matches_fixture
   ```
   The exported JSON + digest mirror the checked-in fixtures under
   `crates/iroha_core/tests/fixtures/`, so reviewers can recompute the lifecycle
   frame without re-running the entire suite (see §2.7). You can also call
   `scripts/regen_repo_proof_fixture.sh --bundle-dir artifacts/finance/repo/<slug>`
   to refresh and copy the same files in one step.
2. **Stage and hash every instruction.** Generate initiation/margin/unwind
   payloads with `iroha app repo ... --output`. Capture the SHA-256 for each file
   (store under `hashes/`) so `docs/examples/finance/repo_governance_packet_template.md`
   can reference the same bytes the desks reviewed.
3. **Save ledger/config snapshots.** Export `repo query list` output before/after
   settlement, dump the `[settlement.repo]` TOML block that will be applied, and
   mirror the relevant `AccountEvent::Repo(*)` SSE feed into
   `artifacts/finance/repo/<slug>/events/repo-events.ndjson`. After the GAR
   passes, capture `/v1/configuration` snapshots per peer (§2.9) and store them
   under `config/peers/` so the governance packet proves the rollout succeeded.
4. **Generate the evidence manifest.**
   ```bash
   python3 scripts/repo_evidence_manifest.py \
     --root artifacts/finance/repo/<slug> \
     --agreement-id <repo-id> \
     --output artifacts/finance/repo/<slug>/manifest.json
   ```
   Include the manifest hash inside the governance ticket or GAR minutes so
   auditors can diff the packet without downloading the raw bundle (see §3.2).
5. **Assemble the packet.** Copy the template from
   `docs/examples/finance/repo_governance_packet_template.md`, fill the metadata,
   attach the proof snapshot/digest, manifest, config hash, SSE export, and test
   logs, then cite the manifest SHA-256 inside the referendum `--notes` field.
   Store the completed Markdown next to the artefacts so rollbacks inherit the
   exact evidence you shipped for approval.

Running the steps above immediately after staging a repo request means the
governance packet is ready as soon as the council convenes, avoiding last-minute
scrambles to recreate hashes or event streams.

## 4. Tri-Party Custody & Collateral Substitution

- **Custodians:** Passing `--custodian <account>` routes collateral to the
  custodian vault; runtime enforces account existence and emits role-tagged
  events so custodians can reconcile (`RepoAccountRole::Custodian`). The state
  machine rejects agreements whose custodian matches either party.
- **Collateral substitution:** The unwind leg may deliver a different collateral
  quantity/series during substitution so long as it is **not less** than the
  pledged amount *and* the substitution matrix allows the pair; `ReverseRepoIsi`
  enforces both conditions
  (`crates/iroha_core/src/smartcontracts/isi/repo.rs:414`–`437`). The integration
  test suite exercises both the rejection path and a successful substitution
  roundtrip (`integration_tests/tests/repo.rs:261`–`359`), while the repo unit
  tests cover the new matrix policy.
- **ISO 20022 mapping:** When building ISO envelopes or reconciling external
  systems, reuse the field mapping documented in
  `docs/source/finance/settlement_iso_mapping.md` (`colr.007`, `sese.023`,
  `sese.025`) so the Norito payload and ISO confirmations stay in sync.

## 5. Operational Checklists

### Daily pre-open

1. Export the outstanding agreement set via `iroha app repo query list`.
2. Compare against treasury inventory and ensure eligible collateral config
   matches the planned book.
3. Stage upcoming repos/unwinds with `--output` and collect dual approvals.

### Intraday monitoring

1. Subscribe to `AccountEvent::Repo` for initiator/counterparty/custodian
   accounts; alert when unexpected initiations occur.
2. Use `iroha app repo margin --agreement-id ID` (or
   `RepoAgreementRecord::next_margin_check_after`) hourly to detect cadence
   drift; trigger `repo margin-call` when `is_due = true`.
3. Log all margin calls with operator initials and attach the CLI JSON output to
   the evidence directory.

### End-of-day + post-settlement

1. Re-run `repo query list` and confirm unwound agreements have been removed.
2. Archive the `RepoAccountEvent::Settled` payloads and cross-check cash/collateral
   balances via `FindAssets`.
3. File a drill entry in `ops/drill-log.md` when repo drills or incident tests
   run; reuse `scripts/telemetry/log_sorafs_drill.sh` conventions for timestamps.

## 6. Fraud & Rollback Procedures

- **Dual control:** Always generate instructions with `--output` and store the
  JSON for co-signing. Reject single-party submissions at the process level
  even though the runtime enforces initiator authority.
- **Tamper-evident logging:** Mirror the `RepoAccountEvent` stream into your
  SIEM so any forged instruction would be detectable (missing peer signatures).
- **Rollback:** If a repo must be unwound prematurely, submit `repo unwind`
  with the same agreement id and attach the `--notes` field in your incident
  tracker referencing the GAR-approved rollback playbook.
- **Fraud escalation:** If unauthorised repos appear, export the offending
  `RepoAccountEvent` payloads, freeze the accounts via governance policy, and
  notify the council per the repo governance SOP.

## 7. Reporting & Follow-Up

### 7.1 Treasury reconciliation & ledger evidence

Roadmap **F1** and the global settlement guardrail (roadmap.md#L1975-L1978)
require every repo review to include deterministic treasury proofs. Produce a
quarterly bundle per book by following the checklist below.

1. **Snapshot balances.** Use the `FindAssets` query that powers
   `iroha ledger asset list` (`crates/iroha_cli/src/main_shared.rs`) or the
   `iroha_python` helper to export XOR balances for `i105...`,
   `i105...`, and every desk account involved in the review. Store
   the JSON under
   `artifacts/finance/repo/<period>/treasury_assets.json` and record the git
   commit/toolchain in the accompanying `README.md`.
2. **Cross-check ledger projections.** Re-run
   `sorafs reserve ledger --quote <...> --json-out ...` and normalise the output
   via `scripts/telemetry/reserve_ledger_digest.py`. Place the digest next to
   the asset snapshot so auditors can diff the XOR totals against the repo
   ledger projection without replaying the CLI.
3. **Publish the reconciliation note.** Summarise deltas in
   `artifacts/finance/repo/<period>/treasury_reconciliation.md` by referencing:
   the asset snapshot hash, the ledger digest hash, and the agreements covered.
   Link the note from the finance governance tracker so reviewers can confirm
   treasury coverage before approving the repo release.

### 7.2 Drill & rollback rehearsal evidence

The acceptance criteria also demand staged rollbacks and incident drills. Every
drill or chaos rehearsal must collect the following artefacts:

1. `repo_runbook.md` Sections 4–5 checklist signed by the incident commander and
   finance council.
2. CLI/SDK logs for the rehearsal (`repo initiate|margin-call|unwind`) plus the
   refreshed lifecycle proof snapshot and evidence manifest (§§2.7–3.2) stored
   under `artifacts/finance/repo/drills/<timestamp>/`.
3. Alertmanager or pager transcripts showing the injected signals and the
   acknowledgement trail. Drop the transcript next to the drill artefacts and
   include the Alertmanager silence ID when one was used.
4. An `ops/drill-log.md` entry referencing the GAR id, manifest hash, and drill
   bundle path so future audits can trace rehearsals without scraping chat logs.

### 7.3 Governance tracker & doc hygiene

- Keep this document, `repo_runbook.md`, and the finance governance tracker in
  lockstep whenever CLI/SDK or runtime behaviour changes; reviewers expect the
  acceptance table to stay accurate.
- Attach the full evidence bundle (`agreements.json`, staged instructions, SSE
  transcripts, config snapshot, reconciliations, drill artefacts, and test
  logs) to the tracker for every quarterly review.
- Reference `docs/source/finance/settlement_iso_mapping.md` when coordinating
  with ISO bridge operators so cross-system reconciliation remains aligned.

By following this guide, operators satisfy the roadmap F1 acceptance bar:
deterministic proofs are captured, tri-party and substitution flows are
documented, and governance procedures (dual-control + incident logging) are
codified in-tree.
