---
lang: kk
direction: ltr
source: docs/source/references/address_prefix_registry.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 95d764f52f9b0bbaeb7dc20e31debd76f2636404fef5aabdd3e8d9114a7f00dc
source_last_modified: "2026-01-05T09:28:12.037623+00:00"
translation_last_reviewed: 2026-02-07
title: i105 Network Prefix Registry
description: Authoritative mapping between Iroha chain discriminants and i105 address prefixes.
---

# i105 Network Prefix Registry

The i105 encoding reserves a 6‑bit–to–14‑bit prefix that identifies the
originating chain. Each network MUST register a unique prefix so wallets,
relays, and admission pipelines can detect cross-network mismatches before
processing transactions. This registry is the authoritative source for the
assigned values; SDKs and operators should treat it as canonical and watch for
updates.

Machine-readable data is published alongside this document at
[`address_prefix_registry.json`](address_prefix_registry.json). The JSON file
includes a `version` field so automated tooling can detect schema-breaking
changes.

## Registered Prefixes

| Network | Chain ID | Chain Discriminant | i105 Prefix | Status | Notes |
|---------|----------|--------------------|-------------|--------|-------|
| Sora Nexus (Global) | `sora:nexus:global` | `0x02F1` (753) | `0x02F1` (753) | Production | Canonical prefix for the global Nexus network. Matches the Sora `chain_discriminant`; all production SDK builds MUST default to this value. |
| Sora Testus (Testnet) | `809574f5-fee7-5e69-bfcf-52451e42d50f` | `0x0171` (369) | `0x0171` (369) | Testnet | Canonical prefix for the public Testus network. Use this prefix when validating addresses and emitting i105 strings for Testus deployments. |
| Development (local) | `dev.local` | `0x0000` (0) | `0x0000` (0) | Reserved | Convenience prefix for single-node or CI environments. Never use on public networks; collisions are expected. |

### Usage Guidelines

- **Consistency:** Always derive i105 prefixes directly from the chain
  discriminant published in manifests and configuration files. Hard-coding
  divergent values will cause Torii admission to reject requests with
  `ERR_UNEXPECTED_NETWORK_PREFIX`.
- **Collision avoidance:** Prefix assignments are globally unique. Before
  onboarding a new chain, file a registration entry in this document (and the
  accompanying JSON) and ensure downstream SDKs include the mapping.
- **Validation:** Wallets and relays SHOULD compare the i105 prefix in incoming
  addresses against the expected chain discriminant before submitting requests.
- **Change management:** Any update that modifies an existing prefix MUST be
  coordinated across the governance council and SDK maintainers. Add a new row
  instead of mutating historical assignments to preserve auditability.

## Requesting a New Prefix

1. Determine the chain discriminant that will identify the new network.
2. Submit a pull request updating both this document and the JSON registry with
   the new tuple `{network, chain_id, chain_discriminant, i105_prefix}`. Prefix
   values MUST live in the inclusive range `0x0000..=0x3FFF` (0–16383).
3. Engage SDK owners (Rust, JS/TS, Swift, Android, Python) to ensure the new
   prefix is available in configuration defaults before launch.
4. Update operational runbooks and manifests so operators can validate the
   assigned value during rollout.
