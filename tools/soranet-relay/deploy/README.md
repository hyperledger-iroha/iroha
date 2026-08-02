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
`specs/soranet_handshake.md`.

## Directory Layout

- `config/relay.entry.json` – sample relay configuration suitable for entry
  nodes. Edit the listen addresses, capability policy, proof-of-work settings,
  descriptor commitment, congestion limits, and compliance logging target
  before use. The `guard_directory` block points at the current
  `GuardDirectorySnapshotV2` emitted by the directory publisher; update the
  `snapshot_path` and `expected_snapshot_digest_hex` whenever a new consensus
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
   - Set `listen` to the desired public bind address. `admin_listen` must stay
     on a loopback address. Set `admin_auth_token_path` to a root/operator-only
     file containing 32–256 random printable ASCII bytes; the relay rejects
     group-writable or world-accessible token files. Export admin telemetry through a
     separately authenticated and encrypted local proxy when remote scraping is
     required.
   - Replace the TLS certificate paths or remove the `tls` section to use the
     built-in self-signed certificate during non-VPN development only. A relay
     with `vpn.enabled = true` must use durable certificate and private-key
     files. Its authenticated guard-directory certificate must authorize the
     exit role and a VPN-tagged endpoint whose exact TLS server name and leaf
     SPKI SHA-256 match those files; startup fails closed on any mismatch.
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
    configuration. First-release snapshots and relay certificate configuration
    always require both Ed25519 and ML-DSA-65 signatures; there is no
    validation-phase downgrade setting. Use the required
    `expected_snapshot_digest_hex` field to pin
    the domain-separated BLAKE3 digest printed by `soranet-directory build`.
    Distribute that digest over an independent governance channel; the
    snapshot's embedded `directory_hash` does not authenticate its issuer set.
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
  - Keep `pow.revocation_store_path` on durable storage writable only by the
    relay account. The sample systemd unit creates `/var/lib/soranet-relay`
    automatically. Startup fails closed if this replay ledger cannot be read or
    parsed; do not delete it while unexpired tickets exist.
  - When `pow.token.enabled` is true, keep
    `pow.token.replay_store_path` on the same class of durable storage. Active
    token records are never evicted; exhausted capacity rejects new token
    admissions, and malformed/over-capacity snapshots fail startup.
  - For VPN exits, place `vpn.helper_ticket_replay_store_path` on that same
    operator-protected durable volume and size
    `vpn.helper_ticket_replay_store_capacity` for the maximum number of
    simultaneously unexpired leases. Helper-ticket redemption is not accepted
    until this ledger is fsynced; corruption, write failure, lock contention,
    or capacity exhaustion fails closed. The ledger's persisted clock
    high-water mark also prevents a wall-clock regression from reopening an
    expired redemption. Do not delete or roll back the ledger while any
    recorded helper ticket remains valid.
   - Set the `compliance` block to match your log retention requirements. The
     default writes JSON Lines events to `/var/log/soranet/relay_compliance.jsonl`,
     rotates the file when it reaches 64 MiB (retaining seven backups), and mirrors
     each event into `/var/spool/soranet/audit` so downstream jobs can ship entries
     into the central audit pipeline. Remote hashes are salted so auditors can
     correlate events without exposing client identities. See
     `specs/soranet/relay_audit_pipeline.md` for automation tips.
3. Place the descriptor manifest (for example copied from
   `config/relay-descriptor-manifest.sample.json`) at
   `/etc/soranet/relay/secrets/relay-descriptor-manifest.json` with
   permissions `0640` and ownership restricted to the relay operator.
   Create `/etc/soranet/relay/secrets/admin-token` with at least 32 random
   printable ASCII bytes and permissions `0600`, or `0640` with a dedicated
   read-only relay group; do not place this bearer token in the JSON
   configuration or an environment variable.
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
7. Confirm the QUIC listener is reachable, query the loopback admin endpoint
   with its bearer token, and monitor logs via
   `journalctl -u soranet-relay`.

The relay requires `SNR1` protected records on every post-handshake application
stream. There is no plaintext-stream compatibility mode: clients must retain
the hybrid handshake session key and derive the direction- and stream-specific
ChaCha20-Poly1305 record keys. The shipped Sora VPN helper implements this
protocol. Relay QUIC endpoints reject TLS 0-RTT and the helper does not offer
it, so application traffic cannot precede authenticated hybrid key derivation.
The relay preloads its TLS identity and authenticated transport trust at
startup. VPN helper tickets that outlive the authenticated directory snapshot
are rejected, so rotate the snapshot, certificate, and TLS identity together
and restart the relay before the current trust interval expires.

## Using the Kubernetes manifests

The sample manifest targets the `soranet` namespace and runs one relay identity
behind a ClusterIP service. Its spent-ticket ledger is mounted from a
`ReadWriteOnce` persistent volume claim. A relay identity must have exactly one
authoritative ledger: scale out by deploying separate relay identities and
descriptors, not by cloning one identity across pods with independent replay
state. The sample uses the `Recreate` update strategy so a rollout cannot
overlap two processes using the same identity; the relay also takes an
exclusive process-lifetime sidecar lock on each replay ledger and fails startup
if another owner is active. Before applying it:

1. Replace the inline configuration under `ConfigMap.data["relay.json"]` with
   policy values produced by your directory tooling.
2. Replace the descriptor manifest under
   `Secret.stringData["relay-descriptor-manifest.json"]` with the identity seed
   issued for the relay.
   - Populate the ML-KEM keypair fields alongside the Ed25519 seed (`ml_kem_private_key_hex`
     and `ml_kem_public_hex`) so the runtime can advertise PQ capabilities.
   - Replace `Secret.stringData["admin-token"]` with at least 32 random
     printable ASCII bytes. The sample projects the secret with mode `0440`
     to the relay pod's dedicated `fsGroup`.
