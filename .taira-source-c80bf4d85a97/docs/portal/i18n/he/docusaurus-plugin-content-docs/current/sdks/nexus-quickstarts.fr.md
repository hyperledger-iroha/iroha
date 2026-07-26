---
lang: he
direction: rtl
source: docs/portal/docs/sdks/nexus-quickstarts.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Le guide complet se trouve dans `docs/source/nexus_sdk_quickstarts.md`. זה קורות חיים של פורטיל עם התקדמות מוקדמות ותקשורות ל-SDK כדי לאמת את תצורת המהירות.

## קומונת תצורה

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

הורד את חבילת התצורה Nexus, התקן את התלויות של ה-SDK ואת האישורים לכתבי TLS בפרופיל השחרור (לפי `docs/source/sora_nexus_operator_onboarding.md`).

## חלודה

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

רפים: `docs/source/sdk/rust.md`

## JavaScript / TypeScript

```bash
npm run demo:nexus
```

מופע התסריט `ToriiClient` כולל משתנים סביבתיים וציורי גוש.

## סוויפט

```bash
make swift-nexus-demo
```

השתמש ב-`Torii.Client` de `IrohaSwift` pour recuperer `FindNetworkStatus`.

## אנדרואיד

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

בצע את מבחן הלבוש ב-Nexus.

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Depannage

- Echecs TLS -> Verifier le bundle CA du tarball de release Nexus.
- `ERR_UNKNOWN_LANE` -> עובר `--lane-id`/`--dataspace-id` une fois le routage אפליקציה מרובה נתיבים.
- `ERR_SETTLEMENT_PAUSED` -> יועץ [Nexus פעולות](../nexus/nexus-operations) pour le processus d'incident; la governance a peut etre mis lane en pause.

Pour plus de contexte et d'explications par SDK, voir `docs/source/nexus_sdk_quickstarts.md`.