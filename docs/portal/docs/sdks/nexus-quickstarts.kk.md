---
lang: kk
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 08d231445c0eb56985d360594393a1fd0fec06b53fdcf8defbe0b2439191ee2f
source_last_modified: "2026-01-22T14:45:01.264878+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-quickstarts
title: Nexus SDK quickstarts
description: Minimal steps for Rust/JS/Swift/Android/CLI SDKs to connect to Sora Nexus.
---

The full quickstart lives at `docs/source/nexus_sdk_quickstarts.md`. This portal
summary highlights the shared prerequisites and per-SDK commands so developers
can verify their setup quickly.

## Shared setup

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Download the Nexus config bundle, install each SDK’s dependencies, and ensure
TLS certificates match the release profile (see
`docs/source/sora_nexus_operator_onboarding.md`).

## Rust

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

Refs: `docs/source/sdk/rust.md`

## JavaScript / TypeScript

```bash
npm run demo:nexus
```

The script instantiates `ToriiClient` with the env vars above and prints the
latest block.

## Swift

```bash
make swift-nexus-demo
```

Uses `Torii.Client` from `IrohaSwift` to fetch `FindNetworkStatus`.

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Runs the managed-device test hitting the Nexus staging endpoint.

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Troubleshooting

- TLS failures → confirm the CA bundle from the Nexus release tarball.
- `ERR_UNKNOWN_LANE` → pass `--lane-id`/`--dataspace-id` once multi-lane routing
  is enforced.
- `ERR_SETTLEMENT_PAUSED` → check [Nexus operations](../nexus/nexus-operations) for the
  incident process; governance may have paused the lane.

For deeper context and SDK-specific explanations see
`docs/source/nexus_sdk_quickstarts.md`.
