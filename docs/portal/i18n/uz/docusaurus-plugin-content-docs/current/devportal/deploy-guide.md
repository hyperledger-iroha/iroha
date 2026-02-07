---
id: deploy-guide
lang: uz
direction: ltr
source: docs/portal/docs/devportal/deploy-guide.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Deployment Guide
sidebar_label: Deployment Guide
description: Promote the developer portal through the SoraFS pipeline with deterministic builds, Sigstore signing, and rollback drills.
---

## Overview

This playbook converts roadmap items **DOCS-7** (SoraFS publishing) and **DOCS-8**
(CI/CD pin automation) into an actionable procedure for the developer portal.
It covers the build/lint phase, SoraFS packaging, Sigstore-backed manifest
signing, alias promotion, verification, and rollback drills so every preview and
release artefact is reproducible and auditable.

The flow assumes you have the `sorafs_cli` binary (built with
`--features cli`), access to a Torii endpoint with pin-registry permissions, and
OIDC credentials for Sigstore. Store long-lived secrets (`IROHA_PRIVATE_KEY`,
`SIGSTORE_ID_TOKEN`, Torii tokens) in your CI vault; local runs can source them
from shell exports.

## Prerequisites

- Node 18.18+ with `npm` or `pnpm`.
- `sorafs_cli` from `cargo run -p sorafs_car --features cli --bin sorafs_cli`.
- Torii URL that exposes `/v1/sorafs/*` plus an authority account/private key
  that can submit manifests and aliases.
- OIDC issuer (GitHub Actions, GitLab, workload identity, etc.) to mint a
  `SIGSTORE_ID_TOKEN`.
- Optional: `examples/sorafs_cli_quickstart.sh` for dry runs and
  `docs/source/sorafs_ci_templates.md` for GitHub/GitLab workflow scaffolding.
- Configure the Try it OAuth variables (`DOCS_OAUTH_*`) and run the
  [security-hardening checklist](./security-hardening.md) before promoting a build
  outside the lab. The portal build now fails when these variables are missing
  or when the TTL/polling knobs fall outside the enforced windows; export
  `DOCS_OAUTH_ALLOW_INSECURE=1` only for disposable local previews. Attach the
  pen-test evidence to the release ticket.

## Step 0 — Capture a Try it proxy bundle

Before promoting a preview to Netlify or the gateway, stamp the Try it proxy
sources and signed OpenAPI manifest digest into a deterministic bundle:

```bash
cd docs/portal
npm run release:tryit-proxy -- \
  --out ../../artifacts/tryit-proxy/$(date -u +%Y%m%dT%H%M%SZ) \
  --target https://torii.dev.sora \
  --label preview-2026-02-14
```

`scripts/tryit-proxy-release.mjs` copies the proxy/probe/rollback helpers,
verifies the OpenAPI signature, and writes `release.json` plus
`checksums.sha256`. Attach this bundle to the Netlify/SoraFS gateway promotion
ticket so reviewers can replay the exact proxy sources and Torii target hints
without rebuilding. The bundle also records whether client-supplied bearers were
enabled (`allow_client_auth`) to keep the rollout plan and CSP rules in sync.

## Step 1 — Build and lint the portal

```bash
cd docs/portal
npm ci
npm run sync-openapi
npm run sync-norito-snippets
npm run test:norito-snippets
npm run test:widgets
npm run check:links
npm run build
```

`npm run build` automatically executes `scripts/write-checksums.mjs`, producing:

- `build/checksums.sha256` — SHA256 manifest suitable for `sha256sum -c`.
- `build/release.json` — metadata (`tag`, `generated_at`, `source`) pinned into
  every CAR/manifest.

Archive both files alongside the CAR summary so reviewers can diff preview
artefacts without rebuilding.

## Step 2 — Package the static assets

Run the CAR packer against the Docusaurus output directory. The example below
writes all artefacts under `artifacts/devportal/`.

```bash
OUT=artifacts/devportal
mkdir -p "$OUT"

sorafs_cli car pack \
  --input build \
  --car-out "$OUT"/portal.car \
  --plan-out "$OUT"/portal.plan.json \
  --summary-out "$OUT"/portal.car.json \
  --chunker-handle sorafs.sf1@1.0.0
```

The summary JSON captures chunk counts, digests, and proof-planning hints that
`manifest build` and CI dashboards reuse later.

## Step 2b — Package OpenAPI and SBOM companions

DOCS-7 requires publishing the portal site, OpenAPI snapshot, and SBOM payloads
as distinct manifests so gateways can staple `Sora-Proof`/`Sora-Content-CID`
headers for each artefact. The release helper
(`scripts/sorafs-pin-release.sh`) already packages the OpenAPI directory
(`static/openapi/`) and SBOMs emitted via `syft` into separate
`openapi.*`/`*-sbom.*` CARs and records the metadata in
`artifacts/sorafs/portal.additional_assets.json`. When running the manual flow,
repeat Steps 2–4 for each payload with its own prefixes and metadata labels
(for example `--car-out "$OUT"/openapi.car` plus
`--metadata alias_label=docs.sora.link/openapi`). Register every manifest/alias
pair in Torii (site, OpenAPI, portal SBOM, OpenAPI SBOM) before switching DNS so
the gateway can serve stapled proofs for all published artefacts.

## Step 3 — Build the manifest

```bash
sorafs_cli manifest build \
  --summary "$OUT"/portal.car.json \
  --manifest-out "$OUT"/portal.manifest.to \
  --manifest-json-out "$OUT"/portal.manifest.json \
  --pin-min-replicas 5 \
  --pin-storage-class warm \
  --pin-retention-epoch 14 \
  --metadata alias_label=docs.sora.link
```

Tune pin-policy flags to your release window (for example, `--pin-storage-class
hot` for canaries). The JSON variant is optional but convenient for code review.

