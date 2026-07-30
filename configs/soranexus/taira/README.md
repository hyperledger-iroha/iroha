# Sora Taira public NPoS bootstrap

Taira is the Sora Nexus public testnet. This directory
contains the repo-shipped bootstrap bundle for a public, stake-elected NPoS
deployment.

## Network identity

- Public Sumeragi-v2 chain ID: `fc56984b-2be7-431d-840e-21514d1883f0`
- Archived pre-v2 chain ID: `809574f5-fee7-5e69-bfcf-52451e42d50f`
- Address chain discriminant: `369` (this is what drives canonical I105 literals such as `testu...`)
- Consensus protocol: Sumeragi v2 state machine, wire revision 3 only (`wire_protocol_version = 3`)
- Timing profile: authoritative 4,000 ms block cadence and one absolute 40,000 ms view-zero round deadline
- Candidate bounds: 96 transactions, 21 MiB canonical body, and a four-times bounded queue scan
- Role/mode boundary: each validator config says `role = "validator"`; NPoS mode and DA/chunk
  geometry come from signed genesis, not a mutable local mode or RBC selector

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
- Kura 75%, WSV snapshots 20%, SoraFS 0%, SoraNet spool 2.5%, and SoraVPN
  spool 2.5%

SoraFS storage is disabled on this profile. Do not remove the aggregate budget
or reassign its zero share without rerunning the free-space and fsync
preflight; near-full shared-host storage can turn restart durability barriers
into multi-second stalls.

## Included artifacts

- `config.toml`: baseline validator config for peer 1 and the shared template
  source for rendered per-validator configs. The checked-in file is template
  only and intentionally does not carry runtime-only private keys.
- `validator_roster.example.toml`: copy-me roster template for all validator
  public addresses, public keys, and PoPs. Keep the populated file user-local.
- `validator_secrets.example.toml`: copy-me runtime template for per-validator
  private keys, shared onboarding/faucet authority and streaming identity key
  material, plus the public SoraFS admission-council roots and quorum. Keep the
  populated file user-local.
- `genesis.json`: NPoS genesis with DA enabled.
- `dns_records.json`: DNS targets for the convenience host, explorer host, and
  direct per-validator Torii hostnames.
- `explorer.runtime-config.json`: runtime config example for the Explorer
  frontend; point it at the explicit public Torii base URL you want the UI to
  query.
- `sorafs_sites.json`: optional host-to-manifest bindings for Torii-served static sites. Keep `taira.sora.org` out of this file. Enable it only through the rendered validator config's `[sorafs.gateway.site_bindings]` table; Torii reads, validates, and caches the document once at startup.
- `taira-irohad.service`: sample systemd unit that starts the validator from
  the shipped Taira config and genesis.
- `taira-irohad.env.example`: sample `/etc/default/taira-irohad` overrides for
  pointing the systemd unit at a rendered validator config.
- `docker-compose.validator.yml`: sample containerized validator deployment
  that mounts one rendered validator config plus persistent `/storage`.
- `taira-validator-container.compose.env.example`: sample compose env file for
  a single validator host using the published Taira image.
- `taira-validator-container.sh`: plain-`docker` wrapper for hosts that do not
  have the Docker Compose plugin installed.
- `taira-validator-container.service`: sample systemd wrapper that keeps the
  validator container under service management without requiring Docker Compose.
- `scripts/render_taira_localnet_container_bundle.py`: rewrites a fresh
  `kagami localnet` bundle into four container-ready configs/env files with
  canonical `addr:...#CRC16` literals for shared-bridge Docker validation.
- `taira-canary-client.example.toml`: runtime-only example signer config for
  the signed rollout canary.
- `build_taira_rollout_bundle.sh`: packages the exact checked-out `irohad` /
  `iroha` / `sorafs_manifest_builder` / `sorafs_tx_stdin_builder` build plus the
  checked-in Taira config bundle into one timestamped rollout artifact. It
  builds `irohad` with the exact production
  `embedded-soracloud-runtime,zk-stark` features, separately builds the native
  privacy evidence runner with `privacy-release-evidence`, produces and
  re-verifies native evidence only after the validator build, runs the focused
  SoraSwap regressions, and records the frozen workspace-source identity plus
  release checks in `rollout.manifest.json`.
- `scripts/render_taira_edge_nginx_conf.py`: renders the shared-edge nginx
  config directly from the same validator roster used for per-validator
  `config.toml` generation so public Torii ingress cannot drift onto stale
  loopback ports.
- `scripts/deploy_taira_v21_reset.py`: performs the authenticated four-validator
  fresh-reset cutover and requires a fresh lowercase 64-hex
  `--restart-generation`. It emits identity-scoped terminal-unhealthy paths
  for all four supervisors and fails immediately if any current-generation
  marker appears during initial health, consensus advancement, or the child
  restart proof.
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
  checks used by the Taira Codex rollout, with wire-revision-3 reducer health read
  from `/v1/sumeragi/status` and an optional signed write canary for final
  public cutover. Every invocation must pass `--offline-asset-definition-id`
  with the operator-provisioned canonical ID for the registered scale-2
  `ds#boi.is` asset and `--offline-expected-identity` with an absolute path to
  the external, operator-reviewed JSON identity. The identity seals the exact
  capability, ABI, asset ID/scale, authenticated artifact set, and every field
  of all five governed verifier identities. It must remain outside the source
  repository. The script never falls back to the faucet/gas asset, a checked-in
  digest, or a validator-selected release.
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

Every Taira release bundle and Taira validator image must carry native
end-to-end evidence for the exact 12 privacy protocols. This evidence is a
post-build release gate, not a source-schema or pre-bundle report:

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
- the frozen expectations enforce a peak-RSS ceiling and a generous elapsed
  ceiling for every exact-12 maximum-shape row. The stage bundle records case
  descriptors, failure classes, and the canonical valid proof bytes needed for
  independent audit and reverification. It contains no witnesses or private
  inputs. Every proof is bounded by its exact protocol ceiling and the 8 MiB
  Taira consensus cap, and its recorded SHA-256 is recomputed from those bytes.

