---
lang: ja
direction: ltr
source: docs/portal/docs/sdks/android-telemetry-redaction.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: es
direction: ltr
source: docs/portal/docs/sdks/android-telemetry-redaction.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f22e82241a07052e62531058ef612cbd99bfe52a0bb0cf53ec6d2e28bd6bf389
source_last_modified: "2025-11-18T05:53:43.294424+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: Plan de redacción de telemetría de Android
sidebar_label: Telemetría de Android
slug: /sdks/android-telemetry
---

:::note Fuente canonica
:::

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Plan de redacción de telemetría de Android (AND7)

## Alcance

Este documento recoge la política propuesta de redacción de telemetría y los artefactos de habilitación para el SDK de Android según lo exige el punto del roadmap **AND7**. Alinea la instrumentación móvil con la línea base de los nodos Rust mientras contempla garantías de privacidad específicas del dispositivo. El resultado sirve como material previo para la revisión de gobernanza de SRE de febrero de 2026.

Objetivos:

- Catalogar cada señal emitida por Android que llega a los backends de observabilidad compartidos (trazas OpenTelemetry, logs codificados en Norito, exportaciones de métricas).
- Clasificar los campos que difieren de la línea base de Rust y documentar los controles de redacción o retención.
- Esbozar el trabajo de habilitación y pruebas para que los equipos de soporte respondan de forma determinista a las alertas relacionadas con la redacción.

## Inventario de señales (borrador)

Instrumentación planificada agrupada por canal. Todos los nombres de campo siguen el esquema de telemetría del SDK de Android (`org.hyperledger.iroha.android.telemetry.*`). Los campos opcionales se marcan con `?`.

