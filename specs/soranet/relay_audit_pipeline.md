---
title: SoraNet Relay Audit Pipeline
summary: Operating guide for shipping compliance events and tuning guard capacity.
---

## Overview

The relay runtime now emits compliance events whenever a handshake succeeds or
is rejected. Events land in two places:

- `log_path` – an on-disk JSONL file that rotates once it reaches the configured
  size (`compliance.max_log_bytes`, default 64 MiB)
- `pipeline_spool_dir` – per-event JSON blobs ready for downstream shipping

Every correlation identifier (client address, descriptor, circuit, exit-route
IDs, bandwidth measurement, relay, verifier, and adapter destination) is
recorded only as a field-domain-separated keyed BLAKE3 digest. Negotiated
capability labels and bounded stable reason codes remain in clear text.
Compliance logging therefore requires
`compliance.hash_salt_path` to name a direct, owner-private, single-link file
containing exactly 64 lowercase hexadecimal bytes with no newline. The key path
is redacted from debug output. The log and optional spool directories must be
pre-provisioned as owner-owned mode-`0700` directories; files are mode `0600`.

Beyond the handshake lifecycle, the relay also records:

- `circuit_closed` — emitted when a QUIC circuit terminates, including the
  observed lifetime, negotiated padding/KEM set, restart state, and the
  post-close active circuit count.
- `exit_route_opened` — emitted whenever an exit adapter accepts a route,
  capturing keyed hashes of the channel/route/stream/room identifiers, access
  mode, optional padding budget, and keyed hashes of the configured multiaddr
  and adapter target.
- `exit_route_rejected` — emitted when exit routing fails, preserving the
  stream type, keyed channel hash (if known), and stable rejection code so
  operators can correlate policy denials without persisting raw identifiers.

These events follow the same hashing rules for remote addresses and are written
to both the rotating JSONL log and the audit spool directory.

## Shipping the Spool Directory

Use `scripts/soranet_audit_spool_shipper.py` to batch spool files into archives
and hand them off to your audit transport. Pre-provision the spool, archive, and
processed directories as distinct absolute paths owned by the service account
with mode `0700`; event files must be direct, single-link `0600` regular files.
The shipper bounds every input and archive, publishes a fully synced archive
without clobbering, and moves the exact scanned JSON inode only after shipping
succeeds. The spool and processed directories must share a filesystem: the
move uses an atomic no-clobber hard-link publication and then removes the spool
name, rejecting replacements before, during, or after either read. An exclusive
lock on the validated spool-directory inode covers scanning, shipping, and
processed publication, so overlapping timer invocations fail instead of
shipping the same evidence twice.

```bash
scripts/soranet_audit_spool_shipper.py \
  --spool-dir /var/spool/soranet/audit \
  --archive-dir /var/lib/soranet/audit-archives \
  --processed-dir /var/lib/soranet/audit-processed \
  --ship-command /usr/bin/scp '{archive}' audit@collector:/srv/audit/inbox
```

The command runs once per archive. Its executable must be an absolute, trusted,
non-link path, and exactly one literal `{archive}` argument is replaced with the
archive path. No shell parsing or expansion occurs. The same path is also
available as `SORANET_AUDIT_ARCHIVE` for a purpose-built transport. The child
receives a minimal fixed environment; explicitly forward a required runtime
credential or socket with `--ship-env NAME` before `--ship-command`. Dynamic
loader and language-path variables are never forwarded. Add the script to a
cron job or systemd timer to run every few minutes.

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
`max_circuits_per_client` (within the configured `max_active_circuits` global
corridor) or lower the cooldown based on the ratios it observes. Global circuit
capacity failures remain fail-closed rather than allocating another remote
tracker or circuit-registry entry. The quota-specific metrics expose throttles
caused by per-remote limits, while the corresponding `soranet_abuse_*` gauge
surfaces the number of active cooldowns in effect. The retired per-descriptor
counter and gauge remain at zero for telemetry compatibility because the
descriptor is relay-static. Combine these with the compliance spool, where each
`handshake_rejected` entry now includes an optional `throttle` object describing
the enforced scope, cooldown, burst limit, and observation window for downstream
analytics.

The helper fails closed if the requested relay mode is absent or any of the four
core handshake counters are missing, and rejects non-positive circuit or
cooldown settings instead of producing a healthy-looking empty report.

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

1. Provision the log parent and optional spool directory with mode `0700`, and
   ensure `/var/log/soranet/relay_compliance.jsonl` rotates and retains the
   expected number of backups (default five).
2. Schedule the audit shipper to collect archives from
   `/var/spool/soranet/audit`.
3. Monitor `soranet_guard_descriptor_commit` to confirm guard manifests match
   the directory consensus.
4. Review capacity reports weekly and adjust relay flags as traffic scales.
