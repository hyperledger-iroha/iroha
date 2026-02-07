---
lang: es
direction: ltr
source: docs/source/agents/env_var_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9ce6010594e495116c1397b984000d1ee5d45d064294eca046f8dc762fa73b6
source_last_modified: "2026-01-04T10:50:53.607349+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Env → Configurar el rastreador de migración

Este rastreador resume los cambios de variables del entorno de producción que surgieron
por `docs/source/agents/env_var_inventory.{json,md}` y la migración prevista
ruta a `iroha_config` (o alcance explícito solo para desarrollo/prueba).


Nota: `ci/check_env_config_surface.sh` ahora falla cuando se crea un nuevo entorno de **producción**
aparecen cuñas en relación con `AGENTS_BASE_REF` a menos que `ENV_CONFIG_GUARD_ALLOW=1` sea
conjunto; documente las adiciones intencionales aquí antes de usar la anulación.

## Migraciones completadas- **IVM Exclusión voluntaria de ABI** — Se eliminó `IVM_ALLOW_NON_V1_ABI`; el compilador ahora rechaza
  ABI que no son v1 incondicionalmente con una prueba unitaria que protege la ruta del error.
- **IVM corrección de entorno del banner de depuración**: se eliminó la opción de exclusión del entorno `IVM_SUPPRESS_BANNER`;
  la supresión de banners sigue estando disponible a través del configurador programático.
- **IVM caché/dimensionamiento** — Caché subproceso/prover/tamaño de GPU mediante
  `iroha_config` (`pipeline.{cache_size,ivm_cache_max_decoded_ops,ivm_cache_max_bytes,ivm_prover_threads}`,
  `accel.max_gpus`) y se eliminaron las correcciones de entorno de tiempo de ejecución. Los anfitriones ahora llaman
  `ivm::ivm_cache::configure_limits` y `ivm::zk::set_prover_threads`, uso de pruebas
  `CacheLimitsGuard` en lugar de anulaciones de entorno.
- **Conectar raíz de cola**: se agregó `connect.queue.root` (predeterminado:
  `~/.iroha/connect`) a la configuración del cliente y lo pasó a través de la CLI y
  Diagnóstico JS. Los ayudantes de JS resuelven la configuración (o un `rootDir` explícito) y
  solo respete `IROHA_CONNECT_QUEUE_ROOT` en desarrollo/prueba a través de `allowEnvOverride`;
  Las plantillas documentan la perilla para que los operadores ya no necesiten anulaciones de entorno.
- **Izanami suscripción de red**: se agregó un indicador CLI/config `allow_net` explícito para
  la herramienta de caos Izanami; las ejecuciones ahora requieren `allow_net=true`/`--allow-net` y
- **Bip de banner IVM**: se reemplazó la cuña de entorno `IROHA_BEEP` por una basada en configuración.
  `ivm.banner.{show,beep}` alterna (predeterminado: verdadero/verdadero). Banner/pitido de inicio
  el cableado ahora lee la configuración sólo en producción; Las compilaciones de desarrollo/prueba siguen siendo honradas.
  la anulación de entorno para cambios manuales.
- **Anulación de carrete DA (solo pruebas)** — La anulación `IROHA_DA_SPOOL_DIR` ahora está
  vallado detrás de los ayudantes `cfg(test)`; El código de producción siempre obtiene el spool.
  ruta desde la configuración.
- **Cripto intrínsecos** — Reemplazado `IROHA_DISABLE_SM_INTRINSICS` /
  `IROHA_ENABLE_SM_INTRINSICS` con el controlador de configuración
  Política `crypto.sm_intrinsics` (`auto`/`force-enable`/`force-disable`) y
  Se quitó la protección `IROHA_SM_OPENSSL_PREVIEW`. Los anfitriones aplican la política en
  inicio, los bancos/pruebas pueden optar por registrarse a través de `CRYPTO_SM_INTRINSICS` y OpenSSL
  La vista previa ahora respeta solo el indicador de configuración.
  Izanami ya requiere `--allow-net`/configuración persistente, y las pruebas ahora dependen de
  esa perilla en lugar de alternar el entorno ambiental.
