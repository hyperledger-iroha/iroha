# SoraNet Relay Deployment Manifests

This directory provides opinionated deployment artefacts for the reference
`soranet-relay` daemon. The manifests are intended as starting points for
operators and integrators; review and harden them for your own environment
before production use.

The samples cover two common targets:

1. `systemd` units for bare-metal or VM deployments.
2. A Kubernetes `Deployment` with accompanying `ConfigMap`, `Secret`, and
   dedicated persistent-volume resources.

All manifests consume the same Norito JSON configuration file and rely on the
`RelayDescriptorManifestV1` secret format documented in
`specs/soranet_handshake.md`.

The first-release loader admits relay configuration JSON up to 1 MiB, with at
most 8,192 entries in one collection, 32 levels of nesting, and 128 KiB in one
decoded string (the latter preserves the maximum legal GREASE payload). It
admits descriptor-manifest JSON up to 16 KiB, with at most 8 entries in one
collection, 16 entries overall, 4 levels of nesting, 8 KiB in one decoded
string, and 12 KiB of decoded strings in total. SRCv2 certificate bundles use
their protocol-defined 64 KiB maximum, while authenticated guard-directory
snapshots can be up to 5 MiB. All four inputs must be direct regular files:
symbolic links/reparse points and files
replaced or changed during a read are rejected. Publish rotations by writing a
complete bounded file and atomically renaming it into place before restarting
the relay; do not mutate a configured file in place while startup is reading
it.

## Directory Layout

- `config/relay.entry.json` – sample relay configuration suitable for entry
  nodes. Edit the listen addresses, capability policy, proof-of-work settings,
  descriptor commitment, certificate bundle and issuer keys, congestion limits,
  and compliance logging target before use. The exact-length repeated-byte
  issuer values are deliberately non-production placeholders. The
  `guard_directory` block points at the current
  `GuardDirectorySnapshotV2` emitted by the directory publisher; update the
  `snapshot_path` and `expected_snapshot_digest_hex` whenever a new consensus
  bundle is promoted so the relay can fail fast if its pinned descriptor
  diverges from the published directory.
- `config/relay-descriptor-manifest.sample.json` – companion manifest that
  supplies only the Ed25519 identity seed and raw ML-DSA-65 signing key. Its
  placeholders are intentionally invalid; replace both and inject the real
  manifest only as a runtime secret. Static ML-KEM fields are not part of this
  schema because the handshake encapsulates only to client-provided ephemeral
  KEM shares.
- `systemd/` – unit file and environment example for Linux hosts.
- `kubernetes/soranet-relay.yaml` – namespace-scoped resources that mount the
  configuration and manifest into the container filesystem.

## Using the systemd unit

1. Create the dedicated `soranet` user and group, then install the
   `soranet-relay` binary (for example under `/usr/local/bin/`). The unit does
   not use `DynamicUser` because the replay ledgers, compliance records, and
   guard-pinning evidence must retain one stable owner across restarts.