## Step 4 — Sign with Sigstore

```bash
sorafs_cli manifest sign \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --bundle-out "$OUT"/portal.manifest.bundle.json \
  --signature-out "$OUT"/portal.manifest.sig \
  --identity-token-provider github-actions \
  --identity-token-audience sorafs-devportal
```

The bundle records the manifest digest, chunk digests, and a BLAKE3 hash of the
OIDC token without persisting the JWT. Keep both the bundle and detached
signature; production promotions can reuse the same artefacts instead of resigning.
Local runs can replace the provider flags with `--identity-token-env` (or set
`SIGSTORE_ID_TOKEN` in the environment) when an external OIDC helper issues the
token.

## Step 5 — Submit to the pin registry

Submit the signed manifest (and chunk plan) to Torii. Always request a summary
so the resulting registry entry/alias proof is auditable.

```bash
sorafs_cli manifest submit \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --torii-url "$TORII_URL" \
  --authority ih58... \
  --private-key "$IROHA_PRIVATE_KEY" \
  --submitted-epoch 20260101 \
  --alias-namespace docs \
  --alias-name sora.link \
  --alias-proof "$OUT"/docs.alias.proof \
  --summary-out "$OUT"/portal.submit.json \
  --response-out "$OUT"/portal.submit.response.json
```

When rolling out a preview or canary alias (`docs-preview.sora`), repeat the
submission with a unique alias so QA can verify content before production
promotion.

Alias binding requires three fields: `--alias-namespace`, `--alias-name`, and
`--alias-proof`. Governance produces the proof bundle (base64 or Norito bytes)
when the alias request is approved; store it in CI secrets and surface it as a
file before invoking `manifest submit`. Leave the alias flags unset when you
only intend to pin the manifest without touching DNS.

## Step 5b — Generate a governance proposal

Every manifest should travel with a Parliament-ready proposal so that any Sora
citizen can introduce the change without borrowing privileged credentials.
After the submit/sign steps, run:

```bash
sorafs_cli manifest proposal \
  --manifest "$OUT"/portal.manifest.to \
  --chunk-plan "$OUT"/portal.plan.json \
  --submitted-epoch 20260101 \
  --alias-hint docs.sora.link \
  --proposal-out "$OUT"/portal.pin.proposal.json
```

`portal.pin.proposal.json` captures the canonical `RegisterPinManifest`
instruction, chunk digest, policy, and alias hint. Attach it to the governance
ticket or Parliament portal so delegates can diff the payload without rebuilding
the artefacts. Because the command never touches the Torii authority key, any
citizen can draft the proposal locally.

## Step 6 — Verify proofs and telemetry

After pinning, run the deterministic verification steps:

```bash
sorafs_cli proof verify \
  --manifest "$OUT"/portal.manifest.to \
  --car "$OUT"/portal.car \
  --summary-out "$OUT"/portal.proof.json

sorafs_cli manifest verify-signature \
  --manifest "$OUT"/portal.manifest.to \
  --bundle "$OUT"/portal.manifest.bundle.json \
  --chunk-plan "$OUT"/portal.plan.json
```

- Check `torii_sorafs_gateway_refusals_total` and
  `torii_sorafs_replication_sla_total{outcome="missed"}` for anomalies.
- Run `npm run probe:portal` to exercise the Try-It proxy and recorded links
  against the newly pinned content.
- Capture the monitoring evidence described in
  [Publishing & Monitoring](./publishing-monitoring.md) so DOCS-3c’s
  observability gate is satisfied alongside the publishing steps. The helper
  now accepts multiple `bindings` entries (site, OpenAPI, portal SBOM, OpenAPI
  SBOM) and enforces `Sora-Name`/`Sora-Proof`/`Sora-Content-CID` on the target
  host via the optional `hostname` guard. The invocation below writes both a
  single JSON summary and the evidence bundle (`portal.json`, `tryit.json`,
  `binding.json`, and `checksums.sha256`) under the release directory:

  ```bash
  npm run monitor:publishing -- \
    --config ../../configs/docs_monitor.json \
    --json-out ../../artifacts/sorafs/preview-2026-02-14/monitoring/summary.json \
    --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
  ```

## Step 6a — Plan gateway certificates

Derive the TLS SAN/challenge plan before creating GAR packets so the gateway
team and DNS approvers review the same evidence. The new helper mirrors the
DG-3 automation inputs by enumerating canonical wildcard hosts,
pretty-host SANs, DNS-01 labels, and recommended ACME challenges:

```bash
cargo xtask soradns-acme-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.acme-plan.json
```

Commit the JSON alongside the release bundle (or upload it with the change
ticket) so operators can paste the SAN values into Torii’s
`torii.sorafs_gateway.acme` configuration and GAR reviewers can confirm the
canonical/pretty mappings without re-running host derivations. Add additional
`--name` arguments for each suffix promoted in the same release.

## Step 6b — Derive canonical host mappings

Before templating GAR payloads, record the deterministic host mapping for every
alias. `cargo xtask soradns-hosts` hashes each `--name` into its canonical
label (`<base32>.gw.sora.id`), emits the required wildcard
(`*.gw.sora.id`), and derives the pretty host (`<alias>.gw.sora.name`). Persist
the output in the release artefacts so DG-3 reviewers can diff the mapping
alongside the GAR submission:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json
```

Use `--verify-host-patterns <file>` to fail fast whenever a GAR or gateway
binding JSON omits one of the required hosts. The helper accepts multiple
verification files, making it easy to lint both the GAR template and the
stapled `portal.gateway.binding.json` in the same invocation:

```bash
cargo xtask soradns-hosts \
  --name docs.sora \
  --json-out artifacts/sorafs/portal.canonical-hosts.json \
  --verify-host-patterns artifacts/sorafs/portal.gar.json \
  --verify-host-patterns artifacts/sorafs/portal.gateway.binding.json