The validator-image workflow does not send the mutable checkout as the Docker
context. It freezes the raw-byte-sorted source path list, creates a
deterministic `iroha-workspace-source-seal-v1` archive containing one explicit
record for every regular file, directory, symlink, or deletion in that closure,
and constructs a unique read-only context containing only the Dockerfile,
source-seal verifier, archive, path list, and checksum controls. The verifier
rejects path traversal, `.git`, hard links, special files, escaping symlinks,
duplicates, extras, missing members, size/count overflow, mutated controls, and
non-empty or symlink extraction destinations before Cargo runs. BuildKit
recomputes the archive checksum and the canonical workspace manifest from the
detached extraction before Cargo and again after evidence generation. The
sealed context also contains the checksum-bound image smoke script. Taira uses
only the reviewed, digest-pinned preprovisioned builder and runtime bases and
fails if their required tools are absent; it does not run mutable package
installation during the release build.

Publication builds the final `BINARIES=irohad kagami` image once, loads and tags
that single image locally, and uses its packaged `kagami` to generate the
four-peer smoke bundle exercised by its packaged `irohad`. It then pushes those
same local tags with the sealed smoke script. Dispatch strings enter shell steps
only through environment variables and invalid tag components are rejected; the
immutable tag always carries the full workspace-source digest. Before any push,
the workflow creates, signs, verifies, and uploads the portable exact-12 release
authority. Each pushed and registry-reported manifest digest must equal the
Buildx digest that authority signed. There is no separate smoke build and no
second publish build with a different binary set.

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
  --cargo-lock "${bundle}/provenance/Cargo.lock" \
  --validator-binary "${bundle}/bin/irohad" \
  --command-manifest-norito "${bundle}/provenance/privacy-native/command-manifest-v1.norito" \
  --command-manifest-json "${bundle}/provenance/privacy-native/command-manifest-v1.json" \
  --stage-artifacts-norito "${bundle}/provenance/privacy-native/stage-artifacts-v1.norito" \
  --stage-artifacts-json "${bundle}/provenance/privacy-native/stage-artifacts-v1.json" \
  --receipt-norito "${bundle}/provenance/privacy-native/receipt-v1.norito" \
  --receipt-json "${bundle}/provenance/privacy-native/receipt-v1.json"
```

For a container image, the equivalent runner is
`/usr/local/bin/taira_privacy_release_runner`; use the same filenames under
`/opt/iroha/provenance/privacy-native/` and
`/opt/iroha/provenance/Cargo.lock`. Image construction already runs this
bundled-path verification, but operators should repeat it when admitting an
image into the rollout registry.

A JSON-only `privacy-release.json`, the retired
`taira_privacy_prebundle_gate` output, a test log, or a report generated before
the final validator binary exists is not native privacy release evidence and
must not be attached to a Taira rollout ticket as though it were.

## Signed release authority

A release-profile bundle or validator image is not a Taira release candidate
until its portable Ed25519 authority tuple passes. Production builders require
five paths/pins provisioned outside the checkout:

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
No private key path or signing seed is accepted. Bare-metal builds emit
`<bundle>.authority/`; image publication uploads
`taira-validator-release-authority-<run-id>`. Each contains
`release_manifest.json`, its raw `.sig` and `.pub`, and an `artifacts/`
directory holding the canonical exact-12 authority, `SHA256SUMS`, the pinned
native verifier, and the portable authority validator. The signed payload has
no build-host absolute path. It binds the full workspace-source identity,
exact-12 registry and retired-label set, validator, evidence runner, Cargo
lock, matrix, all authoritative Norito evidence and mandatory JSON
projections, plus either the archive digest or both OCI manifest and image
configuration digests.

Admission must first verify `release_manifest.json.sig` with the separately
reviewed signer fingerprint and verifier digest, then run
`taira_release_authority.py verify` against the candidate archive/image
evidence. The archive verifier parses the tar directly and rejects traversal,
duplicate members, links, sparse/special members, missing evidence, and any
evidence size or digest mismatch. An archive, image tag, unsigned
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
  python3 -S -c \
    'import hashlib, pathlib, sys; print(hashlib.sha256(pathlib.Path(sys.argv[1]).read_bytes()).hexdigest())' \
    "${authority}/artifacts/sorafs-validate"
)"
test "${actual_verifier_sha}" = "${verifier_sha}"
"${authority}/artifacts/sorafs-validate" release-manifest \
  --manifest "${authority}/release_manifest.json" \
  --public-key "${authority}/release_manifest.json.pub" \
  --public-key-fingerprint "${fingerprint}" \
  --signature "${authority}/release_manifest.json.sig"
python3 -S "${authority}/artifacts/taira_release_authority.py" verify \
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
   roster, then put the matching validator `private_key` values and the shared
   `account_onboarding_*`, `torii_faucet_*`, `streaming_identity_*`,
   `kagemusha_commands_private_key`, `offline_asset_alias = "ds#boi.is"`,
   the operator-provisioned canonical `offline_asset_definition_id`,
   `offline_asset_scale = 2`, and canonical Taira I105
   `offline_escrow_account`,
   `sorafs_council_public_keys`, and `sorafs_council_signature_threshold`
   values in the runtime file. SoraFS council roots must be canonical Ed25519
   governance keys; never substitute validator, node identity, or provider
   advert keys.
4. Render the per-validator bundle:
   - `python3 scripts/render_taira_validator_bundle.py --roster configs/soranexus/taira/validator_roster.local.toml --secrets configs/soranexus/taira/validator_secrets.local.toml --output-dir dist/taira-validators`
5. Copy each validator's complete generated directory to that validator
   host's canonical `/etc/iroha/taira-validator` directory. Every rendered
   config binds signer sidecars and governance manifests to that same
   first-release install root; it never embeds the developer checkout or
   `dist/` path. The renderer creates bundle and runtime directories with mode
   `0700`, creates the onboarding/faucet signer and API-token sidecars with mode
   `0600`, writes only canonical signer paths and the BLAKE3 token digest to
   peer config, and emits a protective `.gitignore`. It also creates co-located
   `kagemusha/` and `sorafs_admission/` directories and rewrites the policy,
   admission-envelope, signer, and manifest paths together when
   `--install-root` is changed. It prints sidecar paths but never their contents.

The bundle also contains one shared unsigned `genesis.json` whose dedicated
topology transaction is rebuilt from the public roster and PoPs, plus
`genesis-signing-command.txt`. Set `TAIRA_GENESIS_PRIVATE_KEY` only in the
operator shell and run that command. `kagami genesis sign --config` executes
genesis in a disposable state block, replaces the template Nexus/AMX context
hash with the exact staged value, recomputes the consensus fingerprint, and
then writes `genesis.signed.nrt`. Never copy the genesis signer or validator
private keys into the checked-in template or rendered genesis JSON.

## Authenticated offline-cash bootstrap

A fresh public Taira reset must activate the genuine ABI-21/V4 release at
height 2. Height 1 executes the genesis instructions; the mandatory staged
readiness gate evaluates that state at height 2. An activation height of 1 is
invalid, and the synthetic mobile-acceptance roster must not be used for a
deployed validator set.

Offline cash is non-optional on the canonical public Taira chain. `/health`
and `/readyz` fail closed with HTTP 503 until startup validation succeeds.
Cutover requires HTTP 200 with `mandatory: true`, `ready: true`,
`cash_handoff_capability: "cash_handoff_v1"`,
`required_bridge_abi_version: 21`, and an empty blocker list. Native MCP
availability alone is not sufficient.

First seal the rendered public validator keys and PoPs into the release-bound
top-up roster. The input config may contain runtime secrets, but the command
reads only `trusted_peers_pop` and emits only the public canonical roster:

```bash
cargo run -p iroha_kagami --bin kagami -- \
  kagemusha prepare-taira-release-roster-v4 \
  --validator-config /absolute/path/to/rendered-validator/config.toml \
  --output /absolute/private/path/taira-release-roster.norito
