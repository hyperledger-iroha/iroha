---
lang: my
direction: ltr
source: docs/source/sorafs_gateway_tls_automation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd97ee9a4a923b734a3624afbe33b498bb0f3b3d042f6e87bef88667db66a26e
source_last_modified: "2026-01-22T14:35:37.640084+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Gateway TLS & ECH Operator Guide
summary: Configuration, telemetry, playbooks, and compliance requirements for SF-5b certificate automation.
---

# SoraFS Gateway TLS & ECH Operator Guide

## Scope

This guide documents how to operate the Torii SoraFS gateway when the SF-5b
certificate automation stack is enabled. It replaces the earlier planning
outline and now focuses on day-to-day operator tasks: provisioning secrets,
configuring ACME/ECH, understanding telemetry, meeting governance obligations,
and executing the approved incident playbooks.

The guidance assumes the gateway includes the automation controller in
`iroha_torii::sorafs::gateway` and that `cargo xtask sorafs-self-cert` is
available on the bastion host.

## Quick Start Checklist

1. Stage ACME credentials and GAR artefacts in the sealed
   `sorafs_gateway_tls/` secret store (KMS or Vault) and mirror them to the
   bastion host.
2. Fill in `torii.sorafs_gateway.acme` configuration with DNS-01 and
   TLS-ALPN-01 challenge preferences, enable ECH if supported, and set renewal
   thresholds.
3. Deploy or enable the `sorafs-gateway-acme@<environment>.service` unit so the
   automation controller runs continuously.
4. Run `cargo xtask sorafs-self-cert --check-tls` to capture a baseline
   attestation bundle and verify headers/metrics.
5. Publish telemetry dashboards and alerts for the metrics in the **Telemetry
   Reference** section and subscribe the on-call rotation.
6. Upload the incident playbooks to your runbook tooling (PagerDuty, Notion) and
   schedule the drills listed in **Drill Cadence & Evidence**.

## Configuration Walkthrough

### Prerequisites

| Requirement | Why it matters |
|-------------|----------------|
| ACME account credentials stored in `sorafs_gateway_tls/` sealed secrets (KMS/Vault) | Required for manual orders, revocation, and renewing automation tokens. |
| Offline copy of the latest `torii.sorafs_gateway` config bundle | Lets operators patch `ech_enabled`, host lists, or retry timing without waiting on config-management pipelines. |
| Access to governance manifests (`manifest_signatures.json`, GAR envelopes) | Needed when publishing updated certificate fingerprints or rotating canonical host mappings. |
| `cargo xtask` toolchain on the bastion host | Provides `sorafs-gateway-attest`/`sorafs-self-cert` checks used in verification steps. |
| Playbook template stored in incident tooling (PagerDuty/Notion) | Ensures the checklists in this guide are one click away during an incident. |

### Secrets & File Layout

- Store ACME account material under `/etc/iroha/sorafs_gateway_tls/` with
  ownership `iroha:iroha` and `0600` permissions.
- Maintain a sealed backup in your KMS or Vault namespace so manual rotation can
  recover quickly.
- The automation controller writes state to
  `/var/lib/sorafs_gateway_tls/automation.json`; include that path in backups so
  renewal jitter and retry windows persist across restarts.
- Generate SAN manifests with `cargo xtask soradns-acme-plan`; the helper
  derives the canonical wildcard + pretty-host SAN values, recommended ACME
  challenges, and DNS-01 labels for each alias. Store the output under
  `fixtures/sorafs_gateway/acme_san/<alias>.san.json` so GAR reviewers and the
  TLS automation controller share the same evidence bundle:

  ```bash
  cargo xtask soradns-acme-plan \
    --name docs.sora \
    --json-out fixtures/sorafs_gateway/acme_san/docs.sora.san.json
  ```

  Reuse the `san` entries when templating manual ACME orders and attach the JSON
  to DG-3 change tickets so canonical + wildcard pairings do not need to be
  recomputed.

### Torii Configuration

Add or update the following section in the Torii configuration bundle
(e.g., `configs/production.toml`):

```toml
[torii.sorafs_gateway.acme]
account_email = "tls-ops@example.com"
directory_url = "https://acme-v02.api.letsencrypt.org/directory"
dns_provider = "route53"
renewal_threshold_seconds = 2_592_000  # 30 days
retry_backoff_seconds = 900
retry_jitter_seconds = 120
use_tls_alpn_challenge = true
ech_enabled = true
```

