---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de nó
título: Plano de implementação do nó SoraFS
sidebar_label: Plano de implementação do nó
descrição: Converta a estrada de almacenamiento SF-3 em trabalho de engenharia acionável com hitos, tareas e cobertura de pruebas.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/sorafs_node_plan.md`. Mantenha ambas as cópias sincronizadas até que a documentação herdada do Sphinx seja retirada.
:::

SF-3 entrega o primer crate ejecutable `sorafs-node` que converte um processo Iroha/Torii em um provedor de armazenamento SoraFS. Usamos este plano junto com o [guia de armazenamento do nó](node-storage.md), a [política de admissão de provedores](provider-admission-policy.md) e a [hoja de rota do mercado de capacidade de armazenamento](storage-capacity-marketplace.md) para garantir entregas.

## Alcance objetivo (Hito M1)

1. **Integração do armazenamento de chunks.** Envuelve `sorafs_car::ChunkStore` com um backend persistente que guarda bytes de chunk, manifestos e árvores PoR no diretório de dados configurado.
2. **Endpoints de gateway.** Expor endpoints HTTP Norito para envio de pinos, busca de pedaços, exibição de PoR e telemetria de armazenamento dentro do processo Torii.
3. **Plomeria de configuração.** Agrega uma estrutura de configuração `SoraFsStorage` (sinalizador habilitado, capacidade, diretórios, limites de concorrência) cabeada através de `iroha_config`, `iroha_core` e `iroha_torii`.
4. **Cotas/planejamento.** Imponha limites de disco/paralelismo definidos pelo operador e inclua solicitações com contrapressão.
5. **Telemetria.** Emite métricas/logs para sucesso de pinos, latência de busca de pedaços, utilização de capacidade e resultados de exibição PoR.

## Desglose do trabalho

### A. Estrutura de caixa e módulos

| Tara | Responsáveis(es) | Notas |
|------|---------------|------|
| Crie `crates/sorafs_node` com módulos: `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Equipamento de Armazenamento | Reexporte tipos reutilizáveis ​​para integração com Torii. |
| Implementar `StorageConfig` mapeado a partir de `SoraFsStorage` (usuário → real → padrões). | Equipamento de armazenamento / configuração WG | Certifique-se de que as capas Norito/`iroha_config` permaneçam deterministas. |
| Prove uma fachada `NodeHandle` que Torii usa para enviar pinos/buscas. | Equipamento de Armazenamento | Encapsula interna de armazenamento e plomeria assíncrona. |

### B. Almacén de pedaços persistentes| Tara | Responsáveis(es) | Notas |
|------|---------------|------|
| Construa um backend em disco que envolva `sorafs_car::ChunkStore` com um índice de manifestos em disco (`sled`/`sqlite`). | Equipamento de Armazenamento | Determinista de layout: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| Mantenha metadados PoR (árboles de 64 KiB/4 KiB) usando `ChunkStore::sample_leaves`. | Equipamento de Armazenamento | Soporta replay tras reinícios; falha rápida antes da corrupção. |
| Implementar replay de integridade no início (rehash de manifestos, podar pinos incompletos). | Equipamento de Armazenamento | Bloqueie a inicialização de Torii até completar o replay. |

### C. Endpoints do gateway

| Ponto final | Comportamento | Taras |
|----------|----------------|-------|
| `POST /sorafs/pin` | Aceite `PinProposalV1`, valide manifestos, encola a ingestão, responda com o CID do manifesto. | Valide o perfil do chunker, impone cotas, transmita dados através da chunk store. |
| `GET /sorafs/chunks/{cid}` + consulta por rango | Sirva bytes de pedaços com cabeçalhos `Content-Chunker`; respeite a especificação de capacidade de rango. | Agendador dos EUA + requisitos de stream (vinculados à capacidade de rango SF-2d). |
| `POST /sorafs/por/sample` | Execute o museu PoR para um manifesto e devolva um pacote de testes. | Reutilize o mapa do chunk store, responda com payloads Norito JSON. |
| `GET /sorafs/telemetry` | Currículos: capacidade, sucesso de PoR, conteúdo de erros de busca. | Proporciona dados para dashboards/operadores. |

