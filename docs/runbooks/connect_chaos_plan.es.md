---
lang: es
direction: ltr
source: docs/runbooks/connect_chaos_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b1d414173d2f43d403a6a1ba5cd59a645cb0b94f5765e69a00f7078b1e96b1cd
source_last_modified: "2025-11-18T04:13:57.609769+00:00"
translation_last_reviewed: 2026-01-01
---

# Plan de ensayo de caos y fallos de Connect (IOS3 / IOS7)

Este playbook define los drills de caos repetibles que cumplen la accion de roadmap _"plan joint chaos rehearsal"_ (`roadmap.md:1527`). Combinalo con el runbook de preview de Connect (`docs/runbooks/connect_session_preview_runbook.md`) cuando hagas demos cross-SDK.

## Objetivos y criterios de exito
- Ejercitar la politica compartida de retry/back-off de Connect, los limites de cola offline y los exporters de telemetria bajo fallos controlados sin mutar codigo de produccion.
- Capturar artefactos deterministas (salida de `iroha connect queue inspect`, snapshots de metricas `connect.*`, logs de SDK Swift/Android/JS) para que governance pueda auditar cada drill.
- Probar que wallets y dApps respetan cambios de config (desfase de manifest, rotacion de sal, fallos de attestation) mostrando la categoria canonica `ConnectError` y eventos de telemetria seguros para redaccion.

## Prerequisitos
1. **Bootstrap de entorno**
   - Inicia el stack demo de Torii: `scripts/ios_demo/start.sh --telemetry-profile full`.
   - Lanza al menos un sample de SDK (`examples/ios/NoritoDemoXcode/NoritoDemoXcode`,
     `examples/ios/NoritoDemo`, Android `demo-connect`, JS `examples/connect`).
2. **Instrumentacion**
   - Habilita diagnosticos del SDK (`ConnectQueueDiagnostics`, `ConnectQueueStateTracker`,
     `ConnectSessionDiagnostics` en Swift; equivalentes `ConnectQueueJournal` + `ConnectQueueJournalTests`
     en Android/JS).
   - Asegura que el CLI `iroha connect queue inspect --sid <sid> --metrics` resuelva
     la ruta de cola producida por el SDK (`~/.iroha/connect/<sid>/state.json` y
     `metrics.ndjson`).
   - Conecta exporters de telemetria para que las siguientes series sean visibles en
     Grafana y via `scripts/swift_status_export.py telemetry`: `connect.queue_depth`,
     `connect.queue_dropped_total`, `connect.reconnects_total`,
     `connect.resume_latency_ms`, `swift.connect.frame_latency`,
     `android.telemetry.redaction.salt_version`.
3. **Carpetas de evidencia** - crea `artifacts/connect-chaos/<date>/` y guarda:
   - logs crudos (`*.log`), snapshots de metricas (`*.json`), exports de dashboard
     (`*.png`), salidas de CLI y IDs de PagerDuty.

## Matriz de escenarios

