---
lang: es
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ea0c0b2f750131568e801b5fe583ae46ebddda3ce4f9fb52387725c2e227520
source_last_modified: "2025-11-07T12:25:39.145308+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: Recorrido del libro mayor
description: Reproduce un flujo determinista de register -> mint -> transfer con el CLI `iroha` y verifica el estado resultante del ledger.
slug: /norito/ledger-walkthrough
---

Este recorrido complementa el [inicio rapido de Norito](./quickstart.md) mostrando como mutar e inspeccionar el estado del ledger con el CLI `iroha`. Registraras una nueva definicion de activo, acunaras unidades en la cuenta de operador por defecto, transferiras parte del balance a otra cuenta y verificaras las transacciones y tenencias resultantes. Cada paso refleja los flujos cubiertos en los quickstarts de SDK de Rust/Python/JavaScript para que puedas confirmar la paridad entre CLI y SDK.

## Requisitos previos

- Sigue el [quickstart](./quickstart.md) para iniciar la red de un solo peer via
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Asegurate de que `iroha` (el CLI) este compilado o descargado y que puedas
  alcanzar el peer usando `defaults/client.toml`.
- Helpers opcionales: `jq` (formateo de respuestas JSON) y un shell POSIX para
  los snippets de variables de entorno usados abajo.

A lo largo de la guia, reemplaza `$ADMIN_ACCOUNT` y `$RECEIVER_ACCOUNT` con los
IDs de cuenta que planeas usar. El bundle por defecto ya incluye dos cuentas
Derivadas de las claves demo:

```sh
export ADMIN_ACCOUNT="i105..."
export RECEIVER_ACCOUNT="i105..."
```

Confirma los valores listando las primeras cuentas:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```

## 1. Inspecciona el estado genesis

Empieza explorando el ledger al que apunta el CLI:

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

Estos comandos se basan en respuestas respaldadas por Norito, por lo que el filtrado y la paginacion son deterministas y coinciden con lo que reciben los SDK.

## 2. Registra una definicion de activo

Crea un nuevo activo infinitamente acunable llamado `coffee` dentro del dominio
`wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

El CLI imprime el hash de la transaccion enviada (por ejemplo, `0x5f...`). Guardalo para consultar el estado mas tarde.

## 3. Acuna unidades en la cuenta del operador

Las cantidades de activos viven bajo el par `(asset definition, account)`. Acuna
250 unidades de `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` en `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

De nuevo, captura el hash de transaccion (`$MINT_HASH`) de la salida del CLI. Para
verificar el balance, ejecuta:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

o, para apuntar solo al nuevo activo:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Transfiere parte del balance a otra cuenta

Mueve 50 unidades de la cuenta del operador a `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Guarda el hash de transaccion como `$TRANSFER_HASH`. Consulta los holdings en ambas
cuentas para verificar los nuevos balances:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Verifica la evidencia del ledger

Usa los hashes guardados para confirmar que ambas transacciones se confirmaron:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```

Tambien puedes transmitir bloques recientes para ver que bloque incluyo la transferencia:

```sh
# Stream desde el ultimo bloque y detente despues de ~5 segundos
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Cada comando anterior usa los mismos payloads Norito que los SDK. Si replicas
este flujo mediante codigo (ver quickstarts de SDK abajo), los hashes y balances
coincidiran siempre que apuntes a la misma red y defaults.

## Enlaces de paridad con SDK

- [Rust SDK quickstart](../sdks/rust) - demuestra como registrar instrucciones,
  enviar transacciones y consultar estado desde Rust.
- [Python SDK quickstart](../sdks/python) - muestra las mismas operaciones de register/mint
  con helpers JSON respaldados por Norito.
- [JavaScript SDK quickstart](../sdks/javascript) - cubre solicitudes Torii,
  helpers de gobernanza y wrappers de queries tipados.

Ejecuta primero el recorrido del CLI, luego repite el escenario con tu SDK
preferido para asegurar que ambas superficies concuerdan en hashes de transaccion,
balances y resultados de consultas.
