---
lang: fr
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Procédure pas à pas pour le grand livre
description : Reproduire un flux déterministe de registre -> menthe -> transfert avec la CLI `iroha` et vérifier l'état du grand livre résultant.
slug : /norito/ledger-walkthrough
---

Cette procédure pas à pas complète le [démarrage rapide Norito] (./quickstart.md) pour montrer comment changer et inspecter l'état du grand livre avec la CLI `iroha`. Vous allez enregistrer une nouvelle définition d'activité, créer des unités auprès de l'opérateur responsable, transférer une partie de la vente pour l'autre compte et vérifier les transactions et les avoirs résultants. En passant par les flux liés à nos démarrages rapides du SDK Rust/Python/JavaScript pour confirmer la parité entre CLI et SDK.

## Pré-requis

- Consultez le [quickstart](./quickstart.md) pour démarrer le redémarrage d'un homologue unique via
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Garanta que `iroha` (ou CLI) est déjà compilé ou baixé et que vous avez lu
  accéder au peer en utilisant `defaults/client.toml`.
- Helpers optionnels : `jq` (format de réponse JSON) et un shell POSIX pour
  les extraits de variations de l'environnement utilisé sont abaissés.

Pendant ce temps, remplacez `$ADMIN_ACCOUNT` et `$RECEIVER_ACCOUNT` par les ID de
conta que voce planja usar. Le bundle padrao comprend deux éléments dérivés de ceux-ci
chaves de démo:

```sh
export ADMIN_ACCOUNT="soraカタカナ..."
export RECEIVER_ACCOUNT="soraカタカナ..."
```

Confirmez les valeurs répertoriées comme les premiers contenants :

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Inspection de l'état de genèseVenez explorer le grand livre que la CLI est en train de regarder :

```sh
# Domains registrados no genesis
iroha --config defaults/client.toml domain list all --table

# Accounts dentro de wonderland (substitua --limit por um numero maior se necessario)
iroha --config defaults/client.toml account list filter \
  '{"domain":"wonderland"}' \
  --limit 10 --table

# Asset definitions que ja existem
iroha --config defaults/client.toml asset definition list all --table
```

Ces commandes dépendent des réponses fournies par Norito, puis le filtre et la page sont déterminés et batem comme les SDK reçus.

## 2. Enregistrez une définition d'activité

Crie un nouveau travail infiniment minable chamado `coffee` à l'intérieur du domicile
`wonderland` :

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

La CLI imprime le hachage de la transaction envoyée (par exemple, `0x5f...`). Garde-o para
consulter le statut plus tard.

## 3. Donnez des unités au contact de l'opérateur

As quantidades de activos vivem sob o par `(asset definition, account)`. Neuf 250
unités de `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` dans `$ADMIN_ACCOUNT` :

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

De nouveau, capturez le hachage de la transaction (`$MINT_HASH`) via la CLI. Para
confirmer le saldo, monté :

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

ou, pour regarder apenas ou novo ativo:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Transfira parte do saldo para outra conta

Déplacez 50 unités du contact de l'opérateur pour `$RECEIVER_ACCOUNT` :

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Gardez le hachage de la transaction comme `$TRANSFER_HASH`. Consulter les avoirs en ambas
comme contas pour vérifier les nouveaux saldos :

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Vérifier les preuves dans le grand livre

Utilisez les hachages salvos pour confirmer que les ambas sont des transacoes foram commit :

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Vous pouvez également diffuser des blocs récents pour voir quel bloc inclut un
transfert :

```sh
# Stream a partir do ultimo bloco e pare apos ~5 segundos
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```Chaque commande permet d'utiliser mes charges utiles Norito que les SDK. Se voix répéter
ce flux via codigo (voir les démarrages rapides du SDK abaixo), les hachages et les sauvegardes
vao alinhar dede que voce mire a mesma rede e os mesmos default.

## Liens de parité du SDK

- [Démarrage rapide du SDK Rust](../sdks/rust) - instructions de démonstration du registraire,
  transacoes submétriques et statut de consultant à partir de Rust.
- [Démarrage rapide du SDK Python](../sdks/python) - afficher les opérations de registre/mint
  com helpers JSON répondu par Norito.
- [Démarrage rapide du SDK JavaScript](../sdks/javascript) - requêtes cobre Torii,
  helpers de gouvernance et wrappers de types de requêtes.

J'ai parcouru la procédure pas à pas de la CLI d'abord, après avoir répété le scénario avec le SDK de votre
préférence pour garantir que les deux surfaces concordent avec les hachages de
transacao, saldos et sorties de requête.