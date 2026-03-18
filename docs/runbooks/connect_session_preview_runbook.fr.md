---
lang: fr
direction: ltr
source: docs/runbooks/connect_session_preview_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b4dbba7711a733a9c2736410db29b035ce8f13bb50b532fe509a6492f239a1fe
source_last_modified: "2025-11-19T04:38:08.010772+00:00"
translation_last_reviewed: 2026-01-01
---

# Runbook de previsualisation des sessions Connect (IOS7 / JS4)

Ce runbook documente la procedure end-to-end pour preparer, valider et demanteler les sessions de previsualisation Connect, comme l'exigent les jalons **IOS7** et **JS4** (`roadmap.md:1340`, `roadmap.md:1656`). Suivez ces etapes chaque fois que vous demontrez le strawman Connect (`docs/source/connect_architecture_strawman.md`), exercez les hooks de file/telemetrie promis dans les roadmaps SDK, ou collectez des preuves pour `status.md`.

## 1. Checklist de preflight

| Element | Details | References |
|------|---------|------------|
| Endpoint Torii + politique Connect | Confirmez l'URL de base Torii, `chain_id`, et la politique Connect (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`). Capturez le snapshot JSON dans le ticket du runbook. | `javascript/iroha_js/src/toriiClient.js`, `docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
| Versions du fixture + bridge | Notez le hash du fixture Norito et le build du bridge que vous utiliserez (Swift requiert `NoritoBridge.xcframework`, JS requiert `@iroha/iroha-js` >= la version qui a livre `bootstrapConnectPreviewSession`). | `docs/source/sdk/swift/reproducibility_checklist.md`, `javascript/iroha_js/CHANGELOG.md` |
| Tableaux de bord de telemetrie | Assurez-vous que les tableaux de bord qui tracent `connect.queue_depth`, `connect.queue_overflow_total`, `connect.resume_latency_ms`, `swift.connect.session_event`, etc., sont accessibles (board Grafana `Android/Swift Connect` + snapshots Prometheus exportes). | `docs/source/connect_architecture_strawman.md`, `docs/source/sdk/swift/telemetry_redaction.md`, `docs/source/sdk/js/quickstart.md` |
| Dossiers de preuves | Choisissez une destination telle que `docs/source/status/swift_weekly_digest.md` (digest hebdo) et `docs/source/sdk/swift/connect_risk_tracker.md` (risk tracker). Stockez logs, captures de metriques, et accuses sous `docs/source/sdk/swift/readiness/archive/<date>/connect/`. | `docs/source/status/swift_weekly_digest.md`, `docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. Amorcer la session de previsualisation

1. **Valider la politique + les quotas.** Appelez:
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ```
   Echouez l'execution si `queue_max` ou le TTL differe de la config prevue.
2. **Generer des SID/URI deterministes.** L'aide `bootstrapConnectPreviewSession` de `@iroha/iroha-js` lie la generation de SID/URI a l'enregistrement des sessions Torii; utilisez-la meme quand Swift pilote la couche WebSocket.
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
   - Mettez `register: false` pour des scenarios QR/deep-link en dry-run.
   - Conservez le `sidBase64Url` retourne, les URL deeplink, et le blob `tokens` dans le dossier de preuves; la revue de gouvernance attend ces artefacts.
3. **Distribuer les secrets.** Partagez l'URI deeplink avec l'operateur wallet (sample dApp Swift, wallet Android, ou harness QA). Ne collez jamais les tokens bruts dans le chat; utilisez le coffre chiffre documente dans le packet d'enablement.

## 3. Piloter la session

1. **Ouvrir le WebSocket.** Les clients Swift utilisent en general:
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
   Consultez `docs/connect_swift_integration.md` pour la configuration additionnelle (imports du bridge, adaptateurs de concurrence).
