# Taira

Taira's disposable testnet path is one command:

```bash
python3 scripts/taira_devnet.py up
```

It builds the current `kagami`, `iroha3d`, and `iroha` binaries, replaces the
previous script-owned bundle under `dist/taira-devnet/`, generates exactly four
fresh-key NPoS validators for the canonical Taira chain, validates every peer
configuration, starts the peers, and waits for all four nodes to become ready.
It then submits one blocking signed `iroha tx ping`, requires all four committed
heights to advance and converge, and checks that the generated MCP endpoint can
initialize and list tools.

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
same ports cannot satisfy it.

Stop it while retaining generated configs and logs:

```bash
python3 scripts/taira_devnet.py down
```

Teardown returns success only after every managed PID file and matching peer
process is gone. If that cannot be proved, the bundle is retained for diagnosis
and the command fails instead of deleting its ownership evidence.

Run the broader public-product route diagnostic only when that is the purpose
of the deployment:

```bash
python3 scripts/taira_devnet.py up --full-doctor
```

Use already-built binaries when iterating on orchestration:

```bash
python3 scripts/taira_devnet.py up \
  --no-build \
  --bin-dir "$PWD/target/local-release"
```

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
- `privacy_bootstrap_plan.json`, `privacy_rollout_plan_v1.json`, and
  `validate_privacy_bootstrap.py` remain coupled to Kagami's compiled privacy
  bootstrap feature.
- `dns_records.json`, `explorer.runtime-config.json`, `sorafs_sites.json`, and
  `taira-canary-client.example.toml` describe the live public profile.
- `validator_roster.example.toml`, the edge renderer, nginx template, and edge
  installer remain the public-ingress configuration surface.
- `check_sorafs_rollout.sh`, `verify_soraswap_rollout.sh`, and
  `check_inrou_host_prereqs.sh` are application checks, not deployment gates.

The retired reset, release, evidence, host-supervision, and soak scripts are
intentionally gone. New deployment behavior belongs in the compiled Kagami,
daemon, or CLI surface first; keep this wrapper limited to process orchestration
and end-to-end smoke verification.