```

Generate the real Eq/Ep artifacts through the source-sealed two-stage
packager. Start from a checkout whose `HEAD` commit signature has been
verified, then explicitly review and seal its complete dirty closure before
building the exact candidate binary and entering the non-raiseable 16 GiB
generation guard. Keep at least 16 GiB free on its pinned disk-backed output
filesystem for the raw proving-key spools and framed artifact copy.
Retain the helper's canonical JSON report. The reviewed source closure and its
digest are release inputs; the report's `source_commit` must equal the verified
`HEAD`, and unreviewed working-tree changes fail closed.

```bash
python3 -I scripts/build_kagemusha_v4_candidate_bundle.py \
  --root "$PWD" \
  --target-dir /absolute/private/path/kagemusha-sealed-target \
  --reviewed-source-closure /absolute/private/path/reviewed-source-closure.json \
  --reviewed-source-closure-sha256 '<64-lowercase-hex>' \
  > /absolute/private/path/sealed-kagemusha-candidate-build.json

python3 scripts/run_kagemusha_v4_generation.py \
  --resource-report /absolute/private/path/taira-release-generation \
  -- \
  /absolute/path/from/sealed-build-report/kagemusha_recursive_spend_v4_bundle \
  generate-candidate \
  --out-dir /absolute/private/path/taira-release-candidate \
  --chain-id fc56984b-2be7-431d-840e-21514d1883f0 \
  --asset-definition-id 7ZepsJTHCVLKsrFFNZGSRGZgvBhv \
  --asset-scale 2 \
  --generation production-gate-real-artifacts-v4 \
  --parameter-generation production-gate-real-artifacts-v4 \
  --source-commit '<source_commit-from-sealed-build-report>' \
  --source-tree-sha256 '<source_tree_sha256-from-sealed-build-report>' \
  --activation-height 2 \
  --withdrawal-height 1000000000 \
  --step-eq-circuit-params /absolute/private/path/step-eq-circuit-params.norito \
  --step-ep-circuit-params /absolute/private/path/step-ep-circuit-params.norito \
  --topup-finality-roster /absolute/private/path/taira-release-roster.norito

/absolute/path/from/sealed-build-report/kagemusha_recursive_spend_v4_bundle \
  finalize-release \
  --candidate-dir /absolute/private/path/taira-release-candidate \
  --out-dir /absolute/private/path/taira-final-release \
  --release-policy /absolute/private/path/release-policy-v1.norito \
  --release-attestation /absolute/private/path/release-attestation-v4.norito \
  --benchmark-evidence /absolute/private/path/benchmark-evidence-v1.json \
  --cryptographic-review /absolute/private/path/cryptographic-review-v4.norito
```

`generate-candidate` is the only command accepted by the guarded runner; do not
wrap Cargo, a shell, or `env`. It publishes an immutable pre-evidence candidate
and owner-private JSONL/summary resource evidence, not an approved release.
`finalize-release` authenticates the supplied policy, attestation, physical
benchmark, and signed cryptographic review, then copies the exact candidate
bytes into a new sixteen-file release directory without regenerating proof
material. Provision the same policy as
`taira-release/release-policy-v1.norito` and install that finalized directory
as `taira-release/catalog/<manifest_sha256>/`, where `manifest_sha256` is the
lowercase digest recorded by the finalized `manifest.norito.sha256`.

Finally append the complete state to a clean unsigned Taira genesis. The
command never overwrites its input. App identities and signing-certificate
digests must describe the exact BOI builds being admitted; the helper supplies
only the native Apple App Attest and Android KeyMint roots and keeps both
platform policies fail-closed.

```bash
fresh_genesis_public_key='<fresh-ed0120-public-key>'
fresh_authority="$(
  /absolute/path/from/sealed-build-report/iroha \
    tools address convert \
    --profile taira \
    --format json \
    "${fresh_genesis_public_key}" |
  /opt/homebrew/bin/python3 -c \
    'import json, sys; p=json.load(sys.stdin); assert p["i105"]["network_prefix"] == 369; print(p["i105"]["value"])'
)"

cargo run -p iroha_kagami --bin kagami -- \
  kagemusha prepare-taira-testnet-bootstrap-v4 \
  --genesis /absolute/path/to/fresh-taira-genesis.json \
  --release-bundle /absolute/private/path/taira-release \
  --genesis-authority "${fresh_authority}" \
  --command-authority "${fresh_authority}" \
  --fee-mint 1000000 \
  --ios-team-id '<apple-team-id>' \
  --ios-bundle-id '<ios-bundle-id>' \
  --ios-validation-category 4 \
  --ios-bundle-version '<cf-bundle-version>' \
  --android-package-name '<android-package>' \
  --android-signing-certificate-sha256 '<64-lowercase-hex>' \
  --output /absolute/path/to/genesis.offline.json \
  --operator-identity-output /absolute/private/path/taira-offline-release-identity.json
