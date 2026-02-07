---
lang: ka
direction: ltr
source: docs/source/sorafs_gateway_dns_owner_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 93e964470884486630328eee9eb06c6fbbf020a4030958c444ad6c5ea0de70b4
source_last_modified: "2026-01-22T14:35:37.507026+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Gateway & DNS Owner Runbook
summary: Operational checklist for the Decentralized DNS & Gateway workstream covering automation owners, rehearsals, and cutover evidence.
---

# SoraFS Gateway & DNS Owner Runbook

This runbook closes the **Decentralized DNS & Gateway** follow-up tracked in
`roadmap.md` by spelling out how the three accountable owners coordinate DNS
automation, alias promotion, telemetry sampling, and rollback drills ahead of
the 2025‑03‑03 kickoff. It complements the attendance tracker
(`docs/source/sorafs_gateway_dns_design_attendance.md`), the pre-read, and the
GAR telemetry snapshot and should be updated whenever any of the referenced
tooling or evidence buckets change.

## 1. Roles & Scope

| Scope | Primary owner | Backup | Inputs / tooling |
|-------|---------------|--------|------------------|
| SoraDNS zonefile automation & GAR pinning | Ops Lead (`ops.lead@soranet`) | Networking TL (`networking.tl@soranet`) | `tools/soradns-resolver/`, `docs/source/sns/governance_playbook.md`, `promtool`, `ops/drill-log.md` |
| Gateway alias & SoraFS cutover metadata | Tooling WG (`tooling.wg@sorafs`) | Docs/DevRel (`docs.devrel@sora`) | `docs/portal/scripts/sorafs-pin-release.sh`, `docs/portal/scripts/generate-dns-cutover-plan.mjs`, portal deploy guide |
| Telemetry snapshots & rollback automation | QA Guild (`qa.guild@sorafs`) | Security Engineering (`security@soranet`) | `scripts/telemetry/run_soradns_transparency_tail.sh`, `scripts/telemetry/schedule_soradns_ir_drill.sh`, GAR telemetry notes |

## 2. Timeline & Evidence Checklist

| Window | Owner(s) | Checklist | Evidence |
|--------|----------|-----------|----------|
| T‑14 days | Tooling WG + Docs/DevRel | Build portal, run packaging script in dry-run mode, and emit the DNS cutover descriptor.<br>`npm ci && npm run build && ./docs/portal/scripts/sorafs-pin-release.sh --skip-submit --dns-change-ticket OPS-XXXX --dns-cutover-window 2025-03-03T16:00Z/2025-03-03T16:30Z --dns-hostname docs.sora.link --dns-zone sora.link --ops-contact ops.lead@soranet --cache-purge-endpoint https://cache.api/purge --cache-purge-auth-env CACHE_PURGE_TOKEN --previous-dns-plan artifacts/sorafs/previous.dns-cutover.json` | `artifacts/sorafs/portal.pin.report.json`, `artifacts/sorafs/portal.additional_assets.json`, `artifacts/sorafs/portal.dns-cutover.json`, `artifacts/sorafs/portal.gateway.binding.json`, `artifacts/sorafs/portal.gateway.headers.txt`, `artifacts/sorafs/gateway.route_plan.json`, `artifacts/sorafs/checksums.sha256`, portal deploy ticket |
| T‑10 days | Ops Lead | Update zonefile skeleton per `docs/source/sns/governance_playbook.md`, stash under `artifacts/sns/zonefiles/<zone>/<version>.json`, and verify resolver config picks up the new records:<br>`cargo run -p soradns-resolver -- --config ops/soradns/resolver.staging.json`<br>`python3 scripts/sns_zonefile_skeleton.py --cutover-plan artifacts/sorafs/portal.dns-cutover.json --out artifacts/sns/zonefiles/sora.link/20250303.docs.sora.json --resolver-snippet-out ops/soradns/static_zones.docs.json --ipv4 <gateway-ip> --ttl 600 --zonefile-version 20250303.docs.sora --effective-at 2025-03-03T16:00Z --gar-digest <gar-digest-hex> --freeze-state soft --freeze-ticket SNS-DF-XXXX --freeze-expires-at 2025-03-10T12:00Z --freeze-note "guardian review" --txt ChangeTicket=OPS-XXXX` | Zonefile JSON, resolver sync log, commit hash of `ops/soradns/resolver.staging.json` |

