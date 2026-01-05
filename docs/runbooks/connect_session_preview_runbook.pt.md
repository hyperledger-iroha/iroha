---
lang: pt
direction: ltr
source: docs/runbooks/connect_session_preview_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b4dbba7711a733a9c2736410db29b035ce8f13bb50b532fe509a6492f239a1fe
source_last_modified: "2025-11-19T04:38:08.010772+00:00"
translation_last_reviewed: 2026-01-01
---

# Runbook de preview de sessoes Connect (IOS7 / JS4)

Este runbook documenta o procedimento end-to-end para preparar, validar e desmontar sessoes de preview do Connect conforme exigido pelos marcos **IOS7** e **JS4** (`roadmap.md:1340`, `roadmap.md:1656`). Siga estes passos sempre que voce demonstrar o strawman do Connect (`docs/source/connect_architecture_strawman.md`), exercitar os hooks de fila/telemetria prometidos nos roadmaps de SDK, ou coletar evidencia para `status.md`.

## 1. Checklist de preflight

| Item | Detalhes | Referencias |
|------|---------|------------|
| Endpoint Torii + politica Connect | Confirme o URL base do Torii, `chain_id`, e a politica do Connect (`ToriiClient.getConnectStatus()` / `getConnectAppPolicy()`). Capture o snapshot JSON no ticket do runbook. | `javascript/iroha_js/src/toriiClient.js`, `docs/source/sdk/js/quickstart.md#connect-sessions--queueing` |
| Versoes de fixture + bridge | Anote o hash do fixture Norito e o build do bridge que voce usara (Swift requer `NoritoBridge.xcframework`, JS requer `@iroha/iroha-js` >= a versao que entregou `bootstrapConnectPreviewSession`). | `docs/source/sdk/swift/reproducibility_checklist.md`, `javascript/iroha_js/CHANGELOG.md` |
| Dashboards de telemetria | Garanta que os dashboards que graficam `connect.queue_depth`, `connect.queue_overflow_total`, `connect.resume_latency_ms`, `swift.connect.session_event`, etc., estejam acessiveis (board Grafana `Android/Swift Connect` + snapshots exportados do Prometheus). | `docs/source/connect_architecture_strawman.md`, `docs/source/sdk/swift/telemetry_redaction.md`, `docs/source/sdk/js/quickstart.md` |
| Pastas de evidencia | Escolha um destino como `docs/source/status/swift_weekly_digest.md` (resumo semanal) e `docs/source/sdk/swift/connect_risk_tracker.md` (tracker de risco). Guarde logs, capturas de metricas, e confirmacoes em `docs/source/sdk/swift/readiness/archive/<date>/connect/`. | `docs/source/status/swift_weekly_digest.md`, `docs/source/sdk/swift/connect_risk_tracker.md` |

## 2. Bootstrap da sessao de preview

1. **Valide politica + quotas.** Chame:
   ```js
   const status = await torii.getConnectStatus();
   console.log(status.policy.queue_max, status.policy.offline_timeout_ms);
   ```
   Falhe a execucao se `queue_max` ou o TTL diferir da config que voce planejou testar.
2. **Gere SID/URI deterministas.** O helper `bootstrapConnectPreviewSession` do `@iroha/iroha-js` vincula a geracao de SID/URI ao registro de sessao no Torii; use-o mesmo quando o Swift conduzir a camada WebSocket.
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
   - Defina `register: false` para testes a seco de QR/deep-link.
   - Guarde o `sidBase64Url` retornado, as URLs de deeplink, e o blob `tokens` na pasta de evidencia; a revisao de governance espera esses artefatos.
3. **Distribua segredos.** Compartilhe o URI deeplink com o operador da wallet (dApp Swift de exemplo, wallet Android, ou harness de QA). Nunca cole tokens brutos no chat; use o cofre criptografado documentado no pacote de enablement.

## 3. Conduza a sessao

1. **Abra o WebSocket.** Clientes Swift normalmente usam:
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
   Consulte `docs/connect_swift_integration.md` para configuracao adicional (imports do bridge, adaptadores de concorrencia).
