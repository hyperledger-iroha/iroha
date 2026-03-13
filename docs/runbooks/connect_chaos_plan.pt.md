---
lang: pt
direction: ltr
source: docs/runbooks/connect_chaos_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b1d414173d2f43d403a6a1ba5cd59a645cb0b94f5765e69a00f7078b1e96b1cd
source_last_modified: "2025-11-18T04:13:57.609769+00:00"
translation_last_reviewed: 2026-01-01
---

# Plano de ensaio de caos e falhas do Connect (IOS3 / IOS7)

Este playbook define os drills de caos repetiveis que cumprem a acao de roadmap _"plan joint chaos rehearsal"_ (`roadmap.md:1527`). Combine-o com o runbook de preview do Connect (`docs/runbooks/connect_session_preview_runbook.md`) ao fazer demos cross-SDK.

## Objetivos e criterios de sucesso
- Exercitar a politica compartilhada de retry/back-off do Connect, limites de fila offline e exporters de telemetria sob falhas controladas sem alterar o codigo de producao.
- Capturar artefatos deterministas (saida de `iroha connect queue inspect`, snapshots de metricas `connect.*`, logs de SDK Swift/Android/JS) para que a governance possa auditar cada drill.
- Provar que wallets e dApps respeitam mudancas de config (manifest drift, rotacao de sal, falhas de attestation) ao expor a categoria canonica `ConnectError` e eventos de telemetria seguros para redacao.

## Prerequisitos
1. **Bootstrap de ambiente**
   - Inicie a stack demo do Torii: `scripts/ios_demo/start.sh --telemetry-profile full`.
   - Inicie pelo menos um sample de SDK (`examples/ios/NoritoDemoXcode/NoritoDemoXcode`,
     `examples/ios/NoritoDemo`, Android `demo-connect`, JS `examples/connect`).
2. **Instrumentacao**
   - Habilite diagnosticos do SDK (`ConnectQueueDiagnostics`, `ConnectQueueStateTracker`,
     `ConnectSessionDiagnostics` no Swift; equivalentes `ConnectQueueJournal` + `ConnectQueueJournalTests`
     em Android/JS).
   - Garanta que o CLI `iroha connect queue inspect --sid <sid> --metrics` resolva
     o caminho da fila produzido pelo SDK (`~/.iroha/connect/<sid>/state.json` e
     `metrics.ndjson`).
   - Conecte exporters de telemetria para que as series a seguir estejam visiveis no
     Grafana e via `scripts/swift_status_export.py telemetry`: `connect.queue_depth`,
     `connect.queue_dropped_total`, `connect.reconnects_total`,
     `connect.resume_latency_ms`, `swift.connect.frame_latency`,
     `android.telemetry.redaction.salt_version`.
3. **Pastas de evidencia** - crie `artifacts/connect-chaos/<date>/` e guarde:
   - logs brutos (`*.log`), snapshots de metricas (`*.json`), exports de dashboard
     (`*.png`), saidas de CLI e IDs de PagerDuty.

## Matriz de cenarios

