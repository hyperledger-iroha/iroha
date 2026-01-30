---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/sorafs/node-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b70bf5f879b8d794392997add8a9952804a4978087d5af51f67002025e496205
source_last_modified: "2025-11-14T04:43:21.893703+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Fonte canonica
Esta pagina espelha `docs/source/sorafs/sorafs_node_plan.md`. Mantenha ambas as copias sincronizadas ate que a documentacao Sphinx alternativa seja retirada.
:::

SF-3 entrega o primeiro crate executavel `sorafs-node` que transforma um processo Iroha/Torii em um provedor de storage SoraFS. Use este plano junto com o [guia de storage do nodo](node-storage.md), a [politica de admissao de provedores](provider-admission-policy.md) e o [roadmap do marketplace de capacidade de storage](storage-capacity-marketplace.md) ao sequenciar entregas.

## Escopo alvo (Marco M1)

1. **Integracao do chunk store.** Envolver `sorafs_car::ChunkStore` com um backend persistente que armazena bytes de chunk, manifests e arvores PoR no diretorio de dados configurado.
2. **Endpoints de gateway.** Expor endpoints HTTP Norito para submissao de pin, fetch de chunks, amostragem PoR e telemetria de storage dentro do processo Torii.
3. **Plumbing de configuracao.** Adicionar uma struct de config `SoraFsStorage` (flag habilitado, capacidade, diretorios, limites de concorrencia) conectada via `iroha_config`, `iroha_core` e `iroha_torii`.
4. **Quota/agendamento.** Impor limites de disco/paralelismo definidos pelo operador e enfileirar requisicoes com back-pressure.
5. **Telemetria.** Emitir metricas/logs para sucesso de pin, latencia de fetch de chunks, utilizacao de capacidade e resultados de amostragem PoR.

## Quebra de trabalho

### A. Estrutura de crate e modulos