3. Mount the guard directory snapshot (for example via an additional `Secret`
   or CSI driver) at the path referenced by `guard_directory.snapshot_path` and
   update `expected_snapshot_digest_hex` whenever the committee publishes a new
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
5. Bind `soranet-relay-state` to durable storage appropriate for the cluster.
   The relay refuses startup if a persisted spent-ticket, consumed-token, or
   consumed VPN helper-ticket snapshot is unreadable or malformed.
6. Adjust the container image (defaults to
   `ghcr.io/sora-nexus/soranet-relay:latest`), resource requests, and security
   context as needed. The sample deliberately does not publish or probe the
   admin listener through the pod IP. For remote Prometheus collection, add a
   co-located scraper/proxy that reads the bearer token, talks to
   `127.0.0.1:9090`, and exposes metrics over an authenticated encrypted
   transport.
7. Apply the manifest:
   ```
   kubectl apply -f kubernetes/soranet-relay.yaml
   ```
8. Expose the QUIC service externally using your preferred ingress mechanism
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

## VPN backend daemon

When `vpn.backend_endpoint` is set in the relay configuration, `soranet-relay`
bridges helper-authenticated VPN traffic to that local privileged endpoint. The
default endpoint is the permissioned Unix socket
`unix:/tmp/sora-vpn-backend.sock`. TCP remains available as `tcp://host:port`,
but both the relay and backend must configure the same 32-byte
`vpn.backend_bootstrap_secret_hex` / `SORANET_VPN_BACKEND_BOOTSTRAP_SECRET_HEX`
value so bootstrap frames carry a valid keyed MAC. The `sora-vpn-backend`
binary in `tools/sora-vpn-backend` provides that relay-side bridge for Linux
hosts:

- It listens on `SORANET_VPN_BACKEND_ENDPOINT` (default
  `unix:/tmp/sora-vpn-backend.sock`).
- Unix-socket endpoints are chmodded to `0660` and peer credentials are checked
  against `SORANET_VPN_BACKEND_ALLOWED_UID` / `SORANET_VPN_BACKEND_ALLOWED_GID`
  (defaulting to the backend process uid/gid on Linux).
- TCP endpoints require `SORANET_VPN_BACKEND_BOOTSTRAP_SECRET_HEX`; bootstrap
  frames are Norito envelopes with timestamp, nonce, and keyed MAC, and the
  backend rejects stale timestamps, bad MACs, and replayed nonces.
- It derives a per-session Linux `tun` interface name from
  `SORANET_VPN_BACKEND_INTERFACE` (used as an interface prefix, default
  `svpn`).
- It receives the per-session tunnel addresses, session subnet routes, and MTU
  from the relay over the local bootstrap frame instead of relying on one fixed
  address plan.
- It can enable IP forwarding and per-session MASQUERADE rules using
  `iptables`/`ip6tables`.

Typical relay deployments should either:

1. Run `sora-vpn-backend` as a companion service on the same host and point
   `vpn.backend_endpoint` at its Unix socket, or at a TCP endpoint with a
   matching bootstrap secret when TCP is explicitly required.
2. Keep helper-ticket access disabled if the relay is not meant to terminate VPN
   traffic.

The backend now supports concurrent sessions on one daemon instance, but it
still relies on deterministic session-derived address allocation. If you need
strong collision-avoidance guarantees across large shared fleets, extend the
relay-to-backend bootstrap contract with an operator-assigned address pool
allocator before deploying it to multi-tenant infrastructure.

For paid VPN exits, set `vpn.receipt_spool_dir` to an operator-owned private
directory (for example `/var/spool/soranet/vpn-receipts`). When a helper-ticket
session closes after accepting a client usage voucher, the relay writes a JSON
settlement artifact containing the exact `relay_receipt_hex`,
`client_voucher_hex`, and `lease_id_hex` request body for
`POST /v1/vpn/receipts`. Its top-level `earned_fee` audit field is the canonical
exact XOR decimal string mirrored into the encoded relay receipt; it is never an
implicit nano-XOR integer. Submit that request with the configured operator
account, then sign the returned `SettleVpnLease` transaction instruction so the
earned XOR and refund are split from native custody. If no client voucher was
accepted, no settlement artifact is written; the relay logs this so the
operator does not accidentally settle an unverifiable prepaid claim.

To prepare the signed Torii request without storing operator signing material in
the repo, run the helper with runtime-only seed material and either submit the
JSON body/headers through your deployment runner or render a one-shot curl
command:

```bash
soranet-vpn-settlement \
  --artifact /var/spool/soranet/vpn-receipts/vpn-settlement-...json \
  --account-id "$VPN_OPERATOR_ACCOUNT_ID" \
  --private-key-seed-hex "$VPN_OPERATOR_PRIVATE_KEY_SEED_HEX" \
  --torii-root "$PUBLIC_TORII_ROOT" \
  --output curl
```

The helper signs the exact compact JSON body it prints. Do not edit the body
after signing; Torii verifies the body hash in the canonical request headers.

## Runtime endpoints and persistence

The relay exposes an authenticated admin HTTP listener at `admin_listen` for
operational telemetry and policy signals. The listener is accepted only on a
loopback address, and every route except the non-sensitive `GET /healthz`
requires `Authorization: Bearer <token>` using the secret loaded from
`admin_auth_token_path`:

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