The helper now stamps `zonefile.{name,version,ttl,effective_at,cid,gar_digest,proof}` metadata alongside
`static_zone`. Always point `--effective-at` at the cutover window start, set `--zonefile-version`
to the ticket/tag logged in change control, and capture the GAR digest emitted by the signing
automation (or pass the manual digest once the GAR envelope is staged). Leave `--zonefile-proof`
unset unless the automation cannot surface the `Sora-Proof` header—when that occurs, copy the proof
literal from the gateway runbook entry so Torii/resolver evidence agrees with the GAR bundle.
| T‑7 days | QA Guild + Security | Capture GAR telemetry snapshot, regenerate Prometheus scrape, and update the GAR telemetry note:<br>`scripts/telemetry/run_soradns_transparency_tail.sh --log /var/log/soradns/transparency.log --metrics-output docs/source/sorafs_gateway_dns_design_metrics_$(date +%Y%m%d).prom --format jsonl` | Updated `docs/source/sorafs_gateway_dns_design_gar_telemetry.md`, new `.prom` artefact, alert screenshot |
| T‑5 days | Ops Lead + Docs | Schedule the DNS/GAR rehearsal via the drill logger:<br>`scripts/telemetry/schedule_soradns_ir_drill.sh --date 2025-02-27 --log ops/drill-log.md --notes "DNS automation rehearsal for kickoff"` | `ops/drill-log.md` entry, calendar invite |
| T‑2 days | All owners | Review `artifacts/sorafs/portal.dns-cutover.json`, confirm `change_ticket`, `cutover_window`, alias namespace/name, cache purge block (endpoint/payload/auth var), rollback metadata, the embedded `gateway_binding` (content CID + headers template), and the new `route_plan` stanza (path, primary + rollback headers). Re-run `iroha app sorafs gateway route-plan …` (or `scripts/sorafs-gateway route plan …` from CI) so `artifacts/sorafs_gateway/route_plan.json` plus the header templates (`gateway.route.headers.txt`, `gateway.route.rollback.headers.txt` when present) reflect the final manifest. Attach the descriptor + header/route plan bundle to the rollout ticket and Slack thread. | Rollout ticket comment with descriptor diff + route plan |

> **New SN-7 validation step:** The packaging script runs
> `cargo xtask soradns-verify-binding --binding artifacts/sorafs/portal.gateway.binding.json`
> (plus the alias/hostname/manifest flags) automatically, but you must re-run it whenever the
> binding file is tweaked outside CI. The helper decodes `Sora-Proof`, checks that the route
> binding matches the manifest CID + hostname, and refuses to pass when headers drift. Capture
> the console output in the rollout ticket so reviewers see the evidence inline.
| Cutover (T) | Ops + Tooling + QA | Apply zonefile, restart resolvers, and run portal probe listed in the descriptor:<br>`npm run probe:portal -- --base-url=https://docs.sora.link --expect-release=$(jq -r '.release.tag' artifacts/sorafs/portal.dns-cutover.json)`.<br>Monitor `torii_sorafs_gar_violations_total`/`torii_sorafs_gateway_refusals_total`. | `artifacts/sorafs/dns-cutover/<timestamp>/` bundle containing resolver logs, portal probe output, Grafana screenshot |
| T + 1 day | QA Guild + Security | Re-run transparency tailer with Pushgateway upload, file post-cutover telemetry update, and archive signed evidence in the governance DAG. | `artifacts/soradns/transparency.prom`, governance log link, `status.md` note |

## 3. Automation Procedures

### 3.1 Release packaging & alias metadata (Tooling WG / Docs)

1. Build the docs portal (`npm ci && npm run build`), ensuring `build/checksums.sha256`
   exists (generated by `docs/portal/scripts/write-checksums.mjs` via `postbuild`).
2. Run `docs/portal/scripts/sorafs-pin-release.sh`, supplying DNS metadata via flags
   or `DNS_*` env vars. Include `--skip-submit` for rehearsal; omit it when
   promoting to Torii. The script emits:
   - `artifacts/sorafs/portal.pin.report.json`
   - `artifacts/sorafs/portal.additional_assets.json`
   - `artifacts/sorafs/portal.manifest.to` + `.json`
   - `artifacts/sorafs/portal.dns-cutover.json` (via the embedded call to
     `docs/portal/scripts/generate-dns-cutover-plan.mjs`)
   - `artifacts/sorafs/portal.gateway.binding.json`
   - `artifacts/sorafs/portal.gateway.headers.txt`
