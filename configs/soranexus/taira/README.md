# Sora Taira public NPoS bootstrap

Taira is the Sora Nexus public testnet. This directory
contains the repo-shipped bootstrap bundle for a public, stake-elected NPoS
deployment.

## Topology model

Taira keeps three independent topology layers:

- A **dataspace** is a physical security, execution, and storage boundary backed
  by its own server/validator cohort. A catalog entry is valid only when that
  distinct deployment exists; a workload name alone must not create one.
- A **lane** is a logical execution and routing stream inside exactly one
  dataspace. Several lanes may share the same physical dataspace.
- A **namespace** is an independently governed naming scope that is bound to a
  dataspace. Reusing a word in a namespace and a lane alias does not merge
  either identity with the dataspace.

The canonical Taira mapping is:

| Lane | Owning dataspace |
|------|------------------|
| `core` | `universal` |
| `governance` | `universal` |
| `zk` | `universal` |
| `dpn` | `dpn` |
| `external-poc` | `is` |
| `boi-mobile` | `is2` |
| `cbsi` | `cbsi` |

Consequently the physical dataspace catalog is exactly `universal`, `dpn`,
`is`, `is2`, and `cbsi`. `governance` and `zk` are workload lanes in the
`universal` dataspace, not physical dataspaces. Namespace bindings such as
`boi` in `boi.is2` remain namespace-to-dataspace bindings and do not add a
lane or dataspace.

## Universal offline capability and BOI dataspaces

Offline cash is an application/device protocol, not a Taira deployment mode.
Every compatible Iroha validator exposes ABI-21/V4 and `cash_handoff_v1`.
There is no backend enable flag, asset `offline.enabled` metadata, configured
escrow catalog, or offline-specific health/admission gate. `/health` and
`/readyz` describe ordinary validator and consensus readiness only. Missing or
invalid material referenced by a particular top-up or redemption operation is
reported as that transaction's validation result.

The checked-in topology declares both BOI dataspace bindings: `is` for the
scenario browser and `is2` for the mobile wallet. The declarations do not by
themselves prove that either physical cohort is deployed; release evidence must
bind each one to its own manifest and server/validator set. Their logical lanes
do not change this capability contract. The mobile product may expose offline
UI while the scenario browser does not; validators in either physical cohort
run the same universally capable Iroha software.

## Network identity

- Public Sumeragi-v2 chain ID: `fc56984b-2be7-431d-840e-21514d1883f0`
- Archived pre-v2 chain ID: `809574f5-fee7-5e69-bfcf-52451e42d50f`
- Address chain discriminant: `369` (this is what drives canonical I105 literals such as `testu...`)
- Consensus protocol: Sumeragi v2 state machine, wire revision 4 only (`wire_protocol_version = 4`)
- Timing profile: authoritative 4,000 ms block cadence and one absolute 40,000 ms view-zero round deadline
- Candidate bounds: 96 transactions, 16 MiB canonical body, and a four-times bounded queue scan
- Role/mode boundary: each validator config says `role = "validator"`; NPoS mode and DA/chunk
  geometry come from signed genesis, not a mutable local mode or RBC selector

The checked-in `genesis.json` is an unsigned deployment template. Its
`nexus_amx_context_hash` is the config-only projection and its
`execution_policy_hash` is a non-deployable template value. The private bundle
renderer must supply the final per-dataspace manifests, validator rosters, and
runtime signer bindings; the deployment signer then replaces both commitments,
refreshes the fingerprint, and signs the resulting exact manifest. Never treat
the source template's execution-policy value as proof of the deployed policy.

The v2 chain is a fresh-genesis reset. Never point a v2 validator at the archived chain's Kura,
queue journal, or RBC session directories, and never attempt a mixed v1/v2 rolling upgrade. Keep
the archived chain data read-only for incident analysis.

`check_mcp_rollout.sh` derives its default expected chain ID from this
directory's canonical `config.toml`, so an archived-chain signer fails closed
by default. When an operator deliberately restores the archived testnet, keep
the canonical config and examples unchanged and pass the deployed identity
explicitly:

```bash
--expected-chain-id 809574f5-fee7-5e69-bfcf-52451e42d50f
```

The override is enforced when the rollout checker prepares the signed-canary
config and when it derives the canary account for faucet recovery.

## Public API contract

For the examples below, replace `PUBLIC_TORII_ROOT` with the live public
Torii URL you are validating. On the current deployment that is:

- `PUBLIC_TORII_ROOT=https://taira.sora.org`

Validator-specific hostnames such as `https://taira-validator-1.sora.org` may
also exist as alternate public Torii roots when the edge is configured to
expose them directly.

Use the exact public root the deployment or operator gives you. Do not
silently swap `https://taira.sora.org` for a guessed direct-validator host,
and do not infer validator-set size from `/status.peers`: that field is the
queried node's current remote-peer count, not the chain's validator-set size.

`https://taira.sora.org` is the shared Torii origin, not an app website. Do
not bind product frontends to that host root through `sorafs_sites.json`.

For day-to-day validation, prefer the first-class CLI:

- `iroha taira doctor --public-root https://taira.sora.org --output-format text`
- `iroha taira write-canary --public-root https://taira.sora.org --output-format text`

`check_mcp_rollout.sh` remains the fuller rollout harness for operators who
need local/public comparisons, curl `--resolve` overrides, or shell-only
environments. The CLI is the blessed single-node Taira devex path and emits a
redacted JSON receipt with `--json`.

The shipped public Taira profile pins the first-release Torii posture in
config rather than wrapper-local defaults:

- `torii.max_content_len = 1_073_741_824`
- `torii.deploy_rate_per_origin_per_sec = 4`
- `torii.deploy_burst_per_origin = 8`
- `torii.query_rate_per_authority_per_sec = 160`
- `torii.query_burst_per_authority = 320`
- `torii.webhooks_enabled = false`
- `torii.zk_attachments_enabled = false`

The co-located-validator storage contract is also explicit:

- `nexus.storage.local_budget_bytes = 68_719_476_736` (64 GiB per validator)
- Kura 74.99%, WSV snapshots 20%, SoraFS 0.01%, SoraNet spool 2.5%, and
  SoraVPN spool 2.5%

SoraFS storage is disabled on this profile; its one-basis-point share is the
minimum parser-valid reserve and does not enable the service. Do not remove the
aggregate budget or reassign that reserve without rerunning the free-space and
fsync preflight; near-full shared-host storage can turn restart durability
barriers into multi-second stalls.

## Included artifacts

- `config.toml`: baseline validator config for peer 1 and the shared template
  source for rendered per-validator configs. The checked-in file is template
  only and intentionally does not carry runtime-only private keys.
- `validator_roster.example.toml`: copy-me roster template for all validator
  public addresses, public keys, and PoPs. Keep the populated file user-local.
- `validator_secrets.example.toml`: copy-me runtime template for per-validator
  BLS private keys, dedicated SoraNet Ed25519 transport key pairs, and dedicated
  Torii secp256k1 submission-receipt key pairs, shared
  onboarding/faucet authority and streaming identity key
  material, the public identity of the provider-backed Soracloud mutation
  signer, plus the public SoraFS admission-council roots and quorum. Keep the
  populated file user-local. The Soracloud provider owns its private key; only
  its credential-free handle and authenticated public binding belong here.
  At startup and before use, `irohad` resolves that provider and fails closed
  unless its handle, authority key, revision, and public-policy digest exactly
  match the rendered binding.
- `genesis.json`: NPoS genesis with DA enabled.
- `privacy_bootstrap_plan.json`: public, secret-free first-release privacy
  activation contract. It fixes the exact-12 order, retired labels, genesis
  authority and heights, and the stock broker slot used by the designated
  Bootle/Lantern issuer. Empty digest fields are an intentional staging gate,
  not placeholders that a validator may ignore.
- `validate_privacy_bootstrap.py`: strict staging/release validator for the
  privacy plan, genesis authority, validator config, exact-12 matrix, canonical
  broker public export, encoded instruction inventory, and complete
  provider/issuer-policy bindings. Release mode requires `--broker-public`.
- `privacy_bootstrap_validation_test.py`: negative and adversarial bootstrap
  tests, including SIS alias injection, partial activation inventories,
  governance-grant substitution, provider drift, dormant bindings, malformed
  base64, duplicate JSON keys, and symlinked input.
- `dns_records.json`: DNS targets for the convenience host, explorer host, and
  direct per-validator Torii hostnames.
- `explorer.runtime-config.json`: runtime config example for the Explorer
  frontend; point it at the explicit public Torii base URL you want the UI to
  query.
- `sorafs_sites.json`: optional host-to-manifest bindings for Torii-served static sites. Keep `taira.sora.org` out of this file. Enable it only through the rendered validator config's `[sorafs.gateway.site_bindings]` table; Torii reads, validates, and caches the document once at startup.
- `taira-irohad.service`: sample systemd unit that starts the validator from
  the shipped Taira config and genesis.
- `taira-bootle-lantern-broker.service`: peer-1-only systemd unit for the
  native slot-56 issuer broker. It loads exactly three encrypted systemd
  credentials into the unit's private credential directory, publishes the
  fixed local socket as the same `iroha` UID as Torii, and does not restart
  past an uninspected crash.
- `taira-bootle-lantern-broker.env.example`: public reviewed policy-digest
  latch for the broker unit. It contains no credential material.
- `taira-irohad-peer1-privacy.conf`: peer-1-only systemd drop-in. Its
  `BindsTo`/`After` contract prevents Torii from running after loss of its sole
  local issuer broker and waits for broker readiness during startup.
- `wait_taira_bootle_lantern_broker_ready.sh`: bounded same-UID, mode, type,
  link-count, and parent-directory readiness gate for the fixed broker socket.
- `taira-irohad.env.example`: sample `/etc/default/taira-irohad` overrides for
  pointing the systemd unit at a rendered validator config.
- `docker-compose.validator.yml`: sample containerized validator deployment
  for local development; it mounts one rendered validator config plus
  persistent `/storage` and is not a first-release publication target.
- `docker-compose.peer1-privacy.yml` and
  `taira-validator-peer1-privacy.sh`: opt-in peer-1 container override and
  two-process fail-together supervisor. The broker and validator have numeric
  UID/GID 1001, share only the hardened pathname socket, and receive three
  read-only bind-mounted credential files. The ordinary compose file remains
  broker-free for peers 2–4.
- `taira-validator-container.compose.env.example`: sample compose env file for
  a local containerized validator host using an explicitly built image.
- `taira-validator-container.sh`: plain-`docker` wrapper for hosts that do not
  have the Docker Compose plugin installed.
- `taira-validator-container.service`: sample systemd wrapper that keeps the
  validator container under service management without requiring Docker Compose.
- `scripts/render_taira_localnet_container_bundle.py`: rewrites a fresh
  `kagami localnet` bundle into four container-ready configs/env files with
  canonical `addr:...#CRC16` literals for shared-bridge Docker validation.
- `taira-canary-client.example.toml`: runtime-only example signer config for
  the signed rollout canary.
- `build_taira_rollout_bundle.sh`: produces an unsigned archive from the exact
  checked-out `irohad` /
  `iroha` / `sorafs_manifest_builder` / `sorafs_tx_stdin_builder` build plus the
  checked-in Taira config bundle into one timestamped rollout artifact. It
  builds `irohad` with the exact production
  `embedded-soracloud-runtime,zk-stark` features, separately builds the native
  privacy evidence runner with `privacy-release-evidence`, produces and
  re-verifies native evidence only after the validator build, rejects an
  incomplete privacy bootstrap before building a release, runs the focused
  SoraSwap regressions, and records the frozen workspace-source identity plus
  release checks in `rollout.manifest.json`. It never accepts or inherits a
  release signer, signing key, trusted fingerprint, or native manifest
  verifier.
- `scripts/finalize_taira_rollout_authority.py`: authenticates one completed
  unsigned Linux archive and creates its portable signed authority in a
  separate post-build process. It never invokes Cargo, Git, Kagami, the
  validator, the evidence runner, or another source-built executable; its only
  child executables are the reviewed external signer and checksum-pinned
  `sorafs-validate`. It verifies the root-owned controller closure and includes
  that closure's canonical manifest in the signed authority.
- `scripts/seal_taira_release_controllers.py`: is the source used by trusted
  configuration management to build the versioned release-controller bundle.
  The workflow never installs or refreshes that bundle. Each authority runner
  is preprovisioned with the fixed root-owned launcher
  `/usr/local/libexec/iroha-taira-release-controller-v1`, the immutable closure
  below `/usr/local/libexec/iroha-taira-release-controller-v1.d`, and a
  canonical root-owned host/role installation record. The launcher attests
  those installed bytes and then exposes only role-specific operations with
  exact flag contracts; it never accepts a script path, shell, or unrestricted
  privileged argument vector.
- `scripts/render_taira_edge_nginx_conf.py`: renders the shared-edge nginx
  config directly from the same validator roster used for per-validator
  `config.toml` generation so public Torii ingress cannot drift onto stale
  loopback ports.
- `scripts/deploy_taira_v21_reset.py`: performs the authenticated four-validator
  fresh-reset cutover and requires a fresh lowercase 64-hex
  `--restart-generation`. It emits identity-scoped terminal-unhealthy paths
  for all four supervisors and fails immediately if any current-generation
  marker appears during initial health, consensus advancement, or the child
  restart proof. On macOS it authenticates native NUL-delimited process argv
  rather than `ps` rendering and launches new supervisors through the exact
  validated root-controlled Python.app executable. The emergency
  `--allow-framework-python-argv0-rewrite` option is only for migrating a
  legacy testnet Homebrew supervisor whose same-framework Python.app rewrite,
  remaining argv, parent/UID, child, and rollback identity all match exactly;
  it is refused by default.
- `scripts/deploy_taira_testnet_update.py`: is the small routine-update
  controller for this testnet. A preinstalled root-owned copy changes only the
  content-addressed `iroha3d` path and its supervisor stat seal, restarts one
  peer at a time, and preserves every live config, genesis, working directory,
  and storage inode. It is deliberately separate from release admission and
  fresh-reset tooling.
- `scripts/migrate_taira_peer_supervision.py`: creates a sealed, read-only
  adoption plan for an existing four-peer macOS deployment, then performs an
  explicitly confirmed maintenance-window cutover from `run-canonical.sh` or
  `launchd-run.sh` to four independent launchd jobs without replacing storage.
- `scripts/taira_peer_supervisor.py`: launchd-owned single-validator restart
  loop used by the migration. It guards the planned binary, config, and storage
  identities and caps exponential child-restart backoff. Three consecutive
  identical normalized fatal exits inside the rapid-start window atomically
  publish a bounded mode-0600 terminal-unhealthy fingerprint and leave the
  supervisor alive without spawning a fourth child. The marker contains only
  its schema, hit count, and SHA-256 fingerprints; it never contains child
  stderr, commands, config contents, keys, or paths.

  The live reset controller may hash a separately root-controlled validator
  binary once and seal its device, inode, size, mtime, and ctime into every
  plist. Each child restart validates that all-or-none seal through a no-follow
  descriptor in O(1). This fast path requires the binary and every path
  component from `/` to be root-owned, non-symlink, and not group/world
  writable, so the runtime user cannot swap the pathname after descriptor
  validation.

  Generic storage adoption automatically emits the same complete stat seal
  when `--irohad` resolves through an entirely root-controlled path. If the
  validator remains below its existing non-root-owned deployment base, its
  generated plists omit all five fields and hash the complete binary before
  each child start. Any partial seal fails closed. Config files remain
  content-hashed in both modes because they are small and operator-editable.
- `check_mcp_rollout.sh`: smoke script for the local and public `/v1/mcp`
  checks used by the Taira Codex rollout, with wire-revision-4 reducer health read
  from an exact-NetworkId operator-signed `/v1/sumeragi/status` request and an
  optional signed write canary for final public cutover. It verifies ordinary
  node/consensus health, the common `is` and `is2` dataspace catalog, and
  universal ABI-21 `cash_handoff_v1` discovery. It never treats an asset,
  application proof release, or device UI state as a validator admission
  condition.
- `check_sorafs_rollout.sh`: public SoraFS surface + signed capacity-declaration
  canary that catches stale validators still missing the capacity/order ISI
  dispatch table.
- `check_sorafs_rollout_mock_test.sh`: local mock regression suite for the
  SoraFS rollout smoke, covering read-only mode, implicit bootstrap, explicit
  signer-config preservation, sponsored bootstrap, faucet retry,
  stale-validator dispatch errors, capacity-state visibility failures,
  `/status` and Sumeragi commit-QC/finality health failures, and bounded HTTP
  timeout controls without mutating public Taira state.
- `clear_volatile_consensus_state.sh`: archived v1 incident-evidence helper.
  Do not use it to recover a v2 deployment; v2 preserves its safety WAL and
  fails the rollout instead of clearing consensus state.
- `verify_soraswap_rollout.sh`: post-upgrade wrapper that first runs the local
  `iroha_core` SoraSwap deploy-route router regression and three-hop nested
  transfer authority canary, then runs the public MCP canary, the SoraFS capacity canary, the SoraSwap
  nested-call probe, and the optional `deploy-testnet` / signed
  `smoke-testnet` / `release-checklist` chain in the canonical order.
- `bootstrap_kaigi_localnet.sh`: local-only relay bootstrap that re-signs the
  served `dist/taira-localnet` genesis with seeded Kaigi relay metadata,
  health samples, and one shared local onboarding/faucet signer account, then
  rewrites the live peer configs and restarts the detached
  `taira-localnet` session.
- `taira-explorer.nginx.conf`: example rendered multi-domain nginx edge config
  for `taira.sora.org`, `taira-explorer.sora.org`, and the current
  `taira-validator-{1,2,3,4}.sora.org` direct-hostname layout on a shared host.

## Native privacy release evidence

### First-release activation and issuance bootstrap

The checked-in genesis grants the exact genesis authority
`CanEnactGovernance`, but deliberately carries no partial privacy activation
set. A release renderer must obtain all 12 compiled profiles from one
qualified native candidate and encode exactly 12
`RegisterPrivacyProtocolActivationV1` instruction boxes in canonical matrix
order. Every record starts as `Proposed` at genesis height 1, schedules height
301, and uses the protocol's canonical compiled-profile digest and bounds.
The 300-block delay is consensus-enforced; no bootstrap shortcut or already
active lifecycle is accepted.

`sis-with-hints`, `sis-hints-anoncred-pq-v0`, and the other retired labels are
retirement evidence only. They have no activation row, compatibility alias,
or fallback engine. Jindo, ZK-AMS, Vega, and the other canonical rows are the
only release identities.

Bootle/Lantern issuance uses stock local runtime-provider broker slot 56. The
deployment-owned broker backend supplies opaque bearer authentication and
bounded native Falcon operations. Torii remains the sole durable one-shot
authorization-state authority. Issuer trapdoors, bearer credentials, backend
endpoints, and credential loaders are forbidden from the plan, genesis,
validator config, and rendered bundle.

Only `taira-validator-1` is the first-release issuance endpoint. The edge must
route both issuance paths to that validator rather than round-robin them
across the fleet, because its Torii store is intentionally local:

- `/v1/privacy/bootle-lantern/issuance/authorize`
- `/v1/privacy/bootle-lantern/issuance/issue`

The shared config keeps issuance disabled. Enabling the peer-1 rendered config
requires the explicit first-release admission bound `max_inflight = 2` (the
field has no enabled-mode default) and all of the following public material
from the qualified deployment provider: its exact non-zero
qualification-policy digest, the matching Falcon public issuer matrix and
issuer-parameter identity/digest, the governed issuer-policy record and record
digest, and the canonical Norito registration instruction. The issuer-policy
instruction follows the 12 activation instructions in genesis. Other
validators remain disabled and must contain no dormant issuer/provider binding
fields.

The native `taira_bootle_lantern_broker export-public` command is the sole
producer of that public issuer material. Its JSON contains the complete Falcon
public matrix and governed policy, parameter identity and digest, policy
record digest, provider qualification, stable public principal commitment,
and a canonical boxed registration instruction. The
`registration_instruction_norito_hex` value is the exact canonical
`InstructionBox` payload consumed by genesis; the matching
`registration_instruction_norito_sha256` is SHA-256 over those exact bytes.
Neither field is a bare instruction encoding.

Release composition uses Kagami's native
`privacy-bootstrap render-taira-release-v1` command. The operator supplies one
reviewed broker `export-public` JSON document as an explicit input; the
composer consumes it as an immutable snapshot and emits four fresh,
peer-1-enabled, secret-free artifacts: the privacy plan, validator config,
genesis, and a byte-identical verified broker public export. The plan's
`bootle_lantern_issuer.public_export_sha256` binds that fourth artifact; it is
null in staging and mandatory in release. Use the emitted validator config as
the `config.toml` input to `render_taira_validator_bundle.py`: it is the exact
peer-1 release template. The bundle renderer preserves that complete binding
only for `taira-validator-1`; it deterministically disables issuance on peers
2–4, removes every dormant issuer/provider binding there, and assigns each
peer its own issuer state directory. Partial or fleet-wide bindings fail
closed. Do not manually splice matrix rows or individual digests, and do not
treat an unreviewed path as a bundle default. Run the command with `--help`
from the exact release binary for its required input/output arguments, then
run the release validator over the four emitted files before packaging:

```bash
kagami privacy-bootstrap render-taira-release-v1 --help
python3 configs/soranexus/taira/validate_privacy_bootstrap.py --mode release \
  --plan /absolute/reviewed/privacy_bootstrap_plan.json \
  --config /absolute/reviewed/taira-validator-1/config.toml \
  --genesis /absolute/reviewed/genesis.json \
  --broker-public /absolute/reviewed/broker-public.verified.json
```

For the bare-metal peer-1 process, install the bundled broker binary and the
four systemd artifacts listed above. Install the drop-in only as
`/etc/systemd/system/taira-irohad.service.d/peer1-privacy.conf` on
`taira-validator-1`; peers 2–4 must not install or enable the broker unit.
Copy the example public environment file to
`/etc/default/taira-bootle-lantern-broker`; set `TAIRA_NETWORK_ID` from the
deployment's mandatory genesis expected hash and replace its two digest latches
from the same reviewed export. The export's `network_id` must equal that value.

Provision the issuer seed (exactly 32 random bytes), opaque bearer (32–4096
random bytes), and stable-principal seed (exactly 32 independent random bytes)
as three separately encrypted systemd credentials under
`/etc/credstore.encrypted/`. Use the exact credential names in the unit and
bind encryption to the deployment host/TPM with `systemd-creds encrypt`.
Plaintext provisioning files are not rollout-bundle inputs and must be removed
through the operator's approved secret-destruction workflow after encryption.
At activation, the broker accepts only singly linked regular files of exact
mode 0400, with canonical no-symlink paths and immutable opened-file
snapshots. The owner must be the effective service UID. Root ownership is
accepted only for the three exact credential names beneath
`/run/credentials/taira-bootle-lantern-broker.service/`, where systemd may
retain root ownership and grant the unit user read access through its private
credential-delivery ACL; root-owned files at every other path are rejected.

After reviewing `systemd-analyze verify` output, activate peer 1 with:

```bash
sudo systemctl daemon-reload
sudo systemctl enable taira-irohad.service
sudo systemctl restart taira-irohad.service
sudo systemctl is-active taira-bootle-lantern-broker.service taira-irohad.service
sudo stat -Lc '%u:%g:%a:%h:%F' /run/iroha/runtime-provider-broker-v1.sock
```

The expected socket is owned by the numeric `iroha` UID/GID, mode 0660,
single-linked, and of type `socket`. A broker crash stops peer-1 Torii via
`BindsTo`; it is intentionally not auto-restarted across a potentially stale
socket. Inspect the failed unit and endpoint identity before operator-directed
recovery.

For container-only peer-1 testing, prepare one canonical host credential
directory owned by numeric UID/GID 1001, mode 0700, with distinct singly
linked 0400 files named `issuer-seed`, `bearer-token`, and `principal-seed`.
Set the three `TAIRA_BOOTLE_LANTERN_*` values documented in the compose env
example and apply the override explicitly:

```bash
docker compose --env-file /etc/default/taira-validator-container.compose.env \
  -f configs/soranexus/taira/docker-compose.validator.yml \
  -f configs/soranexus/taira/docker-compose.peer1-privacy.yml up -d
```

The peer-1 supervisor starts Torii only after the exact socket is ready and
sends SIGTERM to both processes when either exits. Never apply this override
to peers 2–4.

Validate the safe checked-in staging state and its adversarial suite with:

```bash
python3 configs/soranexus/taira/validate_privacy_bootstrap.py --mode staging
python3 configs/soranexus/taira/privacy_bootstrap_validation_test.py -v
```

The final bundle gate uses `--mode release --broker-public <verified-export>`.
It fails closed until the plan
contains exactly 12 canonical activation instruction digests, the complete
public provider and governed issuer-policy digests, the exact canonical broker
export SHA-256, the peer-1 config is enabled with exact matching bindings, and
genesis contains exactly those 12 Norito instructions followed by the
issuer-policy instruction. Native genesis
decoding and the exact-12 release-evidence runner remain mandatory alongside
this public-input validator.

Every first-release Taira archive authority must carry native end-to-end
evidence for the exact 12 privacy protocols. This evidence is a post-build
release gate, not a source-schema or pre-bundle report:

- the ordinary `irohad` feature graph must not contain
  `privacy-release-evidence`;
- `taira_privacy_release_runner` is built separately with that feature after
  the ordinary validator binary exists;
- `scripts/compute_workspace_source_manifest.py` freezes the canonical
  workspace source identity before any Cargo command and recomputes it after
  the builds, after evidence generation, and before the archive is created;
- the receipt, fixed-order 48-block stage-artifact bundle, and command manifest
  are authoritative Norito. Their matching JSON files are mandatory,
  deterministic projections which the runner decodes, re-encodes, and compares
  for typed equality;
- `native_release_expectations_v1.norito` is the authoritative frozen
  expectation set. Its checked-in JSON projection is equally mandatory for
  review, and the runner rejects any byte or typed-value disagreement between
  them;
- `zk_x509_native_resource_v1.norito` is the authoritative typed X.509
  native-resource certificate. Its mandatory JSON projection binds the exact
  Linux/ARM64/Graviton3 environment, fixed process and isolation limits,
  expectation digests, deterministic KAT digest and size, and the separately
  observed positive and maximum-shape resource measurements;
- the frozen expectations enforce a peak-RSS ceiling and a generous elapsed
  ceiling for every exact-12 maximum-shape row. The stage bundle records case
  descriptors, failure classes, and the canonical valid proof bytes needed for
  independent audit and reverification. It contains no witnesses or private
  inputs. Every proof is bounded by its exact protocol ceiling and the 8 MiB
  Taira consensus cap, and its recorded SHA-256 is recomputed from those bytes.

The first release has one capture corridor:
`.github/workflows/capture_taira_privacy_native_evidence.yml`. It is a
manual-only workflow for the existing self-hosted c7g.4xlarge ARM64 release
runner. Dispatch requires the exact Iroha commit, reviewed DPN source-tree and
Cargo-lock pins, exact-12 fixture pin, and reviewed generic ceilings. It fails
unless all four native fixture targets are absent and every KAT, expectation,
resource-observation, and resource-certificate source pin is still zero. The
job captures and validates the four artifacts, installs them with create-new
semantics, recomputes the source identity, performs a clean final build, and
uploads a non-publishing provenance archive. It never pushes, deploys, or
modifies `Cargo.lock`; a failed or partial run must be discarded rather than
reused.

The Linux build job reconstructs one reviewed source closure from the exact
Iroha commit, DPN source commit, `Cargo.lock`, and canonical workspace-source
manifest. It builds directly on an untrusted native Linux/aarch64 runner and
generates and verifies the exact-12 evidence after the final validator binary
exists. A fresh Linux authority job, with no checkout or product execution,
copies the hostile handoff through the installed controller's descriptor-based
inspector and signs only the frozen staged bytes.

The untrusted macOS build job reconstructs the same source identity and emits
only inert macOS/arm64 binaries. A separate secret-free qualification runner
uses a qualification-only reset bundle and signer to execute the built
validator, complete the exact-four-peer validation and every-peer restart
proof, and bind the inspected macOS handoff digest into the receipt. It has no
production reset, release signer, registry, or deployment authority. A fresh
candidate authority job then authenticates that receipt and the Linux
authority without executing the product. The final candidate therefore binds
both native targets, both installed-controller digests, one source identity,
and the exact inspected binary handoff before deployment or publication.

The bundle stores these files under `provenance/privacy-native/` and includes
the runner as `bin/taira_privacy_release_runner`. `rollout.manifest.json` and
`sha256sums.txt` bind the authoritative Norito hashes. The JSON projections are
for human inspection and cannot replace their paired Norito files.

Before installing a bare-metal bundle, rerun the packaged verifier from the
bundle root:

```bash
bundle=/path/to/taira-rollout-...
source_sha="$(tr -d '\n' < "${bundle}/provenance/privacy-native/workspace-source-manifest.sha256")"
"${bundle}/bin/taira_privacy_release_runner" verify \
  --build-profile release \
  --source-sha256 "${source_sha}" \
  --exact12-matrix "${bundle}/provenance/privacy-native/exact12-v1.tsv" \
  --expectations-norito "${bundle}/provenance/privacy-native/expectations-v1.norito" \
  --expectations-json "${bundle}/provenance/privacy-native/expectations-v1.json" \
  --x509-resource-norito "${bundle}/provenance/privacy-native/zk-x509-resource-v1.norito" \
  --x509-resource-json "${bundle}/provenance/privacy-native/zk-x509-resource-v1.json" \
  --cargo-lock "${bundle}/provenance/Cargo.lock" \
  --validator-binary "${bundle}/bin/iroha3d" \
  --command-manifest-norito "${bundle}/provenance/privacy-native/command-manifest-v1.norito" \
  --command-manifest-json "${bundle}/provenance/privacy-native/command-manifest-v1.json" \
  --stage-artifacts-norito "${bundle}/provenance/privacy-native/stage-artifacts-v1.norito" \
  --stage-artifacts-json "${bundle}/provenance/privacy-native/stage-artifacts-v1.json" \
  --receipt-norito "${bundle}/provenance/privacy-native/receipt-v1.norito" \
  --receipt-json "${bundle}/provenance/privacy-native/receipt-v1.json"
```

A JSON-only `privacy-release.json`, the retired
`taira_privacy_prebundle_gate` output, a test log, or a report generated before
the final validator binary exists is not native privacy release evidence and
must not be attached to a Taira rollout ticket as though it were.

## Signed release authority

A release-profile archive is not a Taira release candidate until its portable
Ed25519 authority tuple passes. The native builder is deliberately unsigned
and must not receive any of the authority values below. The separate
post-build finalizer requires five paths/pins provisioned outside the checkout:

- `TAIRA_RELEASE_EXTERNAL_SIGNER_PATH`: reviewed signer accepting the canonical
  manifest path and one create-new raw-signature path;
- `TAIRA_RELEASE_SIGNING_PUBLIC_KEY_PATH`: raw 32-byte Ed25519 public key;
- `TAIRA_TRUSTED_RELEASE_SIGNING_FINGERPRINT`: independently reviewed SHA-256
  of that raw public key;
- `TAIRA_RELEASE_MANIFEST_VERIFIER_PATH`: independently reviewed
  `sorafs-validate`;
- `TAIRA_TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256`: its reviewed SHA-256.

Every file variable must use its canonical physical absolute path, without
symlinked or `..` path components, and must resolve outside the checkout.
No private key path or signing seed is accepted. The unsigned build emits the
evidence directory and matching `.tar.gz`; only
`scripts/finalize_taira_rollout_authority.py` may emit
`<bundle>.authority/`. The final macOS candidate carries an independently
signed top-level authority tuple. Each authority contains
`release_manifest.json`, its raw `.sig` and `.pub`, and an `artifacts/`
directory holding the canonical exact-12 authority, `SHA256SUMS`, the pinned
native verifier, the portable authority validator, and
`authority-controller-v1.json`. The signed payload has no build-host absolute
path. It binds the full workspace-source identity and controller digest,
exact-12 registry and retired-label set, validator, evidence runner, Cargo
lock, matrix, all authoritative Norito evidence and mandatory JSON
projections, and the exact archive digest.

The Linux workflow enforces three separate roles. First, a no-checkout public-
input authority runs only the preinstalled controller as root. It descriptor-
snapshots exactly the four documented secret-free files from
`TAIRA_PRIVACY_RELEASE_INPUT_DIR`, rejects links, special files, mode or inode
drift, and freezes a root-owned `0555` handoff with `0444` files. Second, the
untrusted archive builder runs beneath `env -i` and receives only the downloaded
public bytes plus public source-provenance paths; it receives no protected input
path, signer, public-key, fingerprint, reset input, or native-verifier value.
Third, the no-checkout Linux authority copies both hostile handoffs into its
persistent root-owned staging tree. The finalizer byte-compares every composed
privacy input with the trusted public snapshot before and after signing, replays
the authority subject and manifest, and rejects any archive, evidence, helper,
verifier, or closed-inventory drift.

Admission must first verify `release_manifest.json.sig` with the separately
reviewed signer fingerprint and verifier digest, then run
`taira_release_authority.py verify` against the candidate archive
evidence. The archive verifier parses the tar directly and rejects traversal,
duplicate members, links, sparse/special members, missing evidence, and any
evidence size or digest mismatch. An archive, mutable registry tag, unsigned
`rollout.manifest.json`, standalone public key, or self-reported registry
digest is never release authority.

For a bare-metal candidate, the portable verification shape is:

```bash
authority=/path/to/taira-rollout-....authority
bundle=/path/to/extracted/taira-rollout-...
archive=/path/to/taira-rollout-....tar.gz
commit=<reviewed-full-40-character-commit>
fingerprint=<reviewed-release-signing-public-key-sha256>
verifier_sha=<reviewed-sorafs-validate-sha256>

actual_verifier_sha="$(
  python3 -I -S -c \
    'import hashlib, pathlib, sys; print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())' \
    "${authority}/artifacts/sorafs-validate"
)"
test "${actual_verifier_sha}" = "${verifier_sha}"
"${authority}/artifacts/sorafs-validate" release-manifest \
  --manifest "${authority}/release_manifest.json" \
  --public-key "${authority}/release_manifest.json.pub" \
  --public-key-fingerprint "${fingerprint}" \
  --signature "${authority}/release_manifest.json.sig"
python3 -I -S "${authority}/artifacts/taira_release_authority.py" verify \
  --evidence-root "${bundle}" \
  --commit "${commit}" \
  --signing-fingerprint "${fingerprint}" \
  --native-verifier-sha256 "${verifier_sha}" \
  --archive "${archive}" \
  --authority \
    "${authority}/artifacts/taira-exact12-release-authority-v1.json"
```

## Render validator configs

Do not hand-edit `config.toml` into multiple validator copies. Instead:

1. Copy `validator_roster.example.toml` to a user-local path such as
   `configs/soranexus/taira/validator_roster.local.toml`.
2. Copy `validator_secrets.example.toml` to a user-local path such as
   `configs/soranexus/taira/validator_secrets.local.toml`.
3. Fill in every validator's real `public_key`, `pop_hex`, and
   `public_address` plus its own direct `torii_public_address` in the public
   roster, then put each matching validator `private_key` plus its dedicated
   Ed25519 `soranet_transport_public_key`/`soranet_transport_private_key` pair,
   its secp256k1 `receipt_public_key`/`receipt_private_key` pair, and the shared
   `account_onboarding_*`, `torii_faucet_*`, `streaming_identity_*`, every
   `soracloud_runtime_signer_*` public binding field,
   `sorafs_council_public_keys`, and `sorafs_council_signature_threshold`
   values in the runtime file. A validator's SoraNet transport identity must
   be distinct from both its BLS node identity and the shared streaming
   identity. The Torii receipt key is also validator-specific and distinct from
   every validator and transport key; the renderer checks each public/private
   pair on secp256k1 and rejects duplicate receipt identities.
   SoraFS council roots must be canonical Ed25519
   governance keys; never substitute validator, node identity, or provider
   advert keys.
4. Create an owner-private absolute render root and render the per-validator
   bundle beneath it:
   - `TAIRA_VALIDATOR_RENDER_ROOT="$(mktemp -d /private/var/tmp/iroha-taira-validator-render.XXXXXX)"`
   - `chmod 0700 "${TAIRA_VALIDATOR_RENDER_ROOT}"`
   - `python3 scripts/render_taira_validator_bundle.py --roster configs/soranexus/taira/validator_roster.local.toml --secrets configs/soranexus/taira/validator_secrets.local.toml --output-dir "${TAIRA_VALIDATOR_RENDER_ROOT}/taira-validators"`
5. Copy each validator's complete generated directory to that validator
   host's canonical `/etc/iroha/taira-validator` directory. Every rendered
   config binds signer sidecars and governance manifests to that same
   first-release install root; it never embeds the developer checkout or
   `dist/` path. The renderer creates bundle and runtime directories with mode
   `0700`, creates validator, SoraNet transport, streaming, Kagemusha command,
   onboarding, faucet, and API-token sidecars with mode `0600`, injects only
   that validator's receipt key pair directly into its owner-private config,
   writes canonical paths for the other signers plus the BLAKE3 token digest,
   and emits a protective `.gitignore`. It also creates the
   co-located `sorafs_admission/` directory and rewrites admission-envelope,
   signer, and manifest paths together when
   `--install-root` is changed. It prints sidecar paths but never their contents.

The bundle also contains one shared unsigned `genesis.json` whose dedicated
topology transaction is rebuilt from the public roster and PoPs, plus
`genesis-signing-command.txt`. Provision `TAIRA_GENESIS_EXTERNAL_SIGNER` as an
independently built, reviewed executable outside the checkout and run that
command. The fixed protocol passes only `--unsigned-genesis`, `--peer-config`,
`--bound-manifest-out`, `--signed-genesis-out`, and `--expected-hash-out`.
The qualified signer must also publish the sibling `genesis.identity.toml`
which binds its one exact signed-header hash as both client `network_id` and
validator `genesis.expected_hash`; deployment automation must consume that
paired artifact and reject either independently supplied value. The published
template resolves `/run/iroha/genesis.expected_hash` through
`genesis.expected_hash_file`; clients mount the same file as `network_id_file`.
The signer owns its isolated external software-signing service and encrypted
runtime-only key access internally; it must never accept a private-key path or
key bytes through argv, environment, or the rendered tree. Source-built Kagami
is not a genesis signer in this release path. The
external signer binds the staged Nexus/AMX and execution-policy context,
recomputes the consensus fingerprint, atomically replaces only the rendered
`genesis.json`, and writes `genesis.signed.nrt`. Never copy the genesis signer,
genesis key, or validator private keys into the checkout, template, rendered
genesis JSON, or Actions storage.

## Kagemusha production release material

The ABI-21/V4 `cash_handoff_v1` protocol is available on every compatible
Iroha deployment without a bootstrap switch. A production Kagemusha top-up or
redemption still authenticates one exact release and its verifier material.
That material does not enable offline support or enroll an asset, but a
validator configured to use the production catalog must have the complete
qualified catalog before it starts. The default Taira render omits those paths,
so unpublished artifacts cannot break ordinary startup, `/health`, or
`/readyz`.

For a fresh production reset, the ordering is strict. Install the reviewed
canonical release policy first, but do not generate or install a release and
do not create a qualification seal yet. The authenticated genesis must
directly grant the account that will execute activation both
`CanActivateKagemushaRecursiveReleaseV4` and
`CanManageOfflineDeviceAttestationPolicy`. Compose and sign the final genesis
with that explicit account and the policy-only staging context; the composer
rejects a missing grant, a later revoke, or a policy change during signing.
Only the resulting signed-genesis hash is the release `NetworkId`. Generate
the exact-network release after that hash exists, install it on all four
validators, qualify it with each validator's exact installed `iroha3d`, and
only then start any validator process. Starting from a config without the
same policy digest or starting before qualification is unsupported.

The release root and policy file must already exist at the canonical paths
below. The catalog and qualification seal must both be absent at this stage;
an empty or stale catalog is rejected rather than silently ignored. Run the
composer only through the installed release controller. Every other external
input path in this command must be the exact path pre-authorized by the
root-owned runner trust record; the controller separately accepts the
Kagemusha release root only when that absolute canonical directory and every
ancestor are root-owned and not group- or world-writable:

```bash
ACTIVATION_AUTHORITY="${ACTIVATION_AUTHORITY:?set the genesis-authorized executing account}"
TAIRA_CONTROLLER_COMMAND=/usr/local/libexec/iroha-taira-release-controller-v1
TAIRA_CONTROLLER_ROOT=/usr/local/libexec/iroha-taira-release-controller-v1.d
EXPECTED_LAUNCHER_SHA256="${EXPECTED_LAUNCHER_SHA256:?set the installed launcher digest}"
EXPECTED_CONTROLLER_DIGEST="${EXPECTED_CONTROLLER_DIGEST:?set the installed closure digest}"
EXPECTED_CONTROLLER_VERSION="${EXPECTED_CONTROLLER_VERSION:?set the installed controller version}"
EXPECTED_HOST_ID="${EXPECTED_HOST_ID:?set the attested qualification host ID}"
EXPECTED_INSTALLATION_ID="${EXPECTED_INSTALLATION_ID:?set the attested installation ID}"
SOURCE_COMMIT="${SOURCE_COMMIT:?set the exact release source commit}"

test "$(shasum -a 256 "${TAIRA_CONTROLLER_COMMAND}" | awk '{print $1}')" = \
  "${EXPECTED_LAUNCHER_SHA256}"
CONTROLLER_COMMON=(
  --expected-launcher-sha256 "${EXPECTED_LAUNCHER_SHA256}"
  --expected-controller-digest "${EXPECTED_CONTROLLER_DIGEST}"
  --expected-version "${EXPECTED_CONTROLLER_VERSION}"
  --expected-host-id "${EXPECTED_HOST_ID}"
  --expected-installation-id "${EXPECTED_INSTALLATION_ID}"
  --expected-uid 0
  --source-commit "${SOURCE_COMMIT}"
  --platform macos
  --role macos-qualification
)
CONTROLLER_ATTESTATION="$(sudo -n "${TAIRA_CONTROLLER_COMMAND}" \
  attest "${CONTROLLER_COMMON[@]}")"
CONTROLLER_RUNTIME_ROOT="$(/usr/bin/python3 -I -S -c \
  'import json,sys; print(json.load(sys.stdin)["runtime_root"])' \
  <<<"${CONTROLLER_ATTESTATION}")"
RESET_BUNDLE="${CONTROLLER_RUNTIME_ROOT}/taira-reset-kagemusha-v4-r1"
test ! -e "${RESET_BUNDLE}"

sudo -n "${TAIRA_CONTROLLER_COMMAND}" run "${CONTROLLER_COMMON[@]}" \
  prepare-reset -- \
  --source-bundle /absolute/private/path/admitted-source-reset \
  --source-bundle-sha256 "${SOURCE_BUNDLE_SHA256}" \
  --privacy-release-dir /absolute/private/path/authenticated-privacy-release \
  --genesis-external-signer /absolute/reviewed/path/genesis-external-signer \
  --trusted-genesis-external-signer-sha256 "${GENESIS_SIGNER_SHA256}" \
  --onboarding-token-hash-tool /absolute/reviewed/path/onboarding-token-hash-tool \
  --kagemusha-release-root /srv/iroha-kagemusha/taira-v4-r1 \
  --kagemusha-activation-authority "${ACTIVATION_AUTHORITY}" \
  --irohad-sha256 "${IROHAD_SHA256}" \
  --source-commit "${SOURCE_COMMIT}" \
  --dpn-validator-release-commit "${DPN_VALIDATOR_RELEASE_COMMIT}" \
  --cargo-lock-sha256 "${CARGO_LOCK_SHA256}" \
  --workspace-source-manifest-sha256 "${WORKSPACE_SOURCE_MANIFEST_SHA256}" \
  --controller-manifest \
    "${TAIRA_CONTROLLER_ROOT}/authority-controller-v1.json" \
  --controller-digest "${EXPECTED_CONTROLLER_DIGEST}" \
  --output-bundle "${RESET_BUNDLE}"

GENESIS_EXPECTED_HASH="$(jq -er '.genesis_expected_hash' \
  "${RESET_BUNDLE}/reset-manifest.json")"
test -n "${GENESIS_EXPECTED_HASH}"
```