| ID | Falha | Passos de injecao | Sinais esperados | Evidencia |
|----|-------|-------------------|------------------|----------|
| C1 | Queda de WebSocket e reconexao | Envolva `/v2/connect/ws` atras de um proxy (ex: `kubectl -n demo port-forward svc/torii 18080:8080` + `toxiproxy-cli toxic add ... timeout`) ou bloqueie temporariamente o servico (`kubectl scale deploy/torii --replicas=0` por <=60 s). Force a wallet a continuar enviando frames para preencher as filas offline. | `connect.reconnects_total` incrementa, `connect.resume_latency_ms` tem picos mas fica <1 s p95, as filas entram em `state=Draining` via `ConnectQueueStateTracker`. Os SDKs emitem `ConnectError.Transport.reconnecting` uma vez e retomam. | - Saida de `iroha connect queue inspect --sid <sid>` mostrando `resume_attempts_total` nao-zero.<br>- Anotacao de dashboard para a janela de outage.<br>- Trecho de logs com mensagens de reconnect + drain. |
| C2 | Overflow de fila offline / expiracao TTL | Ajuste o sample para reduzir limites de fila (Swift: instanciar `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)` dentro de `ConnectSessionDiagnostics`; Android/JS usam construtores equivalentes). Suspenda a wallet por >=2x `retentionInterval` enquanto a dApp continua enfileirando requests. | `connect.queue_dropped_total{reason="overflow"}` e `{reason="ttl"}` incrementam, `connect.queue_depth` estabiliza no novo limite, os SDKs exibem `ConnectError.QueueOverflow(limit: 4)` (ou `.QueueExpired`). `iroha connect queue inspect` mostra `state=Overflow` com marcas `warn/drop` a 100%. | - Captura dos contadores de metricas.<br>- JSON do CLI capturando o overflow.<br>- Log Swift/Android com a linha `ConnectError`. |
| C3 | Manifest drift / rejeicao de admission | Altere o manifest Connect servido as wallets (ex: modifique o sample manifest `docs/connect_swift_ios.md`, ou inicie o Torii com `--connect-manifest-path` apontando para uma copia onde `chain_id` ou `permissions` diferem). Faca a dApp pedir aprovacao e confirme que a wallet rejeita por politica. | Torii retorna `HTTP 409` para `/v2/connect/session` com `manifest_mismatch`, os SDKs emitem `ConnectError.Authorization.manifestMismatch(manifestVersion)`, a telemetria eleva `connect.manifest_mismatch_total`, e as filas ficam vazias (`state=Idle`). | - Trecho de logs do Torii mostrando deteccao de mismatch.<br>- Captura do SDK com o erro exposto.<br>- Snapshot de metricas provando que nao houve frames na fila durante o teste. |
| C4 | Rotacao de chave / salto de versao de sal | Rode o sal ou chave AEAD do Connect no meio da sessao. Em stacks dev, reinicie o Torii com `CONNECT_SALT_VERSION=$((old+1))` (espelha o teste de sal Android em `docs/source/sdk/android/telemetry_schema_diff.md`). Mantenha a wallet offline ate a rotacao terminar, depois retome. | A primeira retomada falha com `ConnectError.Authorization.invalidSalt`, as filas esvaziam (a dApp descarta frames em cache com razao `salt_version_mismatch`), a telemetria emite `android.telemetry.redaction.salt_version` (Android) e `swift.connect.session_event{event="salt_rotation"}`. A segunda sessao apos refresh do SID tem sucesso. | - Anotacao de dashboard com a epoca de sal antes/depois.<br>- Logs com o erro invalid-salt e o sucesso posterior.<br>- Saida de `iroha connect queue inspect` mostrando `state=Stalled` e depois `state=Active`. |
| C5 | Falha de attestation / StrongBox | Em wallets Android, configure `ConnectApproval` para incluir `attachments[]` + attestation StrongBox. Use o harness de attestation (`scripts/android_keystore_attestation.sh` com `--inject-failure strongbox-simulated`) ou altere o JSON de attestation antes de entregar a dApp. | A dApp rejeita a aprovacao com `ConnectError.Authorization.invalidAttestation`, o Torii registra a razao da falha, os exporters incrementam `connect.attestation_failed_total`, e a fila remove a entrada ofensiva. As dApps Swift/JS registram o erro mantendo a sessao ativa. | - Log do harness com o ID de falha injetado.<br>- Log de erro do SDK + captura do contador de telemetria.<br>- Evidencia de que a fila removeu o frame ruim (`recordsRemoved > 0`). |

## Detalhes dos cenarios

### C1 - Queda de WebSocket e reconexao
1. Envolva o Torii atras de um proxy (toxiproxy, Envoy, ou um `kubectl port-forward`) para
   alternar a disponibilidade sem derrubar o node inteiro.
2. Dispare uma queda de 45 s:
   ```bash
   toxiproxy-cli toxic add connect-ws --type timeout --toxicity 1.0 --attribute timeout=45000
   sleep 45 && toxiproxy-cli toxic remove connect-ws --toxic timeout
   ```
3. Observe dashboards de telemetria e `scripts/swift_status_export.py telemetry
   --json-out artifacts/connect-chaos/<date>/c1_metrics.json`.
4. Despeje o estado da fila imediatamente apos a queda:
   ```bash
   iroha connect queue inspect --sid "$SID" --metrics > artifacts/connect-chaos/<date>/c1_queue.txt
   ```
5. Sucesso = uma unica tentativa de reconexao, crescimento de fila limitado e drain
   automatico apos o proxy voltar.