2. Copy `config/relay.entry.json` to `/etc/soranet/relay/relay.json` and edit
   the policy values:
   - Set `listen` to the desired public bind address. `admin_listen` must stay
     on a loopback address. Set `admin_auth_token_path` to a root/operator-only
     file containing 32–256 random printable ASCII bytes; the relay rejects
     any group or other Unix permissions on token files. Export admin telemetry through a
     separately authenticated and encrypted local proxy when remote scraping is
     required.
   - Configure durable TLS certificate/private-key files for every relay.
     Production has no self-signed transport path. The dual-signed relay
     certificate and authenticated guard-directory entry must select an exact
     endpoint whose TLS server name and leaf SPKI SHA-256 match the served leaf;
     startup fails closed on any absence or mismatch. VPN exits additionally
     require the signed exit role and VPN endpoint tag.
   - Update the `descriptor_commit_hex` to match the directory-issued descriptor
     and point `descriptor_manifest_path` at the location of the private
     manifest. Inline identity secrets are rejected. Give the manifest and TLS
     private key mode `0600`; symbolic links and group/other permissions fail
     startup. The sample assumes `/etc/soranet/relay/secrets/`.
   - Set `handshake.certificate.bundle_path` to the direct regular SRCv2 bundle
     issued for this relay. Replace the sample's repeated-byte issuer values
     with the bundle issuer's exact Ed25519 public key (64 lowercase hex
     characters) and ML-DSA-65 public key (3904 lowercase hex characters).
     Both issuer fields and both certificate signatures are mandatory; the
     configured keys must verify the same bundle that is embedded in the
     authenticated guard-directory entry.
   - Provision the descriptor manifest as exactly version 1 with both mandatory
     fields: `identity.ed25519_private_key_hex` (64 lowercase hex characters)
     and `identity.mldsa65_private_key_hex` (8064 lowercase hex characters).
     The signing keys must derive the Ed25519 and ML-DSA-65 identities in the
     relay's dual-signed certificate. Every field is mandatory and aliases or
     static ML-KEM fields are rejected as unknown. Relay-side KEM encapsulation
     uses the public shares supplied in each client handshake and requires no
     persistent relay KEM key pair.
     Inject this manifest at runtime from an owner-private secret store; never
     place private keys in the public relay configuration, an image, or a
     committed deployment file.
   - Place the latest guard directory snapshot (for example under
     `/etc/soranet/relay/guards/current_snapshot.norito`) and set the
     `guard_directory` block so the runtime can verify that the descriptor
     commitment and authenticated certificate bundle published by the directory
     match the local configuration. First-release snapshots and relay
     certificate configuration require both Ed25519 and ML-DSA-65 signatures. Use the
     required `expected_snapshot_digest_hex` field to pin the domain-separated
     BLAKE3 digest printed by `soranet-directory build`. Distribute that digest
     over an independent governance channel; the snapshot's embedded
     `directory_hash` does not authenticate its issuer set. A configured
     directory always requires an exact relay entry whose identity, descriptor
     commitment, and certificate match the local relay; startup fails if
     membership is missing or stale.
   - Configure `guard_directory.pinning_proof_path` to a writable location
     (the sample uses
     `/var/lib/soranet-relay/guard-pinning-proofs/relay.json`). After every
     successful validation the relay rewrites this JSON proof with the relay id,
     descriptor commit, directory hash, issuer fingerprint, validity window,
     and relay weights advertised in the snapshot so directory publishers and
     auditors can ingest deterministic guard pinning evidence. This evidence is
     sourced from the authenticated snapshot rather than the private manifest.
     A configured proof path is mandatory persistence: failure to create or
     atomically replace the proof makes relay startup fail closed.
     Publishers can point `soranet-directory collect-proofs` at the directory
     containing these files (for example,
     `soranet-directory collect-proofs --snapshot snapshots/current.norito --proofs-dir /var/lib/soranet-relay/guard-pinning-proofs --out guard_pinning_summaries.json --overwrite`)
     to verify every submission against the committee-issued snapshot and export
     a JSON summary bundle for governance evidence packets. Directory publishers
     can also add `guard_pinning_proofs_dir` to the build configuration (or pass
     `--guard-proofs-dir` to `soranet-directory build`) so the snapshot compiler
     automatically collects and staples the verified proofs into the metadata
     without editing the JSON by hand.
  - Review the `congestion` block (`max_circuits_per_client`,
    `max_active_circuits`, `handshake_cooldown_millis`) and tune it for your
    operator policy.
  - Keep `pow.revocation_store_path` on durable storage writable only by the
    relay account. The sample systemd unit creates `/var/lib/soranet-relay`
    automatically. Startup fails closed if this replay ledger cannot be read or
    parsed; do not delete it while unexpired tickets exist. V1 admits at most
    65,536 active records. Reads require a stable direct regular file and use
    explicit Norito allocation/depth limits; writes use a unique, bounded
    temporary file and atomically replace the durable snapshot.
  - When `pow.token.enabled` is true, keep
    `pow.token.replay_store_path` on the same class of durable storage. Active
    token records are never evicted; exhausted capacity rejects new token
    admissions, and malformed/over-capacity snapshots fail startup. The v1
    capacity ceiling is 65,536; the snapshot reader and atomic writer apply the
    same stable-file, decoder, and encoded-byte bounds as the ticket ledger.
    If `pow.token.revocation_list_path` is configured, its Norito JSON document
    must be a flat array of at most 8,192 unique 32-byte token IDs written as
    64 decoded hex characters each. The loader admits at most 4 MiB of raw JSON,
    512 KiB of aggregate decoded string data, and opens only a stable direct
    regular file (not a symbolic link or reparse point). The
    `soranet_admission_token revoke` command writes the same sorted, lowercase
    canonical form and refuses to grow a list beyond the entry cap.
  - For VPN exits, set `vpn.helper_ticket_issuer_public_key_path` to a direct,
    single-link, owner-private file containing exactly the VPN operator's
    Ed25519 public key as 64 lowercase hexadecimal characters, with no prefix,
    whitespace, or newline. This must be the public half of the Torii operator
    key that signs quotes and helper tickets; startup fails closed if the file
    is absent, mutable by another user, replaced during its bounded read, or
    not a canonical Ed25519 key. Provision that same public key at the local
    helper's fixed root-custodied trust-anchor path documented in
    `specs/soranet_vpn.md`.
  - Place `vpn.helper_ticket_replay_store_path` on that same
    operator-protected durable volume and size
    `vpn.helper_ticket_replay_store_capacity` for the maximum number of
    simultaneously unexpired leases. Helper-ticket redemption is not accepted
    until this ledger is fsynced; corruption, write failure, lock contention,
    or capacity exhaustion fails closed. V1 caps it at 65,536 active entries
    and uses stable direct-file reads, explicit decode limits, and unique
    bounded atomic replacement. The ledger's persisted clock
    high-water mark also prevents a wall-clock regression from reopening an
    expired redemption. Do not delete or roll back the ledger while any
    recorded helper ticket remains valid.
  - Keep `vpn.usage_voucher_max_age_ms` between the v1 bounds of 2,000 and
    30,000 milliseconds (default 5,000). The client helper emits an initial
    signed prepaid voucher before the relay connects its backend and refreshes
    it every second. Set `vpn.usage_voucher_setup_timeout_ms` at least as high as the
    freshness window and no higher than 120,000 milliseconds (default 30,000).
    The separate setup deadline bounds pre-service resource occupancy; after
    the first voucher, the relay closes the bridge when the next voucher misses
    the freshness deadline. `vpn.usage_voucher_credit_window_bytes` limits how
    far each signed ingress or egress ceiling may lead observed payload usage
    (256 KiB through 16 MiB; default 1 MiB), while the freshness window also caps prepaid active-time
    lead. The relay checks a complete packet and its active-time deadline before
    forwarding it, so no unvouched tail is provided. Client cells are also
    bound to the helper session's exact circuit/flow identifiers. V1 rejects
    client-originated cover/keepalive, empty or non-progressing data, and
    non-voucher control; it accepts at most 256 vouchers and caps cumulative
    client wire traffic at 64 times the signed ingress-payload ceiling.
   - Set the `compliance` block to match your log retention requirements.
     Compliance logging is disabled by the Rust configuration default and has
     no default log or spool path. The deployment sample explicitly enables
     JSON Lines output at `/var/log/soranet/relay_compliance.jsonl`, rotates at
     64 MiB with seven backups, and mirrors each event into
     `/var/lib/soranet-relay/audit-spool`. If `max_backup_files` is omitted, the
     code default is five. Remote hashes are salted so auditors can correlate
     events without exposing client identities. See
     `specs/soranet/relay_audit_pipeline.md` for automation tips.