The first private renderer pass contains the policy and artifact paths but no
seal path, and uses only a canonical marker-bearing staging hash while the
external signer computes the final hash. The signer authenticates the policy
without opening the not-yet-generated artifact directory. The second pass
embeds the final genesis hash and adds the future seal path. Both passes bind
the same policy digest, recorded as `kagemusha_release_policy_sha256` in the
reset manifest; changing the policy after signing makes the reset fail.

Select and validate the release heights before producing the roster or any
candidate bytes. A fresh reset starts with height-one genesis committed, but
height 2 leaves no safe interval in which to start four validators, collect
evidence, and execute threshold approval. Pick an explicit operational margin
and fail unless activation is beyond that margin. For an already-running
chain, replace `RESET_COMMITTED_HEIGHT=1` with the maximum
`.last_committed_height` captured from all four validators. The activation and
withdrawal heights are authenticated release inputs; never edit or reuse the
roster or generated release after either value becomes stale.

```bash
RESET_COMMITTED_HEIGHT=1
ACTIVATION_SUBMISSION_MARGIN_BLOCKS="${ACTIVATION_SUBMISSION_MARGIN_BLOCKS:?set a reviewed positive submission margin}"
ACTIVATION_HEIGHT="${ACTIVATION_HEIGHT:?set a reviewed activation height}"
WITHDRAWAL_HEIGHT="${WITHDRAWAL_HEIGHT:?set a reviewed withdrawal height}"

/usr/bin/python3 -I -S - \
  "${RESET_COMMITTED_HEIGHT}" \
  "${ACTIVATION_SUBMISSION_MARGIN_BLOCKS}" \
  "${ACTIVATION_HEIGHT}" \
  "${WITHDRAWAL_HEIGHT}" <<'PY'
import sys

labels = (
    "reset committed height",
    "activation submission margin",
    "activation height",
    "withdrawal height",
)
try:
    current, margin, activation, withdrawal = map(int, sys.argv[1:])
except ValueError as error:
    raise SystemExit(f"release height is not an integer: {error}")
if any(
    value < 0 or value > 2**64 - 1
    for value in (current, margin, activation, withdrawal)
):
    raise SystemExit("release height is outside the u64 range")
if margin == 0:
    raise SystemExit("activation submission margin must be positive")
if activation <= current + margin:
    raise SystemExit(
        f"activation height {activation} is not beyond committed height "
        f"{current} plus the {margin}-block submission margin"
    )
if withdrawal <= activation:
    raise SystemExit("withdrawal height must be greater than activation height")
values = (current, margin, activation, withdrawal)
print(", ".join(f"{label}={value}" for label, value in zip(labels, values)))
PY
```

First seal the rendered public validator keys and PoPs into the release-bound
top-up roster. The input config may contain runtime secrets, but the command
reads only `trusted_peers_pop` and emits only the public canonical roster.
Use one independently reviewed Kagami executable for this command, circuit
parameter construction, activation preparation, and the production readiness
gate. The path and SHA-256 below are public release inputs, not secrets. Keep
them in the same operator shell for the complete workflow; do not rebuild or
replace Kagami between steps.

Before and after every invocation, the helper below requires a canonical
absolute non-symlink path, a root-owned and non-group/world-writable directory
chain, and a root-owned, single-link, executable regular file whose bytes match
the independently reviewed SHA-256. The descriptor and pathname metadata must
also remain identical across each check:

```bash
KAGEMUSHA_V4_KAGAMI_BIN=/absolute/root-custodied/kagami
KAGEMUSHA_V4_KAGAMI_SHA256='<reviewed-kagami-64-lowercase-hex>'
export KAGEMUSHA_V4_KAGAMI_BIN KAGEMUSHA_V4_KAGAMI_SHA256
readonly KAGEMUSHA_V4_KAGAMI_BIN KAGEMUSHA_V4_KAGAMI_SHA256

assert_kagemusha_v4_kagami_custody() {
  /usr/bin/python3 -I -S - \
    "${KAGEMUSHA_V4_KAGAMI_BIN}" \
    "${KAGEMUSHA_V4_KAGAMI_SHA256}" <<'PY'
import hashlib
import os
from pathlib import Path
import re
import stat
import sys

raw_path, expected_sha256 = sys.argv[1:]
if (
    re.fullmatch(r"[0-9a-f]{64}", expected_sha256) is None
    or expected_sha256 == "0" * 64
):
    raise SystemExit("Kagami SHA-256 pin is not canonical")

path = Path(raw_path)
try:
    resolved = path.resolve(strict=True)
except OSError as error:
    raise SystemExit(f"Kagami path cannot be resolved: {error}")
if not path.is_absolute() or resolved != path:
    raise SystemExit("Kagami path must be canonical, absolute, and symlink-free")

current = Path(path.anchor)
directories = [current]
for component in path.parts[1:-1]:
    current /= component
    directories.append(current)
for directory in directories:
    metadata = directory.lstat()
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISDIR(metadata.st_mode):
        raise SystemExit(f"Kagami path component is not a real directory: {directory}")
    if metadata.st_uid != 0 or stat.S_IMODE(metadata.st_mode) & 0o022:
        raise SystemExit(f"Kagami path component lacks production custody: {directory}")

before = path.lstat()
if (
    stat.S_ISLNK(before.st_mode)
    or not stat.S_ISREG(before.st_mode)
    or before.st_uid != 0
    or stat.S_IMODE(before.st_mode) & 0o022
    or not stat.S_IMODE(before.st_mode) & 0o111
    or before.st_nlink != 1
    or before.st_size <= 0
    or before.st_size > 512 * 1024 * 1024
):
    raise SystemExit("Kagami executable lacks production custody")

fingerprint = (
    before.st_dev,
    before.st_ino,
    before.st_nlink,
    before.st_mode,
    before.st_size,
    before.st_mtime_ns,
    before.st_ctime_ns,
    before.st_uid,
    before.st_gid,
)
flags = os.O_RDONLY | getattr(os, "O_CLOEXEC", 0) | getattr(os, "O_NOFOLLOW", 0)
descriptor = os.open(path, flags)
try:
    opened = os.fstat(descriptor)
    opened_fingerprint = (
        opened.st_dev,
        opened.st_ino,
        opened.st_nlink,
        opened.st_mode,
        opened.st_size,
        opened.st_mtime_ns,
        opened.st_ctime_ns,
        opened.st_uid,
        opened.st_gid,
    )
    if not os.path.samestat(before, opened) or opened_fingerprint != fingerprint:
        raise SystemExit("Kagami executable changed while it was opened")
    digest = hashlib.sha256()
    offset = 0
    while offset < opened.st_size:
        chunk = os.pread(descriptor, min(1024 * 1024, opened.st_size - offset), offset)
        if not chunk:
            raise SystemExit("Kagami executable became truncated while it was hashed")
        digest.update(chunk)
        offset += len(chunk)
    after_descriptor = os.fstat(descriptor)
    after_path = path.lstat()
    after_descriptor_fingerprint = (
        after_descriptor.st_dev,
        after_descriptor.st_ino,
        after_descriptor.st_nlink,
        after_descriptor.st_mode,
        after_descriptor.st_size,
        after_descriptor.st_mtime_ns,
        after_descriptor.st_ctime_ns,
        after_descriptor.st_uid,
        after_descriptor.st_gid,
    )
    after_path_fingerprint = (
        after_path.st_dev,
        after_path.st_ino,
        after_path.st_nlink,
        after_path.st_mode,
        after_path.st_size,
        after_path.st_mtime_ns,
        after_path.st_ctime_ns,
        after_path.st_uid,
        after_path.st_gid,
    )
    if (
        after_descriptor_fingerprint != fingerprint
        or after_path_fingerprint != fingerprint
        or digest.hexdigest() != expected_sha256
    ):
        raise SystemExit("Kagami executable changed or differs from its reviewed SHA-256")
finally:
    os.close(descriptor)
PY
}

assert_kagemusha_v4_kagami_custody || exit 1
if /usr/bin/env -i LANG=C LC_ALL=C PATH=/usr/bin:/bin \
  "${KAGEMUSHA_V4_KAGAMI_BIN}" \
  kagemusha prepare-taira-release-roster-v4 \
  --validator-config /absolute/path/to/rendered-validator/config.toml \
  --network-id "${GENESIS_EXPECTED_HASH}" \
  --withdrawal-height "${WITHDRAWAL_HEIGHT}" \
  --output /absolute/private/path/taira-release-roster.norito
then
  KAGEMUSHA_COMMAND_STATUS=0
else
  KAGEMUSHA_COMMAND_STATUS=$?
fi
assert_kagemusha_v4_kagami_custody || exit 1
test "${KAGEMUSHA_COMMAND_STATUS}" -eq 0 || exit "${KAGEMUSHA_COMMAND_STATUS}"

mkdir -m 700 /absolute/private/path/kagemusha-release-inputs
assert_kagemusha_v4_kagami_custody || exit 1
if /usr/bin/env -i LANG=C LC_ALL=C PATH=/usr/bin:/bin \
  "${KAGEMUSHA_V4_KAGAMI_BIN}" \
  kagemusha prepare-release-circuit-params-v4 \
  --output-dir /absolute/private/path/kagemusha-release-inputs/circuit-params-v4
then
  KAGEMUSHA_COMMAND_STATUS=0
else
  KAGEMUSHA_COMMAND_STATUS=$?
fi
assert_kagemusha_v4_kagami_custody || exit 1
test "${KAGEMUSHA_COMMAND_STATUS}" -eq 0 || exit "${KAGEMUSHA_COMMAND_STATUS}"
```

The circuit-parameter command is the official constructor for both reviewed
first-release inputs. It publishes the two canonical Norito files together by
one no-replace directory rename, with owner-private custody and raw-file plus
domain-separated parameter hashes in its report. Refuse to continue on the
exit-75 `commit-uncertain` outcome until the visible directory has been
inspected; rerunning cannot overwrite it.

Generate the real Eq/Ep artifacts through the source-sealed two-stage
packager. Configure the reviewer's user-level
`gpg.ssh.allowedSignersFile` to one absolute, owner-controlled, single-key
policy (and `gpg.ssh.revocationFile` when applicable). The seal ignores
repository-local signature configuration, pins `/usr/bin/ssh-keygen`, and
requires exactly one trusted SSH signature on `HEAD`. Then explicitly review
and seal its exact clean closure (index equal to `HEAD`, exact worktree blob
and mode identity, zero untracked files, present-empty tracked gitlink directories,
and the separately bound root `Cargo.lock`) before
building the exact candidate binary and entering the non-raiseable 64 GiB /
half-physical-RAM generation guard. Its polling stop uses process-tree RSS and
its final gate uses the direct child's kernel peak RSS; macOS footprint remains
diagnostic only. Keep at least 16 GiB free on its pinned
disk-backed output filesystem for the raw proving-key spools and framed
artifact copy.
Retain the helper's canonical JSON report. The reviewed source closure and its
digest are release inputs; the report's `source_commit` must equal the verified
`HEAD`, `source_repo_dirty` must be `false`, and any working-tree change fails
closed with no dirty-closure compatibility path.
Pass direct Cargo and rustc toolchain binaries, not the `~/.cargo/bin` rustup
proxies, and bind both to independently reviewed SHA-256 pins. The selected
Cargo home is cache-only, must contain no `config` or `config.toml`, and the
sealed build runs offline with a fixed `HOME` and `PATH`.

```bash
python3 -I scripts/build_kagemusha_v4_candidate_bundle.py \
  --root "$PWD" \
  --cargo /absolute/direct/toolchain/bin/cargo \
  --cargo-sha256 '<reviewed-64-lowercase-hex>' \
  --rustc /absolute/direct/toolchain/bin/rustc \
  --rustc-sha256 '<reviewed-64-lowercase-hex>' \
  --cargo-home /absolute/owner-controlled/cache-only-cargo-home \
  --target-dir /absolute/private/path/kagemusha-sealed-target \
  --reviewed-source-closure /absolute/private/path/reviewed-source-closure.json \
  --reviewed-source-closure-sha256 '<64-lowercase-hex>' \
  --authenticated-source-seal-projection /absolute/private/path/authenticated-source-seal-projection.json \
  --authenticated-source-seal-projection-sha256 '<64-lowercase-hex>' \
  > /absolute/private/path/sealed-kagemusha-candidate-build.json

python3 scripts/run_kagemusha_v4_generation.py \
  --resource-report /absolute/private/path/taira-release-generation \
  -- \
  /absolute/path/from/sealed-build-report/kagemusha_recursive_spend_v4_bundle \
  generate-candidate \
  --out-dir /absolute/private/path/taira-release-candidate \
  --network-id "${GENESIS_EXPECTED_HASH}" \
  --asset-definition-id 7ZepsJTHCVLKsrFFNZGSRGZgvBhv \
  --asset-scale 2 \
  --generation production-gate-real-artifacts-v4 \
  --parameter-generation production-gate-real-artifacts-v4 \
  --source-commit '<source_commit-from-sealed-build-report>' \
  --source-tree-sha256 '<source_tree_sha256-from-sealed-build-report>' \
  --activation-height "${ACTIVATION_HEIGHT}" \
  --withdrawal-height "${WITHDRAWAL_HEIGHT}" \
  --step-eq-circuit-params /absolute/private/path/kagemusha-release-inputs/circuit-params-v4/step-eq-circuit-params.norito \
  --step-ep-circuit-params /absolute/private/path/kagemusha-release-inputs/circuit-params-v4/step-ep-circuit-params.norito \
  --topup-finality-roster /absolute/private/path/taira-release-roster.norito

/absolute/path/from/sealed-build-report/kagemusha_recursive_spend_v4_bundle \
  finalize-release \
  --candidate-dir /absolute/private/path/taira-release-candidate \
  --out-dir /absolute/private/path/taira-final-release \
  --release-policy /srv/iroha-kagemusha/taira-v4-r1/policy/release-policy-v1.norito \
  --release-attestation /absolute/private/path/release-attestation-v4.norito \
  --benchmark-evidence /absolute/private/path/benchmark-evidence-v1.json \
  --cryptographic-review /absolute/private/path/cryptographic-review-v4.norito
```

Before accepting the finalized release, a separately authenticated controller
must install the exact reviewed checkout under a root-owned,
non-group/world-writable path and verify the readiness-gate digest before
execution. Pre-create `/var/lib/iroha/kagemusha-readiness-v1` as root mode
`0700` (macOS controllers use
`/private/var/db/iroha-kagemusha-readiness-v1`). The allowed-signers and
revocation files below are the exact policies bound by the authenticated
source-seal projection. Both are mandatory and digest pinned; use an explicitly
pinned empty revocation file when the reviewed policy has no revoked keys.
Promotion does not consult the invoking account's Git configuration. The gate
inherits the exported read-only `KAGEMUSHA_V4_KAGAMI_BIN` and
`KAGEMUSHA_V4_KAGAMI_SHA256` verified above, so it authenticates the same
executable used to construct the roster, circuit parameters, and activation:

```bash
KAGEMUSHA_PRODUCTION_READINESS_GATE_SHA256='<reviewed-gate-64-lowercase-hex>' \
KAGEMUSHA_PRODUCTION_READINESS_PYTHON=/absolute/root-custodied/python3 \
KAGEMUSHA_PRODUCTION_READINESS_PYTHON_SHA256='<reviewed-python-64-lowercase-hex>' \
KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE=/absolute/root-custodied/reviewed-source-closure.json \
KAGEMUSHA_BUILD_REVIEWED_SOURCE_CLOSURE_SHA256='<reviewed-closure-64-lowercase-hex>' \
KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION=/absolute/root-custodied/authenticated-source-seal-projection.json \
KAGEMUSHA_BUILD_AUTHENTICATED_SOURCE_SEAL_PROJECTION_SHA256='<reviewed-projection-64-lowercase-hex>' \
KAGEMUSHA_PRODUCTION_SOURCE_SSH_ALLOWED_SIGNERS_PATH=/absolute/root-custodied/allowed-signers \
KAGEMUSHA_PRODUCTION_SOURCE_SSH_ALLOWED_SIGNERS_SHA256='<reviewed-allowed-signers-64-lowercase-hex>' \
KAGEMUSHA_PRODUCTION_SOURCE_SSH_REVOCATION_PATH=/absolute/root-custodied/revocation \
KAGEMUSHA_PRODUCTION_SOURCE_SSH_REVOCATION_SHA256='<reviewed-revocation-64-lowercase-hex>' \
KAGEMUSHA_V4_RELEASE_POLICY_PATH=/srv/iroha-kagemusha/taira-v4-r1/policy/release-policy-v1.norito \
KAGEMUSHA_V4_ARTIFACT_ROOT=/srv/iroha-kagemusha/taira-v4-r1/catalog \
KAGEMUSHA_IOS_DEVICE_EVIDENCE_ROOT=/absolute/root-custodied/ios-device-evidence \
KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_KEY_ID='<reviewed-key-id>' \
KAGEMUSHA_IOS_DEVICE_EVIDENCE_TRUSTED_PUBLIC_KEY=/absolute/root-custodied/ios-evidence-ed25519.pub.pem \
  /absolute/root-custodied/reviewed-iroha/ci/check_kagemusha_production_readiness.sh promotion
```

`generate-candidate` is the only command accepted by the guarded runner; do not
wrap Cargo, a shell, or `env`. It publishes an immutable pre-evidence candidate
and owner-private JSONL/summary resource evidence, not an approved release.
The production generator, `validate-candidate`, and finalizer expose no
fault-injection flag: adding one would create a test backdoor on the exact
source-sealed release surface. Use a substituted copy with the
candidate-preserving validation command and retain the existing
role/header/key-substitution plus atomic-publication regressions as the
negative gate.
`finalize-release` authenticates the supplied policy, attestation, physical
benchmark, signed cryptographic review, and
`recursive-step-two-qualification-v4.norito` receipt, then copies the exact
candidate bytes into a new seventeen-file release directory without
regenerating proof material. Provision the same policy as
`/srv/iroha-kagemusha/taira-v4-r1/policy/release-policy-v1.norito` and install that
finalized directory as
`/srv/iroha-kagemusha/taira-v4-r1/catalog/<manifest_sha256>/`, where
`manifest_sha256` is the lowercase digest recorded by the finalized
`manifest.norito.sha256`.

Install into a new versioned root; do not replace an existing release in place.
Every directory in the release path must be root-owned mode `0755`. Install the
policy and all seventeen finalized files as root-owned mode `0444`, and verify
that the manifest-digest sidecar equals the release directory name. These are
public release bytes, not runtime secrets. Never recursively `chown` this tree
to the validator user.

For a standalone render outside the reset composer, wait until the immutable
bytes exist on every validator before opting newly rendered configs into the
production catalog. The fresh reset flow above already emitted the equivalent
final configs; retain those exact files so their signed policy identity does
not drift:

```bash
python3 scripts/render_taira_validator_bundle.py \
  --roster configs/soranexus/taira/validator_roster.local.toml \
  --secrets configs/soranexus/taira/validator_secrets.local.toml \
  --output-dir "${TAIRA_VALIDATOR_RENDER_ROOT}/taira-validators" \
  --install-root /etc/iroha/taira-validator \
  --kagemusha-release-root /srv/iroha-kagemusha/taira-v4-r1
```

The opt-in adds these absolute paths without copying or inventing release
bytes:

- `/srv/iroha-kagemusha/taira-v4-r1/policy/release-policy-v1.norito`
- `/srv/iroha-kagemusha/taira-v4-r1/catalog`
- `/srv/iroha-kagemusha/taira-v4-r1/seals/catalog-qualification-v1.norito`

The authenticated policy digest, rather than the mutable presence of a cache
path or seal file, participates in `execution_policy_hash`. Consequently, a
config rendered with the production catalog must not be started against chain
state signed without that exact policy identity. The supported source-side
path is the fresh compatible reset composed above, unless governance has
separately approved and implemented an explicit consensus-context migration.
This is not an in-place config rollout.

Install the policy and manifest-digest release directory at the first two
paths. Keep the qualification-seal directory separate from the policy parent,
artifact directory, and executable directory. On each validator, use the
exact canonical installed executable selected by that validator's service and
the config, signed genesis, and bound genesis manifest emitted by the same
reset bundle. Verify all four byte identities against `reset-manifest.json`
before executing the binary. The final reset config carries the expected hash
inline; the composer does not emit or require a separate
`genesis.expected_hash` file. Run the full one-time qualification as root; the
configured seal destination must not already exist:

```bash
RESET_MANIFEST="${RESET_BUNDLE}/reset-manifest.json"
VALIDATOR_SLUG="${VALIDATOR_SLUG:?set taira-validator-1 through taira-validator-4}"
VALIDATOR_CONFIG="${RESET_BUNDLE}/rendered/${VALIDATOR_SLUG}/config.toml"
SIGNED_GENESIS="${RESET_BUNDLE}/genesis.signed.nrt"
BOUND_GENESIS_MANIFEST="${RESET_BUNDLE}/genesis.json"
IROHAD_BIN="${IROHAD_BIN:?set the exact canonical executable used by this validator service}"
RELEASE_ROOT=/srv/iroha-kagemusha/taira-v4-r1
QUALIFICATION_SEAL="${RELEASE_ROOT}/seals/catalog-qualification-v1.norito"

test "${IROHAD_BIN}" = "$(/usr/bin/python3 -I -S -c \
  'from pathlib import Path; import sys; print(Path(sys.argv[1]).resolve(strict=True))' \
  "${IROHAD_BIN}")"
test -x "${IROHAD_BIN}"
test "$(shasum -a 256 "${IROHAD_BIN}" | awk '{print $1}')" = \
  "$(jq -er '.irohad_sha256' "${RESET_MANIFEST}")"
test "$(shasum -a 256 "${VALIDATOR_CONFIG}" | awk '{print $1}')" = \
  "$(jq -er --arg slug "${VALIDATOR_SLUG}" '.configs[$slug]' \
    "${RESET_MANIFEST}")"
test "$(shasum -a 256 "${SIGNED_GENESIS}" | awk '{print $1}')" = \
  "$(jq -er '.signed_genesis_sha256' "${RESET_MANIFEST}")"
test "$(shasum -a 256 "${BOUND_GENESIS_MANIFEST}" | awk '{print $1}')" = \
  "$(jq -er '.bound_genesis_manifest_sha256' "${RESET_MANIFEST}")"
test "$(jq -er '.kagemusha_release_root' "${RESET_MANIFEST}")" = \
  "${RELEASE_ROOT}"
sudo test ! -e "${QUALIFICATION_SEAL}"

sudo env GENESIS="${SIGNED_GENESIS}" \
  "${IROHAD_BIN}" \
  --sora \
  --config "${VALIDATOR_CONFIG}" \
  --genesis-manifest-json "${BOUND_GENESIS_MANIFEST}" \
  --check-config \
  --write-kagemusha-catalog-qualification-seal "${QUALIFICATION_SEAL}"

sudo chmod 0444 "${QUALIFICATION_SEAL}"
```

The Kagemusha release root must be outside `/etc/iroha/taira-validator` and
outside any reset bundle because the deployment workflow assigns those trees
to the validator user. Never include the release root in that recursive
`chown`. The policy, catalog, executable, and seal path chains must be
canonical, symlink-free, root-owned, and not group- or world-writable. On
macOS, use an equivalent root-controlled path such as
`/Library/SORA/Taira/kagemusha-v4-r1`. The seal authenticates the canonical
`${IROHAD_BIN}` identity/build as well as the qualified catalog; a binary
replacement requires a new qualification and must never overwrite the old
seal.

Retain concrete qualification evidence from every node: the successful
`--check-config` output, `stat` output proving the root ownership and modes,
the exact manifest directory name and `manifest.norito.sha256` contents, and a
sorted `sha256sum` inventory of the policy plus all seventeen release files.
Compare those captured values across all four validators before startup. There
is no authoritative "same catalog digest" status endpoint.
`/v1/offline/readiness` reports universal route capability and is never proof
of catalog qualification, governed activation, or release identity.

```bash
RELEASE_ROOT=/srv/iroha-kagemusha/taira-v4-r1
RELEASE_DIR="${RELEASE_ROOT}/catalog/${MANIFEST_SHA256}"
test "$(basename "${RELEASE_DIR}")" = "${MANIFEST_SHA256}"
test "$(tr -d '\n' < "${RELEASE_DIR}/manifest.norito.sha256")" = \
  "${MANIFEST_SHA256}"
test "$(find "${RELEASE_DIR}" -mindepth 1 -maxdepth 1 -type f | wc -l)" -eq 17

stat -Lc '%U:%G:%a:%h:%F:%n' \
  "${RELEASE_ROOT}" \
  "${RELEASE_ROOT}/policy" \
  "${RELEASE_ROOT}/catalog" \
  "${RELEASE_ROOT}/seals" \
  "${RELEASE_DIR}" \
  "${RELEASE_ROOT}/seals/catalog-qualification-v1.norito" \
  | tee /absolute/private/path/validator-kagemusha-stat.txt

{
  sha256sum "${RELEASE_ROOT}/policy/release-policy-v1.norito"
  find "${RELEASE_DIR}" -mindepth 1 -maxdepth 1 -type f -print0 \
    | sort -z | xargs -0 sha256sum
} | tee /absolute/private/path/validator-kagemusha-sha256.txt
```

Catalog qualification is the filesystem boundary; governed activation is the
separate consensus boundary. Only after all four validators are healthy,
advancing, and have the matching evidence above may activation begin.

First identify the account that will actually execute the activation. Query
both its direct permissions and all permissions inherited from its roles:

```bash
READ_CFG=/absolute/runtime-only/read-client.toml
ACTIVATION_AUTHORITY="${ACTIVATION_AUTHORITY:?set the executing account}"

iroha --machine --config "${READ_CFG}" --output-format json \
  ledger account permission list --id "${ACTIVATION_AUTHORITY}" \
  | tee /absolute/private/path/activation-authority-direct-permissions.json

iroha --machine --config "${READ_CFG}" --output-format json \
  ledger account role list --id "${ACTIVATION_AUTHORITY}" \
  | tee /absolute/private/path/activation-authority-roles.json

jq -er '.[]' /absolute/private/path/activation-authority-roles.json \
  | while IFS= read -r ROLE_ID; do
      iroha --machine --config "${READ_CFG}" --output-format json \
        ledger role permission list --id "${ROLE_ID}"
    done \
  | tee /absolute/private/path/activation-authority-role-permissions.jsonl
```

The combined direct and role-derived results must contain both
`CanActivateKagemushaRecursiveReleaseV4` and
`CanManageOfflineDeviceAttestationPolicy`. They are immutable genesis-only
permissions; there is no post-genesis repair if the executing account lacks
either one. The stock `prepare-taira-testnet-base-genesis-v4` helper grants
both to `genesis_authority`, not to an activation multisig. If threshold
activation is required, the multisig account itself—not merely its signer
accounts—must be created and receive both grants at genesis. The remaining
example assumes `ACTIVATION_MULTISIG_ACCOUNT` is that pre-authorized executing
account.

Build the exact composite activation instruction into new owner-private files.
Kagami prints a durable-publication result followed by the prepared report; the
last line contains the instruction hash, while `--output` names the separate
instruction-array file. Continue in the same operator shell so this invocation
uses the same read-only `KAGEMUSHA_V4_KAGAMI_BIN`,
`KAGEMUSHA_V4_KAGAMI_SHA256`, and
`assert_kagemusha_v4_kagami_custody` established above. Set
`REVIEWED_DEVICE_ATTESTATION_POLICY_STATE_SHA256` from the independent review
of the canonical governed policy state; never copy it from Kagami's report.
Validate all release-binding report fields against those reviewed inputs before
extracting `instructions_hash`:

```bash
ACTIVATION_JSON=/absolute/private/path/kagemusha-activation-v4.json
PREPARE_REPORT=/absolute/private/path/kagemusha-activation-v4.prepare.jsonl
REVIEWED_DEVICE_ATTESTATION_POLICY_STATE_SHA256='<reviewed-device-policy-state-64-lowercase-hex>'
set -o pipefail

assert_kagemusha_v4_kagami_custody || exit 1
if /usr/bin/env -i LANG=C LC_ALL=C PATH=/usr/bin:/bin \
  "${KAGEMUSHA_V4_KAGAMI_BIN}" \
  kagemusha prepare-activation-v4 \
  --artifact-root /srv/iroha-kagemusha/taira-v4-r1/catalog \
  --release-policy /srv/iroha-kagemusha/taira-v4-r1/policy/release-policy-v1.norito \
  --manifest-sha256 "${MANIFEST_SHA256}" \
  --verifier-version "${NEXT_VERIFIER_VERSION}" \
  --device-attestation-policy /absolute/private/path/device-attestation-policy.json \
  --output "${ACTIVATION_JSON}" \
  | /usr/bin/tee "${PREPARE_REPORT}"
then
  KAGEMUSHA_COMMAND_STATUS=0
else
  KAGEMUSHA_COMMAND_STATUS=$?
fi
assert_kagemusha_v4_kagami_custody || exit 1
test "${KAGEMUSHA_COMMAND_STATUS}" -eq 0 || exit "${KAGEMUSHA_COMMAND_STATUS}"

PREPARED_REPORT_LINE="$(/usr/bin/tail -n 1 "${PREPARE_REPORT}")"
if ! /usr/bin/jq -e \
  --arg manifest_sha256 "${MANIFEST_SHA256}" \
  --argjson verifier_version "${NEXT_VERIFIER_VERSION}" \
  --arg device_policy_state_sha256 \
    "${REVIEWED_DEVICE_ATTESTATION_POLICY_STATE_SHA256}" \
  '
    ($manifest_sha256 | test("^[0-9a-f]{64}$") and (test("^0{64}$") | not)) and
    ($device_policy_state_sha256 | test("^[0-9a-f]{64}$") and (test("^0{64}$") | not)) and
    ($verifier_version | type == "number" and . == floor and . >= 0 and . <= 4294967295) and
    type == "object" and
    .status == "prepared" and
    .manifest_sha256 == $manifest_sha256 and
    .verifier_version == $verifier_version and
    .instruction_count == 1 and
    .device_attestation_policy_state_sha256 == $device_policy_state_sha256 and
    (.instructions_hash |
      type == "string" and
      test("^[0-9a-f]{64}$") and
      (test("^0{64}$") | not))
  ' <<<"${PREPARED_REPORT_LINE}" >/dev/null
then
  echo "Kagami activation report does not match reviewed release inputs" >&2
  exit 1
fi

INSTRUCTIONS_HASH="$(/usr/bin/jq -er '.instructions_hash' <<<"${PREPARED_REPORT_LINE}")"
test -n "${INSTRUCTIONS_HASH}"
```

Immediately before proposing—and again before the quorum-crossing
approval—capture the committed height from every validator and repeat the
future-height check. Stop if the checked margin has been consumed; do not
submit a stale release or edit its authenticated heights. Generate, finalize,
install, and qualify a new release instead.

```bash
VALIDATOR_READ_CFGS=(
  /absolute/runtime-only/validator-1-read-client.toml
  /absolute/runtime-only/validator-2-read-client.toml
  /absolute/runtime-only/validator-3-read-client.toml
  /absolute/runtime-only/validator-4-read-client.toml
)
VALIDATOR_OPERATOR_KEY_FILES=(
  /absolute/runtime-only/validator-1-operator.key
  /absolute/runtime-only/validator-2-operator.key
  /absolute/runtime-only/validator-3-operator.key
  /absolute/runtime-only/validator-4-operator.key
)
HEIGHT_EVIDENCE_DIR="${HEIGHT_EVIDENCE_DIR:?set a fresh evidence directory for this check}"
test "${#VALIDATOR_READ_CFGS[@]}" -eq 4
test "${#VALIDATOR_OPERATOR_KEY_FILES[@]}" -eq 4
test ! -e "${HEIGHT_EVIDENCE_DIR}"
mkdir -m 700 "${HEIGHT_EVIDENCE_DIR}"

for index in "${!VALIDATOR_READ_CFGS[@]}"; do
  iroha --machine --config "${VALIDATOR_READ_CFGS[$index]}" \
    --operator-private-key-file "${VALIDATOR_OPERATOR_KEY_FILES[$index]}" \
    --output-format json ops sumeragi status \
    | tee "${HEIGHT_EVIDENCE_DIR}/validator-$((index + 1)).json" >/dev/null
done

MAX_COMMITTED_HEIGHT="$(
  /usr/bin/python3 -I -S - \
    "${HEIGHT_EVIDENCE_DIR}"/validator-{1,2,3,4}.json <<'PY'
import json
import sys

heights = []
for path in sys.argv[1:]:
    with open(path, encoding="utf-8") as source:
        height = json.load(source).get("last_committed_height")
    if (
        not isinstance(height, int)
        or isinstance(height, bool)
        or not 0 < height <= 2**64 - 1
    ):
        raise SystemExit(f"invalid last_committed_height in {path}: {height!r}")
    heights.append(height)
print(max(heights))
PY
)"

/usr/bin/python3 -I -S - \
  "${MAX_COMMITTED_HEIGHT}" \
  "${ACTIVATION_SUBMISSION_MARGIN_BLOCKS}" \
  "${ACTIVATION_HEIGHT}" <<'PY'
import sys

current, margin, activation = map(int, sys.argv[1:])
if not 0 < margin <= 2**64 - 1:
    raise SystemExit("activation submission margin is outside the positive u64 range")
if not 0 < current <= 2**64 - 1 or not 0 < activation <= 2**64 - 1:
    raise SystemExit("committed or activation height is outside the positive u64 range")
if activation <= current + margin:
    raise SystemExit(
        f"stale release: activation height {activation} is not beyond fleet height "
        f"{current} plus the {margin}-block submission margin"
    )
print(
    f"checked activation window: fleet={current}, "
    f"activation={activation}, margin={margin}"
)
PY
```

Submit the immutable instruction file through the authorized multisig. The
proposer automatically supplies the first approval. Each signer uses its own
runtime-only client config, and every submitting transaction explicitly pays
fees from its authority. First use the CLI's no-submit `--output` mode to
reparse that same file and prove that the proposal key equals Kagami's
`INSTRUCTIONS_HASH`; then capture the machine-readable transaction hashes:

```bash
SIGNER1_CFG=/absolute/runtime-only/activation-signer-1.toml
SIGNER_N_CFG=/absolute/runtime-only/activation-signer-N.toml
PROPOSAL_PREVIEW=/absolute/private/path/kagemusha-proposal.preview.txt
PROPOSAL_RESULT=/absolute/private/path/kagemusha-proposal.json
APPROVAL_N_RESULT=/absolute/private/path/kagemusha-approval-N.json

iroha --machine --config "${SIGNER1_CFG}" --output --output-format text \
  ledger multisig propose \
  --account "${ACTIVATION_MULTISIG_ACCOUNT}" \
  < "${ACTIVATION_JSON}" \
  | tee "${PROPOSAL_PREVIEW}"

CLI_INSTRUCTIONS_HASH="$(sed -n 's/^instructions_hash: //p' \
  "${PROPOSAL_PREVIEW}" | head -n 1)"
test "${CLI_INSTRUCTIONS_HASH}" = "${INSTRUCTIONS_HASH}"

iroha --machine --config "${SIGNER1_CFG}" --fee-payer authority \
  --output-format json ledger multisig propose \
  --account "${ACTIVATION_MULTISIG_ACCOUNT}" \
  < "${ACTIVATION_JSON}" \
  | tee "${PROPOSAL_RESULT}"

PROPOSAL_TX_HASH="$(jq -er '.hash' "${PROPOSAL_RESULT}")"

iroha --machine --config "${SIGNER_N_CFG}" --fee-payer authority \
  --output-format json ledger multisig approve \
  --account "${ACTIVATION_MULTISIG_ACCOUNT}" \
  --instructions-hash "${INSTRUCTIONS_HASH}" \
  | tee "${APPROVAL_N_RESULT}"

APPROVAL_N_TX_HASH="$(jq -er '.hash' "${APPROVAL_N_RESULT}")"
```

Repeat the approval command only for the independently authorized signers
required by the configured quorum, using a different signer config and result
file each time. Prove `Applied` finality for the proposal, every approval, and
especially the quorum-crossing approval that executes the nested activation:

```bash
iroha --machine --config "${READ_CFG}" --output-format json \
  tx status --hash "${PROPOSAL_TX_HASH}" --wait \
  --terminal-status applied --timeout-ms 120000 --poll-interval-ms 500

iroha --machine --config "${READ_CFG}" --output-format json \
  tx status --hash "${APPROVAL_N_TX_HASH}" --wait \
  --terminal-status applied --timeout-ms 120000 --poll-interval-ms 500
```

After every validator has committed at least `ACTIVATION_HEIGHT`, capture its
consensus status and the exact release-qualified Eq/Ep registry records. The
operator key remains a runtime-only mode-`0600` file:

```bash
VALIDATOR_CFG=/absolute/runtime-only/validator-client.toml
OPERATOR_PRIVATE_KEY_FILE=/absolute/runtime-only/operator.key

iroha --machine --config "${VALIDATOR_CFG}" \
  --operator-private-key-file "${OPERATOR_PRIVATE_KEY_FILE}" \
  --output-format json ops sumeragi status \
  | tee /absolute/private/path/validator-sumeragi-status.json

iroha --machine --config "${VALIDATOR_CFG}" --output-format json \
  app zk vk get \
  --backend halo2/ipa-pasta-cycle-compact-v5 \
  --name "kagemusha-recursive-spend-step-eq-compact-layout-v5-${MANIFEST_SHA256}" \
  | tee /absolute/private/path/validator-kagemusha-eq-vk.json

iroha --machine --config "${VALIDATOR_CFG}" --output-format json \
  app zk vk get \
  --backend halo2/ipa-pasta-cycle-compact-v5 \
  --name "kagemusha-recursive-spend-step-ep-compact-lineage-v5-${MANIFEST_SHA256}" \
  | tee /absolute/private/path/validator-kagemusha-ep-vk.json
```

Require `.last_committed_height >= ACTIVATION_HEIGHT`, verifier version
`NEXT_VERIFIER_VERSION`, activation height `ACTIVATION_HEIGHT`, and the expected
Eq/Ep identities and commitments. The complete Eq and Ep JSON records must be
identical across all four validators. Failure or absence on any peer blocks the
canary.

Construct and sign the canary with the typed Rust wallet API
(`Client::submit_offline_top_up` / `Client::submit_offline_redeem`) or the
corresponding IrohaSwift or Kotlin Kagemusha wallet APIs. There is no generic
CLI command that safely invents these release-bound requests. Let the typed API
send the signed `POST /v1/offline/top-up`, wait until its operation is terminal
`applied`, and only then construct the full redemption from that committed
wallet state. Let it send the signed `POST /v1/offline/redeem`, wait for that
operation to become terminal `applied`, and reconcile escrow, wallet, and final
balances.

Only when a wallet explicitly exports the canonical signed Norito request
archive may a raw transport client send those already-constructed bytes. Such
a request uses exact `Content-Type: application/x-norito` and exactly one
lowercase 64-hex operation ID as `Idempotency-Key`; it does not add
`X-Iroha-Account`, `X-Iroha-Signature`, or other substitute authentication
headers. Poll `/v1/offline/operations/{operation_id}` until terminal `applied`.
A `pending` or `rejected` response is not a production canary pass.

`render_taira_validator_bundle.py` rewrites the checked-in peer-1 baseline with
the complete `trusted_peers` / `trusted_peers_pop` roster, the five-dataspace
catalog, and operator-provided runtime credentials. It rejects retired offline
enrollment fields instead of turning them into node configuration. Use the
ordinary empty-reset composer shown above; do not use the retired
offline-specialized reset helpers.

The Digital Shekel assets and accounts are ordinary genesis or post-genesis
state. Register `ds#boi.is` and `ds#boi.is2` at scale 2 as required by their
applications, but do not attach offline enablement metadata. Top-up and
redemption derive and validate their exact asset, scale, escrow, authority,
release, and balance bindings when the operation is submitted. A hosted Torii
command-submitter key is an application-service credential, not a validator
capability switch.

Before cutover, prove that every validator runs the admitted binary and the
same lane-to-dataspace mapping, and that each non-universal dataspace is backed
by its declared distinct server/validator cohort and storage boundary. A
shared catalog or repeated lane-manifest roster is not evidence of a separate
physical dataspace. Health checks use ordinary node and consensus evidence.
Mobile QR/NFC/Nearby acceptance remains an app/device release test and never
changes validator admission.

## Private profiles

