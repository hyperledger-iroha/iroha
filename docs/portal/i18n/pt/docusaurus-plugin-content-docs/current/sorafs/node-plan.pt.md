---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de nó
título: Plano de implementação do nodo SoraFS
sidebar_label: Plano de implementação do nodo
descrição: Converta o roadmap de armazenamento SF-3 em trabalho de engenharia acionavel com marcos, tarefas e cobertura de testes.
---

:::nota Fonte canônica
Esta página espelha `docs/source/sorafs/sorafs_node_plan.md`. Mantenha ambas as cópias sincronizadas até que a documentação alternativa do Sphinx seja retirada.
:::

SF-3 entrega o primeiro caixote executável `sorafs-node` que transforma um processo Iroha/Torii em um provedor de armazenamento SoraFS. Use este plano junto com o [guia de armazenamento do nodo](node-storage.md), a [política de admissão de provedores](provider-admission-policy.md) e o [roadmap do marketplace de capacidade de armazenamento](storage-capacity-marketplace.md) ao entregas sequenciais.

## Escopo alvo (Marco M1)

1. **Integração do chunk store.** Envolver `sorafs_car::ChunkStore` com um backend persistente que armazena bytes de chunk, manifestos e árvores PoR no diretório de dados configurado.
2. **Endpoints de gateway.** Exportar endpoints HTTP Norito para envio de pin, busca de chunks, amostragem PoR e telemetria de armazenamento dentro do processo Torii.
3. **Plumbing de configuração.** Adicione uma struct de configuração `SoraFsStorage` (flag habilitado, capacidade, diretórios, limites de concorrência) conectada via `iroha_config`, `iroha_core` e `iroha_torii`.
4. **Cota/agendamento.** Imponha limites de disco/paralelismo definidos pelo operador e enfileirar requisitos com contrapressão.
5. **Telemetria.** Emitir métricas/logs para sucesso de pin, latência de busca de chunks, utilização de capacidade e resultados de amostragem PoR.

## Quebra de trabalho

### A. Estrutura de caixa e módulos