| ID de señal | Canal | Campos clave | Clasificación PII/PHI | Redacción / Retención | Notas |
|-----------|---------|------------|------------------------|-----------------------|-------|
| `android.torii.http.request` | Span de traza | `authority_hash`, `route`, `status_code`, `latency_ms` | La autoridad es pública; la ruta no contiene secretos | Emitir la autoridad hasheada (`blake2b_256`) antes de exportar; retener 7 días | Refleja `torii.http.request` de Rust; el hashing garantiza la privacidad del alias móvil. |
| `android.torii.http.retry` | Evento | `route`, `retry_count`, `error_code`, `backoff_ms` | Ninguna | Sin redacción; retener 30 días | Usado para auditorías deterministas de reintento; idéntico a los campos de Rust. |
| `android.pending_queue.depth` | Métrica gauge | `queue_type`, `depth` | Ninguna | Sin redacción; retener 90 días | Coincide con `pipeline.pending_queue_depth` de Rust. |
| `android.keystore.attestation.result` | Evento | `alias_label`, `security_level`, `attestation_digest`, `device_brand_bucket` | Alias (derivado), metadatos del dispositivo | Reemplazar el alias por una etiqueta determinista, redaccionar la marca a bucket enum | Requerido para la preparación de atestación AND2; los nodos Rust no emiten metadatos del dispositivo. |
| `android.keystore.attestation.failure` | Contador | `alias_label`, `failure_reason` | Ninguna después de la redacción del alias | Sin redacción; retener 90 días | Soporta simulacros de chaos; `alias_label` deriva del alias hasheado. |
| `android.telemetry.redaction.override` | Evento | `override_id`, `actor_role_masked`, `reason`, `expires_at` | El rol del actor califica como PII operativa | El campo exporta la categoría de rol enmascarada; retener 365 días con log de auditoría | No está presente en Rust; los operadores deben tramitar overrides a través de soporte. |
| `android.telemetry.export.status` | Contador | `backend`, `status` | Ninguna | Sin redacción; retener 30 días | Paridad con los contadores de estado del exportador en Rust. |
| `android.telemetry.redaction.failure` | Contador | `signal_id`, `reason` | Ninguna | Sin redacción; retener 30 días | Requerido para reflejar `streaming_privacy_redaction_fail_total` de Rust. |
| `android.telemetry.device_profile` | Métrica gauge | `profile_id`, `sdk_level`, `hardware_tier` | Metadatos del dispositivo | Emitir buckets gruesos (SDK mayor, hardware tier); retener 30 días | Habilita dashboards de paridad sin exponer detalles del OEM. |
| `android.telemetry.network_context` | Evento | `network_type`, `roaming` | El operador puede ser PII | Eliminar `carrier_name` por completo; retener otros campos 7 días | `ClientConfig.networkContextProvider` proporciona la captura saneada para que las apps emitan tipo de red + roaming sin exponer datos del abonado; los dashboards de paridad tratan la señal como el análogo móvil de `peer_host` de Rust. |
| `android.telemetry.config.reload` | Evento | `source`, `result`, `duration_ms` | Ninguna | Sin redacción; retener 30 días | Refleja los spans de recarga de configuración de Rust. |
| `android.telemetry.chaos.scenario` | Evento | `scenario_id`, `outcome`, `duration_ms`, `device_profile` | El perfil de dispositivo está bucketizado | Igual que `device_profile`; retener 30 días | Registrado durante los ensayos de chaos requeridos para la preparación AND7. |
| `android.telemetry.redaction.salt_version` | Métrica gauge | `salt_epoch`, `rotation_id` | Ninguna | Sin redacción; retener 365 días | Rastrea la rotación del salt Blake2b; alerta de paridad cuando el epoch de hash de Android diverge de los nodos Rust. |
| `android.crash.report.capture` | Evento | `crash_id`, `signal`, `process_state`, `has_native_trace`, `anr_watchdog_bucket` | Huella de crash + metadatos de proceso | Hashear `crash_id` con el salt compartido de redacción, bucketizar el estado del watchdog, eliminar stack frames antes de exportar; retener 30 días | Se habilita automáticamente cuando se llama `ClientConfig.Builder.enableCrashTelemetryHandler()`; alimenta dashboards de paridad sin exponer trazas identificables del dispositivo. |
| `android.crash.report.upload` | Contador | `crash_id`, `backend`, `status`, `retry_count` | Huella de crash | Reutilizar el `crash_id` hasheado, emitir solo el estado; retener 30 días | Emitir vía `ClientConfig.crashTelemetryReporter()` o `CrashTelemetryHandler.recordUpload` para que las cargas compartan las mismas garantías Sigstore/OLTP que el resto de la telemetría. |

### Puntos de integración

- `ClientConfig` ahora propaga los datos de telemetría derivados del manifest vía `setTelemetryOptions(...)`/`setTelemetrySink(...)`, registrando automáticamente `TelemetryObserver` para que las autoridades hasheadas y las métricas de salt fluyan sin observers ad hoc. Ver `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java` y las clases asociadas bajo `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/`.
- Las aplicaciones pueden llamar `ClientConfig.Builder.enableAndroidNetworkContext(android.content.Context)` para registrar el `AndroidNetworkContextProvider` basado en reflexión, que consulta `ConnectivityManager` en tiempo de ejecución y emite el evento `android.telemetry.network_context` sin introducir dependencias Android en tiempo de compilación.
- Las pruebas unitarias `TelemetryOptionsTests` y `TelemetryObserverTests` (`java/iroha_android/src/test/java/org/hyperledger/iroha/android/telemetry/`) resguardan los helpers de hashing y el gancho de integración de ClientConfig para que las regresiones de manifiestos se detecten de inmediato.
- El kit/labs de habilitación ahora cita APIs concretas en lugar de pseudocódigo, manteniendo este documento y el runbook alineados con el SDK en producción.

> **Nota de operaciones:** la hoja de responsables/estado vive en `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` y debe actualizarse junto a esta tabla en cada checkpoint AND7.

## Listas de permitidos de paridad y flujo de diff de esquemas