3. Place the descriptor manifest (for example copied from
   `config/relay-descriptor-manifest.sample.json`) at
   `/etc/soranet/relay/secrets/relay-descriptor-manifest.json` with
   permissions `0400` or `0600` and ownership restricted to the relay operator.
   Replace both deliberately invalid sample placeholders; the loader
   rejects missing, extra, aliased, nested, uppercase, or all-zero identity
   material, including any static ML-KEM field, or either signing identity that
   differs from the authenticated relay certificate.
   Create `/etc/soranet/relay/secrets/admin-token` with at least 32 random
   printable ASCII bytes and permissions `0400` or `0600`; do not place this bearer token in the JSON
   configuration or an environment variable.
4. Optionally create `/etc/soranet/relay/relay.env` using
   `systemd/relay.env.example` to set `RUST_LOG` or other environment variables.
5. Copy `systemd/soranet-relay.service` to `/etc/systemd/system/` and adjust the
   user/group if using a different stable dedicated account. At service start,
   systemd creates `/var/lib/soranet-relay` and `/var/log/soranet` with mode
   `0700`; `ExecStartPre` creates the `audit-spool` and
   `guard-pinning-proofs` leaves under the state directory with the same mode.
   The unit's `UMask=0077` keeps newly created files owner-private. Do not move
   compliance or proof paths outside these managed directories without
   pre-creating their complete canonical parent chain with the relay user as
   owner and mode `0700` on each writable leaf.