| ID | Falla | Pasos de inyeccion | Senales esperadas | Evidencia |
|----|-------|--------------------|------------------|----------|
| C1 | Corte de WebSocket y reconexion | Envuelve `/v1/connect/ws` detras de un proxy (p. ej., `kubectl -n demo port-forward svc/torii 18080:8080` + `toxiproxy-cli toxic add ... timeout`) o bloquea temporalmente el servicio (`kubectl scale deploy/torii --replicas=0` por <=60 s). Fuerza a la wallet a seguir enviando frames para que se llenen las colas offline. | `connect.reconnects_total` incrementa, `connect.resume_latency_ms` tiene picos pero queda <1 s p95, las colas entran en `state=Draining` via `ConnectQueueStateTracker`. Los SDKs emiten `ConnectError.Transport.reconnecting` una vez y luego reanudan. | - Salida de `iroha connect queue inspect --sid <sid>` mostrando `resume_attempts_total` no-cero.<br>- Anotacion de dashboard para la ventana de outage.<br>- Extracto de logs con mensajes de reconnect + drain. |
| C2 | Overflow de cola offline / expiracion TTL | Parchea el sample para reducir limites de cola (Swift: instanciar `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` dentro de `ConnectSessionDiagnostics`; Android/JS usan constructores equivalentes). Suspende la wallet por >=2x `retentionInterval` mientras la dApp sigue encolando requests. | `connect.queue_dropped_total{reason="overflow"}` y `{reason="ttl"}` incrementan, `connect.queue_depth` se estabiliza en el nuevo limite, los SDKs muestran `ConnectError.QueueOverflow(limit: 4)` (o `.QueueExpired`). `iroha connect queue inspect` muestra `state=Overflow` con marcas `warn/drop` al 100%. | - Captura de los contadores de metricas.<br>- JSON del CLI capturando el overflow.<br>- Log de Swift/Android con la linea `ConnectError`. |
| C3 | Desfase de manifest / rechazo de admission | Manipula el manifest de Connect servido a wallets (p. ej., modifica el sample manifest `docs/connect_swift_ios.md`, o inicia Torii con `--connect-manifest-path` apuntando a una copia donde `chain_id` o `permissions` difieren). Haz que la dApp pida aprobacion y confirma que la wallet rechaza por politica. | Torii responde `HTTP 409` para `/v1/connect/session` con `manifest_mismatch`, los SDKs emiten `ConnectError.Authorization.manifestMismatch(manifestVersion)`, la telemetria eleva `connect.manifest_mismatch_total`, y las colas quedan vacias (`state=Idle`). | - Extracto de logs de Torii mostrando deteccion de mismatch.<br>- Captura del SDK con el error expuesto.<br>- Snapshot de metricas que pruebe que no hubo frames en cola durante la prueba. |
| C4 | Rotacion de claves / salto de version de sal | Rota la sal o clave AEAD de Connect a mitad de sesion. En stacks dev, reinicia Torii con `CONNECT_SALT_VERSION=$((old+1))` (refleja la prueba de sal de Android en `docs/source/sdk/android/telemetry_schema_diff.md`). Manten la wallet offline hasta que la rotacion termine, luego reanuda. | El primer intento de reanudacion falla con `ConnectError.Authorization.invalidSalt`, las colas se vacian (la dApp descarta frames cacheados con razon `salt_version_mismatch`), la telemetria emite `android.telemetry.redaction.salt_version` (Android) y `swift.connect.session_event{event="salt_rotation"}`. La segunda sesion tras refrescar el SID tiene exito. | - Anotacion de dashboard con la epoca de sal antes/despues.<br>- Logs con el error invalid-salt y el exito posterior.<br>- Salida de `iroha connect queue inspect` mostrando `state=Stalled` y luego `state=Active`. |
| C5 | Fallo de attestation / StrongBox | En wallets Android, configura `ConnectApproval` para incluir `attachments[]` + attestation StrongBox. Usa el harness de attestation (`scripts/android_keystore_attestation.sh` con `--inject-failure strongbox-simulated`) o manipula el JSON de attestation antes de pasarlo a la dApp. | La dApp rechaza la aprobacion con `ConnectError.Authorization.invalidAttestation`, Torii registra la razon del fallo, los exporters incrementan `connect.attestation_failed_total`, y la cola purga la entrada ofensiva. Las dApps Swift/JS registran el error manteniendo la sesion activa. | - Log del harness con el ID de fallo inyectado.<br>- Log de error del SDK + captura del contador de telemetria.<br>- Evidencia de que la cola removio el frame malo (`recordsRemoved > 0`). |

## Detalles de escenarios

### C1 - Corte de WebSocket y reconexion
1. Envuelve Torii detras de un proxy (toxiproxy, Envoy, o un `kubectl port-forward`) para
   poder alternar disponibilidad sin tumbar todo el nodo.
2. Dispara un corte de 45 s:
   ```bash
   toxiproxy-cli toxic add connect-ws --type timeout --toxicity 1.0 --attribute timeout=45000
   sleep 45 && toxiproxy-cli toxic remove connect-ws --toxic timeout
   ```
3. Observa dashboards de telemetria y `scripts/swift_status_export.py telemetry
   --json-out artifacts/connect-chaos/<date>/c1_metrics.json`.
4. Vuelca el estado de la cola inmediatamente despues del corte:
   ```bash
   iroha connect queue inspect --sid "$SID" --metrics > artifacts/connect-chaos/<date>/c1_queue.txt
   ```