La gobernanza requiere una doble lista de permitidos para que las exportaciones de Android nunca filtren identificadores que los servicios Rust muestran intencionalmente. Esta sección refleja la entrada del runbook (`docs/source/android_runbook.md` §2.3) pero mantiene el plan de redacción AND7 autocontenido.

| Categoría | Exportadores Android | Servicios Rust | Gancho de validación |
|----------|-------------------|---------------|-----------------|
| Contexto de autoridad/ruta | Hashear `authority`/`alias` con Blake2b-256 y eliminar hostnames Torii en bruto antes de exportar; emitir `android.telemetry.redaction.salt_version` para probar la rotación del salt. | Emitir hostnames Torii completos y peer IDs para correlación. | Comparar entradas `android.torii.http.request` vs `torii.http.request` en el último diff de esquema bajo `docs/source/sdk/android/readiness/schema_diffs/`, luego ejecutar `scripts/telemetry/check_redaction_status.py` para confirmar epochs del salt. |
| Identidad de dispositivo y firmante | Bucketizar `hardware_tier`/`device_profile`, hashear aliases del controlador y nunca exportar números de serie. | Emitir `peer_id` de validador, `public_key` del controlador y hashes de cola sin cambios. | Alinear con `docs/source/sdk/mobile_device_profile_alignment.md`, ejecutar pruebas de hashing de alias dentro de `java/iroha_android/run_tests.sh`, y archivar outputs del queue inspector durante los labs. |
| Metadatos de red | Exportar solo `network_type` + `roaming`; eliminar `carrier_name`. | Retener metadatos de hostname/TLS de peers. | Guardar cada diff de esquema en `readiness/schema_diffs/` y alertar si el widget “Network Context” de Grafana muestra cadenas de carriers. |
| Evidencia de override/chaos | Emitir `android.telemetry.redaction.override`/`android.telemetry.chaos.scenario` con roles de actor enmascarados. | Emitir aprobaciones de override sin enmascarar; sin spans específicos de chaos. | Contrastar `docs/source/sdk/android/readiness/and7_operator_enablement.md` tras los drills para asegurar que los tokens de override y los artefactos de chaos convivan con los eventos Rust sin enmascarar. |

Flujo de trabajo:

1. Después de cada cambio de manifest/exporter, ejecutar `scripts/telemetry/run_schema_diff.sh --android-config <android.json> --rust-config <rust.json>` y colocar el JSON bajo `docs/source/sdk/android/readiness/schema_diffs/`.
2. Revisar el diff contra la tabla anterior. Si Android emite un campo solo de Rust (o viceversa), registrar un bug de preparación AND7 y actualizar tanto este plan como el runbook.
3. Durante revisiones semanales de ops, ejecutar `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status` y registrar el epoch del salt más el timestamp del diff de esquema en la hoja de preparación.
4. Registrar cualquier desviación en `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` para que los paquetes de gobernanza capturen las decisiones de paridad.

> **Referencia de esquema:** los identificadores canónicos de campos se originan en `android_telemetry_redaction.proto` (materializado durante la build del SDK de Android junto a los descriptores Norito). El esquema expone los campos `authority_hash`, `alias_label`, `attestation_digest`, `device_brand_bucket` y `actor_role_masked` usados ahora en todo el SDK y los exportadores de telemetría.

`authority_hash` es un digest fijo de 32 bytes del valor de autoridad Torii registrado en el proto. `attestation_digest` captura la huella de la declaración de atestación canónica, mientras que `device_brand_bucket` mapea la cadena de marca Android cruda al enum aprobado (`generic`, `oem`, `enterprise`). `actor_role_masked` transporta la categoría del actor de override (`support`, `sre`, `audit`) en lugar del identificador de usuario crudo.

### Alineación de exportación de telemetría de fallos

