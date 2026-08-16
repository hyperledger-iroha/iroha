# Kagami Iroha3 Profiles

Kagami ships presets for Iroha 3 networks so operators can stamp deterministic
genesis manifests without juggling per-network knobs.

- Profiles: `iroha3-dev` uses chain `iroha3-dev.local` and derives its VRF seed
  from the chain id when NPoS is selected. `iroha3-taira` uses chain
  `iroha3-taira`, and `iroha3-nexus` uses chain `iroha3-nexus`; both public
  profiles require `--vrf-seed-hex` when NPoS is selected.
- Consensus: Sora profile networks (Nexus + dataspaces) require NPoS and disallow staged cutovers; permissioned Iroha3 deployments must run without a Sora profile.
- Generation: `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>] [--xor-asset-definition-id <BASE58>]`. Use `--consensus-mode npos` for Nexus; `--vrf-seed-hex` is only valid for NPoS (required for taira/nexus). Public Taira defaults `xor#universal` to the live canonical XOR id `6TEAJqbb8oEPmLncoNiMRbLEK6tw`; public Nexus requires `--xor-asset-definition-id` because the canonical Nexus XOR id is operator-supplied.
- Verification: `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` replays profile expectations for chain id, cadence, consensus mode and VRF seed, PoP coverage, and the normalized consensus fingerprint. Supply `--vrf-seed-hex` only when verifying an NPoS manifest for taira/nexus.
- Signed fixture bundles pass an explicit `--creation-time-ms` to `kagami genesis sign`; this fixes transaction and block timestamps so repeated profile generation produces identical `genesis.signed.nrt` bytes.
- Every generated validator config allocates one isolated body-ingress byte partition per frozen validator, configured authenticated non-validator source, and anonymous source. The aggregate therefore scales with the complete generated committee instead of inheriting the four-validator default floor.
- Topology: the Taira fixture renders seven logical lanes over the five catalogued
  physical dataspaces (`universal`, `dpn`, `is`, `is2`, `cbsi`), with
  `core`/`governance`/`zk` all bound to `universal`. The Nexus/Minamoto fixture
  renders those three logical lanes in the single `universal` dataspace.
  Namespace text remains a separate binding layer and is never promoted into
  either catalog.
- Physical-deployment limit: the deterministic Taira sample uses one harness
  committee to test config/genesis binding. It does not provision five
  disjoint server cohorts or their per-dataspace manifests, so it is not
  deployable evidence for those physical boundaries; the rollout gate remains
  fail-closed until the deployment repository supplies that evidence.
- Sample bundles: pre-generated bundles live under `defaults/kagami/iroha3-{dev,taira,nexus}/` (genesis.json, config.toml, docker-compose.yml, verify.txt, README). Regenerate with `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>] [--nexus-xor-asset-definition-id <BASE58>]`; the Nexus flag is required when generating `iroha3-nexus` or `all`.
- Mochi: `mochi`/`mochi-genesis` accept `--genesis-profile <profile>` and `--vrf-seed-hex <hex>` (NPoS only), forward them to Kagami, and print the same Kagami summary to stdout/stderr when a profile is used.

The bundles embed BLS PoPs alongside topology entries so `kagami verify` succeeds
out of the box; adjust the trusted peers/ports in the configs as needed for local
smoke runs.
