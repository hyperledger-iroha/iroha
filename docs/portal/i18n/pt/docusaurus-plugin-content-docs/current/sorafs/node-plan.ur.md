---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de nó
título: Plano de implementação do nó SoraFS
sidebar_label: plano de implementação do nó
descrição: roteiro de armazenamento SF-3 کو marcos, tarefas اور cobertura de teste کے ساتھ trabalho de engenharia acionável میں تبدیل کرتا ہے۔
---

:::nota مستند ماخذ
:::

SF-3 é uma caixa `sorafs-node` executável. ہے۔ Aqui está [guia de armazenamento de nó](node-storage.md), [política de admissão do provedor](provider-admission-policy.md) e [roteiro do mercado de capacidade de armazenamento](storage-capacity-marketplace.md) کے ساتھ استعمال کریں جب sequência de entregas کریں۔

## Escopo alvo (Milestone M1)

1. **Integração de armazenamento de pedaços.** `sorafs_car::ChunkStore` کو ایسے backend persistente سے wrap کریں جو diretório de dados configurado میں chunk bytes، manifestos اور árvores PoR محفوظ کرے۔
2. **Endpoints de gateway.** Processo Torii کے اندر envio de pin, busca de pedaços, amostragem PoR e telemetria de armazenamento کے لیے Norito Os endpoints HTTP expõem کریں۔
3. ** Encanamento de configuração. ** `SoraFsStorage` struct de configuração (sinalizador habilitado, capacidade, diretórios, limites de simultaneidade) شامل کریں اور `iroha_config`, `iroha_core`, `iroha_torii` کے Fio ذریعے کریں۔
4. **Cota/agendamento.** Limites de disco/paralelismo definidos pelo operador impõem solicitações e contrapressão کے ساتھ fila کریں۔
5. **Telemetria.** sucesso do pino, latência de busca de pedaços, utilização da capacidade e resultados de amostragem PoR کے لیے métricas/logs emitem کریں۔

## Análise do trabalho

### A. Caixa e estrutura do módulo

| Tarefa | Proprietário(s) | Notas |
|------|----------|-------|
| `crates/sorafs_node` بنائیں جس میں `config`, `store`, `gateway`, `scheduler`, `telemetry` módulos ہوں۔ | Equipe de armazenamento | Integração Torii کے لیے reexportação de tipos reutilizáveis ​​کریں۔ |
| `StorageConfig` implementar کریں جو `SoraFsStorage` سے mapeado ہو (usuário → real → padrões)۔ | Equipe de armazenamento / GT de configuração | Camadas Norito/`iroha_config` e camadas determinísticas |
| Torii کے pinos/buscas کے لیے `NodeHandle` fachada فراہم کریں۔ | Equipe de armazenamento | armazenamento interno e encanamento assíncrono encapsulado |

### B. Armazenamento de pedaços persistentes

| Tarefa | Proprietário(s) | Notas |
|------|----------|-------|
| `sorafs_car::ChunkStore` کو índice de manifesto em disco (`sled`/`sqlite`) کے ساتھ backend de disco میں wrap کریں۔ | Equipe de armazenamento | Layout determinístico: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| `ChunkStore::sample_leaves` کے ذریعے Metadados PoR (árvores de 64 KiB/4 KiB) mantêm کریں۔ | Equipe de armazenamento | Reinicie para obter suporte de repetição; corrupção پر falhar rápido۔ |
| Inicialização پر implementação de repetição de integridade کریں (repetição de manifestos, remoção incompleta de pinos) ۔ | Equipe de armazenamento | replay مکمل ہونے تک Torii bloco inicial کریں۔ |

### C. Terminais de gateway| Ponto final | Comportamento | Tarefas |
|----------|-----------|-------|
| `POST /sorafs/pin` | `PinProposalV1` قبول کریں, manifestos validam کریں, fila de ingestão کریں, manifesto CID واپس دیں۔ | perfil de pedaço validar کریں، cotas impor کریں، armazenamento de pedaços کے ذریعے fluxo de dados کریں۔ |
| `GET /sorafs/chunks/{cid}` + consulta de intervalo | Cabeçalhos `Content-Chunker` کے ساتھ chunk bytes servem کریں؛ respeito das especificações de capacidade de alcance کریں۔ | agendador + orçamentos de fluxo استعمال کریں (capacidade de intervalo SF-2d کے ساتھ tie کریں)۔ |
| `POST /sorafs/por/sample` | manifesto کے لیے amostragem PoR چلائیں اور pacote de prova واپس کریں۔ | reutilização de amostragem de armazenamento de blocos کریں, cargas úteis JSON Norito کے ساتھ responder کریں۔ |
| `GET /sorafs/telemetry` | Resumos: capacidade, sucesso de PoR, contagem de erros de busca۔ | painéis/operadores کے لیے dados فراہم کریں۔ |

