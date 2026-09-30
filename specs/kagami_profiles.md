# Kagami Iroha3 Profiles

Kagami ships presets for Iroha 3 networks so operators can stamp deterministic
genesis manifests without juggling per-network knobs.

- Profiles: `iroha3-dev` uses chain `iroha3-dev.local` and derives its VRF seed
  from the chain id when NPoS is selected. `iroha3-taira` uses the live chain
  `fc56984b-2be7-431d-840e-21514d1883f0`; `iroha3-nexus` uses the public
  Minamoto/Nexus chain `00000000-0000-0000-0000-000000000753`. Both public profiles require NPoS and an explicit
  `--vrf-seed-hex`. Profile chain identities are immutable: an explicit
  `--chain-id` is accepted only when it exactly repeats the profile's pinned id.
- Consensus: Sora profile networks (Nexus + dataspaces) require NPoS and disallow staged cutovers; permissioned Iroha3 deployments must run without a Sora profile.
- Generation: `cargo run -p iroha_kagami -- genesis generate --profile <profile> --ivm-dir . --genesis-public-key <pk> --kagemusha-mint-finality-parameters <PUBLIC-PARAMETERS.json> --consensus-mode <npos|permissioned> [--vrf-seed-hex <hex>] [--xor-asset-definition-id <BASE58>]`. The public parameter file is mandatory and must contain the separately provisioned generation-zero KAGEMUSHA authority; generation never derives authority secrets from validator identities. Use `--consensus-mode npos` for Taira and Nexus; `--vrf-seed-hex` is required for both public profiles. Public Taira defaults `xor#universal` to the live canonical XOR id `6TEAJqbb8oEPmLncoNiMRbLEK6tw`; public Nexus requires `--xor-asset-definition-id` because the canonical Nexus XOR id is operator-supplied.
- Verification: `cargo run -p iroha_kagami -- verify --profile <profile> --genesis <path> [--vrf-seed-hex <hex>]` replays profile expectations for chain id, cadence, consensus mode and VRF seed, PoP coverage, and the normalized consensus fingerprint. Supply `--vrf-seed-hex` only when verifying an NPoS manifest for taira/nexus.
- Signed fixture bundles pass an explicit `--creation-time-ms` to `kagami genesis sign`; this fixes transaction and block timestamps so repeated profile generation produces identical `genesis.signed.nrt` bytes.
- Every generated validator config renders only the node-local `[sumeragi]` role. Committee geometry, block limits, and payload limits come from signed genesis and committed state; the node rejects retired `sumeragi.queues` and `sumeragi.block` tables.
- Topology: the Nexus/Minamoto fixture renders `core`, `governance`, and `zk`
  lanes in the single `universal` dataspace. Namespace text remains a separate
  binding layer and is never promoted into either catalog.
- Disposable Taira: use
  `python3 scripts/taira_devnet.py up --inrou-canary-dir <owner-only-workspace>`, inspect it with
  `python3 scripts/taira_devnet.py check`, and remove it with
  `python3 scripts/taira_devnet.py down`. This is the only repository-owned
  disposable Taira deployment workflow.
- Profile sources: `defaults/kagami/iroha3-{dev,nexus}/` contains incomplete
  `genesis.template.json` sources plus non-secret topology/configuration scaffolding, not runnable
  signed bundles. Materialize complete operator-owned output with
  `cargo xtask kagami-profiles --kagemusha-mint-finality-parameters-dir <DIR> --xor-allocations-dir <ALLOCATIONS> [--profile <name>|all] [--out <private-dir>] [--kagami <bin>] [--nexus-xor-asset-definition-id <BASE58>]`; `<DIR>` must contain one operator-provisioned `<profile>.json` public-authority file containing the required generation-zero `authority_generation` template, whose validator identities exactly match the final topology. Scheduling authorizations are derived from the signed genesis and later certified boundaries; no successor schedule is a genesis parameter. The Nexus flag is required when generating `iroha3-nexus` or `all`.
- Mochi: `mochi` accepts `--genesis-profile <profile>` and `--vrf-seed-hex <hex>` (NPoS only) and forwards them to Kagami for generation, signing, and verification.

Generated bundles embed BLS PoPs alongside topology entries and bind the separately provisioned
KAGEMUSHA authority, so `kagami verify` can validate the complete operator-owned output. Adjust the
trusted peers and ports before materialization when preparing a deployment.

The profile publisher requires `--xor-allocations-dir <DIR>` alongside its explicit
public authority directory. Each `<profile>.json` allocation file contains
`version: 1`, the exact committed `asset_definition_id`, and `allocations` sorted
by canonical account identity, each with `account` and positive exact `amount`.
Every generated validator must be explicitly allocated at least its 10,000 XOR
genesis self bond. These are reviewed genesis allocations; neither genesis signing
nor localnet startup mints missing balances. Nexus requires its operator-provisioned
mainnet XOR identity and rejects the Taira definition.
