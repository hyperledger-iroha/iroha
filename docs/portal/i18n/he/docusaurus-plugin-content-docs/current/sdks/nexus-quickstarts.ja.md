---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/sdks/nexus-quickstarts.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 57e02ce6942e29a92bc1c8dfbc6ab5fd02308d5d1a15656834ad5a6c95afb884
source_last_modified: "2025-11-14T04:43:21.095307+00:00"
translation_last_reviewed: 2026-01-30
---

המדריך המלא נמצא ב-`docs/source/nexus_sdk_quickstarts.md`. תקציר הפורטל הזה מדגיש את דרישות הקדם המשותפות ואת הפקודות לכל SDK כדי שמפתחים יאמתו במהירות את ההתקנה שלהם.

## הגדרה משותפת

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

הורידו את חבילת הקונפיגורציה של Nexus, התקינו את התלויות לכל SDK, ודאו שתעודות ה-TLS תואמות לפרופיל ההפצה (ראו `docs/source/sora_nexus_operator_onboarding.md`).

## Rust

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

מקורות: `docs/source/sdk/rust.md`

## JavaScript / TypeScript

```bash
npm run demo:nexus
```

הסקריפט יוצר מופע של `ToriiClient` עם משתני הסביבה שלמעלה ומדפיס את הבלוק האחרון.

## Swift

```bash
make swift-nexus-demo
```

משתמש ב-`Torii.Client` מ-`IrohaSwift` כדי למשוך `FindNetworkStatus`.

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

מריץ את בדיקת המכשיר המנוהל שפונה לנקודת הקצה של ה-staging של Nexus.

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## פתרון תקלות

- כשלים ב-TLS -> ודאו את חבילת ה-CA מה-tarball של גרסת Nexus.
- `ERR_UNKNOWN_LANE` -> העבירו `--lane-id`/`--dataspace-id` לאחר שאכיפת ניתוב multi-lane מופעלת.
- `ERR_SETTLEMENT_PAUSED` -> בדקו את [Nexus operations](../nexus/nexus-operations) לתהליך האירוע; ייתכן שהממשל השהה את ה-lane.

להקשר נוסף ולהסברים לכל SDK ראו `docs/source/nexus_sdk_quickstarts.md`.