Encanamento de tempo de execução `sorafs_node::por` کے ذریعے Interações PoR کو thread کرتی ہے: rastreador ہر `PorChallengeV1`, `PorProofV1`, `AuditVerdictV1` کو registro کرتا ہے تاکہ Veredictos de governança de métricas `CapacityMeter` کو Lógica específica de Torii کے بغیر refletir کریں۔【crates/sorafs_node/src/scheduler.rs#L147】

Notas de implementação:

- Torii کے Pilha Axum کو Cargas úteis `norito::json` کے ساتھ استعمال کریں۔
- respostas کے لیے Esquemas Norito شامل کریں (`PinResultV1`, `FetchErrorV1`, estruturas de telemetria)۔

- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}` اب profundidade do backlog کے ساتھ época/prazo mais antigo اور ہر provedor کے carimbos de data/hora recentes de sucesso/falha دکھاتا ہے, جو `sorafs_node::NodeHandle::por_ingestion_status` سے alimentado ہے، Os painéis Torii possuem registro de medidores `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` ہے۔【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. Agendador e aplicação de cotas

| Tarefa | Detalhes |
|------|---------|
| Cota de disco | Faixa de bytes de disco کریں؛ `max_capacity_bytes` پر نئے pinos rejeitados کریں۔ مستقبل کی políticas کے لیے ganchos de despejo فراہم کریں۔ |
| Buscar simultaneidade | Semáforo global (`max_parallel_fetches`) کے ساتھ orçamentos por provedor e limites de intervalo SF-2d سے آتے ہیں۔ |
| Fila de fixação | Trabalhos de ingestão excelentes محدود کریں؛ profundidade da fila کے لیے Norito status endpoints expõem کریں۔ |
| Cadência PoR | `por_sample_interval_secs` é um trabalhador em segundo plano |

### E. Telemetria e registro

Métricas (Prometheus):

-`sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (histograma com rótulos `result`)
-`torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
-`torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
-`torii_sorafs_storage_fetch_bytes_per_sec`
-`torii_sorafs_storage_por_inflight`
-`torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Registros/eventos:

- ingestão de governança کے لیے telemetria Norito estruturada (`StorageTelemetryV1`).
- utilização > 90% یا Limite de sequência de falhas PoR سے اوپر ہو تو alertas۔

### F. Estratégia de teste1. **Testes de unidade.** persistência de armazenamento de blocos, cálculos de cota, invariantes do agendador (consulte `crates/sorafs_node/src/scheduler.rs`).
2. **Testes de integração** (`crates/sorafs_node/tests`). Pin → buscar ida e volta, reiniciar recuperação, rejeição de cota, verificação de prova de amostragem PoR۔
3. **Testes de integração Torii.** Torii کو armazenamento habilitado کے ساتھ چلائیں, endpoints HTTP کو `assert_cmd` کے ذریعے exercício کریں۔
4. **Roteiro do caos.** مستقبل کے perfura a exaustão do disco, IO lento ou simulação de remoção do provedor کریں گے۔

## Dependências

- Política de admissão SF-2b - nós کو publicação de anúncios کرنے سے پہلے verificação de envelopes de admissão کرنے ہوں گے۔
- Mercado de capacidade SF-2c – telemetria کو declarações de capacidade کے ساتھ tie کریں۔
- Extensões de anúncio SF-2d - capacidade de alcance + orçamentos de fluxo دستیاب ہونے پر consumir کریں۔

## Critérios de saída do marco

- `cargo run -p sorafs_node --example pin_fetch` luminárias locais کے خلاف کام کرے۔
- Torii `--features sorafs-storage` کے ساتھ build ہو اور testes de integração پاس کرے۔
- Documentação ([guia de armazenamento de nó] (node-storage.md)) padrões de configuração + exemplos CLI کے ساتھ atualizado ہو؛ runbook do operador
- Painéis de teste de telemetria میں نظر آئے؛ saturação de capacidade اور falhas PoR کے لیے configuração de alertas ہوں۔

## Documentação e resultados operacionais

- [referência de armazenamento de nó](node-storage.md) کو padrões de configuração, uso de CLI e etapas de solução de problemas کے ساتھ atualização کریں۔
- [runbook de operações de nó](node-operations.md) کو implementação کے ساتھ alinhar رکھیں جیسے SF-3 evoluir ہو۔
- Endpoints `/sorafs/*` کی Referências de API portal do desenvolvedor میں publicar کریں اور Manipuladores Torii آنے کے بعد Manifesto OpenAPI سے wire کریں۔