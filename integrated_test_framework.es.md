---
lang: es
direction: ltr
source: integrated_test_framework.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ff9e1108802fdd57703749069f87270c4195f4037a32aa65c28cde9a67b63e98
source_last_modified: "2025-11-02T04:40:28.026560+00:00"
translation_last_reviewed: 2026-01-21
---

# Marco de pruebas de integración para Hyperledger Iroha (red de 7 nodos)

## Introducción

Hyperledger Iroha v2 proporciona un conjunto rico de Instrucciones Especiales de Iroha (ISI) para la gestión de dominios, activos, permisos y pares. Este documento especifica un marco de pruebas de integración para una red de 7 pares, con el fin de validar la corrección, el consenso y la consistencia entre pares en condiciones normales y con fallos (tolerando hasta 2 pares defectuosos).

Estas directrices reflejan la reciente migración del esquema de génesis a bloques con múltiples transacciones.

Nota sobre la API: En esta base de código, Torii es una API HTTP/WebSocket (Axum). Las pruebas deben usar endpoints HTTP (comúnmente puertos 8080+). No hay ningún servicio RPC adicional escuchando en 50051.

## Objetivos

- Orquestación automatizada de 7 pares: iniciar y gestionar pares programáticamente para CI rápido (principal), o mediante Docker Compose (opcional).
- Configuración de génesis: partir de una génesis Norito común con cuentas/claves deterministas y permisos requeridos.
- Ejercitar ISI: cubrir sistemáticamente los ISI mediante transacciones y verificar el estado resultante.
- Consistencia entre pares: consultar múltiples pares después de cada paso para asegurar un estado de libro mayor idéntico.
- Comprobaciones de tolerancia a fallos: con hasta 2 pares fuera de línea o particionados, los pares restantes continúan; los pares que regresan se ponen al día sin divergencias.

## Modos de orquestación

- Arnés Rust (recomendado): `crates/iroha_test_network` inicia procesos `irohad` localmente, asigna puertos, escribe configuraciones por ejecución y génesis Norito `.nrt`, monitorea la preparación y las alturas de bloque, y proporciona utilidades de apagado/reinicio.
- Docker Compose (opcional): `crates/iroha_swarm` genera un archivo Compose para N pares. Úsalo cuando se desee redes en contenedor u orquestación externa.

## Configuración de la red de pruebas

**Bloque de génesis:**

- Fuente: `defaults/genesis.json` que ahora agrupa instrucciones en un arreglo `transactions`. Las pruebas pueden anexar instrucciones con `NetworkBuilder::with_genesis_instruction` y comenzar una nueva transacción con `.next_genesis_transaction()`. El bloque resultante se serializa a Norito `.nrt`.
- Topología: Se almacena en la primera transacción (`transactions[0].topology`) e incluye los 7 pares (clave pública + dirección), de modo que cada par conoce la red desde el inicio.
- Cuentas/Permisos: Prefiere cuentas estandarizadas de `crates/iroha_test_samples` (`ALICE_ID`, `BOB_ID`, `SAMPLE_GENESIS_ACCOUNT_KEYPAIR`) con concesiones explícitas para escenarios de prueba, p. ej. `CanManagePeers`, `CanManageRoles`, `CanMintAssetWithDefinition`.
- Estrategia de inyección: Con el arnés, la génesis normalmente se proporciona a un par (el “emisor de génesis”); otros pares se ponen al día vía sincronización de bloques. Con Compose, apunta todos los pares al mismo path de génesis.

**Red y puertos:**

- API HTTP Torii: `API_ADDRESS` (Axum). Para Compose, mapea `8080`–`8086` para 7 pares al host. El arnés auto‑asigna puertos de loopback.
- P2P: La dirección peer‑to‑peer interna se configura en `trusted_peers` y se propaga por gossip. El arnés auto‑configura `trusted_peers` por ejecución.

**Almacenamiento de datos:**

- Kura: almacenamiento de bloques de Iroha v2 (no se necesita contenedor RocksDB/Postgres). Configura vía `[kura]` (p. ej., `store_dir`). Desactiva snapshots para pruebas vía `[snapshot]`.

**Flujo del arnés:**

1) Construir red: `NetworkBuilder::new().with_peers(7)`; opcionalmente `.with_pipeline_time(...)` y `.with_config_layer(...)` para overrides; elegir combustible de IVM con `IvmFuelConfig`.
2) Iniciar pares: `.start()` o `.start_blocking()`; escribe capas de configuración, establece `trusted_peers`, inyecta génesis para un par y espera la preparación.
3) Preparación: `Network::ensure_blocks(height)` o `once_blocks_sync(...)` asegura que las alturas de bloque no vacías alcancen las expectativas en todos los pares. Alternativamente, consulta `Client::get_status()`.

