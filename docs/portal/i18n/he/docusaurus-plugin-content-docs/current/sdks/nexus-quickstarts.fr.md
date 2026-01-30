---
lang: he
direction: rtl
source: docs/portal/docs/sdks/nexus-quickstarts.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

Le guide complet se trouve dans `docs/source/nexus_sdk_quickstarts.md`. Ce resume du portail met en avant les prerequis communs et les commandes par SDK pour que les developpeurs verifient rapidement leur configuration.

## Configuration commune

```bash
export NEXUS_TORII_URL="https://torii.nexus.sora.org"
export NEXUS_PIPELINE_URL="https://torii.nexus.sora.org/v1/pipeline"
export NEXUS_CHAIN_ID="iroha3"
export NEXUS_TRUSTED_PUBKEY="<peer-public-key>"
```

Telechargez le paquet de configuration Nexus, installez les dependances de chaque SDK et assurez-vous que les certificats TLS correspondent au profil de release (voir `docs/source/sora_nexus_operator_onboarding.md`).

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

Le script instancie `ToriiClient` avec les variables d'environnement ci-dessus et affiche le dernier bloc.

## Swift

```bash
make swift-nexus-demo
```

Utilise `Torii.Client` de `IrohaSwift` pour recuperer `FindNetworkStatus`.

## Android

```bash
./gradlew :iroha-android:nexusQuickstartTest \
  -PNEXUS_TORII_URL="${NEXUS_TORII_URL}" \
  -PNEXUS_PIPELINE_URL="${NEXUS_PIPELINE_URL}"
```

Execute le test d'appareil gere qui vise le point de terminaison de staging Nexus.

## CLI

```bash
iroha_cli app nexus quickstart \
  --torii-url "${NEXUS_TORII_URL}" \
  --pipeline-url "${NEXUS_PIPELINE_URL}" \
  --chain-id "${NEXUS_CHAIN_ID}"
```

## Depannage

- Echecs TLS -> verifier le bundle CA du tarball de release Nexus.
- `ERR_UNKNOWN_LANE` -> passer `--lane-id`/`--dataspace-id` une fois le routage multi-lane applique.
- `ERR_SETTLEMENT_PAUSED` -> consulter [Nexus operations](../nexus/nexus-operations) pour le processus d'incident; la gouvernance a peut etre mis la lane en pause.

Pour plus de contexte et d'explications par SDK, voir `docs/source/nexus_sdk_quickstarts.md`.
