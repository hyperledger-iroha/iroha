---
lang: es
direction: ltr
source: docs/source/android_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: da7119ab99121dbcfc268f5406f43b16ac9149cef6500a45c6717ad16c02ab80
source_last_modified: "2026-01-28T17:01:56.615899+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Runbook de operaciones del SDK de Android

Este runbook es compatible con operadores e ingenieros de soporte que administran el SDK de Android.
implementaciones para AND7 y más allá. Emparéjelo con el Playbook de soporte de Android para SLA
definiciones y vías de escalada.

> **Nota:** Al actualizar los procedimientos de incidentes, actualice también el archivo compartido
> matriz de solución de problemas (`docs/source/sdk/android/troubleshooting.md`) para que el
> la tabla de escenarios, los SLA y las referencias de telemetría se mantienen alineados con este runbook.

## 0. Inicio rápido (cuando se activan los buscapersonas)

Tenga esta secuencia a mano para las alertas Sev1/Sev2 antes de profundizar en los detalles.
secciones siguientes:

1. **Confirme la configuración activa:** Capture la suma de comprobación del manifiesto `ClientConfig`
   emitido al inicio de la aplicación y compararlo con el manifiesto anclado en
   `configs/android_client_manifest.json`. Si los hashes divergen, detenga las liberaciones y
   presente un ticket de deriva de configuración antes de tocar telemetría/anulaciones (consulte §1).
2. **Ejecute la puerta de diferenciación del esquema:** Ejecute la CLI `telemetry-schema-diff` contra
   la instantánea aceptada
   (`docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`).
   Trate cualquier salida `policy_violations` como Sev2 y bloquee las exportaciones hasta que
   se entiende la discrepancia (ver §2.6).
3. **Verifique los paneles + CLI de estado:** Abra la redacción de telemetría de Android y
   Tableros de salud del exportador, luego ejecute
   `scripts/telemetry/check_redaction_status.py --status-url <collector>`. si
   las autoridades están bajo el piso o exportan error, capturar capturas de pantalla y
   Salida CLI para el documento del incidente (consulte §2.4–§2.5).
4. **Decida las anulaciones:** Solo después de los pasos anteriores y con el incidente/propietario
   grabado, emita una anulación limitada a través de `scripts/android_override_tool.sh`
   y regístrelo en `telemetry_override_log.md` (ver §3). Caducidad predeterminada: <24h.
5. **Escalar por lista de contactos:** Localice el TL de observación y de guardia de Android
   (contactos en §8), luego siga el árbol de escalada en §4.1. Si la certificación o
   Las señales de StrongBox están involucradas, extraiga el último paquete y ejecute el arnés.
   controles del artículo 7 antes de volver a permitir las exportaciones.

## 1. Configuración e implementación

- **Abastecimiento de ClientConfig:** Asegúrese de que los clientes de Android carguen el punto final Torii, TLS
  políticas y reintentar botones de manifiestos derivados de `iroha_config`. Validar
  valores durante el inicio de la aplicación y la suma de comprobación del registro del manifiesto activo.
  Referencia de implementación: `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`
  hilos `TelemetryOptions` de `java/iroha_android/src/main/java/org/hyperledger/iroha/android/telemetry/TelemetryOptions.java`
  (más el `TelemetryObserver` generado) para que las autoridades hash se emitan automáticamente.
- **Recarga en caliente:** Utilice el observador de configuración para seleccionar `iroha_config`
  actualizaciones sin reinicios de la aplicación. Las recargas fallidas deberían emitir el
  Evento `android.telemetry.config.reload` y reintento de activación con exponencial
  retroceso (máximo 5 intentos).
- **Comportamiento alternativo:** Cuando la configuración falta o no es válida, recurra a
  valores predeterminados seguros (modo de solo lectura, sin envío de cola pendiente) y mostrar un usuario
  rápido. Registre el incidente para su seguimiento.

### 1.1 Diagnóstico de recarga de configuración- El monitor de configuración emite señales `android.telemetry.config.reload` con
  `source`, `result`, `duration_ms` y campos opcionales `digest`/`error` (consulte
  `configs/android_telemetry.json` y
  `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ConfigWatcher.java`).
  Espere un único evento `result:"success"` por manifiesto aplicado; repetido
  Los registros `result:"error"` indican que el observador agotó sus 5 intentos de retroceso
  a partir de 50 ms.
- Durante un incidente, capture la última señal de recarga del recolector
  (OTLP/span store o el punto final de estado de redacción) y registre el `digest` +
  `source` en el documento del incidente. Compara el resumen con
  `configs/android_client_manifest.json` y el manifiesto de lanzamiento distribuido a
  operadores.
- Si el observador continúa emitiendo errores, ejecute el arnés de destino para reproducir
  el error de análisis con el manifiesto sospechoso:
  `ci/run_android_tests.sh org.hyperledger.iroha.android.client.ConfigWatcherTests`.
  Adjunte el resultado de la prueba y el manifiesto fallido al paquete de incidentes para que SRE
  puede diferenciarlo del esquema de configuración preparado.
- Cuando falta la telemetría de recarga, confirme que el `ClientConfig` activo lleva un
  sumidero de telemetría y que el recopilador OTLP aún acepta el
  Identificación `android.telemetry.config.reload`; de lo contrario, trátelo como una telemetría Sev2
  regresión (mismo camino que §2.4) y pausar las liberaciones hasta que la señal regrese.

### 1.2 Paquetes de exportación clave deterministas
- Las exportaciones de software ahora emiten paquetes v3 con salt + nonce por exportación, `kdf_kind` e `kdf_work_factor`.
  El exportador prefiere Argon2id (64 MiB, 3 iteraciones, paralelismo = 2) y recurre a
  PBKDF2-HMAC-SHA256 con un piso de iteración de 350 k cuando Argon2id no está disponible en el dispositivo. paquete
  AAD todavía se vincula al alias; Las frases de contraseña deben tener al menos 12 caracteres para las exportaciones v3 y el
  El importador rechaza las semillas totalmente cero sal/nonce.
  `KeyExportBundle.decode(Base64|bytes)`, importe con la frase de contraseña original y vuelva a exportar a v3 para
  pasar al formato de memoria dura. El importador rechaza los pares de sal/nonce totalmente cero o reutilizados; siempre
  rote paquetes en lugar de reutilizar exportaciones antiguas entre dispositivos.
- Pruebas de camino negativo en `ci/run_android_tests.sh --tests org.hyperledger.iroha.android.crypto.export.DeterministicKeyExporterTests`
  rechazo. Borre las matrices de caracteres de frase de contraseña después de su uso y capture tanto la versión del paquete como `kdf_kind`
  en notas de incidentes cuando falla la recuperación.

## 2. Telemetría y redacción

> Referencia rápida: ver
> [`telemetry_redaction_quick_reference.md`](sdk/android/telemetry_redaction_quick_reference.md)
> para la lista de verificación condensada de comando/umbral utilizada durante la habilitación
> sesiones y puentes de incidencias.- **Inventario de señales:** Consulte `docs/source/sdk/android/telemetry_redaction.md`
  para obtener la lista completa de intervalos/métricas/eventos emitidos y
  `docs/source/sdk/android/readiness/signal_inventory_worksheet.md`
  para obtener detalles del propietario/validación y lagunas pendientes.
