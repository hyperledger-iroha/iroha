---
lang: az
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
translator: machine-google-reviewed
---

Tam sürətli başlanğıc `docs/source/nexus_sdk_quickstarts.md`-də yaşayır. Bu portal
xülasə, tərtibatçılar üçün paylaşılan ilkin şərtləri və hər SDK əmrlərini vurğulayır
onların quraşdırılmasını tez yoxlaya bilər.

## Paylaşılan quraşdırma

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Nexus konfiqurasiya paketini endirin, hər bir SDK-nın asılılığını quraşdırın və əmin olun
TLS sertifikatları buraxılış profilinə uyğun gəlir (bax
`docs/source/sora_nexus_operator_onboarding.md`).

## Pas

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

Skript yuxarıdakı env varsları ilə `ToriiClient`-i yaradır və çap edir.
son blok.

## Sürətli

```bash
make swift-nexus-demo
```

`FindNetworkStatus` əldə etmək üçün `IrohaSwift`-dən `Torii.Client` istifadə edir.

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Nexus səviyyəli son nöqtəyə çatan idarə olunan cihaz testini həyata keçirir.

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Problemlərin aradan qaldırılması

- TLS uğursuzluqları → Nexus buraxılış tarballundan CA paketini təsdiq edin.
- `ERR_UNKNOWN_LANE` → bir dəfə çox zolaqlı marşrutla `--lane-id`/`--dataspace-id` keçin
  icra olunur.
- `ERR_SETTLEMENT_PAUSED` → [Nexus əməliyyatları](../nexus/nexus-operations) üçün yoxlayın
  insident prosesi; idarəetmə zolağı dayandırmış ola bilər.

Daha dərin kontekst və SDK-ya xas izahatlar üçün baxın
`docs/source/nexus_sdk_quickstarts.md`.