La telemetría de fallos ahora comparte los mismos exportadores OpenTelemetry y la misma canalización de procedencia que las señales de red Torii, cerrando el follow-up de gobernanza sobre exportadores duplicados. El crash handler alimenta el evento `android.crash.report.capture` con un `crash_id` hasheado (Blake2b-256 usando el salt de redacción ya rastreado por `android.telemetry.redaction.salt_version`), buckets de estado de proceso y metadatos sanitizados del ANR watchdog. Las trazas se mantienen en el dispositivo y solo se resumen en los campos `has_native_trace` y `anr_watchdog_bucket` antes de exportar, por lo que no salen PII ni cadenas OEM del dispositivo.

Subir un crash crea el contador `android.crash.report.upload`, permitiendo a SRE auditar la confiabilidad del backend sin aprender nada sobre el usuario o la traza. Como ambas señales reutilizan el exportador Torii, heredan las mismas garantías de firma Sigstore, políticas de retención y ganchos de alertas ya definidos para AND7. Los runbooks de soporte pueden, por tanto, correlacionar un identificador de crash hasheado entre los bundles de evidencia Android y Rust sin una canalización de crash a medida.

Habilita el handler vía `ClientConfig.Builder.enableCrashTelemetryHandler()` una vez configuradas las opciones y sinks de telemetría; los bridges de carga de crash pueden reutilizar `ClientConfig.crashTelemetryReporter()` (o `CrashTelemetryHandler.recordUpload`) para emitir resultados de backend en la misma canalización firmada.

## Diferencias de política respecto a la línea base de Rust

Diferencias entre las políticas de telemetría de Android y Rust con pasos de mitigación.

| Categoría | Línea base de Rust | Política Android | Mitigación / Validación |
|----------|---------------|----------------|-------------------------|
| Identificadores de autoridad/peer | Cadenas de autoridad en claro | `authority_hash` (Blake2b-256, salt rotativo) | Salt compartido publicado vía `iroha_config.telemetry.redaction_salt`; prueba de paridad asegura mapeo reversible para soporte. |
| Metadatos de host/red | Hostnames/IPs de nodos exportados | Solo tipo de red + roaming | Dashboards de salud de red actualizados para usar categorías de disponibilidad en lugar de hostnames. |
| Características del dispositivo | N/A (lado servidor) | Perfil bucketizado (SDK 21/23/29+, tier `emulator`/`consumer`/`enterprise`) | Ensayos de chaos verifican el mapeo de buckets; el runbook de soporte documenta la ruta de escalamiento cuando se requiere más detalle. |
| Overrides de redacción | No soportado | Token de override manual almacenado en el ledger Norito (`actor_role_masked`, `reason`) | Overrides requieren solicitud firmada; log de auditoría retenido 1 año. |
| Trazas de atestación | Atestación de servidor vía SRE solamente | El SDK emite resumen de atestación sanitizado | Cruzar hashes de atestación con el validador Rust; el alias hasheado evita fugas. |

Checklist de validación:

- Pruebas unitarias de redacción para cada señal verificando campos hasheados/enmascarados antes del envío al exportador.
- Herramienta de diff de esquemas (compartida con nodos Rust) ejecutada cada noche para confirmar la paridad de campos.
- Script de ensayo de chaos que ejerce el flujo de override y confirma el logging de auditoría.

## Tareas de implementación (pre-gobernanza SRE)

1. **Confirmación del inventario** — Contrastar la tabla anterior con los hooks reales de instrumentación del SDK de Android y las definiciones de esquema Norito. Responsables: Android Observability TL, LLM.
2. **Diff del esquema de telemetría** — Ejecutar la herramienta compartida de diff contra métricas Rust para producir artefactos de paridad para la revisión SRE. Responsable: SRE privacy lead.
3. **Borrador del runbook (completado 2026-02-03)** — `docs/source/android_runbook.md` ahora documenta el flujo end-to-end de overrides (Sección 3) y la matriz de escalamiento ampliada más responsabilidades de rol (Sección 3.1), vinculando los helpers de CLI, evidencia de incidentes y scripts de chaos con la política de gobernanza. Responsables: LLM con edición de Docs/Support.
4. **Contenido de habilitación** — Preparar diapositivas de briefing, instrucciones de laboratorio y preguntas de conocimiento para la sesión de febrero de 2026. Responsables: Docs/Support Manager, equipo de habilitación SRE.