- **Diferencia de esquema canónico:** La instantánea AND7 aprobada es
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`.
  Cada nueva ejecución de CLI debe compararse con este artefacto para que los revisores puedan ver
  que los `intentional_differences` e `android_only_signals` aceptados todavía
  coincidir con las tablas de políticas documentadas en
  `docs/source/sdk/android/telemetry_schema_diff.md` §3. La CLI ahora agrega
  `policy_violations` cuando falta alguna diferencia intencional
  `status:"accepted"`/`"policy_allowlisted"` (o cuando los registros solo de Android se pierden
  su estado aceptado), así que trate las violaciones no vacías como Sev2 y detenga
  exportaciones. Los fragmentos `jq` que aparecen a continuación permanecen como una verificación manual de integridad en archivos archivados.
  artefactos:
  ```bash
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted")' "$OUT"
  jq '.android_only_signals[] | select(.status != "accepted")' "$OUT"
  jq '.field_mismatches[] | {signal, field, android, rust}' "$OUT"
  ```
  Trate cualquier resultado de estos comandos como una regresión de esquema que necesita una
  Error de preparación de AND7 antes de que continúen las exportaciones de telemetría; `field_mismatches`
  debe permanecer vacío según `telemetry_schema_diff.md` §5. El ayudante ahora escribe
  `artifacts/android/telemetry/schema_diff.prom` automáticamente; pasar
  `--textfile-dir /var/lib/node_exporter/textfile_collector` (o configurar
  `ANDROID_SCHEMA_DIFF_TEXTFILE_DIR`) cuando se ejecuta en hosts de ensayo/producción
  entonces el indicador `telemetry_schema_diff_run_status` cambia a `policy_violation`
  automáticamente si la CLI detecta una desviación.
- **Ayudante de CLI:** `scripts/telemetry/check_redaction_status.py` inspecciona
  `artifacts/android/telemetry/status.json` por defecto; pasar `--status-url` a
  preparación de consultas e `--write-cache` para actualizar la copia local sin conexión
  taladros. Utilice `--min-hashed 214` (o configure
  `ANDROID_TELEMETRY_MIN_HASHED_AUTHORITIES=214`) para hacer cumplir la gobernanza
  piso sobre las autoridades hash durante cada encuesta de estado.
- **Hashing de autoridades:** Todas las autoridades se procesan usando Blake2b-256 con el
  Sal de rotación trimestral almacenada en la bóveda secreta segura. Las rotaciones ocurren en
  el primer lunes de cada trimestre a las 00:00 UTC. Verificar que el exportador recoja
  la nueva sal comprobando la métrica `android.telemetry.redaction.salt_version`.
- **Grupos de perfil de dispositivo:** Solo `emulator`, `consumer` e `enterprise`
  Los niveles se exportan (junto con la versión principal del SDK). Los paneles comparan estos
  cuenta contra las líneas base de Rust; La variación >10% genera alertas.
- **Metadatos de red:** Android exporta únicamente los indicadores `network_type` e `roaming`.
  Los nombres de los operadores nunca se emiten; Los operadores no deben solicitar abonado.
  información en los registros de incidentes. La instantánea desinfectada se emite como
  Evento `android.telemetry.network_context`, así que asegúrese de que las aplicaciones registren un
  `NetworkContextProvider` (ya sea a través de
  `ClientConfig.Builder.setNetworkContextProvider(...)` o la conveniencia
  `enableAndroidNetworkContext(...)` helper) antes de que se emitan las llamadas Torii.
- **Puntero Grafana:** El tablero `Android Telemetry Redaction` es el
  verificación visual canónica para la salida CLI anterior: confirme la
  El panel `android.telemetry.redaction.salt_version` coincide con la época de sal actual
  y el widget `android_telemetry_override_tokens_active` se queda en cero
  siempre que no se estén ejecutando simulacros o incidencias. Escalar si alguno de los paneles se desvía
  antes de que los scripts CLI informen una regresión.

### 2.1 Exportar flujo de trabajo de canalización1. **Distribución de configuración.** `ClientConfig.telemetry.redaction` está enhebrado desde
   `iroha_config` y recargado en caliente por `ConfigWatcher`. Cada recarga registra el
   resumen manifiesto más época de sal: capture esa línea en incidentes y durante
   ensayos.
2. **Instrumentación.** Los componentes del SDK emiten intervalos/métricas/eventos en el
   `TelemetryBuffer`. El búfer etiqueta cada carga útil con el perfil del dispositivo y
   época de sal actual para que el exportador pueda verificar las entradas de hash de manera determinista.
3. **Filtro de redacción.** `RedactionFilter` hashes `authority`, `alias` y
   identificadores de dispositivo antes de que abandonen el dispositivo. Las fallas emiten
   `android.telemetry.redaction.failure` y bloquear el intento de exportación.
4. **Exportador + recolector.** Las cargas útiles desinfectadas se envían a través de Android
   Exportador de OpenTelemetry a la implementación `android-otel-collector`. el
   salidas de los ventiladores del recopilador a trazas (Tempo), métricas (Prometheus) e Norito
   fregaderos de troncos.
5. **Ganchos de observabilidad.** `scripts/telemetry/check_redaction_status.py` lee
   contadores colectores (`android.telemetry.export.status`,
   `android.telemetry.redaction.salt_version`) y genera el paquete de estado
   referenciado a lo largo de este runbook.

### 2.2 Puertas de validación

- **Diferencia de esquema:** Ejecutar
  `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json`
  siempre que los manifiestos cambien. Después de cada ejecución, confirme cada
  La entrada `intentional_differences[*]` e `android_only_signals[*]` está estampada
  `status:"accepted"` (o `status:"policy_allowlisted"` para hash/bucketed
  campos) como se recomienda en `telemetry_schema_diff.md` §3 antes de adjuntar el
  artefacto hasta incidentes y caos en informes de laboratorio. Utilice la instantánea aprobada
  (`android_vs_rust-20260305.json`) como barandilla y pelusa el recién emitido
  JSON antes de archivarlo:
  ```bash
  LATEST=docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json
  jq '.intentional_differences[] | select(.status != "accepted" and .status != "policy_allowlisted") | {signal, field, status}' "$LATEST"
  jq '.android_only_signals[] | select(.status != "accepted") | {signal, status}' "$LATEST"
  ```
  Comparar `$LATEST` con
  `docs/source/sdk/android/readiness/schema_diffs/android_vs_rust-20260305.json`
  para demostrar que la lista de permitidos se mantuvo sin cambios. `status` faltante o en blanco
  entradas (por ejemplo en `android.telemetry.redaction.failure` o
  `android.telemetry.redaction.salt_version`) ahora se tratan como regresiones y
  debe resolverse antes de que se pueda cerrar la revisión; la CLI muestra el aceptado
  estado directamente, por lo que la referencia cruzada del manual §3.4 solo se aplica cuando
  explicando por qué aparece un estado que no es `accepted`.

  **Señales AND7 canónicas (instantánea del 5 de marzo de 2026)**| Señal | Canal | Estado | Nota de gobernanza | Gancho de validación |
  |--------|---------|--------|-----------------|-----------------|
  | `android.telemetry.redaction.override` | Evento | `accepted` | Los espejos anulan los manifiestos y deben coincidir con las entradas `telemetry_override_log.md`. | Mire `android_telemetry_override_tokens_active` y archive los manifiestos según §3. |
  | `android.telemetry.network_context` | Evento | `accepted` | Android redacta intencionalmente los nombres de los operadores; solo se exportan `network_type` e `roaming`. | Asegúrese de que las aplicaciones registren un `NetworkContextProvider` y confirmen que el volumen del evento sigue al tráfico Torii en `Android Telemetry Overview`. |
  | `android.telemetry.redaction.failure` | Mostrador | `accepted` | Emite cada vez que falla el hash; La gobernanza ahora requiere metadatos de estado explícitos en el artefacto de diferenciación del esquema. | El panel del tablero `Redaction Compliance` y la salida CLI de `check_redaction_status.py` deben permanecer en cero, excepto durante los simulacros. |
  | `android.telemetry.redaction.salt_version` | Calibre | `accepted` | Demuestra que el exportador está utilizando la época de sal trimestral actual. | Compare el widget salt de Grafana con la época de la bóveda de secretos y asegúrese de que las ejecuciones de diferencias de esquema conserven la anotación `status:"accepted"`. |

  Si alguna entrada en la tabla anterior arroja un `status`, el artefacto diferencial debe ser
  regenerado **y** `telemetry_schema_diff.md` actualizado antes del AND7
  Se distribuye el paquete de gobernanza. Incluya el JSON actualizado en
  `docs/source/sdk/android/readiness/schema_diffs/` y vincúlelo desde el
  incidente, laboratorio de caos o informe de habilitación que desencadenó la repetición.
- **Cobertura de CI/unidad:** `ci/run_android_tests.sh` debe pasar antes
  compilaciones editoriales; la suite impone un comportamiento de hash/anulación mediante el ejercicio
  los exportadores de telemetría con cargas útiles de muestra.
- **Comprobaciones de integridad del inyector:** Uso
  `scripts/telemetry/inject_redaction_failure.sh --dry-run` antes de los ensayos
  para confirmar fallas en los trabajos de inyección y que alerta de fuego cuando se hacen hash a los guardias
  están tropezados. Limpie siempre el inyector con `--clear` una vez validado
  completa.

### 2.3 Lista de verificación de paridad de telemetría móvil ↔ Rust

Mantenga alineados los exportadores de Android y los servicios del nodo Rust respetando la
diferentes requisitos de redacción documentados en
`docs/source/sdk/android/telemetry_redaction.md`. La siguiente tabla sirve como
lista de permitidos dual a la que se hace referencia en la entrada de la hoja de ruta de AND7: actualícela cada vez que
Schema diff introduce o elimina campos.| Categoría | Exportadores de Android | Servicios de óxido | Gancho de validación |
|----------|-------------------|---------------|-----------------|
| Autoridad/contexto de ruta | Hash `authority`/`alias` a través de Blake2b-256 y elimine los nombres de host Torii sin procesar antes de exportar; emite `android.telemetry.redaction.salt_version` para probar la rotación de la sal. | Emita nombres de host Torii completos e ID de pares para correlación. | Compare las entradas `android.torii.http.request` con `torii.http.request` en la última diferencia de esquema en `readiness/schema_diffs/`, luego confirme que `android.telemetry.redaction.salt_version` coincida con la sal del clúster ejecutando `scripts/telemetry/check_redaction_status.py`. |
| Dispositivo e identidad del firmante | Bucket `hardware_tier`/`device_profile`, alias de controlador hash y nunca exporte números de serie. | Sin metadatos del dispositivo; Los nodos emiten el validador `peer_id` y el controlador `public_key` palabra por palabra. | Refleje las asignaciones en `docs/source/sdk/mobile_device_profile_alignment.md`, audite las salidas de `PendingQueueInspector` durante las prácticas de laboratorio y asegúrese de que las pruebas de hash de alias dentro de `ci/run_android_tests.sh` permanezcan en verde. |
| Metadatos de red | Exportar solo valores booleanos `network_type` + `roaming`; `carrier_name` se elimina. | Rust conserva los nombres de host de los pares más los metadatos completos del punto final TLS. | Almacene el JSON de diferencia más reciente en `readiness/schema_diffs/` y confirme que el lado de Android aún omite `carrier_name`. Alerta si el widget "Contexto de red" de Grafana muestra alguna cadena de operador. |
| Anulación/evidencia de caos | Emite eventos `android.telemetry.redaction.override` e `android.telemetry.chaos.scenario` con roles de actor enmascarados. | Los servicios de Rust emiten aprobaciones de anulación sin enmascaramiento de roles y sin intervalos específicos de caos. | Verifique `docs/source/sdk/android/readiness/and7_operator_enablement.md` después de cada ejercicio para garantizar que los tokens de anulación y los artefactos del caos se archiven junto con los eventos de Rust desenmascarados. |

Flujo de trabajo de paridad:

1. Después de cada cambio de manifiesto o exportador, ejecute
   `scripts/telemetry/run_schema_diff.sh --android-config configs/android_telemetry.json --rust-config configs/rust_telemetry.json --out docs/source/sdk/android/readiness/schema_diffs/$(date -u +%Y%m%d).json --textfile-dir /var/lib/node_exporter/textfile_collector`
   por lo que el artefacto JSON y las métricas reflejadas se encuentran en el paquete de evidencia.
   (el ayudante todavía escribe `artifacts/android/telemetry/schema_diff.prom` de forma predeterminada).
2. Revise la diferencia con la tabla anterior; si Android ahora emite un campo que es
   solo permitido en Rust (o viceversa), presente un error de preparación de AND7 y actualice
   el plan de redacción.
3. Durante las comprobaciones semanales, ejecute
   `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`
   para confirmar que las épocas de sal coincidan con el widget Grafana y anote la época en el
   diario de guardia.
4. Registre cualquier delta en
   `docs/source/sdk/android/readiness/signal_inventory_worksheet.md` entonces
   La gobernanza puede auditar las decisiones de paridad.

### 2.4 Paneles de observabilidad y umbrales de alerta

Mantenga los paneles y las alertas alineados con las aprobaciones de diferencias de esquema AND7 cuando
revisando la salida `scripts/telemetry/check_redaction_status.py`:

- `Android Telemetry Redaction`: widget de época de sal, anulación del indicador de token.
- `Redaction Compliance` — `android.telemetry.redaction.failure` contador y
  Paneles de tendencia del inyector.
- `Exporter Health` — `android.telemetry.export.status` desgloses de tarifas.
- `Android Telemetry Overview`: depósitos de perfil de dispositivo y volumen de contexto de red.

Los siguientes umbrales reflejan la tarjeta de referencia rápida y deben aplicarse
durante la respuesta a incidentes y los ensayos:| Métrico/panel | Umbral | Acción |
|----------------|-----------|--------|
| `android.telemetry.redaction.failure` (placa `Redaction Compliance`) | >0 durante una ventana móvil de 15 minutos | Investigue la señal defectuosa, ejecute la limpieza del inyector, registre la salida CLI + captura de pantalla Grafana. |
| `android.telemetry.redaction.salt_version` (placa `Android Telemetry Redaction`) | Se diferencia de la época de la sal de la bóveda secreta | Detener las liberaciones, coordinar con la rotación de secretos, presentar la nota AND7. |
| `android.telemetry.export.status{status="error"}` (placa `Exporter Health`) | >1% de las exportaciones | Inspeccione el estado del recopilador, capture diagnósticos de CLI y escale a SRE. |
| `android.telemetry.device_profile{tier="enterprise"}` frente a la paridad de Rust (`Android Telemetry Overview`) | Variación >10 % respecto de la línea base de Rust | Seguimiento de la gobernanza de archivos, verificar grupos de dispositivos, anotar artefactos de diferencias de esquema. |
| Volumen `android.telemetry.network_context` (`Android Telemetry Overview`) | Cae a cero mientras existe tráfico Torii | Confirme el registro `NetworkContextProvider`, vuelva a ejecutar la diferencia de esquema para garantizar que los campos no se modifiquen. |
| `android.telemetry.redaction.override` / `android_telemetry_override_tokens_active` (`Android Telemetry Redaction`) | Ventana de anulación/perforación aprobada fuera de cero | Vincular el token a un incidente, regenerar el resumen y revocarlo mediante el flujo de trabajo en §3. |

### 2.5 Ruta de preparación y habilitación del operador

El elemento de la hoja de ruta AND7 incluye un plan de estudios de operador dedicado para que el soporte, SRE y
Las partes interesadas en la versión comprenden las tablas de paridad anteriores antes de que se publique el runbook.
GA. Utilice el esquema en
`docs/source/sdk/android/telemetry_readiness_outline.md` para logística canónica
(agenda, presentadores, cronograma) y `docs/source/sdk/android/readiness/and7_operator_enablement.md`
para obtener la lista de verificación detallada, los enlaces de evidencia y el registro de acciones. Mantenga lo siguiente
fases sincronizadas cada vez que cambia el plan de telemetría:| Fase | Descripción | Paquete de pruebas | Propietario principal |
|-------|-------------|-----------------|---------------|
| Distribución de lectura previa | Envíe la lectura previa de la póliza, `telemetry_redaction.md`, y la tarjeta de referencia rápida al menos cinco días hábiles antes de la sesión informativa. Realice un seguimiento de los reconocimientos en el registro de comunicaciones del esquema. | `docs/source/sdk/android/telemetry_readiness_outline.md` (Logística de sesión + Registro de comunicaciones) y el correo electrónico archivado en `docs/source/sdk/android/readiness/archive/<YYYY-MM>/`. | Administrador de Documentos/Soporte |
| Sesión de preparación en vivo | Ofrezca la capacitación de 60 minutos (análisis profundo de políticas, tutorial del runbook, paneles, demostración del laboratorio del caos) y mantenga la grabación en ejecución para los espectadores asincrónicos. | Grabación + diapositivas almacenadas en `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` con referencias capturadas en el §2 del esquema. | LLM (propietario interino de AND7) |
| Ejecución en el laboratorio del caos | Ejecute al menos C2 (anulación) + C6 (reproducción en cola) desde `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md` inmediatamente después de la sesión en vivo y adjunte registros/capturas de pantalla al kit de habilitación. | Informes de escenarios y capturas de pantalla dentro de `docs/source/sdk/android/readiness/labs/reports/<YYYY-MM>/` e `/screenshots/<YYYY-MM>/`. | Observabilidad Android TL + SRE de guardia |
| Verificación de conocimientos y asistencia | Recopile envíos de cuestionarios, corrija a cualquiera que obtenga una puntuación <90 % y registre las estadísticas de asistencia y cuestionarios. Mantenga las preguntas de referencia rápida alineadas con la lista de verificación de paridad. | Exportaciones de cuestionarios en `docs/source/sdk/android/readiness/forms/responses/`, resumen Markdown/JSON producido a través de `scripts/telemetry/generate_and7_quiz_summary.py` y la tabla de asistencia dentro de `and7_operator_enablement.md`. | Ingeniería de soporte |
| Archivo y seguimientos | Actualice el registro de acciones del kit de habilitación, cargue artefactos en el archivo y anote la finalización en `status.md`. Cualquier token de corrección o anulación emitido durante la sesión se debe copiar en `telemetry_override_log.md`. | `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6 (registro de acciones), `.../archive/<YYYY-MM>/checklist.md` y el registro de anulación al que se hace referencia en §3. | LLM (propietario interino de AND7) |

