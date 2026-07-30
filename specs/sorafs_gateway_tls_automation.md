---
title: SoraFS Gateway TLS & ECH Operator Guide
summary: Fail-closed runtime ACME integration, telemetry, playbooks, and compliance requirements for SF-5b.
---

# SoraFS Gateway TLS & ECH Operator Guide

## Scope

This guide documents how to operate the Torii SoraFS gateway when the SF-5b
certificate automation stack is enabled. It replaces the earlier planning
outline and now focuses on day-to-day operator tasks: provisioning secrets,
configuring ACME/ECH, understanding telemetry, meeting governance obligations,
and executing the approved incident playbooks.

The guidance assumes the gateway embeds the generic automation controller in
`iroha_torii::sorafs::gateway`, injects an independently audited
`AcmeClient` implementation at runtime, and that the repository
`scripts/sorafs_gateway_self_cert.sh` wrapper is available on the bastion host.
The wrapper invokes the `sorafs-gateway-attest` xtask command and falls back to
`cargo run -p xtask --bin xtask -- ...` when `cargo xtask` is not installed.

> **Runtime ACME boundary (V1):** the repository does not ship a production
> ACME client, DNS-provider adapter, account credential loader, or self-signed
> fallback. `SelfSignedAcmeClient` exists only as a `cfg(test)` fixture.
> Enabling `sorafs.gateway.acme` without an audited runtime-injected
> `AcmeClient` is a startup error. There is no production repository renewal
> command or stored-certificate fallback. On renewal or validation failure,
> withdraw the affected gateway until the audited adapter has atomically
> installed a CA-valid replacement and the controller and external probes agree.

## Quick Start Checklist

1. Stage ACME credentials only in the runtime adapter's sealed KMS/Vault
   namespace. Do not mirror account credentials or private keys to the bastion
   host or readiness artifacts.
2. Fill in `sorafs.gateway.acme` configuration with DNS-01 and
   TLS-ALPN-01 challenge preferences, enable ECH if supported, and set renewal
   thresholds.
3. Deploy the embedding that injects the reviewed runtime ACME adapter. The
   adapter owns provider transport, challenge execution, key custody, atomic
   certificate installation, and reload coordination. Do not set
   `acme.enabled = true` before that dependency is present.
4. Run `scripts/sorafs_gateway_self_cert.sh` to capture a baseline attestation
   bundle and verify headers/metrics.
5. Publish telemetry dashboards and alerts for the metrics in the **Telemetry
   Reference** section and subscribe the on-call rotation.
6. Upload the incident playbooks to your runbook tooling (PagerDuty, Notion) and
   schedule the drills listed in **Drill Cadence & Evidence**.

## Configuration Walkthrough

### Prerequisites

| Requirement | Why it matters |
|-------------|----------------|
| Audited runtime ACME client supplied by the deployment embedding | Required before `acme.enabled = true`; the repository has no built-in production client. |
| ACME account credentials stored in the runtime client's sealed KMS/Vault namespace | Credentials are owned by the injected client and must not enter config, logs, or repository artifacts. |
| Offline copy of the latest `sorafs.gateway` config bundle | Lets operators patch `ech_enabled`, host lists, or retry timing without waiting on config-management pipelines. |
| Access to governance manifests (`manifest_signatures.json`, GAR envelopes) | Needed when publishing updated certificate fingerprints or rotating canonical host mappings. |
| Repository checkout with Cargo/xtask access on the bastion host | Provides the self-cert wrapper, `sorafs-gateway-attest`, and `sorafs-gateway-probe` checks used in verification steps. |
| Playbook template stored in incident tooling (PagerDuty/Notion) | Ensures the checklists in this guide are one click away during an incident. |

### Secrets & File Layout

- Keep ACME account material and certificate private keys inside the runtime
  adapter's approved KMS/Vault boundary. Do not export them to a repository
  checkout, bastion workspace, readiness bundle, or process log.
- Maintain recovery capability through the KMS/Vault backup and independently
  reviewed adapter procedure; a copied local certificate/key directory is not a
  recovery path.