**Flujo Docker Compose (opcional):**

1) Generar compose: Usa `iroha_swarm::Swarm` para emitir un archivo compose con N pares. Mapea puertos API al host y establece variables de entorno (CHAIN, claves, TRUSTED_PEERS, GENESIS).
2) Iniciar: `docker compose up`.
3) Preparación: Sondea endpoints HTTP de Torii hasta que el estado sea saludable y la altura de bloque ≥ 1.

## Implementación del arnés de pruebas (Rust)

**Cliente y transacciones:**

- Usa `iroha::client::Client` (HTTP/WebSocket) para enviar transacciones y consultas a Torii.
- Construye transacciones a partir de secuencias de ISI `InstructionBox` o bytecode IVM; firma con valores deterministas de `KeyPair` de `iroha_test_samples`.
- Llamadas útiles: `submit_blocking`, `submit_all_blocking`, `query(...).execute()/execute_all()`, `get_status()`, y flujos de bloques y eventos por WebSocket.

**Consistencia entre pares:**

- Después de cada operación, consulta cada par en ejecución (`Network::peers()` → `peer.client()`) y compara resultados (p. ej., balances, definiciones, listas de pares). Esto asegura comprobaciones de consistencia fuertes más allá de la verificación de un solo par.

**Inyección de fallos (sin contenedores):**

- Usa las utilidades de relay/proxy en `integration_tests/tests/extra_functional/unstable_network.rs` para reescribir `trusted_peers` a proxies TCP y suspender enlaces selectivamente. Esto habilita particiones, pérdidas y reconexiones dirigidas.

**Requisitos previos de IVM:**

- Algunas pruebas requieren muestras de IVM preconstruidas; asegúrate de que `crates/ivm/target/prebuilt/build_config.toml` exista. Cuando falte, las pruebas pueden omitirse (las pruebas de integración actuales hacen este check).

## Esquema de escenario de 7 nodos

1) Inicia una red de 7 pares (arnés o Compose) y espera la confirmación de génesis en todos los pares.
2) Ejecuta una suite de ISI:
   - Registrar dominios/cuentas/activos; otorgar/revocar permisos; acuñar/quemar/transferir activos; establecer/eliminar clave‑valor; registrar triggers; actualizar el executor.
3) Después de cada paso lógico, ejecuta consultas entre pares para verificar estado idéntico.
4) Detén 1–2 pares, continúa enviando transacciones con los 5 pares restantes; asegura progreso y consistencia entre los pares en ejecución.
5) Reinicia los pares detenidos y verifica la puesta al día y la igualdad entre pares.

## Uso en CI

- Build: `cargo build --workspace`
- Preconstruir muestras IVM (según requieran las pruebas)
- Test: `cargo test --workspace`
- Lint más estricto (opcional): `cargo clippy --workspace --all-targets -- -D warnings`

## Conclusión

Este marco aprovecha el arnés Rust del repositorio (`iroha_test_network`) y el cliente basado en HTTP (`iroha::client::Client`) para validar una red Iroha v2 de 7 pares. Enfatiza la consistencia entre pares, escenarios de fallos realistas y una configuración/teardown repetibles adecuadas para CI. Docker Compose vía `iroha_swarm` está disponible cuando se prefiere la contenedorización.

## Explorer

- Cualquier explorador de blockchain que hable Torii HTTP/WebSocket puede conectarse a cada par de forma independiente. Cada par expone un endpoint Torii (host:puerto) adecuado para estado, bloques, consultas y eventos.
- Con el arnés Rust: después de construir una red puedes derivar las URLs de todos los pares:

  ```rust
  use iroha_test_network::NetworkBuilder;

  let network = NetworkBuilder::new().with_peers(7).build();
  let urls = network.torii_urls();
  // p. ej. ["http://127.0.0.1:8080", ..., "http://127.0.0.1:8086"]
  ```

  También hay helpers por par: `peer.api_address()` y `peer.torii_url()`.

- Con Docker Compose (`iroha_swarm`): genera un compose de 7 pares y mapea `8080..8086` al host; apunta tu explorador a cada dirección. Si tu explorador soporta múltiples endpoints, configura los 7; de lo contrario ejecuta una instancia por par.
