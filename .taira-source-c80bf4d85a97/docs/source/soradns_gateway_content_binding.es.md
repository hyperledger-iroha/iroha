---
lang: es
direction: ltr
source: docs/source/soradns_gateway_content_binding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 27d1a87a26e0dbcb6253953c9c8e3a57959daf5589f0671dbca2154512ee9cf8
source_last_modified: "2026-01-03T18:08:01.696079+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraDNS Gateway Content Binding Checklist
summary: Step-by-step workflow for DG-3 to bind SoraFS releases to registered SoraDNS names with GAR evidence and Sora-* headers.
---

# SoraDNS Gateway Content Binding Checklist (DG-3)

Status: Completed 2025-11-20  
Owners: Networking TL, Docs/DevRel, Storage/Ops  
Roadmap reference: **DG-3 — Gateway Content Binding & TLS Automation**

This runbook stitches together the CLI helpers and artefact formats introduced
for DG-3 so the docs portal (and any other SoraDNS property such as
`docs.sora`) can be tied to a gateway manifest with deterministic evidence. It
is meant to be read alongside:

- `docs/source/soradns/deterministic_hosts.md` (host derivation policy)
- `docs/source/sorafs_gateway_tls_automation.md` (TLS/ECH automation)
- `docs/source/sorafs_gateway_dns_owner_runbook.md` (cutover cadence)

## 1. Prerequisites

| Requirement | Notes |
|-------------|-------|
| Cargo workspace + `xtask` helpers | Needed for `soradns-hosts`, `soradns-gar-template`, and `sorafs-gateway-probe`. |
| Signed SoraFS manifest bundle | Produced by `docs/portal/scripts/sorafs-pin-release.sh` (see `docs/portal/docs/devportal/deploy-guide.md`). |
| Governance ticket & alias approval | Ticket must list the registered FQDN, GAR version, and DNS cutover window. |
| Access to GAR signing key + Sigstore tooling | The GAR template emitted below becomes the payload for the signature workflow. |
| Probe target | Either a staging gateway (`https://docs.sora.link`) or captured headers under `artifacts/sorafs_gateway_probe/`. |

## 2. Workflow

### Step 0 — Render the ACME SAN/challenge plan

Run the TLS helper before touching GAR files so the canonical wildcard/pretty
host SAN set and DNS-01 labels are captured once and reused by the automation
controller:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/soradns/docs.sora.acme_plan.json
```

Attach the JSON to the DG-3 change ticket and mirror it into the sealed
`sorafs_gateway_tls/` secret store referenced by the TLS automation guide. The
plan records the ACME directory URL, recommended DNS-01 vs TLS-ALPN-01
challenges, and `_acme-challenge` labels so cert orders and GAR host patterns
cannot diverge.

### Step 1 — Derive canonical and pretty hosts

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/soradns/host_summary.docs.sora.json \
  --verify-host-patterns docs/examples/soradns_host_patterns_docs_sora.json
```

The helper normalises the name, prints the canonical Base32 label, and ensures
the provided GAR host-pattern JSON authorises:

1. `b6ukbnpp2tthm3e7fmaxqll2aq.gw.sora.id`
2. `*.gw.sora.id`
3. `docs.sora.gw.sora.name`

Example verification payloads live under
`docs/examples/soradns_host_patterns_docs_sora.json` so onboarding teams can
exercise the CLI without writing bespoke JSON.

### Step 2 — Generate the GAR template

```bash
cargo xtask soradns-gar-template \
  --name docs.sora \
  --manifest artifacts/sorafs/portal.manifest.json \
  --manifest-digest 8A2A332D5E52EDC13ED088B79B6B2940AF0B31C7F7FBB9324A88ACDF1A0AF07D \
  --telemetry-label dg-3 \
  --valid-from 1735771200 \
  --valid-until 1767307200 \
  --json-out docs/examples/soradns_gar_docs_sora.json
```

The emitted template includes canonical/pretty/wildcard host patterns, default
CSP/HSTS/Permissions-Policy templates, an empty
`license_sets`/`moderation_directives` scaffold, and the validity window.
Operators can tweak any fields before signing, but
the runbook expects the canonical host list to remain untouched. Attach the
JSON to the governance ticket together with the GAR signature artefacts.

