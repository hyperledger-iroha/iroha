---
lang: uz
direction: ltr
source: docs/source/sorafs_gateway_fixtures.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4fa6bff21db4a389a869499b8ff325637c05dc0332513dfaf9f863587bf5018e
source_last_modified: "2025-12-29T18:16:36.145738+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Gateway Fixture Governance
summary: Lifecycle, release metadata, and telemetry for the SF-5a conformance fixtures.
---

# SoraFS Gateway Fixture Governance

The SF-5a trustless delivery profile is validated against a deterministic
fixture bundle comprising manifests, PoR artefacts, a deterministic payload,
and replay scenarios. This document records the maintenance policy, release
metadata, and operator expectations for those fixtures.

## Current Release

| Field   | Value |
|---------|-------|
| Version | `1.0.0` |
| Profile | `sf1` |
| Released | 2026-02-12 00:00:00Z (`1770854400` Unix seconds) |
| Digests |
| | `fixtures_digest_blake3_hex = 83427e9d34ecdf4e2df6df159a0e2cb0a1fa6611b8898ef53e319ab535ba9588` |
| | `manifest_blake3_hex = 9bd42b2e745e8f808dd9c5b641297fd3b637b09283023aaaecdc2d013648645a` |
| | `payload_blake3_hex = 91275991d58858bdc7ce3eb4472b61c5289dec3ecc6cf43c6411db772c1888a8` |
| | `car_blake3_hex = ce50a9aadf84e57559208d39201621262fd1b1887ae490ca54470e2a00153f27` |

The digest advertised in telemetry is the BLAKE3 hash of the canonical
manifest, PoR challenge/proof, deterministic payload, and CAR archive
concatenated in that order. When the fixtures are re-derived the same digest
must be observed; any drift indicates the bundle has diverged from the
published release.

## Directory Layout

Fixture bundles live under `fixtures/sorafs_gateway/<semver>/`. Version `1.0.0`
contains the following artefacts:

| File | Description |
|------|-------------|
| `manifest_v1.to` | Norito-encoded manifest aligned with the `sf1` chunker profile |
| `challenge_v1.to`, `proof_v1.to` | PoR fixtures bound to the manifest digest |
| `payload.bin` | Deterministic 1 MiB payload used for CAR generation |
| `gateway.car` | Deterministic CAR stream emitted by the harness |
| `payload.blake3`, `gateway_car.blake3` | Convenience digests for spot checks |
| `scenarios.json` | Replay matrix (success + refusal cases) |
| `metadata.json` | Release timestamp, fixture digest, manifest and payload digests |

JSON mirrors of the Norito payloads are bundled alongside the `.to` artefacts
for convenience, but downstream SDKs should continue to rely on the canonical
Norito encodings when validating manifests and proofs.

## Regeneration Workflow

1. Run `cargo xtask sorafs-gateway-fixtures --out fixtures/sorafs_gateway/<ver>`
   to materialise the bundle under the desired version directory.
2. Verify `metadata.json` and the `*.blake3` helper files against the
   `torii_sorafs_gateway_fixture_info` (`version`, `profile`, `fixtures_digest`)
   and `torii_sorafs_gateway_fixture_version` (`version`) telemetry metrics.
   Run `scripts/verify_sorafs_fixtures.sh --dir fixtures/sorafs_gateway/<ver>`
   (wraps `cargo xtask sorafs-gateway-fixtures --verify`) to perform the digest
   checks, Norito decoding, and scenario diffing automatically.
3. Produce an attestation bundle via `cargo xtask sorafs-gateway-attest` and
   archive the signed manifest/digest alongside the release notes.
4. Once governance keys are available, update
   `fixtures/sorafs_chunker/manifest_signatures.json` with the council envelope.

## Verification Helpers

Teams that regenerate or mirror the fixtures can run the automated probe
`scripts/verify_sorafs_fixtures.sh` to confirm the bundle matches the
published metadata. The wrapper invokes
`cargo xtask sorafs-gateway-fixtures --verify` to:

- decode the Norito `.to` artefacts and ensure they match the expected schema;
- recompute the manifest/payload/CAR digests plus the aggregate
  `fixtures_digest_blake3_hex`;
- confirm the helper files (`payload.blake3`, `gateway_car.blake3`) mirror the
  computed hashes; and
- diff `scenarios.json` against the canonical matrix shipped with the repo.

For CI or governance packets that need a single summary of the trustless checks,
run the dedicated verifier CLI:

```bash
cargo run -p sorafs_car --bin soranet_trustless_verifier --features cli --locked -- \
  --manifest fixtures/sorafs_gateway/1.0.0/manifest_v1.to \
  --car fixtures/sorafs_gateway/1.0.0/gateway.car \
  --json-out artifacts/sorafs_gateway/trustless_summary.json
```

The summary records the manifest/CAR/payload digests, the reconstructed chunk
plan SHA3-256 digest, and the PoR root so the gateway conformance suite can
prove chunk-plan and PoR alignment without shelling out to ad hoc scripts.

Pass `--dir <path>` to check a different release directory and `--allow-online`
if Cargo must download dependencies (CI defaults to offline mode).

## Telemetry & Drift Detection

Torii exposes two metrics:

* `torii_sorafs_gateway_fixture_info{version="…",profile="…",fixtures_digest="…"}` –
  gauge value is the release timestamp; alerts should fire when unknown labels
  appear.
* `torii_sorafs_gateway_fixture_version{version="…"}` – new gauge set to `1`
  for the active version. Prometheus/Alertmanager can detect unexpected fixture
  versions by checking for a zero value.

Both metrics are populated at startup via the new telemetry hook and provide
operators with immediate visibility into fixture drift alongside existing TLS
and GAR telemetry.

## Release Governance

1. Tooling WG prepares fixture updates on a feature branch and regenerates the
   bundle locally.
2. Networking TL reviews manifest, metadata, and scenario diffs. Any change to
   the PoR fixtures or chunker profile requires explicit sign-off.
3. Governance Council signs the updated manifest envelope and attaches the new
   digest to the release notes.
4. Update this document and
   `docs/source/sorafs_gateway_conformance.md` with the release timestamp,
   digests, and operator guidance. Tag the commit that lands the fixtures and
   publish IPFS/SoraFS mirrors if applicable.
5. Operators regenerate attestations via
   `cargo xtask sorafs-gateway-attest` and archive the signed bundle alongside
   the metric output from their telemetry plane.

## Pending Actions

- [x] Regenerate the canonical fixtures and publish the digests above. ✅
- [ ] Attach the signed manifest envelope for version `1.0.0` once council key
      material is available.
