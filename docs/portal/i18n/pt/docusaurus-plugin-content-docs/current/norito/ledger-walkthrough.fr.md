---
lang: pt
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Parcours du registre
description: Reproduza um registro determinado de fluxo -> mint -> transfira com CLI `iroha` e verifique o estado do razão resultante.
slug: /norito/ledger-passo a passo
---

Este trecho completa o [quickstart Norito](./quickstart.md) e monta o modificador de comentários e inspeciona o estado do livro-razão com o CLI `iroha`. Você registrará uma nova definição de ativo, manterá as unidades na conta operacional padrão, transferirá uma parte da venda para outra conta e verificará as transações e os resultados resultantes. Cada etapa reflete o fluxo coberto pelos quickstarts SDK Rust/Python/JavaScript para confirmar a paridade entre a CLI e o comportamento do SDK.

## Pré-requisito

- Execute o [quickstart](./quickstart.md) para iniciar o recurso mono-par via
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Certifique-se de que `iroha` (o CLI) foi construído ou carregado e que você pode
  joindre le peer avec `defaults/client.toml`.
- Opções de utilitários: `jq` (formatação de respostas JSON) e um shell POSIX para os
  trechos de variáveis de ambiente ci-dessous.

Em todo o guia, substitua `$ADMIN_ACCOUNT` e `$RECEIVER_ACCOUNT` por les
IDs de conta que você usará. Le bundle par defaut inclut deja deux comptes
emite as fichas de demonstração:

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

Confirme os valores na lista das principais contas:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Inspetor do Estado Gênesis

Comece pelo explorer do livro-razão pela CLI:

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

Esses comandos são apresentados nas respostas Norito, pois a filtragem e a paginação são
 determina e corresponde ao que recupera o SDK.

## 2. Registrar uma definição de ação

Crie um novo recurso infinito mintable `coffee` no domínio
`wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

A CLI exibe o hash da transação desejada (por exemplo, `0x5f...`). Conserve-o
para consultar o status mais tarde.

## 3. Manter as unidades na operação da conta

As quantidades de atividades vivem sob o par `(asset definition, account)`. Mentez 250
une de `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` a `$ADMIN_ACCOUNT` :

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Mais uma vez, recupere o hash da transação (`$MINT_HASH`) a partir da saída da CLI. Despeje
verificador de solda, executez :

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

ou, para cibler apenas o novo ativo:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Transferir uma parte do dinheiro para outra conta

Coloque 50 unidades do operador da conta para `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Salve o hash da transação como `$TRANSFER_HASH`. Interrogue as informações de duas contas
para verificar as novas vendas:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Verificando as verificações do livro-razão

Use os hashes salvos para confirmar que as duas transações estão em seus comitês:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Você também pode transmitir os blocos recentes para ver esse bloco incluindo a transferência:

```sh
# Stream depuis le dernier bloc et arretez apres ~5 secondes
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```Todos os comandos ci-dessus usam as cargas úteis de memes Norito que o SDK. Se você
reproduza esse fluxo via código (veja os quickstarts SDK ci-dessous), os hashes e
soldes seront alinha-se para que você possa visualizar o meme reseau e os memes defaults.

## Garantias de paridade SDK

- [Início rápido do Rust SDK](../sdks/rust) - monitore o registro de instruções,
  a transmissão de transações e a pesquisa de status desde Rust.
- [Início rápido do SDK do Python] (../sdks/python) - montre os memes Operations Register/mint
  com os auxiliares JSON incorpora um Norito.
- [JavaScript SDK quickstart](../sdks/javascript) - abra os requisitos Torii,
  os ajudantes de governança e os invólucros de receitas digitadas.

Execute primeiro o passo a passo CLI e depois repita o cenário com seu SDK
prefiro garantir que as duas superfícies estejam de acordo com os hashes de
transações, vendas e resultados de solicitações.