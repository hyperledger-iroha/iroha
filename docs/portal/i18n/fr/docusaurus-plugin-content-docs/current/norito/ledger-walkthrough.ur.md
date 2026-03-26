---
lang: fr
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : لیجر واک تھرو
description : `iroha` CLI est un registre déterministe -> menthe -> transfert. کریں۔
slug : /norito/ledger-walkthrough
---

Voici la procédure pas à pas [démarrage rapide Norito] (./quickstart.md) pour télécharger la CLI `iroha`. لیجر اسٹیٹ کو کیسے بدلیں اور چیک کریں۔ آپ ایک نئی Asset Definition رجسٹر کریں گے، ڈیفالٹ آپریٹر اکاؤنٹ میں unités menthe کریں گے، بیلنس کا Les transactions et les participations sont liées aux transactions et aux avoirs. کریں گے۔ Voici les démarrages rapides du SDK Rust/Python/JavaScript et la parité entre la CLI et le SDK کر سکیں۔

## پیشگی تقاضے

- [démarrage rapide](./quickstart.md) فالو کریں تاکہ سنگل-پیئر نیٹ ورک کو
  `docker compose -f defaults/docker-compose.single.yml up --build` est en cours de réalisation
- Téléchargez `iroha` (CLI) build et téléchargez et téléchargez `defaults/client.toml` pour les pairs.
- Nom du produit : `jq` (réponses JSON et formatage) et le shell POSIX contient des extraits de variables d'environnement et des extraits de variables d'environnement.

Il s'agit d'un `$ADMIN_ACCOUNT` et d'un `$RECEIVER_ACCOUNT` pour les identifiants de compte et les identifiants de compte. ensemble de valeurs par défaut avec 3 clés de démonstration et 3 comptes pour les comptes :

```sh
export ADMIN_ACCOUNT="soraカタカナ..."
export RECEIVER_ACCOUNT="soraカタカナ..."
```

پہلے چند comptes لسٹ کر کے ویلیوز کی تصدیق کریں:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Genèse اسٹیٹ کا معائنہLa CLI est la suivante:

```sh
# genesis میں رجسٹرڈ domains
iroha --config defaults/client.toml domain list all --table

# wonderland کے اندر accounts (ضرورت ہو تو --limit بڑھائیں)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# وہ asset definitions جو پہلے سے موجود ہیں
iroha --config defaults/client.toml asset definition list all --table
```

Il existe des réponses basées sur Norito et des kits de filtrage et de pagination déterministes et des SDK. حاصل کرتے ہیں۔

## 2. définition des actifs رجسٹر کریں

`wonderland` est un actif monnayable `coffee`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

Hachage de transaction soumis par CLI (مثلاً `0x5f…`) Vous avez besoin d'un statut et d'une requête pour obtenir un statut

## 3. آپریٹر اکاؤنٹ میں unités menthe کریں

quantités d'actifs `(asset definition, account)` کے جوڑے کے تحت رہتی ہیں۔ `$ADMIN_ACCOUNT` pour `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` pour 250 unités menthe:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Sortie CLI et hachage de transaction (`$MINT_HASH`) بیلنس دوبارہ چیک کرنے کے لئے چلائیں:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

L'actif de votre actif est le suivant :

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. بیلنس کا کچھ حصہ دوسرے اکاؤنٹ کو ٹرانسفر کریں

Il s'agit d'un produit `$RECEIVER_ACCOUNT` pour 50 unités:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

hachage de transaction comme `$TRANSFER_HASH` dans le fichier de hachage Il s'agit d'une requête sur les avoirs et les soldes des comptes :

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. لیجر ایویڈنس کی تصدیق

Les hachages sont utilisés pour les transactions et les commits et les transactions :

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Il y a des blocs pour le flux et un bloc de transfert pour le bloc de transfert :

```sh
# جدید ترین block سے stream کریں اور ~5 seconds بعد رک جائیں
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```Il s'agit de charges utiles Norito et de SDK. Il s'agit d'un processus de démarrage rapide (démarrages rapides du SDK en cours) et des hachages et des soldes pour aligner Les valeurs par défaut sont les mêmes que les valeurs par défaut et les valeurs par défaut.

## Parité SDK

- [Démarrage rapide du SDK Rust] (../sdks/rust) — Instructions pour Rust pour les transactions soumises et sondage d'état pour les transactions.
- [Démarrage rapide du SDK Python] (../sdks/python) — Assistants JSON pris en charge par Norito pour les opérations de registre/mint.
- [Démarrage rapide du SDK JavaScript] (../sdks/javascript) — Requêtes Torii, aides à la gouvernance, et wrappers de requêtes typés et wrappers de requêtes typés.

Procédure pas à pas de la CLI pour le SDK et le kit de développement logiciel. hachages de transaction, soldes et résultats de requêtes