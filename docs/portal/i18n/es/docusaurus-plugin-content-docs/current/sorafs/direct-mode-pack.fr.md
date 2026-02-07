---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/direct-mode-pack.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: paquete de modo directo
título: Paquete de respuesta en modo directo SoraFS (SNNet-5a)
sidebar_label: Modo paquete directo
descripción: Requisitos de configuración, controles de conformidad y etapas de implementación durante la explotación de SoraFS en modo directo Torii/QUIC colgante de la transición SNNet-5a.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/direct_mode_pack.md`. Guarde las dos copias sincronizadas justo al retrato del conjunto histórico Sphinx.
:::

Los circuitos SoraNet mantienen el transporte por defecto para SoraFS, pero el elemento de hoja de ruta **SNNet-5a** exige una respuesta regulada para que los operadores puedan conservar un acceso en lectura determinada mientras termina el despliegue anónimo. Este paquete reúne los botones CLI/SDK, los perfiles de configuración, las pruebas de conformidad y la lista de verificación de implementación necesarias para ejecutar SoraFS en modo directo Torii/QUIC sin tocar auxiliares de transporte de confidencialidad.

La respuesta se aplica a los entornos de puesta en escena y producción regulados según las puertas de preparación del franquiciador SNNet-5 a SNNet-9. Conserve los artefactos ci-dessous con el material de implementación SoraFS habitualmente porque los operadores pueden cambiar entre los modos anónimos y directos a la demanda.

## 1. Banderas CLI y SDK- `sorafs_cli fetch --transport-policy=direct-only ...` desactiva la ordenación de relés e impone los transportes Torii/QUIC. La ayuda de la lista CLI desactivada `direct-only` como valor aceptado.
- El SDK debe definir `OrchestratorConfig::with_transport_policy(TransportPolicy::DirectOnly)` cuando se expone a alternar "modo directo". Los enlaces generados en `iroha::ClientOptions` y `iroha_android` propagan la misma enumeración.
- Los arneses de puerta de enlace (`sorafs_fetch`, enlaces Python) pueden analizar el analizador para alternar solo directamente a través de los asistentes Norito JSON divididos para que la automatización obtenga el mismo comportamiento.

Documente la bandera en los runbooks orientados a partenaires y haga pasar los conmutadores a través de `iroha_config` junto con las variables de entorno.

## 2. Perfiles políticos de la puerta de enlace

Utilice JSON Norito para conservar una configuración determinada por el orquestador. El perfil del ejemplo en `docs/examples/sorafs_direct_mode_policy.json` codifica:

- `transport_policy: "direct_only"`: rechace los proveedores que no anuncian que des transports relais SoraNet.
- `max_providers: 2`: limita los pares dirigidos a los puntos finales auxiliares Torii/QUIC y los más confiables. Ajuste según las restricciones de conformidad regional.
- `telemetry_region: "regulated-eu"` — Etiqueta las métricas emitidas según los paneles de control y las auditorías distinguen las ejecuciones de respuesta.
- Presupuestos de reintento de conservadores (`retry_budget: 2`, `provider_failure_threshold: 3`) para evitar enmascarar las puertas de enlace mal configuradas.Cargue el JSON a través de `sorafs_cli fetch --config` (automatización) o mediante los enlaces SDK (`config_from_json`) antes de exponer la política a los operadores. Mantenga la salida del marcador (`persist_path`) para las pistas de auditoría.

Los ajustes de la aplicación en la puerta de enlace están capturados en `docs/examples/sorafs_gateway_direct_mode.toml`. El modelo refleja la salida de `iroha app sorafs gateway direct-mode enable`, desactiva los cheques de sobre/admisión, cambia los valores por defecto de límite de tasa y reemplaza la tabla `direct_mode` con los nombres de host derivados del plan y los resúmenes de manifiesto. Reemplace los valores de espacio reservado con su plan de implementación antes de versionar el extrait en la gestión de configuración.

## 3. Conjunto de pruebas de conformidad

La preparación del modo directo incluye una cobertura en el orquestador y en las cajas CLI:- `direct_only_policy_rejects_soranet_only_providers` garantiza que `TransportPolicy::DirectOnly` se escuchará rápidamente cuando cada anuncio candidato no prend en charge que les relais SoraNet.【crates/sorafs_orchestrator/src/lib.rs:7238】
- `direct_only_policy_prefers_direct_transports_when_available` garantiza que los transportes Torii/QUIC se utilizan cuando están disponibles y que las relaciones SoraNet son excluyentes de la sesión. 【crates/sorafs_orchestrator/src/lib.rs:7285】
- `direct_mode_policy_example_is_valid` analiza `docs/examples/sorafs_direct_mode_policy.json` para asegurar que la documentación esté alineada con los ayudantes utilitarios. 【crates/sorafs_orchestrator/src/lib.rs:7509】【docs/examples/sorafs_direct_mode_policy.json:1】
- `fetch_command_respects_direct_transports` ejercita `sorafs_cli fetch --transport-policy=direct-only` contra una puerta de enlace Torii simultáneamente, realiza una prueba de humo para los entornos regulados que permiten el transporte directo.【crates/sorafs_car/tests/sorafs_cli.rs:2733】
- `scripts/sorafs_direct_mode_smoke.sh` envuelve el mismo comando con el JSON de política y la persistencia del marcador para la automatización del lanzamiento.

Ejecute la suite ciblée antes de publicar las actualizaciones del día:

```bash
cargo test -p sorafs_orchestrator direct_only_policy
cargo test -p sorafs_car --features cli fetch_command_respects_direct_transports
```

Si la compilación del espacio de trabajo se produce debido a cambios en sentido ascendente, indique el error bloqueado en `status.md` y vuelva a ejecutar una vez la dependencia puesta a punto.

## 4. Ejecuciones de humo automatizadasLa cobertura CLI solo no revela regresiones específicas del medio ambiente (por ejemplo, deriva de puertas de enlace políticas o inadecuaciones de manifiesto). Un ayudante de humo dédié vit dans `scripts/sorafs_direct_mode_smoke.sh` y sobre `sorafs_cli fetch` con la política de orquesta en modo directo, la persistencia del marcador y la captura del currículum.

Ejemplo de utilización:

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=gw-regulated,provider-id=001122...,base-url=https://gw.example/direct/,stream-token=BASE64
```- El script respeta las siguientes banderas CLI y archivos de configuración clave=valor (ver `docs/examples/sorafs_direct_mode_smoke.conf`). Renseignez le digest du manifest et les entrrées d'adverts de proveedores avec des valeurs de production avant l'exécution.
- `--policy` apunta por defecto a `docs/examples/sorafs_direct_mode_policy.json`, pero todo el JSON del producto orquestado por `sorafs_orchestrator::bindings::config_to_json` puede estar disponible. La CLI acepta la política a través de `--orchestrator-config=PATH`, lo que permite ejecutar ejecuciones reproducibles sin ajustar las banderas a la principal.
- Quand `sorafs_cli` n'est pas dans `PATH`, le helper le construit depuis le crate `sorafs_orchestrator` (liberación de perfil) afin que les smokes exercent le plumbing du mode direct livré.
- Salidas :
  - Carga útil ensamblada (`--output`, por defecto `artifacts/sorafs_direct_mode/payload.bin`).
  - Resumen de recuperación (`--summary`, por defecto en la base de carga útil) que contiene la región de télémétrie y las relaciones de proveedores utilizados para la evidencia de implementación.
  - La instantánea del marcador persiste frente al camino declarado en el JSON de política (por ejemplo, `fetch_state/direct_mode_scoreboard.json`). Archivez-le avec le résumé dans les tickets de changement.- Automatización de la puerta de adopción: una vez finalizada la búsqueda, el asistente llama a `cargo xtask sorafs-adoption-check` para utilizar los caminos persistentes del marcador y del resumen. El quórum requerido por defecto corresponde al nombre de los proveedores que se encuentran en la línea de comando; Reemplácelo con `--min-providers=<n>` cuando tenga un échantillon plus grande. Los informes de adopción están escritos en el corazón del currículum (`--adoption-report=<path>` pueden definir un lugar personalizado) y el asistente pasado `--require-direct-only` por defecto (alineado en la respuesta) y `--require-telemetry` cuando le proporcionamos la bandera correspondiente. Utilice `XTASK_SORAFS_ADOPTION_FLAGS` para transmitir argumentos complementarios de xtask (por ejemplo, `--allow-single-source` durante una degradación aprobada para que la puerta tolere e imponga la respuesta). Ne sautez le gate d'adoption avec `--skip-adoption-check` que lors de diagnostics localux; La hoja de ruta exige que cada ejecución se regule en modo directo y incluya el paquete de relación de adopción.