Cuando se vuelva a ejecutar el plan de estudios (trimestralmente o antes de cambios importantes en el esquema), actualice
el esquema con la nueva fecha de la sesión, mantener actualizada la lista de asistentes y
regenerar los artefactos JSON/Markdown del resumen del cuestionario para que los paquetes de gobernanza puedan
hacer referencia a evidencia consistente. La entrada `status.md` para AND7 debe vincularse al
la última carpeta de archivo una vez que se cierra cada sprint de habilitación.

### 2.6 Listas permitidas de diferencias de esquema y comprobaciones de políticas

La hoja de ruta menciona explícitamente una política de lista doble permitida (redacciones móviles vs.
Retención de óxido) que aplica la CLI `telemetry-schema-diff` alojada en
`tools/telemetry-schema-diff`. Cada artefacto diferencial registrado en
`docs/source/sdk/android/readiness/schema_diffs/` debe documentar qué campos están
hash/bucketed en Android, qué campos permanecen sin hash en Rust y si
cualquier señal no permitida se deslizó en la compilación. Captar esas decisiones
directamente en el JSON ejecutando:

```bash
cargo run -p telemetry-schema-diff -- \
  --android-config configs/android_telemetry.json \
  --rust-config configs/rust_telemetry.json \
  --format json \
  > "$LATEST"

if jq -e '.policy_violations | length > 0' "$LATEST" >/dev/null; then
  jq '.policy_violations[]' "$LATEST"
  exit 1
fi
```El `jq` final se evalúa como no operativo cuando el informe está limpio. Trate cualquier salida
de ese comando como un error de preparación de Sev2: un `policy_violations` poblado
matriz significa que la CLI descubrió una señal que no está en la lista de solo Android
ni en la lista de exenciones exclusivas de Rust documentada en
`docs/source/sdk/android/telemetry_schema_diff.md`. Cuando esto ocurra, detenga
exporta, presenta un ticket AND7 y vuelve a ejecutar la diferencia solo después de que el módulo de política
y se han corregido las instantáneas del manifiesto. Almacene el JSON resultante en
`docs/source/sdk/android/readiness/schema_diffs/` con el sufijo de fecha y nota
la ruta dentro del incidente o informe de laboratorio para que la gobernanza pueda reproducir las comprobaciones.

