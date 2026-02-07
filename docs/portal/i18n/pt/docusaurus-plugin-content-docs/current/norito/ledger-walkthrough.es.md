---
lang: pt
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Recorrido del libro mayor
description: Reproduza um fluxo determinista de registro -> mint -> transfira com o CLI `iroha` e verifique o estado resultante do razão.
slug: /norito/ledger-passo a passo
---

Este retorno complementa o [início rápido de Norito](./quickstart.md) mostrando como alterar e inspecionar o estado do razão com o CLI `iroha`. Registra uma nova definição de ativo, conhece unidades na conta do operador por defeito, transfere parte do saldo para outra conta e verifica as transações e tenências resultantes. Cada passo reflete os fluxos criados nos inícios rápidos do SDK de Rust/Python/JavaScript para que você possa confirmar a paridade entre CLI e SDK.

## Requisitos anteriores

- Siga o [início rápido](./quickstart.md) para iniciar a rede de um único peer via
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Certifique-se de que `iroha` (o CLI) foi compilado ou baixado e que você pode
  Alcance o par usando `defaults/client.toml`.
- Helpers opcionais: `jq` (formato de respostas JSON) e um shell POSIX para
  os trechos de variáveis de ambiente usados abaixo.

Ao longo da guia, substitua `$ADMIN_ACCOUNT` e `$RECEIVER_ACCOUNT` com os
IDs de conta que os planos usam. O pacote por defeito inclui duas contas
Demonstração de Derivadas das Claves:

```sh
export ADMIN_ACCOUNT="ih58..."
export RECEIVER_ACCOUNT="ih58..."
```

Confirme os valores listando as primeiras contas:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Inspeção do estado de gênese

Empieza explorando o razão ao qual a CLI é acessada:

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

Esses comandos são baseados em respostas respaldadas por Norito, porque o filtrado e a paginação são deterministas e coincidem com o que recebe o SDK.

## 2. Registre uma definição de ativo

Crie um novo ativo infinitamente acunável chamado `coffee` dentro do domínio
`wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id coffee#wonderland
```

A CLI imprime o hash da transação enviada (por exemplo, `0x5f...`). Guarde-o para consultar o estado mais tarde.

## 3. Acuna unidades na conta do operador

As quantidades de ativos vivem abaixo do par `(asset definition, account)`. Acuna
250 unidades de `coffee#wonderland` em `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id coffee#wonderland##${ADMIN_ACCOUNT} \
  --quantity 250
```

De novo, capture o hash da transação (`$MINT_HASH`) da saída da CLI. Pará
verifique o saldo, execute:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

o, para apontar apenas para o novo ativo:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${ADMIN_ACCOUNT}\"}" \
  --limit 1 | jq .
```

## 4. Transfira parte do saldo para outra conta

Mueve 50 unidades da conta do operador para `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id coffee#wonderland##${ADMIN_ACCOUNT} \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Guarde o hash da transação como `$TRANSFER_HASH`. Consulte as participações em ambas
contas para verificar os novos saldos:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${ADMIN_ACCOUNT}\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"coffee#wonderland##${RECEIVER_ACCOUNT}\"}" --limit 1 | jq .
```

## 5. Verifique a evidência do razão

Use os hashes salvos para confirmar que ambas as transações são confirmadas:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Você também pode transmitir bloqueios recentes para verificar se o bloqueio inclui a transferência:

```sh
# Stream desde el ultimo bloque y detente despues de ~5 segundos
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```Cada comando anterior usa os mesmos payloads Norito que o SDK. Réplicas de Si
este fluxo através do código (ver quickstarts do SDK abaixo), os hashes e os saldos
coincidiu sempre que apuntes a la misma red y defaults.

## Links de paridade com SDK

- [Início rápido do Rust SDK](../sdks/rust) - demonstração como instruções do registrador,
  enviar transações e consultar o estado de Rust.
- [Início rápido do Python SDK](../sdks/python) - mostra as operações incorretas de registro/mint
  com ajudantes JSON respaldados por Norito.
- [início rápido do SDK JavaScript](../sdks/javascript) - cubre solicitudes Torii,
  ajudantes de governança e wrappers de consultas tipados.

Execute primeiro o retorno da CLI e depois repita o cenário com seu SDK
preferido para garantir que ambas as superfícies coincidem em hashes de transação,
saldos e resultados de consultas.