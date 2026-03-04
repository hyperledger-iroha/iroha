---
lang: fr
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Le guide complet est sur `docs/source/nexus_sdk_quickstarts.md`. Ce résumé du portail décrit les prérequis partagés et les commandes du SDK pour que les développeurs vérifient rapidement leur configuration.

## Configuration du partage

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Utilisez le paquet de configuration Nexus, installez en tant que dépendances de chaque SDK et garantissez que les certificats TLS correspondent au profil de version (voir `docs/source/sora_nexus_operator_onboarding.md`).

## Rouille

```bash
cargo run --bin nexus_quickstart \
  -- --torii "${NEXUS_TORII_URL}" \
  --pipeline "${NEXUS_PIPELINE_URL}" \
  --chain "${NEXUS_CHAIN_ID}"
```

Réfs : `docs/source/sdk/rust.md`

## JavaScript/TypeScript

```bash
npm run demo:nexus
```

Le script est `ToriiClient` comme variable de température ambiante et imprime le bloc le plus récent.

## Rapide

```bash
make swift-nexus-demo
```

Utilisez `Torii.Client` et `IrohaSwift` pour rechercher `FindNetworkStatus`.

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Exécuter le test du périphérique géré par le point de terminaison de staging Nexus.

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Résolution des problèmes

- Falhas TLS -> confirmez le bundle CA de l'archive tar de la version Nexus.
- `ERR_UNKNOWN_LANE` -> passe `--lane-id`/`--dataspace-id` lorsque le roulement multivoie est imposto.
- `ERR_SETTLEMENT_PAUSED` -> vérifier [Opérations Nexus](../nexus/nexus-operations) pour le processus d'incident ; une gouvernance peut ter ter pauser une voie.

Pour plus de contexte et d'explications sur le SDK, voir `docs/source/nexus_sdk_quickstarts.md`.