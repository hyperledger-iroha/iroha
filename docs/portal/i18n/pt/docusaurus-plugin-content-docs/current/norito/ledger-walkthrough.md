---
lang: pt
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Walkthrough do ledger
description: Reproduza um fluxo deterministico de register -> mint -> transfer com o CLI `iroha` e verifique o estado do ledger resultante.
slug: /norito/ledger-walkthrough
---

Este walkthrough complementa o [Norito quickstart](./quickstart.md) ao mostrar como mutar e inspecionar o estado do ledger com o CLI `iroha`. Voce vai registrar uma nova definicao de ativo, mintar unidades na conta de operador padrao, transferir parte do saldo para outra conta e verificar as transacoes e holdings resultantes. Cada passo espelha os fluxos cobertos nos quickstarts de SDK Rust/Python/JavaScript para confirmar a paridade entre CLI e SDK.

## Pre-requisitos

- Siga o [quickstart](./quickstart.md) para iniciar a rede de um unico peer via
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Garanta que `iroha` (o CLI) esteja compilado ou baixado e que voce consiga
  acessar o peer usando `defaults/client.toml`.
- Helpers opcionais: `jq` (formatacao de respostas JSON) e um shell POSIX para
  os snippets de variaveis de ambiente usados abaixo.

Ao longo do guia, substitua `$ADMIN_ACCOUNT` e `$RECEIVER_ACCOUNT` pelos IDs de
conta que voce planeja usar. O bundle padrao ja inclui duas contas derivadas das
chaves de demo:

```sh
export ADMIN_ACCOUNT="<katakana-i105-account-id>"
export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
```

Confirme os valores listando as primeiras contas:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Inspecione o estado de genesis

Comece explorando o ledger que o CLI esta mirando:

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

Esses comandos dependem de respostas respaldadas por Norito, entao o filtro e a paginacao sao deterministas e batem com o que os SDKs recebem.

## 2. Registre uma definicao de ativo

Crie um novo ativo infinitamente mintable chamado `coffee` dentro do dominio
`wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

O CLI imprime o hash da transacao enviada (por exemplo, `0x5f...`). Guarde-o para
consultar o status mais tarde.

## 3. Minte unidades na conta do operador

As quantidades de ativos vivem sob o par `(asset definition, account)`. Minte 250
unidades de `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` em `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

De novo, capture o hash da transacao (`$MINT_HASH`) na saida do CLI. Para
confirmar o saldo, rode:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

ou, para mirar apenas o novo ativo:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Transfira parte do saldo para outra conta

Mova 50 unidades da conta do operador para `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Guarde o hash da transacao como `$TRANSFER_HASH`. Consulte os holdings em ambas
as contas para verificar os novos saldos:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Verifique as evidencias do ledger

Use os hashes salvos para confirmar que ambas as transacoes foram committed:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Voce tambem pode fazer stream de blocos recentes para ver qual bloco incluiu a
transferencia:

```sh
# Stream a partir do ultimo bloco e pare apos ~5 segundos
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Cada comando acima usa os mesmos payloads Norito que os SDKs. Se voce repetir
este fluxo via codigo (veja os quickstarts de SDK abaixo), os hashes e saldos
vao alinhar desde que voce mire a mesma rede e os mesmos defaults.

## Links de paridade de SDK

- [Rust SDK quickstart](../sdks/rust) - demonstra registrar instrucoes,
  submeter transacoes e consultar status a partir de Rust.
- [Python SDK quickstart](../sdks/python) - mostra as mesmas operacoes register/mint
  com helpers JSON respaldados por Norito.
- [JavaScript SDK quickstart](../sdks/javascript) - cobre requests Torii,
  helpers de governanca e wrappers de query tipados.

Rode o walkthrough do CLI primeiro, depois repita o cenario com o SDK de sua
preferencia para garantir que as duas superficies concordem em hashes de
transacao, saldos e outputs de query.