**Matriz de hash y retención**

| Campo.de.señal | Manejo de Android | Manejo de óxido | Etiqueta de lista permitida |
|--------------|-----------------|---------------|---------------|
| `torii.http.request.authority` | Blake2b-256 hash (`representation: "blake2b_256"`) | Almacenado palabra por palabra para su trazabilidad | `policy_allowlisted` (hash móvil) |
| `attestation.result.alias` | Blake2b-256 hash | Alias ​​de texto sin formato (archivos de certificación) | `policy_allowlisted` |
| `attestation.result.device_tier` | En cubos (`representation: "bucketed"`) | Cadena de niveles lisos | `policy_allowlisted` |
| `hardware.profile.hardware_tier` | Ausente: los exportadores de Android abandonan el campo por completo | Presente sin redacción | `rust_only` (documentado en §3 de `telemetry_schema_diff.md`) |
| `android.telemetry.redaction.override.*` | Señal solo para Android con roles de actores enmascarados | No se emite ninguna señal equivalente | `android_only` (debe permanecer `status:"accepted"`) |

Cuando aparezcan nuevas señales, agréguelas al módulo de política de diferencias de esquema **y** el
tabla anterior para que el runbook refleje la lógica de aplicación incluida en la CLI.
La ejecución del esquema ahora falla si alguna señal solo de Android omite un `status` explícito o si
la matriz `policy_violations` no está vacía, así que mantenga esta lista de verificación sincronizada con
`telemetry_schema_diff.md` §3 y las últimas instantáneas JSON a las que se hace referencia en
`telemetry_redaction_minutes_*.md`.

## 3. Anular el flujo de trabajo

Las anulaciones son la opción "romper el cristal" al aplicar hash en regresiones o privacidad
Las alertas bloquean a los clientes. Aplicarlos sólo después de registrar el proceso de decisión completo.
en el documento del incidente.1. **Confirmar deriva y alcance.** Espere la alerta de PagerDuty o la diferencia de esquema.
   puerta para disparar, luego correr
   `scripts/telemetry/check_redaction_status.py --status-url <collector>` a
   demostrar autoridades no coincidentes. Adjunte la salida CLI y las capturas de pantalla Grafana
   al registro del incidente.
2. **Prepare una solicitud firmada.** Completar
   `docs/examples/android_override_request.json` con el ID del ticket, solicitante,
   caducidad y justificación. Guarde el archivo junto a los artefactos del incidente para que
   El cumplimiento puede auditar las entradas.
3. **Emitir la anulación.** Invocar
   ```bash
   scripts/android_override_tool.sh apply \
     --request docs/examples/android_override_request.json \
     --log docs/source/sdk/android/telemetry_override_log.md \
     --out artifacts/android/telemetry/override-$(date -u +%Y%m%dT%H%M%SZ).json \
     --event-log docs/source/sdk/android/readiness/override_logs/override_events.ndjson \
     --actor-role <support|sre|docs|compliance|program|other>
   ```
   El asistente imprime el token de anulación, escribe el manifiesto y agrega una fila
   al registro de auditoría de Markdown. Nunca publiques el token en el chat; entregarlo directamente
   a los operadores Torii que aplican la anulación.
4. **Monitorear el efecto.** En cinco minutos, verifique un único
   Se emitió el evento `android.telemetry.redaction.override`, el recopilador
   El punto final de estado muestra `override_active=true` y el documento del incidente enumera los
   caducidad. Mire el panel "Anular tokens" del panel de descripción general de telemetría de Android.
   panel activo” (`android_telemetry_override_tokens_active`) para el mismo
   recuento de tokens y continúe ejecutando la CLI de estado cada 10 minutos hasta
   El hash se estabiliza.
5. **Revocar y archivar.** Tan pronto como llegue la mitigación, ejecute
  `scripts/android_override_tool.sh revoke --token <token>` para que el registro de auditoría
  captura el tiempo de revocación, luego ejecuta
  `scripts/android_override_tool.sh digest --out docs/source/sdk/android/readiness/override_logs/override_digest_$(date -u +%Y%m%dT%H%M%SZ).json`
  para actualizar la instantánea desinfectada que espera la gobernanza. Adjunte el
  manifiesto, resumen JSON, transcripciones CLI, instantáneas Grafana y el registro NDJSON
  producido a través de `--event-log` para
  `docs/source/sdk/android/readiness/screenshots/<date>/` y reticular el
  entrada de `docs/source/sdk/android/telemetry_override_log.md`.

Las anulaciones que excedan las 24 horas requieren la aprobación del Director de SRE y de Cumplimiento y
debe destacarse en la próxima revisión semanal de AND7.

### 3.1 Anular matriz de escalamiento

| Situación | Duración máxima | Aprobadores | Notificaciones requeridas |
|-----------|--------------|-----------|------------------------|
| Investigación de inquilino único (no coincide la autoridad hash, cliente Sev2) | 4 horas | Ingeniero de soporte + SRE de guardia | Ticket `SUP-OVR-<id>`, evento `android.telemetry.redaction.override`, registro de incidentes |
| Interrupción de telemetría en toda la flota o reproducción solicitada por SRE | 24 horas | SRE de guardia + Líder de programa | Nota de PagerDuty, anular entrada de registro, actualización en `status.md` |
| Solicitud de cumplimiento/forense o cualquier caso que exceda las 24 horas | Hasta que sea revocado explícitamente | Director SRE + Líder de Cumplimiento | Lista de correo de gobernanza, registro de anulación, estado semanal de AND7 |