```

The emitted manifest enables `offline.enabled` on the existing scale-2 Taira
asset `7ZepsJTHCVLKsrFFNZGSRGZgvBhv`, renames it to `ds`, rebinds it as
`ds#boi.is`, and replaces the legacy display metadata with code `DS`, ISO
currency `ILS`, symbol `₪`, and display name `Digital Shekel`. It preserves the
opaque asset ID, fixed scale, mintability, balances, and total supply. Use the
newly sealed `iroha` binary for the address conversion and require JSON
`i105.network_prefix = 369`; do not reuse a legacy global-profile conversion.
The reset uses one freshly generated key pair for both genesis and command
signing. The helper derives the canonical escrow, treats that shared
genesis/command authority as the account that `irohad` implicitly creates
before height one, registers and binds all three base verifiers, grants the
exact activation/device/escrow permissions, funds the shared authority, and
atomically activates the authenticated Eq/Ep release. The reset packager must
receive the same value through `--command-authority`; it independently derives
the Taira I105 literal from the fresh genesis public key and rejects any
mismatch or archived command-key reuse.
It also refuses a source genesis without non-zero public `ds#boi.is` liquidity outside escrow: top-up
atomically moves that exact user balance into escrow, and redemption can only
draw against finalized top-up provenance. The builder also replaces the legacy
1,000 ms source cadence with the authoritative 4,000 ms Sumeragi parameter
snapshot and verifies that effective cadence before recomputing consensus
metadata. Point every validator at the reported policy and catalog paths before
signing and deploying the reset. Preserve the separately emitted operator
identity outside the source checkout and pass it to rollout verification as
`--offline-expected-identity`; its artifact digests and five verifier
projections are derived from the same authenticated activation, not copied
from live state.

The renderer rewrites the checked-in peer-1 baseline with the full
`trusted_peers` / `trusted_peers_pop` roster so every validator starts from the
same bootstrap source of truth. It refuses to emit a config while the SoraFS
council placeholder remains or the configured quorum is zero, duplicated, or
larger than the trusted set. It also requires explicit per-validator
`torii_public_address` values so direct public Torii hostnames are part of the
checked operator input instead of a hard-coded shared edge default.
It also refuses every render that omits offline cash inputs, retains a
`REPLACE_WITH_` placeholder, uses an asset other than the registered scale-2
`ds#boi.is`, supplies a non-canonical asset-definition ID, or binds escrow to a
non-canonical Taira I105 account. The checked-in config contains no guessed DS
asset ID; the operator-provisioned ID is injected into
`settlement.offline.escrow_accounts` at render time.

## Private profiles

Application-specific private-dataspace profiles should live outside this repo.
When you need one, keep the profile in your own deployment repository and pass
it to the renderer explicitly:

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
and the mutable Kagemusha artifact tree after stopping the container, which is
the required step for a true genesis reset. Reset refuses broad system roots,
the read-only config bundle, and equal or nested state roots before it stops the
running container.

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

It requires four exact peer PID files, exact `irohad --sora --config ...`
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

## Validator container image

The repo now supports a dedicated Taira validator runtime image via the main
`Dockerfile`:

- attested local build helper from the sibling DPN API checkout:
  - `dpn-api-rust/ops/taira/build-validator-image.sh`
- manual publish workflow:
  - `.github/workflows/publish_taira_validator.yml`

Manual publish prerequisites:

- GitHub Actions secrets:
  - `DOCKERHUB_USERNAME`
  - `DOCKERHUB_TOKEN`
  - `HARBOR_USERNAME`
  - `HARBOR_TOKEN`
- a self-hosted runner with enough RAM to finish the release-profile Rust
  compile inside `docker build`; the current Taira workflow now forces
  `CARGO_BUILD_JOBS=1` unchanged and `BINARIES=irohad kagami` so the exact final
  image can run its four-peer smoke without building the unrelated `iroha` CLI
- one explicit `workflow_dispatch` run against the chosen release ref so the
  first `hyperledger/iroha:taira-*` and
  `docker.soramitsu.co.jp/iroha3/iroha:taira-*` tags actually exist before
  operator hosts switch to the published image path
- the exact 40-character DPN API commit containing the reviewed validator
  source bundle and Cargo lock; mutable branches and tags are rejected

If the Docker host is memory-constrained, cap Cargo parallelism during the
image build:

- `dpn-api-rust/ops/taira/build-validator-image.sh --cargo-build-jobs 1 --binaries 'irohad kagami'`

The DPN wrapper verifies the pinned base commit, exact binary worktree patch,
full reconstructed source-tree digest, Rust toolchain, and reviewed Cargo lock
before the Docker build starts. The Dockerfile independently requires the lock
and source-tree digests, uses `cargo --locked`, rejects prebuilt Taira binaries,
and records both digests in the image.

The image ships:

- `irohad`
- `kagami`, because the exact final attested image is itself required to
  generate and boot the four-peer release smoke topology
- the checked-in static Taira bundle under `/opt/iroha/configs/soranexus/taira`
- the bundled rANS codec tables under `/opt/iroha/codec/rans/tables`
- a Taira-aware entrypoint that defaults to:
  - `irohad --sora --config /etc/iroha/taira-validator/config.toml --genesis-manifest-json /opt/iroha/configs/soranexus/taira/genesis.json`

The image does **not** embed validator-specific runtime material. Keep using
`render_taira_validator_bundle.py` to generate the complete read-only
`/etc/iroha/taira-validator` mount from user-local roster/secrets files. The
matching authenticated Kagemusha policy belongs under `kagemusha/` in that
bundle; its mutable artifact tree is mounted separately.

`docker-compose.validator.yml` uses `pull_policy: missing` so an authority-bound
host-local image ID works without forcing a registry lookup. Production
launches require either that exact `sha256:<image-id>` or an admitted
`repository@sha256:<manifest-digest>`; rolling or source tags are not deployment
authority.

Minimal container run example:

```bash
TAIRA_IMAGE='hyperledger/iroha@sha256:<signed-manifest-digest>'
docker run -d --name taira-validator-1 \
  --restart unless-stopped \
  -e TAIRA_RUNTIME_PROFILE=production \
  -e TAIRA_IMAGE_REFERENCE="$TAIRA_IMAGE" \
  -p 1337:1337 \
  -p 18080:18080 \
  -v /etc/iroha/taira-validator:/etc/iroha/taira-validator:ro \
  -v /var/lib/iroha/taira-validator-1:/storage \
  -v /var/lib/iroha/taira-validator/kagemusha/v4:/var/lib/iroha/taira-validator/kagemusha/v4 \
  "$TAIRA_IMAGE"
```

If you need to override the bundled public genesis, point the entrypoint at a
mounted manifest file:

```bash
TAIRA_IMAGE='hyperledger/iroha@sha256:<signed-manifest-digest>'
docker run --rm \
  -e TAIRA_RUNTIME_PROFILE=production \
  -e TAIRA_IMAGE_REFERENCE="$TAIRA_IMAGE" \
  -e IROHA_TAIRA_GENESIS=/config/genesis.json \
  -v /etc/iroha/taira-validator:/etc/iroha/taira-validator:ro \
  -v "$PWD/configs/soranexus/taira/genesis.json:/config/genesis.json:ro" \
  -v /var/lib/iroha/taira-validator-1:/storage \
  -v /var/lib/iroha/taira-validator/kagemusha/v4:/var/lib/iroha/taira-validator/kagemusha/v4 \
  "$TAIRA_IMAGE"
```

If you need a disconnected or one-node smoke boot, mount both the manifest JSON
and a signed genesis payload. The entrypoint rewrites the copied
`/storage/runtime-config.toml` so `genesis.file` points at the mounted
`/config/genesis.signed.nrt` path:

```bash
docker run --rm \
  -e TAIRA_RUNTIME_PROFILE=localnet \
  -e IROHA_TAIRA_CONFIG=/config/config.toml \
  -e IROHA_TAIRA_GENESIS=/config/genesis.json \
  -e IROHA_TAIRA_SIGNED_GENESIS=/config/genesis.signed.nrt \
  -v "$PWD/dist/taira-localnet-smoke-container.toml:/config/config.toml:ro" \
  -v "$PWD/dist/taira-localnet-smoke/genesis.json:/config/genesis.json:ro" \
  -v "$PWD/dist/taira-localnet-smoke/genesis.signed.nrt:/config/genesis.signed.nrt:ro" \
  -v "$PWD/dist/taira-localnet-smoke-container-storage:/storage" \
  -p 28080:8080 \
  local/taira-validator:smoke
```

The checked-in `check_mcp_rollout.sh --skip-public --local-root ...` helper
still expects at least 4 live validators in `/v1/sumeragi/status`, so a
one-node smoke should be validated directly with `curl /health`,
`curl /status`, and `curl /v1/mcp` instead of the full rollout script.

For a local 4-validator container proof, render container-ready configs from a
fresh `kagami localnet` bundle and start the peers on one user-defined Docker
bridge:

```bash
python3 scripts/render_taira_localnet_container_bundle.py \
  --bundle-dir dist/taira-localnet-smoke \
  --output-dir dist/taira-localnet-cluster

docker network create taira-localnet >/dev/null 2>&1 || true
for peer in 0 1 2 3; do
  bash configs/soranexus/taira/taira-validator-container.sh \
    --env-file "dist/taira-localnet-cluster/peer${peer}.env" up
done

bash configs/soranexus/taira/check_mcp_rollout.sh \
  --skip-public \
  --local-root http://127.0.0.1:28080 \
  --offline-asset-definition-id "${OFFLINE_ASSET_DEFINITION_ID}" \
  --offline-expected-identity /run/secrets/taira-offline-release-identity.json \
  --skip-write-canary
```

That path is now validated on this host: peer0 publishes Torii counters on
`/status`, detailed wire-revision-3 reducer health on `/v1/sumeragi/status`, and
the repo rollout script passes end to end against the local cluster.

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
   `torii_public_address`, then start `irohad` against the published seed peers.
2. Wait for the node to sync and confirm lane mode:
   - `iroha app nexus lane-report --summary`
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
   - `curl -sS "${PUBLIC_TORII_ROOT}/v1/sumeragi/validator-sets" | jq .`

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

- `curl -sSki -X POST "${PUBLIC_TORII_ROOT}/v1/sorafs/pin/register" -H 'content-type: application/json' --data '{}'`

Expected result:

- `HTTP 400` with a handler-level validation error such as `missing field authority`

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
direct validator roots, the external reviewed offline identity, and a
runtime-only canary signer config. Define the non-optional fleet arguments once:

```bash
TAIRA_VALIDATOR_ARGS=(
  --validator-root validator-1=https://taira-validator-1.sora.org
  --validator-root validator-2=https://taira-validator-2.sora.org
  --validator-root validator-3=https://taira-validator-3.sora.org
  --validator-root validator-4=https://taira-validator-4.sora.org
)
```

- `bash configs/soranexus/taira/check_mcp_rollout.sh --public-root "${PUBLIC_TORII_ROOT}" "${TAIRA_VALIDATOR_ARGS[@]}" --require-all-validators --offline-asset-definition-id "${OFFLINE_ASSET_DEFINITION_ID}" --offline-expected-identity /run/secrets/taira-offline-release-identity.json --write-config /run/secrets/taira-canary-client.toml --expected-git-sha "${EXPECTED_TAIRA_GIT_SHA}"`

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

The rollout script requires `/v1/sumeragi/status` to advertise wire revision 3, a
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
with the signed canary while the fleet retains one exact offline release
identity.