- `dns_provider` maps to the implementation registered with
  `acme::DnsProvider`; Route53 ships in-tree, additional providers can be added
  without recompiling the gateway.
- `ech_enabled = true` prompts the automation flow to generate ECH configs
  alongside certificates; set to `false` if the upstream CDN or clients do not
  support ECH (see the fallback playbook below).
- `renewal_threshold_seconds` controls when the automation loop triggers a new
  order; default remains 30 days but can be lowered for high-risk deployments.
- The configuration parser lives in
  `iroha_torii::sorafs::gateway::config`, and validation errors surface through
  the Torii logs during startup.

### Configuration Reference

| Key | Default | Production expectation | Compliance tie-in |
|-----|---------|------------------------|-------------------|
| `torii.sorafs_gateway.acme.enabled` | `true` | Leave enabled except during incident drills. | Disabling automation must be captured in the change log with incident/ticket ID. |
| `torii.sorafs_gateway.acme.directory_url` | Let’s Encrypt v2 | Override only when switching CA environments. | Governance requires publishing the selected CA in GAR manifests. |
| `torii.sorafs_gateway.acme.dns_provider` | `route53` | Provide a concrete provider name registered with the automation crate. | IAM policy for the provider must be reviewed quarterly. |
| `torii.sorafs_gateway.acme.renewal_threshold_seconds` | `2_592_000` (30 days) | Tune for risk posture (>=14 days). | Threshold adjustments require risk owner approval. |
| `torii.sorafs_gateway.acme.retry_backoff_seconds` | `900` | Keep ≤ 900 to satisfy GAR recovery SLAs. | Backoff >900 must include a waiver in the governance log. |
| `torii.sorafs_gateway.acme.ech_enabled` | `true` | Toggle only when the fallback playbook runs. | Every toggle must be documented with reason/evidence in the compliance tracker. |
| `torii.sorafs_gateway.acme.telemetry_topic` | `sorafs-tls` | Optional override for log aggregation topic. | If overridden, register the new topic with observability for retention tracking. |

When adjusting configuration by hand, stage the change in
`iroha_config::actual::sorafs_gateway` (or your configuration management
system), capture the diff in your change-control ticket, and attach a fresh
self-cert bundle once the new settings deploy.

### Automation Service

- Enable the systemd template unit that ships with the automation bundle:

  ```bash
  systemctl enable --now sorafs-gateway-acme@production.service
  ```

- Logs stream to the `sorafs_gateway_tls_automation` journal target. Pipe them
  into your log aggregator and tag them with the environment for faster
  incident triage.
- Implementation status: `iroha_torii::sorafs::gateway::acme::AcmeAutomation`
  provides deterministic DNS-01/TLS-ALPN-01 challenge handling, configurable
  jitter/backoff, and renewal state persistence.

### Verification & Attestation

1. After applying the configuration, reload Torii:

   ```bash
   systemctl reload iroha-torii.service
   ```

2. Run the self-cert harness to confirm headers, metrics, and GAR policy
   integration:

   ```bash
   cargo xtask sorafs-self-cert --check-tls \
     --gateway https://gateway.example.com \
     --out artifacts/sorafs_gateway_self_cert
   ```

3. Archive the generated report alongside the GAR envelope for traceability.
4. Implementation status: `iroha_torii::sorafs::gateway::telemetry` exports the
   `SORA_TLS_STATE_HEADER`, `Metrics::set_sorafs_tls_state`, and
   `Metrics::record_sorafs_tls_renewal` helpers consumed by the harness.

### Header & GAR Probe

Run the deterministic header probe before every rollout (and whenever synthetic
monitoring trips) to ensure gateways staple the required Sora headers, match GAR
metadata, and advertise the expected cache/TLS state:

```bash
cargo xtask sorafs-gateway-probe \
  --gateway https://gw.example.com/car/bafy... \
  --gar artifacts/gar/self-cert.gar.jws \
  --gar-key council-key-1=8b9c...c5 \
  --header "Accept: application/vnd.ipld.car; dag-scope=full" \
  --timeout-secs 15
```

- `--gar` points at the compact GAR JWS. Provide the matching Ed25519 public
  keys via repeated `--gar-key kid=hex` flags so the probe can verify the JWS.
- `--gateway` fetches headers live; alternatively use `--headers-file` (with
  `--host`) to inspect captured dumps from `curl -i`.