### Step 2.5 — Validate the GAR payload

Before signing or distributing the GAR JSON, run the verification helper to
ensure host patterns, manifest metadata, and telemetry labels match the
deterministic policy:

```bash
cargo xtask soradns-verify-gar \
  --gar artifacts/soradns/docs.sora.gar.json \
  --name docs.sora \
  --manifest-cid bafybeigdyrzt2vx7demoexamplecid \
  --manifest-digest 8A2A332D5E52EDC13ED088B79B6B2940AF0B31C7F7FBB9324A88ACDF1A0AF07D \
  --telemetry-label dg-3 \
  --json-out artifacts/soradns/docs.sora.gar.summary.json
```

The command fails fast when canonical or pretty hosts are missing, when the
manifest metadata drifts, or when required telemetry labels are absent. Store
the JSON summary next to the GAR artefact so governance can reference the
verification evidence without replaying CLI output.

### Step 3 — Pin the portal release and capture binding metadata

Run the packaging script documented in the portal deploy guide; the example
below shows the minimum flags for a dry run:

```bash
npm ci && npm run build
./docs/portal/scripts/sorafs-pin-release.sh \
  --alias-hint docs.sora.link \
  --dns-change-ticket OPS-XXXX \
  --dns-cutover-window 2025-03-03T16:00Z/2025-03-03T16:30Z \
  --gateway-manifest-envelope artifacts/sorafs/portal.gateway.binding.json \
  --metadata alias_label=docs.sora.link
```

To regenerate the binding outside the portal release script (or to diff changes
locally), call the new xtask helper directly:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Use `--csp-template`, `--permissions-template`, or `--hsts-template` to override
the default header strings for deployments that need bespoke CSP/HSTS/Permissions
language. Supply `--no-csp`, `--no-permissions-policy`, or `--no-hsts` to omit
the headers entirely when governance approves the deviation.

The script emits:

- `portal.pin.report.json` & checksums
- `portal.gateway.binding.json` containing `Sora-Route-Binding`/`Sora-Proof-*`
  templates keyed by the manifest CID
- `portal.dns-cutover.json` describing the alias, cache purge, and probe
  commands

Archive all artefacts in `artifacts/sorafs/` and link them from the change
ticket.

### Step 3.5 — Lint the binding headers

Run the binding verifier to ensure the stapled headers match the alias and
manifest metadata before sharing templates with gateway operators:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --manifest-json artifacts/sorafs/portal.manifest.json \
  --proof-status ok
```

The helper validates `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`/`Status`, and
the rendered `Sora-Route-Binding` while re-deriving the manifest CID from the
JSON. Keep the verification log with the binding artefacts so reviewers can
audit the header block without re-running the script.

### Step 4 — Draft the promotion & rollback plan

`cargo xtask sorafs-gateway route plan` converts the manifest + binding outputs
from step 3 into an auditable promotion packet that change managers and the
resolver ops team can review before cutover. The helper emits both the intended
route headers and an optional rollback block so operators do not have to
reconstruct the previous manifest under pressure.

```bash
cargo xtask sorafs-gateway route plan \
  --manifest-json artifacts/sorafs/portal.manifest.json \
  --hostname docs.sora.link \
  --alias docs.sora \
  --route-label production \
  --release-tag docs-20250303T1600Z \
  --cutover-window 2025-03-03T16:00Z/2025-03-03T16:30Z \
  --rollback-manifest-json artifacts/sorafs/portal.manifest.prev.json \
  --rollback-route-label previous \
  --rollback-release-tag docs-rollback-20250303 \
  --out artifacts/sorafs_gateway/route_plan.json
```

Key outputs:

- `route_plan.json` – Records the manifest path, alias, hostname, route label,
  cutover window, rendered `Sora-Route-Binding`, and the header templates that
  must ship with the GAR update. The optional `rollback` section mirrors the
  previous manifest/label combination so incident leads can agree on the exact
  rollback payload.
- `gateway.route.headers.txt` – Ready-to-paste header block for the promotion.
- `gateway.route.rollback.headers.txt` – Header block for the fallback route
  whenever `--rollback-manifest-json` is supplied.

Attach the JSON plan and both header files to the DG-3 change ticket so the
approvers can validate the promotion/rollback flow without running local
scripts. You can also point the probe helper at these headers for a dry run:

```bash
cargo xtask sorafs-gateway-probe \
  --headers-file artifacts/sorafs_gateway/gateway.route.headers.txt \
  --gar artifacts/soradns/docs.sora.gar.jws \
  --gar-key docs-gateway=public_key_hex