3. When a descriptor needs edits (e.g., updated window), re-run the generator directly:

```bash
node docs/portal/scripts/generate-dns-cutover-plan.mjs \
  --pin-report artifacts/sorafs/portal.pin.report.json \
  --change-ticket OPS-4242 \
  --cutover-window 2025-03-03T16:00Z/2025-03-03T16:30Z \
  --dns-hostname docs.sora.link \
  --dns-zone sora.link \
  --ops-contact ops.lead@soranet \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --previous-dns-plan artifacts/sorafs/previous.dns-cutover.json \
  --out artifacts/sorafs/portal.dns-cutover.json
```

4. Attach `portal.dns-cutover.json`, `portal.pin.report.json`, `portal.additional_assets.json`,
   `portal.gateway.binding.json`, `portal.gateway.headers.txt`, and the signed
   manifest bundle to the rollout ticket. Highlight the `cache_invalidation`
   command and `rollback` block, and update the deploy guide
   (`docs/portal/docs/devportal/deploy-guide.md`) with any new flag usage or
   release-tag metadata.

### 3.2 Zonefile & resolver automation (Ops Lead / Networking TL)

1. Generate the zonefile skeleton following §4.5 of
   `docs/source/sns/governance_playbook.md` using the helper script:
   ```bash
   python3 scripts/sns_zonefile_skeleton.py \
     --cutover-plan artifacts/sorafs/portal.dns-cutover.json \
     --out artifacts/sns/zonefiles/<zone>/<timestamp>.<alias>.json \
     --resolver-snippet-out ops/soradns/static_zones.<alias>.json \
     --ipv4 <gateway-ip> --ipv6 <gateway-ipv6> --ttl 600 \
     --freeze-state <soft|hard|thawing|monitoring|emergency> \
     --freeze-ticket SNS-DF-XXXX \
     --freeze-expires-at 2025-03-10T12:00Z \
     --freeze-note "guardian review" \
     --txt ChangeTicket=OPS-XXXX --spki-pin <base64pin>
   ```
   Freeze metadata flags (`--freeze-ticket`, `--freeze-expires-at`, `--freeze-note`)
   require `--freeze-state`; the helper now fails fast on partial freeze metadata so
   guardian evidence stays consistent.
   Archive the JSON under `artifacts/sns/zonefiles/<zone>/<YYYYMMDD>.json`
   and note the selector hashes. When a freeze is in effect, the helper
   records the metadata under the `freeze` key and emits matching `Sora-Freeze-*`
   TXT records so resolver automation, dashboards, and customer comms can all
   reference the same guardian ticket and expiry timestamp. The resolver snippet
   written to `ops/soradns/static_zones.<alias>.json` now mirrors the `freeze`
   block so guardians can prove propagation—`soradns-resolver` refuses DNS queries
   whenever a selector is marked `soft`, `hard`, or `emergency` frozen (returning
   `SERVFAIL` or `REFUSED`), while `thawing` and `monitoring` phases continue to
   serve records for staged rollbacks.
   The zonefile skeleton only defines the authoritative zone records. It does
   not set parent-zone NS/DS delegation at the registrar; that delegation must
   be configured separately for regular internet resolution.
2. Update the resolver configuration (example path `ops/soradns/resolver.staging.json`)
   so the new bundle and RAD sources point at the staged artefacts.
3. Validate the config locally:

```bash
cargo run -p soradns-resolver -- --config ops/soradns/resolver.staging.json
```

This performs a sync pass, logging bundle and RAD counts. Capture the log and
stash it alongside the zonefile artefact.

4. Tail the resolver transparency log to ensure the bundles landed:

```bash
scripts/telemetry/run_soradns_transparency_tail.sh \
  --log /var/log/soradns/resolver.transparency.log \
  --metrics-output artifacts/soradns/transparency.prom \
  --format jsonl
```

5. Run the alert tests before each cutover:

```bash
promtool test rules dashboards/alerts/tests/soradns_transparency_rules.test.yml
```

6. Commit any resolver/event-log diffs and reference them in `ops/drill-log.md`
   when rehearsals conclude.

### 3.3 Telemetry evidence & rollback readiness (QA Guild / Security)

