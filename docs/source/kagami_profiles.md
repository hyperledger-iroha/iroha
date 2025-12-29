# Kagami Iroha3 Profiles

Kagami ships presets for Iroha 3 networks so operators can stamp deterministic
genesis manifests without juggling per-network knobs.

- Profiles: `iroha3-dev` (chain `iroha3-dev.local`, collectors k=1 r=1, VRF seed derived from chain id), `iroha3-testus` (chain `iroha3-testus`, collectors k=3 r=2, requires `--vrf-seed-hex`), `iroha3-nexus` (chain `iroha3-nexus`, collectors k=5 r=2, requires `--vrf-seed-hex`).
- Generation: `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode npos [--vrf-seed-hex <hex>]`. Kagami pins DA/RBC on the Iroha3 line and emits a summary (chain, collectors, DA/RBC, VRF seed, fingerprint).
- Verification: `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` replays profile expectations (chain id, DA/RBC, collectors, VRF seed rules, PoP coverage, consensus fingerprint) and reports a compact summary.
- Sample bundles: pre-generated bundles live under `defaults/kagami/iroha3-{dev,testus,nexus}/` (genesis.json, config.toml, docker-compose.yml, verify.txt, README). Regenerate with `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>]`.
- Mochi: `mochi`/`mochi-genesis` accept `--genesis-profile <profile>` and `--vrf-seed-hex <hex>`, forward them to Kagami, and print the same Kagami summary to stdout/stderr when a profile is used.

The bundles embed BLS PoPs alongside topology entries so `kagami verify` succeeds
out of the box; adjust the trusted peers/ports in the configs as needed for local
smoke runs.