## 5. Lista de verificación de implementación1. **Gel de configuración:** almacena el perfil JSON del modo directo en tu depósito `iroha_config` y consigna el hash en tu billete de cambio.
2. **Puerta de enlace de auditoría:** confirme que los puntos finales Torii apliquen TLS, los TLV de capacidad y el registro de auditoría previa al basculante en modo directo. Publique el perfil de la puerta de enlace política para los operadores.
3. **Validación conforme:** comparta el libro de jugadas del día con los lectores conformes / réglementaires y capture las aprobaciones para operar en dehors de la superposición anónima.
4. **Ejecución en seco:** ejecute la suite de conformidad plus un fetch de staging contre des proveedores Torii de confianza. Archivez les salidas del marcador y les currículums CLI.
5. **Producción básica:** annoncez la fenêtre de changement, passez `transport_policy` à `direct_only` (si vous aviez opté pour `soranet-first`) et surveillez les Dashboards du mode direct (latence `sorafs_fetch`, compteurs d'échec des proveedores). Documente el plan de reversión para volver a SoraNet: primero una vez que SNNet-4/5/5a/5b/6a/7/8/12/13 están pasados ​​en `roadmap.md:532`.
6. **Revisión posterior al cambio:** adjunte las instantáneas del marcador, los currículums de recuperación y los resultados del seguimiento del billete de cambio. Mettez à jour `status.md` con la fecha efectiva y toda anomalía.Guarde la lista de verificación en la base del runbook `sorafs_node_ops` para que los operadores puedan repetir el flujo antes de que se produzca un basculamiento real. Cuando SNNet-5 pase GA, retire la respuesta después de confirmar la paridad en la televisión de producción.

## 6. Exigencias de preuves et gate d'adoption

Las capturas en modo directo deben cumplir siempre con la puerta de adopción SF-6c. Reagrupe el marcador, el resumen, el sobre de manifiesto y el informe de adopción para cada ejecución hasta que `cargo xtask sorafs-adoption-check` pueda validar la postura de respuesta. Les champs manquants font échouer le gate, consignez donc les métadonnées asistentes dans les tickets de changement.- **Metadonnées de transport:** `scoreboard.json` debe declarar `transport_policy="direct_only"` (y basculer `transport_policy_override=true` cuando haya forzado la degradación). Gardez les champs de politique d'anonymat associés remplis même lorsqu'ils héritent des valeurs por default afin que les relecteurs voient si vous avez devié du plan d'anonymat par étapes.
- **Computadores de proveedores:** Las sesiones de solo puerta de enlace deben persistir `provider_count=0` y reemplazar `gateway_provider_count=<n>` con el nombre de los proveedores Torii utilizados. Evite editar el JSON en el principal: el CLI/SDK deriva de los ordenadores y la puerta de adopción rechaza las capturas que evitan la separación.
- **Preuve de manifest:** Quand des gateways Torii participante, passez le `--gateway-manifest-envelope <path>` signé (ou l'equivalent SDK) afin que `gateway_manifest_provided` et les `gateway_manifest_id`/`gateway_manifest_cid` soient registrados en `scoreboard.json`. Asegúrese de que `summary.json` porta el mismo `manifest_id`/`manifest_cid`; la verificación de adopción se hará eco si el archivo se guarda en la pareja.
- **Attentes de télémétrie:** Cuando la télémétrie acompaña la captura, ejecute le gate avec `--require-telemetry` afin que le rapport d'adoption prouve que les métriques ont été émises. Las repeticiones en espacios de aire pueden omitir la bandera, pero el CI y los billetes de cambio documentan la ausencia.

Ejemplo:

```bash
cargo xtask sorafs-adoption-check \
  --scoreboard fetch_state/direct_mode_scoreboard.json \
  --summary fetch_state/direct_mode_summary.json \
  --allow-single-source \
  --require-direct-only \
  --json-out artifacts/sorafs_direct_mode/adoption_report.json \
  --require-telemetry
```Adjunte `adoption_report.json` con el marcador, el resumen, el sobre de manifiesto y el paquete de troncos de humo. Estos artefactos reflejan el trabajo de adopción de CI (`ci/check_sorafs_orchestrator_adoption.sh`) que imponen y generan degradaciones en modo auditable directo.