- The injected ACME client owns any durable provider state. Include only its
  reviewed, credential-safe recovery metadata in backups and require it to
  install new certificate material atomically before reporting success. Torii
  configuration and readiness artifacts must not contain account keys, private
  keys, or tokens.
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

  For Taira Soracloud browser gateway hosts, render the same plan with the Mon
  pretty suffix so the bind-time order covers the exact public host:

  ```bash
  cargo xtask soradns-acme-plan \
    --name solswap-indexer.sora \
    --pretty-suffix mon.taira.sora.net \
    --json-out fixtures/sorafs_gateway/acme_san/solswap-indexer.sora.mon.san.json
  ```

  Reuse the `san` entries when templating manual ACME orders and attach the JSON
  to DG-3 change tickets so canonical + wildcard pairings do not need to be
  recomputed.

### Torii Configuration

Add or update the following section in the Torii configuration bundle
(e.g., `configs/production.toml`):

```toml
[sorafs.gateway.acme]
enabled = true
provider_handle = "hsm://gateway/acme/primary"
provider_revision = 7
provider_policy_digest_hex = "5151515151515151515151515151515151515151515151515151515151515151"
account_email = "tls-ops@example.com"
directory_url = "https://acme-v02.api.letsencrypt.org/directory"
hostnames = ["gateway.example.com"]
dns_provider_id = "reviewed-production-adapter"
renewal_window = "30d"
retry_backoff = "30m"
retry_jitter = "5m"
ech_enabled = true

[sorafs.gateway.acme.challenges]
dns01 = true
tls_alpn_01 = true
```

- `dns_provider_id` is an opaque selector consumed by the injected adapter; no
  DNS-provider implementation ships in this repository.
- `provider_handle`, `provider_revision`, and
  `provider_policy_digest_hex` form one exact, non-secret runtime binding.
  Replace the example values with the production adapter's reviewed identity;
  partial, zero, uppercase-digest, test-marked, or dormant bindings fail
  configuration. Credentials and private keys remain runtime-only.
- `ech_enabled` sets the controller's expected telemetry state. ECH generation,
  validation, installation, and rollback remain responsibilities of the
  injected adapter.
- `renewal_window` controls when the controller requests a new order; the
  default is 30 days.
- Configuration is parsed through `iroha_config`; an enabled ACME policy
  without the complete provider binding or an injected runtime adapter fails
  during startup. Torii also checks the injected identity before and after
  every certificate order and discards returned key material if the identity
  changes while the operation is in flight.

### Configuration Reference

| Key | Default | Production expectation | Compliance tie-in |
|-----|---------|------------------------|-------------------|
| `sorafs.gateway.acme.enabled` | `false` | Enable only in a daemon embedding that injects the audited runtime adapter. | Enable/disable changes require an approved deployment record. |
| `sorafs.gateway.acme.provider_handle` | unset | Pin the exact stable production adapter handle; test/development markers are rejected. | Bind the approved runtime implementation without storing credentials. |
| `sorafs.gateway.acme.provider_revision` | unset | Pin the non-zero deployed adapter and public-policy revision. | Rotation requires a reviewed configuration revision. |
| `sorafs.gateway.acme.provider_policy_digest_hex` | unset | Pin the non-zero 64-character lowercase digest reported by the adapter. | Proves the runtime's reviewed public policy matches the deployment envelope. |
| `sorafs.gateway.acme.directory_url` | Let’s Encrypt v2 | Override only when switching CA environments. | Governance requires publishing the selected CA in GAR manifests. |
| `sorafs.gateway.acme.dns_provider_id` | unset | Bind the exact reviewed adapter/provider configuration. | Adapter IAM and challenge permissions require periodic review. |
| `sorafs.gateway.acme.renewal_window` | `30d` | Keep enough headroom to withdraw and recover before expiry. | Window changes require risk-owner approval. |
| `sorafs.gateway.acme.retry_backoff` | `30m` | Keep bounded and alert on repeated failure. | Overrides require an incident/change record. |
| `sorafs.gateway.acme.retry_jitter` | `5m` | Keep deterministic and bounded across replicas. | Overrides require an incident/change record. |
| `sorafs.gateway.acme.ech_enabled` | `false` | Enable only when the adapter and public edge both install and validate ECH. | Every toggle must be documented with reason/evidence. |