## Flujo de habilitación y ganchos del runbook

### 1. Cobertura smoke local + CI

- `scripts/android_sample_env.sh --telemetry --telemetry-duration=5m --telemetry-cluster=<host>` levanta el sandbox Torii, reproduce el fixture canónico multi-source de SoraFS (delegando en `ci/check_sorafs_orchestrator_adoption.sh`) y siembra telemetría Android sintética.
  - La generación de tráfico la maneja `scripts/telemetry/generate_android_load.py`, que registra una transcripción request/response bajo `artifacts/android/telemetry/load-generator.log` y respeta headers, overrides de ruta o modo dry-run.
  - El helper copia el scoreboard y los resúmenes de SoraFS en `${WORKDIR}/sorafs/` para que los ensayos AND7 prueben la paridad multi-source antes de tocar clientes móviles.
- CI reutiliza la misma herramienta: `ci/check_android_dashboard_parity.sh` ejecuta `scripts/telemetry/compare_dashboards.py` contra `dashboards/grafana/android_telemetry_overview.json`, el dashboard de referencia Rust y el archivo de allowlist en `dashboards/data/android_rust_dashboard_allowances.json`, emitiendo el snapshot firmado `docs/source/sdk/android/readiness/dashboard_parity/android_vs_rust-latest.json`.
- Los ensayos de chaos siguen `docs/source/sdk/android/telemetry_chaos_checklist.md`; el script de sample-env más el chequeo de paridad de dashboards forman el bundle de evidencia “ready” que alimenta la auditoría de burn-in AND7.

### 2. Emisión de overrides y rastro de auditoría

- `scripts/android_override_tool.py` es la CLI canónica para emitir y revocar overrides de redacción. `apply` ingiere una solicitud firmada, emite el bundle del manifest (`telemetry_redaction_override.to` por defecto) y agrega una fila de token hasheado en `docs/source/sdk/android/telemetry_override_log.md`. `revoke` estampa la marca de revocación en esa misma fila, y `digest` escribe el snapshot JSON sanitizado requerido para gobernanza.
- La CLI se niega a modificar el log de auditoría a menos que el encabezado de la tabla Markdown esté presente, alineándose con el requisito de cumplimiento capturado en `docs/source/android_support_playbook.md`. La cobertura unitaria en `scripts/tests/test_android_override_tool_cli.py` protege el parser de la tabla, los emisores de manifests y el manejo de errores.
- Los operadores adjuntan el manifest generado, el extracto actualizado del log **y** el digest JSON bajo `docs/source/sdk/android/readiness/override_logs/` cada vez que se ejerce un override; el log retiene 365 días de historial según la decisión de gobernanza en este plan.

### 3. Captura de evidencias y retención

- Cada ensayo o incidente produce un bundle estructurado bajo `artifacts/android/telemetry/` que contiene:
  - La transcripción del generador de carga y contadores agregados de `generate_android_load.py`.
  - La diff de dashboards (`android_vs_rust-<stamp>.json`) y el hash de allowlist emitidos por `ci/check_android_dashboard_parity.sh`.
  - Delta del override log (si se otorgó un override), el manifest correspondiente y el digest JSON actualizado.
- El reporte de burn-in SRE referencia esos artefactos más el scoreboard de SoraFS copiado por `android_sample_env.sh`, dando a la revisión AND7 una cadena determinista desde hashes de telemetría → dashboards → estado de overrides.

