---
lang: fr
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Parcours du registre
description : Reproduisez un flux déterministe register -> mint -> transfer avec le CLI `iroha` et vérifiez l'état du grand livre résultant.
slug : /norito/ledger-walkthrough
---

Ce parcours complet le [quickstart Norito](./quickstart.md) en affichant comment modifier et inspecter l'état du grand livre avec le CLI `iroha`. Vous enregistrez une nouvelle définition d'actif, mintez des unités sur le compte opérateur par défaut, transférez une partie du solde vers un autre compte et vérifiez les transactions et avoirs résultants. Chaque étape reflète les flux couverts par les quickstarts SDK Rust/Python/JavaScript afin de confirmer la parité entre la CLI et le comportement du SDK.

## Prérequis

- Suivez le [quickstart](./quickstart.md) pour démarrer le réseau mono-paire via
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Assurez-vous que `iroha` (le CLI) est construit ou téléchargé et que vous pouvez
  joindre le peer avec `defaults/client.toml`.
- Outils optionnels : `jq` (formatage des réponses JSON) et un shell POSIX pour les
  extraits de variables d'environnement ci-dessous.

Tout au long du guide, remplacez `$ADMIN_ACCOUNT` et `$RECEIVER_ACCOUNT` par les
ID de compte que vous comptez utiliser. Le bundle par défaut inclut déjà deux comptes
issue des clés de démo :

```sh
export ADMIN_ACCOUNT="soraカタカナ..."
export RECEIVER_ACCOUNT="soraカタカナ..."
```

Confirmez les valeurs en listant les premiers comptes :

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```## 1. Inspecter l'état Genèse

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

Ces commandes reposent sur des réponses Norito, donc le filtrage et la pagination sont
 déterministes et correspondant à ce que recoivent les SDK.

## 2. Enregistrer une définition d'actif

Créez un nouvel actif infiniment mintable appele `coffee` dans le domaine
`wonderland` :

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

La CLI affiche le hash de transaction soumis (par exemple, `0x5f...`). Conservez-le
pour consulter le statut plus tard.

## 3. Minter des unités sur le compte opérateur

Les quantités d'actifs vivent sous la paire `(asset definition, account)`. Mintez 250
unit de `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` dans `$ADMIN_ACCOUNT` :

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Encore une fois, récupérez le hash de transaction (`$MINT_HASH`) depuis la sortie du CLI. Verser
vérifier le solde, exécuter :

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

ou, pour cibler seulement le nouvel actif :

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Transférer une partie du solde vers un autre compte

Déplacez 50 unités du compte opérateur vers `$RECEIVER_ACCOUNT` :

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Sauvegardez le hash de transaction comme `$TRANSFER_HASH`. Interrogez les avoirs des deux comptes
pour vérifier les nouvelles ventes :

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Vérifier les preuves du grand livre

Utilisez les hashes sauvegardes pour confirmer que les deux transactions ont été comités :

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```Vous pouvez aussi streamer les blocs récents pour voir quel bloc a inclus le transfert :

```sh
# Stream depuis le dernier bloc et arretez apres ~5 secondes
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Toutes les commandes ci-dessus utilisent les memes payloads Norito que les SDK. Si vous
reproduisez ce flux via du code (voir les quickstarts SDK ci-dessous), les hashes et
les ventes seront alignées tant que vous ciblez le meme reseau et les memes defaults.

## Liens de parité SDK

- [Rust SDK quickstart](../sdks/rust) - montre l'enregistrement d'instructions,
  la soumission de transactions et le sondage de statut depuis Rust.
- [Démarrage rapide du SDK Python](../sdks/python) - montre les opérations memes registre/mint
  avec des helpers JSON adosse un Norito.
- [JavaScript SDK quickstart](../sdks/javascript) - couvre les requêtes Torii,
  les helpers de gouvernance et les wrappers de requêtes types.

Exécutez d'abord la CLI pas à pas, puis répétez le scénario avec votre SDK
préférez pour vous assurer que les deux surfaces s'accordent sur les hashes de
transactions, les ventes et les résultats de requêtes.