| Tarefa | Dono(s) | Notas |
|------|---------|------|
| Crie `crates/sorafs_node` com módulos: `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Equipe de armazenamento | Reexporta tipos reutilizáveis ​​para integração com Torii. |
| Implementar `StorageConfig` mapeado de `SoraFsStorage` (usuário -> real -> padrões). | Equipe de armazenamento / GT de configuração | Garanta que as camadas Norito/`iroha_config` permaneçam determinísticas. |
| Fornecer uma fachada `NodeHandle` que Torii usa para pinos/fetches submetidos. | Equipe de armazenamento | Encapsula interna de armazenamento e encanamento assíncrona. |

### B. Armazenamento de pedaços persistente

| Tarefa | Dono(s) | Notas |
|------|---------|------|
| Construir um backend em disco envolvendo `sorafs_car::ChunkStore` com índice de manifesto em disco (`sled`/`sqlite`). | Equipe de armazenamento | Layout determinístico: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| Manter metadados PoR (árvores 64 KiB/4 KiB) usando `ChunkStore::sample_leaves`. | Equipe de armazenamento | Suporta repetição após reinicialização; falha rápida em corrupção. |
| Implementar replay de integridade na inicialização (rehash de manifestos, podar pins incompletos). | Equipe de armazenamento | Bloqueia o início de Torii até o final do replay. |

### C. Endpoints do gateway| Ponto final | Comportamento | Tarefas |
|----------|-------------|--------|
| `POST /sorafs/pin` | Aceita `PinProposalV1`, valida manifests, enfileira ingestão, responda com o CID do manifest. | Validar perfil de chunker, importar cotas, transmitir dados via chunk store. |
| `GET /sorafs/chunks/{cid}` + consulta de intervalo | Servir bytes de chunk com headers `Content-Chunker`; respeita a especificação de capacidade de alcance. | Usar agendador + orcamentos de stream (ligar a capacidade da faixa SF-2d). |
| `POST /sorafs/por/sample` | Rodar amostragem PoR para um manifesto e retornar pacote de prova. | Reutilizar amostras do chunk store, responder com payloads Norito JSON. |
| `GET /sorafs/telemetry` | Resumos: capacidade, sucesso PoR, contadores de erro de busca. | Fornecer dados para dashboards/operadores. |

O encanamento em tempo de execução passa as interações PoR via `sorafs_node::por`: o rastreador registra cada `PorChallengeV1`, `PorProofV1` e `AuditVerdictV1` para que as métricas `CapacityMeter` reflitam vereditos de governança sem lógica Torii sob medida. [crates/sorafs_node/src/scheduler.rs:147]

Notas de implementação:

- Use a pilha Axum de Torii com payloads `norito::json`.
- Adicione esquemas Norito para respostas (`PinResultV1`, `FetchErrorV1`, structs de telemetria).

- `/v2/sorafs/por/ingestion/{manifest_digest_hex}` agora expoe a profundidade do backlog mais a epoca/deadline mais antiga e os timestamps mais recentes de sucesso/falha por provedor, via `sorafs_node::NodeHandle::por_ingestion_status`, e Torii registra os medidores `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` para dashboards. [crates/sorafs_node/src/lib.rs:510] [crates/iroha_torii/src/sorafs/api.rs:1883] [crates/iroha_torii/src/routing.rs:7244] [crates/iroha_telemetry/src/metrics.rs:5390]

### D. Agendador e cumprimento de cotas

| Tarefa | Detalhes |
|------|---------|
| Cota de discoteca | Rastrear bytes em disco; rejeitar novos pinos ao exceder `max_capacity_bytes`. Fornecer ganchos de despejo para políticas futuras. |
| Concorrência de busca | Semaforo global (`max_parallel_fetches`) mais orcamentos por provedor oriundos de bonés da faixa SF-2d. |
| Fila de alfinetes | Limitar empregos de ingestão pendente; exportar endpoints Norito de status para profundidade da fila. |
| Cadência PoR | Trabalhador de fundo direcionado por `por_sample_interval_secs`. |

### E. Telemetria e registro

Métricas (Prometheus):

-`sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (histograma com rótulos `result`)
-`torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
-`torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
-`torii_sorafs_storage_fetch_bytes_per_sec`
-`torii_sorafs_storage_por_inflight`
-`torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Registros / eventos:

- Telemetria Norito estruturada para ingestão de governança (`StorageTelemetryV1`).
- Alertas quando utilização > 90% ou sequência de falhas PoR ultrapassar o limite.

### F. Estratégia de testes1. **Testes unitários.** Persistência do chunk store, cálculos de cota, invariantes do agendador (ver `crates/sorafs_node/src/scheduler.rs`).
2. **Testes de integração** (`crates/sorafs_node/tests`). Pin -> buscar ida e volta, recuperação após reinicialização, rejeição por cota, verificação de provas de amostragem PoR.
3. **Testes de integração Torii.** Rodar Torii com armazenamento habilitado, exercício de endpoints HTTP via `assert_cmd`.
4. **Roadmap de caos.** Drills futuros simulam exaustão de disco, IO lento, remoção de provedores.

## Dependências

- Política de admissão SF-2b - garantir que nós verifiquem os envelopes de admissão antes de anunciar.
- Marketplace de capacidade SF-2c - ligar telemetria de volta a declarações de capacidade.
- Extensões de anúncio SF-2d - consome capacidade de alcance + orcamentos de stream quando disponíveis.

## Critérios de saida do marco

- `cargo run -p sorafs_node --example pin_fetch` funciona contra luminárias locais.
- Torii compila com `--features sorafs-storage` e passa testes de integração.
- Documentação ([guia de armazenamento do nodo](node-storage.md)) atualizada com padrões de configuração + exemplos de CLI; runbook do operador disponível.
- Telemetria visível em dashboards de staging; alertas configurados para saturação de capacidade e falhas PoR.

## Entregas de documentação e operações

- Atualizar a [referência de armazenamento do nodo](node-storage.md) com padrões de configuração, uso de CLI e passos de solução de problemas.
- Manter o [runbook de operações de nodo](node-operations.md) alinhado com a implementação conforme SF-3 evolui.
- Publicar referências de API para endpoints `/sorafs/*` dentro do portal do desenvolvedor e conectar ao manifesto OpenAPI assim que os manipuladores de Torii estiverem prontos.