The canonical catalog and lane mapping live in this repository; runtime host
allocations, credentials, per-dataspace manifests, and application-specific
private-dataspace profiles do not. The checked-in roster and ordinary render
command produce the `universal` cohort only; they are not a five-cohort release
assembler. Keep each additional physical dataspace profile in the deployment
repository and pass it to the renderer explicitly. A lane and a dataspace may
use the same human-readable alias (as `dpn` and `cbsi` do), but their typed
identities remain separate and one must never be inferred from the other. Do
not claim a distinct dataspace without a distinct deployed cohort and its
manifest:

```bash
python3 scripts/render_taira_validator_bundle.py \
  --base-config /absolute/path/to/private-profile.toml \
  --roster configs/soranexus/taira/validator_roster.local.toml \
  --secrets configs/soranexus/taira/validator_secrets.local.toml \
  --output-dir dist/taira-private-validators
```

For a true genesis reset on a validator host, stop the runtime and wipe the
mounted state before starting again:

```bash
bash configs/soranexus/taira/taira-validator-container.sh \
  --env-file /etc/default/taira-validator-container.compose.env reset

bash configs/soranexus/taira/taira-validator-container.sh \
  --env-file /etc/default/taira-validator-container.compose.env up
```

That sequence removes the mounted validator state under `TAIRA_STORAGE_PATH`
after stopping the container, which is the required step for a true genesis
reset. Reset refuses broad system roots, the read-only config bundle, and equal
or nested state roots before it stops the running container.

When you run the shared nginx edge on one host, keep the same roster as the
source of truth for the edge upstreams too:

- add `edge_torii_upstream = "<host>:<port>"` for each validator entry
- render the nginx snippet from that roster instead of hand-editing ports:
  - `python3 scripts/render_taira_edge_nginx_conf.py --roster configs/soranexus/taira/validator_roster.local.toml --output dist/taira-edge/taira.sora.org.conf`
- add `[[soracloud_alias_routes]]` entries to the same local roster for
  dedicated runtime aliases that should terminate at local service listeners.
  For the Solswap indexer on the shared host:
  - `alias = "solswap-indexer.sora"`
  - `edge_upstream = "127.0.0.1:8788"`

That avoids the common drift where the copied nginx snippet still points at
`127.0.0.1:18080..18083` while the live validator listeners have moved to
different loopback ports such as `127.0.0.1:29080..29083`, which turns
`GET /v1/mcp` and the generic public API surface into `502 Bad Gateway`.

## Adopt existing macOS storage into independent supervision

A shared shell runner must not be the long-lived production supervisor. Both
the historical `run-canonical.sh` and generated `launchd-run.sh` own all four
validator children and run an all-peer cleanup trap when one child or the
controller exits. Use the guarded migration below to retain the exact existing
Kura/snapshot state while giving each peer its own launchd failure boundary.

The planning phase only inspects processes and writes a new staging directory:

```bash
python3 scripts/migrate_taira_peer_supervision.py plan \
  --base /absolute/path/to/the/deployed/taira-rollout \
  --output-dir /absolute/path/to/new/taira-supervision-plan
```

It requires four exact peer PID files, exact `iroha3d --sora --config ...`
commands, one common parent whose command names an approved
`run-canonical.sh` or `launchd-run.sh`, non-symlink configs, and four distinct
existing storage directories. Each live peer's exact working directory is
sealed separately from its storage inode; they are intentionally not assumed
to be the same path. The printed manifest digest seals those process, binary,
config, PID-file, working-directory, storage, and generated plist identities.
The terminal-latch contract is supervision manifest schema v4; pre-latch v3
plans are rejected and must be regenerated rather than reinterpreted.
Review the manifest and plists before scheduling the cutover. Generic adoption
uses the supervisor's full-hash path for a binary below the non-root-owned
deployment base. Supplying a separately installed root-controlled `--irohad`
path makes the planner emit all five stat-seal arguments and enables O(1)
validation, as does the live reset controller's content-addressed artifact
store.

Apply only with no active ledger writer and during an announced maintenance
window:

```bash
sudo python3 scripts/migrate_taira_peer_supervision.py apply \
  --manifest /absolute/path/to/new/taira-supervision-plan/manifest.json \
  --expected-manifest-sha256 SHA256_PRINTED_BY_PLAN \
  --confirm ADOPT-EXISTING-TAIRA-STORAGE
```

`apply` rechecks every sealed identity before mutation, retains the exact
authenticated staged asset bytes in memory through installation, installs one
`KeepAlive` LaunchDaemon per validator, stops the legacy controller once, and
starts all four jobs from their exact original working directories while
retaining their separately identified storage directories. It never deletes,
moves, truncates, or recreates storage and refuses to use `SIGKILL` if the
legacy topology does not stop within the configured timeout. Each job runs
only its own validator from that peer's exact legacy working directory,
independently rechecks the separate storage inode before every child start,
forwards shutdown signals, and restarts only that validator with exponential
backoff capped by
`--maximum-backoff-seconds` (30 seconds by default). launchd also throttles a
supervisor-level crash loop. The peer supervisor additionally stops a
deterministic startup-fatal loop after three identical normalized rapid exits.
It keeps the launchd-owned supervisor process alive, removes the child PID
file, and durably publishes an owner-private terminal-unhealthy fingerprint.
An unchanged binary/config/restart-generation binding remains latched across a
supervisor or launchd restart. A changed binary digest/stat identity, config
digest, or explicit `--restart-generation` selects a new binding and permits a
fresh start; changing only the generation is the operator-controlled reset for
an otherwise identical deployment. Nonfatal, signaled, slow, or non-identical
exits continue the same capped exponential retry policy.

Binary, config, working-directory, or storage replacement is fail-closed after
migration. Render a new reviewed plan before an intentional
binary/config/working-directory/storage change; do not edit the installed
plist or supervision receipt in place. The shared public nginx origin should
remain pinned to the chosen canonical validator and be moved only through the
direct height/state parity gate described below.

## SCCP V1 on Taira

Taira is the only SORA settlement target for SCCP V1. The exact runtime target
is the public chain id above and I105 discriminant `369` (`0x0171`). Do not use
the generic/default `753` discriminant for SCCP authorities or recipients.

SCCP policy is not a validator-local `[zk]` overlay and is not loaded from a
route manifest. Consensus owns one typed `SccpRegistryV1`, changed through
authorized `ApplySccpRouteGovernance` transactions. Rendered validator bundles
therefore use the normal Taira configuration; never hand-merge SCCP route
material into per-host TOML files.

The public read-only discovery surface is:

- `GET /v1/sccp/capabilities`;
- `GET /v1/sccp/registry`;
- `GET /v1/sccp/proofs/message/{message_id}`;
- `GET /v1/sccp/proof-requests/{message_id}`; and
- `GET /v1/sccp/messages/recent`.

The exact submit endpoints are `POST /v1/bridge/proofs/submit` for a governed
destination artifact and `POST /v1/bridge/messages` for a native inbound proof.
Preparation omits both detached-signing fields; direct submission provides both
`signature_b64` and the prepared `transaction_payload_b64` with the exact
positive `creation_time_ms`. Do not send private keys, governance credentials,
or node-local route overrides to Torii.

Ethereum, BSC, and TRON mainnet are the production remote profiles. Sepolia,
BSC testnet, Nile, and Shasta are test profiles only and cannot certify public
Taira production readiness. A lane becomes usable only after typed governance,
authenticated contract/native-verifier readback, audited proof material, and
the signed SCCP release-evidence corridor all succeed. The retired
`/v1/sccp/manifests` route readiness check and the old Nile route-config script
are not part of the first-release operator workflow.

## Routine testnet updates (30-minute path)

Use `.github/workflows/update_taira_testnet.yml` for ordinary Taira code
updates. It is the default operational path. It checks out the selected exact
workflow commit on the existing
`[self-hosted, macOS, ARM64, taira-deploy]` runner, builds only `iroha3d` with
`embedded-soracloud-runtime,zk-stark`, and rolls the four validators in place.
There is no Linux build, privacy/BOI qualification, candidate authority,
cross-job artifact handoff, OCI publication, or empty-state reset in this
path. A parallel hosted watchdog cancels an unavailable runner after two
minutes and cancels the whole build-and-update after 30 minutes.

Provision the fixed updater once on the deployment Mac; never run its checkout
copy through `sudo`:

```bash
sudo install -o root -g wheel -m 0555 \
  scripts/deploy_taira_testnet_update.py \
  /usr/local/libexec/iroha-taira-testnet-update-v1
shasum -a 256 /usr/local/libexec/iroha-taira-testnet-update-v1
```

Allow the Actions runner to execute only that installed command without a
password, and set the resulting digest as the repository or organization
Actions variable `TAIRA_TESTNET_UPDATER_SHA256`. The fast job deliberately has
no protected release environment or approval wait. `TAIRA_NATIVE_BUILD_JOBS`
may override the default six Cargo jobs, and `TAIRA_TESTNET_CARGO_TARGET_DIR`
may select the persistent target directory. That writable cache belongs only
on the protected testnet deployment runner; do not share it with pull-request
or other untrusted jobs.

The updater hashes the candidate, validates all four existing configs in
parallel, installs the binary under
`/Library/SORA/Taira/binaries/<sha256>/iroha3d`, and updates one launchd job at
a time. After each peer it requires loopback `/health`, `/readyz`, and the
exact workflow commit in `/status`. If a peer does not return, every touched
peer is restored in reverse order before the updater exits. The updater never
replaces or clears config, genesis, working-directory, or storage paths. Keep
old content-addressed binaries until the operator decides they are no longer
needed. Changes that intentionally migrate an incompatible storage format need
a separately planned testnet migration; they are not ordinary binary updates.

## Dual-target archive publication (exceptional reset)

The first Taira release is archive-only. The manual
`.github/workflows/publish_taira_validator.yml` workflow does not build or push
an OCI image and has no image or legacy publication fallback. OCI is used only
as an immutable generic-artifact transport for secret-free admission evidence.
The dispatch has no arbitrary checkout input. Each job validates the exact
lowercase 40-hex DPN source-closure commit independently, validates the
immutable `${{ github.sha }}` workflow commit before checkout, checks out only
that commit with persisted Git credentials disabled, and runs with
`contents: read` permission.

One dispatch performs these two native builds from one authenticated source
identity:

- The `[self-hosted, Linux, ARM64, taira-untrusted-build]` job reconstructs the reviewed DPN
  source closure and exact `Cargo.lock` only after sealing its controller
  closure, checks the canonical workspace-source manifest, snapshots exactly
  four secret-free privacy inputs, then builds the unsigned Linux/aarch64
  rollout archive in a scrubbed environment. Only after compilation and native
  evidence generation finish does the sealed finalizer authenticate the
  immutable archive and sign its exact-12 authority plus controller manifest.
  An amd64 or emulated build is rejected.
- The `[self-hosted, macOS, ARM64, taira-untrusted-build]` job reconstructs that
  same commit, lock, and source manifest independently. It starts concurrently
  with the Linux path because it does not consume Linux authority bytes; the
  later secret-free qualification job performs the first cross-target identity
  comparison. No source-built genesis signer is compiled or invoked. The exact binary
  then boots exactly four native peers, proves consensus advancement, replaces
  each peer child in turn through the shipped supervisor, and proves fleet
  advancement after every restart. Both the candidate binary and supervisor
  run from temporary root-controlled, content-addressed validation paths so a
  child cannot rewrite its own harness.

The workflow has one 30-minute wall-clock watchdog. Historical image dispatches
spent entire 24-hour windows queued against an unassigned `iroha2` runner and
executed zero steps; repeated serialized dispatches accounted for the apparent
roughly 100-hour rollout. The watchdog cancels after two minutes with no active
job, or at the overall budget boundary, and reports each pending job, runner
name, and requested labels. Keep every listed authority runner online before
dispatch.

Before any self-hosted job or Cargo invocation, the hosted `release-readiness`
job runs `scripts/check_taira_release_prerequisites.py`. It reports and rejects
any critical-path authority that remains an unconditional source-level refusal.
This is intentionally the current result until the independent native-evidence,
protocol-receipt, Exact12-governance, BOI, deploy-issuance, post-deploy native-
evidence, rollout-observation, public-soak producer, and distinct public-soak
observation authorities are implemented and provisioned. The gate
prevents an unavailable release from spending build hours merely to reach the
same refusal later.

The release corridor's fixed 24-hour Taira-profile soak is a four-process local
fault-profile gate; it is not evidence that the deployed public validator cohort
served sustained workload. Public deployment evidence uses the separate
`scripts/check_taira_public_v2_24h_soak_evidence.py` contract: exactly four
validators, quorum three, an exact 86,400-second monotonic workload window, and
432,000 individually inventoried signed transfers scheduled at five per second,
followed by a bounded application/finality drain. The evidence distinguishes
typed Iroha hashes from artifact digests and binds submission, global Applied,
executed-block, finality, deploy-descendant, and zero-lifecycle-drift evidence.
It publishes no authority itself. Admission remains source-disabled until an
independent Ed25519 verifier, pinned native evidence verifier, and atomic replay
broker provision the dedicated public-soak authority contract and durable
admission receipt; no live public-soak receipt is currently claimed.

`scripts/taira_public_v2_24h_soak_state.py` supplies the controller-side lease
primitive for that future runner. One process holds the lock for the entire
attempt; state transitions are atomic and bind the exact source, three handoffs,
deployed tip, native-verified soak anchor and producer launch, controller, and
native verifier. A crash leaves a non-resumable attempt, and clock drift, a
missed capture window, state rewriting, or root-path replacement fails closed.
The module is not a workload runner and is not an installed controller
operation; exposing it directly would exit without contacting Taira or
producing evidence. The reset and legacy-migration plist renderers accept the
exact journal root, validator ID, and node ID only as one all-or-none identity.
Each validator now receives an explicit runtime-only Torii secp256k1 receipt
key pair. Its public key and domain-separated derived node ID are carried,
without the private key, through the reset manifest, four-peer receipt,
admission, qualification, and deployment records. Deployment rederives the
supervisor runtime and lifecycle bindings from the exact binary stat seal,
config digest, restart generation, validator slug, and node ID. This closes the
former pre-deploy lifecycle node-ID source barrier; the separate deployment-
issuance authority remains source-disabled.

The opt-in supervisor journal records owner-private per-peer lifecycle windows.
`scripts/collect_taira_public_v2_lifecycle_evidence.py` validates those four raw
chains, retains the exact source artifacts and their deploy-bound identities,
globally resequences them, and publishes final lifecycle evidence only after
passing all five journals to a digest-pinned native verifier and capturing its
exact request-bound receipt. The collector is still an offline helper, not an
installed controller operation or an authority. The candidate and publication
prerequisite handoffs now have path-only producers behind two `macos-publish`
controller operations, and the publication workflow retains their root-closed
outputs. They remain fail-closed until the existing native-evidence, privacy-
origin, and rollout-observation authorities are provisioned. The structural
post-deploy handoff producer validates the exact applied reset report, reset
manifest, four native-receipt files, candidate/publication handoffs, and
installed deploy-controller attestation before emitting the closed deploy
identity consumed by the soak checker. Its public entry point still refuses
before path I/O until the independent deploy-native evidence authority is
provisioned, and it is not installed as a controller/workflow operation.
`scripts/run_taira_public_v2_24h_soak.py` now supplies the structural long-lived
runner around that contract. Its private injected-backend seam binds the full
candidate/publication/deploy/anchor launch subject, dispatches exactly 432,000
monotonic slots, holds the lease through capture, streams the bounded evidence
inventories, and requires exact native launch and backend-shutdown receipts.
Its public API and CLI remain an unconditional source refusal before caller
path or network I/O. A genuine protected runtime signer/native verifier,
observation authority, replay broker, and controller/workflow integration are
still required before any deployed-public soak can start or claim completion.

Both untrusted native builders default to six Cargo jobs and reuse a Cargo
target outside the authenticated source checkout only for retries of the same
workspace-manifest, DPN commit, and Rust compiler identity. Different source or
toolchain identities never share writable target state. Set
`TAIRA_NATIVE_BUILD_JOBS` to a positive value no greater than 16 when host
memory requires another bound. `TAIRA_LINUX_BUILD_CACHE_ROOT` and
`TAIRA_MACOS_BUILD_CACHE_ROOT` may select owner-private absolute cache roots;
otherwise the jobs use distinct paths beneath `RUNNER_TOOL_CACHE`. These
source-keyed caches never cross into an authority job. Every produced byte
remains hostile until the existing signed authority and native qualification
stages accept it. The seven immutable DPN source inputs download concurrently;
both curl's per-transfer and retry-window limits are 120 seconds.

This workflow remains the exceptional first-release archive,
privacy-evidence, and empty-v21-reset path. It is not used for routine testnet
updates: the reset controller authenticates four empty storage trees and
replaces the fleet as one cohort. Use `update_taira_testnet.yml` for the normal
state-preserving rolling path above.

Neither source reconstruction nor either native Cargo step receives protected
authority, privacy-source, reset-source, staging-root, or external-genesis-signer
environment variables; both native compilation phases construct an explicit
`env -i` allowlist. The macOS reset and external-signer path/digest values are
introduced only in the private reset preparation step, captured into
non-exported shell locals, and removed from the reset composer's environment.
The composer digest-pins and snapshots the reviewed external signer, then
invokes it with a minimal environment and no key material. Release signer and
verifier values are introduced only in authentication/signing steps; the Linux
finalizer similarly unsets their exported names before invoking its single
audited command with explicit arguments.

Both jobs seal their complete privileged-controller closures before source
reconstruction. The bootstrap stable-reads the sealer, compares those bytes to
`git show <workflow-commit>:scripts/seal_taira_release_controllers.py` using
`/usr/bin/git` with system/global configuration disabled, and only then runs
the reviewed bytes as root. All reconstruction, finalization, reset, capture,
candidate, deployment, health, admission, manifest, and receipt-signing helpers
execute from the sealed closure. The Linux physical root is
`/var/tmp/iroha-taira-controller-linux.*`; macOS uses the canonical physical
`/private/var/tmp/iroha-taira-controller-macos.*` path. The controller digest
is signed into the Linux authority, reset manifest, and final macOS candidate.

`capture_taira_macos_four_peer_receipt.py` emits the canonical receipt only
after all four restart proofs pass and the original reset inputs are unchanged.
The untrusted macOS build job only compiles and inventories the Exact12 native
test drivers. `capture_taira_privacy_protocol_four_peer_receipt.py` runs those
frozen drivers inside secret-free qualification, never invokes Cargo, and
preserves one bounded canonical transcript/result pair for each of the six
protocol cases plus the independent governance case. The v2 privacy receipt
binds those bytes, both driver digests, the macOS handoff, validator, Linux
archive, Exact12 matrix, and complete source identity. Admission rejects the
v1 receipt and independently reads, hashes, and parses every preserved v2
record before accepting the signed candidate.
`build_taira_rollout_candidate.py` binds that receipt, the exact binary,
the sealed macOS controller manifest, and signed Linux authority into the final
secret-free Mac admission archive. The private reset bundle, validator secrets,
external genesis software signer and its encrypted runtime-only key state,
validator binary, and
supervisor never enter Actions artifact storage or OCI. After the
candidate authority is assembled, the protected macOS runner passes the local
private bundle and candidate directly to `scripts/deploy_taira_v21_reset.py`,
first without `--apply` for read-only preflight and then with `--apply` for the
guarded four-validator cutover. The same job then requires the public Torii
root and all four protected direct-validator roots to report the exact full
source commit and three advancing aligned fleet samples before publication.

The live bundle is never composed beneath `RUNNER_TEMP`: launchd retains its
bundle-local config, genesis, runtime-sidecar, and storage paths after the job
ends. `TAIRA_MACOS_RESET_STAGING_ROOT` therefore names canonical persistent
storage outside both the checkout and runner temp, owned by the runner user at
exact mode 0700. Each dispatch creates one fresh child whose 64-hex name binds
the exact workflow commit, DPN source-closure commit, reconstructed source
manifest, run ID, and attempt. Do not remove that child while its cohort is
live. The separate `TAIRA_MACOS_SOURCE_RESET_BUNDLE_SHA256` protected value
pins the complete authenticated source-reset tree passed to the native reset
composer; a path alone is not an authority.

Only the admission archive and its `release_manifest.json`, public key, and
signature are staged. The workflow records those exact four files in a
canonical byte inventory, uploads and downloads that secret-free set through
Actions storage, byte-compares every file, and repeats admission before any
registry mutation. Including the inventory itself, the Actions handoff has
exactly five files; the inventory is metadata and is not an OCI layer.

Registry publication uses ORAS `1.3.2` through
`oras-project/setup-oras@22ce207df3b08e061f537244349aac6ae1d214f6` and the
official Darwin arm64 archive. The workflow pins its checksum to
`7929f792cf272268412375ecad6f0fb3c20f164368d5b57966e67ad6d36eca53`.
Do not update any of those three values independently. The primary artifact
and layers use these fixed types:

- `application/vnd.hyperledger.iroha.taira.rollout-admission.v1`
- `application/vnd.hyperledger.iroha.taira.rollout-admission.archive.v1+tar+gzip`
- `application/vnd.hyperledger.iroha.release-manifest.v1+json`
- `application/vnd.hyperledger.iroha.release-manifest.signature.v1+ed25519`
- `application/vnd.hyperledger.iroha.ed25519-public-key.v1`
- `application/vnd.oci.image.manifest.v1+json` with the canonical
  `application/vnd.oci.empty.v1+json` two-byte `{}` config descriptor