6. Reload the systemd daemon, enable the unit, and start the service:
   ```
   sudo systemctl daemon-reload
   sudo systemctl enable soranet-relay
   sudo systemctl start soranet-relay
   ```
7. Confirm the QUIC listener is reachable, query the loopback admin endpoint
   with its bearer token, and monitor logs via
   `journalctl -u soranet-relay`.

The unit intentionally has no reload action because the relay does not support
SIGHUP configuration reload. Use `systemctl restart soranet-relay` after an
atomic configuration replacement. A normal stop/restart sends SIGTERM; the
runtime handles it through the same graceful QUIC endpoint-close path as
Ctrl-C.

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
behind a ClusterIP service. A dedicated `ReadWriteOncePod` claim supplies the
authenticated guard snapshot without subjecting its valid 5 MiB payload to the
1 MiB Kubernetes Secret limit. Replay state, compliance logs, and audit spool
use separate `ReadWriteOncePod` claims. This access mode prevents a second pod
anywhere in the cluster from mounting one identity's custody volume; the
selected CSI driver and cluster must support it. The claims survive pod
replacement and rollouts; retention after PVC deletion remains the selected
StorageClass's policy. A relay identity must have exactly one
authoritative ledger: scale out by deploying separate relay identities and
descriptors, not by cloning one identity across pods with independent replay
state. The sample uses the `Recreate` update strategy so a rollout cannot
overlap two processes using the same identity; the relay also takes an
exclusive process-lifetime sidecar lock on each replay ledger and fails startup
if another owner is active. Before applying it:

1. Replace the inline configuration under `ConfigMap.data["relay.json"]` with
   policy values produced by your directory tooling. In particular, replace
   both exact-length repeated-byte `handshake.certificate` issuer placeholders
   with the Ed25519 and ML-DSA-65 issuer public keys for the mounted SRCv2
   bundle. ConfigMap projections are symbolic links, so the init container
   copies `relay.json` into a memory-backed volume as an owner-readable direct
   regular file. Changes become active only after the pod is recreated.
2. Replace the descriptor manifest under
   `Secret.stringData["relay-descriptor-manifest.json"]` with the exact
   Ed25519 seed and raw ML-DSA-65 private key issued for the relay. Replace both
   deliberately invalid placeholders together; the relay accepts no aliases,
   extra metadata, or static ML-KEM fields. Both derived signing identities
   must match the authenticated relay certificate. Inject the completed value
   through a runtime Secret or external secret provider; never commit real keys
   to this manifest.
   - Replace `Secret.data["relay-certificate.cbor"]` with the base64 encoding
     of the exact binary bundle. `stringData` is used only for textual secrets;
     do not coerce binary CBOR bytes through UTF-8. The guard snapshot is not a
     Secret value and must be provisioned through its dedicated claim.
   - Replace `Secret.stringData["admin-token"]` with at least 32 random
     printable ASCII bytes. Because Kubernetes Secret projections are symlinks,
     the sample init container copies the `0400` projection into a fresh
     memory-backed `emptyDir`, changes ownership to the relay UID, and leaves
     direct `0400` files for the relay to read.
3. Populate the root of the `soranet-relay-guard-snapshot` claim with an exact
   binary file named `current_snapshot.norito`; do not put it in a Secret or
   ConfigMap. On first provisioning, create/bind the named claim from the PVC
   document before applying the remaining resources; the later full apply is
   idempotent for that claim. For first provisioning and every replacement:
   - Scale `deployment/soranet-relay` to zero and wait for its pod to terminate,
     then mount the claim read-write in one short-lived operator pod whose image
     is pinned by a verified `sha256` digest.
   - Copy the new snapshot to a temporary name on that volume, reject it if it
     exceeds 5,242,880 bytes, verify its domain-separated BLAKE3 digest against
     the value distributed through the independent governance channel, set
     mode `0400`, atomically rename it to `/current_snapshot.norito`, and run
     `sync` before unmounting the claim.
     Never overwrite the live inode in place.
   - Remove the writer pod, update
     `ConfigMap.data["relay.json"].guard_directory.expected_snapshot_digest_hex`
     to that verified digest, and scale the relay back to one. The init
     container rejects a missing, symbolic-link, or oversized source and copies
     the accepted bytes into the memory-backed direct-file volume before
     startup.
   This makes kube-managed relays refuse to start if the pinned descriptor or
   authenticated certificate bundle diverges from the directory publisher's
   artefacts. The sample writes `guard_directory.pinning_proof_path` to
   `/var/lib/soranet-relay/guard-pinning-proofs/relay.json` on the persistent
   state claim, so the evidence survives pod replacement.
   Aggregating these artefacts no longer requires bespoke scripts—run
   `soranet-directory collect-proofs --snapshot <path> --proofs-dir <mounted-volume>`
   inside your build/publisher environment to verify every submission and emit
   the JSON summaries that governance expects.