#### Responsabilidades del rol| Rol | Responsabilidades | SLA / Notas |
|------|------------------|-------------|
| Telemetría de Android de guardia (Incident Commander) | Impulse la detección, ejecute las herramientas de anulación, registre las aprobaciones en el documento del incidente y asegúrese de que la revocación se realice antes de que expire. | Reconozca PagerDuty dentro de los 5 minutos y registre el progreso cada 15 minutos. |
| Observabilidad de Android TL (Haruka Yamamoto) | Valide la señal de deriva, confirme el estado del exportador/recolector y firme el manifiesto de anulación antes de entregarlo a los operadores. | Únase al puente en 10 minutos; delega al propietario del clúster provisional si no está disponible. |
| Enlace de SRE (Liam O'Connor) | Aplique el manifiesto a los recopiladores, supervise el trabajo pendiente y coordine con Release Engineering las mitigaciones del lado Torii. | Registre cada acción `kubectl` en la solicitud de cambio y pegue las transcripciones del comando en el documento del incidente. |
| Cumplimiento (Sofía Martins / Daniel Park) | Aprobar anulaciones de más de 30 minutos, verificar la fila del registro de auditoría y asesorar sobre los mensajes del regulador/cliente. | Publicar acuse de recibo en `#compliance-alerts`; para eventos de producción, presente una nota de cumplimiento antes de que se emita la anulación. |
| Gerente de Documentos/Soporte (Priya Deshpande) | Archive los manifiestos/la salida CLI en `docs/source/sdk/android/readiness/…`, mantenga ordenado el registro de anulación y programe laboratorios de seguimiento si surgen lagunas. | Confirma la retención de evidencia (13 meses) y archiva AND7 seguimientos antes de cerrar el incidente. |

Escalar inmediatamente si algún token de anulación se acerca a su vencimiento sin una
plan de revocación documentado.

## 4. Respuesta a incidentes

- **Alertas:** El servicio PagerDuty `android-telemetry-primary` cubre la redacción
  fallas, interrupciones del exportador y deriva del balde. Confirmar dentro de las ventanas SLA
  (ver manual de estrategias de soporte).
- **Diagnóstico:** Ejecute `scripts/telemetry/check_redaction_status.py` para recopilar
  Estado actual del exportador, alertas recientes y métricas de autoridad hash. incluir
  salida en la línea de tiempo del incidente (`incident/YYYY-MM-DD-android-telemetry.md`).
- **Paneles:** Supervisar la redacción de telemetría de Android, telemetría de Android
  Paneles de descripción general, cumplimiento de redacción y estado del exportador. Capturar
  capturas de pantalla para registros de incidentes y anotar cualquier versión salt o anulación
  desviaciones simbólicas antes de cerrar un incidente.
- **Coordinación:** Engage Release Engineering para temas de exportadores, Cumplimiento
  para preguntas de anulación/PII y líder de programa para incidentes del nivel 1.

### 4.1 Flujo de escalamiento

Los incidentes de Android se clasifican utilizando los mismos niveles de gravedad que los de Android.
Manual de estrategias de soporte (§2.1). La siguiente tabla resume quién debe ser localizado y cómo
Se espera que cada socorrista se una rápidamente al puente.| Gravedad | Impacto | Respondedor primario (≤5min) | Escalada secundaria (≤10min) | Notificaciones adicionales | Notas |
|----------|--------|----------------------|--------------------------------|--------------------------|---------------|
| Grave1 | Interrupción de atención al cliente, violación de la privacidad o fuga de datos | Telemetría de Android de guardia (`android-telemetry-primary`) | Torii de guardia + Líder de programa | Cumplimiento + Gobernanza SRE (`#sre-governance`), propietarios de clústeres provisionales (`#android-staging`) | Inicie War Room inmediatamente y abra un documento compartido para el registro de comandos. |
| Grave2 | Degradación de la flota, anulación de uso indebido o acumulación prolongada de reproducciones | Telemetría Android de guardia | Fundamentos de Android TL + Docs/Administrador de soporte | Líder de programa, enlace de ingeniería de lanzamiento | Escalar a Cumplimiento si las anulaciones exceden las 24 horas. |
| Sev3 | Problema de inquilino único, ensayo de laboratorio o alerta de aviso | Ingeniero de soporte | Android de guardia (opcional) | Docs/Apoyo para la concientización | Convierta a Sev2 si el alcance se expande o si varios inquilinos se ven afectados. |

| Ventana | Acción | Propietario(s) | Evidencia/Notas |
|--------|--------|----------|----------------|
| 0–5 minutos | Reconozca PagerDuty, asigne un comandante de incidente (IC) y cree `incident/YYYY-MM-DD-android-telemetry.md`. Suelte el enlace más el estado de una línea en `#android-sdk-support`. | SRE de guardia / Ingeniero de soporte | Captura de pantalla del reconocimiento de PagerDuty + resguardo de incidente cometido junto a otros registros de incidentes. |
| 5–15 minutos | Ejecute `scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status` y pegue el resumen en el documento del incidente. Haga ping a Android Observability TL (Haruka Yamamoto) y al líder de soporte (Priya Deshpande) para una transferencia en tiempo real. | IC + Observabilidad de Android TL | Adjunte el JSON de salida de la CLI, observe las URL del panel abiertas y marque quién es el propietario de los diagnósticos. |
| 15-25 minutos | Involucre a los propietarios del clúster de preparación (Haruka Yamamoto para la observabilidad, Liam O'Connor para SRE) para reproducirlo en `android-telemetry-stg`. Carga inicial con `scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg` y captura de volcados de cola desde el emulador Pixel + para confirmar la paridad de síntomas. | Propietarios de clústeres de ensayo | Cargue la salida desinfectada `pending.queue` + `PendingQueueInspector` a la carpeta del incidente. |
| 25–40 minutos | Decida las anulaciones, la limitación Torii o el respaldo de StrongBox. Si se sospecha exposición de PII o hash no determinista, contacte a Cumplimiento (Sofia Martins, Daniel Park) a través de `#compliance-alerts` y notifique al líder del programa en el mismo hilo del incidente. | IC + Cumplimiento + Líder de Programa | Tokens de anulación de enlaces, manifiestos Norito y comentarios de aprobación. |
| ≥40min | Proporcione actualizaciones de estado cada 30 minutos (notas de PagerDuty + `#android-sdk-support`). Programe el puente de la sala de guerra si aún no está activo, documente la ETA de mitigación y asegúrese de que Ingeniería de lanzamiento (Alexei Morozov) esté en espera para lanzar los artefactos del recopilador/SDK. | CI | Actualizaciones con marca de tiempo más registros de decisiones almacenados en el archivo de incidentes y resumidos en `status.md` durante la próxima actualización semanal. |- Todas las escaladas deben permanecer reflejadas en el documento del incidente utilizando la tabla "Propietario/Próxima actualización" del Manual de soporte de Android.
- Si ya hay otro incidente abierto, únase a la sala de guerra existente y agregue el contexto de Android en lugar de crear uno nuevo.
- Cuando el incidente afecte a las lagunas del runbook, cree tareas de seguimiento en la epopeya AND7 JIRA y etiquete `telemetry-runbook`.

## 5. Ejercicios de caos y preparación

- Ejecutar los escenarios detallados en
  `docs/source/sdk/android/telemetry_chaos_checklist.md` trimestralmente y antes de
  lanzamientos importantes. Registre los resultados con la plantilla de informe de laboratorio.
- Almacenar evidencia (capturas de pantalla, registros) en
  `docs/source/sdk/android/readiness/screenshots/`.
- Seguimiento de tickets de remediación en la epopeya AND7 con etiqueta `telemetry-lab`.
- Mapa de escenario: C1 (fallo de redacción), C2 (anulación), C3 (caída del exportador), C4
  (puerta de diferenciación de esquema usando `run_schema_diff.sh` con una configuración derivada), C5
  (desviación del perfil del dispositivo sembrada a través de `generate_android_load.sh`), C6 (tiempo de espera de Torii
  + repetición de cola), C7 (rechazo de atestación). Mantenga esta numeración alineada con
  `telemetry_lab_01.md` y la lista de verificación del caos al agregar ejercicios.

### 5.1 Ejercicio de derivación y anulación de redacción (C1/C2)

1. Inyecte un error de hash a través de
   `scripts/telemetry/inject_redaction_failure.sh` y espere el PagerDuty
   alerta (`android.telemetry.redaction.failure`). Capture la salida CLI de
   `scripts/telemetry/check_redaction_status.py --status-url <collector>` para
   el registro del incidente.
2. Borre la falla con `--clear` y confirme que la alerta se resuelva dentro de
   10 minutos; adjunte capturas de pantalla Grafana de los paneles de sal/autoridad.
3. Cree una solicitud de anulación firmada usando
   `docs/examples/android_override_request.json`, aplicarlo con
   `scripts/android_override_tool.sh apply` y verifique la muestra sin procesar mediante
   inspeccionar la carga útil del exportador en la etapa de preparación (busque
   `android.telemetry.redaction.override`).
4. Revocar la anulación con `scripts/android_override_tool.sh revoke --token <token>`,
   agregue el hash del token de anulación más la referencia del ticket a
   `docs/source/sdk/android/telemetry_override_log.md` y crear un resumen JSON
   bajo `docs/source/sdk/android/readiness/override_logs/`. Esto cierra el
   Escenario C2 en la lista de verificación del caos y mantiene actualizada la evidencia de gobernanza.

### 5.2 Caída del exportador y simulacro de repetición de cola (C3/C6)1. Reduzca la escala del recopilador de etapas (`escala kubectl
   implementar/android-otel-collector --replicas=0`) para simular un exportador
   apagón. Realice un seguimiento de las métricas del búfer a través de la CLI de estado y confirme que se activen las alertas en
   la marca de los 15 minutos.
2. Restaure el recopilador, confirme el drenaje del trabajo pendiente y archive el registro del recopilador.
   Fragmento que muestra la finalización de la repetición.
3. Tanto en el píxel de prueba como en el emulador, siga el Escenario C6: instalar
   `examples/android/operator-console`, alternar modo avión, enviar la demostración
   transferencias, luego deshabilite el modo avión y observe las métricas de profundidad de la cola.
4. Extraiga cada cola pendiente (`adb shell run-as  cat files/pending.queue >
   /tmp/.queue`), compile the inspector (`gradle -p java/iroha_android
   :core:clases >/dev/null`), and run `java -cp build/clases
   org.hyperledger.iroha.android.tools.PendingQueueInspector --archivo
   /tmp/.queue --json > queue-replay-.json`. Adjuntar decodificado
   sobres más hashes de repetición en el registro del laboratorio.
5. Actualice el informe de caos con la duración de la interrupción del exportador, la profundidad de la cola antes/después,
   y confirmación de que `android_sdk_offline_replay_errors` seguía siendo 0.

### 5.3 Script de preparación del caos del clúster (android-telemetry-stg)

Propietarios del clúster de puesta en escena Haruka Yamamoto (Android Observability TL) y Liam O'Connor
(SRE) siga este guión siempre que esté programado un ensayo. La secuencia se mantiene
participantes alineados con la lista de verificación del caos de telemetría al tiempo que garantizan que
los artefactos se capturan para la gobernanza.

**Participantes**

| Rol | Responsabilidades | Contacto |
|------|------------------|---------|
| IC de guardia de Android | Conduce el taladro, coordina las notas de PagerDuty, posee el registro de comandos | Buscapersonas `android-telemetry-primary`, `#android-sdk-support` |
| Propietarios del clúster de puesta en escena (Haruka, Liam) | Ventanas de cambio de puerta, ejecutar acciones `kubectl`, telemetría de clúster de instantáneas | `#android-staging` |
| Gerente de Documentos/Soporte (Priya) | Registrar evidencia, realizar un seguimiento de la lista de verificación de los laboratorios, publicar tickets de seguimiento | `#docs-support` |

**Coordinación previa al vuelo**

- 48 horas antes del simulacro, presente una solicitud de cambio que enumere lo planeado.
  escenarios (C1–C7) y pegue el enlace en `#android-staging` para que los propietarios del clúster
  puede bloquear implementaciones conflictivas.
- Recopile el último hash `ClientConfig` y `kubectl --context staging get pods
  -n salida android-telemetry-stg` para establecer el estado de referencia y luego almacenar
  ambos bajo `docs/source/sdk/android/readiness/labs/reports/<date>/`.
- Confirme la cobertura del dispositivo (Pixel + emulador) y asegúrese
  `ci/run_android_tests.sh` compiló las herramientas utilizadas durante el laboratorio.
  (`PendingQueueInspector`, inyectores de telemetría).

**Puntos de control de ejecución**

- Anuncie “inicio del caos” en `#android-sdk-support`, comience la grabación del puente,
  y mantenga `docs/source/sdk/android/telemetry_chaos_checklist.md` visible para que
  cada orden es narrada para el escriba.
- Haga que un propietario provisional refleje cada acción del inyector (`kubectl scale`, exportador
  se reinicia, carga generadores) para que tanto Observability como SRE confirmen el paso.
- Capture la salida de `scripts/telemetry/check_redaction_status.py
  --status-url https://android-telemetry-stg/api/redaction/status` después de cada
  escenario y péguelo en el documento del incidente.

**Recuperación**- No abandone el puente hasta que se hayan limpiado todos los inyectores (`inject_redaction_failure.sh --clear`,
  `kubectl scale ... --replicas=1`) e Grafana muestran el estado verde.
- Volcados de cola de archivos de Docs/Support, registros CLI y capturas de pantalla en
  `docs/source/sdk/android/readiness/screenshots/<date>/` y marca el archivo
  lista de verificación antes de que se cierre la solicitud de cambio.
- Registre tickets de seguimiento con la etiqueta `telemetry-chaos` para cualquier escenario que
  falló o produjo métricas inesperadas y haga referencia a ellas en `status.md`
  durante la próxima revisión semanal.

| Hora | Acción | Propietario(s) | Artefacto |
|------|--------|----------|----------|
| T-30 min | Verifique el estado de `android-telemetry-stg`: `kubectl --context staging get pods -n android-telemetry-stg`, confirme que no haya actualizaciones pendientes y anote las versiones del recopilador. | Haruka | `docs/source/sdk/android/readiness/screenshots/<date>/cluster-health.png` |
| T-20 min | Carga de línea base inicial (`scripts/telemetry/generate_android_load.sh --cluster android-telemetry-stg --duration 20m`) y salida estándar de captura. | Liam | `readiness/labs/reports/<date>/load-generator.log` |
| T-15min | Copie `docs/source/sdk/android/readiness/incident/telemetry_chaos_template.md` a `docs/source/sdk/android/readiness/incident/<date>-telemetry-chaos.md`, enumere los escenarios para ejecutar (C1-C7) y asigne escribas. | Priya Deshpande (Apoyo) | Rebaja del incidente cometido antes de que comience el ensayo. |
| T-10 min | Confirme el emulador Pixel + en línea, el último SDK instalado y `ci/run_android_tests.sh` compiló el `PendingQueueInspector`. | Haruka, Liam | `readiness/screenshots/<date>/device-checklist.png` |
| T-5mín | Inicie Zoom Bridge, comience a grabar la pantalla y anuncie "inicio del caos" en `#android-sdk-support`. | IC / Documentos/Soporte | Grabación guardada en `readiness/archive/<month>/`. |
| +0min | Ejecute el escenario seleccionado de `docs/source/sdk/android/readiness/labs/telemetry_lab_01.md` (normalmente C2 + C6). Mantenga la guía de laboratorio visible y diga las invocaciones de comandos a medida que ocurren. | Haruka conduce, Liam refleja los resultados | Registros adjuntos al expediente de incidencias en tiempo real. |
| +15min | Haga una pausa para recopilar métricas (`scripts/telemetry/check_redaction_status.py --status-url https://android-telemetry-stg/api/redaction/status`) y obtenga capturas de pantalla de Grafana. | Haruka | `readiness/screenshots/<date>/status-<scenario>.png` |
| +25min | Restaure cualquier falla inyectada (`inject_redaction_failure.sh --clear`, `kubectl scale ... --replicas=1`), reproduzca colas y confirme el cierre de las alertas. | Liam | `readiness/labs/reports/<date>/recovery.log` |
| +35min | Informe: actualice el documento de incidentes con aprobación/falla por escenario, enumere los seguimientos y envíe artefactos a git. Notifique a Docs/Support que se puede completar la lista de verificación del archivo. | CI | Documento de incidente actualizado, `readiness/archive/<month>/checklist.md` marcado. |

- Mantenga a los propietarios provisionales en el puente hasta que los exportadores estén sanos y se hayan eliminado todas las alertas.
- Almacene volcados de cola sin procesar en `docs/source/sdk/android/readiness/labs/reports/<date>/queues/` y haga referencia a sus hashes en el registro de incidentes.
- Si un escenario falla, cree inmediatamente un ticket JIRA con la etiqueta `telemetry-chaos` y vincúlelo desde `status.md`.
- Ayudante de automatización: `ci/run_android_telemetry_chaos_prep.sh` envuelve el generador de carga, las instantáneas de estado y la tubería de exportación de colas. Configure `ANDROID_TELEMETRY_DRY_RUN=false` cuando el acceso provisional esté disponible y `ANDROID_PENDING_QUEUE_EXPORTS=pixel8=/tmp/pixel.queue,emulator=/tmp/emulator.queue` (etc.) para que el script copie cada archivo de cola, emita `<label>.sha256` y ejecute `PendingQueueInspector` para producir `<label>.json`. Utilice `ANDROID_PENDING_QUEUE_INSPECTOR=false` solo cuando se deba omitir la emisión JSON (por ejemplo, no hay JDK disponible). **Exporte siempre los identificadores de sal esperados antes de ejecutar el asistente** configurando `ANDROID_TELEMETRY_EXPECTED_SALT_EPOCH=<YYYYQ#>` e `ANDROID_TELEMETRY_EXPECTED_SALT_ROTATION=<id>` para que las llamadas `check_redaction_status.py` integradas fallen rápidamente si la telemetría capturada difiere de la línea base de Rust.

## 6. Documentación y habilitación- **Kit de habilitación del operador:** `docs/source/sdk/android/readiness/and7_operator_enablement.md`
  vincula el runbook, la política de telemetría, la guía de laboratorio, la lista de verificación de archivos y el conocimiento
  se registra en un único paquete listo para AND7. Consúltelo al preparar SRE
  lecturas previas de gobernanza o programación de la actualización trimestral.
- **Sesiones de habilitación:** El 18 de febrero de 2026 se realizará una grabación de habilitación de 60 minutos.
  con actualizaciones trimestrales. Los materiales viven debajo.
  `docs/source/sdk/android/readiness/`.
- **Verificaciones de conocimientos:** El personal debe obtener una puntuación ≥90% a través del formulario de preparación. Tienda
  da como resultado `docs/source/sdk/android/readiness/forms/responses/`.
- **Actualizaciones:** Siempre que se anulen esquemas de telemetría, paneles o políticas
  cambiar, actualice este runbook, el playbook de soporte e `status.md` en el mismo
  PR.
- **Revisión semanal:** Después de cada lanzamiento candidato de Rust (o al menos semanalmente), verifique
  `java/iroha_android/README.md` y este runbook aún reflejan la automatización actual,
  procedimientos de rotación de partidos y expectativas de gobernanza. Capture la reseña en
  `status.md` para que la auditoría de hitos de Foundations pueda rastrear la actualización de la documentación.

## 7. Arnés de certificación StrongBox- **Propósito:** Validar paquetes de atestación respaldados por hardware antes de promocionar dispositivos en el
  Grupo StrongBox (AND2/AND6). El arnés consume cadenas de certificados capturadas y las verifica.
  contra raíces confiables usando la misma política que ejecuta el código de producción.
- **Referencia:** Consulte `docs/source/sdk/android/strongbox_attestation_harness_plan.md` para obtener la información completa.
  API de captura, ciclo de vida de alias, cableado CI/Buildkite y matriz de propiedad. Trate ese plan como el
  fuente de confianza al incorporar nuevos técnicos de laboratorio o actualizar artefactos de finanzas/cumplimiento.
- **Flujo de trabajo:**
  1. Recopile un paquete de atestación en el dispositivo (alias, `challenge.hex` e `chain.pem` con el
     orden hoja→raíz) y cópielo a la estación de trabajo.
  2. Ejecute `scripts/android_keystore_attestation.sh --bundle-dir  --trust-root 
     [--trust-root-dir ] --require-strongbox --output ` usando el archivo apropiado
     Raíz de Google/Samsung (los directorios le permiten cargar paquetes completos de proveedores).
  3. Archive el resumen JSON junto con el material de certificación sin procesar en
     `artifacts/android/attestation/<device-tag>/`.
- **Formato del paquete:** Siga `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  para el diseño de archivo requerido (`chain.pem`, `challenge.hex`, `alias.txt`, `result.json`).
- **Raíces confiables:** Obtenga PEM proporcionados por el proveedor del almacén de secretos del laboratorio de dispositivos; pasar múltiples
  argumentos `--trust-root` o apunte `--trust-root-dir` al directorio que contiene los anclajes cuando
  la cadena termina en un ancla que no es de Google.
- **Arnés de CI:** Utilice `scripts/android_strongbox_attestation_ci.sh` para verificar por lotes los paquetes archivados
  en máquinas de laboratorio o corredores de CI. El script escanea `artifacts/android/attestation/**` e invoca el
  arnés para cada directorio que contiene los archivos documentados, escribiendo `result.json` actualizado
  resúmenes en su lugar.
- **Carril CI:** Después de sincronizar nuevos paquetes, ejecute el paso Buildkite definido en
  `.buildkite/android-strongbox-attestation.yml` (`buildkite-agent pipeline upload --pipeline .buildkite/android-strongbox-attestation.yml`).
  El trabajo ejecuta `scripts/android_strongbox_attestation_ci.sh`, genera un resumen con
  `scripts/android_strongbox_attestation_report.py`, sube el informe a `artifacts/android_strongbox_attestation_report.txt`,
  y anota la compilación como `android-strongbox/report`. Investigue cualquier falla inmediatamente y
  vincule la URL de compilación desde la matriz del dispositivo.
- **Informes:** Adjunte la salida JSON a las revisiones de gobernanza y actualice la entrada de la matriz del dispositivo en
  `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` con la fecha de atestación.
- **Ensayo simulado:** Cuando el hardware no esté disponible, ejecute `scripts/android_generate_mock_attestation_bundles.sh`
  (que usa `scripts/android_mock_attestation_der.py`) para crear paquetes de pruebas deterministas más una raíz simulada compartida para que CI y los documentos puedan ejercitar el arnés de un extremo a otro.
- **Valores de seguridad en el código:** `ci/run_android_tests.sh --tests
  org.hyperledger.iroha.android.crypto.keystore.KeystoreKeyProviderTests` cubre vacíos versus desafiados
  regeneración de certificación (metadatos StrongBox/TEE) y emite `android.keystore.attestation.failure`
  en la discrepancia del desafío para que las regresiones de caché/telemetría se detecten antes de enviar nuevos paquetes.

## 8. Contactos

- **Ingeniería de soporte de guardia:** `#android-sdk-support`
- **Gobernanza SRE:** `#sre-governance`
- **Documentos/Soporte:** `#docs-support`
- **Árbol de escalada:** Consulte la guía de soporte de Android §2.1

## 9. Escenarios de solución de problemasEl elemento de la hoja de ruta AND7-P2 menciona tres clases de incidentes que repetidamente paginan el
Android de guardia: Torii/tiempos de espera de red, fallas de certificación de StrongBox y
`iroha_config` deriva manifiesta. Analice la lista de verificación correspondiente antes de presentar la solicitud
Sev1/2 hace seguimiento y archiva la evidencia en `incident/<date>-android-*.md`.

### 9.1 Torii y tiempos de espera de red

**Señales**

- Alertas en `android_sdk_submission_latency`, `android_sdk_pending_queue_depth`,
  `android_sdk_offline_replay_errors` y la tasa de error Torii `/v2/pipeline`.
- Widgets `operator-console` (ejemplos/android) que muestran drenaje de cola estancado o
  Los reintentos se atascan en un retroceso exponencial.

**Respuesta inmediata**

1. Reconozca PagerDuty (`android-networking`) e inicie un registro de incidentes.
2. Capture instantáneas Grafana (latencia de envío + profundidad de cola) que cubran el
   últimos 30 minutos.
3. Registre el hash `ClientConfig` activo de los registros del dispositivo (`ConfigWatcher`
   imprime el resumen del manifiesto cada vez que una recarga tiene éxito o falla).

**Diagnóstico**

- **Estado de la cola:** Extraiga el archivo de cola configurado desde un dispositivo de prueba o el
  emulador (`adb shell run-as  archivos cat/pending.queue >
  /tmp/pending.queue`). Decodifica los sobres con
  `OfflineSigningEnvelopeCodec` como se describe en
  `docs/source/sdk/android/offline_signing.md#4-queueing--replay` para confirmar el
  el trabajo pendiente coincide con las expectativas del operador. Adjunte los hashes decodificados al
  incidente.
- **Inventario de hash:** Después de descargar el archivo de cola, ejecute el asistente del inspector
  para capturar hashes/alias canónicos para los artefactos incidentes:

  ```bash
  gradle -p java/iroha_android :core:classes >/dev/null  # compiles classes if needed
  java -cp build/classes org.hyperledger.iroha.android.tools.PendingQueueInspector \
    --file /tmp/pending.queue --json > queue-inspector.json
  ```

  Adjunte `queue-inspector.json` y la salida estándar bastante impresa al incidente.
  y vincúlelo desde el informe de laboratorio AND7 para el Escenario D.
- **Conectividad Torii:** Ejecute el arnés de transporte HTTP localmente para descartar el SDK
  regresiones: ejercicios `ci/run_android_tests.sh`
  `HttpClientTransportTests`, `HttpClientTransportHarnessTests` y
  `ToriiMockServerTests`. Las fallas aquí indican un error del cliente en lugar de un
  Torii interrupción.
- **Ensayo de inyección de fallos:** En el escenario Pixel (StrongBox) y el AOSP
  emulador, alterne la conectividad para reproducir el crecimiento de la cola pendiente:
  `adb shell cmd connectivity airplane-mode enable` → enviar dos demostraciones
  transacciones a través de la consola del operador → `adb shell cmd conectividad modo avión
  desactivar` → verify the queue drains and `android_sdk_offline_replay_errors`
  permanece 0. Registre los hashes de las transacciones reproducidas.
- **Paridad de alerta:** Al ajustar los umbrales o después de cambios en Torii, ejecute
  `scripts/telemetry/test_torii_norito_rpc_alerts.sh` por lo que las reglas Prometheus permanecen
  alineados con los tableros.

**Recuperación**

1. Si Torii está degradado, active el Torii de guardia y continúe reproduciendo el
   cola una vez que `/v2/pipeline` acepta el tráfico.
2. Reconfigure los clientes afectados únicamente mediante manifiestos `iroha_config` firmados. el
   El observador de recarga en caliente `ClientConfig` debe emitir un registro de éxito antes del incidente
   puede cerrar.
3. Actualice el incidente con el tamaño de la cola antes/después de la reproducción más los hashes de
   cualquier transacción cancelada.

### 9.2 Caja fuerte y fallas de certificación

**Señales**- Alertas en `android_sdk_strongbox_success_rate` o
  `android.keystore.attestation.failure`.
- La telemetría `android.keystore.keygen` ahora registra lo solicitado
  `KeySecurityPreference` y la ruta utilizada (`strongbox`, `hardware`,
  `software`) con un indicador `fallback=true` cuando una preferencia StrongBox aterriza en
  TEE/software. Las solicitudes STRONGBOX_REQUIRED ahora fallan rápidamente en lugar de silenciosamente
  devolver llaves TEE.
- Tickets de soporte que hacen referencia a dispositivos `KeySecurityPreference.STRONGBOX_ONLY`
  recurriendo a las claves de software.

**Respuesta inmediata**

1. Reconozca PagerDuty (`android-crypto`) y capture la etiqueta de alias afectada
   (hachís salado) más depósito de perfil de dispositivo.
2. Verifique la entrada de la matriz de certificación para el dispositivo en
   `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` y
   registre la última fecha verificada.

**Diagnóstico**

- **Verificación del paquete:** Ejecutar
  `scripts/android_keystore_attestation.sh --bundle-dir <bundle> --trust-root <root.pem>`
  en la certificación archivada para confirmar si la falla se debe al dispositivo
  una mala configuración o un cambio de política. Adjunte el `result.json` generado.
- **Regeneración de desafíos:** Los desafíos no se almacenan en caché. Cada solicitud de desafío regenera una nueva
  atestación y cachés por `(alias, challenge)`; Las llamadas sin desafío reutilizan el caché. No compatible
- **Barrido CI:** Ejecute `scripts/android_strongbox_attestation_ci.sh` para que cada
  se revalida el paquete almacenado; esto protege contra problemas sistémicos introducidos
  por nuevas anclas de confianza.
- **Dispositivo taladro:** En hardware sin StrongBox (o forzando el emulador),
  configure el SDK para que solo requiera StrongBox, envíe una transacción de demostración y confirme
  el exportador de telemetría emite el evento `android.keystore.attestation.failure`
  con el motivo esperado. Repita esto en un Pixel compatible con StrongBox para garantizar que
  El camino feliz permanece verde.
- **Verificación de regresión del SDK:** Ejecute `ci/run_android_tests.sh` y pague
  atención a las suites centradas en la certificación (`AndroidKeystoreBackendDetectionTests`,
  `AttestationVerifierTests`, `IrohaKeyManagerDeterministicExportTests`,
  `KeystoreKeyProviderTests` para separación de caché/desafío). Fallos aquí
  indicar una regresión del lado del cliente.

**Recuperación**

1. Regenerar paquetes de atestación si el proveedor rotó los certificados o si el
   El dispositivo recibió recientemente una OTA importante.
2. Cargue el paquete actualizado en `artifacts/android/attestation/<device>/` y
   actualice la entrada de la matriz con la nueva fecha.
3. Si StrongBox no está disponible en producción, siga el flujo de trabajo de anulación en
   Sección 3 y documentar la duración de la reserva; la mitigación a largo plazo requiere
   reemplazo del dispositivo o una reparación del proveedor.

### 9.2a Recuperación determinista de las exportaciones

- **Formatos:** Las exportaciones actuales son v3 (sal por exportación/nonce + Argon2id, registrado como
- **Política de frase de contraseña:** v3 aplica frases de contraseña de ≥12 caracteres. Si los usuarios suministran más cortos
  frases de contraseña, indíqueles que reexporten con una frase de contraseña compatible; Las importaciones v0/v1 son
  está exento pero debe volver a empaquetarse como v3 inmediatamente después de la importación.
- **Protectores contra manipulación/reutilización:** Los decodificadores rechazan longitudes cero/sal corta o nonce y se repiten.
  Los pares salt/nonce aparecen como errores `salt/nonce reuse`. Regenerar la exportación para borrar
  el guardia; no intente forzar la reutilización.
  `SoftwareKeyProvider.importDeterministic(...)` para rehidratar la llave, luego
  `exportDeterministic(...)` para emitir un paquete v3 para que las herramientas de escritorio registren el nuevo KDF
  parámetros.### 9.3 Discrepancias entre manifiesto y configuración

**Señales**

- Errores de recarga de `ClientConfig`, nombres de host de Torii no coincidentes o telemetría
  diferencias de esquema marcadas por la herramienta de diferencias AND7.
- Los operadores informan sobre diferentes perillas de reintento/retroceso en todos los dispositivos del mismo
  flota.

**Respuesta inmediata**

1. Capture el resumen `ClientConfig` impreso en los registros de Android y el
   resumen esperado del manifiesto de lanzamiento.
2. Vuelque la configuración del nodo en ejecución para compararla:
   `iroha_cli config show --actual > /tmp/iroha_config.actual.json`.

**Diagnóstico**

- **Diferencia de esquema:** Ejecute `scripts/telemetry/run_schema_diff.sh --android-config
   --rust-config  --textfile-dir /var/lib/node_exporter/textfile_collector`
  para generar un informe de diferencias Norito, actualice el archivo de texto Prometheus y adjunte el
  Artefacto JSON más evidencia de métricas para el incidente y registro de preparación de telemetría AND7.
- **Validación de manifiesto:** Utilice `iroha_cli runtime capabilities` (o el tiempo de ejecución
  comando de auditoría) para recuperar los hashes criptográficos/ABI anunciados del nodo y garantizar
  coinciden con el manifiesto móvil. Una discrepancia confirma que el nodo se revirtió
  sin volver a emitir el manifiesto de Android.
- **Verificación de regresión del SDK:** `ci/run_android_tests.sh` cubre
  `ClientConfigNoritoRpcTests`, `ClientConfig.ValidationTests` y
  `HttpClientTransportStatusTests`. Los fallos indican que el SDK enviado no puede
  analizar el formato de manifiesto actualmente implementado.

**Recuperación**

1. Regenerar el manifiesto a través de la canalización autorizada (normalmente
   `iroha_cli runtime Capabilities` → manifiesto Norito firmado → paquete de configuración) y
   volver a implementarlo a través del canal del operador. Nunca edite `ClientConfig`
   anula en el dispositivo.
2. Una vez que llegue un manifiesto corregido, esté atento al mensaje `ConfigWatcher` "recargar bien"
   mensaje en cada nivel de flota y cerrar el incidente solo después de la telemetría
   La diferencia de esquema informa la paridad.
3. Registre el hash del manifiesto, la ruta del artefacto de diferenciación del esquema y el enlace del incidente en
   `status.md` en la sección de Android para auditabilidad.

## 10. Plan de estudios de habilitación de operadores

El elemento de la hoja de ruta **AND7** requiere un paquete de capacitación repetible para que los operadores,
ingenieros de soporte, y SRE puede adoptar las actualizaciones de telemetría/redacción sin
conjeturas. Empareja esta sección con
`docs/source/sdk/android/readiness/and7_operator_enablement.md`, que contiene
la lista de verificación detallada y los enlaces de artefactos.

### 10.1 Módulos de sesión (información de 60 minutos)

1. **Arquitectura de telemetría (15 min).** Recorra el búfer del exportador.
   filtro de redacción y herramientas de diferenciación de esquemas. Demostración
   `scripts/telemetry/run_schema_diff.sh --textfile-dir /var/lib/node_exporter/textfile_collector` más
   `scripts/telemetry/check_redaction_status.py` para que los asistentes vean cómo es la paridad
   aplicado.
2. **Runbook + laboratorios de caos (20 min).** Resalte las secciones 2 a 9 de este runbook.
   ensayar un escenario de `readiness/labs/telemetry_lab_01.md` y mostrar cómo
   para archivar artefactos bajo `readiness/labs/reports/<stamp>/`.
3. **Anulación + flujo de trabajo de cumplimiento (10 min).** Revisar las anulaciones de la Sección 3,
   demostrar `scripts/android_override_tool.sh` (aplicar/revocar/digerir), y
   actualice `docs/source/sdk/android/telemetry_override_log.md` más la última
   digerir JSON.
4. **Preguntas y respuestas/verificación de conocimientos (15 min).** Utilice la tarjeta de referencia rápida en
   `readiness/cards/telemetry_redaction_qrc.md` para anclar preguntas, luego
   capturar seguimientos en `readiness/and7_operator_enablement.md`.### 10.2 Cadencia de activos y propietarios

| Activo | Cadencia | Propietario(s) | Ubicación del archivo |
|-------|---------|----------|------------------|
| Tutorial grabado (Zoom/Teams) | Trimestralmente o antes de cada rotación de sal | Observabilidad de Android TL + Administrador de documentos/soporte | `docs/source/sdk/android/readiness/archive/<YYYY-MM>/` (grabación + lista de verificación) |
| Presentación de diapositivas y tarjeta de referencia rápida | Actualizar cada vez que cambie la política/runbook | Administrador de Documentos/Soporte | `docs/source/sdk/android/readiness/deck/` y `/cards/` (exportar PDF + Markdown) |
| Verificación de conocimientos + hoja de asistencia | Después de cada sesión en vivo | Ingeniería de soporte | Bloque de asistencia `docs/source/sdk/android/readiness/forms/responses/` y `and7_operator_enablement.md` |
| Trabajo pendiente de preguntas y respuestas/registro de acciones | Laminación; actualizado después de cada sesión | LLM (DRI en funciones) | `docs/source/sdk/android/readiness/and7_operator_enablement.md` §6 |

### 10.3 Circuito de evidencia y retroalimentación

- Almacenar artefactos de sesión (capturas de pantalla, simulacros de incidentes, exportaciones de cuestionarios) en el
  El mismo directorio fechado utilizado para los ensayos del caos para que la gobernanza pueda auditar ambos.
  pistas de preparación juntas.
- Cuando se complete una sesión, actualice `status.md` (sección de Android) con enlaces a
  el directorio de archivos y anote cualquier seguimiento abierto.
- Las preguntas pendientes de las preguntas y respuestas en vivo deben convertirse en problemas o documentos.
  solicitudes de extracción dentro de una semana; haga referencia a las epopeyas de la hoja de ruta (AND7/AND8) en el
  descripción del ticket para que los propietarios se mantengan alineados.
- Las sincronizaciones de SRE revisan la lista de verificación del archivo más el artefacto de diferenciación de esquema enumerado en
  Apartado 2.3 antes de declarar cerrado el plan de estudios del trimestre.