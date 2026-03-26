---
lang: es
direction: ltr
source: docs/portal/docs/norito/ledger-walkthrough.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Viajes del registro
descripción: Reproduisez un flux deterministe registre -> mint -> transfer avec le CLI `iroha` y verifique el estado del libro mayor resultante.
babosa: /norito/ledger-walkthrough
---

Este recorrido completa el [inicio rápido Norito](./quickstart.md) montando el modificador de comentarios e inspecciona el estado del libro mayor con la CLI `iroha`. Vous registrez una nueva definición de acción, minterez des unites sur le cuenta operatorur por defecto, transfererez una parte de la soldadura a otra cuenta y verifique las transacciones y avoirs resultantes. Cada etapa refleja los flujos cubiertos por los inicios rápidos SDK Rust/Python/JavaScript para confirmar la partición entre la CLI y el soporte del SDK.

## Requisitos previos

- Suivez le [inicio rápido](./quickstart.md) para iniciar la búsqueda mono-par vía
  `docker compose -f defaults/docker-compose.single.yml up --build`.
- Asegúrese de que `iroha` (el CLI) está construido o telecargado y que puede
  joindre le peer con `defaults/client.toml`.
- Herramientas opcionales: `jq` (formato de respuestas JSON) y un shell POSIX para archivos
  fragmentos de variables de entorno ci-dessous.

Durante toda la guía, reemplace `$ADMIN_ACCOUNT` e `$RECEIVER_ACCOUNT` por les
ID de cuenta que vas a utilizar. El paquete por defecto incluye deja dos cuentas
issus des cles de demo:

```sh
export ADMIN_ACCOUNT="<i105-account-id>"
export RECEIVER_ACCOUNT="<i105-account-id>"
```

Confirme los valores en la lista de las primeras cuentas:

```sh
iroha --config defaults/client.toml account list all --limit 5 --table
```## 1. Inspector del estado génesis

Comience a explorar el libro mayor mediante CLI:

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

Estos comandos responden a las respuestas Norito, donde se filtra y se pagina la paginación.
 Determina y corresponde a este SDK.

## 2. Registrar una definición activa

Creez un nouvel actif infiniment mintable appele `coffee` dans le domaine
`wonderland`:

```sh
iroha --config defaults/client.toml asset definition register \
  --id 7Sp2j6zDvJFnMoscAiMaWbWHRDBZ
```

La CLI muestra el hash de la transacción soumis (por ejemplo, `0x5f...`). Conservaz-le
pour consulter le statut plus tard.

## 3. Minter des unites sur le compte operator

Les quantites d'actifs vivent sous la paire `(asset definition, account)`. Míntez 250
une de `7Sp2j6zDvJFnMoscAiMaWbWHRDBZ` en `$ADMIN_ACCOUNT` :

```sh
iroha --config defaults/client.toml asset mint \
  --id norito:4e52543000000002 \
  --quantity 250
```

Una vez más, recupere el hash de transacción (`$MINT_HASH`) después de la salida de CLI. verter
verificador de soldadura, ejecutar:

```sh
iroha --config defaults/client.toml asset list all --limit 5 --table
```

ou, pour cibler solo le nouvel actif :

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" \
  --limit 1 | jq .
```

## 4. Transferir una parte de la soldadura a otra cuenta

Coloque 50 unidades en la cuenta del operador según `$RECEIVER_ACCOUNT`:

```sh
iroha --config defaults/client.toml asset transfer \
  --id norito:4e52543000000002 \
  --to ${RECEIVER_ACCOUNT} \
  --quantity 50
```

Guarde el hash de transacción como `$TRANSFER_HASH`. Interrogez les avoirs des deux comptes
para verificar las nuevas ventas:

```sh
iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000002\"}" --limit 1 | jq .

iroha --config defaults/client.toml asset list filter \
  "{\"id\":\"norito:4e52543000000003\"}" --limit 1 | jq .
```

## 5. Verificador de las cuentas previas del libro mayor

Utilice los hashes guardados para confirmar las dos transacciones en estos comités:

```sh
iroha --config defaults/client.toml transaction get --hash $MINT_HASH | jq .
iroha --config defaults/client.toml transaction get --hash $TRANSFER_HASH | jq .
```También puedes transmitir los bloques recientes para ver qué bloque incluye la transferencia:

```sh
# Stream depuis le dernier bloc et arretez apres ~5 secondes
iroha --config defaults/client.toml blocks 0 --timeout 5s --table
```

Todos los comandos ci-dessus utilizan las cargas útiles de memes Norito que el SDK. si vous
reproduzca este flujo a través del código (consulte los inicios rápidos del SDK), los hashes y
soldes seront alignes tant que vous ciblez le meme reseau et les memes defaults.

## Gravámenes de paridad SDK

- [Inicio rápido de Rust SDK](../sdks/rust) - reloj de registro de instrucciones,
  la transmisión de transacciones y la encuesta de estado después de Rust.
- [Inicio rápido del SDK de Python](../sdks/python) - registrar operaciones de montre les memes/mint
  Con los ayudantes JSON agrega un Norito.
- [Inicio rápido del SDK de JavaScript](../sdks/javascript): abra las solicitudes Torii,
  les helpers de gouvernance et les wrappers de requetes typees.

Ejecute el tutorial CLI y luego repita el escenario con su SDK
Prefiero asegurarme de que las dos superficies estén acordes con los hashes de
transacciones, les soldes et les resultats de requetes.