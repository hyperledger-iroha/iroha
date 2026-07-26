---
lang: pt
direction: ltr
source: docs/source/sorafs_gateway_direct_mode.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4b300b43e15582f88834678d1c2d43b61a71f0de05be978133c80b98752cc767
source_last_modified: "2026-01-22T15:38:30.697810+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS Gateway Direct-Mode Toolkit
summary: CLI workflow and configuration knobs for the `sorafs.gateway.direct_mode` overrides.
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
- Direct-CAR endpoints (`https://{host}/direct/v1/car/{manifest_digest_hex}`) generated from the
  manifest digest.
- Capability flags detected from manifest metadata and admission adverts (Torii gateway, QUIC/Noise,
  `sorafs_manifest::manifest_capabilities::detect_manifest_capabilities`.

Use `--admission-envelope` to supply a governance-signed admission bundle when you need canonical
capability metadata, or pass `--provider-id` directly when running against local fixtures.

## Enabling the Override

Feed the JSON plan into the `enable` subcommand to produce a configuration snippet. The snippet
targets the new `sorafs.gateway.direct_mode` table alongside the standard gateway knobs:

```bash
iroha app sorafs gateway direct-mode enable --plan direct-mode-plan.json
```

The `enable` subcommand fails before printing configuration when the plan is not canonical: provider
and manifest digests must be lowercase 32-byte hex values, derived hostnames and direct-CAR HTTPS
URLs must match the plan inputs, the manifest must require an envelope, and the manifest metadata
must advertise direct-CAR support.

Apply the snippet to your Torii configuration (`config.toml`). The fields under
`sorafs.gateway.direct_mode` map 1:1 to the plan output:

- `provider_id_hex`, `chain_id`
- `canonical_host`, `vanity_host`
- `direct_car_canonical`, `direct_car_vanity`
- `manifest_digest_hex`

While the direct-mode override is active, `sorafs.gateway.require_manifest_envelope`,
`enforce_admission`, and `enforce_capabilities` remain enabled. Direct mode only installs the
deterministic provider/route mapping; it must not be used as a bypass for envelope, admission, or
capability enforcement.

## Rolling Back

To restore the default routing path, remove the `direct_mode` table and keep envelope, admission,
and capability checks enabled. The CLI prints the rollback snippet for convenience:

```bash
iroha app sorafs gateway direct-mode rollback
```

Paste the snippet into your configuration or use it as a checklist when reverting changes in your
configuration management system.

## Smoke Wrapper Output Safety

`scripts/sorafs_direct_mode_smoke.sh` fails before invoking `sorafs_cli` when
the payload output, fetch summary, adoption report, or policy-derived
scoreboard path is a symlink, points at a non-regular file, or sits under a
symlinked parent component. This keeps direct-mode rollout evidence from being
written through ambiguous filesystem aliases while still allowing the wrapper to
create missing ordinary output directories.