4. Adjust the congestion/compliance blocks in the inline configuration to match
   your policy (limits propagate directly to the runtime guard counters and
   compliance logger). The init container sets the compliance log and spool
   mount roots to relay-owned mode `0700`; startup fails closed if the storage
   backend cannot preserve Unix ownership and modes. Both are persistent in
   this sample. Size them for the configured log rotation and shipper backlog,
   and configure the StorageClass reclaim/backup policy explicitly.
5. Bind `soranet-relay-guard-snapshot`, `soranet-relay-state`,
   `soranet-relay-audit-spool`, and `soranet-relay-compliance-logs` to storage
   appropriate for the cluster. Persistent state backends must support fsync,
   atomic same-directory rename, single-link regular files, and stable Unix
   ownership/modes. The relay refuses startup if a persisted spent-ticket,
   consumed-token, or consumed VPN helper-ticket snapshot is unreadable or
   malformed. The pod deliberately has no `fsGroup`: its root init container
   sets the writable custody roots to UID/GID 65532 and mode `0700`, while the
   copied configuration/private leaves are mode `0400` beneath root-owned mode
   `0755` directories. Do not add a pod-level group policy that recursively
   widens persistent `0600` files during replacement. The init container drops
   every Linux capability, then adds only `CHOWN`, `FOWNER`, and `DAC_OVERRIDE`:
   these are required to repair ownership/modes and traverse custody volumes
   left owner-private by a previous relay pod. Do not expand this set; a storage
   backend that cannot be initialized under it is incompatible with the sample.
   Keep the relay's process-lifetime advisory locks even with
   `ReadWriteOncePod`, because storage access mode is not a substitute for
   fail-closed application ownership.
6. Replace both all-zero image digest placeholders with immutable, audited
   digests for the relay and init-container images; the unmodified manifest is
   deliberately non-deployable. Adjust resource requests and security context
   as needed. The sample deliberately does not publish or probe the
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

When compliance logging is enabled, `compliance.hash_salt_path` is mandatory and
must name an owner-private, direct, single-link file containing exactly 64
lowercase hexadecimal characters with no newline. The relay uses those 32 bytes
as the keyed BLAKE3 key for endpoint pseudonyms; never place the key inline in
the public relay configuration.

## VPN backend daemon

When `vpn.backend_endpoint` is set in the relay configuration, `soranet-relay`
bridges helper-authenticated VPN traffic to that local privileged endpoint. The
default endpoint is the permissioned Unix socket
`unix:/run/sora-vpn-backend.sock`. V1 accepts only
`unix:/absolute/path` endpoints so socket custody and peer credentials prevent
unprivileged local processes from consuming backend session capacity. An
enabled relay must set `vpn.backend_expected_uid` and
`vpn.backend_expected_gid` to the backend process's exact effective UID/GID.
The configured socket parent chain must be canonical and owned by root, the
relay user, or that pinned backend UID without unsafe write permissions. On
each connect the relay requires a direct, single-link socket with that exact
owner/group and no permissions for other users, pins its device/inode across
the connect, and verifies the peer credentials before sending any bootstrap
bytes. Configure the backend's `SORANET_VPN_BACKEND_ALLOWED_UID` /
`SORANET_VPN_BACKEND_ALLOWED_GID` reciprocally for the relay identity. Relay
and backend must also configure the same 32-byte secret through owner-private direct files referenced by
`vpn.backend_bootstrap_secret_path` and
`SORANET_VPN_BACKEND_BOOTSTRAP_SECRET_PATH` so bootstrap frames carry a valid
keyed MAC. The file contains exactly 64 lowercase hexadecimal characters, with
no whitespace and no all-zero placeholder. The `sora-vpn-backend`
binary in `tools/sora-vpn-backend` provides that relay-side bridge for Linux
hosts:

- It listens on `SORANET_VPN_BACKEND_ENDPOINT` (default
  `unix:/run/sora-vpn-backend.sock`). The socket parent chain must be owned by
  the backend user or root and must not be group- or other-writable.
- Unix-socket endpoints are chmodded to `0660` and peer credentials are checked
  against `SORANET_VPN_BACKEND_ALLOWED_UID` / `SORANET_VPN_BACKEND_ALLOWED_GID`
  (defaulting to the backend process uid/gid on Linux).
- The relay authenticates and reserves this local connection after validating
  the initial prepaid voucher but before writing the settlement WAL and
  redeeming the helper ticket. Bootstrap remains silent until durable admission
  succeeds, so endpoint failure leaves the ticket retryable.
- The endpoint requires `SORANET_VPN_BACKEND_BOOTSTRAP_SECRET_PATH`; the file
  must be direct, singly linked, owner-only, and contain exactly 64 hex
  characters. Bootstrap
  frames are Norito envelopes with timestamp, nonce, and keyed MAC, and the
  backend rejects stale timestamps, bad MACs, and replayed nonces. Nonces and a
  wall-clock high-water mark are durably reserved in the owner-private
  `--replay-directory` / `SORANET_VPN_BACKEND_REPLAY_DIRECTORY` (default
  `/run/sora-vpn-backend-replay`) before a bootstrap is accepted, so a backend
  restart or clock rollback cannot reopen the authentication window. The
  directory is created as `0700` beneath a trusted direct parent and is held by
  an exclusive process lock; pre-create it with backend-user ownership when the
  service does not run as root.
- It derives a per-session Linux `tun` interface name from
  `SORANET_VPN_BACKEND_INTERFACE` (used as an interface prefix, default
  `svpn`).
- It receives the per-session tunnel addresses, session subnet routes, and MTU
  from the relay over the local bootstrap frame instead of relying on one fixed
  address plan.
- When forwarding or NAT is enabled, it resolves and pins the family-specific
  egress interface. Per-session `iptables`/`ip6tables` rules accept only the
  exact assigned client address toward that egress and established/related
  return traffic from it; wrong interfaces and disabled forwarding families
  are drop-all. The V1 service is a public-Internet exit, not private-network
  access: protected IPv4/IPv6 destinations (including loopback, link-local,
  RFC 1918/ULA, shared, benchmark, documentation/test, multicast, reserved,
  IPv4-mapped, and local translation ranges) are rejected in packet validation
  and by higher-priority kernel rules. This remains enforced when a private LAN
  or metadata route shares the pinned default-egress interface. MASQUERADE is
  limited to the client's `/32` and `/128`.

Typical relay deployments should either:

1. Run `sora-vpn-backend` as a companion service on the same host and point
   `vpn.backend_endpoint` at its permissioned Unix socket with a matching
   bootstrap secret.
2. Keep helper-ticket access disabled if the relay is not meant to terminate VPN
   traffic.

The backend now supports concurrent sessions on one daemon instance, but it
still relies on deterministic session-derived address allocation. If you need
strong collision-avoidance guarantees across large shared fleets, extend the
relay-to-backend bootstrap contract with an operator-assigned address pool
allocator before deploying it to multi-tenant infrastructure.

For every enabled VPN exit, pre-create an absolute operator-owned private directory
with mode `0700` (for example `/var/spool/soranet/vpn-receipts`) and set
`vpn.receipt_spool_dir` to its canonical path. The relay rejects symlinks,
permissive modes, foreign ownership, and untrusted ancestors at startup and
refuses to enable VPN service when the spool is absent. It revalidates directory
custody for every write and holds an exclusive owner lock for its lifetime.
The relay consumes the one-use helper ticket durably immediately after the
authenticated application handshake, before circuit accounting or backend
work. Before any backend protocol/service, it fsyncs a distinct,
non-submit-ready per-session WAL containing a zero-service receipt. That WAL stays at zero during
live service: signed prepaid ceilings authorize forwarding but never prove that
bytes or time were delivered. Graceful close replaces the WAL with actual
observed usage; restart recovery promotes zero usage, so the relay absorbs
uncheckpointed work and a crash cannot overcharge the client. Every WAL create,
final promotion, and
removal fsyncs the owner-private `0600` file and parent directory; a persistence
failure poisons VPN service rather than falling back to volatile accounting.
Only final promotion writes a submit-ready JSON artifact containing the exact `relay_receipt_hex`,
`client_voucher_hex`, and `lease_id_hex` request body for
`POST /v1/vpn/receipts`. Its top-level `earned_fee` audit field is the canonical
exact XOR decimal string mirrored into the encoded relay receipt; it is never an
implicit nano-XOR integer. Settlement counters, service interval, and uptime
are relay-observed actual usage bounded by the highest client-signed prepaid
ceilings. Active duration comes from a monotonic clock, so wall-clock rollback
cannot erase forwarded service; `meter_hash` commits to the signed tariff and
unauthenticated cover telemetry is excluded.
The spool is capped at 8,192 total directory entries. There is no automatic
submission or retention worker: after successful Torii submission, archive or
remove the corresponding final JSON artifact using an operator-controlled
workflow. Startup and new writes fail closed at the ceiling.
A session that never supplies a fresh voucher is
closed at the configured deadline and produces no settlement artifact. Submit
an artifact with the configured operator
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
  --network-id "$(cat /absolute/path/to/genesis.expected_hash)" \
  --private-key-seed-file /run/secrets/vpn-operator-ed25519-seed.hex \
  --torii-root "$PUBLIC_TORII_ROOT" \
  --output curl
