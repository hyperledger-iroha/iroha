---
lang: fr
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Пошаговый разбор реестра
description : Enregistrez le fichier de dépannage -> menthe -> transfert avec CLI `iroha` et vérifiez cette restauration.
slug : /norito/ledger-walkthrough
---

Cette procédure pas à pas est complétée par [Démarrage rapide Norito] (./quickstart.md), permettant de démarrer et de vérifier la restauration à l'aide de la CLI `iroha`. Si vous enregistrez une nouvelle définition d'activité, modifiez les modifications sur le compte de l'opérateur défolt, avant de payer le solde du médicament. compte et vérification de vos transactions et de vos paiements. Si vous souhaitez découvrir les options disponibles dans le SDK de démarrage rapide Rust/Python/JavaScript, vous pouvez modifier la partie via la CLI et utiliser le SDK.

## Требования

- Cliquez sur [démarrage rapide](./quickstart.md) pour configurer la configuration ici.
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Assurez-vous que `iroha` (CLI) est connecté ou téléchargé, et que vous pouvez télécharger
  peer через `defaults/client.toml`.
- Informations supplémentaires : `jq` (format JSON) et shell POSIX pour
  сниппетов с переменными окружения ниже.

Pour toutes les instructions, indiquez `$ADMIN_ACCOUNT` et `$RECEIVER_ACCOUNT` pour votre choix.
Cartes d'identité. Dans le bundle proposé, il y a un compte contenant un extrait de la démo :

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

Подтвердите значения, выведя первые аккаунты:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Осмотрите состояние la genèse

Accédez au restaurant de sécurité, à partir de la CLI :

```sh
# Domains, зарегистрированные в genesis
iroha --config defaults/client.toml domain list all --table

# Accounts внутри wonderland (увеличьте --limit при необходимости)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions, которые уже существуют
iroha --config defaults/client.toml asset definition list all --table
```Ces commandes fonctionnent pour Norito, puis sur la filtration et la configuration.
Déterminez et complétez ce thème pour utiliser le SDK.

## 2. Enregistrez la définition de l'activité

Ajoutez les nouvelles activités personnalisables `coffee` au domaine `wonderland` :

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

La CLI vous permet de passer automatiquement (par exemple, `0x5f…`). Сохраните его, чтобы
Vous pouvez vérifier le statut.

## 3. Planifiez vos modifications sur le compte de l'opérateur

Le colis est activé pour le `(asset definition, account)`. Замитьте 250
Éditer `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` pour `$ADMIN_ACCOUNT` :

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Veuillez prendre en compte votre transition (`$MINT_HASH`) à partir de votre CLI. Чтобы проверить баланс,
выполните:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

ou alors, que pouvez-vous faire avec de nouvelles activités :

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Prévoyez votre solde sur votre compte personnel

Effectuez 50 données sur le compte de l'opérateur pour `$RECEIVER_ACCOUNT` :

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Vérifiez votre transition vers `$TRANSFER_HASH`. Запросите holdings на обоих аккаунтах,
que faire pour prouver de nouveaux équilibres :

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Vérifiez votre restaurant

Utilisez des ressources humaines pour prendre en charge le processus de transition :

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Vous pouvez alors créer des blocs après avoir vérifié que le bloc est activé avant :

```sh
# Стрим от последнего блока и остановка через ~5 секунд
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```Vous pouvez utiliser les charges utiles Norito, à savoir le SDK. Si vous êtes en position de pouvoir
C'est ce qui se passe dans le code (comme le SDK Quickstarts ci-dessous), qui et les soldes sont fournis pour l'utilisation,
C'est ce que vous avez défini et ce sont les valeurs par défaut.

## Paramètres du SDK partagé

- [Démarrage rapide du SDK Rust](../sdks/rust) — démonstration des instructions d'enregistrement,
  Ouvrir la transition et le statut d'interrogation de Rust.
- [Démarrage rapide du SDK Python](../sdks/python) — vous permet d'utiliser votre registre/mint
  с Assistants JSON soutenus par Norito.
- [Démarrage rapide du SDK JavaScript](../sdks/javascript) — lance Torii,
  aides à la gouvernance et wrappers de requêtes typés.

Vous pouvez utiliser la procédure pas à pas dans la CLI pour accéder au scénario avec le SDK précédent,
Ce qui est important, c'est ce qui est important pour votre transition, votre équilibre et
результатам запросов.