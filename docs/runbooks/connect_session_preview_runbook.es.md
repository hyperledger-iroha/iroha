---
lang: es
direction: ltr
source: docs/runbooks/connect_session_preview_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b4dbba7711a733a9c2736410db29b035ce8f13bb50b532fe509a6492f239a1fe
source_last_modified: "2025-11-19T04:38:08.010772+00:00"
translation_last_reviewed: 2026-01-01
---

# Runbook de vista previa de sesiones Connect (IOS7 / JS4)

Este runbook documenta el procedimiento end-to-end para preparar, validar y desmontar sesiones de vista previa de Connect segun lo requerido por los hitos de roadmap **IOS7** y **JS4** (`roadmap.md:1340`, `roadmap.md:1656`). Sigue estos pasos cada vez que hagas una demo del strawman de Connect (`docs/source/connect_architecture_strawman.md`), ejecutes los hooks de cola/telemetria prometidos en los roadmaps de SDK, o recolectes evidencia para `status.md`.

## 1. Checklist de preflight

| Item | Detalles | Referencias |
|------|---------|------------|
| Endpoint de Torii + politica Connect | Confirma la URL base de Torii, `chain_id`, y la politica de Connect (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`). Captura el snapshot JSON en el ticket del runbook. | `javascript/iroha_js/src/toriiClient.js`, `docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
| Versiones de fixture + bridge | Anota el hash del fixture Norito y el build del bridge que usaras (Swift requiere `NoritoBridge.xcframework`, JS requiere `@iroha/iroha-js` >= la version que envio `bootstrapConnectPreviewSession`). | `docs/source/sdk/swift/reproducibility_checklist.md`, `javascript/iroha_js/CHANGELOG.md` |
| Dashboards de telemetria | Asegura que los dashboards que grafican `connect.queue_depth`, `connect.queue_overflow_total`, `connect.resume_latency_ms`, `swift.connect.session_event`, etc., esten disponibles (board Grafana `Android/Swift Connect` + snapshots exportados de Prometheus). | `docs/source/connect_architecture_strawman.md`, `docs/source/sdk/swift/telemetry_redaction.md`, `docs/source/sdk/js/quickstart.md` |
| Carpetas de evidencia | Elige un destino como `docs/source/status/swift_weekly_digest.md` (resumen semanal) y `docs/source/sdk/swift/connect_risk_tracker.md` (tracker de riesgo). Guarda logs, capturas de metricas, y acuses bajo `docs/source/sdk/swift/readiness/archive/<date>/connect/`. | `docs/source/status/swift_weekly_digest.md`, `docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. Bootstrap de la sesion de vista previa

1. **Valida politica + cuotas.** Llama:
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ```
   Falla la ejecucion si `queue_max` o el TTL difiere de la config que planeaste probar.
2. **Genera SID/URI deterministas.** El helper `bootstrapConnectPreviewSession` de `@iroha/iroha-js` liga la generacion de SID/URI con el registro de sesiones en Torii; usalo incluso cuando Swift maneje la capa WebSocket.
   ```js
   import {
     ToriiClient,
     bootstrapConnectPreviewSession,
   } from "@iroha/iroha-js";

   const client = new ToriiClient(process.env.TORII_BASE_URL, { chainId: "sora-mainnet" });
   const { preview, session, tokens } = await bootstrapConnectPreviewSession(client, {
     chainId: "sora-mainnet",
     appBundle: "dev.sora.example.dapp",
     walletBundle: "dev.sora.example.wallet",
     register: true,
   });
   console.log("sid", preview.sidBase64Url, "ws url", preview.webSocketUrl);
   ```
   - Configura `register: false` para pruebas en seco de QR/deep-link.
   - Guarda el `sidBase64Url` devuelto, las URLs de deeplink, y el blob `tokens` en la carpeta de evidencia; la revision de governance espera estos artefactos.
3. **Distribuye secretos.** Comparte el URI deeplink con el operador de wallet (dApp Swift de ejemplo, wallet Android, o harness de QA). Nunca pegues tokens en el chat; usa el vault cifrado documentado en el paquete de enablement.

## 3. Conduce la sesion

1. **Abre el WebSocket.** Los clientes Swift tipicamente usan:
   ```swift
   let connectURL = URL(string: preview.webSocketUrl)!
   let client = ConnectClient(url: connectURL)
   let sid: Data = /* decode preview.sidBase64Url into raw bytes using your harness helper */
   let session = ConnectSession(sessionID: sid, client: client)
   let recorder = ConnectReplayRecorder(sessionID: sid)
   session.addObserver(ConnectEventObserver(queue: .main) { event in
       logger.info("connect event", metadata: ["kind": "\(event.kind)"])
   })
   try client.open()
   ```
   Consulta `docs/connect_swift_integration.md` para configuracion adicional (imports del bridge, adaptadores de concurrencia).
2. **Aprueba + firma flujos.** Las dApps llaman `ConnectSession.requestSignature(...)`, mientras que las wallets responden via `approveSession` / `reject`. Cada aprobacion debe registrar el alias hasheado + permisos para cumplir con el charter de governance de Connect.
3. **Ejercita rutas de cola + reanudacion.** Alterna conectividad de red o suspende la wallet para asegurar que la cola acotada y los hooks de replay registren entradas. Los SDKs JS/Android emiten `ConnectQueueError.overflow(limit)` / `.expired(ttlMs)` cuando descartan frames; Swift deberia observar lo mismo una vez que llegue el andamiaje de cola IOS7 (`docs/source/connect_architecture_strawman.md`). Despues de registrar al menos una reconexion, ejecuta
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   (o pasa el directorio de exportacion devuelto por `ConnectSessionDiagnostics`) y adjunta la tabla/JSON renderizados al ticket del runbook. El CLI lee el mismo par `state.json` / `metrics.ndjson` que produce `ConnectQueueStateTracker`, asi que los revisores de governance pueden trazar evidencia del drill sin tooling a medida.

## 4. Telemetria y observabilidad

- **Metricas a capturar:**
  - `connect.queue_depth{direction}` gauge (debe mantenerse por debajo del cap de politica).
  - `connect.queue_dropped_total{reason="overflow|ttl"}` counter (no-cero solo durante inyeccion de fallos).
  - `connect.resume_latency_ms` histogram (registra el p95 despues de forzar una reconexion).
  - `connect.replay_success_total` / `connect.replay_error_total`.
  - Swift-specific `swift.connect.session_event` y `swift.connect.frame_latency` exports (`docs/source/sdk/swift/telemetry_redaction.md`).
- **Dashboards:** Actualiza los bookmarks del tablero Connect con marcadores de anotacion. Adjunta capturas (o exports JSON) a la carpeta de evidencia junto con los snapshots OTLP/Prometheus extraidos via la CLI del exporter de telemetria.
- **Alerting:** Si se disparan umbrales Sev 1/2 (ver `docs/source/android_support_playbook.md` sec. 5), pagea al SDK Program Lead y documenta el ID de incidente de PagerDuty en el ticket del runbook antes de continuar.

## 5. Limpieza y rollback

1. **Borra sesiones en staging.** Siempre borra las sesiones de preview para que las alarmas de profundidad de cola sigan siendo significativas:
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   Para pruebas solo-Swift, llama el mismo endpoint mediante el helper Rust/CLI.
2. **Purga journals.** Elimina cualquier journal de cola persistido (`ApplicationSupport/ConnectQueue/<sid>.to`, stores de IndexedDB, etc.) para que la siguiente corrida arranque limpia. Registra el hash del archivo antes de borrar si necesitas depurar un problema de replay.
3. **Registra notas del incidente.** Resume la corrida en:
   - `docs/source/status/swift_weekly_digest.md` (bloque de deltas),
   - `docs/source/sdk/swift/connect_risk_tracker.md` (limpia o degrada CR-2 una vez que la telemetria este lista),
   - el changelog del SDK JS o la receta si se valido nuevo comportamiento.
4. **Escala fallas:**
   - Desborde de cola sin fallos inyectados => abre un bug contra el SDK cuya politica diverge de Torii.
   - Errores de reanudacion => adjunta snapshots de `connect.queue_depth` + `connect.resume_latency_ms` al reporte de incidente.
   - Desajustes de governance (tokens reutilizados, TTL excedido) => escala con el SDK Program Lead y anota `roadmap.md` en la siguiente revision.

## 6. Checklist de evidencia

| Artefacto | Ubicacion |
|----------|----------|
| SID/deeplink/tokens JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
| Dashboard exports (`connect.queue_depth`, etc.) | `.../metrics/` subfolder |
| PagerDuty / incident IDs | `.../notes.md` |
| Confirmacion de limpieza (Torii delete, journal wipe) | `.../cleanup.log` |

Completar este checklist cumple el criterio de salida "docs/runbooks updated" para IOS7/JS4 y ofrece a los revisores de governance un rastro determinista para cada sesion de vista previa de Connect.
