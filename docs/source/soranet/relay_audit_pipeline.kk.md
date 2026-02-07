---
lang: kk
direction: ltr
source: docs/source/soranet/relay_audit_pipeline.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: aebd3de902c597f27a3830b598733f325f4775fcaa23d28b47ca1e5061748697
source_last_modified: "2025-12-29T18:16:36.193798+00:00"
translation_last_reviewed: 2026-02-07
title: SoraNet Relay Audit Pipeline
summary: Operating guide for shipping compliance events and tuning guard capacity.
---

## Overview

The relay runtime now emits compliance events whenever a handshake succeeds or
is rejected. Events land in two places:

- `log_path` – an on-disk JSONL file that rotates once it reaches the configured
  size (`compliance.max_log_bytes`, default 64 MiB)
- `pipeline_spool_dir` – per-event JSON blobs ready for downstream shipping

Every event contains a salted hash of the client address plus the negotiated
capabilities, making it safe to forward into shared audit infrastructure without
leaking raw network data.

Beyond the handshake lifecycle, the relay also records:

- `circuit_closed` — emitted when a QUIC circuit terminates, including the
  observed lifetime, negotiated padding/KEM set, restart state, and the
  post-close active circuit count.
- `exit_route_opened` — emitted whenever an exit adapter accepts a route,
  capturing the channel/route identifiers, access mode, optional padding
  budget, and salted hashes of the configured multiaddr and adapter target.
- `exit_route_rejected` — emitted when exit routing fails, preserving the
  stream type, channel (if known), and rejection reason so operators can
  correlate policy denials with downstream telemetry.

These events follow the same hashing rules for remote addresses and are written
to both the rotating JSONL log and the audit spool directory.

## Shipping the Spool Directory

Use `scripts/soranet_audit_spool_shipper.py` to batch spool files into archives
and hand them off to your audit transport. The script creates JSONL archives and
moves the original JSON files into a processed folder, so reruns pick up only
new events.

```bash
scripts/soranet_audit_spool_shipper.py \
  --spool-dir /var/spool/soranet/audit \
  --archive-dir /var/log/soranet/audit-archives \
  --processed-dir /var/log/soranet/audit-processed \
  --ship-command 'scp $SORANET_AUDIT_ARCHIVE audit@collector:/srv/audit/inbox'
```

The command runs once per archive; the example above uses `scp`, but the payload
is exposed via the `SORANET_AUDIT_ARCHIVE` environment variable so you can plug
in curl, aws-cli, or any bespoke transport. Add the script to a cron job or
systemd timer to run every few minutes.

### Dry Runs

Pass `--dry-run` to confirm which files would be archived without touching the
spool directory. This is useful when validating new shipping targets.

## Tuning Congestion Limits with Metrics

The metrics endpoint exposes the guard descriptor commitment alongside core
handshake counters and quota-aware telemetry:

- `soranet_handshake_success_total`
- `soranet_handshake_failure_total`
- `soranet_handshake_throttled_total`
- `soranet_handshake_capacity_reject_total`
- `soranet_handshake_throttled_remote_quota_total`
- `soranet_handshake_throttled_descriptor_quota_total`
- `soranet_handshake_throttled_cooldown_total`
- `soranet_handshake_pow_difficulty`
- `soranet_abuse_remote_cooldowns`
- `soranet_abuse_descriptor_cooldowns`

Download a snapshot and feed it into the helper script to receive tuning hints.

```bash
curl -sS http://relay.example:9090/metrics > /tmp/metrics.txt
scripts/soranet_guard_capacity_report.py \
  /tmp/metrics.txt \
  --mode entry \
  --max-circuits 12 \
  --handshake-cooldown 250
```

The report prints the observed counters and suggests whether to increase
`max_circuits_per_client` (to reduce capacity rejections) or lower the cooldown
(to reduce throttling) based on the ratios it observes. The quota-specific
metrics expose how many throttles were caused by per-remote and per-descriptor
limits, while the `soranet_abuse_*` gauges surface the number of active
cooldowns in effect. Combine these with the compliance spool, where each
`handshake_rejected` entry now includes an optional `throttle` object describing
the enforced scope, cooldown, burst limit, and observation window for downstream
analytics.

## CLI Overrides for Staging

When experimenting in staging environments, the relay binary now exposes knobs
to override JSON configuration values without editing the base files:

- `--max-circuits-per-client`
- `--handshake-cooldown-millis`
- `--compliance-pipeline-spool-dir`
- `--compliance-max-log-bytes`
- `--compliance-max-backup-files`

These flags are applied before the runtime validates the configuration, so the
same invariants (non-zero rotation size, valid paths) still hold.

## Operational Checklist

1. Ensure `/var/log/soranet/relay_compliance.jsonl` rotates and retains the
   expected number of backups (default seven).
2. Schedule the audit shipper to collect archives from
   `/var/spool/soranet/audit`.
3. Monitor `soranet_guard_descriptor_commit` to confirm guard manifests match
   the directory consensus.
4. Review capacity reports weekly and adjust relay flags as traffic scales.