```

Attach the summary JSON and verification log to the DNS/gateway change ticket so
auditors can confirm the canonical, wildcard, and pretty hosts without re-running
the derivation scripts. Re-run the command whenever new aliases are added to the
bundle so subsequent GAR updates inherit the same evidence trail.

## Step 7 — Generate the DNS cutover descriptor

Production cutovers require an auditable change packet. After a successful
submission (alias binding), the helper emits
`artifacts/sorafs/portal.dns-cutover.json`, capturing:

- alias binding metadata (namespace/name/proof, manifest digest, Torii URL,
  submitted epoch, authority);
- release context (tag, alias label, manifest/CAR paths, chunk plan, Sigstore
  bundle);
- verification pointers (probe command, alias + Torii endpoint); and
- optional change-control fields (ticket id, cutover window, ops contact,
  production hostname/zone);
- route promotion metadata derived from the stapled `Sora-Route-Binding`
  header (canonical host/CID, header + binding paths, verification commands),
  ensuring GAR promotion and fallback drills refer to the same evidence;
- the generated route-plan artefacts (`gateway.route_plan.json`,
  header templates, and optional rollback headers) so change tickets and CI
  lint hooks can verify that every DG-3 packet references the canonical
  promotion/rollback plans before approval;
- optional cache invalidation metadata (purge endpoint, auth variable, JSON
  payload, and example `curl` command); and
- rollback hints pointing at the previous descriptor (release tag and manifest
  digest) so change tickets capture a deterministic fallback path.

When the release requires cache purges, generate a canonical plan alongside the
cutover descriptor:

```bash
cargo xtask soradns-cache-plan \
  --name docs.sora \
  --path / \
  --path /gateway/manifest.json \
  --auth-header Authorization \
  --auth-env CACHE_PURGE_TOKEN \
  --json-out artifacts/sorafs/portal.cache_plan.json
```

Attach the resulting `portal.cache_plan.json` to the DG-3 packet so operators
have deterministic hosts/paths (and the matching auth hints) when issuing
`PURGE` requests. The descriptor’s optional cache metadata section can reference
this file directly, keeping change-control reviewers aligned on exactly which
endpoints are flushed during a cutover.

Every DG-3 packet also needs a promotion + rollback checklist. Generate it via
`cargo xtask soradns-route-plan` so change-control reviewers can trace the exact
preflight, cutover, and rollback steps per alias:

```bash
cargo xtask soradns-route-plan \
  --name docs.sora \
  --json-out artifacts/sorafs/gateway.route_plan.json
```

The emitted `gateway.route_plan.json` captures canonical/pretty hosts, staged
health-check reminders, GAR binding updates, cache purges, and rollback actions.
Bundle it with the GAR/binding/cutover artefacts before submitting the change
ticket so Ops can rehearse and sign off on the same scripted steps.

`scripts/generate-dns-cutover-plan.mjs` powers this descriptor and runs
automatically from `sorafs-pin-release.sh`. To regenerate or customize it
manually:

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --pin-report artifacts/sorafs/portal.pin.report.json \
  --out artifacts/sorafs/portal.dns-cutover.json \
  --change-ticket OPS-4821 \
  --dns-hostname docs.sora.link \
  --dns-zone sora.link \
  --ops-contact docs-oncall@sora.link \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --previous-dns-plan artifacts/sorafs/previous.dns-cutover.json
```

Populate the optional metadata via environment variables before running the pin
helper:

| Variable | Purpose |
|----------|---------|
| `DNS_CHANGE_TICKET` | Ticket ID stored in the descriptor. |
| `DNS_CUTOVER_WINDOW` | ISO8601 cutover window (e.g., `2026-03-21T15:00Z/2026-03-21T15:30Z`). |
| `DNS_HOSTNAME`, `DNS_ZONE` | Production hostname + authoritative zone. |
| `DNS_OPS_CONTACT` | On-call alias or escalation contact. |
| `DNS_CACHE_PURGE_ENDPOINT` | Cache purge endpoint recorded in the descriptor. |
| `DNS_CACHE_PURGE_AUTH_ENV` | Env var containing the purge token (defaults to `CACHE_PURGE_TOKEN`). |
| `DNS_PREVIOUS_PLAN` | Path to the prior cutover descriptor for rollback metadata. |

Attach the JSON to the DNS change review so approvers can verify manifest
digests, alias bindings, and probe commands without scraping CI logs.
CLI flags `--dns-change-ticket`, `--dns-cutover-window`, `--dns-hostname`,
`--dns-zone`, `--ops-contact`, `--cache-purge-endpoint`,
`--cache-purge-auth-env`, and `--previous-dns-plan` provide the same overrides
when running the helper outside CI.

## Step 8 — Emit the resolver zonefile skeleton (optional)

When the production cutover window is known, the release script can emit the
SNS zonefile skeleton and resolver snippet automatically. Pass the desired DNS
records and metadata via either environment variables or CLI options; the helper
will call `scripts/sns_zonefile_skeleton.py` immediately after the cutover
descriptor is generated. Provide at least one A/AAAA/CNAME value and the GAR
digest (BLAKE3-256 of the signed GAR payload). If the zone/hostname are known
and `--dns-zonefile-out` is omitted, the helper writes to
`artifacts/sns/zonefiles/<zone>/<hostname>.json` and populates
`ops/soradns/static_zones.<hostname>.json` as the resolver snippet.

