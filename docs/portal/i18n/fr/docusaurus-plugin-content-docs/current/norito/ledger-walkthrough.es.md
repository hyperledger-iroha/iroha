---
lang: fr
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Recorrido del libro mayor
description : Reproduisez un flux déterminé de registre -> menthe -> transfert avec le CLI `iroha` et vérifiez l'état résultant du grand livre.
slug : /norito/ledger-walkthrough
---

Cet enregistrement complète le [initiale rapide de Norito](./quickstart.md) comme suit et inspecte l'état du grand livre avec la CLI `iroha`. Enregistrer une nouvelle définition d'activité, connaître les unités dans le compte de l'opérateur par défaut, transférer une partie du solde à un autre compte et vérifier les transactions et les tenencias résultantes. Chaque fois que vous réfléchissez aux flux inclus dans les démarrages rapides du SDK de Rust/Python/JavaScript, vous pourrez confirmer la parité entre CLI et SDK.

## Conditions préalables

- Suivez le [démarrage rapide](./quickstart.md) pour démarrer le rouge d'un homologue solo via
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Assurez-vous que `iroha` (dans la CLI) est compilé ou téléchargé et que vous pouvez le faire
  alcanzar el peer en utilisant `defaults/client.toml`.
- Helpers optionnels : `jq` (format de réponse JSON) et un shell POSIX pour
  les extraits de variables d'entrée sont utilisés plus bas.

Sur la grande longueur du guide, remplacez `$ADMIN_ACCOUNT` et `$RECEIVER_ACCOUNT` avec les
ID du compte que les avions utilisent. Le bundle par défaut inclut les comptes
Démo des dérivés des touches :

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

Confirmez les valeurs répertoriées dans les premières données :

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```## 1. Inspection de la genèse de l'état

Utilisez l'exploration du grand livre pour indiquer la CLI :

```sh
# Domains registrados en genesis
iroha --config defaults/client.toml domain list all --table

# Accounts dentro de wonderland (reemplaza --limit por un numero mayor si hace falta)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions que ya existen
iroha --config defaults/client.toml asset definition list all --table
```

Ces commandes sont basées sur des réponses apportées par Norito, car le filtrage et la page sont déterminés et coïncident avec ce qui reçoit le SDK.

## 2. Enregistrer une définition d'actif

Créer un nouvel actif infiniment accessible appelé `coffee` à l'intérieur du domaine
`wonderland` :

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

La CLI imprime le hachage de la transaction envoyée (par exemple, `0x5f...`). Guardalo para consultar el estado mas tarde.

## 3. Acuna unidades en la cuenta del operador

Les nombres d’actifs vivent sous le par `(asset definition, account)`. Acuna
250 unités de `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` et `$ADMIN_ACCOUNT` :

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

De nouveau, capturez le hachage de transaction (`$MINT_HASH`) de la sortie de la CLI. Para
vérifier le solde, ejecuta:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

o, pour ajouter seul un nouvel actif :

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Transférer une partie du solde vers un autre compte

Conservez 50 unités du compte de l'opérateur sur `$RECEIVER_ACCOUNT` :

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Gardez le hachage de la transaction comme `$TRANSFER_HASH`. Consulter les holdings et les ambassadeurs
points pour vérifier les nouveaux soldes :

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Vérifier les preuves du grand livre

Utilisez les hachages gardés pour confirmer que les transactions se confirment :

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```Vous pouvez également transmettre des blocs récents pour bloquer le transfert :

```sh
# Stream desde el ultimo bloque y detente despues de ~5 segundos
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Cette commande a été effectuée précédemment avec les mêmes charges utiles Norito que le SDK. Si répliques
ce flux au milieu du code (voir les démarrages rapides du SDK ci-dessous), les hachages et les soldes
coïncidant toujours avec les points rouges et par défaut.

## Liens de parité avec le SDK

- [Démarrage rapide du SDK Rust](../sdks/rust) - voir les instructions du registraire,
  envoyer des transactions et consulter l'état de Rust.
- [Démarrage rapide du SDK Python](../sdks/python) - montre les mêmes opérations de registre/mint
  avec les helpers JSON répondus par Norito.
- [Démarrage rapide du SDK JavaScript](../sdks/javascript) - sollicitudes cubre Torii,
  helpers de gouvernance et wrappers de requêtes tipados.

Exécutez d'abord l'enregistrement de la CLI, puis répétez le scénario avec votre SDK
Il est préférable de garantir que les surfaces utilisées dans les hachages de transaction,
soldes et résultats de consultations.