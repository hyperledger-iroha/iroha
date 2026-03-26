---
lang: es
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Tutorial del libro mayor
descripción: Reproduza un flujo determinístico de registro -> mint -> transfer con el CLI `iroha` y verifique el estado del libro mayor resultante.
babosa: /norito/ledger-walkthrough
---

Este tutorial complementa el [Norito inicio rápido](./quickstart.md) para mostrar cómo mutar e inspeccionar el estado del libro mayor con CLI `iroha`. Voce vai registrar uma nova definicao de ativo, mintar unidades na conta de operador padrao, transferir parte del saldo para outra conta y verificar as transacoes e holdings resultantes. Cada paso espelha los flujos cubiertos en los inicios rápidos de SDK Rust/Python/JavaScript para confirmar la paridad entre CLI y SDK.

##Requisitos previos

- Siga el [inicio rápido](./quickstart.md) para iniciar la red de un único peer vía
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Garanta que `iroha` (o CLI) esteja compilado ou baixado e que voce consiga
  acceder o peer usando `defaults/client.toml`.
- Ayudantes opcionales: `jq` (formato de respuestas JSON) y un shell POSIX para
  os snippets de variaveis de ambiente usados abaixo.

Durante la guía, sustituya `$ADMIN_ACCOUNT` e `$RECEIVER_ACCOUNT` por los ID de
conta que voce planeja usar. O bundle padrao ja inclui duas contas derivadas das
chaves de demostración:

```sh
export ADMIN_ACCOUNT="<katakana-i105-account-id>"
export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
```

Confirme los valores listados como primeras cuentas:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Inspección del estado de génesisComece explorando el libro mayor que el CLI está mirando:

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

Estos comandos dependen de las respuestas respaldadas por Norito, entre ellas el filtro y la paginación de los deterministas y el batem con los SDK recibidos.

## 2. Registre una definición de activo

Llora un novo ativo infinitamente mintable chamado `coffee` dentro del dominio
`wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

La CLI imprime el hash de la transacción enviada (por ejemplo, `0x5f...`). Guarde-o para
consultar el estado más tarde.

## 3. Minte unidades na contacto do operador

As quantidades de ativos vivenm sob o par `(asset definition, account)`. Moneda 250
unidades de `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` en `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

De novo, capture el hash de transacao (`$MINT_HASH`) en la CLI. Párrafo
confirmar o saldo, montó:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

ou, para mirar apenas el novo activo:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Transfira parte del saldo para otro contacto

Mova 50 unidades da conta do operador para `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Guarde o hash da transacao como `$TRANSFER_HASH`. Consulte os holdings en ambas
como cuentas para verificar los nuevos saldos:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Verifique las evidencias del libro mayor

Use os hashes salvos para confirmar que ambas as transacoes foram commited:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Voce tambem pode fazer stream de blocos recientes para ver cual bloque incluido a
transferencia:

```sh
# Stream a partir do ultimo bloco e pare apos ~5 segundos
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```Cada comando acima usa las cargas útiles Norito que los SDK. Se voce repetir
este flujo a través del código (veja los inicios rápidos del SDK a continuación), los hashes y los saldos
Vao alinhar desde que voce mire a mesma rede y os mesmos defaults.

## Enlaces de paridad de SDK

- [Inicio rápido de Rust SDK] (../sdks/rust): muestra las instrucciones del registrador,
  Transacciones submétricas y consultar estado a partir de Rust.
- [Inicio rápido del SDK de Python](../sdks/python) - mostra as mesmas operacoes registro/mint
  com helpers JSON respaldados por Norito.
- [Inicio rápido del SDK de JavaScript](../sdks/javascript) - cobre solicita Torii,
  ayudantes de gobierno y envoltorios de consulta tipados.

Realicé el tutorial de CLI primero, luego repita el escenario con su SDK
preferencia para garantizar que as dos superficies concordem em hashes de
transacao, saldos y salidas de consulta.