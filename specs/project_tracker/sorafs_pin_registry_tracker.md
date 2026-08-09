# SoraFS Pin Registry Contract Tracker

This tracker coordinates local implementation and rollout evidence for the SoraFS
Pin Registry contract under SF-4. It inherits the requirements defined in the
[SoraFS Architecture RFC (SF-1)](../sorafs_architecture_rfc.md), including the
canonical manifest digest flow and governance envelopes.

| ID | Milestone | Owners | Target Window | Status | Notes |
|----|-----------|--------|---------------|--------|-------|
| PR-001 | Contract scaffolding (`RegisterPinManifest`, `ApprovePinManifest`, `RetirePinManifest`) | Storage Team; Nexus Core Infra TL | Q4 2025 | Complete | The native lifecycle instructions, world-state storage, and ISI dispatch are implemented. Registration is a paid public operation for an authenticated account; approval authority comes from the threshold envelope; retirement is submitter-only. All three event epochs are derived from block consensus time and retired client epoch fields are rejected. |
| PR-002 | Governance signature plumbing | Governance Secretariat; Tooling WG | Q1 2026 | Complete locally | Core validates Ed25519 council envelopes during `ApprovePinManifest`; any authenticated account may relay the bounded envelope without acquiring governance authority. Torii/CLI submit an exact-network signed transaction with one canonical manifest instruction, while Dilithium/ML-DSA governance verification lives in the SF-11 reference validator and release-policy surface. |
| PR-003 | Alias, retention, quota, and finalized-query enforcement | Storage Team | Q1 2026 | Complete | Alias binding validation, uniqueness, retention windows, replica-count policy, global/per-account count-and-byte accounting, lineage depth/fanout, consensus-time expiry, and lifecycle-status indexes live in Core. Torii returns finalized exclusive-keyset `PinManifestPageV1` pages with row/byte ceilings and O(1) charged usage; the digest route returns one exact bounded native record. |
| PR-004 | CI + fixture parity | Tooling WG | Q1 2026 | Complete | `ci/check_sorafs_fixtures.sh` regenerates chunker, provider-admission, and pin-registry fixtures; core unit coverage includes contract-focused alias, successor, replication-order, and policy guard tests. |
| PR-005 | Rollout documentation & operator guide | Docs Team | Q1 2026 | Rollout evidence | `specs/sorafs/runbooks/pin_registry_ops.md`, migration docs, CLI docs, and API surfaces are published; live cutover packets and governance archive handoff are deployment evidence. |

## References

- [`specs/sorafs_architecture_rfc.md`](../sorafs_architecture_rfc.md)
- [`fixtures/sorafs_chunker/manifest_signatures.json`](../../fixtures/sorafs_chunker/manifest_signatures.json)
- [`ci/check_sorafs_fixtures.sh`](../../ci/check_sorafs_fixtures.sh)
