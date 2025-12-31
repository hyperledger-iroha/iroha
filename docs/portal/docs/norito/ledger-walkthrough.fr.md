<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ea0c0b2f750131568e801b5fe583ae46ebddda3ce4f9fb52387725c2e227520
source_last_modified: "2025-11-07T12:25:39.145308+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: Parcours du registre
description: Reproduisez un flux déterministe register -> mint -> transfer avec le CLI `iroha` et vérifiez l'état du ledger résultant.
slug: /norito/ledger-walkthrough
---

Ce parcours complète le [quickstart Norito](./quickstart.md) en montrant comment modifier et inspecter l'état du ledger avec le CLI `iroha`. Vous enregistrerez une nouvelle définition d'actif, minterez des unités sur le compte opérateur par défaut, transférerez une partie du solde vers un autre compte et vérifierez les transactions et avoirs résultants. Chaque étape reflète les flux couverts par les quickstarts SDK Rust/Python/JavaScript afin de confirmer la parité entre le CLI et le comportement des SDK.

## Prérequis

- Suivez le [quickstart](./quickstart.md) pour démarrer le réseau mono-pair via
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Assurez-vous que `iroha` (le CLI) est construit ou téléchargé et que vous pouvez
  joindre le peer avec `defaults/client.toml`.
- Outils optionnels : `jq` (formatage des réponses JSON) et un shell POSIX pour les
  snippets de variables d'environnement ci-dessous.

Tout au long du guide, remplacez `$ADMIN_ACCOUNT` et `$RECEIVER_ACCOUNT` par les
IDs de compte que vous comptez utiliser. Le bundle par défaut inclut déjà deux comptes
issus des clés de démo :

```sh
export ADMIN_ACCOUNT="ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
export RECEIVER_ACCOUNT="ed012004FF5B81046DDCCF19E2E451C45DFB6F53759D4EB30FA2EFA807284D1CC33016@wonderland"
```

Confirmez les valeurs en listant les premiers comptes :

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Inspecter l'état genesis

Commencez par explorer le ledger ciblé par le CLI :

```sh
# Domains enregistrés en genesis
iroha --config defaults/client.toml domain list all --table

# Accounts dans wonderland (remplacez --limit par un nombre plus élevé si besoin)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions qui existent déjà
iroha --config defaults/client.toml asset definition list all --table
```

Ces commandes reposent sur des réponses Norito, donc le filtrage et la pagination sont
 déterministes et correspondent à ce que reçoivent les SDK.

## 2. Enregistrer une définition d'actif

Créez un nouvel actif infiniment mintable appelé `coffee` dans le domaine
`wonderland` :

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

Le CLI affiche le hash de transaction soumis (par exemple, `0x5f…`). Conservez-le
pour consulter le statut plus tard.

## 3. Minter des unités sur le compte opérateur

Les quantités d'actifs vivent sous la paire `(asset definition, account)`. Mintez 250
unités de `coffee#wonderland` dans `$ADMIN_ACCOUNT` :

```sh
iroha --config defaults/client.toml asset mint \
  --id coffee#wonderland##${ADMIN_ACCOUNT} \
  --quantity 250
```

Encore une fois, récupérez le hash de transaction (`$MINT_HASH`) depuis la sortie du CLI. Pour
vérifier le solde, exécutez :

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

ou, pour cibler seulement le nouvel actif :

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${ADMIN_ACCOUNT}\"}" \
  --limit 1 | jq .
```

## 4. Transférer une partie du solde vers un autre compte

Déplacez 50 unités du compte opérateur vers `$RECEIVER_ACCOUNT` :

```sh
iroha --config defaults/client.toml asset transfer \
  --id coffee#wonderland##${ADMIN_ACCOUNT} \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Sauvegardez le hash de transaction comme `$TRANSFER_HASH`. Interrogez les avoirs des deux comptes
pour vérifier les nouveaux soldes :

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${ADMIN_ACCOUNT}\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${RECEIVER_ACCOUNT}\"}" --limit 1 | jq .
```

## 5. Vérifier les preuves du ledger

Utilisez les hashes sauvegardés pour confirmer que les deux transactions ont été committées :

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Vous pouvez aussi streamer les blocs récents pour voir quel bloc a inclus le transfert :

```sh
# Stream depuis le dernier bloc et arrêtez après ~5 secondes
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Toutes les commandes ci-dessus utilisent les mêmes payloads Norito que les SDK. Si vous
reproduisez ce flux via du code (voir les quickstarts SDK ci-dessous), les hashes et
soldes seront alignés tant que vous ciblez le même réseau et les mêmes defaults.

## Liens de parité SDK

- [Rust SDK quickstart](../sdks/rust) — montre l'enregistrement d'instructions,
  la soumission de transactions et le polling de statut depuis Rust.
- [Python SDK quickstart](../sdks/python) — montre les mêmes opérations register/mint
  avec des helpers JSON adossés à Norito.
- [JavaScript SDK quickstart](../sdks/javascript) — couvre les requêtes Torii,
  les helpers de gouvernance et les wrappers de requêtes typées.

Exécutez d'abord le walkthrough CLI, puis répétez le scénario avec votre SDK
préféré pour vous assurer que les deux surfaces s'accordent sur les hashes de
transactions, les soldes et les résultats de requêtes.