```

Keep the absolute runtime-only seed file outside every repository with
operator-only permissions (for example, mode `0600`). The helper rejects
symlinks and, on Unix, any group or other permission bits; on other platforms
it still requires a bounded direct regular file. It never accepts the seed
through process arguments and signs only `POST /v1/vpn/receipts` with no query.
The Torii root must be an absolute `http` or `https` origin with no credentials,
base path, query, or fragment, and its UTF-8 representation is capped at 4,096
bytes. The helper emits a compact, bounded JSON envelope and signs the exact
compact JSON request body inside it. Do not edit the body after signing; Torii
verifies the body hash in the canonical request headers.

## Runtime endpoints and persistence

The relay exposes an authenticated admin HTTP listener at `admin_listen` for
operational telemetry and policy signals. The listener is accepted only on a
loopback address, and every route (including `GET /healthz`) requires
`Authorization: Bearer <token>` using the secret loaded from
`admin_auth_token_path`. Body framing headers, folded/malformed fields, and
duplicate authorization fields are rejected rather than interpreted:

- `GET /metrics` returns Prometheus metrics for handshakes, constant-rate lanes,
  privacy counters, and incentive summaries.
- `GET /privacy/events` returns NDJSON privacy events and drains the buffer on
  read.
- `GET /policy/proxy-toggle` returns NDJSON downgrade/proxy-remediation events
  for downstream policy feeds.

First-release privacy telemetry admits at most 16,384 events per in-memory
queue, 256 simultaneously open or completed buckets, 256 GAR category hashes
per bucket, and 256 bytes of detail per event. Configuration above those
ceilings is rejected; bounded JSON/Prometheus rendering drains or fails closed
without cloning the full retained queues.

Each admission quota tracker retains at most 65,536 remote or descriptor
entries. Higher base or per-hop `max_entries` settings are rejected at startup;
once a tracker is full, previously unseen identities fail closed until expired
entries are reclaimed, without materializing a full key snapshot for eviction.
Attacker-influenced downgrade and token-outcome metric labels inspect at most
512 bytes, retain at most 64 bytes per label, and keep 256 distinct series per
family; excess cardinality is accumulated in one deterministic `other` series.
The congestion controller defaults to 4,096 globally active circuits and has a
first-release hard ceiling of 65,536. Global capacity is checked before a new
remote is retained; releasing a circuit atomically returns both its global slot
and its per-client tracking state. The circuit registry independently enforces
the same configured ceiling and uses fallible allocation before retaining a
negotiated circuit.

Persistence surfaces to wire into your ops pipelines:

- Compliance logs are written to the configured JSONL path and mirrored into
  `compliance.pipeline_spool_dir` for shipper automation.
- Incentive snapshots can be enabled via `incentives.enable` and are written
  as Norito `.to` payloads under the configured spool directory (defaults to
  `artifacts/incentives/`). The accumulator defaults to 16 active epochs and
  4,096 measurement IDs per epoch, with hard ceilings of 256 epochs and 65,536
  IDs in aggregate; configuration whose product exceeds the aggregate ceiling
  is rejected. Snapshot encoding is capped at 4 MiB, the digest cache retains
  only the configured newest epoch window, and Prometheus rendering uses a
  fallibly reserved response corridor.
