---
lang: zh-hans
direction: ltr
source: docs/source/sorafs/capacity_onboarding_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d31fb4cb8eb1cdfa7859e53e8fc4d7264bbcc7012efb201c00b91e5e52efcbd7
source_last_modified: "2026-01-22T14:35:37.725544+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Capacity Onboarding & Exit Runbook
summary: Step-by-step workflow for staging provider declarations and retirements required by roadmap item SF-2c.
---

# SoraFS Capacity Onboarding & Exit Runbook

Roadmap item **SF-2c** asks every provider onboarding or exit request to ship a
repeatable packet of artefacts so governance, treasury, and SRE reviewers can
trace the decision. This runbook codifies the workflow referenced by
`roadmap.md` and the capacity marketplace validation checklist
(`docs/source/sorafs/reports/capacity_marketplace_validation.md`). Follow it for
every new provider admission, renewal, or retirement.

## Roles & Pre-flight Requirements

| Role | Responsibilities | Required artefacts |
|------|------------------|--------------------|
| Provider | Draft `CapacityDeclarationV1` spec, capture hardware inventory, supply attestation bundles if mandated by `docs/source/sorafs/provider_admission_policy.md`. | `specs/<provider>.json`, hardware affidavit, contact roll. |
| Storage WG reviewer | Validate schema, run CLI regression, cross-check stake and chunker profile metadata. | CLI logs, `sorafs_manifest_stub` outputs, signed review note. |
| Governance council | Approve final declaration, record vote ID, and track override/penalty hooks. | Council ballot ID, incident log link. |
| Treasury/SRE | Confirm dashboards and quota entries, archive reconciliation/export hashes, verify alerts. | Grafana screenshots, `/v2/sorafs/capacity/state` dump, Alertmanager silence log if used. |

Pre-flight checklist:

1. Provider appears on the vetted list from SF-2b admission (else reject).
2. Required keys exist in the provider registry (`iroha_torii` →
   `/v2/sorafs/providers`).
3. Provider advertises the chunker profiles referenced in the declaration.
4. Treasury confirms a reserve account exists for the expected staking tier.

## Onboarding Workflow

### 1. Prepare the declaration bundle

Create a human-readable spec that mirrors the canonical schema (see
`docs/examples/sorafs_capacity_declaration.spec.json`). Each spec must include a
deterministic `provider_id_hex`, `chunker_capabilities`, stake pointer, per-lane
caps, and SLA contact information. Generate the canonical payloads via the CLI
helper:

```bash
mkdir -p artifacts/sorafs/providers/acme
sorafs_manifest_stub capacity declaration \
  --spec specs/providers/acme.json \
  --json-out artifacts/sorafs/providers/acme/declaration.json \
  --request-out artifacts/sorafs/providers/acme/request.json \
  --norito-out artifacts/sorafs/providers/acme/declaration.to \
  --base64-out artifacts/sorafs/providers/acme/declaration.b64
```

This command validates the schema locally and emits every artefact required by
`/v2/sorafs/capacity/declare`. Store the stdout/stderr in
`artifacts/sorafs/providers/acme/manifest_stub.log`.

### 2. Run admission smoke tests

Before sending the payload to Torii, re-run the CLI regression to prove the
validator is deterministic:

```bash
cargo test -p sorafs_car --test capacity_cli -- capacity_declaration
```

Attach the resulting `test-stdout.txt` to the onboarding packet. Reviewers use
the log to confirm the template exercises the same code paths that Torii
invokes.

### 3. Submit to Torii and capture responses

Submit the request JSON through Torii’s app API. Either call it manually:

```bash
TORII="https://torii.example.net"
curl -sS -X POST "$TORII/v2/sorafs/capacity/declare" \
  -H 'Content-Type: application/json' \
  --data-binary @artifacts/sorafs/providers/acme/request.json \
  | tee artifacts/sorafs/providers/acme/declare_response.json
```