| Variable / flag | Purpose |
|-----------------|---------|
| `DNS_ZONEFILE_OUT`, `--dns-zonefile-out` | Path for the generated zonefile skeleton. |
| `DNS_ZONEFILE_RESOLVER_SNIPPET`, `--dns-zonefile-resolver-snippet` | Resolver snippet path (defaults to `ops/soradns/static_zones.<hostname>.json` when omitted). |
| `DNS_ZONEFILE_TTL`, `--dns-zonefile-ttl` | TTL applied to generated records (default: 600 seconds). |
| `DNS_ZONEFILE_IPV4`, `--dns-zonefile-ipv4` | IPv4 addresses (comma-separated env or repeatable CLI flag). |
| `DNS_ZONEFILE_IPV6`, `--dns-zonefile-ipv6` | IPv6 addresses. |
| `DNS_ZONEFILE_CNAME`, `--dns-zonefile-cname` | Optional CNAME target. |
| `DNS_ZONEFILE_SPKI`, `--dns-zonefile-spki-pin` | SHA-256 SPKI pins (base64). |
| `DNS_ZONEFILE_TXT`, `--dns-zonefile-txt` | Additional TXT entries (`key=value`). |
| `DNS_ZONEFILE_VERSION`, `--dns-zonefile-version` | Override the computed zonefile version label. |
| `DNS_ZONEFILE_EFFECTIVE_AT`, `--dns-zonefile-effective-at` | Force the `effective_at` timestamp (RFC3339) instead of the cutover window start. |
| `DNS_ZONEFILE_PROOF`, `--dns-zonefile-proof` | Override the proof literal recorded in the metadata. |
| `DNS_ZONEFILE_CID`, `--dns-zonefile-cid` | Override the CID recorded in the metadata. |
| `DNS_ZONEFILE_FREEZE_STATE`, `--dns-zonefile-freeze-state` | Guardian freeze state (soft, hard, thawing, monitoring, emergency). |
| `DNS_ZONEFILE_FREEZE_TICKET`, `--dns-zonefile-freeze-ticket` | Guardian/council ticket reference for freezes. |
| `DNS_ZONEFILE_FREEZE_EXPIRES_AT`, `--dns-zonefile-freeze-expires-at` | RFC3339 timestamp for thawing. |
| `DNS_ZONEFILE_FREEZE_NOTES`, `--dns-zonefile-freeze-note` | Additional freeze notes (comma-separated env or repeatable flag). |
| `DNS_GAR_DIGEST`, `--dns-gar-digest` | BLAKE3-256 digest (hex) of the signed GAR payload. Required whenever gateway bindings are present. |

The GitHub Actions workflow reads these values from repository secrets so every production pin emits the zonefile artefacts automatically. Configure the following secrets (strings may contain comma-separated lists for multi-value fields):

| Secret | Purpose |
|--------|---------|
| `DOCS_SORAFS_DNS_HOSTNAME`, `DOCS_SORAFS_DNS_ZONE` | Production hostname/zone passed to the helper. |
| `DOCS_SORAFS_DNS_OPS_CONTACT` | On-call alias stored in the descriptor. |
| `DOCS_SORAFS_ZONEFILE_IPV4`, `DOCS_SORAFS_ZONEFILE_IPV6` | IPv4/IPv6 records to publish. |
| `DOCS_SORAFS_ZONEFILE_CNAME` | Optional CNAME target. |
| `DOCS_SORAFS_ZONEFILE_SPKI` | Base64 SPKI pins. |
| `DOCS_SORAFS_ZONEFILE_TXT` | Additional TXT entries. |
| `DOCS_SORAFS_ZONEFILE_FREEZE_STATE/TICKET/EXPIRES_AT/NOTES` | Freeze metadata recorded in the skeleton. |
| `DOCS_SORAFS_GAR_DIGEST` | Hex-encoded BLAKE3 digest of the signed GAR payload. |

When triggering `.github/workflows/docs-portal-sorafs-pin.yml`, provide the `dns_change_ticket` and `dns_cutover_window` inputs so the descriptor/zonefile inherit the correct change window metadata. Leave them blank only when running dry runs.

Typical invocation (matching the SN-7 owner runbook):

```bash
./docs/portal/scripts/sorafs-pin-release.sh \
  --dns-zonefile-out artifacts/sns/zonefiles/sora.link/20250303.docs.sora.json \
  --dns-zonefile-resolver-snippet ops/soradns/static_zones.docs.json \
  --dns-zonefile-ipv4 198.51.100.4 \
  --dns-zonefile-ttl 600 \
  --dns-zonefile-freeze-state soft \
  --dns-zonefile-freeze-ticket SNS-DF-XXXX \
  --dns-zonefile-freeze-expires-at 2025-03-10T12:00Z \
  --dns-gar-digest <gar-digest-hex> \
  …other flags…
```

The helper automatically carries over the change ticket as a TXT entry and
inherits the cutover window start as the `effective_at` timestamp unless
overridden. For the full operational workflow, see
`docs/source/sorafs_gateway_dns_owner_runbook.md`.

### Public DNS delegation note

The zonefile skeleton only defines authoritative records for the zone. You
still need to configure parent-zone NS/DS delegation at your registrar or DNS
provider so the regular internet can discover the nameservers.

- For apex/TLD cutovers, use ALIAS/ANAME (provider-specific) or publish A/AAAA
  records pointing at the gateway anycast IPs.
- For subdomains, publish a CNAME to the derived pretty host
  (`<fqdn>.gw.sora.name`).
- The canonical host (`<hash>.gw.sora.id`) stays under the gateway domain and
  is not published inside your public zone.

### Gateway header template

The deploy helper also emits `portal.gateway.headers.txt` and
`portal.gateway.binding.json`, two artefacts that satisfy DG-3’s
gateway-content-binding requirement:

- `portal.gateway.headers.txt` contains the full HTTP header block (including
  `Sora-Name`, `Sora-Content-CID`, `Sora-Proof`, CSP, HSTS, and the
  `Sora-Route-Binding` descriptor) that edge gateways must staple onto every
  response.