- The probe asserts `Sora-Name`/`Sora-Proof` consistency, GAR host/manifest
  coverage, `Cache-Control: max-age=600, stale-while-revalidate=120`,
  `Content-Security-Policy`, `Strict-Transport-Security`, and `X-Sora-TLS-State`.
- `--report-json <path|->` writes the machine-readable summary consumed by the
  automation helpers. When targeting stdout the probe prints its human-readable
  results to stderr so the JSON stream stays parseable.
- Exits non-zero on mismatch so CI/paging hooks can fail fast. Set
  the TLS-state header yet.

#### Paging & rollback drill automation

The probe now exposes native hooks for drill logging, JSON summaries, and
PagerDuty payloads so you can run it directly from CI or ops bastions:

- `--drill-scenario <name>` plus optional `--drill-log`, `--drill-ic`,
  `--drill-scribe`, `--drill-notes`, and `--drill-link` append a row to
  `ops/drill-log.md` using the same escaping rules as
  `scripts/telemetry/log_sorafs_drill.sh`.
- `--summary-json <path>` emits a structured record (findings, GAR metadata,
  timestamps) that can be attached to the attestation bundle or drill evidence.
- `--pagerduty-routing-key <key>` enables PagerDuty integration. Combine it with
  `--pagerduty-payload <path>` (defaults to
  `artifacts/sorafs_gateway_probe/pagerduty_event.json`) and, when ready,
  `--pagerduty-url https://events.pagerduty.com/v1/enqueue` to post the event.
  Additional flags (`--pagerduty-component`, `--pagerduty-group`,
  `--pagerduty-link text=url`, `--pagerduty-dedup-key`, etc.) map directly to
  the Events API payload.

Example drill invocation:

```bash
cargo xtask sorafs-gateway-probe \
  --gateway https://gw.example.com/car/bafy... \
  --gar artifacts/gar/self-cert.gar.jws \
  --gar-key council-key-1=8b9c...c5 \
  --drill-scenario tls-renewal \
  --drill-ic "Automation Harness" \
  --drill-scribe "Ops Bot" \
  --drill-notes "Quarterly TLS rotation drill" \
  --summary-json artifacts/sorafs_gateway_probe/tls_probe.json \
  --pagerduty-routing-key "$PAGERDUTY_ROUTING_KEY" \
  --pagerduty-link "Rollback plan=https://git.example.com/sorafs/tls-rotation" \
  --pagerduty-url https://events.pagerduty.com/v1/enqueue
```

Failed probes immediately trigger PagerDuty (omit the URL during training if you
only need the payload) and still exit non-zero so CI can halt. The helper script
under `scripts/telemetry/run_sorafs_gateway_probe.sh` remains available for
preconfigured drill bundles, and CI exercises the workflow via
`ci/check_sorafs_gateway_probe.sh` using the demo fixtures in
`fixtures/sorafs_gateway/probe_demo/`; reuse that script (or copy its command
line) when wiring periodic paging drills. The native flags mean most teams can
call the probe directly without bolting on custom logging or PagerDuty glue.

### Route promotion & rollback plan