1. Refresh the GAR telemetry snapshot file (`docs/source/sorafs_gateway_dns_design_gar_telemetry.md`)
   by scraping staging metrics into a new `.prom` artefact (see §2 timeline).
2. Verify the GAR/telemetry dashboards referenced in the roadmap remain green
   and export PNGs for the evidence bundle.
3. Prepare rollback commands by recording:
   - `sorafs_cli manifest revert` invocation with the previous manifest digest (available via the `rollback.manifest_digest_hex` field in `portal.dns-cutover.json`).
   - DNS reverse plan derived from the descriptor’s `rollback.plan_path` (swap the hostname back to `docs.staging.sora.link`, reuse `npm run probe:portal` to ensure rollback succeeded).
4. Keep `scripts/telemetry/inject_redaction_failure.sh` handy for chaos drills
   and document any anomalies in the drill log.

## 4. Automation Rehearsal & Incident Drills

- Schedule rehearsals with
  `scripts/telemetry/schedule_soradns_ir_drill.sh --log ops/drill-log.md`.
  Include the DNS/GAR scenario label, IC, scribe, and notes regarding which
  artefacts will be rotated.
- During rehearsals, run the full toolchain end-to-end (portal packaging, zonefile
  publish, resolver sync, telemetry scrape) and record:
  - Generated artefacts under `artifacts/sorafs/dns-rehearsal/<date>/`
  - `ops/drill-log.md` entry containing links to the artefacts and any follow-up
    JIRA tickets.
- Confirm `docs/portal/scripts/__tests__/dns-cutover-plan.test.mjs` is green so
  descriptor regressions are caught in CI.

## 5. Cutover-Day Checklist

1. **Freeze inputs:** Lock the manifest bundle and zonefile artefacts in Git
   (`git tag dns-cutover-20250303`) and share the SHA in `#sorafs-gateway`.
2. **Apply zonefile:** Publish the signed zonefile skeleton to the resolver
   bundle bucket and reload the resolvers. Watch `soradns_bundle_proof_age_seconds`
   for regressions.
3. **Verify resolver + DNS:** Run `cargo run -p soradns-resolver -- --config ...`
   on at least one production node to confirm the new records are served. Follow
   with `dig +dnssec docs.sora.link` pointed at each resolver endpoint.
4. **Portal probe:** Execute the verification command embedded in
   `portal.dns-cutover.json` (typically `npm run probe:portal ...`) and attach the
   JSON output to the change ticket.
5. **Gateway headers:** Apply the `gateway_binding.headers_template` (or the
   regenerated `gateway.route.headers.txt` attachment) to the edge automation so the
   `Sora-Name/Sora-Proof/CSP` bundle matches the release. Capture the rendered
   config snippet and stash it with the cutover artefacts. When the route plan
   includes rollback metadata, archive `gateway.route.rollback.headers.txt` with
   the ticket so reviewers can flip back without recomputing headers.
6. **Cache purge:** If the descriptor includes a `cache_invalidation` block,
   run the recorded `curl` command (substituting the real token) and archive
   the output alongside the cutover artefacts.
7. **Telemetry watch:** Keep `torii_sorafs_gar_violations_total`,
   `torii_sorafs_gateway_refusals_total`, and
   `torii_sorafs_tls_cert_expiry_seconds` dashboards in view for ≥30 minutes.
   Export PNG snapshots and add them to the artefact bundle.
8. **Rollback triggers:** If GAR violations spike or probe fails,
   - Revert the DNS entry using the previous zonefile skeleton.
   - Run `npm run probe:portal` against the prior hostname.
   - File an incident per `docs/portal/docs/devportal/incident-runbooks.md`.

## 6. Evidence & Reporting

- Bundle every run’s artefacts under
  `artifacts/sorafs/dns-cutover/<YYYYMMDD>/` with subdirectories for
  `portal/`, `zonefiles/`, `telemetry/`, and `drills/`.
- Update `docs/source/sorafs_gateway_dns_design_gar_telemetry.md` after each
  snapshot and reference the new `.prom` file.
- Log completion in `status.md` (Latest Updates) along with roadmap reference
  so governance sees the trail.
- Provide a summary in the weekly `status.md` refresh and in the kickoff deck
  once the runbook changes.

Keeping this document current is mandatory for SF‑4/SF‑5 readiness; when any
CLI flag, evidence path, or owner assignment changes, update this runbook and
link the change in the associated rollout ticket.