| Tarefa | Dono(s) | Notas |
|------|---------|------|
| Criar `crates/sorafs_node` com modulos: `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Storage Team | Reexporta tipos reutilizaveis para integracao com Torii. |
| Implementar `StorageConfig` mapeado de `SoraFsStorage` (user -> actual -> defaults). | Storage Team / Config WG | Garante que as camadas Norito/`iroha_config` permanecam deterministicas. |
| Fornecer uma facade `NodeHandle` que Torii usa para submeter pins/fetches. | Storage Team | Encapsula internos de storage e plumbing async. |

### B. Chunk store persistente

| Tarefa | Dono(s) | Notas |
|------|---------|------|
| Construir um backend em disco envolvendo `sorafs_car::ChunkStore` com indice de manifest em disco (`sled`/`sqlite`). | Storage Team | Layout deterministico: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| Manter metadados PoR (arvores 64 KiB/4 KiB) usando `ChunkStore::sample_leaves`. | Storage Team | Suporta replay apos restart; falha rapido em corrupcao. |
| Implementar replay de integridade no startup (rehash de manifests, podar pins incompletos). | Storage Team | Bloqueia o start de Torii ate o replay terminar. |

### C. Endpoints de gateway

| Endpoint | Comportamento | Tarefas |
|----------|--------------|--------|
| `POST /sorafs/pin` | Aceita `PinProposalV1`, valida manifests, enfileira ingestao, responde com o CID do manifest. | Validar perfil de chunker, impor quotas, stream de dados via chunk store. |
| `GET /sorafs/chunks/{cid}` + query de range | Servir bytes de chunk com headers `Content-Chunker`; respeita a especificacao de capacidade de range. | Usar scheduler + orcamentos de stream (ligar a capacidade de range SF-2d). |
| `POST /sorafs/por/sample` | Rodar amostragem PoR para um manifest e retornar bundle de prova. | Reutilizar amostragem do chunk store, responder com payloads Norito JSON. |
| `GET /sorafs/telemetry` | Resumos: capacidade, sucesso PoR, contadores de erro de fetch. | Fornecer dados para dashboards/operadores. |

O plumbing em runtime passa as interacoes PoR via `sorafs_node::por`: o tracker registra cada `PorChallengeV1`, `PorProofV1` e `AuditVerdictV1` para que as metricas `CapacityMeter` reflitam vereditos de governanca sem logica Torii bespoke. [crates/sorafs_node/src/scheduler.rs:147]

Notas de implementacao:

- Use o stack Axum de Torii com payloads `norito::json`.
- Adicione schemas Norito para respostas (`PinResultV1`, `FetchErrorV1`, structs de telemetria).

- `/v1/sorafs/por/ingestion/{manifest_digest_hex}` agora expoe a profundidade do backlog mais a epoca/deadline mais antiga e os timestamps mais recentes de sucesso/falha por provedor, via `sorafs_node::NodeHandle::por_ingestion_status`, e Torii registra os gauges `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` para dashboards. [crates/sorafs_node/src/lib.rs:510] [crates/iroha_torii/src/sorafs/api.rs:1883] [crates/iroha_torii/src/routing.rs:7244] [crates/iroha_telemetry/src/metrics.rs:5390]

### D. Scheduler e cumprimento de quotas

| Tarefa | Detalhes |
|------|---------|
| Quota de disco | Rastrear bytes em disco; rejeitar novos pins ao exceder `max_capacity_bytes`. Fornecer hooks de eviccao para politicas futuras. |
| Concorrencia de fetch | Semaforo global (`max_parallel_fetches`) mais orcamentos por provedor oriundos de caps de range SF-2d. |
| Fila de pins | Limitar jobs de ingestao pendentes; expor endpoints Norito de status para profundidade da fila. |
| Cadencia PoR | Worker de background dirigido por `por_sample_interval_secs`. |

### E. Telemetria e logging

Metricas (Prometheus):

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (histograma com labels `result`)
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Logs / eventos:

- Telemetria Norito estruturada para ingestao de governanca (`StorageTelemetryV1`).
- Alertas quando utilizacao > 90% ou streak de falhas PoR exceder o threshold.

### F. Estrategia de testes

1. **Testes unitarios.** Persistencia do chunk store, calculos de quota, invariantes do scheduler (ver `crates/sorafs_node/src/scheduler.rs`).
2. **Testes de integracao** (`crates/sorafs_node/tests`). Pin -> fetch round trip, recovery apos restart, rejeicao por quota, verificacao de provas de amostragem PoR.
3. **Testes de integracao Torii.** Rodar Torii com storage habilitado, exercitar endpoints HTTP via `assert_cmd`.
4. **Roadmap de caos.** Drills futuros simulam exaustao de disco, IO lento, remocao de provedores.

## Dependencias

- Politica de admissao SF-2b - garantir que nodes verifiquem envelopes de admissao antes de anunciar.
- Marketplace de capacidade SF-2c - ligar telemetria de volta a declaracoes de capacidade.
- Extensoes de advert SF-2d - consumir capacidade de range + orcamentos de stream quando disponiveis.

## Criterios de saida do marco

- `cargo run -p sorafs_node --example pin_fetch` funciona contra fixtures locais.
- Torii compila com `--features sorafs-storage` e passa testes de integracao.
- Documentacao ([guia de storage do nodo](node-storage.md)) atualizada com defaults de configuracao + exemplos de CLI; runbook de operador disponivel.
- Telemetria visivel em dashboards de staging; alertas configurados para saturacao de capacidade e falhas PoR.

## Entregaveis de documentacao e ops

- Atualizar a [referencia de storage do nodo](node-storage.md) com defaults de configuracao, uso de CLI e passos de troubleshooting.
- Manter o [runbook de operacoes de nodo](node-operations.md) alinhado com a implementacao conforme SF-3 evolui.
- Publicar referencias de API para endpoints `/sorafs/*` dentro do portal do desenvolvedor e conectar ao manifesto OpenAPI assim que handlers de Torii estiverem prontos.