- `portal.gateway.binding.json` records the same information in machine-readable
  form so change tickets and automation can diff host/cid bindings without
  scraping shell output.

They are generated automatically via
`cargo xtask soradns-binding-template`
and capture the alias, manifest digest, and gateway hostname that were supplied
to `sorafs-pin-release.sh`. To regenerate or customise the header block, run:

```bash
cargo xtask soradns-binding-template \
  --manifest artifacts/sorafs/portal.manifest.json \
  --alias docs.sora \
  --hostname docs.sora.link \
  --route-label production \
  --json-out artifacts/sorafs/portal.gateway.binding.json \
  --headers-out artifacts/sorafs/portal.gateway.headers.txt
```

Pass `--csp-template`, `--permissions-template`, or `--hsts-template` to override
the default header templates when a specific deployment needs additional
directives; combine them with the existing `--no-*` switches to drop a header
entirely.

Attach the header snippet to the CDN change request and feed the JSON document
into the gateway automation pipeline so the actual host promotion matches the
release evidence.

The release script runs the verification helper automatically so DG-3 tickets
always include recent evidence. Re-run it manually whenever you tweak the
binding JSON by hand:

```bash
cargo xtask soradns-verify-binding \
  --binding artifacts/sorafs/portal.gateway.binding.json \
  --alias docs.sora.link \
  --hostname docs.sora.link \
  --proof-status ok \
  --manifest-json artifacts/sorafs/portal.manifest.json
```

The command validates the `Sora-Proof` payload captured in the binding bundle,
ensures the `Sora-Route-Binding` metadata matches the manifest CID + hostname,
and fails fast if any header drifts. Archive the console output next to the
other deployment artefacts whenever you run the command outside CI so DG-3
reviewers have proof that the binding was validated prior to cutover.

> **DNS descriptor integration:** `portal.dns-cutover.json` now embeds a
> `gateway_binding` section pointing at these artefacts (paths, content CID,
> proof status, and the literal header template) **and** a `route_plan` stanza
> referencing `gateway.route_plan.json` plus the main + rollback header
> templates. Include those blocks in every DG-3 change ticket so reviewers can
> diff the exact `Sora-Name/Sora-Proof/CSP` values and confirm that the route
> promotion/rollback plans match the evidence bundle without opening the build
> archive.

## Step 9 — Run publishing monitors

Roadmap task **DOCS-3c** requires continuous evidence that the portal, Try it
proxy, and gateway bindings stay healthy after a release. Run the consolidated
monitor immediately after Steps 7–8 and wire it into your scheduled probes:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/sorafs/${RELEASE_TAG}/monitoring/summary-$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/${RELEASE_TAG}/monitoring
```

- `scripts/monitor-publishing.mjs` loads the config file (see
  `docs/portal/docs/devportal/publishing-monitoring.md` for the schema) and
  executes three checks: portal path probes + CSP/Permissions-Policy validation,
  Try it proxy probes (optionally hitting its `/metrics` endpoint), and the
  gateway binding verifier (`cargo xtask soradns-verify-binding`) which checks
  the captured binding bundle against the expected alias, host, proof status,
  and manifest JSON.
- The command exits non-zero whenever any probe fails so CI, cron jobs, or
  runbook operators can halt a release before promoting aliases.
- Passing `--json-out` writes a single summary JSON payload with per-target
  status; `--evidence-dir` emits `summary.json`, `portal.json`, `tryit.json`,
  `binding.json`, and `checksums.sha256` so governance reviewers can diff the
  results without re-running the monitors. Archive this directory under
  `artifacts/sorafs/<tag>/monitoring/` alongside the Sigstore bundle and DNS
  cutover descriptor.
- Include the monitor output, Grafana export (`dashboards/grafana/docs_portal.json`),
  and Alertmanager drill ID in the release ticket so the DOCS-3c SLO can be
  audited later. The dedicated publishing monitor playbook lives at
  `docs/portal/docs/devportal/publishing-monitoring.md`.

Portal probes require HTTPS and reject `http://` base URLs unless
`allowInsecureHttp` is set in the monitor config; keep production/staging
targets on TLS and only enable the override for local previews.

Automate the monitor via `npm run monitor:publishing` in Buildkite/cron once the
portal is live. The same command, pointed at production URLs, feeds the ongoing
health checks that SRE/Docs rely on between releases.

## Automating with `sorafs-pin-release.sh`

`docs/portal/scripts/sorafs-pin-release.sh` encapsulates Steps 2–6. It:

1. archives `build/` into a deterministic tarball,
2. runs `car pack`, `manifest build`, `manifest sign`, `manifest verify-signature`,
   and `proof verify`,
3. optionally executes `manifest submit` (including alias binding) when Torii
   credentials are present, and
4. writes `artifacts/sorafs/portal.pin.report.json`, the optional
  `portal.pin.proposal.json`, the DNS cutover descriptor (after submissions),
  and the gateway binding bundle (`portal.gateway.binding.json` plus the
  text header block) so governance, networking, and ops teams can diff the
  evidence bundle without scraping CI logs.

Set `PIN_ALIAS`, `PIN_ALIAS_NAMESPACE`, `PIN_ALIAS_NAME`, and (optionally)
`PIN_ALIAS_PROOF_PATH` before invoking the script. Use `--skip-submit` for dry
runs; the GitHub workflow described below toggles this via the `perform_submit`
input.

## Step 8 — Publish OpenAPI specs & SBOM bundles

DOCS-7 requires the portal build, OpenAPI spec, and SBOM artefacts to travel
through the same deterministic pipeline. The existing helpers cover all three:

1. **Regenerate & sign the spec.**

   ```bash
   npm run sync-openapi -- --version=2025-q3 --mirror=current --latest
   cargo xtask openapi --sign docs/portal/static/openapi/manifest.json
   ```

   Pass a release label via `--version=<label>` whenever you want to preserve a
   historical snapshot (for example `2025-q3`). The helper writes the snapshot
   to `static/openapi/versions/<label>/torii.json`, mirrors it into
   `versions/current`, and records the metadata (SHA-256, manifest status, and
   updated timestamp) in `static/openapi/versions.json`. The developer portal
   reads that index so the Swagger/RapiDoc panels can present a version picker
   and display the associated digest/signature info inline. Omitting
   `--version` keeps the previous release labels intact and only refreshes the
   `current` + `latest` pointers.

   The manifest captures SHA-256/BLAKE3 digests so the gateway can staple
   `Sora-Proof` headers for `/reference/torii-swagger`.

2. **Emit CycloneDX SBOMs.** The release pipeline already expects syft-based
   SBOMs per `docs/source/sorafs_release_pipeline_plan.md`. Keep the output
   next to the build artefacts:

   ```bash
   syft dir:build -o json > "$OUT"/portal.sbom.json
   syft file:docs/portal/static/openapi/torii.json -o json > "$OUT"/openapi.sbom.json
   ```

3. **Pack each payload into a CAR.**

   ```bash
   sorafs_cli car pack \
     --input docs/portal/static/openapi \
     --car-out "$OUT"/openapi.car \
     --plan-out "$OUT"/openapi.plan.json \
     --summary-out "$OUT"/openapi.car.json

   sorafs_cli car pack \
     --input "$OUT"/portal.sbom.json \
     --car-out "$OUT"/portal.sbom.car \
     --plan-out "$OUT"/portal.sbom.plan.json \
     --summary-out "$OUT"/portal.sbom.car.json
   ```

   Follow the same `manifest build` / `manifest sign` steps as the main site,
   tuning aliases per asset (for example, `docs-openapi.sora` for the spec and
   `docs-sbom.sora` for the signed SBOM bundle). Maintaining distinct aliases
   keeps SoraDNS proofs, GARs, and rollback tickets scoped to the exact payload.

4. **Submit and bind.** Reuse the existing authority + Sigstore bundle, but
   record the alias tuple in the release checklist so auditors can track which
   Sora name maps to which manifest digest.

Archiving the spec/SBOM manifests alongside the portal build ensures every
release ticket contains the full artefact set without rerunning the packer.

### Automation helper (CI/package script)

`./ci/package_docs_portal_sorafs.sh` codifies Steps 1–8 so roadmap item
**DOCS‑7** can be exercised with a single command. The helper:

- runs the required portal prep (`npm ci`, OpenAPI/norito sync, widget tests);
- emits the portal, OpenAPI, and SBOM CARs + manifest pairs via `sorafs_cli`;
- optionally runs `sorafs_cli proof verify` (`--proof`) and Sigstore signing
  (`--sign`, `--sigstore-provider`, `--sigstore-audience`);
- drops every artefact under `artifacts/devportal/sorafs/<timestamp>/` and
  writes `package_summary.json` so CI/release tooling can ingest the bundle; and
- refreshes `artifacts/devportal/sorafs/latest` to point at the most recent run.

Example (full pipeline with Sigstore + PoR):

```bash
./ci/package_docs_portal_sorafs.sh \
  --proof \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal
```

Flags worth knowing:

- `--out <dir>` – override the artefact root (default keeps timestamped folders).
- `--skip-build` – reuse an existing `docs/portal/build` (handy when CI cannot
  rebuild due to offline mirrors).
- `--skip-sync-openapi` – skip `npm run sync-openapi` when `cargo xtask openapi`
  cannot reach crates.io.
- `--skip-sbom` – avoid calling `syft` when the binary is not installed (the
  script prints a warning instead).
- `--proof` – run `sorafs_cli proof verify` for each CAR/manifest pair. Multi-
  file payloads still require chunk-plan support in the CLI, so leave this flag
  unset if you hit `plan chunk count` errors and verify manually once the
  upstream gate lands.
- `--sign` – invoke `sorafs_cli manifest sign`. Provide a token with
  `SIGSTORE_ID_TOKEN` (or `--sigstore-token-env`) or let the CLI fetch it using
  `--sigstore-provider/--sigstore-audience`.

When shipping production artefacts use `docs/portal/scripts/sorafs-pin-release.sh`.
It now packages the portal, OpenAPI, and SBOM payloads, signs each manifest, and
records extra asset metadata in `portal.additional_assets.json`. The helper
understands the same optional knobs used by the CI packager plus the new
`--openapi-*`, `--portal-sbom-*`, and `--openapi-sbom-*` switches so you can
assign alias tuples per artefact, override the SBOM source via
`--openapi-sbom-source`, skip certain payloads (`--skip-openapi`/`--skip-sbom`),
and point at a non-default `syft` binary with `--syft-bin`.

The script surfaces every command it runs; copy the log into the release ticket
alongside `package_summary.json` so reviewers can diff CAR digests, plan
metadata, and Sigstore bundle hashes without spelunking ad‑hoc shell output.

## Step 9 — Gateway + SoraDNS verification

Before announcing a cutover, prove the new alias resolves via SoraDNS and that
gateways staple fresh proofs:

1. **Run the probe gate.** `ci/check_sorafs_gateway_probe.sh` exercises
   `cargo xtask sorafs-gateway-probe` against the demo fixtures in
   `fixtures/sorafs_gateway/probe_demo/`. For real deployments, point the probe
   at the target hostname:

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --header "Accept: application/json" \
     --gar fixtures/sorafs_gateway/probe_demo/demo.gar.jws \
     --gar-key "demo-gar=$(<fixtures/sorafs_gateway/probe_demo/gar_pub.hex>)" \
     --host "docs.sora" \
     --report-json artifacts/sorafs_gateway_probe/ci/docs.json
   ```

   The probe decodes `Sora-Name`, `Sora-Proof`, and `Sora-Proof-Status` per
   `docs/source/sorafs_alias_policy.md` and fails when the manifest digest,
   TTLs, or GAR bindings drift.

   For lightweight spot checks (for example, when only the binding bundle
   changed), run `cargo xtask soradns-verify-binding --binding <portal.gateway.binding.json> --alias "<alias>" --hostname "<gateway-host>" --proof-status ok --manifest-json <portal.manifest.json>`.
   The helper validates the captured binding bundle and is handy for release
   tickets that only need binding confirmation instead of a full probe drill.

2. **Capture drill evidence.** For operator drills or PagerDuty dry runs, wrap
   the probe with `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario
   devportal-rollout -- …`. The wrapper stores headers/logs under
   `artifacts/sorafs_gateway_probe/<stamp>/`, updates `ops/drill-log.md`, and
   (optionally) triggers rollback hooks or PagerDuty payloads. Set
   `--host docs.sora` to validate the SoraDNS path instead of hard-coding an IP.

3. **Verify DNS bindings.** When governance publishes the alias proof, record
   the GAR file referenced in the probe (`--gar`) and attach it to the release
   evidence. Resolver owners can mirror the same input through
   `tools/soradns-resolver` to ensure cached entries honour the new manifest.
   Before attaching the JSON, run
   `cargo xtask soradns-verify-gar --gar <path> --name <alias> [--manifest-cid <cid>] [--telemetry-label <label>]`
   so the deterministic host mapping, manifest metadata, and telemetry labels are
   validated offline. The helper can emit a `--json-out` summary alongside the
   signed GAR so reviewers have verifiable evidence without opening the binary.
  When drafting a new GAR, prefer
  `cargo xtask soradns-gar-template --name <alias> --manifest <portal.manifest.json> --telemetry-label <label> ...`
  (fall back to `--manifest-cid <cid>` only when a manifest file is not
  available). The helper now derives the CID **and** BLAKE3 digest directly from
  the manifest JSON, trims whitespace, deduplicates repeated `--telemetry-label`
  flags, sorts the labels, and emits the default CSP/HSTS/Permissions-Policy
  templates before writing the JSON so the payload stays deterministic even when
  operators capture labels from different shells.

4. **Watch alias metrics.** Keep `torii_sorafs_alias_cache_refresh_duration_ms`
   and `torii_sorafs_gateway_refusals_total{profile="docs"}` on screen while the
   probe is running; both series are charted in
   `dashboards/grafana/docs_portal.json`.

## Step 10 — Monitoring & evidence bundling

- **Dashboards.** Export `dashboards/grafana/docs_portal.json` (portal SLOs),
  `dashboards/grafana/sorafs_gateway_observability.json` (gateway latency +
  proof health), and `dashboards/grafana/sorafs_fetch_observability.json`
  (orchestrator health) for every release. Attach the JSON exports to the
  release ticket so reviewers can replay the Prometheus queries.
- **Probe archives.** Keep `artifacts/sorafs_gateway_probe/<stamp>/` in git-annex
  or your evidence bucket. Include the probe summary, headers, and PagerDuty
  payload captured by the telemetry script.
- **Release bundle.** Store the portal/SBOM/OpenAPI CAR summaries, manifest
  bundles, Sigstore signatures, `portal.pin.report.json`, Try-It probe logs, and
  link-check reports under a single timestamped folder (for example,
  `artifacts/sorafs/devportal/20260212T1103Z/`).
- **Drill log.** When probes are part of a drill, let
  `scripts/telemetry/run_sorafs_gateway_probe.sh` append to `ops/drill-log.md`
  so the same evidence satisfies the SNNet-5 chaos requirement.
- **Ticket links.** Reference the Grafana panel IDs or attached PNG exports in
  the change ticket, together with the probe report path, so change-reviewers
  can cross-check the SLOs without shell access.

## Step 11 — Multi-source fetch drill & scoreboard evidence

Publishing to SoraFS now requires multi-source fetch evidence (DOCS-7/SF-6)
alongside the DNS/gateway proofs above. After pinning the manifest:

1. **Run `sorafs_fetch` against the live manifest.** Use the same plan/manifest
   artefacts produced in Steps 2–3 plus the gateway credentials issued for each
   provider. Persist every output so auditors can replay the orchestrator
   decision trail:

   ```bash
   OUT=artifacts/sorafs/devportal
   FETCH_OUT="$OUT/fetch/$(date -u +%Y%m%dT%H%M%SZ)"
   mkdir -p "$FETCH_OUT"

   cargo run -p sorafs_car --bin sorafs_fetch -- \
     --plan "$OUT/portal.plan.json" \
     --manifest-json "$OUT/portal.manifest.json" \
     --gateway-provider name=docs-us,provider-id="$DOCS_US_PROVIDER_ID",base-url="$DOCS_US_GATEWAY",stream-token="$DOCS_US_STREAM_TOKEN" \
     --gateway-provider name=docs-eu,provider-id="$DOCS_EU_PROVIDER_ID",base-url="$DOCS_EU_GATEWAY",stream-token="$DOCS_EU_STREAM_TOKEN" \
     --scoreboard-out "$FETCH_OUT/scoreboard.json" \
     --provider-metrics-out "$FETCH_OUT/providers.ndjson" \
     --json-out "$FETCH_OUT/fetch.json" \
     --chunk-receipts-out "$FETCH_OUT/chunk_receipts.ndjson" \
     --telemetry-json artifacts/sorafs/provider_telemetry.json \
     --max-peers=3 \
     --retry-budget=4
   ```

   - Fetch the provider adverts referenced by the manifest first (for example
     `sorafs_cli manifest describe --provider-adverts-out artifacts/sorafs/provider_adverts/`)
     and pass them via `--provider-advert name=path` so the scoreboard can
     evaluate capability windows deterministically. Use
     `--allow-implicit-provider-metadata` **only** when replaying fixtures in
     CI; production drills must cite the signed adverts that landed with the
     pin.
   - When the manifest references additional regions, repeat the command with
     the corresponding provider tuples so every cache/alias has a matching
     fetch artefact.

2. **Archive the outputs.** Store `scoreboard.json`,
   `providers.ndjson`, `fetch.json`, and `chunk_receipts.ndjson` under the
   release evidence folder. These files capture the peer weighting, retry
   budget, latency EWMA, and per-chunk receipts that the governance packet must
   retain for SF-7.

3. **Update telemetry.** Import the fetch outputs into the **SoraFS Fetch
   Observability** dashboard (`dashboards/grafana/sorafs_fetch_observability.json`),
   watching `torii_sorafs_fetch_duration_ms`/`_failures_total` and the
   provider-range panels for anomalies. Link the Grafana panel snapshots to the
   release ticket alongside the scoreboard path.

4. **Smoke the alert rules.** Run `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   to validate the Prometheus alert bundle before closing the release. Attach
   the promtool output to the ticket so DOCS-7 reviewers can confirm the stall
   and slow-provider alerts remain armed.