2. **Aprovacao + assinatura.** As dApps chamam `ConnectSession.requestSignature(...)`, enquanto as wallets respondem via `approveSession` / `reject`. Cada aprovacao deve registrar o alias com hash + permissoes para cumprir a carta de governance do Connect.
3. **Exercite caminhos de fila + retomada.** Alterne a conectividade de rede ou suspenda a wallet para garantir que a fila limitada e os hooks de replay registrem eventos. Os SDKs JS/Android emitem `ConnectQueueError.overflow(limit)` / `.expired(ttlMs)` quando descartam frames; o Swift deve observar o mesmo quando o scaffolding de fila do IOS7 chegar (`docs/source/connect_architecture_strawman.md`). Depois de registrar ao menos uma reconexao, execute
   ```bash
   iroha connect queue inspect --sid "$SID" --root ~/.iroha/connect --metrics
   ```
   (ou passe o diretorio de exportacao retornado por `ConnectSessionDiagnostics`) e anexe a tabela/JSON renderizados ao ticket do runbook. O CLI le o mesmo par `state.json` / `metrics.ndjson` que o `ConnectQueueStateTracker` produz, de modo que revisores de governance possam rastrear a evidencia do drill sem tooling sob medida.

## 4. Telemetria e observabilidade

- **Metricas a capturar:**
  - `connect.queue_depth{direction}` gauge (deve ficar abaixo do cap de politica).
  - `connect.queue_dropped_total{reason="overflow|ttl"}` counter (nao zero apenas durante injecao de falhas).
  - `connect.resume_latency_ms` histogram (registre o p95 apos forcar uma reconexao).
  - `connect.replay_success_total` / `connect.replay_error_total`.
  - Exports Swift `swift.connect.session_event` e `swift.connect.frame_latency` (`docs/source/sdk/swift/telemetry_redaction.md`).
- **Dashboards:** Atualize os bookmarks do board Connect com marcadores de anotacao. Anexe capturas (ou exports JSON) a pasta de evidencia junto com snapshots OTLP/Prometheus obtidos via a CLI do exporter de telemetria.
- **Alerting:** Se limites Sev 1/2 forem acionados (ver `docs/source/android_support_playbook.md` sec. 5), page o SDK Program Lead e registre o ID de incidente do PagerDuty no ticket do runbook antes de continuar.

## 5. Limpeza e rollback

1. **Delete sessoes em staging.** Sempre delete sessoes de preview para que os alarmes de profundidade de fila continuem significativos:
   ```js
   await client.deleteConnectSession(preview.sidBase64Url);
   ```
   Para testes apenas Swift, chame o mesmo endpoint via o helper Rust/CLI.
2. **Purgue journals.** Remova qualquer journal de fila persistido (`ApplicationSupport/ConnectQueue/<sid>.to`, stores IndexedDB, etc.) para que o proximo run inicie limpo. Registre o hash do arquivo antes de apagar se precisar depurar um problema de replay.
3. **Registre notas do incidente.** Resuma o run em:
   - `docs/source/status/swift_weekly_digest.md` (bloco de deltas),
   - `docs/source/sdk/swift/connect_risk_tracker.md` (limpe ou rebaixe CR-2 quando a telemetria estiver pronta),
   - o changelog do SDK JS ou a receita se novo comportamento foi validado.
4. **Escalacao de falhas:**
   - Overflow de fila sem falhas injetadas => abra um bug contra o SDK cuja politica diverge do Torii.
   - Erros de retomada => anexe snapshots de `connect.queue_depth` + `connect.resume_latency_ms` ao relatorio do incidente.
   - Divergencias de governance (tokens reutilizados, TTL excedido) => escale com o SDK Program Lead e anote `roadmap.md` na proxima revisao.

## 6. Checklist de evidencia

| Artefato | Localizacao |
|----------|----------|
| SID/deeplink/tokens JSON | `docs/source/sdk/swift/readiness/archive/<date>/connect/session.json` |
| Dashboard exports (`connect.queue_depth`, etc.) | `.../metrics/` subfolder |
| PagerDuty / incident IDs | `.../notes.md` |
| Confirmacao de limpeza (Torii delete, journal wipe) | `.../cleanup.log` |

Completar este checklist atende ao criterio de saida "docs/runbooks updated" para IOS7/JS4 e fornece aos revisores de governance uma trilha deterministica para cada sessao de preview do Connect.