## Alineación de perfil de dispositivo entre SDKs

Los dashboards traducen el `hardware_tier` de Android al `mobile_profile_class` canónico definido en `docs/source/sdk/mobile_device_profile_alignment.md` para que la telemetría AND7 e IOS7 compare las mismas cohortes:

- `lab` — emitido como `hardware_tier = emulator`, coincidiendo con `device_profile_bucket = simulator` de Swift.
- `consumer` — emitido como `hardware_tier = consumer` (con el sufijo del SDK mayor) y agrupado con los buckets `iphone_small`/`iphone_large`/`ipad` de Swift.
- `enterprise` — emitido como `hardware_tier = enterprise`, alineado con el bucket `mac_catalyst` de Swift y futuros runtimes gestionados/iOS de escritorio.

Cualquier tier nuevo debe añadirse al documento de alineación y a los artefactos de diff de esquema antes de que los dashboards lo consuman.

## Gobernanza y distribución

- **Paquete de pre-read** — Este documento más los artefactos de apéndice (diff de esquema, diff de runbook, esquema de deck de readiness) se distribuirán a la lista de correo de gobernanza SRE a más tardar el **2026-02-05**.
- **Bucle de feedback** — Los comentarios recogidos durante la gobernanza alimentarán el epic de JIRA `AND7`; los bloqueos se reflejan en `status.md` y en las notas del stand-up semanal de Android.
- **Publicación** — Una vez aprobado, el resumen de la política se enlazará desde `docs/source/android_support_playbook.md` y será referenciado por el FAQ de telemetría compartido en `docs/source/telemetry.md`.

## Notas de auditoría y cumplimiento

- La política respeta GDPR/CCPA eliminando datos de suscriptor móvil antes de exportar; el salt de autoridad hasheada rota trimestralmente y se almacena en el vault compartido de secretos.
- Los artefactos de habilitación y actualizaciones del runbook se registran en el registro de cumplimiento.
- Las revisiones trimestrales confirman que los overrides siguen en circuito cerrado (sin accesos obsoletos).

## Resultado de la gobernanza (2026-02-12)

La sesión de gobernanza SRE del **2026-02-12** aprobó la política de redacción de Android sin modificaciones. Decisiones clave (ver `docs/source/sdk/android/telemetry_redaction_minutes_20260212.md`):

- **Aceptación de política.** Se ratificaron la autoridad hasheada, el bucketizado del perfil de dispositivo y la omisión de nombres de carriers. El seguimiento de rotación de salt vía `android.telemetry.redaction.salt_version` pasa a ser un ítem de auditoría trimestral.
- **Plan de validación.** Se respaldaron la cobertura unitaria/integración, los diffs de esquema nocturnos y los ensayos de chaos trimestrales. Acción: publicar un reporte de paridad de dashboards después de cada ensayo.
- **Gobernanza de overrides.** Se aprobaron tokens de override registrados en Norito con ventana de retención de 365 días. Soporte de ingeniería será responsable de la revisión del digest del override log durante las sincronizaciones mensuales de operaciones.

## Estado de seguimiento

1. **Alineación de perfil de dispositivo (vence 2026-03-01).** ✅ Completado — el mapeo compartido en `docs/source/sdk/mobile_device_profile_alignment.md` define cómo los valores `hardware_tier` de Android se traducen a la clase canónica `mobile_profile_class` consumida por los dashboards de paridad y el tooling de diff de esquemas.

## Próximo briefing de gobernanza SRE (Q2 2026)

El punto de roadmap **AND7** requiere que la próxima sesión de gobernanza SRE reciba un pre-read conciso sobre redacción de telemetría Android. Usa esta sección como brief vivo; mantenla actualizada antes de cada reunión del consejo.

### Checklist de preparación

