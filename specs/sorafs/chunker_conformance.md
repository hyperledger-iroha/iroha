# SoraFS Chunker Conformance Guide

This guide codifies the requirements every implementation must follow to stay
aligned with the SoraFS deterministic chunker profile (SF1). It also
documents the regeneration workflow, signing policy, and verification steps so
fixture consumers across SDKs remain in sync.

> Public and in-depth documentation is maintained in the sibling
> `iroha-docs` repository and published at <https://docs.iroha.tech/>.

## Canonical Profile

- Profile handle: `sorafs.sf1@1.0.0`
- Input seed (hex): `0000000000dec0ded`
- Target size: 262 144 bytes (256 KiB)
- Minimum size: 65 536 bytes (64 KiB)
- Maximum size: 524 288 bytes (512 KiB)
- Rolling polynomial: `0x3DA3358B4DC173`
- Gear table seed: `sorafs-v1-gear`
- Break mask: `0x0000FFFF`

Reference implementation: `sorafs_chunker::chunk_bytes_with_digests_profile`.
Any SIMD acceleration must produce identical boundaries and digests.

## Fixture Bundle

`export_vectors --write` regenerates the fixtures in a private external stage,
validates the complete signed tree, and only then publishes the following
checked-in files:

- `sf1_profile_v1.{json,rs,ts,go}` — canonical chunk boundaries for Rust,
  TypeScript, and Go consumers. Each file advertises the canonical handle as the
  first (and only) entry in `profile_aliases`. The ordering is enforced by
  `ensure_charter_compliance` and MUST NOT be altered.
- `manifest_blake3.json` — BLAKE3-verified manifest covering every fixture file.
- `manifest_signatures.json` — Council signatures (Ed25519) over the manifest
  digest.
- `fuzz/sorafs_chunker/sf1_profile_v1_{input.bin,backpressure.json}` —
  deterministic generated input/scenario fixtures consumed by
  `crates/sorafs_chunker/tests/backpressure.rs`; they are not evidence that a
  libFuzzer target ran.

### Signing Policy

Fixture regeneration **must** include a valid council signature. The generator
rejects unsigned output in every mode. An unchanged manifest reuses and verifies
the checked-in signature envelope inside the private stage; a changed manifest
requires an explicit private signing-key file before any checked-in path can be
published. `--check` performs the same staging and validation without publishing.

To add a council signature:

```bash
sf1_stage="$(mktemp -d "${TMPDIR:-/tmp}/iroha-sf1.XXXXXX")"
sf1_stage="$(cd -- "$sf1_stage" && pwd -P)"
chmod 700 "$sf1_stage"
cargo run --locked -p sorafs_chunker --features dev-tools --bin export_vectors \
  -- --write --staging-root "$sf1_stage" \
  --signing-key-file /absolute/private/council-signing-key.hex
```

## Verification

The CI helper `ci/check_sorafs_fixtures.sh` replays the generator with
`--locked`. If fixtures drift or signatures are missing, the job fails. Use
this script in nightly workflows and before submitting fixture changes.

Manual verification steps:

1. Run `node scripts/check_sf1_vectors.mjs`.
2. Run `cargo test -p sorafs_chunker`.
3. Invoke `ci/check_sorafs_fixtures.sh` locally.
4. Confirm `git status -- fixtures/sorafs_chunker fuzz/sorafs_chunker` is clean.

## Upgrade Playbook

When proposing a new chunker profile or updating SF1:

See also: [`specs/sorafs/chunker_profile_authoring.md`](chunker_profile_authoring.md) for
metadata requirements, proposal templates, and validation checklists.

1. Draft a `ChunkProfileUpgradeProposalV1` (see RFC SF‑1) with new parameters.
2. Regenerate fixtures via `export_vectors` and record the new manifest digest.
3. Sign the manifest with the required council quorum. All signatures must be
   appended to `manifest_signatures.json`.
4. Update affected SDK fixtures (Rust/Go/TS) and ensure cross-runtime parity.
5. Regenerate fuzz corpora if parameters change.
6. Update this guide with the new profile handle, seeds, and digest.
7. Submit the change alongside updated tests and roadmap updates.

Changes that affect chunk boundaries or digests without following this process
are invalid and must not be merged.