…or wrap the request by embedding it in a governance transaction if the
provider must wait for a council vote. Capture the HTTP status, Torii log
snippet, and app API correlation ID; governance reviewers expect those in the
release ticket.

### 4. Verify registry state and publish evidence

After Torii accepts the declaration, export a state snapshot and the
governance-friendly JSON by calling:

```bash
curl -sS "$TORII/v2/sorafs/capacity/state" \
  | jq '.providers[] | select(.provider_id_hex=="0xACME...")' \
  > artifacts/sorafs/providers/acme/state_record.json
```

Confirm that `capacity_gib`, `chunker_profiles[*].handle`, and `stake_pointer`
match the original spec. Cross-check the provider’s entry on the
`sorafs_capacity_health` Grafana board (`dashboards/grafana/sorafs_capacity_health.json`).
Take a screenshot with the timestamp visible and archive it alongside the JSON
dump so SRE can prove telemetry coverage.

### 5. Governance & treasury hand-off

1. Governance creates a ballot referencing the payload hash and logs the vote
   outcome in `docs/examples/sorafs_capacity_marketplace_validation/<date>_onboarding_signoff.md`.
2. Treasury annotates the nightly reconciliation output produced by
   `scripts/telemetry/capacity_reconcile.py --snapshot … --ledger …` with the provider ID
   so payout reviewers can trace the first rent accrual and Alertmanager gating.
3. Support/SRE note the onboarding in `docs/source/sorafs/ops_log.md`, linking
   to the packet path and Grafana capture.

## Exit / Retirement Workflow

1. **Drain assignments.** Run `iroha app sorafs replication list --status active \
   --provider-id <hex>` until no manifest references remain. Queue reassignment
   ballots for any stragglers before approving retirement.
2. **Publish final telemetry.** Use `sorafs_manifest_stub capacity telemetry` to
   generate the final snapshot and submit it via
   `POST /v2/sorafs/capacity/telemetry`. Record the response and ensure the
   provider’s row in `/v2/sorafs/capacity/state` moves to `status="retiring"` or
   `"inactive"`.
3. **Revoke access.** Remove the provider’s credentials from advert
   distribution, revoke OAuth/API tokens, and rotate any shared secret material.
4. **Archive artefacts.** Store the telemetry request/response, final state
   snapshot, Grafana screenshot, and the strike/approval IDs inside
   `docs/examples/sorafs_capacity_marketplace_validation/<date>_exit_signoff.md`.

Running the exit checklist once per release candidate satisfies the smoke test
requirement in `roadmap.md` and the GA acceptance criteria in
`docs/source/sorafs/storage_capacity_marketplace.md`.

## Evidence & Storage Layout

Maintain the following structure (hashes captured via `b2sum`) for every
provider packet:

```
artifacts/sorafs/providers/<provider>/
  declaration.json
  declaration.to
  declaration.b64
  request.json
  declare_response.json
  state_record.json
  telemetry_final.json              # exit only
  grafana_capacity_health_<ts>.png
  manifest_stub.log
  tests/capacity_cli.log
docs/examples/sorafs_capacity_marketplace_validation/
  YYYY-MM-DD_<provider>_{onboarding,exit}_signoff.md
```

Signoff files include hashes for every artefact plus links to Alertmanager
silence entries or incident numbers. Treasury and governance rely on these
hashes when approving payouts or slashing requests.

## Role-Based Review Matrix

| Phase | Storage WG | Governance | Treasury/SRE |
|-------|------------|------------|---------------|
| Spec validation | Check schema, run CLI test, confirm chunker profiles | — | — |
| Torii submission | Monitor `/v2/sorafs/capacity/state`, update ops log | Record ballot ID, attach vote artefact | Watch Grafana and alert rules, capture reconciliation logs |
| Exit | Verify reassignment + telemetry, compile packet | Approve retirement & link dispute log | Confirm dashboards drop provider, archive payout audit |

Keep this matrix beside the release checklist; auditors expect to see the owner
names and timestamps filled in for each onboarding or exit wave.

_Last updated: 2026-02-11_
