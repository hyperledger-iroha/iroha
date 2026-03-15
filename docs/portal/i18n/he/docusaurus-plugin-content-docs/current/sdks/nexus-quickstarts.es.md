---
lang: he
direction: rtl
source: docs/portal/docs/sdks/nexus-quickstarts.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

La guia completa esta en `docs/source/nexus_sdk_quickstarts.md`. קורות חיים של פורטל נדל"ן או שירותים מוקדמים ותוספות ל-SDK עבור בדיקת הגדרות התצורה המהירות.

## התאמה לתצורה

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

הורד את חבילת התצורה של Nexus, התקן את התלויות של ה-SDK ואת האישורים לאישורי TLS בצירוף אישורים לשחרור (גרסה `docs/source/sora_nexus_operator_onboarding.md`).

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

El script instancia `ToriiClient` con las variables de entorno de arriba e imprime el ultimo bloque.

## סוויפט

```bash
make swift-nexus-demo
```

Usa `Torii.Client` de `IrohaSwift` למשתמש `FindNetworkStatus`.

## אנדרואיד

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Ejecuta la prueba de dispositivo administrado que apunta al endpoint de staging de Nexus.

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## פתרון בעיות

- Fallas TLS -> מאשר את הצרור CA del tarball de release de Nexus.
- `ERR_UNKNOWN_LANE` -> pasa `--lane-id`/`--dataspace-id` cuando el enrutamiento multi-lane sea obligatorio.
- `ERR_SETTLEMENT_PAUSED` -> revisa [Nexus פעולות](../nexus/nexus-operations) para el processo de incidentes; la gobernanza pudo pausar la lane.

עבור ההקשר וההסברים של SDK consulta `docs/source/nexus_sdk_quickstarts.md`.