A plomeria em tempo de execução enlaça as interações PoR através de `sorafs_node::por`: o rastreador registra cada `PorChallengeV1`, `PorProofV1` e `AuditVerdictV1` para que as estatísticas de `CapacityMeter` reflitam os veredictos de governo sem lógica Torii personalizada.【crates/sorafs_node/src/scheduler.rs#L147】

Notas de implementação:

- Usa a pilha Axum de Torii com cargas úteis `norito::json`.
- Agregar esquemas Norito para respostas (`PinResultV1`, `FetchErrorV1`, estruturas de telemetria).

- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}` agora expõe a profundidade do backlog mais da época/limite mais antigo e os timestamps de sucesso/fallo mais recentes por fornecedor, impulsionado por `sorafs_node::NodeHandle::por_ingestion_status`, e Torii registre os medidores `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` para painéis.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. Agendador e cumprimento de contas

| Tara | Detalhes |
|------|----------|
| Cuota de discoteca | Seguimento de bytes em disco; rechaza novos pinos para superar `max_capacity_bytes`. Prove ganchos de expulsão para políticas futuras. |
| Concorrência de busca | Semáforo global (`max_parallel_fetches`) mais pressupostos pelo provedor obtido dos limites do intervalo SF-2d. |
| Cola de alfinetes | Limite os trabalhos de ingestão pendentes; expor endpoints Norito de estado para profundidade de cola. |
| Cadência PoR | Trabalhador em segundo plano, impulsionado por `por_sample_interval_secs`. |

### E. Telemetria e registro

Métricas (Prometheus):-`sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (histograma com etiquetas `result`)
-`torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
-`torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
-`torii_sorafs_storage_fetch_bytes_per_sec`
-`torii_sorafs_storage_por_inflight`
-`torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Registros / eventos:

- Telemetria Norito estruturada para ingestão de governo (`StorageTelemetryV1`).
- Alertas quando a utilização > 90% ou a racha de falhas PoR supera o umbral.

### F. Estratégia de teste

1. **Testes unitários.** Persistência do armazenamento de pedaços, cálculos de cota, invariantes do agendador (ver `crates/sorafs_node/src/scheduler.rs`).
2. **Testes de integração** (`crates/sorafs_node/tests`). Pin → buscar ida e volta, recuperação de retorno, rechazo por cuota, verificação de testes de museu PoR.
3. **Testes de integração de Torii.** Execute Torii com armazenamento habilitado, execute endpoints HTTP via `assert_cmd`.
4. **Hoja de rota de caos.** Futuros treinos simulando agotamiento de disco, IO lento, retiro de fornecedores.

## Dependências

- Política de admissão SF-2b — certifique-se de que os nós verifiquem os envelopes de admissão antes de anunciarem.
- Marketplace de capacidade SF-2c — vincula a telemetria de volta às declarações de capacidade.
- Extensões de anúncio SF-2d — consomem capacidade de alcance + pressupostos de stream quando estão disponíveis.

## Critérios de saída do hit

- `cargo run -p sorafs_node --example pin_fetch` funciona contra localidades de luminárias.
- Torii compilado com `--features sorafs-storage` e alguns testes de integração.
- Documentação ([guia de armazenamento do nó](node-storage.md)) atualizada com padrões de configuração + exemplos de CLI; runbook do operador disponível.
- Telemetria visível nos painéis de teste; alertas ajustados para saturação de capacidade e falhas de PoR.

## Entregas de documentação e operações

- Atualizar a [referência de armazenamento do nó](node-storage.md) com padrões de configuração, uso de CLI e etapas de solução de problemas.
- Mantenha o [runbook de operações de nó](node-operations.md) alinhado com a implementação em conformidade com a evolução SF-3.
- Publicar referências de API para endpoints `/sorafs/*` dentro do portal de desenvolvimento e conectá-las ao manifesto OpenAPI uma vez que os manipuladores de Torii estiverem listados.