Run the new route planner once the release manifest has been built so the
cutover ticket carries a deterministic `Sora-*` header block and explicit
rollback metadata. The helper now ships inside the CLI (`iroha app sorafs gateway
wrapper when you need to automate from CI:

```bash
iroha app sorafs gateway route-plan \
  --manifest-json artifacts/sorafs_cli/portal.manifest.json \
  --hostname docs.sora.link \
  --alias sora:docs \
  --route-label docs@2026-03-21 \
  --release-tag v2026.03.21 \
  --cutover-window 2026-03-21T15:00Z/2026-03-21T15:30Z \
  --rollback-manifest-json artifacts/sorafs_cli/portal.manifest.previous.json \
  --rollback-route-label docs@previous
```

The command produces a JSON descriptor
(`artifacts/sorafs_gateway/route_plan.json` by default) plus header templates
(`gateway.route.headers.txt` and, when `--rollback-manifest-json` is supplied,
`gateway.route.rollback.headers.txt`). Each plan embeds:

- the resolved `Sora-Content-CID`,
- the fully rendered `Sora-Name`/`Sora-Proof` headers and CSP/HSTS templates,
- the canonical `Sora-Route-Binding` string (`host=…;cid=…;generated_at=…;label=…`),
- optional rollback metadata tying the previous manifest/header block to a
  human-readable label.

Attach the plan JSON and the header templates to the release ticket alongside
the DNS cutover descriptor so reviewers can diff the new binding versus the
recorded rollback state. The descriptor mirrors the `gateway_binding` block used
by `docs/portal/scripts/sorafs-pin-release.sh`, ensuring the DNS and gateway
automation pipelines promote identical headers.

## Telemetry Reference

| Surface | Name | Description | Alert / Action |
|---------|------|-------------|----------------|
| Metrics | `sorafs_gateway_tls_cert_expiry_seconds` | Seconds until the active certificate expires. | Page on-call when `< 1_209_600` (14 days).
| Metrics | `sorafs_gateway_tls_renewal_total{result}` | Renewal attempt counter labelled by `success`/`error`. | Investigate if error rate exceeds 5% in an hour.
| Metrics | `sorafs_gateway_tls_ech_enabled` | Gauge (`0`/`1`) reflecting current ECH state. | Alert when it drops to `0` unexpectedly.
| Metrics | `torii_sorafs_gar_violations_total{reason,detail}` | Policy violation counter surfaced from GAR enforcement. | Escalate to governance immediately; attach violation logs.
| Header | `X-Sora-TLS-State` | Embedded in gateway responses (e.g., `ech-enabled;expiry=2025-06-12T12:00:00Z`). | Monitor synthetically; on `ech-disabled` or `degraded`, follow the playbooks below.
| Logs | `journalctl -u sorafs-gateway-acme@*.service` | Renewal traces, challenge errors, and manual overrides. | Capture logs with incident tickets and during drills.

Expose the metrics via Prometheus/OpenTelemetry, wire dashboards for expiry and
renewal trends, and create synthetic probes that verify the
`X-Sora-TLS-State` header hourly.

### Alert Wiring

- **Expiry runway:** Alert when `sorafs_gateway_tls_cert_expiry_seconds` drops
  below 14 days (warning) and 7 days (critical). Page the gateway on-call and
  link to the **Emergency Certificate Rotation** playbook.
- **Renewal failures:** Trigger an incident when
  `sorafs_gateway_tls_renewal_total{result="error"}` increases more than three
  times in a six-hour window. Attach automation logs to the ticket.
- **GAR violations:** Route `torii_sorafs_gar_violations_total` alarms directly
  to the governance council channel so policy waivers can be granted or traffic
  can be diverted.
- **ECH state drift:** Alert the developer experience rotation when
  `sorafs_gateway_tls_ech_enabled` flips from `1` to `0` outside of a scheduled
  play; downstream SDKs must be notified to adjust expectations.

## GAR Policy Hooks

- Gateway policy denials increment `torii_sorafs_gar_violations_total{reason,detail}` so Prometheus/Alertmanager can trigger governance-alarm playbooks automatically.
- Torii now emits `DataEvent::Sorafs(GarViolation)` messages containing provider identifiers, denylist metadata, and rate-limit context. Subscribe via the existing SSE/webhook adapters to feed governance compliance pipelines.
- Request logs add structured `policy_reason`, `policy_detail`, and `provider_id_hex` fields, simplifying forensic triage and audit evidence collection.

## Compliance & Governance

Operators must satisfy the following obligations to remain in good standing
with Nexus governance:

- **GAR alignment:** publish updated certificate fingerprints in GAR manifests
  whenever a renewal completes. Submit evidence (automation logs, self-cert
  bundle, attestation fingerprint) to the governance council.
- **Policy logging:** retain GAR violation logs for at least 180 days and
  include them in quarterly compliance reports.
- **Attestation retention:** archive every `cargo xtask sorafs-self-cert`
  output under `artifacts/sorafs_gateway_tls/<YYYYMMDD>/` and grant auditors
  read-only access.
- **Config change management:** record `torii.sorafs_gateway` changes in your
  change-control system, including the reason for toggling `ech_enabled` or
  adjusting renewal thresholds.
- **Drill execution:** run the drills defined in this guide and document the
  results within three business days.

### Compliance Evidence Checklist

| Obligation | Evidence to collect | Retention | Owner |
|------------|--------------------|-----------|-------|
| GAR alignment | Updated GAR manifest, signed certificate fingerprint bundle, incident/change ticket link. | 3 years | Governance liaison |
| Policy logging | `torii_sorafs_gar_violations_total` exports, structured log excerpts, Alertmanager notifications. | 180 days | Observability |
| Attestation retention | `cargo xtask sorafs-self-cert` JSON report, TLS header snapshot, OpenSSL fingerprint output. | 3 years | Gateway operations |
| Change management | Config diff (`torii.sorafs_gateway`), approval record, deployment timestamp. | 2 years | Change manager |
| Drill documentation | Drill tracker entry, participant list, follow-up issues. | 2 years | Chaos coordinator |
| ECH toggle events | Config change log, signed bulletin to SDK/ops mailing list, telemetry snapshot before/after. | 2 years | Developer experience |

## Operational Playbooks

### Emergency Certificate Rotation

**Trigger criteria**
- `sorafs_gateway_tls_cert_expiry_seconds` < 1,209,600 (14 days).
- `sorafs_gateway_tls_renewal_total{result="failure"}` fires twice within the renewal window.
- `X-Sora-TLS-State` advertises `last-error=` or clients report certificate mismatch/handshake failures.

**Stabilise**
1. Page the SoraFS gateway TLS on-call and open an incident in `#sorafs-incident`.
2. Pause configuration rollouts and record the current config commit in the incident ticket.
3. Disable automation by setting `torii.sorafs_gateway.acme.enabled = false`, commit the change, and restart the gateway deployment.

**Issue and deploy replacement bundle**
1. Capture the current state for auditing:
   ```bash
   curl -sD - https://gateway.example/status \
     | grep -i '^x-sora-tls-state'
   openssl s_client -connect gateway.example:443 -servername gateway.example \
     < /dev/null 2>/dev/null | openssl x509 -noout -fingerprint -sha256
   ```
2. Generate a replacement bundle with the repository wrapper (writes PEM files to the pending directory):
   ```bash
   scripts/sorafs-gateway tls renew \
     --host gateway.example \
     --out /var/lib/sorafs_gateway_tls/pending \
     --account-email tls-ops@example.com \
     --directory-url https://acme-v02.api.letsencrypt.org/directory \
     --force
   ```
   The command emits `fullchain.pem`, `privkey.pem`, and `ech.json` under the pending directory.
   If the systemd automation unit manages renewals in your environment, restart it after running the CLI to resume background polling:
   ```bash
   sudo systemctl restart sorafs-gateway-acme@production.service
   journalctl -fu sorafs-gateway-acme@production.service
   ```
   Production ACME clients remain available for validated accounts, but the CLI above provides the deterministic self-signed bundle required for staging drills.
3. Stage the bundle in secrets and reload Torii:
   ```bash
   install -m 600 /var/lib/sorafs_gateway_tls/pending/fullchain.pem /etc/iroha/sorafs_gateway_tls/fullchain.pem
   install -m 600 /var/lib/sorafs_gateway_tls/pending/privkey.pem  /etc/iroha/sorafs_gateway_tls/privkey.pem
   install -m 640 /var/lib/sorafs_gateway_tls/pending/ech.json     /etc/iroha/sorafs_gateway_tls/ech.json
   systemctl reload iroha-torii.service
   ```

**Validate and restore automation**
1. Run the self-cert harness:
   ```bash
   scripts/sorafs_gateway_self_cert.sh \
     --gateway https://gateway.example \
     --cert /etc/iroha/sorafs_gateway_tls/fullchain.pem \
     --ech-config /etc/iroha/sorafs_gateway_tls/ech.json \
     --out artifacts/sorafs_gateway_self_cert
   ```
2. Confirm telemetry recovered:
   - `sorafs_gateway_tls_cert_expiry_seconds` > 2,592,000 (30 days).
   - `sorafs_gateway_tls_renewal_total{result="success"}` increments once.
   - `X-Sora-TLS-State` reports `ech-enabled;expiry=…;renewed-at=…` with no `last-error`.
3. Update governance artefacts with the new fingerprint (GAR manifest + attestation bundle) and attach them to the incident ticket.
4. Re-enable automation by setting `torii.sorafs_gateway.acme.enabled = true` and reloading Torii. Resume paused pipelines and file the post-incident report within three business days.

### ECH Fallback / Degraded Mode

Use this play when CDNs or clients fail to consume ECH.

1. Detect via `sorafs_gateway_tls_ech_enabled == 0`, customer incidents citing `GREASE_ECH_MISMATCH`, or governance directives.
2. Disable ECH:
   ```toml
   [torii.sorafs_gateway.acme]
   ech_enabled = false
   ```
   Apply the config (or use `iroha_cli config apply`), then restart Torii. The `X-Sora-TLS-State` header should now advertise `ech-disabled`.
3. Broadcast the downgrade to storage operators, SDK teams, and governance, including the expected review window.
4. Monitor `sorafs_gateway_tls_renewal_total{result="failure"}` for additional TLS churn while ECH remains disabled.
5. Once upstream services recover, restart the automation service (or invoke your ACME client manually) to produce a fresh bundle with ECH configs, rerun the self-cert harness, toggle `ech_enabled = true`, and share the updated telemetry snippets.

### Compromised-Key Revocation

Apply this playbook when private keys are exposed or the CA reports mis-issuance.

1. Keep automation disabled and rotate stream-token signing keys per the operations handbook to prevent credential reuse.
2. Revoke the current bundle with the repository wrapper (archives artifacts under `revoked/`):
   ```bash
   scripts/sorafs-gateway tls revoke \
     --out /var/lib/sorafs_gateway_tls \
     --reason keyCompromise
   ```
   The command moves `fullchain.pem`, `privkey.pem`, and `ech.json` into a timestamped backup and records an audit JSON file alongside them.
3. Issue a fresh bundle via `scripts/sorafs-gateway tls renew --out /var/lib/sorafs_gateway_tls/pending --force` (or your production ACME workflow), then follow the validation steps from the rotation playbook.
4. Publish updated GAR envelopes and `manifest_signatures.json` so downstream nodes adopt the new fingerprint.
5. Notify the governance council and SDK teams with incident ID, revocation timestamp, and remediation instructions.

## Drill Cadence & Evidence

| Drill | Frequency | Scenario | Success criteria |
|-------|-----------|----------|------------------|
| `tls-renewal` | Quarterly | Execute the full rotation playbook in staging (automation off/on). | Renewal completes in < 15 min, telemetry updated, artefacts archived. |
| `ech-fallback` | Twice yearly | Disable and restore ECH for one hour. | Clients receive bulletins, `X-Sora-TLS-State` reflects both states, zero lingering alerts. |
| `tls-revocation` | Annually | Simulate key compromise and revoke staging cert. | Revocation confirmed, replacement bundle deployed, GAR update published. |

After every drill:
- Archive automation logs, Prometheus snapshots, and self-cert output in `artifacts/sorafs_gateway_tls/<YYYYMMDD>/`.
- Update the chaos drill tracker (`docs/source/sorafs_chaos_plan.md`) with drill participants, duration, observations, and follow-up tasks.
- File any automation regressions as roadmap follow-ups (SF-5, SF-7) with linked evidence.

## Troubleshooting

- **Automation log shows repeated DNS-01 failures:** verify IAM permissions for
  the configured `dns_provider` and confirm TXT propagation with
  `dig _acme-challenge.gateway.example.com txt`.
- **`X-Sora-TLS-State` missing or malformed:** ensure the Torii service was
  reloaded after copying certificates and that
  `Metrics::set_sorafs_tls_state` succeeds (check Torii logs for `warn` level
  failures).
- **GAR violation counters increasing:** inspect the structured logs emitted in
  `torii_sorafs_gar_violations_total`, remediate the offending provider or
  manifest, and notify governance before re-enabling traffic.
- **ECH clients still failing after toggle:** confirm caches were invalidated
  (Cloudflare/CloudFront) and share fallback hostnames with integrators.

## Implementation Notes

- **ACME client library:** the deployment uses
  [`letsencrypt-rs`](https://crates.io/crates/letsencrypt-rs) for both DNS-01 and
  TLS-ALPN-01 challenges. It integrates with the async executor already present
  in the gateway and ships under Apache-2.0.
- **DNS provider abstraction:** Route53 support is available by default. The
  automation exposes a `DnsProvider` trait so teams can implement Cloudflare or
  Google Cloud DNS providers without touching the controller. Configure the
  provider via `acme.dns_provider = "<provider>"`.
- **Telemetry naming alignment:** Observability approved the metric names in the
  telemetry table; durations are exposed in seconds. Review dashboards after
  upgrades to ensure schema changes are reflected.
- **Self-cert integration:** the TLS automation flow invokes the self-cert kit
  after each renewal. `sorafs_gateway_tls_automation.yml` calls
  `cargo xtask sorafs-self-cert --check-tls` before notifying operators, making
  TLS/ECH validation part of the automated rollout.
