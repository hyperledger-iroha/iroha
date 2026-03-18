---
lang: fr
direction: ltr
source: docs/portal/docs/sdks/nexus-quickstarts.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Le guide complet est en `docs/source/nexus_sdk_quickstarts.md`. Ce résumé du portail indique les prérequis partagés et les commandes du SDK pour que les développeurs vérifient rapidement leur configuration.

## Configuration partagée

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Téléchargez le paquet de configuration de Nexus, installez les dépendances de chaque SDK et confirmez que les certificats TLS correspondent au profil de version (version `docs/source/sora_nexus_operator_onboarding.md`).

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

L'instance de script `ToriiClient` avec les variables d'arrivée et imprime le dernier blocage.

## Rapide

```bash
make swift-nexus-demo
```

Utilisez `Torii.Client` de `IrohaSwift` pour obtenir `FindNetworkStatus`.

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Effectuez la vérification du périphérique administré qui pointe vers le point final de staging de Nexus.

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Solution aux problèmes

- Fallas TLS -> confirma el bundle CA del tarball of release de Nexus.
- `ERR_UNKNOWN_LANE` -> passer `--lane-id`/`--dataspace-id` lorsque l'enrutamiento maritime multivoie est obligatoire.
- `ERR_SETTLEMENT_PAUSED` -> révision [Opérations Nexus](../nexus/nexus-operations) pour le processus d'incident ; la gobernanza peut interrompre la voie.

Pour plus de contexte et d'explications concernant le SDK, consultez `docs/source/nexus_sdk_quickstarts.md`.