When adjusting configuration by hand, stage the change in
`iroha_config::actual::sorafs_gateway` (or your configuration management
system), capture the diff in your change-control ticket, and attach a fresh
self-cert bundle once the new settings deploy.

### Automation Service

- Package the daemon embedding and its reviewed adapter under the deployment's
  supervisor. The repository does not ship a production adapter or a standalone
  ACME systemd unit.
- The in-tree controller schedules requests, applies bounded deterministic
  retry jitter, and updates TLS telemetry. The adapter performs ACME transport,
  challenge handling, durable provider recovery, key custody, atomic
  installation, and service reload.
- Route controller and adapter outcomes into payload-free operational logs.
  Certificate bodies, private keys, account credentials, and challenge tokens
  must never enter logs or readiness evidence.

### Verification & Attestation

1. After applying the configuration, reload Torii:

   ```bash
   systemctl reload iroha-torii.service
   ```

2. Run the self-cert harness to confirm headers, metrics, and GAR policy
   integration:

   ```bash
   scripts/sorafs_gateway_self_cert.sh \
     --config /run/sorafs-release/gateway-self-cert.conf \
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
  `--pagerduty-url https://v1/events/ws.pagerduty.com/v1/enqueue` to post the event.
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
  --pagerduty-url https://v1/events/ws.pagerduty.com/v1/enqueue
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
recorded rollback state. The property release system must consume the same
`gateway_binding` block so the DNS and gateway automation pipelines promote
identical headers.

## Telemetry Reference

| Surface | Name | Description | Alert / Action |
|---------|------|-------------|----------------|
| Metrics | `torii_sorafs_tls_cert_expiry_seconds` | Seconds until the active certificate expires. | Page on-call when `< 1_209_600` (14 days).
| Metrics | `torii_sorafs_tls_renewal_total{result}` | Renewal attempt counter labelled by `success`/`failure`. | Investigate any failure and withdraw before the recovery window closes.
| Metrics | `torii_sorafs_tls_ech_enabled` | Gauge (`0`/`1`) reflecting current ECH state. | Alert when it drops to `0` unexpectedly.
| Metrics | `torii_sorafs_gar_violations_total{reason,detail}` | Policy violation counter surfaced from GAR enforcement. | Escalate to governance immediately; attach violation logs.
| Header | `X-Sora-TLS-State` | Embedded in gateway responses (e.g., `ech-enabled;expiry=2025-06-12T12:00:00Z`). | Monitor synthetically; on `ech-disabled` or `degraded`, follow the playbooks below.
| Logs | Deployment-owned controller/adapter supervisor | Payload-free renewal outcomes and bounded failure classes. | Capture sanitized logs with incident tickets and during drills.

Expose the metrics via Prometheus/OpenTelemetry, wire dashboards for expiry and
renewal trends, and create synthetic probes that verify the
`X-Sora-TLS-State` header hourly.

### Alert Wiring

- **Expiry runway:** Alert when `torii_sorafs_tls_cert_expiry_seconds` drops
  below 14 days (warning) and 7 days (critical). Page the gateway on-call and
  link to the **Emergency Certificate Rotation** playbook.
- **Renewal failures:** Trigger an incident when
  `torii_sorafs_tls_renewal_total{result="failure"}` increases. Attach
  payload-free controller and adapter logs to the ticket.
- **GAR violations:** Route `torii_sorafs_gar_violations_total` alarms directly
  to the governance council channel so policy waivers can be granted or traffic
  can be diverted.
- **ECH state drift:** Alert the developer experience rotation when
  `torii_sorafs_tls_ech_enabled` flips from `1` to `0` outside of a scheduled
  play; downstream SDKs must be notified to adjust expectations.

## GAR Policy Hooks

- Gateway policy denials increment
  `torii_sorafs_gar_violations_total{reason,detail}` with bounded labels so
  Prometheus/Alertmanager can trigger governance playbooks.
- Torii emits typed `DataEvent::Sorafs(GarViolation)` events for authorized
  policy consumers. Do not copy event identifiers or request context into
  metric labels, ordinary logs, or readiness artifacts.