2. **Approuver + signer les flux.** Les dApps appellent `ConnectSession.requestSignature(...)`, tandis que les wallets repondent via `approveSession` / `reject`. Chaque approbation doit journaliser l'alias hashe + permissions pour correspondre a la charte de gouvernance Connect.
3. **Exercer les chemins de file et de reprise.** Basculez la connectivite reseau ou suspendez le wallet pour garantir que la file bornee et les hooks de replay enregistrent des entrees. Les SDK JS/Android emettent `ConnectQueueError.overflow(limit)` / `.expired(ttlMs)` lorsqu'ils abandonnent des frames; Swift devrait observer la meme chose une fois que le squelette de file IOS7 arrive (`docs/source/connect_architecture_strawman.md`). Apres au moins une reconnexion, executez
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   (ou passez le repertoire d'export retourne par `ConnectSessionDiagnostics`) et attachez la table/le JSON rendus au ticket du runbook. Le CLI lit la meme paire `state.json` / `metrics.ndjson` que produit `ConnectQueueStateTracker`, de sorte que les relecteurs de gouvernance peuvent tracer les preuves du drill sans outil sur mesure.

## 4. Telemetrie et observabilite

- **Mesures a capturer:**
  - `connect.queue_depth{direction}` gauge (doit rester sous le cap de politique).
  - `connect.queue_dropped_total{reason="overflow|ttl"}` counter (non nul uniquement pendant l'injection de pannes).
  - `connect.resume_latency_ms` histogram (enregistrer le p95 apres avoir force une reconnexion).
  - `connect.replay_success_total` / `connect.replay_error_total`.
  - Exports Swift `swift.connect.session_event` et `swift.connect.frame_latency` (`docs/source/sdk/swift/telemetry_redaction.md`).
- **Tableaux de bord:** Mettez a jour les bookmarks du board Connect avec des marqueurs d'annotation. Joignez des captures (ou exports JSON) au dossier de preuves avec les snapshots OTLP/Prometheus recuperes via la CLI de l'exporter de telemetrie.
- **Alerting:** Si des seuils Sev 1/2 se declenchent (voir `docs/source/android_support_playbook.md` sec. 5), alertez le SDK Program Lead et consignez l'ID d'incident PagerDuty dans le ticket du runbook avant de continuer.

## 5. Nettoyage et rollback

1. **Supprimez les sessions en staging.** Supprimez toujours les sessions de preview pour que les alarmes de profondeur de file restent utiles:
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   Pour les runs Swift uniquement, appelez le meme endpoint via le helper Rust/CLI.
2. **Purgez les journaux.** Supprimez tout journal de file persistant (`ApplicationSupport/ConnectQueue/<sid>.to`, stores IndexedDB, etc.) afin que le prochain run demarre proprement. Enregistrez le hash du fichier avant suppression si vous devez depanner un probleme de replay.
3. **Consignez les notes d'incident.** Resume le run dans:
   - `docs/source/status/swift_weekly_digest.md` (bloc de deltas),
   - `docs/source/sdk/swift/connect_risk_tracker.md` (nettoyez ou degradez CR-2 une fois la telemetrie en place),
   - le changelog du SDK JS ou la recette si un nouveau comportement a ete valide.
4. **Escalader les echecs:**
   - Debordement de file sans pannes injectees => ouvrez un bug contre le SDK dont la politique diverge de Torii.
   - Erreurs de reprise => joignez des snapshots `connect.queue_depth` + `connect.resume_latency_ms` au rapport d'incident.
   - Ecarts de gouvernance (tokens reutilises, TTL depasse) => escaladez vers le SDK Program Lead et annotez `roadmap.md` a la prochaine revision.

## 6. Checklist des preuves

| Artefact | Emplacement |
|----------|----------|
| SID/deeplink/tokens JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
| Dashboard exports (`connect.queue_depth`, etc.) | `.../metrics/` subfolder |
| PagerDuty / incident IDs | `.../notes.md` |
| Confirmation de nettoyage (Torii delete, journal wipe) | `.../cleanup.log` |

Completer cette checklist satisfait le critere de sortie "docs/runbooks updated" pour IOS7/JS4 et fournit aux relecteurs de gouvernance une piste deterministe pour chaque session de preview Connect.
