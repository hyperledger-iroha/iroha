# Kagami Iroha3 Profiles

Kagami ships presets for Iroha 3 networks so operators can stamp deterministic
genesis manifests without juggling per-network knobs.

- Profiles: `iroha3-dev` uses chain `iroha3-dev.local` and derives its VRF seed
  from the chain id when NPoS is selected. `iroha3-taira` uses the live chain
  `fc56984b-2be7-431d-840e-21514d1883f0`; `iroha3-nexus` uses chain
  `iroha3-nexus`. Both public profiles require NPoS and an explicit
  `--vrf-seed-hex`. Profile chain identities are immutable: an explicit
  `--chain-id` is accepted only when it exactly repeats the profile's pinned id.
- Consensus: Sora profile networks (Nexus + dataspaces) require NPoS and disallow staged cutovers; permissioned Iroha3 deployments must run without a Sora profile.
- Generation: `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>] [--xor-asset-definition-id <BASE58>]`. Use `--consensus-mode npos` for Taira and Nexus; `--vrf-seed-hex` is required for both public profiles. Public Taira defaults `xor#universal` to the live canonical XOR id `6TEAJqbb8oEPmLncoNiMRbLEK6tw`; public Nexus requires `--xor-asset-definition-id` because the canonical Nexus XOR id is operator-supplied.
- Verification: `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` replays profile expectations for chain id, cadence, consensus mode and VRF seed, PoP coverage, and the normalized consensus fingerprint. Supply `--vrf-seed-hex` only when verifying an NPoS manifest for taira/nexus.
- Signed fixture bundles pass an explicit `--creation-time-ms` to `kagami genesis sign`; this fixes transaction and block timestamps so repeated profile generation produces identical `genesis.signed.nrt` bytes.
- Every generated validator config allocates one isolated body-ingress byte partition per frozen validator and configured authenticated non-validator source; identityless ingress has no partition. The aggregate is at least `(N + H) * body_source_bytes` for the complete generated committee, with the intentional generic configured floor retained when it is larger.
- Topology: the Nexus/Minamoto fixture renders `core`, `governance`, and `zk`
  lanes in the single `universal` dataspace. Namespace text remains a separate
  binding layer and is never promoted into either catalog.
- Disposable Taira: use
  `python3 scripts/taira_devnet.py up --inrou-canary-dir <owner-only-workspace>`, inspect it with
  `python3 scripts/taira_devnet.py check`, and remove it with
  `python3 scripts/taira_devnet.py down`. This is the only repository-owned
  disposable Taira deployment workflow.
- Sample bundles: pre-generated bundles live under
  `defaults/kagami/iroha3-{dev,nexus}/` (genesis.json, config.toml,
  docker-compose.yml, verify.txt, README). Regenerate them with
  `cargo xtask kagami-profiles [--profile <name>|all] [--out <dir>] [--kagami <bin>] [--nexus-xor-asset-definition-id <BASE58>]`; the Nexus flag is required when generating `iroha3-nexus` or `all`.
- Mochi: `mochi`/`mochi-genesis` accept `--genesis-profile <profile>` and `--vrf-seed-hex <hex>` (NPoS only), forward them to Kagami, and print the same Kagami summary to stdout/stderr when a profile is used.

The bundles embed BLS PoPs alongside topology entries so `kagami verify` succeeds
out of the box; adjust the trusted peers/ports in the configs as needed for local
smoke runs.
