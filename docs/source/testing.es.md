---
lang: es
direction: ltr
source: docs/source/testing.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7d9bce40727d178bcc7d780c608d82bcd14b0814a7b537cbe9c39a539a200c8
source_last_modified: "2025-12-19T22:31:17.718007+00:00"
translation_last_reviewed: 2026-01-01
---

# Guia de pruebas y solucion de problemas

Esta guia explica como reproducir escenarios de integracion, que infraestructura debe estar en linea y como recopilar logs accionables. Consulte el [informe de estado](../../status.md) del proyecto antes de empezar para saber que componentes estan actualmente en verde.

## Pasos de reproduccion

### Pruebas de integracion (`integration_tests` crate)

1. Asegure que las dependencias del workspace esten compiladas: `cargo build --workspace`.
2. Ejecute la suite de pruebas de integracion con logs completos: `cargo test -p integration_tests -- --nocapture`.
3. Si necesita reejecutar un escenario especifico, use su ruta de modulo, p. ej. `cargo test -p integration_tests settlement::happy_path -- --nocapture`.
4. Capture fixtures serializadas con Norito para garantizar entradas consistentes entre nodos:
   ```rust
   use norito::json;

   let genesis_payload = json::to_string_pretty(&json::json!({
       "chain" : "testnet",
       "peers" : ["127.0.0.1:1337"],
       "accounts" : [{
           "id" : "<i105-account-id>",
           "public_key" : "ed0120..."
       }]
   }))?;
   ```
   Guarde el Norito JSON resultante junto a los artefactos de prueba para que los peers puedan reproducir el mismo estado.

### Pruebas del cliente Python (`pytests` directory)

1. Instale los requisitos de Python con `pip install -r pytests/requirements.txt` en un entorno virtual.
2. Exporte las fixtures en formato Norito generadas arriba mediante una ruta compartida o variable de entorno.
3. Ejecute la suite con salida verbosa: `pytest -vv pytests`.
4. Para depuracion dirigida, ejecute `pytest -k "Query" pytests/tests/test_queries.py --log-cli-level=INFO`.

## Puertos y servicios requeridos

Los siguientes servicios deben ser accesibles antes de ejecutar cualquiera de las suites:

- **API HTTP de Torii**: por defecto `127.0.0.1:1337`. Reemplace via `torii.address` en su config (ver `docs/source/references/peer.template.toml`).
- **Notificaciones WebSocket de Torii**: por defecto `127.0.0.1:8080` para suscripciones de cliente usadas por `pytests`.
- **Exportador de telemetria**: por defecto `127.0.0.1:8180`. Las pruebas de integracion esperan que las metricas lleguen aqui para las comprobaciones de salud.
- **PostgreSQL** (cuando esta habilitado): por defecto `127.0.0.1:5432`. Asegure que las credenciales esten alineadas con el perfil de compose en [`defaults/docker-compose.local.yml`](../../defaults/docker-compose.local.yml).

Consulte la [guia de solucion de problemas de telemetria](telemetry.md) si algun endpoint no esta disponible.

### Estabilidad de peers embebidos

`NetworkBuilder::start()` ahora impone una ventana de liveness posterior a genesis de cinco segundos para cada peer embebido. Si un proceso termina durante este periodo de guardia, el builder aborta con un error detallado que apunta a los logs stdout/stderr en cache. En maquinas con recursos limitados puede extender la ventana (en milisegundos) configurando `IROHA_TEST_POST_GENESIS_LIVENESS_MS`; bajarla a `0` deshabilita el guard por completo. Asegure que su entorno deje suficiente margen de CPU durante los primeros segundos de cada suite de integracion para que los peers alcancen el bloque 1 sin activar el watchdog.

## Recoleccion y analisis de logs

Empiece desde un directorio de ejecucion limpio para que los artefactos previos no oculten problemas nuevos. Los scripts abajo recopilan logs en formatos que las herramientas Norito posteriores pueden consumir.

- Use [`scripts/analyze_telemetry.sh`](../../scripts/analyze_telemetry.sh) despues de la ejecucion de pruebas para agregar metricas de nodos en snapshots Norito JSON con marca de tiempo.
- Al investigar problemas de red, ejecute [`scripts/run_iroha_monitor_demo.py`](../../scripts/run_iroha_monitor_demo.py) para transmitir eventos Torii a `monitor_output.norito.json`.
- Los logs de pruebas de integracion se guardan en `integration_tests/target/`; comprimalos con [`scripts/profile_build.sh`](../../scripts/profile_build.sh) para compartirlos con otros equipos.
- Los logs del cliente Python se escriben en `pytests/.pytest_cache`. Exporte estos junto con la telemetria capturada con:
  ```bash
  ./scripts/report_red_team_failures.py --tests pytests --artifacts out/logs
  ```

Reuna un paquete completo (integracion, Python, telemetria) antes de abrir un issue para que los maintainers puedan reproducir las trazas Norito.

## Siguientes pasos

Para listas de verificacion especificas de release, consulte [pipeline](pipeline.md). Si detecta regresiones o fallas, documentelas en el [status tracker](../../status.md) compartido y haga referencia a cualquier entrada relevante de [solucion de problemas de sumeragi](sumeragi.md).
