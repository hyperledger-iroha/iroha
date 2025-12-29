# SoraNet Relay Deployment Manifests

This directory provides opinionated deployment artefacts for the reference
`soranet-relay` daemon. The manifests are intended as starting points for
operators and integrators; review and harden them for your own environment
before production use.

The samples cover two common targets:

1. `systemd` units for bare-metal or VM deployments.
2. A Kubernetes `Deployment` with accompanying `ConfigMap`, `Secret`, and
   `Service` resources.

All manifests consume the same Norito JSON configuration file and rely on the
`RelayDescriptorManifestV1` secret format documented in
`docs/source/soranet_handshake.md`.

## Directory Layout

- `config/relay.entry.json` – sample relay configuration suitable for entry
  nodes. Edit the listen addresses, capability policy, proof-of-work settings,
  descriptor commitment, congestion limits, and compliance logging target
  before use. The `guard_directory` block points at the current
  `GuardDirectorySnapshotV2` emitted by the directory publisher; update the
  `snapshot_path` and `expected_directory_hash_hex` whenever a new consensus
  bundle is promoted so the relay can fail fast if its pinned descriptor
  diverges from the published directory.
- `config/relay-descriptor-manifest.sample.json` – companion manifest that
  supplies the Ed25519 identity seed. Store the real manifest as a secret.
- `systemd/` – unit file and environment example for Linux hosts.
- `kubernetes/soranet-relay.yaml` – namespace-scoped resources that mount the
  configuration and manifest into the container filesystem.

## Using the systemd unit

1. Install the `soranet-relay` binary (for example under `/usr/local/bin/`).
2. Copy `config/relay.entry.json` to `/etc/soranet/relay/relay.json` and edit
   the policy values:
   - Set `listen`/`admin_listen` to the desired bind addresses.
   - Replace the TLS certificate paths or remove the `tls` section to use the
     built-in self-signed certificate during testing.
   - Update the `descriptor_commit_hex` to match the directory-issued descriptor
     and point `descriptor_manifest_path` at the location of the private
     manifest. The sample assumes `/etc/soranet/relay/secrets/`.
  - Ensure the descriptor manifest contains both `ml_kem_private_key_hex`
    (2400-byte hex) and `ml_kem_public_hex` (1184-byte hex) so the relay can
    expose its PQ handshake identity for guard directory generation.
  - Place the latest guard directory snapshot (for example under
    `/etc/soranet/relay/guards/current_snapshot.norito`) and set the
    `guard_directory` block so the runtime can verify that the descriptor
    commitment and ML-KEM key published by the directory match the local
    configuration. Use the `expected_directory_hash_hex` field to pin the
    snapshot digest that the directory committee published.
  - Configure `guard_directory.pinning_proof_path` to a writable location
    (e.g., `/var/lib/soranet/relay/guard_pinning_proof.json`). After every
    successful validation the relay rewrites this JSON proof with the relay id,
    descriptor commit, directory hash, and ML-KEM key advertised in the
    snapshot so directory publishers and auditors can ingest deterministic guard
    pinning evidence.
    Publishers can point `soranet-directory collect-proofs` at the directory
    containing these files (for example,
    `soranet-directory collect-proofs --snapshot snapshots/current.norito --proofs-dir /var/lib/soranet/relay/evidence --out guard_pinning_summaries.json --overwrite`)
    to verify every submission against the committee-issued snapshot and export
    a JSON summary bundle for governance evidence packets. Directory publishers
    can also add `guard_pinning_proofs_dir` to the build configuration (or pass
    `--guard-proofs-dir` to `soranet-directory build`) so the snapshot compiler
    automatically collects and staples the verified proofs into the metadata
    without editing the JSON by hand.
  - Review the `congestion` block (`max_circuits_per_client`,
    `handshake_cooldown_millis`) and tune it for your operator policy.
   - Set the `compliance` block to match your log retention requirements. The
     default writes JSON Lines events to `/var/log/soranet/relay_compliance.jsonl`,
     rotates the file when it reaches 64 MiB (retaining seven backups), and mirrors
     each event into `/var/spool/soranet/audit` so downstream jobs can ship entries
     into the central audit pipeline. Remote hashes are salted so auditors can
     correlate events without exposing client identities. See
     `docs/source/soranet/relay_audit_pipeline.md` for automation tips.