- Governed compliance decisions use their dedicated bounded telemetry and
  promoted-catalog audit surface; TLS automation must not recreate a local
  policy list.

## Compliance & Governance

Operators must satisfy the following obligations to remain in good standing
with Nexus governance:

- **GAR alignment:** publish updated certificate fingerprints in GAR manifests
  whenever a renewal completes. Submit evidence (automation logs, self-cert
  bundle, attestation fingerprint) to the governance council.
- **Policy logging:** retain GAR violation logs for at least 180 days and
  include them in quarterly compliance reports.
- **Attestation retention:** archive every `scripts/sorafs_gateway_self_cert.sh`
  output under `artifacts/sorafs_gateway_tls/<YYYYMMDD>/` and grant auditors
  read-only access.
- **Config change management:** record `sorafs.gateway` changes in your
  change-control system, including the reason for toggling `ech_enabled` or
  adjusting renewal thresholds.
- **Drill execution:** run the drills defined in this guide and document the
  results within three business days.

### Compliance Evidence Checklist

| Obligation | Evidence to collect | Retention | Owner |
|------------|--------------------|-----------|-------|
| GAR alignment | Updated GAR manifest, signed certificate fingerprint bundle, incident/change ticket link. | 3 years | Governance liaison |
| Policy logging | `torii_sorafs_gar_violations_total` exports, structured log excerpts, Alertmanager notifications. | 180 days | Observability |
| Attestation retention | Self-cert JSON report, TLS header snapshot, OpenSSL fingerprint output. | 3 years | Gateway operations |
| Change management | Config diff (`sorafs.gateway`), approval record, deployment timestamp. | 2 years | Change manager |
| Drill documentation | Drill tracker entry, participant list, follow-up issues. | 2 years | Chaos coordinator |
| ECH toggle events | Config change log, signed bulletin to SDK/ops mailing list, telemetry snapshot before/after. | 2 years | Developer experience |

## Operational Playbooks

### Emergency Certificate Rotation

**Trigger criteria**
- `torii_sorafs_tls_cert_expiry_seconds` < 1,209,600 (14 days).
- `torii_sorafs_tls_renewal_total{result="failure"}` fires within the renewal window.
- `X-Sora-TLS-State` advertises `last-error=` or clients report certificate mismatch/handshake failures.

**Stabilise**
1. Page the SoraFS gateway TLS on-call and open an incident in `#sorafs-incident`.
2. Pause configuration rollouts and record the current config commit in the incident ticket.
3. Withdraw the affected gateway from the provider-admission inventory, regional
   load balancer, and public DNS. Do not keep it in service on a stored
   certificate when the active chain is invalid, cannot be verified, or lacks
   enough lifetime for the recovery window.
4. Leave the controller enabled when its adapter is healthy enough to retry. If
   the adapter itself is compromised, disable ACME in the controlled
   configuration rollout while the gateway remains withdrawn.

**Issue and deploy a replacement**
1. Capture the current state for auditing:
   ```bash
   curl -sD - https://gateway.example/status \
     | grep -i '^x-sora-tls-state'
   openssl s_client -connect gateway.example:443 -servername gateway.example \
     < /dev/null 2>/dev/null | openssl x509 -noout -fingerprint -sha256
   ```
2. Repair the deployment's audited runtime adapter or invoke its independently
   controlled emergency issuance path. The adapter must validate the CA chain,
   SAN set, key binding, expiry, and optional ECH material, install the new
   secret atomically inside its KMS/Vault boundary, and reload the serving edge.
   Repository tooling neither issues nor installs production certificates.
3. Confirm the controller records exactly one successful renewal for the
   replacement and that the adapter reports an atomic install/reload. A
   successful order without a successful install is not recovery.

**Validate and restore service**
1. Run the self-cert harness:
   ```bash
   scripts/sorafs_gateway_self_cert.sh \
     --config /run/sorafs-release/gateway-self-cert.conf \
     --gateway https://gateway.example \
     --out artifacts/sorafs_gateway_self_cert
   ```
2. Confirm telemetry recovered:
   - `torii_sorafs_tls_cert_expiry_seconds` > 2,592,000 (30 days).
   - `torii_sorafs_tls_renewal_total{result="success"}` increments once.
   - `X-Sora-TLS-State` reports `ech-enabled;expiry=…;renewed-at=…` with no `last-error`.
