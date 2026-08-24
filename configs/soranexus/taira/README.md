# Taira

Taira's disposable testnet path is one command:

```bash
python3 scripts/taira_devnet.py up
```

It builds the current `kagami`, `iroha3d_taira`, and `iroha` binaries, replaces
the previous script-owned bundle under `dist/taira-devnet/`, generates exactly
four fresh-key NPoS validators for the canonical Taira chain, validates every
peer configuration, starts the peers, and waits for all four nodes to become
ready. It then submits one signed `iroha tx ping`, waits for its typed `Applied`
status, requires all four committed heights to advance and converge, and checks
that every generated MCP endpoint can initialize and list tools. The build uses
the repository's stable local metadata and shared target cache, so unrelated
worktree metadata does not force a cold rebuild.

There is no release authority, source-closure ceremony, evidence archive,
promotion state, 24-hour soak, host service installation, or predecessor
rollback in this disposable path.

## Daily commands

Inspect the running cohort without writing to it:

```bash
python3 scripts/taira_devnet.py check
```

`check` binds the listeners to the generated Taira chain, genesis hash,
loopback ports, and the four exact PID/config pairs; unrelated services on the
same ports cannot satisfy it. It reads the Torii base port from the generated
`client.toml`, so an `up` started with a custom `--base-api-port` needs no
repeated port argument.

Stop it while retaining generated configs and logs:

```bash
python3 scripts/taira_devnet.py down
```

Teardown returns success only after every managed PID file and matching peer
process is gone. If that cannot be proved, the bundle is retained for diagnosis
and the command fails instead of deleting its ownership evidence.

Public-product route qualification is separate from this disposable local
smoke. Run the same-revision `iroha taira doctor` directly against the public
ingress being qualified; local `up` intentionally has no doctor gate or
application-route options.

The dedicated daemon's config validation, help, and version commands are
offline introspection surfaces: they never open or consume the inherited
runtime-signer descriptor. Every node-starting invocation still requires the
exact descriptor and compiled Taira profile.

Use already-built binaries when iterating on orchestration:

```bash
python3 scripts/taira_devnet.py up \
  --no-build \
  --bin-dir "$PWD/target/local-release"
```

The directory needs the three default binaries above.

The output directory is owner-only and contains private keys and runtime
tokens. Never commit, print, upload, or archive it. On failure the command stops
the failed peers, keeps the bounded peer logs in place, and exits non-zero.

## Public Taira endpoint checks

The compiled CLI owns the current public API contract. Build it from the same
revision being deployed, copy the example config to an owner-only runtime path,
and replace its key placeholders before use:

```bash
cargo build --locked --profile local-release -p iroha_cli --bin iroha
target/local-release/iroha -c /private/runtime/client.toml \
  taira doctor --public-root https://taira.sora.org --json
```

The public doctor remains non-mutating and does not impersonate an operator.
It requires the exact two-field operator-signature `401` from
`/v1/sumeragi/status` and the exact two-field canonical-account `401` from
`/v1/soracloud/status`; arbitrary gateway challenges fail closed. Exact runtime
topology and four-replica Inrou convergence belong to the signed Inrou canary,
not the public route-posture probe.

The offline `taira inrou-stage` command assigns the canary an immutable
`artifact-<digest>` service version derived from the complete canonical bundle
with only the version field cleared. Final guest publication references are
therefore part of the revision identity. A staged directory and receipt bind
the service manifest, container manifest, materialized bundle, and both SoraFS
manifests; changing any input produces a different revision.

The signed `taira inrou-canary` mutation path checks authoritative SoraCloud
state before publishing either staged SoraFS manifest. `deploy` requires the
service to be absent. `upgrade` requires it to exist at a different immutable
revision; replaying the current staged revision fails before upload. That
preflight produces a mandatory signed compare-and-set condition: deploy binds
service absence, while upgrade binds the exact current version, service and
container manifest hashes, positive process generation, and the current config
and secret generations. The ledger checks the condition atomically in the same
transaction that admits the new revision, so revision or material drift after
preflight cannot become a lost update. Process, config, and secret generations
use checked monotonic increments; an exhausted counter fails closed and never
wraps or saturates into a replayable token.

An upgrade cannot supersede an active rollout or change the service execution
plane, container runtime, or route identity. The admitted candidate becomes the
current revision while the required, distinct baseline remains active for the
complement of the canary traffic split; promotion or rollback must finish that
rollout before another upgrade begins.

After the explicit mutation, convergence requires the exact current/latest
version, both staged manifest hashes, four placements, and a positive process
generation on every status poll. A failed or changed status generation discards
all collected route evidence. Each accepted health response must also carry Torii-owned
served-service, served-version, replica-slot, process-generation, and
materialized-bundle headers. Torii only stamps those headers for an
authoritatively healthy placement whose host capability is valid, unexpired,
and matches the validator, peer, backend, and guest ISA, and whose exact bundle,
generation, and snapshot peer identity match the node-local process. Local-only
health or an expired host advert never substitutes for authoritative state.
Both local and remote ingress overwrite upstream values, so guest self-reporting
or a stale process cannot satisfy the proof.

For an explicitly authorized public write canary, copy the example client
configuration to a runtime-only location and supply the onboarding token from
an owner-only runtime file:

```bash
target/local-release/iroha -c /private/runtime/client.toml \
  --fee-payer authority \
  taira write-canary \
  --public-root https://taira.sora.org \
  --onboarding-token-file /private/runtime/onboarding.token \
  --write-config /private/runtime/canary-client.toml \
  --json
```

Do not persist signing keys, onboarding tokens, bearer tokens, or forwarded
authorization headers in this repository.

## Retained source-coupled assets

- `config.toml` and `genesis.json` are canonical profile fixtures consumed by
  compiled Kagami/config/genesis tests. They are not inputs to the disposable
  generator.
- `privacy_bootstrap_plan.json` and `privacy_rollout_plan_v1.json` remain
  coupled to Kagami's compiled privacy bootstrap feature.
- `dns_records.json`, `explorer.runtime-config.json`, `sorafs_sites.json`, and
  `taira-canary-client.example.toml` describe the live public profile.
- `validator_roster.example.toml`, the edge renderer, nginx template, and edge
  installer remain the public-ingress configuration surface.

The retired reset, release, evidence, host-supervision, and soak scripts are
intentionally gone. New deployment behavior belongs in the compiled Kagami,
daemon, or CLI surface first; keep this wrapper limited to process orchestration
and end-to-end smoke verification.