5. **Wire into CI.** The portal pin workflow keeps a `sorafs_fetch` step behind
   the `perform_fetch_probe` input; enable it for staging/production runs so the
   fetch evidence is produced alongside the manifest bundle without manual
   intervention. Local drills can reuse the same script by exporting the
   gateway tokens and setting `PIN_FETCH_PROVIDERS` to the comma-separated
   provider list.

## Promotion, observability, and rollback

1. **Promotion:** keep separate staging and production aliases. Promote by
   re-running `manifest submit` with the same manifest/bundle, swapping
   `--alias-namespace/--alias-name` to point at the production alias. This
   avoids rebuilding or resigning once QA approves the staging pin.
2. **Monitoring:** import the pin-registry dashboard
   (`docs/source/grafana_sorafs_pin_registry.json`) plus the portal-specific
   probes (see `docs/portal/docs/devportal/observability.md`). Alert on checksum
   drift, failed probes, or proof retry spikes.
3. **Rollback:** to revert, resubmit the previous manifest (or retire the
   current alias) using `sorafs_cli manifest submit --alias ... --retire`.
   Always keep the last known-good bundle and CAR summary so rollback proofs can
   be recreated if the CI logs rotate.

## CI workflow template

At minimum, your pipeline should:

1. Build + lint (`npm ci`, `npm run build`, checksum generation).
2. Package (`car pack`) and compute manifests.
3. Sign using the job-scoped OIDC token (`manifest sign`).
4. Upload artefacts (CAR, manifest, bundle, plan, summaries) for auditing.
5. Submit to the pin registry:
   - Pull requests → `docs-preview.sora`.
   - Tags / protected branches → production alias promotion.
6. Run probes + proof verification gates before exiting.

`.github/workflows/docs-portal-sorafs-pin.yml` wires all of these steps together
for manual releases. The workflow:

- builds/tests the portal,
- packages the build via `scripts/sorafs-pin-release.sh`,
- signs/verifies the manifest bundle using GitHub OIDC,
- uploads the CAR/manifest/bundle/plan/proof summaries as artifacts, and
- (optionally) submits the manifest + alias binding when secrets are present.

Configure the following repository secrets/variables before triggering the job:

| Name | Purpose |
|------|---------|
| `DOCS_SORAFS_TORII_URL` | Torii host that exposes `/v1/sorafs/pin/register`. |
| `DOCS_SORAFS_SUBMITTED_EPOCH` | Epoch identifier recorded with submissions. |
| `DOCS_SORAFS_AUTHORITY` / `DOCS_SORAFS_PRIVATE_KEY` | Signing authority for the manifest submission. |
| `DOCS_SORAFS_ALIAS_NAMESPACE` / `DOCS_SORAFS_ALIAS_NAME` | Alias tuple bound to the manifest when `perform_submit` is `true`. |
| `DOCS_SORAFS_ALIAS_PROOF_B64` | Base64-encoded alias proof bundle (optional; omit to skip alias binding). |
| `DOCS_ANALYTICS_*` | Existing analytics/probe endpoints reused by other workflows. |

Trigger the workflow via the Actions UI:

1. Provide `alias_label` (e.g., `docs.sora.link`), optional `proposal_alias`,
   and an optional `release_tag` override.
2. Leave `perform_submit` unchecked to generate artefacts without touching Torii
   (useful for dry runs) or enable it to publish directly to the configured
   alias.

`docs/source/sorafs_ci_templates.md` still documents the generic CI helpers for
projects outside this repository, but the portal workflow should be preferred
for day-to-day releases.

## Checklist

- [ ] `npm run build`, `npm run test:*`, and `npm run check:links` are green.
- [ ] `build/checksums.sha256` and `build/release.json` captured in artefacts.
- [ ] CAR, plan, manifest, and summary generated under `artifacts/`.
- [ ] Sigstore bundle + detached signature stored with logs.
- [ ] `portal.manifest.submit.summary.json` and `portal.manifest.submit.response.json`
      captured when submissions occur.
- [ ] `portal.pin.report.json` (and optional `portal.pin.proposal.json`)
      archived alongside CAR/manifest artefacts.
- [ ] `proof verify` and `manifest verify-signature` logs archived.
- [ ] Grafana dashboards updated + Try-It probes successful.
- [ ] Rollback notes (previous manifest ID + alias digest) attached to the
      release ticket.