5. Exito = un solo intento de reconexion, crecimiento de cola acotado y drenaje
   automatico despues de recuperar el proxy.

### C2 - Overflow de cola offline / expiracion TTL
1. Reduce los umbrales de cola en builds locales:
   - Swift: actualiza el inicializador de `ConnectQueueJournal` dentro del sample
     (p. ej., `examples/ios/NoritoDemoXcode/NoritoDemoXcode/ContentView.swift`)
     para pasar `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)`.
   - Android/JS: pasa el config equivalente al construir `ConnectQueueJournal`.
2. Suspende la wallet (background del simulador o modo avion del dispositivo) por >=60 s
   mientras la dApp emite llamadas `ConnectClient.requestSignature(...)`.
3. Usa `ConnectQueueDiagnostics`/`ConnectQueueStateTracker` (Swift) o el helper de
   diagnosticos JS para exportar el bundle de evidencia (`state.json`, `journal/*.to`,
   `metrics.ndjson`).
4. Exito = los contadores de overflow incrementan, el SDK muestra `ConnectError.QueueOverflow`
   una vez, y la cola se recupera al reanudar la wallet.

### C3 - Desfase de manifest / rechazo de admission
1. Crea una copia del manifest de admission, por ejemplo:
   ```bash
   cp configs/connect_manifest.json /tmp/manifest_drift.json
   sed -i '' 's/"chain_id": ".*"/"chain_id": "bogus-chain"/' /tmp/manifest_drift.json
   ```
2. Inicia Torii con `--connect-manifest-path /tmp/manifest_drift.json` (o
   actualiza la config de docker compose/k8s para el drill).
3. Intenta iniciar una sesion desde la wallet; espera HTTP 409.
4. Captura logs de Torii + SDK y `connect.manifest_mismatch_total` desde
   el dashboard de telemetria.
5. Exito = rechazo sin crecimiento de cola y la wallet muestra el error de
   taxonomia compartida (`ConnectError.Authorization.manifestMismatch`).

### C4 - Rotacion de claves / salto de sal
1. Registra la version actual de sal desde telemetria:
   ```bash
   scripts/swift_status_export.py telemetry --json-out artifacts/connect-chaos/<date>/c4_before.json
   ```
2. Reinicia Torii con una sal nueva (`CONNECT_SALT_VERSION=$((OLD+1))` o actualiza el
   config map). Manten la wallet offline hasta que termine el reinicio.
3. Reanuda la wallet; el primer intento debe fallar con error invalid-salt
   y `connect.queue_dropped_total{reason="salt_version_mismatch"}` incrementa.
4. Fuerza a la app a descartar frames cacheados borrando el directorio de sesion
   (`rm -rf ~/.iroha/connect/<sid>` o el clear de cache por plataforma), luego
   reinicia la sesion con tokens frescos.
5. Exito = la telemetria muestra el salto de sal, el evento invalid-resume se registra
   una vez, y la siguiente sesion tiene exito sin intervencion manual.

### C5 - Fallo de attestation / StrongBox
1. Genera un bundle de attestation usando `scripts/android_keystore_attestation.sh`
   (usa `--inject-failure strongbox-simulated` para alterar la firma).
2. Haz que la wallet adjunte este bundle via su API `ConnectApproval`; la dApp
   debe validar y rechazar el payload.
3. Verifica telemetria (`connect.attestation_failed_total`, metricas de incidente
   Swift/Android) y confirma que la cola elimino la entrada envenenada.
4. Exito = el rechazo queda aislado al approval malo, las colas se mantienen sanas
   y el log de attestation queda guardado con la evidencia del drill.

## Checklist de evidencia
- exports `artifacts/connect-chaos/<date>/c*_metrics.json` desde
  `scripts/swift_status_export.py telemetry`.
- salidas de CLI (`c*_queue.txt`) de `iroha connect queue inspect`.
- logs de SDK + Torii con timestamps y hashes de SID.
- capturas de dashboard con anotaciones para cada escenario.
- PagerDuty / incident IDs si se dispararon alertas Sev 1/2.

Completar la matriz completa una vez por trimestre cumple el gate de roadmap y
muestra que las implementaciones Connect de Swift/Android/JS responden de forma
determinista en los modos de fallo de mayor riesgo.