- **Ajuste FastPQ GPU** — Añadido `fastpq.metal.{max_in_flight,threadgroup_width,metal_trace,metal_debug_enum,metal_debug_fused}`
  perillas de configuración (predeterminadas: `None`/`None`/`false`/`false`/`false`) y pasarlas por el análisis CLI
  Las cuñas `FASTPQ_METAL_*` / `FASTPQ_DEBUG_*` ahora se comportan como respaldos de desarrollo/prueba y
  se ignoran una vez que se carga la configuración (incluso cuando la configuración los deja sin configurar); documentos/inventario fueron
  actualizado para marcar la migración.【crates/irohad/src/main.rs:2609】【crates/iroha_core/src/fastpq/lane.rs:109】【crates/fastpq_prover/src/overrides.rs:11】
  (`IVM_DECODE_TRACE`, `IVM_DEBUG_WSV`, `IVM_DEBUG_COMPACT`, `IVM_DEBUG_INVALID`,
  `IVM_DEBUG_REGALLOC`, `IVM_DEBUG_METAL_ENUM`, `IVM_DEBUG_METAL_SELFTEST`,
  `IVM_FORCE_METAL_ENUM`, `IVM_FORCE_METAL_SELFTEST_FAIL`, `IVM_FORCE_CUDA_SELFTEST_FAIL`,
  `IVM_DISABLE_METAL`, `IVM_DISABLE_CUDA`) ahora están protegidos detrás de compilaciones de depuración/prueba a través de un compartido
  ayudante para que los binarios de producción los ignoren y al mismo tiempo conserven las perillas para el diagnóstico local. sobre
  El inventario se regeneró para reflejar el alcance de desarrollo/prueba únicamente.- **Actualizaciones de dispositivos FASTPQ** — `FASTPQ_UPDATE_FIXTURES` ahora aparece solo en la integración FASTPQ
  pruebas; Las fuentes de producción ya no leen el cambio de entorno y el inventario refleja la opción de solo prueba.
  alcance.
- **Actualización de inventario + detección de alcance**: las herramientas de inventario env ahora etiquetan archivos `build.rs` como
  construye el alcance y rastrea los módulos de arnés de integración/`#[cfg(test)]` para que los cambios de solo prueba (p. ej.,
  `IROHA_TEST_*`, `IROHA_RUN_IGNORED`) y los indicadores de compilación CUDA aparecen fuera del recuento de producción.
  El inventario se regeneró el 7 de diciembre de 2025 (518 referencias / 144 vars) para mantener la diferencia de protección de env-config en verde.
- **Protección de liberación de cuña de entorno de topología P2P** — `IROHA_P2P_TOPOLOGY_UPDATE_MS` ahora activa un determinista
  error de inicio en las versiones de lanzamiento (solo advertencia en depuración/prueba), por lo que los nodos de producción dependen únicamente de
  `network.peer_gossip_period_ms`. El inventario ambiental fue regenerado para reflejar el guardia y el
  El clasificador actualizado ahora abarca los conmutadores protegidos por `cfg!` como depuración/prueba.

## Migraciones de alta prioridad (rutas de producción)

- _Ninguno (inventario actualizado con cfg!/detección de depuración; protección de env-config verde después del endurecimiento de compensación P2P)._

## Solo desarrollo/prueba cambia a valla

- Barrido actual (7 de diciembre de 2025): los indicadores CUDA de solo compilación (`IVM_CUDA_*`) tienen el alcance `build` y el
  Los conmutadores de arnés (`IROHA_TEST_*`, `IROHA_RUN_IGNORED`, `IROHA_SKIP_BIND_CHECKS`) ahora se registran como
  `test`/`debug` en el inventario (incluidas las cuñas protegidas `cfg!`). No se requieren cercas adicionales;
  mantenga las adiciones futuras detrás de `cfg(test)`/ayudantes solo de banco con marcadores TODO cuando las calzas sean temporales.

## Envs de tiempo de compilación (dejar como está)

- Entornos de carga/funciones (`CARGO_*`, `OUT_DIR`, `DOCS_RS`, `PROFILE`, `CUDA_HOME`,
  `CUDA_PATH`, `JSONSTAGE1_CUDA_ARCH`, `FASTPQ_SKIP_GPU_BUILD`, etc.) permanecen
  preocupaciones sobre el script de compilación y están fuera del alcance de la migración de la configuración del tiempo de ejecución.

## Próximas acciones

1) Ejecute `make check-env-config-surface` después de las actualizaciones de config-surface para detectar nuevas correcciones de entorno de producción
   temprano y asignar propietarios de subsistemas/ETA.  
2) Actualice el inventario (`make check-env-config-surface`) después de cada barrido para
   el rastreador permanece alineado con las nuevas barandillas y el diferencial de protección env-config permanece libre de ruido.