### C2 - Overflow de fila offline / expiracao TTL
1. Reduza os limites de fila em builds locais:
   - Swift: atualize o inicializador de `ConnectQueueJournal` no sample
     (ex: `examples/ios/NoritoDemoXcode/NoritoDemoXcode/ContentView.swift`)
     para passar `ConnectQueueJournal.Configuration(maxRecordsPerQueue: 4, maxBytesPerQueue: 4096, retentionInterval: 30)`.
   - Android/JS: passe a config equivalente ao construir `ConnectQueueJournal`.
2. Suspenda a wallet (background do simulador ou modo aviao) por >=60 s
   enquanto a dApp emite chamadas `ConnectClient.requestSignature(...)`.
3. Use `ConnectQueueDiagnostics`/`ConnectQueueStateTracker` (Swift) ou o helper
   de diagnosticos JS para exportar o bundle de evidencia (`state.json`, `journal/*.to`,
   `metrics.ndjson`).
4. Sucesso = contadores de overflow incrementam, o SDK mostra `ConnectError.QueueOverflow`
   uma vez, e a fila se recupera ao retomar a wallet.

### C3 - Manifest drift / rejeicao de admission
1. Crie uma copia do manifest de admission, por exemplo:
   ```bash
   cp configs/connect_manifest.json /tmp/manifest_drift.json
   sed -i '' 's/"chain_id": ".*"/"chain_id": "bogus-chain"/' /tmp/manifest_drift.json
   ```
2. Inicie o Torii com `--connect-manifest-path /tmp/manifest_drift.json` (ou
   atualize a config docker compose/k8s para o drill).
3. Tente iniciar uma sessao a partir da wallet; espere HTTP 409.
4. Capture logs do Torii + SDK e `connect.manifest_mismatch_total` no dashboard
   de telemetria.
5. Sucesso = rejeicao sem crescimento de fila e a wallet mostra o erro de
   taxonomia compartilhada (`ConnectError.Authorization.manifestMismatch`).

### C4 - Rotacao de chave / salto de sal
1. Registre a versao atual de sal na telemetria:
   ```bash
   scripts/swift_status_export.py telemetry --json-out artifacts/connect-chaos/<date>/c4_before.json
   ```
2. Reinicie o Torii com um novo sal (`CONNECT_SALT_VERSION=$((OLD+1))` ou atualize o
   config map). Mantenha a wallet offline ate o reinicio terminar.
3. Retome a wallet; a primeira tentativa deve falhar com erro invalid-salt
   e `connect.queue_dropped_total{reason="salt_version_mismatch"}` incrementa.
4. Force o app a descartar frames em cache removendo o diretorio da sessao
   (`rm -rf ~/.iroha/connect/<sid>` ou limpeza de cache por plataforma), depois
   reinicie a sessao com tokens novos.
5. Sucesso = telemetria mostra o salto de sal, o evento invalid-resume e logado
   uma vez, e a proxima sessao tem sucesso sem intervencao manual.

### C5 - Falha de attestation / StrongBox
1. Gere um bundle de attestation usando `scripts/android_keystore_attestation.sh`
   (use `--inject-failure strongbox-simulated` para inverter a assinatura).
2. Faca a wallet anexar esse bundle via sua API `ConnectApproval`; a dApp
   deve validar e rejeitar o payload.
3. Verifique telemetria (`connect.attestation_failed_total`, metricas de incidente
   Swift/Android) e confirme que a fila removeu a entrada envenenada.
4. Sucesso = a rejeicao fica isolada ao approval ruim, as filas permanecem saudaveis
   e o log de attestation fica guardado com a evidencia do drill.

## Checklist de evidencia
- exports `artifacts/connect-chaos/<date>/c*_metrics.json` via
  `scripts/swift_status_export.py telemetry`.
- saidas de CLI (`c*_queue.txt`) de `iroha connect queue inspect`.
- logs de SDK + Torii com timestamps e hashes de SID.
- capturas de dashboard com anotacoes para cada cenario.
- PagerDuty / incident IDs se alertas Sev 1/2 foram disparados.

Completar a matriz uma vez por trimestre satisfaz o gate de roadmap e mostra
que as implementacoes Connect de Swift/Android/JS respondem de forma
deterministica nos modos de falha de maior risco.