```

Generate the supporting DG-3 plans in parallel so change tickets cover cache
purges and per-alias action lists:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --json-out artifacts/sorafs_gateway/portal.cache_plan.json

cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs_gateway/portal.route_plan.json
```

The cache plan lists canonical/pretty hosts and PURGE paths (plus optional auth
hints); the route plan captures the promote/cutover/observe/rollback steps for
each alias so change-control reviewers can diff planned actions without running
local scripts.

### Step 5 — Verify headers & GAR enforcement

Use the probe helper to validate Sora-* headers, GAR metadata, TLS/ECH, and
route binding prior to cutover:

```bash
cargo xtask sorafs-gateway-probe \
  --url https://docs.sora.link \
  --expect-alias docs.sora \
  --expect-manifest bafybeigdyrzt2vx7demoexamplecid \
  --out artifacts/sorafs_gateway_probe/docs.sora.latest.json
```

The probe (or `ci/check_sorafs_gateway_probe.sh` in CI) must confirm:

| Header | Purpose |
|--------|---------|
| `Sora-Name` | Registered alias bound to the response. |
| `Sora-Content-CID` | BLAKE3-based manifest root pulled from the SoraFS bundle. |
| `Sora-Proof` / `Sora-Proof-Status` | Signed CAR proof and freshness indicator. |
| `Sora-Route-Binding` | `host`, `cid`, `generated_at`, and `label` metadata linking the GAR template to the manifest. |
| `Sora-PoTR-*` (when enabled) | Latency receipt for DA/PoTR evidence. |

Capture the rendered headers (`gateway.route.headers.txt`) and keep them in the
cutover artefact bucket.

### Step 6 — Publish DNS + GAR updates

1. Update the zonefile skeleton per
   `docs/source/sorafs_gateway_dns_owner_runbook.md`.
2. Attach the signed GAR template from Step 2 to the change ticket.
3. Feed the new host bindings into the resolver bundle (see
   `tools/soradns-resolver/` README).
4. Schedule cutover drills with
   `scripts/telemetry/schedule_soradns_ir_drill.sh`.
5. During cutover, re-run the probe and stash the JSON under
   `artifacts/sorafs_gateway_probe/<stamp>/`.

## 3. Artefact Reference

| File | Description |
|------|-------------|
| `docs/examples/soradns_host_patterns_docs_sora.json` | Canonical GAR host-pattern verification payload referenced by `soradns-hosts`. |
| `docs/examples/soradns_gar_docs_sora.json` | Sample GAR template emitted by `soradns-gar-template` (pre-signature). |
| `artifacts/soradns/host_summary.docs.sora.json` | Derived host summary captured per release (not checked into Git). |
| `artifacts/sorafs/portal.gateway.binding.json` | Manifest → Sora-* header template used by the gateway. |
| `artifacts/sorafs_gateway_probe/*.json` | Probe outputs proving GAR headers/TLS/ECH enforcement. |

## 4. Automation Hooks

- Add `cargo xtask soradns-hosts --verify-host-patterns` and
  `cargo xtask soradns-gar-template --json-out` to the docs portal release
  workflow (`ci/check_docs_portal.sh`) so GAR drift blocks CI.
- Run `cargo xtask soradns-verify-gar` and `cargo xtask soradns-verify-binding`
  in the same workflow to pin GAR and header templates before artefacts are
  published; retain `cargo xtask soradns-route-plan` output as the promotion
  checklist for CI linting.
- Mirror `cargo xtask soradns-cache-plan` output into the change packet so cache
  purges always reference the canonical host/path set and auth hints.
- Wire `ci/check_sorafs_gateway_probe.sh` into the same workflow to ensure the
  manifest, headers, and GAR metadata line up before artefacts are uploaded to
  SoraFS.
- Mirror the JSON artefacts produced in steps 1–4 to SoraFS so governance can
  verify DG-3 evidence bundles directly from the ledger.