3. Place the descriptor manifest (for example copied from
   `config/relay-descriptor-manifest.sample.json`) at
   `/etc/soranet/relay/secrets/relay-descriptor-manifest.json` with
   permissions `0640` and ownership restricted to the relay operator.
4. Optionally create `/etc/soranet/relay/relay.env` using
   `systemd/relay.env.example` to set `RUST_LOG` or other environment variables.
5. Copy `systemd/soranet-relay.service` to `/etc/systemd/system/` and adjust the
   user/group if running under a dedicated account.
6. Reload the systemd daemon, enable the unit, and start the service:
   ```
   sudo systemctl daemon-reload
   sudo systemctl enable soranet-relay
   sudo systemctl start soranet-relay
   ```
7. Confirm the QUIC listener and metrics port are reachable and monitor logs
   via `journalctl -u soranet-relay`.

## Using the Kubernetes manifests

The sample manifest targets the `soranet` namespace and runs two relay replicas
behind a ClusterIP service. Before applying it:

1. Replace the inline configuration under `ConfigMap.data["relay.json"]` with
   policy values produced by your directory tooling.
2. Replace the descriptor manifest under
   `Secret.stringData["relay-descriptor-manifest.json"]` with the identity seed
   issued for the relay.
   - Populate the ML-KEM keypair fields alongside the Ed25519 seed (`ml_kem_private_key_hex`
     and `ml_kem_public_hex`) so the runtime can advertise PQ capabilities.
3. Mount the guard directory snapshot (for example via an additional `Secret`
   or CSI driver) at the path referenced by `guard_directory.snapshot_path` and
   update `expected_directory_hash_hex` whenever the committee publishes a new
   consensus bundle. This ensures kube-managed relays refuse to start if the
   pinned descriptor (and ML-KEM key) diverge from the directory publisher’s
   artefacts. Point `guard_directory.pinning_proof_path` at a persistent volume
   so relays can keep the JSON evidence file the directory publisher ingests.
   Aggregating these artefacts no longer requires bespoke scripts—run
   `soranet-directory collect-proofs --snapshot <path> --proofs-dir <mounted-volume>`
   inside your build/publisher environment to verify every submission and emit
   the JSON summaries that governance expects.
4. Adjust the congestion/compliance blocks in the inline configuration to match
   your policy (limits propagate directly to the runtime guard counters and
   compliance logger).
5. Adjust the container image (defaults to
   `ghcr.io/sora-nexus/soranet-relay:latest`), resource requests, probes, and
   security context as needed.
6. Apply the manifest:
   ```
   kubectl apply -f kubernetes/soranet-relay.yaml
   ```
7. Expose the QUIC service externally using your preferred ingress mechanism
   (for example, a LoadBalancer Service or MetalLB) and map UDP port 4433.

Both deployment paths expect deterministic configuration management. Whenever
you rotate keys, tweak capability policy, or change proof-of-work difficulty,
replace the configuration/manifest files and restart the daemon to ensure the
relay advertises the updated settings. Use the `--max-circuits-per-client` and
`--handshake-cooldown-millis` CLI flags for rapid congestion tuning, and the
`--compliance-pipeline-spool-dir`/`--compliance-max-log-bytes` overrides when
experimenting in staging without editing the base JSON. Pair these with
`scripts/soranet_guard_capacity_report.py` and
`scripts/soranet_audit_spool_shipper.py` to analyse metrics and ship compliance
archives.

The metrics endpoint (`/metrics`) now exposes the guard descriptor commitment via
`soranet_guard_descriptor_commit{mode="…",commit="…"} 1`, making it easy to
validate that the pinning material served to clients matches the directory
consensus.

## Runtime endpoints and persistence

The relay exposes an admin HTTP listener at `admin_listen` for operational
telemetry and policy signals:

- `GET /metrics` returns Prometheus metrics for handshakes, constant-rate lanes,
  privacy counters, and incentive summaries.
- `GET /privacy/events` returns NDJSON privacy events and drains the buffer on
  read.
- `GET /policy/proxy-toggle` returns NDJSON downgrade/proxy-remediation events
  for downstream policy feeds.

Persistence surfaces to wire into your ops pipelines:

- Compliance logs are written to the configured JSONL path and mirrored into
  `compliance.pipeline_spool_dir` for shipper automation.
- Incentive snapshots can be enabled via `incentive_log.enable` and are written
  as Norito `.to` payloads under the configured spool directory (defaults to
  `artifacts/incentives/`).