The mutable-looking source tag is only a publication locator. The workflow
accepts the digest returned by `oras push` only after `oras resolve` agrees,
the raw OCI manifest hashes to that same digest, and every descriptor matches
the expected path, media type, size, and SHA-256. It then performs a pull by digest
into a fresh directory, byte-compares the admission archive and three-file
authority tuple, and reruns admission. The resulting
`repository@sha256:...` reference is an immutable evidence reference, not a
deployment payload; deployment has already consumed the private local bundle.

Finally, the workflow creates a canonical receipt binding the source identity,
admission result, immutable primary manifest digest, every layer, and exact ORAS
provenance. It signs that receipt with the protected release authority, attaches
the tuple as a second fixed-type OCI generic artifact, pulls the attachment by
its immutable digest, and byte-verifies its signature. The uploaded signed publication receipt
is the handoff record for testnet rollout.

The protected `taira-validator-publish` environment must provision canonical,
non-symlinked paths outside the checkout for the external signer, raw public
key, pinned `sorafs-validate`, the four-file secret-free
`TAIRA_PRIVACY_RELEASE_INPUT_DIR`, macOS reset bundle, operator identity, and
the independently built external genesis signer. Pin the latter with
`TAIRA_MACOS_GENESIS_EXTERNAL_SIGNER_SHA256`; its path is
`TAIRA_MACOS_GENESIS_EXTERNAL_SIGNER_PATH`. No genesis private-key path or
bytes may be configured in Actions. The protected privacy path is visible only
to the inline snapshot step; native build steps see only its copied public
bytes. The environment also supplies the public genesis key and command
authority, the public Torii root, exactly four direct validator Torii roots,
the canonical owner-private `TAIRA_MACOS_RESET_STAGING_ROOT`, the exact
`TAIRA_MACOS_SOURCE_RESET_BUNDLE_SHA256`, `TAIRA_OCI_REPOSITORY`, and the
`TAIRA_OCI_USERNAME` and
`TAIRA_OCI_PASSWORD` secrets. Protected release-signer, verifier, reset, and
external-genesis-signer paths are step-scoped; the OCI username and password are exposed only within the two
exact login-and-publish steps and are removed by step-local traps plus the
always-run residual logout cleanup. The dispatch
requires the exact 40-character DPN release commit; `artifact_suffix`, when
present, is restricted to a short lowercase OCI-safe component.

Both protected runner accounts require non-interactive sudo for the exact
`/usr/bin/python3 -I -S` controller-seal/cleanup invocation. The macOS account
also requires the already documented exact reset capture and deploy invocations
through `/usr/bin/python3 -E -S`; do not grant a shell, wildcard command, or
workspace-script sudo rule. `/var/tmp` (physical `/private/var/tmp` on macOS)
must retain root ownership and the operating system's standard sticky-directory
policy; the sealed child tree itself is root-owned mode `0555`. A runner that
cannot create, verify, and always clean its exact controller closure is not
release-capable.

## Minimum viable topology

Use at least 4 validator peers (plus optional observers). Single-peer setups are
not representative for NPoS and can stall DA/RBC consensus paths.

Suggested validator hostnames:

- `taira-validator-1.sora.org`
- `taira-validator-2.sora.org`
- `taira-validator-3.sora.org`
- `taira-validator-4.sora.org`

## Bootstrap peers vs active validators

- `trusted_peers` and `trusted_peers_pop` are bootstrap discovery inputs, not
  the validator-admission policy.
- Genesis selects NPoS and commits its election parameters, while
  `nexus.staking.public_validator_mode = "stake_elected"` selects on-chain
  public-lane staking as the active validator-roster authority. These are
  signed protocol inputs, not mutable node-local Sumeragi switches.
- The checked-in/public roster file is therefore a deploy/bootstrap artifact.
  It helps nodes find each other and agree on the bootstrap set after genesis,
  but it does not decide which operators stay active validators over time.
- Taira resets should seed only the minimum bootstrap validators needed to
  start the chain. After genesis, validator-set growth is driven by XOR stake
  plus the active-validator snapshot views.

## Public validator join flow

Use the public-lane staking flow for validator candidacy instead of manual
allowlisting:

1. Render a per-validator config with the node's own `public_address` and
   `torii_public_address`, then start `iroha3d` against the published seed peers.
2. Wait for the node to sync and confirm lane mode:
   - `iroha --operator-private-key-file /run/secrets/taira-operator-private-key app nexus lane-report --summary`
   - `curl -sS "${PUBLIC_TORII_ROOT}/status" | jq .`
3. Fund the candidate account with `xor#universal`.
4. Register the validator on the public lane with its live peer identity:
   - `iroha app staking register --lane-id 0 --validator <i105-account-id> --peer-id <peer-id> --initial-stake <amount>`
5. When the activation boundary is reached, activate the candidacy if needed:
   - `iroha app staking activate --lane-id 0 --validator <i105-account-id>`
6. Verify that the node is visible through on-chain staking and validator-set
   views rather than a static file roster:
   - `iroha app nexus public-lane validators --lane 0 --summary`
   - `iroha app nexus public-lane stake --lane 0 --validator <i105-account-id> --summary`
   - `curl -sS "${PUBLIC_TORII_ROOT}/v1/nexus/public-lanes/0/validators" | jq .`
   - run the operator-authenticated rollout gate below; it also validates
     `/v1/sumeragi/validator-sets` against the live consensus roster

## Public endpoints

- `https://taira.sora.org` is the primary public Torii/API origin on the
  current deployment. Keep it on Torii/API duties only and do not mount
  websites at its root.
- Every public validator should still be able to expose Torii directly on its
  own TLS hostname and advertise that URL through `[torii].public_address`
  when validator-specific ingress is desired.
- `https://taira-explorer.sora.org` points to the Iroha 2 Explorer instance.
- Shared nginx edge configs such as `taira-explorer.nginx.conf` are optional
  convenience infrastructure, not the primary public API design.

### SoraFS CID gateway

Taira serves SoraFS-published static content primarily through immutable CID
gateway paths on the Torii origin:

- `GET /sorafs/cid/<cid>`
- `GET /sorafs/cid/<cid>/<path...>`
- `GET /v1/sorafs/cid/<cid>` for lookup metadata

For the Polkaswap static bundle, the browser URL is:

- `${PUBLIC_TORII_ROOT}/sorafs/cid/<cid>`

This keeps the chosen public node as the Torii/API origin while giving every
public Torii node an IPFS-style address surface for static content.

Gateway behavior:

- Torii serves CID routes from local storage when the manifest is already
  cached.
- On a local miss, Torii resolves the CID through the approved replication
  order set, uses the provider advert cache to find a Torii-capable provider,
  fetches the manifest and payload over the existing storage endpoints, and
  stores the bundle locally before serving it.
- Keep both `torii.sorafs.discovery_enabled = true` and
  `torii.sorafs_storage.enabled = true` on public gateway nodes so CID
  browsing can rehydrate from peer providers.

Named host bindings in `sorafs_sites.json` remain available as an optional
alias layer, but they are no longer the primary deployment path. Reserve
`taira.sora.org` for Torii itself and serve apps from `/sorafs/cid/<cid>` or
`<cid>.sorafs.taira.sora.org`.

Named bindings are production configuration, not a runtime environment
override. Add the following to each rendered validator config and restart
Torii after changing the document:

```toml
[sorafs.gateway.site_bindings]
path = "/config/sorafs_sites.json"
max_bytes = 1048576
max_sites = 1024
```

The file must be a regular, single-link file owned by the Torii user or root,
must not be group/world writable, and neither it nor an ancestor may be a
symbolic link; ancestor directories must not be group/world writable. Version
1 requires canonical lowercase DNS names, 64-character lowercase manifest
digests, unique hosts, and safe single-component index file names. Invalid
configuration aborts startup instead of falling back to stale or partially
parsed bindings.

Soracloud runtime apps use the SoraDNS/Soracloud alias route instead of SoraFS
CID hosts. For clients without native SoraDNS resolution, the public browser
gateway is `<alias>.mon.taira.sora.net`, for example
`https://solswap-indexer.sora.mon.taira.sora.net/api/indexer/v1/health`. Keep
`https://mon.taira.sora.net/soradns/<alias>/...` available as the Mon debug
fallback and `https://taira.sora.org/soradns/<alias>/...` available only as the
legacy Torii compatibility path.

### Governed gateway compliance

Taira does not load local denylist files or packs. Gateway serving policy comes
only from the threshold-signed, predecessor-bound compliance catalog configured
under `[sorafs.gateway.compliance]`. Catalog construction, acknowledgement,
promotion, rollback, and appeal/hold precedence remain operator-controlled; no
repository bootstrap file authorizes live Taira mutation.

An enabled controller must also pin
`feed_transport_provider_handle`,
`feed_transport_provider_revision`, and
`feed_transport_provider_policy_digest_hex`. The digest is the runtime
transport's non-zero lowercase digest of the exact canonical hostname/SPKI
inventory in the same resolved configuration. Keep credentials out of the
bundle. Missing, partial, zero, test-marked, substituted, or stale bindings
abort startup, and Torii rechecks the identity around every DNS and HTTPS
operation.

Taira's public edge does not accept SoraFS payload uploads in V1. Public
publishers submit only the canonical caller-signed pin-registration
transaction. After finality, each independently administered provider consumes
its committed replication assignment through the durable provider outbox. The
outbox binds the exact finalized height/hash, manifest digest, provider id, and
replication-order id, so public traffic cannot mutate storage, reserve capacity,
or overwrite provider-keyed metadata. Large-body and route-specific timeout or
quota overrides are therefore not part of the Taira storage contract.

After every Taira reset or `irohad` rebuild, verify the manifest-registration
ingress before retrying `yarn taira:publish`:

- `curl -sSki -X POST "${PUBLIC_TORII_ROOT}/v1/sorafs/pin/register" -H 'content-type: application/x-norito' --data-binary ''`

Expected result:

- `HTTP 400` with `x-iroha-reject-code: invalid_transaction_payload` and a
  versioned signed-transaction decode error

Unexpected result:

- `HTTP 405` with `Allow: GET,HEAD`

That `405` means the served `irohad` is stale and missing the mounted
`POST /v1/sorafs/pin/register` route, even if
`GET /v1/sorafs/pin/register` still falls through to the digest lookup
handler.

### Codex / MCP rollout

Each public Taira node should expose native MCP on the same direct Torii root
once the validator is
redeployed with the shipped `[torii.mcp]` block from `config.toml`:

- `torii.mcp.enabled = true`
- `torii.mcp.profile = "writer"`
- `torii.mcp.expose_operator_routes = false`
- `torii.mcp.allow_tool_prefixes = ["iroha."]`

This intentionally exposes only curated `iroha.*` tools on the public network
so Codex sees the stable live-network aliases and not the full raw `torii.*`
OpenAPI-derived surface. The rollout smoke now also rejects any advertised MCP
tool whose top-level `inputSchema` is not an OpenAI-compatible object schema.

After rollout, verify the chosen public node directly:

- `curl -sS "${PUBLIC_TORII_ROOT}/v1/mcp" | jq .`
- `curl -sS "${PUBLIC_TORII_ROOT}/v1/mcp" -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"probe","version":"1"}}}' | jq .`
- `curl -sS -D - "${PUBLIC_TORII_ROOT}/v1/mcp" -H 'content-type: application/json' -d '{"jsonrpc":"2.0","method":"notifications/initialized"}'`
- `curl -sS "${PUBLIC_TORII_ROOT}/v1/mcp" -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"tools/list"}' | jq .`
- `curl -sS "${PUBLIC_TORII_ROOT}/status" | jq .`

The `notifications/initialized` probe should return `HTTP 202 Accepted` with
an empty body. A `200` JSON-RPC error there means the endpoint advertises MCP
but still fails the standard post-initialize handshake that Codex and other
streamable-HTTP MCP clients require.

The `tools/list` payload must also keep every tool's `inputSchema` as a
top-level `"type": "object"` schema without top-level `anyOf`, `oneOf`,
`allOf`, `enum`, or `not`. If a live node still advertises an invalid schema
for tools such as `iroha.connect.session.delete`, `check_mcp_rollout.sh` now
fails the rollout immediately instead of letting Codex discover the breakage.

The repo-local Codex plugin and Taira skill now treat
`https://taira.sora.org/v1/mcp` as the current primary public MCP endpoint
while still allowing operator-provided alternate public roots. Future
Nexus/Torii deployments should keep the same `/v1/mcp` path and be added as
user-local MCP servers with the exact public root under test.

For final public rollout, do not stop at MCP discovery. Run the repo smoke with
the public endpoint, the exact full 40-character deployment git SHA, all four
direct validator roots, a runtime-only canary signer config, and an allow-listed
runtime-only operator key bound to the exact genesis `NetworkId`. The operator
private-key file must be an absolute, singly linked regular file with mode
`0600`; the rollout scripts generate a fresh empty-body GET signature for the
final path and query and never use token fallback, redirects, or retries. Define
the operator inputs and non-optional fleet arguments once:

```bash
export OPERATOR_NETWORK_ID='hash:<exact-genesis-network-id>'
export OPERATOR_PRIVATE_KEY_FILE='/run/secrets/taira-operator-private-key'
operator_get() {
  local url="$1"
  shift
  local header_output
  local -a header_args=()
  header_output="$(python3 scripts/operator_http_headers.py \
    --network-id "${OPERATOR_NETWORK_ID}" \
    --private-key-file "${OPERATOR_PRIVATE_KEY_FILE}" \
    --method GET --url "${url}")" || return
  while IFS= read -r header; do
    header_args+=(--header "${header}")
  done <<<"${header_output}"
  [[ ${#header_args[@]} -eq 8 ]] || return 1
  curl --fail --silent --show-error --max-redirs 0 --retry 0 \
    "$@" "${header_args[@]}" "${url}"
}
TAIRA_VALIDATOR_ARGS=(
  --validator-root validator-1=https://taira-validator-1.sora.org
  --validator-root validator-2=https://taira-validator-2.sora.org
  --validator-root validator-3=https://taira-validator-3.sora.org
  --validator-root validator-4=https://taira-validator-4.sora.org
)
```

- `bash configs/soranexus/taira/check_mcp_rollout.sh --public-root "${PUBLIC_TORII_ROOT}" "${TAIRA_VALIDATOR_ARGS[@]}" --require-all-validators --write-config /run/secrets/taira-canary-client.toml --expected-git-sha "${EXPECTED_TAIRA_GIT_SHA}" --expected-dpn-validator-release-commit "${EXPECTED_DPN_VALIDATOR_RELEASE_COMMIT}"`

The MCP smoke has one absolute 240-second deadline, including all validator
alignment retries, HTTP calls, retry delays, signer bootstrap, faucet work, and
the signed canary. `--deadline-seconds N` changes that positive bound. Every
curl and transaction status timeout is clamped to the remaining budget, and
fleet alignment makes at most two attempts per sample by default. This replaces
the former pathological 200-minute nested retry bound.

Then gate the SoraFS path on the same public node:

- `bash configs/soranexus/taira/check_sorafs_rollout.sh --public-root "${PUBLIC_TORII_ROOT}" --write-config /run/secrets/taira-canary-client.toml`

When `--write-config` is supplied, both rollout scripts read that runtime-only
signer config as-is and fail if it is missing; neither script overwrites or
bootstraps over an operator-supplied path. Omit `--write-config` only when the
intended flow is to bootstrap the default runtime canary config automatically,
and then pass the exact owner-private credential with
`--onboarding-token-file /absolute/runtime/path/onboarding-token`.

Expected result:

- `POST /v1/sorafs/pin/register` and `POST /v1/sorafs/capacity/declare`
  reach their signed-transaction handlers instead of returning `HTTP 404/405`
- the signed capacity canary lands and becomes visible in
  `GET /v1/sorafs/capacity/state`

The SoraFS rollout script validates all numeric canary controls before it
bootstraps a signer or submits a transaction. `ROLLOUT_CANARY_TIME_TO_LIVE_MS`,
`ROLLOUT_CANARY_STATUS_TIMEOUT_MS`, `DECLARED_CAPACITY_GIB`, `STAKE_AMOUNT`,
`DECLARATION_VALID_BLOCKS`, and `CAPACITY_STATE_RECHECK_ATTEMPTS` must be
positive integers; `CAPACITY_STATE_RECHECK_DELAY_SECONDS` must be a
non-negative integer. HTTP probes also use bounded curl timeouts:
`SORAFS_ROLLOUT_CURL_CONNECT_TIMEOUT_SECONDS` and
`SORAFS_ROLLOUT_CURL_MAX_TIME_SECONDS` default to `5` and `20` seconds and can
be overridden with `--curl-connect-timeout-seconds` and
`--curl-max-time-seconds`.

If the canary fails with `Unknown instruction type`, the served validator build
is stale and missing the SoraFS capacity/order entries in
`iroha_core`'s instruction dispatch table even if the Torii route surface is
otherwise up.

On a freshly reset local bundle, the same signed canary tolerates the brief
startup window where the authoritative v2 snapshot is still at the genesis
frontier and no CommitQC exists yet,
submits the first post-genesis write, and then re-checks `/status` plus
`/v1/sumeragi/status` strictly after that write lands.

The rollout script requires `/v1/sumeragi/status` to advertise wire revision 4, a
frozen `height_context` with at least 4 validators and a consistent dual
quorum, an exact durable `last_commit_qc` after genesis, bounded `operator`
queues, and all canonical lane-evidence arrays. It rejects mismatched CommitQC
height/subject, insufficient signer count or power, out-of-range leaders,
impossible queue occupancy, and any legacy RBC/recovery status shape.
If it fails that check, stop the rollout, preserve the v2 WAL and incident
evidence, and rebuild the validator configs from the shared roster before
debugging ingress or MCP. It also verifies that the same direct node serves:

- `/v1/sccp/capabilities`
- `/v1/sccp/registry`
- `/v1/zk/proofs/count`
- `/v1/sumeragi/validator-sets`
- `/v1/nexus/public-lanes/0/{validators,stake}`
- `/v1/bridge/messages` preflight
- no retired server-side `/v1/contracts/deploy` route (`404 route_not_found`)
- `/v1/contracts/state`
- canonical `/v1/pipeline/transactions/status` with a typed
  `query_validation_failed` response when the hash is omitted
- no retired `/v1/transactions/status` alias (`404 route_not_found`)

The same gate must sample public ingress and all four direct validator roots
repeatedly. `/status.blocks` is the query-visible WSV committed height, not a
lazy Kura telemetry counter or a pre-apply CommitQC height, and must advance
with the signed canary while the fleet retains one exact build, configuration,
catalog, and committed-chain identity.

That config must be a normal `iroha` client TOML for a low-risk runtime-only
signer. Start from `taira-canary-client.example.toml`, not
`defaults/client.toml`: the generic repo client uses the zero chain id and the
development genesis `network_id`, so it is not valid for Taira. The canary
alias defaults to the dataspace-root form
`<label>@universal`; do not expand it to `@wonderland.universal` or
`@universal.universal`. When `--write-config` is omitted and the automatically
selected runtime path is missing, `check_mcp_rollout.sh` requires
`--onboarding-token-file /absolute/runtime/path/onboarding-token`, generates a
fresh keypair, onboards the account on public Taira, and writes that
runtime-only config before the signed ping. An explicit config path is never replaced,
including when it contains a stale or placeholder authority. The Torii
onboarding authority enrolls the account in the configured sponsor program;
onboarding does not accept fee or gas overrides. Bootstrap requires the onboarding endpoint to return a
`202 QUEUED` receipt and follows it through the canonical
`/v1/pipeline/transactions/status` route before using the signer; the faucet
helper follows the same canonical receipt path when it runs. With the default
Taira sponsor program configured, bootstrap skips faucet funding by default, so
the write canary proves the sponsored-fee path directly. Set
`ROLLOUT_CANARY_SKIP_FAUCET=0` only when intentionally validating an
unsponsored/faucet-funded network. The signed ping requests an exact fee quote
for `testuﾛ1PｵEmｷjMZZﾑﾙeｱﾁﾎﾅﾂﾊmECepdbﾎｳ2uWﾃｸﾊﾘvｵi2ｦP1Y18A/default` revision 1 by default. Keep the populated canary config
out of the repo and out of shell history where possible.

If the script fails with `route_unavailable`, treat that as a deployment or
topology failure, not an app-level validation issue: the public Torii ingress is
up, but it still cannot reach an authoritative peer for lane `0` / dataspace
`0`.
If it fails with `Failed to find asset` even after the automatic faucet
bootstrap path runs, treat that as a faucet-health or signer-selection issue:
the configured account either does not exist on Taira yet or the live faucet
could not fund it.

### Public write failure triage

When public reads succeed but writes fail or hang, classify the failure from
the queried public Torii node first before assuming a malformed request or a
full validator-set outage.

Before long public writes such as Soracloud releases or large SoraFS publishes:

- treat `https://taira.sora.org` as the current primary public Torii/API root
- confirm that `/status` counters advance and use `/v1/sumeragi/status` for
  detailed finality and queue health:
  - `curl -sS "${PUBLIC_TORII_ROOT}/status" | jq '{build_git_sha: .build.git_commit_sha, blocks, queue_size, peers, teu_dataspace_backlog}'`
  - `operator_get "${PUBLIC_TORII_ROOT}/v1/sumeragi/status" | jq '{protocol_version, node_fingerprint, build_fingerprint, config_fingerprint, height_context_id, height, view, phase: .phase.phase, leader, locked_prepare_qc, highest_prepare_qc, last_timeout_certificate, body_state: .body_state.state, pending_persistence_id, mode: .height_context.mode.mode, epoch: .height_context.epoch, validator_count: .height_context.validator_count, quorum: .height_context.quorum, last_committed_height, last_committed_subject, commit_qc_height: .last_commit_qc.certificate.round.height, commit_qc_signers: .last_commit_qc.signer_count, commit_qc_min_signers: .last_commit_qc.min_signers, commit_qc_signed_power: .last_commit_qc.signed_power, commit_qc_total_power: .last_commit_qc.total_power, view_change_install_total: .operator.view_change_install_total, busy_deferral_total: .operator.busy_deferral_total, tx_queue_depth: .operator.tx_queue.queued_transactions, tx_queue_capacity: .operator.tx_queue.capacity, saturated_by_count: .operator.tx_queue.saturated_by_count, saturated_by_age: .operator.tx_queue.saturated_by_age, oldest_queued_age_ms: .operator.tx_queue.oldest_queued_age_ms, lane_block_sessions: (.lane_block_sessions | length)}'`
- verify the signer you intend to use still exists on the current Taira chain
  and still has a positive fee-asset balance
- for Soracloud mutations specifically, also verify that the signer still
  holds `CanManageSoracloud` and `CanPublishSpaceDirectoryManifest` before
  starting a large upload
- after a Taira reset or redeploy, treat cached or previously faucet-funded
  signers as stale until those checks pass again

When a public write still fails, start with the same status samples above.

Interpret the common public failures as follows:

- `502` / `503` from `GET /v1/mcp`, `POST /v1/mcp`, or other public Torii routes:
  ingress or rollout health degradation. Treat this as deployment health first.
- `route_unavailable` from a live write:
  the public Torii ingress is up, but the write path still cannot reach an
  authoritative peer for the target lane. Capture the response headers
  `x-iroha-route-lane-id`, `x-iroha-route-dataspace-id`,
  `x-iroha-route-unavailable-reason`,
  `x-iroha-route-authoritative-total`,
  `x-iroha-route-authoritative-offline`, and
  `x-iroha-route-loop-prevention-drops`; they identify whether the failure is
  a missing authoritative binding, offline authoritative peers, or proxy-hop
  loop prevention.
- successful read/query fanout with non-zero
  `x-iroha-fanout-routes-failed`, `x-iroha-fanout-routes-unavailable`, or
  `x-iroha-fanout-routes-not-found`:
  the public read was recovered from another dataspace, but some authoritative
  routes are degraded. Capture all `x-iroha-fanout-*` headers with the status
  samples before deciding the request is fully healthy.
- `Transaction expired`:
  likely chain-health, consensus-latency, or queue-saturation trouble first.
  Report the current `blocks`, `queue_size`, `teu_dataspace_backlog`,
  v2 `height`, `view`, `phase`, `body_state`, `pending_persistence_id`,
  `last_committed_height`, exact CommitQC count/power, frozen mode/epoch/roster,
  `operator` queue occupancy/saturation, view-change installs, busy deferrals,
  canonical lane-session evidence, `protocol_version`, build/config/context
  fingerprints, `leader`, lock/highest-QC/last-TC references, and samples from
  every validator alongside the failure.
- `403 Forbidden` immediately after a reset or redeploy:
  likely signer-permission or signer-state drift first. Re-check that the
  signer still exists on-chain, still holds a fee asset balance, and still has
  the permissions required for the mutation.
- `GET /v1/pipeline/transactions/status?hash=...` returning `404 not_found` for a
  previously submitted hash:
  the queried public node currently has no visibility for that hash. Do not
  infer commit, reject, or network-wide disappearance from that result alone.

If the latest committed block timestamp and v2 `last_committed_height` stop
advancing, sample the compact v2 status from every labeled validator. Different
build/config/context fingerprints are a rollout failure. A
`pending_persistence_id` that does not clear, a body stuck before
`validated`/`applied`, or validators diverging across heights and views while
`operator.view_change_install_total`, busy deferrals, persistence blockage, or
bounded queue occupancy continue to rise is evidence that the queried finality
path is stalled. A single one-height reducer/commit gap is normal pipeline state
and is not sufficient evidence by itself. Unless you have validator-side
access, describe this as a public-node or public-finality-path observation
rather than proof that the full validator set is down.

A direct Vote rejected only because its execution commitment is not yet bound
is recoverable, not malformed. On a fixed build it remains fair-ingress-owned
until local proposal validation establishes the exact commitment. If such
votes disappear while completion age or service debt rises and proposal-body
recovery does not drain, treat the node as an unfixed completion-starvation
deployment and stop the rollout.

Do not clear volatile consensus state or use an adaptive recovery path. Preserve
the WAL, Kura, per-validator status samples, and logs as incident evidence. A
Sumeragi v2 Taira rollout must use one build, one shared config/context
fingerprint, and a fresh genesis/chain ID; stop the rollout if those invariants
or the bounded post-healing progress check fail.

## Governance mode

`config.toml` pins Taira to Sora parliament sortition governance for Nexus lanes:

- `nexus.governance.default_module = "parliament"`
- `nexus.governance.modules.parliament.module_type = "parliament_sortition_jit"`
- governance lane metadata binds lane 1 to `governance = "parliament"`
- top-level `[gov]` sets multibody committee/quorum parameters

This avoids fallback to legacy council-epoch approval mode during deployment.

## Fee config

Taira must declare the Nexus fee asset explicitly as the live XOR alias:

```toml
[nexus.fees]
fee_asset_id = "xor#universal"
fee_sink_account_id = "testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV"
base_fee = "0"
per_byte_fee = "0"
per_instruction_fee = "0.001"
per_gas_unit_fee = "0.00005"
sponsor_vault_custody_account_id = "testuﾛ1NｱｻｸYSafﾇｷヰc5ﾇﾄVxﾏ9jLZヱﾋzsKqurﾊﾘ9ｸ3eｴAｶD54TDT"
```

Without this Taira-specific block, the node uses the generic configured XOR
selector rather than Taira's on-chain `xor#universal` alias. This is a node
configuration default, not a transaction payer fallback: clients cannot repair
the mismatch with metadata, and quote/admission reject the transaction before a
public deploy or call can activate.

The live Taira manifest registers, funds, enrolls, and activates revision 1 of
`testuﾛ1PｵEmｷjMZZﾑﾙeｱﾁﾎﾅﾂﾊmECepdbﾎｳ2uWﾃｸﾊﾘvｵi2ｦP1Y18A/default`. That owner is the live
`[genesis].public_key` signer. The checked-in Kagami Taira profile deliberately
uses its Alice genesis signer and therefore owns
`testuﾛ1PﾉｳﾇmEｴWｵebHﾑ6ﾔﾙｲヰiwuCWErJ7uｽoPGｱﾔnjﾑKﾋTCW2PV/default` instead. Do not copy one profile's
program ID into the other. Funds are transferred into the configured custody
account while remaining isolated and accounted for by exact program ID.

For an already-running network, `taira_fee_sponsor_program` provisions one
owner-signed revision in the required `create -> stage -> enroll -> fund ->
activate` order. Pass canonical Norito JSON through `--revision-json` and
`--fee-payment-json`; the helper quotes the complete unsigned payload, replaces
only its fee intent with the response, and signs once. The signer loaded from
`--profile-config` must own the program—there is no implicit policy-account or
default-sponsor fallback.

Normal CLI submissions also quote before signing and require an explicit payer:

- authority paid: `iroha --fee-payer authority <command>`
- sponsored: `iroha --fee-payer sponsor --fee-program <canonical-I105>/<name> --fee-program-revision <nonzero-u64> <command>`

For contract or IVM execution, add the command's positive `--gas-limit`; the
CLI binds it inside `fee_payment`. Do not put `fee_sponsor`, `gas_asset_id`, or
`gas_limit` in transaction metadata.

## Development-only containerized validator deployment

This path is excluded from first-release Taira publication and admission. No
release OCI image is produced by `publish_taira_validator.yml`; use
`update_taira_testnet.yml` for a routine deployed-network update. The primary local wrapper is
`taira-validator-container.sh`, which uses plain `docker` and therefore works
on hosts that lack the Compose plugin. `docker-compose.validator.yml` remains
available as an optional convenience for environments that do have Compose.

1. Build or load an explicitly local development image and retain its local
   image ID for `TAIRA_IMAGE`; do not treat it as release authority.
   - `docker load < iroha3-<version>-linux-image.tar`
2. Create an owner-private absolute render root and render the validator config
   bundle from your user-local roster and secrets:
   - `TAIRA_VALIDATOR_RENDER_ROOT="$(mktemp -d /private/var/tmp/iroha-taira-validator-render.XXXXXX)"`
   - `chmod 0700 "${TAIRA_VALIDATOR_RENDER_ROOT}"`
   - `python3 scripts/render_taira_validator_bundle.py --roster configs/soranexus/taira/validator_roster.local.toml --secrets configs/soranexus/taira/validator_secrets.local.toml --output-dir "${TAIRA_VALIDATOR_RENDER_ROOT}/taira-validators"`
3. Install the rendered config and storage directories on the validator host:
   - `sudo install -d -m 0700 -o 1001 -g 1001 /etc/iroha/taira-validator`
   - `sudo install -d -o 1001 -g 1001 /var/lib/iroha/taira-validator-1`
   - `sudo cp -a "${TAIRA_VALIDATOR_RENDER_ROOT}/taira-validators/taira-validator-1/." /etc/iroha/taira-validator/`
   - install reviewed SoraFS admission envelopes, if any, under the rendered
     `/etc/iroha/taira-validator/sorafs_admission` directory
   - after installing all runtime inputs, run
     `sudo chown -R 1001:1001 /etc/iroha/taira-validator /var/lib/iroha/taira-validator-1`
4. Copy the sample env file and adjust the host-specific values:
   - `sudo cp configs/soranexus/taira/taira-validator-container.compose.env.example /etc/default/taira-validator-container.compose.env`
   - set at least:
     - `TAIRA_IMAGE=sha256:<local-image-id>`
     - `TAIRA_RUNTIME_PROFILE=localnet`
     - `TAIRA_CONFIG_BUNDLE_PATH=/etc/iroha/taira-validator`
     - `TAIRA_STORAGE_PATH=/var/lib/iroha/taira-validator-1`
     - `TAIRA_TORII_PORT=18080` unless your ingress expects another loopback port
5. Start the validator directly with the plain-`docker` wrapper:
   - `bash configs/soranexus/taira/taira-validator-container.sh --env-file /etc/default/taira-validator-container.compose.env up`
   - `bash configs/soranexus/taira/taira-validator-container.sh --env-file /etc/default/taira-validator-container.compose.env status`
   - `bash configs/soranexus/taira/taira-validator-container.sh --env-file /etc/default/taira-validator-container.compose.env logs`
   - to inspect the exact `docker run` invocation before starting:
     `bash configs/soranexus/taira/taira-validator-container.sh --env-file /etc/default/taira-validator-container.compose.env config`
6. If you prefer Docker Compose and the host actually has the plugin, the
   equivalent commands are:
   - `docker compose --env-file /etc/default/taira-validator-container.compose.env -f configs/soranexus/taira/docker-compose.validator.yml up -d`
   - `docker compose --env-file /etc/default/taira-validator-container.compose.env -f configs/soranexus/taira/docker-compose.validator.yml ps`
   - `docker compose --env-file /etc/default/taira-validator-container.compose.env -f configs/soranexus/taira/docker-compose.validator.yml logs --tail=200`
7. If you want systemd ownership, install the wrapper service:
   - `sudo cp configs/soranexus/taira/taira-validator-container.service /etc/systemd/system/`
   - if the repo checkout is not `/opt/iroha`, edit the script path in
     `ExecStart*=` before enabling the unit
   - `sudo systemctl daemon-reload`
   - `sudo systemctl enable --now taira-validator-container.service`
8. Prove the local MCP surface before any public cutover:
   - `bash configs/soranexus/taira/check_mcp_rollout.sh --skip-public --local-root http://127.0.0.1:18080 --skip-write-canary`
   - for a signed local write-path check:
     `bash configs/soranexus/taira/check_mcp_rollout.sh --skip-public --local-root http://127.0.0.1:18080 --write-config /run/secrets/taira-canary-client.toml --write-target local`

Optional container overrides:

- if you need a validator-specific `genesis.json`, uncomment the matching
  `IROHA_TAIRA_GENESIS` and volume lines in
  `docker-compose.validator.yml`, then set `TAIRA_GENESIS_PATH=...` in
  `/etc/default/taira-validator-container.compose.env`
- if the validator host should serve named SoraFS host bindings directly from
  the container, set `[sorafs.gateway.site_bindings].path =
  "/config/sorafs_sites.json"` in the rendered validator config, uncomment the
  matching volume line, then set `TAIRA_SORAFS_SITE_BINDINGS_PATH=...`

## Bare-metal validator deployment

Install the validator from the repo checkout so the live process cannot drift
away from the shipped MCP-enabled config:

1. Check out this repository on the validator host, for example at
   `/opt/iroha`.