3. Verify the public hostname from outside the deployment network and confirm
   the served chain, SAN set, and fingerprint match the controller/adapter
   evidence.
4. Update governance artefacts with the new fingerprint (GAR manifest +
   attestation bundle), then re-admit the gateway and restore traffic. If ACME
   was disabled during adapter repair, re-enable it only after the runtime
   dependency passes its readiness check.
5. Resume paused pipelines and file the post-incident report within three
   business days.

### ECH Fallback / Degraded Mode

Use this play when CDNs or clients fail to consume ECH.

1. Detect via `torii_sorafs_tls_ech_enabled == 0`, customer incidents citing `GREASE_ECH_MISMATCH`, or governance directives.
2. Disable ECH:
   ```toml
   [sorafs.gateway.acme]
   ech_enabled = false
   ```
   Apply the config (or use `iroha_cli config apply`), then restart Torii. The `X-Sora-TLS-State` header should now advertise `ech-disabled`.
3. Broadcast the downgrade to storage operators, SDK teams, and governance, including the expected review window.
4. Monitor `torii_sorafs_tls_renewal_total{result="failure"}` for additional TLS churn while ECH remains disabled.
5. Once upstream services recover, use the reviewed runtime adapter to issue,
   validate, atomically install, and externally probe fresh ECH material. Rerun
   the self-cert harness, toggle `ech_enabled = true`, and share the updated
   payload-free telemetry snippets.

### Compromised-Key Revocation

Apply this playbook when private keys are exposed or the CA reports mis-issuance.

1. Keep automation disabled and rotate stream-token signing keys per the operations handbook to prevent credential reuse.
2. Withdraw the gateway immediately and revoke the certificate through the CA
   using the reviewed runtime adapter or its independently controlled emergency
   workflow. Do not archive a compromised private key as a fallback.
3. Destroy the compromised key under the KMS/HSM erasure procedure, issue and
   atomically install a fresh CA-valid bundle through the reviewed adapter, then
   follow the validation steps from the rotation playbook.
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
- Update the chaos drill tracker (`specs/sorafs_chaos_plan.md`) with drill participants, duration, observations, and follow-up tasks.
- File any automation regressions as roadmap follow-ups (SF-5, SF-7) with linked evidence.

## Troubleshooting

- **Automation log shows repeated DNS-01 failures:** verify IAM permissions for
  the configured `dns_provider` and confirm TXT propagation with
  `dig _acme-challenge.gateway.example.com txt`.
- **`X-Sora-TLS-State` missing or malformed:** keep the gateway withdrawn,
  verify that the runtime adapter completed its atomic install/reload, and that
  `Metrics::set_sorafs_tls_state` succeeds (check Torii logs for `warn` level
  failures).
- **GAR violation counters increasing:** inspect the structured logs emitted in
  `torii_sorafs_gar_violations_total`, remediate the offending provider or
  manifest, and notify governance before re-enabling traffic.
- **ECH clients still failing after toggle:** confirm caches were invalidated
  (Cloudflare/CloudFront) and share fallback hostnames with integrators.

## Implementation Notes

- **ACME client boundary:** `TlsAutomationHandle` accepts
  `Arc<dyn AcmeClient>`. The deployment embedding must supply and audit that
  implementation, including ACME account custody, challenge adapters, DNS
  credentials, revocation, durable retry state, and certificate installation.
  No provider library or DNS adapter is selected by this repository.
- **Serving boundary:** the in-tree controller records renewal state but does
  not authorize continued service on a stale or invalid certificate. The
  deployment must withdraw an affected gateway and may re-admit it only after
  the adapter's atomic installation and independent public probes succeed.
- **Telemetry naming alignment:** Observability approved the metric names in the
  telemetry table; durations are exposed in seconds. Review dashboards after
  upgrades to ensure schema changes are reflected.
- **Self-cert integration:** the deployment workflow must invoke
  `scripts/sorafs_gateway_self_cert.sh` after the injected client installs a
  renewal and before notifying operators. Torii does not claim that an
  unconfigured external workflow ran.
