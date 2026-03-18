---
lang: pt
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Passo a passo do razão
description: Reproduza um fluxo determinístico de registro -> mint -> transfer com o CLI `iroha` e verifique o estado do ledger resultante.
slug: /norito/ledger-passo a passo
---

Este passo a passo complementa o [Norito quickstart](./quickstart.md) para mostrar como mudar e funcionar o estado do ledger com o CLI `iroha`. Você vai registrar uma nova definição de ativo, mintar unidades na conta do operador padrão, transferir parte do saldo para outra conta e verificar as transações e participações resultantes. Cada passo reflete os fluxos cobertos nos quickstarts do SDK Rust/Python/JavaScript para confirmar a paridade entre CLI e SDK.

## Pré-requisitos

- Siga o [quickstart](./quickstart.md) para iniciar a rede de um peer único via
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Garanta que `iroha` (o CLI) esteja compilado ou baixado e que você encontre
  acessando o peer usando `defaults/client.toml`.
- Helpers contribuem: `jq` (formatação de respostas JSON) e um shell POSIX para
  os trechos de variáveis de ambiente usados abaixo.

Ao longo do guia, substitua `$ADMIN_ACCOUNT` e `$RECEIVER_ACCOUNT` pelos IDs de
conta que você planeja usar. O pacote padrão já inclui duas contas derivadas das
chaves de demonstração:

```sh
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
```

Confirme os valores listados como primeiras contas:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Inspeção do estado de gênese

Comece explorando o razão que a CLI está olhando:

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

Esses comandos dependem de respostas respaldadas por Norito, então o filtro e a paginação são deterministas e batem com o que os SDKs recebem.

## 2. Registre uma definição de ativo

Crie um novo ativo infinitamente mintable chamado `coffee` dentro do domínio
`wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

O CLI imprime o hash da transação enviada (por exemplo, `0x5f...`). Guarde-o para
consultar o status mais tarde.

## 3. Minte unidades na conta do operador

As quantidades de ativos vivem sob o par `(asset definition, account)`. Mente 250
unidades de `coffee#wonderland` em `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

De novo, capture o hash da transacao (`$MINT_HASH`) na saida do CLI. Pará
confirme o saldo, rode:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

ou, para ver apenas o novo ativo:

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

Guarde o hash da transação como `$TRANSFER_HASH`. Consulte as participações em ambas
as contas para verificar os novos saldos:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Verifique as evidências do razão

Use os hashes salvos para confirmar que ambas as transações foram cometidas:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Você também pode fazer stream de blocos recentes para ver qual bloco incluiu a
transferência:

```sh
# Stream a partir do ultimo bloco e pare apos ~5 segundos
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Cada comando acima usa os mesmos payloads Norito que os SDKs. Se você repetir
este fluxo via código (veja os quickstarts do SDK abaixo), os hashes e saldos
vao alinhar desde que você mire a mesma rede e os mesmos defaults.

## Links de paridade do SDK- [Início rápido do Rust SDK](../sdks/rust) - demonstra instruções do registrador,
  submeter transações e consultar status a partir de Rust.
- [Python SDK quickstart](../sdks/python) - mostra as mesmas operações Register/mint
  com helpers JSON respaldados por Norito.
- [início rápido do SDK JavaScript](../sdks/javascript) - solicitações de cobre Torii,
  helpers de governança e wrappers de consulta digitados.

Rode o walkthrough do CLI primeiro, depois repita o cenário com o SDK de sua
preferência para garantir que as duas superfícies concordem em hashes de
transação, saldos e saídas de consulta.