That config must be a normal `iroha` client TOML for a low-risk runtime-only
signer. Start from `taira-canary-client.example.toml`, not
`defaults/client.toml`: the generic repo client uses the zero chain id and is
not valid for Taira. The canary alias defaults to the dataspace-root form
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
  - `curl -sS "${PUBLIC_TORII_ROOT}/v1/sumeragi/status" | jq '{protocol_version, node_fingerprint, build_fingerprint, config_fingerprint, height_context_id, height, view, phase: .phase.phase, leader, locked_prepare_qc, highest_prepare_qc, last_timeout_certificate, body_state: .body_state.state, pending_persistence_id, mode: .height_context.mode.mode, epoch: .height_context.epoch, validator_count: .height_context.validator_count, quorum: .height_context.quorum, last_committed_height, last_committed_subject, commit_qc_height: .last_commit_qc.certificate.round.height, commit_qc_signers: .last_commit_qc.signer_count, commit_qc_min_signers: .last_commit_qc.min_signers, commit_qc_signed_power: .last_commit_qc.signed_power, commit_qc_total_power: .last_commit_qc.total_power, view_change_install_total: .operator.view_change_install_total, busy_deferral_total: .operator.busy_deferral_total, tx_queue_depth: .operator.tx_queue.queued_transactions, tx_queue_capacity: .operator.tx_queue.capacity, saturated_by_count: .operator.tx_queue.saturated_by_count, saturated_by_age: .operator.tx_queue.saturated_by_age, oldest_queued_age_ms: .operator.tx_queue.oldest_queued_age_ms, lane_block_sessions: (.lane_block_sessions | length)}'`
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

## Qualification-sealed public Taira layout

The public Taira reset does not use the validator-owned Kagemusha directories
shown in the generic deployment examples below. The reset controller installs
the exact admitted `irohad` binary and release tree under a release-specific
`/Library/SORA/Taira/releases/<release-tree-sha256>/...` root. Every qualified
directory is owned by root and the validator runtime group with mode `0550`;
every qualified file has the same ownership and mode `0440`. This keeps the
tree immutable to the non-root validator while allowing that runtime identity
to read it. Generated validator configs point only at that installed policy
and artifact tree.

Before starting a validator, the controller injects the matching
`settlement.offline.kagemusha_catalog_qualification_seal_path`:

```text
/Library/SORA/Taira/seals/kagemusha-v4-<release-tree-sha256>.norito
```

It then runs the exact installed binary as root with `--check-config`, the
locally available genesis, and
`--write-kagemusha-catalog-qualification-seal` set to that same path. This
no-bind pass performs full catalog and genesis authentication and publishes a
new root-owned mode `0444` seal without replacing any existing path. Normal
validator startup may run as its non-root service identity, but the policy,
artifact, executable, and seal path chains remain root-controlled and only
readable by that identity. A new release tree always receives a new seal
filename.

Do not recursively `chown` any public-lane qualified source or seal path to uid
1001 or `iroha`; doing so invalidates the root-trust invariant and makes sealed
startup fail closed. Keep `/Library/SORA/Taira/seals` separate from the policy,
artifact, and executable directories because publishing the seal changes its
parent directory identity.

## Containerized validator deployment

Use this path when the validator host should run the published Docker image
instead of locally installed `irohad` binaries. The primary wrapper is
`taira-validator-container.sh`, which uses plain `docker` and therefore works
on hosts that lack the Compose plugin. `docker-compose.validator.yml` remains
available as an optional convenience for environments that do have Compose.
The uid-1001 Kagemusha layout in this section is an unsealed development or
private-testnet example. It is not valid for the qualification-sealed public
Taira lane. A public container deployment must instead mount the
controller-installed `/Library/SORA/Taira/releases/<release-tree-sha256>`
source tree and release-specific seal read-only, and use the injected generated
configuration described above.

1. Publish or otherwise load the image you intend to run on the host, verify
   its signed release authority, and retain the admitted manifest digest or
   image ID for `TAIRA_IMAGE`.
   - manual publish path:
     `workflow_dispatch` `.github/workflows/publish_taira_validator.yml`
   - local host-side override:
     `docker load < iroha3-<version>-linux-image.tar`
2. Render the validator config bundle from your user-local roster and secrets:
   - `python3 scripts/render_taira_validator_bundle.py --roster configs/soranexus/taira/validator_roster.local.toml --secrets configs/soranexus/taira/validator_secrets.local.toml --output-dir dist/taira-validators`
3. Install the rendered config and storage directories on the validator host:
   - the ownership commands in this step apply only to the unsealed generic
     layout; never apply them to public-lane qualified source or seal paths
   - `sudo install -d -m 0700 -o 1001 -g 1001 /etc/iroha/taira-validator`
   - `sudo install -d -o 1001 -g 1001 /var/lib/iroha/taira-validator-1`
   - `sudo install -d -m 0700 -o 1001 -g 1001 /var/lib/iroha/taira-validator/kagemusha/v4`
   - `sudo cp -a dist/taira-validators/taira-validator-1/. /etc/iroha/taira-validator/`
   - install the authenticated rollout policy as
     `/etc/iroha/taira-validator/kagemusha/release-policy.norito`, and install
     its reviewed artifact tree under
     `/var/lib/iroha/taira-validator/kagemusha/v4`; both are mandatory
   - install reviewed SoraFS admission envelopes, if any, under the rendered
     `/etc/iroha/taira-validator/sorafs_admission` directory
   - after installing all runtime inputs, run
     `sudo chown -R 1001:1001 /etc/iroha/taira-validator /var/lib/iroha/taira-validator-1 /var/lib/iroha/taira-validator/kagemusha/v4`
4. Copy the sample env file and adjust the host-specific values:
   - `sudo cp configs/soranexus/taira/taira-validator-container.compose.env.example /etc/default/taira-validator-container.compose.env`
   - set at least:
     - `TAIRA_IMAGE=hyperledger/iroha@sha256:<signed-manifest-digest>` or the
       authority-bound `sha256:<image-id>`; tags are rejected in production
     - `TAIRA_RUNTIME_PROFILE=production`
     - `TAIRA_CONFIG_BUNDLE_PATH=/etc/iroha/taira-validator`
     - `TAIRA_STORAGE_PATH=/var/lib/iroha/taira-validator-1`
     - `TAIRA_KAGEMUSHA_ARTIFACT_PATH=/var/lib/iroha/taira-validator/kagemusha/v4`
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
   - `bash configs/soranexus/taira/check_mcp_rollout.sh --skip-public --local-root http://127.0.0.1:18080 --offline-asset-definition-id "${OFFLINE_ASSET_DEFINITION_ID}" --offline-expected-identity /run/secrets/taira-offline-release-identity.json --skip-write-canary`
   - for a signed local write-path check:
     `bash configs/soranexus/taira/check_mcp_rollout.sh --skip-public --local-root http://127.0.0.1:18080 --offline-asset-definition-id "${OFFLINE_ASSET_DEFINITION_ID}" --offline-expected-identity /run/secrets/taira-offline-release-identity.json --write-config /run/secrets/taira-canary-client.toml --write-target local`

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

