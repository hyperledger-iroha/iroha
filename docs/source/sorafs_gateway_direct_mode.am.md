---
lang: am
direction: ltr
source: docs/source/sorafs_gateway_direct_mode.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d811b95f39cf7b2d8b7e554a5242d24b7e9eee8a0716be1da86644163b000fa7
source_last_modified: "2026-01-22T14:35:37.556642+00:00"
translation_last_reviewed: 2026-02-07
title: SoraFS Gateway Direct-Mode Toolkit
summary: CLI workflow and configuration knobs for the `torii.sorafs_gateway.direct_mode` overrides.
---

# SoraFS Gateway Direct-Mode Toolkit

The SoraFS gateway ships with a conservative security posture: manifest envelopes, admission
membership, and capability checks are enforced on every request. When operators need a deterministic
fallback (for example, while onboarding providers before SoraNet transports are live) they can use
the direct-mode toolkit to plan the rollout, generate configuration snippets, and safely revert to
default settings.

## Planning a Direct-Mode Rollout

The CLI inspects manifests and (optionally) admission envelopes to compute the derived hostnames,
direct-CAR endpoints, and capability flags required for a safe rollout:

```bash
iroha app sorafs gateway direct-mode plan \
  --manifest fixtures/sorafs_manifest/example_manifest.to \
  --provider-id 1111111111111111111111111111111111111111111111111111111111111111
```

The command emits JSON capturing:

- Canonical and vanity hostnames derived from the provider id (`HostMappingInput` in
  `sorafs_manifest::hosts`).
- Direct-CAR endpoints (`https://{host}/direct/v2/car/{manifest_digest_hex}`) generated from the
  manifest digest.
- Capability flags detected from manifest metadata and admission adverts (Torii gateway, QUIC/Noise,
  `sorafs_manifest::manifest_capabilities::detect_manifest_capabilities`.

Use `--admission-envelope` to supply a governance-signed admission bundle when you need canonical
capability metadata, or pass `--provider-id` directly when running against local fixtures.

## Enabling the Override

Feed the JSON plan into the `enable` subcommand to produce a configuration snippet. The snippet
targets the new `torii.sorafs_gateway.direct_mode` table alongside the standard gateway knobs:

```bash
iroha app sorafs gateway direct-mode enable --plan direct-mode-plan.json
```

Apply the snippet to your Torii configuration (`config.toml`). The fields under
`torii.sorafs_gateway.direct_mode` map 1:1 to the plan output:

- `provider_id_hex`, `chain_id`
- `canonical_host`, `vanity_host`
- `direct_car_canonical`, `direct_car_vanity`
- `manifest_digest_hex`

While the direct-mode override is active, `torii.sorafs_gateway.require_manifest_envelope` and
`enforce_admission` are explicitly disabled to match the snippet output.

## Rolling Back

To restore the secure defaults, remove the `direct_mode` table and re-enable envelope/admission
checks. The CLI prints the rollback snippet for convenience:

```bash
iroha app sorafs gateway direct-mode rollback
```

Paste the snippet into your configuration or use it as a checklist when reverting changes in your
configuration management system.