1. **Bundle de evidencias** — exporta el último diff de esquema, capturas de dashboards y digest del override log (ver matriz abajo) y colócalos bajo una carpeta fechada (por ejemplo `docs/source/sdk/android/readiness/and7_sre_brief/2026-02-07/`) antes de enviar la invitación.
2. **Resumen de drills** — adjunta el log del ensayo de chaos más reciente y el snapshot de la métrica `android.telemetry.redaction.failure`; asegúrate de que las anotaciones de Alertmanager referencien el mismo timestamp.
3. **Auditoría de overrides** — confirma que todos los overrides activos estén registrados en el registro Norito y resumidos en el deck de la reunión. Incluye fechas de expiración y los IDs de incidentes correspondientes.
4. **Nota de agenda** — avisa al chair de SRE 48 horas antes de la reunión con el enlace al brief, resaltando cualquier decisión requerida (nuevas señales, cambios de retención o actualizaciones de política de overrides).

### Matriz de evidencias

| Artefacto | Ubicación | Responsable | Notas |
|----------|----------|-------|-------|
| Diff de esquema vs Rust | `docs/source/sdk/android/readiness/schema_diffs/<latest>.json` | Telemetry tooling DRI | Debe generarse <72 h antes de la reunión. |
| Capturas del diff de dashboards | `docs/source/sdk/android/readiness/dashboards/<date>/` | Observability TL | Incluir `sorafs.fetch.*`, `android.telemetry.*` y snapshots de Alertmanager. |
| Digest de overrides | `docs/source/sdk/android/readiness/override_logs/<date>.json` | Support engineering | Ejecutar `scripts/android_override_tool.sh digest` (ver README en ese directorio) contra el `telemetry_override_log.md` más reciente; los tokens siguen hasheados antes de compartir. |
| Log de ensayo de chaos | `artifacts/android/telemetry/chaos/<date>/log.ndjson` | QA automation | Adjuntar resumen de KPI (conteo de stalls, ratio de reintentos, uso de overrides). |

### Preguntas abiertas para el consejo

- ¿Necesitamos acortar la ventana de retención de overrides de 365 días ahora que el digest está automatizado?
- ¿Debe `android.telemetry.device_profile` adoptar las nuevas etiquetas compartidas `mobile_profile_class` en la próxima release, o esperar a que los SDK Swift/JS publiquen el mismo cambio?
- ¿Se requiere orientación adicional sobre residencia regional de datos una vez que los eventos Torii Norito-RPC lleguen a Android (follow-up NRPC-3)?

### Procedimiento de diff del esquema de telemetría

Ejecuta la herramienta de diff de esquema al menos una vez por release candidate (y siempre que cambie la instrumentación Android) para que el consejo SRE reciba artefactos de paridad actualizados junto con el diff de dashboards:

1. Exporta los esquemas de telemetría Android y Rust que quieras comparar. Para CI las configs viven bajo `configs/android_telemetry.json` y `configs/rust_telemetry.json`.
2. Ejecuta `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/<date>-android_vs_rust.json`.
   - Alternativamente pasa commits (`scripts/telemetry/run_schema_diff.sh android-main rust-main`) para extraer las configs directamente desde git; el script fija los hashes dentro del artefacto.
3. Adjunta el JSON generado al bundle de readiness y enlázalo desde `status.md` + `docs/source/telemetry.md`. El diff resalta campos añadidos/eliminados y deltas de retención para que los auditores confirmen la paridad sin volver a ejecutar la herramienta.
4. Cuando el diff revele divergencia permitida (por ejemplo señales de override solo en Android), actualiza el archivo de allowlist referenciado por `ci/check_android_dashboard_parity.sh` y anota la justificación en el README del directorio de diff de esquema.

> **Reglas de archivo:** conserva los cinco diffs más recientes bajo `docs/source/sdk/android/readiness/schema_diffs/` y mueve snapshots más antiguos a `artifacts/android/telemetry/schema_diffs/` para que los revisores de gobernanza siempre vean los datos más recientes.