The `iroha:iroha` Kagemusha ownership examples below describe an unsealed
generic deployment. They must not be used for the public lane. Public Taira
uses the controller-installed, root-controlled release tree, executable, and
release-specific seal described in
[Qualification-sealed public Taira layout](#qualification-sealed-public-taira-layout).

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
3. Render the per-validator config bundle from a user-local roster file, then
   copy the correct validator config onto the host, for example:
   - `python3 scripts/render_taira_validator_bundle.py --roster configs/soranexus/taira/validator_roster.local.toml --secrets configs/soranexus/taira/validator_secrets.local.toml --output-dir dist/taira-validators`
   - `validator_secrets.local.toml` must include both the validator private
     keys and the shared `account_onboarding_*`, `torii_faucet_*`, and
     `streaming_identity_*`, `sorafs_council_public_keys`, and
     `sorafs_council_signature_threshold` fields because the checked-in template
     intentionally leaves those deployment values as fail-closed placeholders
   - the following `iroha`-owned paths are for the unsealed generic layout
     only; never recursively chown the public-lane qualified tree or seal
   - `sudo install -d -m 0700 -o iroha -g iroha /etc/iroha/taira-validator`
   - `sudo install -d -m 0700 -o iroha -g iroha /var/lib/iroha/taira-validator-1`
   - `sudo install -d -m 0700 -o iroha -g iroha /var/lib/iroha/taira-validator/kagemusha/v4`
   - `sudo cp -a dist/taira-validators/taira-validator-1/. /etc/iroha/taira-validator/`
   - preserve the generated `0600` modes; signer and governance paths already
     target this canonical install root and must not be rewritten
   - install the authenticated rollout policy at
     `/etc/iroha/taira-validator/kagemusha/release-policy.norito` and its
     artifact tree at `/var/lib/iroha/taira-validator/kagemusha/v4`
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
   - `sudo install -m 0755 dist/taira-rollout/<bundle>/bin/irohad /usr/local/bin/irohad`
   - `sudo install -m 0755 dist/taira-rollout/<bundle>/bin/iroha /usr/local/bin/iroha`
   - `sudo cp configs/soranexus/taira/taira-irohad.service /etc/systemd/system/`
   - copy `configs/soranexus/taira/taira-irohad.env.example` to
     `/etc/default/taira-irohad`; the supplied unit deliberately preflights the
     complete canonical `/etc/iroha/taira-validator` bundle, so do not override
     only `IROHA_TAIRA_CONFIG` to another install root
   - if a deployment intentionally uses another renderer `--install-root`,
     update the unit's config, signer, manifest, SoraFS admission, Kagemusha
     policy, and artifact preflight paths together before enabling it
   - if your repo checkout or binary path differs from `/opt/iroha` and
     `/usr/local/bin/irohad`, adjust `WorkingDirectory=` and set
     `IROHA_TAIRA_IROHAD_BIN=` in `/etc/default/taira-irohad` before enabling
     the unit
5. Reload systemd and restart the validator:
   - `sudo systemctl daemon-reload`
   - `sudo systemctl enable --now taira-irohad.service`
   - `sudo systemctl restart taira-irohad.service`
6. Capture the resolved config in the rollout ticket:
   - `sudo journalctl -u taira-irohad.service -n 200 --no-pager`
   - `cd /opt/iroha && sudo -u iroha env KURA_STORE_DIR=/var/lib/iroha/taira-validator-1 SNAPSHOT_STORE_DIR=/var/lib/iroha/taira-validator-1/snapshot /usr/local/bin/irohad --sora --config /etc/iroha/taira-validator/config.toml --genesis-manifest-json /opt/iroha/configs/soranexus/taira/genesis.json --trace-config | tee /tmp/taira-trace-config.txt`
   - verify `/tmp/taira-trace-config.txt` includes `nexus.fees.fee_asset_id = "xor#universal"`
7. Prove the validator's loopback Torii endpoint exposes MCP and the expected
   direct-ingress routes before any public cutover:
   - `bash configs/soranexus/taira/check_mcp_rollout.sh --skip-public --local-root http://127.0.0.1:18080 --offline-asset-definition-id "${OFFLINE_ASSET_DEFINITION_ID}" --offline-expected-identity /run/secrets/taira-offline-release-identity.json --skip-write-canary`
   - for a full local write-path check, use a runtime-only canary signer:
     `bash configs/soranexus/taira/check_mcp_rollout.sh --skip-public --local-root http://127.0.0.1:18080 --offline-asset-definition-id "${OFFLINE_ASSET_DEFINITION_ID}" --offline-expected-identity /run/secrets/taira-offline-release-identity.json --write-config /run/secrets/taira-canary-client.toml --write-target local`
8. After the public node is back, prove the direct hostname is healthy before
   any convenience host or client cutover:
   - `bash configs/soranexus/taira/check_mcp_rollout.sh --public-root "${PUBLIC_TORII_ROOT}" "${TAIRA_VALIDATOR_ARGS[@]}" --require-all-validators --offline-asset-definition-id "${OFFLINE_ASSET_DEFINITION_ID}" --offline-expected-identity /run/secrets/taira-offline-release-identity.json --write-config /run/secrets/taira-canary-client.toml --expected-git-sha "${EXPECTED_TAIRA_GIT_SHA}"`
   - if contract deploy/view health still fails after the route checks pass,
     redeploy SoraSwap with the updated `../soraswap` `deploy-testnet` flow
     before blaming the frontend
9. Before declaring public Codex/Torii rollout complete, require the SoraSwap
   gate to pass behind the same runtime candidate:
   - probe-only:
     `bash configs/soranexus/taira/verify_soraswap_rollout.sh --public-root "${PUBLIC_TORII_ROOT}" "${TAIRA_VALIDATOR_ARGS[@]}" --offline-asset-definition-id "${OFFLINE_ASSET_DEFINITION_ID}" --offline-expected-identity /run/secrets/taira-offline-release-identity.json --write-config /run/secrets/taira-canary-client.toml --expected-git-sha "${EXPECTED_TAIRA_GIT_SHA}" --soraswap-client-config /path/to/soraswap/config/testnet/taira.client.toml`
   - full gate:
     `bash configs/soranexus/taira/verify_soraswap_rollout.sh --public-root "${PUBLIC_TORII_ROOT}" "${TAIRA_VALIDATOR_ARGS[@]}" --offline-asset-definition-id "${OFFLINE_ASSET_DEFINITION_ID}" --offline-expected-identity /run/secrets/taira-offline-release-identity.json --write-config /run/secrets/taira-canary-client.toml --expected-git-sha "${EXPECTED_TAIRA_GIT_SHA}" --soraswap-client-config /path/to/soraswap/config/testnet/taira.client.toml --run-release-checklist --allow-testnet-mutations`
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
    upstream as `/v1/connect/session`, `/v1/connect/status`, and
    `/v1/connect/ws`. MCP exposes Connect session creation and management
    tools, and Connect tokens/state are process-local at creation time.
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
     `max_transactions = 96`, `max_payload_bytes = 22020096`, and
     `proposal_queue_scan_multiplier = 4` bounds, plus the
     `[sumeragi.queues]` canonical outer-ingress wire-byte baseline
     `authenticated_non_validator_sources = 2`, `body_bytes = 315621376`, and
     `body_source_bytes = 45088768`, before running public write canaries or
     scenario sweeps. The four-validator baseline isolates every validator,
     both authenticated non-validator source lanes, and anonymous delivery;
     `render_taira_validator_bundle.py` raises `body_bytes` to at least
     `(validator_count + authenticated_non_validator_sources + 1) *
     body_source_bytes` for larger legal rosters.
     The 21 MiB body is derived from two 10 MiB transaction-admission ceilings
     (each carrying one 9 MiB privacy action) plus 1 MiB of canonical block
     framing. The matching DA ceiling is `22020096`; the per-source queue
     rounds the exact ordinary/completion/timeout minimum up to 43 MiB. Keep
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
     `curl -sk --resolve taira-validator-1.sora.org:443:127.0.0.1 https://taira-validator-1.sora.org/v1/sumeragi/status | jq '.height, .last_committed_height, .last_commit_qc.certificate.round.height'`
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
   - `bash configs/soranexus/taira/check_mcp_rollout.sh --public-root "${PUBLIC_TORII_ROOT}" "${TAIRA_VALIDATOR_ARGS[@]}" --require-all-validators --offline-asset-definition-id "${OFFLINE_ASSET_DEFINITION_ID}" --offline-expected-identity /run/secrets/taira-offline-release-identity.json --expected-git-sha "${EXPECTED_TAIRA_GIT_SHA}"`
   - when you are validating edge-local SNI before public DNS or TLS is fully
     live, pin the public host to the edge IP explicitly:
     `bash configs/soranexus/taira/check_mcp_rollout.sh --public-root https://taira.sora.org "${TAIRA_VALIDATOR_ARGS[@]}" --require-all-validators --offline-asset-definition-id "${OFFLINE_ASSET_DEFINITION_ID}" --offline-expected-identity /run/secrets/taira-offline-release-identity.json --resolve-host taira.sora.org:443:127.0.0.1 --expected-git-sha "${EXPECTED_TAIRA_GIT_SHA}"`
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
     `curl -sS "${PUBLIC_TORII_ROOT}/v1/sumeragi/status" | jq '{protocol_version, node_fingerprint, build_fingerprint, config_fingerprint, height_context_id, height, view, phase: .phase.phase, leader, locked_prepare_qc, highest_prepare_qc, last_timeout_certificate, body_state: .body_state.state, pending_persistence_id, mode: .height_context.mode.mode, epoch: .height_context.epoch, validator_count: .height_context.validator_count, quorum: .height_context.quorum, last_committed_height, last_committed_subject, commit_qc_height: .last_commit_qc.certificate.round.height, commit_qc_signers: .last_commit_qc.signer_count, commit_qc_min_signers: .last_commit_qc.min_signers, commit_qc_signed_power: .last_commit_qc.signed_power, commit_qc_total_power: .last_commit_qc.total_power, tx_queue_depth: .operator.tx_queue.queued_transactions, tx_queue_capacity: .operator.tx_queue.capacity, saturated_by_count: .operator.tx_queue.saturated_by_count, saturated_by_age: .operator.tx_queue.saturated_by_age, view_change_install_total: .operator.view_change_install_total, busy_deferral_total: .operator.busy_deferral_total}'`
   - remember that `/status.peers` is the queried node's current remote-peer
     count, not the validator-set size; use
     `/v1/sumeragi/status` `height_context.validator_count` and
     `last_commit_qc.validator_count`, or
     `/v1/sumeragi/validator-sets` for validator-set visibility.
   - create a Connect session through the proxy and ask explicitly for JSON:
     `curl -sS -X POST "${PUBLIC_TORII_ROOT}/v1/connect/session" -H 'content-type: application/json' -H 'accept: application/json' -d '{"sid":"<32-byte-base64url-sid>"}'`
   - verify Connect websocket upgrades on both public hostnames with the
     returned `sid` and app token:
     `curl --http1.1 -i -N -H 'Connection: Upgrade' -H 'Upgrade: websocket' -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: dGVzdGtleTEyMzQ1Njc4OTA=' -H 'Sec-WebSocket-Protocol: iroha-connect.token.v1.<token_app>' "${PUBLIC_TORII_ROOT}/v1/connect/ws?sid=<sid>&role=app"`
     `curl --http1.1 -i -N -H 'Connection: Upgrade' -H 'Upgrade: websocket' -H 'Sec-WebSocket-Version: 13' -H 'Sec-WebSocket-Key: dGVzdGtleTEyMzQ1Njc4OTA=' -H 'Sec-WebSocket-Protocol: iroha-connect.token.v1.<token_app>' 'https://taira-explorer.sora.org/v1/connect/ws?sid=<sid>&role=app'`
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
3. Verify the relay endpoints and explorer page:
   - `curl -sk https://taira.sora.org/v1/kaigi/relays | jq .`
   - `curl -sk https://taira.sora.org/v1/kaigi/relays/health | jq .`
   - open `https://taira-explorer.sora.org/kaigi/relays`

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
