---
lang: es
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Recorrido del libro mayor
descripción: Reproduzca un flujo determinista de registro -> mint -> transfer con el CLI `iroha` y verifique el estado resultante del libro mayor.
babosa: /norito/ledger-walkthrough
---

Este recorrido complementa el [inicio rápido de Norito](./quickstart.md) mostrando como mutar e inspeccionar el estado del libro mayor con el CLI `iroha`. Registraras una nueva definición de activo, acunaras unidades en la cuenta de operador por defecto, transferiras parte del saldo a otra cuenta y verificaras las transacciones y tenencias resultantes. Cada paso refleja los flujos cubiertos en los inicios rápidos de SDK de Rust/Python/JavaScript para que puedas confirmar la paridad entre CLI y SDK.

##Requisitos previos

- Sigue el [quickstart](./quickstart.md) para iniciar la red de un solo peer vía
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Asegurate de que `iroha` (el CLI) este compilado o descargado y que puedas
  alcanzar el peer usando `defaults/client.toml`.
- Helpers opcionales: `jq` (formateo de respuestas JSON) y un shell POSIX para
  Los fragmentos de variables de entorno usados abajo.

A lo largo de la guía, reemplace `$ADMIN_ACCOUNT` y `$RECEIVER_ACCOUNT` con los
ID de cuenta que planeas usar. El paquete por defecto ya incluye dos cuentas
Demostración de Derivadas de las claves:

```sh
export ADMIN_ACCOUNT="<katakana-i105-account-id>"
export RECEIVER_ACCOUNT="<katakana-i105-account-id>"
```

Confirma los valores listando las primeras cuentas:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```## 1. Inspecciona el estado génesis

Empieza explorando el libro mayor al que apunta el CLI:

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

## 2. Registra una definición de activo

Crea un nuevo activo infinitamente acunable llamado `coffee` dentro del dominio
`wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

La CLI imprime el hash de la transacción enviada (por ejemplo, `0x5f...`). Guárdalo para consultar el estado más tarde.

## 3. Acuña unidades en la cuenta del operador

Las cantidades de activos viven bajo el par `(asset definition, account)`. Acuña
250 unidades de `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` y `$ADMIN_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

De nuevo, capture el hash de transacción (`$MINT_HASH`) de la salida del CLI. Párrafo
verificar el saldo, ejecuta:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

o, para apuntar solo al nuevo activo:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Transfiere parte del saldo a otra cuenta

Mueve 50 unidades de la cuenta del operador a `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Guarde el hash de transacción como `$TRANSFER_HASH`. Consulta los holdings en ambas
cuentas para verificar los nuevos saldos:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Verifica la evidencia del libro mayor

Usa los hashes guardados para confirmar que ambas transacciones se confirmaron:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```También puedes transmitir bloques recientes para ver que bloque incluye la transferencia:

```sh
# Stream desde el ultimo bloque y detente despues de ~5 segundos
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Cada comando anterior usa las mismas cargas útiles Norito que los SDK. si replicas
este flujo mediante código (ver inicios rápidos de SDK abajo), los hashes y balances
coincidirán siempre que apuntes a la misma red y defaults.

## Enlaces de paridad con SDK

- [Inicio rápido de Rust SDK](../sdks/rust) - muestra como registrador instrucciones,
  enviar transacciones y consultar estado desde Rust.
- [Inicio rápido de Python SDK](../sdks/python) - muestra las mismas operaciones de Register/mint
  con ayudantes JSON respaldados por Norito.
- [Inicio rápido de JavaScript SDK](../sdks/javascript) - cubre aplicaciones Torii,
  ayudantes de gobernanza y envoltorios de consultas tipados.

Ejecuta primero el recorrido del CLI, luego repite el escenario con tu SDK
preferido para asegurar que ambas superficies concuerdan en hashes de transacción,
saldos y resultados de consultas.