2. Build a rollout bundle from the exact runtime revision you intend to ship:
   - from the sibling DPN API checkout, run
     `IROHA_DIR=/opt/iroha ops/taira/build-validator-bundle.sh`
   - the wrapper accepts either the policy-pinned clean commit or the one exact
     attested source patch, then writes
     `dist/taira-rollout/<bundle>/rollout.manifest.json`,
     `sha256sums.txt`, the archive, and its sibling signed
     `<bundle>.authority/` tuple
   - it freezes the canonical workspace source manifest before Cargo runs,
     builds the production validator without the evidence feature, then builds
     `taira_privacy_release_runner` separately and emits the paired Norito/JSON
     receipt, 48-block stage bundle, command manifest, and frozen expectations
     under `provenance/privacy-native/`
   - run the packaged `verify` command from
     [Native privacy release evidence](#native-privacy-release-evidence) before
     installation; this re-executes the native stages through the copied runner
     and rejects a changed validator, runner, Cargo lock, source identity,
     expectation ceiling, artifact order, digest, or JSON projection
   - the script runs `cargo test -p iroha_core queue::router::tests::smart_contract_deploy_rule --lib`
     and `cargo test -p iroha_core call_contract_syscall_preserves_root_and_nested_transfer_authorities_in_artifacts --lib`
     before packaging so SoraSwap universal contract deploy routing and nested
     AssetOps transfer authority are proven against the exact source checkout.
     Release profile rejects both `--skip-build` and
     `--skip-local-regressions`; those flags are debug-only.
   - the bundle now includes both `scripts/render_taira_validator_bundle.py`
     and `scripts/render_taira_edge_nginx_conf.py` so validator config and
     shared-edge nginx can be rendered from the same roster artifact
   - capture the emitted git revision, archive path, signed authority tuple,
     reviewed signer fingerprint, and authority-manifest SHA-256 in the rollout
     ticket; together they identify the exact candidate the later SoraSwap gate
     must approve
3. Create an owner-private absolute render root, render the per-validator
   config bundle from a user-local roster file, then copy the correct validator
   config onto the host, for example:
   - `TAIRA_VALIDATOR_RENDER_ROOT="$(mktemp -d /private/var/tmp/iroha-taira-validator-render.XXXXXX)"`
   - `chmod 0700 "${TAIRA_VALIDATOR_RENDER_ROOT}"`
   - `python3 scripts/render_taira_validator_bundle.py --roster configs/soranexus/taira/validator_roster.local.toml --secrets configs/soranexus/taira/validator_secrets.local.toml --output-dir "${TAIRA_VALIDATOR_RENDER_ROOT}/taira-validators"`
   - `validator_secrets.local.toml` must include every validator BLS private
     key, its dedicated Ed25519 `soranet_transport_public_key` and
     `soranet_transport_private_key`, its dedicated secp256k1
     `receipt_public_key` and `receipt_private_key`, and the shared
     `account_onboarding_*`, `torii_faucet_*`, and
     `streaming_identity_*`, `soracloud_runtime_signer_*`,
     `sorafs_council_public_keys`, and `sorafs_council_signature_threshold`
     fields because the checked-in template intentionally leaves those
     deployment values as fail-closed placeholders
   - `sudo install -d -m 0700 -o iroha -g iroha /etc/iroha/taira-validator`
   - `sudo install -d -m 0700 -o iroha -g iroha /var/lib/iroha/taira-validator-1`
   - `sudo cp -a "${TAIRA_VALIDATOR_RENDER_ROOT}/taira-validators/taira-validator-1/." /etc/iroha/taira-validator/`
   - preserve the generated `0600` modes; signer and governance paths already
     target this canonical install root and must not be rewritten
   - install reviewed SoraFS admission envelopes, if any, under
     `/etc/iroha/taira-validator/sorafs_admission`
   - after installing all runtime inputs, run
     `sudo chown -R iroha:iroha /etc/iroha/taira-validator /var/lib/iroha/taira-validator-1 /var/lib/iroha/taira-validator`
4. Install the newly built binaries plus the sample systemd unit from
   `configs/soranexus/taira/taira-irohad.service`:
   - install native Inrou prerequisites before enabling the unit, for example
     on Debian/Ubuntu:
     `sudo apt-get update && sudo apt-get install -y qemu-system-x86 qemu-system-arm qemu-utils e2fsprogs iproute2 iptables`
   - verify the host will advertise real Inrou capacity:
     `bash configs/soranexus/taira/check_inrou_host_prereqs.sh`
   - `sudo install -m 0755 dist/taira-rollout/<bundle>/bin/iroha3d /usr/local/bin/iroha3d`
   - `sudo install -m 0755 dist/taira-rollout/<bundle>/bin/iroha /usr/local/bin/iroha`
   - `sudo cp configs/soranexus/taira/taira-irohad.service /etc/systemd/system/`
   - copy `configs/soranexus/taira/taira-irohad.env.example` to
     `/etc/default/taira-irohad`; the supplied unit deliberately preflights the
     complete canonical `/etc/iroha/taira-validator` bundle, so do not override
     only `IROHA_TAIRA_CONFIG` to another install root
   - if a deployment intentionally uses another renderer `--install-root`,
     update the unit's config, signer, manifest, and SoraFS admission preflight
     paths together before enabling it
   - if your repo checkout or binary path differs from `/opt/iroha` and
     `/usr/local/bin/iroha3d`, adjust `WorkingDirectory=` and set
     `IROHA_TAIRA_IROHAD_BIN=` in `/etc/default/taira-irohad` before enabling
     the unit
5. Reload systemd and restart the validator:
   - `sudo systemctl daemon-reload`
   - `sudo systemctl enable --now taira-irohad.service`
   - `sudo systemctl restart taira-irohad.service`
6. Capture the resolved config in the rollout ticket:
   - `sudo journalctl -u taira-irohad.service -n 200 --no-pager`
   - `cd /opt/iroha && sudo -u iroha env KURA_STORE_DIR=/var/lib/iroha/taira-validator-1 SNAPSHOT_STORE_DIR=/var/lib/iroha/taira-validator-1/snapshot /usr/local/bin/iroha3d --sora --config /etc/iroha/taira-validator/config.toml --genesis-manifest-json /opt/iroha/configs/soranexus/taira/genesis.json --trace-config | tee /tmp/taira-trace-config.txt`
   - verify `/tmp/taira-trace-config.txt` includes `nexus.fees.fee_asset_id = "xor#universal"`
7. Prove the validator's loopback Torii endpoint exposes MCP and the expected
   direct-ingress routes before any public cutover:
   - `bash configs/soranexus/taira/check_mcp_rollout.sh --skip-public --local-root http://127.0.0.1:18080 --skip-write-canary`
   - for a full local write-path check, use a runtime-only canary signer:
     `bash configs/soranexus/taira/check_mcp_rollout.sh --skip-public --local-root http://127.0.0.1:18080 --write-config /run/secrets/taira-canary-client.toml --write-target local`
8. After the public node is back, prove the direct hostname is healthy before
   any convenience host or client cutover:
   - `bash configs/soranexus/taira/check_mcp_rollout.sh --public-root "${PUBLIC_TORII_ROOT}" "${TAIRA_VALIDATOR_ARGS[@]}" --require-all-validators --write-config /run/secrets/taira-canary-client.toml --expected-git-sha "${EXPECTED_TAIRA_GIT_SHA}"`
   - if contract deploy/view health still fails after the route checks pass,
     redeploy SoraSwap with the updated `../soraswap` `deploy-testnet` flow
     before blaming the frontend
9. Before declaring public Codex/Torii rollout complete, require the SoraSwap
   gate to pass behind the same runtime candidate:
   - probe-only:
     `bash configs/soranexus/taira/verify_soraswap_rollout.sh --public-root "${PUBLIC_TORII_ROOT}" "${TAIRA_VALIDATOR_ARGS[@]}" --write-config /run/secrets/taira-canary-client.toml --expected-git-sha "${EXPECTED_TAIRA_GIT_SHA}" --soraswap-client-config /path/to/soraswap/config/testnet/taira.client.toml`
   - full gate:
     `bash configs/soranexus/taira/verify_soraswap_rollout.sh --public-root "${PUBLIC_TORII_ROOT}" "${TAIRA_VALIDATOR_ARGS[@]}" --write-config /run/secrets/taira-canary-client.toml --expected-git-sha "${EXPECTED_TAIRA_GIT_SHA}" --soraswap-client-config /path/to/soraswap/config/testnet/taira.client.toml --run-release-checklist --allow-testnet-mutations`
   - the wrapper runs the focused `iroha_core` SoraSwap deploy-route router
     regression and three-hop nested transfer canary, `check_mcp_rollout.sh`,
     `check_sorafs_rollout.sh`, the trader app-api CID probe when a bundle is
     present, `make testnet-nested-call-probe`, then the exact `deploy-testnet`
     / signed `smoke-testnet` / `release-checklist` sequence when those deeper
     flags are enabled
   - run it from a full `../iroha` source checkout for the default local
     regressions; use `--skip-local-regressions` only after that exact runtime
     bundle was already validated from source
   - the wrapper refuses an invocation where every verification phase is
     skipped, because that would otherwise report a false rollout pass
   - the script auto-discovers `${REPO_ROOT}/../soraswap` when the sibling repo
     exists, but `--soraswap-root` is available for non-default layouts

## Explorer integration (sibling repo)

From `../iroha2-block-explorer-web`:

1. Copy this file to runtime config:
   - `cp ../iroha/configs/soranexus/taira/explorer.runtime-config.json public/config.json`
   - update `toriiBaseUrl` if you want the explorer to query a different
     public node than the checked-in example
2. Build and deploy static assets:
   - `corepack enable && pnpm i && pnpm build`
3. Render and install the nginx snippet from the same validator roster you use
   for the validator configs:
   - preferred edge-host helper for the Solswap indexer binding:
     `bash configs/soranexus/taira/install_taira_edge_nginx_conf.sh --roster configs/soranexus/taira/validator_roster.local.toml --soracloud-alias-route solswap-indexer.sora=127.0.0.1:8788 --require-alias solswap-indexer.sora --install --reload`
   - the helper renders the same config as the manual command below, refuses
     stale backup `.conf` files in the nginx include directory by default,
     validates the rendered snippet with a temporary nginx config, runs live
     `nginx -t` after install, rolls the target back if that live validation
     fails, and reloads nginx only when `--reload` is explicit
   - `python3 scripts/render_taira_edge_nginx_conf.py --roster configs/soranexus/taira/validator_roster.local.toml --output dist/taira-edge/taira.sora.org.conf`
   - `sudo cp dist/taira-edge/taira.sora.org.conf /etc/nginx/conf.d/taira.conf`
   - on the shared macOS/Homebrew host, install the rendered file as
     `/opt/homebrew/etc/nginx/servers/taira.sora.org.conf` instead
   - set each validator entry's `edge_torii_upstream` in the roster to the
     real Torii listener the edge should proxy to, for example the current
     shared-host `127.0.0.1:29080..29083` layout rather than the old
     `127.0.0.1:18080..18083` default
  - add `[[soracloud_alias_routes]]` entries to the same local roster for
    every dedicated Soracloud runtime that is served from the shared edge
    instead of Torii's generic public alias path. For the Solswap indexer on
    the shared host, the local roster entry is currently:
    `alias = "solswap-indexer.sora"` with
    `edge_upstream = "127.0.0.1:8788"`. This renders the exact Mon host
    `solswap-indexer.sora.mon.taira.sora.net` and both `/soradns/` debug
    fallbacks to that service upstream while leaving unknown aliases on the
    generic Mon fallback. You can also pass
    `--soracloud-alias-route solswap-indexer.sora=127.0.0.1:8788` directly to
    the renderer for one-off overrides.
  - keep the shared `taira_public_edge_upstream` pinned to one explicitly
    selected canonical validator. A live but lagging validator still returns
    HTTP 200, so passive nginx failover cannot safely distinguish it from the
    current state view. Move the public pin only after direct height and state
    parity checks pass. The validator-specific hostnames remain available for
    consensus diagnostics.
  - keep the dedicated `location = /v1/mcp` blocks pinned to the same Torii
    upstream as `/v1/connect/session`, management-token session status at
    `/v1/connect/status`, operator-signed aggregate status at
    `/v1/connect/status/aggregate`, and `/v1/connect/ws`. MCP exposes Connect
    session creation and management tools, and Connect tokens/state are
    process-local at creation time.
  - keep the `proxy_next_upstream ... non_idempotent` retry policy on shared
    public locations only. Do not add upstream failover to the pinned
    Connect/MCP locations until Connect session state is shared across
    validators.
  - keep the shared convenience host on the same canonical
    `taira_public_edge_upstream` for the public SoraFS and app-api surface as
    well. The checked-in nginx example keeps these paths symmetric with the
    rest of the public edge:
    - `/v1/app-api/`
    - `/v1/sorafs/storage/`
    - `/v1/sorafs/pin/`
    - `/v1/sorafs/cid/`
    - `/sorafs/cid/`
  - if CID hydration is still inconsistent after the runtime rollout, treat
    that as a provider-capacity/bootstrap problem and inspect
    `/v1/sorafs/capacity/state`; do not add unchecked multi-validator failover
    to the canonical public origin.
    - `*.sorafs.taira.sora.org`
    Keep CID reads routed to admitted providers with the same finalized
    assignment context; never reintroduce a public storage-upload route to
    compensate for inconsistent provider hydration.
   - after every local reset, confirm the served `dist/taira-localnet/peer*.toml`
     copies still contain `max_content_len = 1073741824`; the local bootstrap
     script patches them from `configs/soranexus/taira/config.toml`, but a
     stale bundle can still bring the old default back.
   - confirm those peer configs also retain the Taira `[sumeragi.block]`
     `max_transactions = 96`, `max_payload_bytes = 16777216`, and
     `proposal_queue_scan_multiplier = 4` bounds, plus the
     `[sumeragi.queues]` canonical outer-ingress wire-byte baseline
     `authenticated_non_validator_sources = 2`, `body_bytes = 242221056`, and
     `body_source_bytes = 34603008`, before running public write canaries or
     scenario sweeps. The four-validator baseline isolates every validator,
     both authenticated non-validator source lanes, and anonymous delivery;
     `render_taira_validator_bundle.py` raises `body_bytes` to at least
     `(validator_count + authenticated_non_validator_sources + 1) *
     body_source_bytes` for larger legal rosters.
     The revision-4 protocol caps the complete canonical body at 16 MiB. This
     admits one maximum 10 MiB transaction carrying one 9 MiB privacy action
     while retaining 6 MiB for canonical block framing and context attachments;
     smaller transactions can still share the block. The per-source queue
     rounds the exact ordinary/completion/timeout minimum up to 33 MiB. Keep
     `[network] max_frame_bytes_tx_gossip = 11534336` (11 MiB plaintext),
     `[network] max_frame_bytes_block_sync = 23068672` (22 MiB plaintext) and
     `max_frame_bytes = 23068700` (the same ceiling plus 28 AEAD bytes) with
     those values.
     Fast-finality caps are retired in v2.
   - assert that an attempted direct SoraFS payload POST returns `404` and leaves
     both the local manifest count and storage-byte reservation unchanged.
   - keep the dedicated `location = /v1/connect/ws` blocks intact; they forward
     the required websocket `Upgrade` / `Connection: upgrade` headers for
     Iroha Connect on `taira.sora.org`.
   - do not fold `/v1/connect/ws` into the generic `location /` or
     `location ^~ /v1/` proxy rules; it must stay an exact-match websocket
     location with `proxy_http_version 1.1`.
   - ensure `taira.sora.org`, `taira-explorer.sora.org`, `mon.taira.sora.net`,
     every published `taira-validator-{1,2,3,4}.sora.org` hostname, and the
     required `*.sorafs.taira.sora.org` and `*.mon.taira.sora.net` records
     resolve to the shared edge host from `dns_records.json` before relying on
     this nginx configuration.
   - add wildcard edge routing for `*.sorafs.taira.sora.org` and preserve the
     incoming host header when proxying to Torii; the checked-in nginx example
     now includes that wildcard `server_name`.
   - keep Mon gateway routing generic with the apex `mon.taira.sora.net`
     server block plus the regex alias server block for
     `<alias>.mon.taira.sora.net`, and add dedicated service bindings through
     local-roster `[[soracloud_alias_routes]]` entries or repeatable
     `--soracloud-alias-route <alias>=<host>:<port>` flags when an alias must
     terminate at a runtime process instead of the generic Torii alias path.
     Do not add per-service path rewrites such as
     `/solswap-indexer/...`.
   - do not leave backup `.conf` files under the nginx `servers/` include
     directory. Homebrew nginx deployments often include the whole directory,
     so backup configs can create duplicate `server_name` entries and shadow
     the intended Mon gateway block.
4. Issue/refresh TLS certificates for the public hosts, direct validator names,
   CID-origin wildcard, and Mon gateway exact hosts:
   - `taira.sora.org`
   - `taira-explorer.sora.org`
   - `taira-validator-1.sora.org`
   - `taira-validator-2.sora.org`
   - `taira-validator-3.sora.org`
   - `taira-validator-4.sora.org`
   - `*.sorafs.taira.sora.org`
   - `mon.taira.sora.net`
   - exact Mon hosts such as `solswap-indexer.sora.mon.taira.sora.net`
   - the convenience, explorer, and direct validator names can share one SAN
     certificate stored under `.../live/taira.sora.org/` if your ACME client
     keeps those names in one lineage.
   - the wildcard requires DNS-01 validation; `certbot --nginx` alone is not
     enough for the `*.sorafs.taira.sora.org` SAN.
   - Mon gateway aliases require exact bind-time certificates. A wildcard cert
     for `*.mon.taira.sora.net` does not cover multi-label aliases such as
     `solswap-indexer.sora.mon.taira.sora.net`.
   - if your ACME client stores all SANs under one lineage, nginx can keep
     pointing at a single certificate bundle for all names served from this
     edge.
   - before DNS propagates or before the SAN cert is refreshed, you can still
     validate local SNI routing on the edge host with `curl --resolve` plus
     `-k`, for example:
     `operator_get https://taira-validator-1.sora.org/v1/sumeragi/status --insecure --resolve taira-validator-1.sora.org:443:127.0.0.1 | jq '.height, .last_committed_height, .last_commit_qc.certificate.round.height'`
     `curl -sk --resolve taira-validator-1.sora.org:443:127.0.0.1 https://taira-validator-1.sora.org/status | jq '.blocks'`
   - if a client network intercepts or blocks `sora.net`, HTTP may be replaced
     before nginx and HTTPS may reset during the TLS ClientHello. This is stale
     reputation filtering from `sora.net` prior ownership, not evidence that
     current SORA content is pornographic. Confirm from the edge host or an
     unfiltered external network before treating that as a Soracloud runtime
     failure, and treat the durable fix as ISP/filter-vendor delisting.
5. Validate and reload nginx:
   - `sudo nginx -t && sudo systemctl reload nginx`
   - on the shared macOS/Homebrew host, use `nginx -t && nginx -s reload`
6. Run the MCP rollout smoke from any host that can see the validator loopback
   and the public endpoint:
   - `bash configs/soranexus/taira/check_mcp_rollout.sh --public-root "${PUBLIC_TORII_ROOT}" "${TAIRA_VALIDATOR_ARGS[@]}" --require-all-validators --expected-git-sha "${EXPECTED_TAIRA_GIT_SHA}"`
   - when you are validating edge-local SNI before public DNS or TLS is fully
     live, pin the public host to the edge IP explicitly:
     `bash configs/soranexus/taira/check_mcp_rollout.sh --public-root https://taira.sora.org "${TAIRA_VALIDATOR_ARGS[@]}" --require-all-validators --resolve-host taira.sora.org:443:127.0.0.1 --expected-git-sha "${EXPECTED_TAIRA_GIT_SHA}"`
   - the public check auto-bootstraps a runtime-only canary config when
    `--write-config` is omitted and
    `--onboarding-token-file /absolute/runtime/path/onboarding-token` names the
    exact owner-private route credential, preferring `/run/secrets` only when
    that directory is writable and otherwise using the local temp directory; when
    the default Taira sponsor program is configured, bootstrap skips faucet and
    signs its exact quoted intent unless you set `ROLLOUT_CANARY_SKIP_FAUCET=0`
7. Verify that SNI now serves the correct cert for each host and that MCP,
   Connect, and CID-host routing still work through the public edge:
   - `curl -vI https://taira.sora.org`
   - `curl -vI https://taira-explorer.sora.org`
   - `curl -vI "${PUBLIC_TORII_ROOT}/status"`
   - `echo | openssl s_client -connect taira-explorer.sora.org:443 -servername taira-explorer.sora.org 2>/dev/null | openssl x509 -noout -subject -issuer -ext subjectAltName`
   - `echo | openssl s_client -connect taira.sora.org:443 -servername example.sorafs.taira.sora.org 2>/dev/null | openssl x509 -noout -subject -issuer -ext subjectAltName`
   - verify MCP over the direct node host:
     `curl -sS "${PUBLIC_TORII_ROOT}/v1/mcp" | jq .`
   - verify curated `iroha.*` exposure:
     `curl -sS "${PUBLIC_TORII_ROOT}/v1/mcp" -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}' | jq .`
   - verify native counters and detailed Sumeragi health before trusting public
     writes:
     `curl -sS "${PUBLIC_TORII_ROOT}/status" | jq '{build_git_sha: .build.git_commit_sha, blocks, queue_size, peers, teu_dataspace_backlog}'`
     `operator_get "${PUBLIC_TORII_ROOT}/v1/sumeragi/status" | jq '{protocol_version, node_fingerprint, build_fingerprint, config_fingerprint, height_context_id, height, view, phase: .phase.phase, leader, locked_prepare_qc, highest_prepare_qc, last_timeout_certificate, body_state: .body_state.state, pending_persistence_id, mode: .height_context.mode.mode, epoch: .height_context.epoch, validator_count: .height_context.validator_count, quorum: .height_context.quorum, last_committed_height, last_committed_subject, commit_qc_height: .last_commit_qc.certificate.round.height, commit_qc_signers: .last_commit_qc.signer_count, commit_qc_min_signers: .last_commit_qc.min_signers, commit_qc_signed_power: .last_commit_qc.signed_power, commit_qc_total_power: .last_commit_qc.total_power, tx_queue_depth: .operator.tx_queue.queued_transactions, tx_queue_capacity: .operator.tx_queue.capacity, saturated_by_count: .operator.tx_queue.saturated_by_count, saturated_by_age: .operator.tx_queue.saturated_by_age, view_change_install_total: .operator.view_change_install_total, busy_deferral_total: .operator.busy_deferral_total}'`
   - remember that `/status.peers` is the queried node's current remote-peer
     count, not the validator-set size; use
     `/v1/sumeragi/status` `height_context.validator_count` and
     `last_commit_qc.validator_count`, or
     `/v1/sumeragi/validator-sets` for validator-set visibility.
   - create a Connect session through the proxy and ask explicitly for JSON;
     derive `sid` as BLAKE2b-256 over `iroha-connect|sid|`, the raw 32-byte
     exact `NetworkId`, the raw 32-byte X25519 app key, and the fresh raw
     16-byte nonce, in that order, then encode all binary fields as canonical
     unpadded base64url:
     `curl -sS -X POST "${PUBLIC_TORII_ROOT}/v1/connect/session" -H 'content-type: application/json' -H 'accept: application/json' -d '{"sid":"<derived-32-byte-base64url-sid>","network_id":"<canonical-hash-network-id>","app_pk":"<32-byte-base64url-x25519-app-key>","nonce":"<16-byte-base64url-random-nonce>"}'`
   - verify Connect websocket upgrades on both public hostnames with the
     returned `sid` and app token:
     `curl --http1.1 -i -N -H 'Connection: Upgrade' -H 'Upgrade: websocket' -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: dGVzdGtleTEyMzQ1Njc4OTA=' -H 'Sec-WebSocket-Protocol: iroha-connect.token.v1.<token_app>' "${PUBLIC_TORII_ROOT}/v1/connect/ws?sid=<sid>&role=app"`
     `curl --http1.1 -i -N -H 'Connection: Upgrade' -H 'Upgrade: websocket' -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: dGVzdGtleTEyMzQ1Njc4OTA=' -H 'Sec-WebSocket-Protocol: iroha-connect.token.v1.<token_app>' 'https://taira-explorer.sora.org/v1/connect/ws?sid=<sid>&role=app'`
     These curl commands only probe the HTTP upgrade. A functional client must
     send the one-shot app `Open` at sequence 1 with the same app key and exact
     `NetworkId`, accept only the wallet's one-shot `Approve` at sequence 1 after
     verifying its account signature and relay-token binding, and start
     contiguous encrypted traffic at sequence 2.
   - verify CID-host origin isolation with a known site CID:
     `curl -vkI "https://<cid>.sorafs.taira.sora.org/"`
     `curl -vkI "https://taira.sora.org/sorafs/cid/<cid>/swap/ton/usdt" -H 'accept: text/html'`
   - browser-style navigations should `308` to
     `https://<cid>.sorafs.taira.sora.org/...`, while asset/tooling requests can
     still stay on `/sorafs/cid/<cid>/...`.
   - if those websocket probes now return a Torii-generated app error
     (`400/401/...`) instead of a proxy-layer `404` / missing-upgrade failure,
     the reverse-proxy websocket hop is working and any remaining error is in
     Connect session or token handling rather than nginx.

The Explorer runtime config should target an explicit public node URL. The
checked-in example now uses `https://taira.sora.org`, while deployments that
want a direct-validator Explorer path can still override it at deploy time.

## Local Kaigi bootstrap

The served local Taira testnet on this machine does not expose a working public
lane write path after a fresh reset yet, so Kaigi relay metadata must be seeded
into the localnet's signed genesis overlay rather than submitted live through
Torii. Without that overlay, `/v1/kaigi/relays` will stay empty.

For the local `dist/taira-localnet` deployment, use:

1. Build the helper used to re-sign the localnet genesis overlay:
   - `cargo build -p iroha_kagami --example taira_kaigi_localnet --release`
2. Run the local bootstrap:
   - `bash configs/soranexus/taira/bootstrap_kaigi_localnet.sh`
   - the script reads the owner-held `genesis.private_key` emitted by
     `kagami localnet`; override its path with
     `IROHA_TAIRA_GENESIS_PRIVATE_KEY_FILE` when custody lives elsewhere.
     Raw private-key environment variables and command-line values are not
     accepted.
   - if you built the helper in a non-default target dir, point the bootstrap
     at it explicitly, for example:
     `IROHA_TAIRA_KAIGI_HELPER_BIN=/tmp/iroha_taira_kaigi_helper/debug/examples/taira_kaigi_localnet bash configs/soranexus/taira/bootstrap_kaigi_localnet.sh`
   - if `configs/soranexus/taira/validator_secrets.local.toml` is not present,
     provide `IROHA_TAIRA_AUTHORITY` and `IROHA_TAIRA_AUTHORITY_PRIVATE_KEY`
     (or point `IROHA_TAIRA_SECRETS_FILE` at a populated secrets file) so the
     bootstrap can inject the shared local onboarding signer, which it also
     reuses as the served local faucet signer, into the localnet configs
   - when setting `DPN_SPONSOR_ACCOUNT_ID`, use the same account as the local
     genesis signer unless the overlay already grants that signer exact
     fee-program lifecycle delegation. The bootstrap creates, funds, and
     activates `{DPN_SPONSOR_ACCOUNT_ID}/default`; ownership is enforced and a
     different unfederated account fails closed.
3. Verify the relay endpoints:
   - the bootstrap prints list and health responses through the maintained
     Python SDK using `peer0.toml` as a runtime-only operator key source and
     `client.toml` as the exact `NetworkId` source;
   - for another deployment, construct the Python or JavaScript client with an
     immutable runtime `OperatorSigningContext` and call the typed Kaigi list
     and health helpers. Unsigned curl, API tokens, redirected requests, and
     precomputed operator headers now fail closed;
   - do not place a validator/operator private key in browser Explorer config.
     The Explorer Kaigi page requires a separately deployed server-side signed
     diagnostic adapter before it can consume these operator-only snapshots;
     otherwise a `401` is the expected result.

The script is intentionally localnet-specific:

- it reuses the first three validator accounts already present in
  `dist/taira-localnet/peer{0,1,2}.toml`, so no extra linked-domain account
  registration is required;
- it derives the local client account from `dist/taira-localnet/client.toml`,
  signs a fresh `genesis.signed.nrt` overlay from `genesis.json`, and seeds the
  `nexus` domain metadata keys `kaigi_relay__*` and
  `kaigi_relay_feedback__*` so Torii's Kaigi relay endpoints have data to
  serve immediately after restart; and
- it skips `cargo test --example ...` harness binaries during helper
  auto-detection, so the bootstrap only reuses executables that actually
  expose the `--genesis` overlay CLI; and
- after any fresh local Taira reset, rerun this script if you want the Kaigi
  explorer page to reflect live relay data again.

The health snapshot's `healthy_total` will reflect the seeded relay feedback,
but `registrations_total` can remain `0` because that counter comes from live
telemetry rather than the seeded metadata overlay. The explorer overview still
shows the correct relay count because it floors the overview total to the
actual relay list length.
