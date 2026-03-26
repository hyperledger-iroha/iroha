---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/norito/ledger-walkthrough.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9981e42f0a0ebf8dc56390dc8d1b3a3322619a6b0a02767a83cf36ab0c77d53d
source_last_modified: "2026-01-22T06:58:49+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: fr
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Parcours du registre
description: Reproduisez un flux deterministe register -> mint -> transfer avec le CLI `iroha` et verifiez l'etat du ledger resultant.
slug: /norito/ledger-walkthrough
---

Ce parcours complete le [quickstart Norito](./quickstart.md) en montrant comment modifier et inspecter l'etat du ledger avec le CLI `iroha`. Vous enregistrerez une nouvelle definition d'actif, minterez des unites sur le compte operateur par defaut, transfererez une partie du solde vers un autre compte et verifierez les transactions et avoirs resultants. Chaque etape reflete les flux couverts par les quickstarts SDK Rust/Python/JavaScript afin de confirmer la parite entre le CLI et le comportement des SDK.

## Prerequis

- Suivez le [quickstart](./quickstart.md) pour demarrer le reseau mono-pair via
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Assurez-vous que `iroha` (le CLI) est construit ou telecharge et que vous pouvez
  joindre le peer avec `defaults/client.toml`.
- Outils optionnels : `jq` (formatage des reponses JSON) et un shell POSIX pour les
  snippets de variables d'environnement ci-dessous.

Tout au long du guide, remplacez `$ADMIN_ACCOUNT` et `$RECEIVER_ACCOUNT` par les
IDs de compte que vous comptez utiliser. Le bundle par defaut inclut deja deux comptes
issus des cles de demo :

```sh
export ADMIN_ACCOUNT="<katakana-i105-account-id>"
export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
```

Confirmez les valeurs en listant les premiers comptes :

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Inspecter l'etat genesis

Commencez par explorer le ledger cible par le CLI :

```sh
# Domains enregistres en genesis
iroha --config defaults/client.toml domain list all --table

# Accounts dans wonderland (remplacez --limit par un nombre plus eleve si besoin)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions qui existent deja
iroha --config defaults/client.toml asset definition list all --table
```

Ces commandes reposent sur des reponses Norito, donc le filtrage et la pagination sont
 deterministes et correspondent a ce que recoivent les SDK.

## 2. Enregistrer une definition d'actif

Creez un nouvel actif infiniment mintable appele `coffee` dans le domaine
`wonderland` :

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

Le CLI affiche le hash de transaction soumis (par exemple, `0x5f...`). Conservez-le
pour consulter le statut plus tard.

## 3. Minter des unites sur le compte operateur

Les quantites d'actifs vivent sous la paire `(asset definition, account)`. Mintez 250
unites de `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` dans `$ADMIN_ACCOUNT` :

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Encore une fois, recuperez le hash de transaction (`$MINT_HASH`) depuis la sortie du CLI. Pour
verifier le solde, executez :

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

ou, pour cibler seulement le nouvel actif :

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Transferer une partie du solde vers un autre compte

Deplacez 50 unites du compte operateur vers `$RECEIVER_ACCOUNT` :

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Sauvegardez le hash de transaction comme `$TRANSFER_HASH`. Interrogez les avoirs des deux comptes
pour verifier les nouveaux soldes :

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Verifier les preuves du ledger

Utilisez les hashes sauvegardes pour confirmer que les deux transactions ont ete committees :

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Vous pouvez aussi streamer les blocs recents pour voir quel bloc a inclus le transfert :

```sh
# Stream depuis le dernier bloc et arretez apres ~5 secondes
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Toutes les commandes ci-dessus utilisent les memes payloads Norito que les SDK. Si vous
reproduisez ce flux via du code (voir les quickstarts SDK ci-dessous), les hashes et
soldes seront alignes tant que vous ciblez le meme reseau et les memes defaults.

## Liens de parite SDK

- [Rust SDK quickstart](../sdks/rust) - montre l'enregistrement d'instructions,
  la soumission de transactions et le polling de statut depuis Rust.
- [Python SDK quickstart](../sdks/python) - montre les memes operations register/mint
  avec des helpers JSON adosses a Norito.
- [JavaScript SDK quickstart](../sdks/javascript) - couvre les requetes Torii,
  les helpers de gouvernance et les wrappers de requetes typees.

Executez d'abord le walkthrough CLI, puis repetez le scenario avec votre SDK
prefere pour vous assurer que les deux surfaces s'accordent sur les hashes de
transactions, les soldes et les resultats de requetes.
