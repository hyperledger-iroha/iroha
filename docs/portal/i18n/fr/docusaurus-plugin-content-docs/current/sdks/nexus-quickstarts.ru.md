---
lang: fr
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Le démarrage rapide est effectué dans `docs/source/nexus_sdk_quickstarts.md`. Ce portail permet d'obtenir des pré-commandes et des commandes pour le SDK, afin que les robots puissent être vérifiés.

## Общая настройка

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v2/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Téléchargez le paquet de configuration Nexus, installez le SDK et ouvrez le certificat TLS. профилю реLISа (см. `docs/source/sora_nexus_operator_onboarding.md`).

## Rouille

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

Numéro : `docs/source/sdk/rust.md`

## JavaScript/TypeScript

```bash
npm run demo:nexus
```

Le script est créé `ToriiClient` avec l'ouverture temporaire de votre commande et le bloc suivant.

## Rapide

```bash
make swift-nexus-demo
```

Utilisez `Torii.Client` ou `IrohaSwift` pour utiliser `FindNetworkStatus`.

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Lors du test, l'utilisateur a effectué le test Nexus.

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Устранение неполадок

- Utilisez TLS -> vérifiez le bundle CA à partir de l'archive tar de la version Nexus.
- `ERR_UNKNOWN_LANE` -> avant `--lane-id`/`--dataspace-id`, vous pouvez ouvrir une marche multivoie.
- `ERR_SETTLEMENT_PAUSED` -> смотрите [Opérations Nexus](../nexus/nexus-operations) pour le processus d'incident ; возможно, la gouvernance приостановила voie.

Pour plus de détails sur le contexte et l'utilisation du SDK. `docs/source/nexus_sdk_quickstarts.md`.