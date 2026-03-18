---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de nó
título: Plano de implementação do nó SoraFS
sidebar_label: Plano de implementação do nó
descrição: Transforme a folha de rota de armazenamento SF-3 em um trabalho de engenharia acionável com pneus, sacos e cobertura de testes.
---

:::nota Fonte canônica
Esta página reflete `docs/source/sorafs/sorafs_node_plan.md`. Gardez les duas cópias sincronizadas jusqu'au retrait da documentação histórica da Esfinge.
:::

SF-3 livre a primeira caixa executável `sorafs-node` que transforma um processo Iroha/Torii no fornecedor de estoque SoraFS. Utilize este plano com o [guia de estoque do novo](node-storage.md), a [política de admissão de provedores](provider-admission-policy.md) e a [folha de rota do mercado de capacidade de estoque](storage-capacity-marketplace.md) para sequenciar os disponíveis.

## Portée cible (Jalon M1)

1. **Integração do armazenamento de pedaços.** Envelopper `sorafs_car::ChunkStore` com um backend persistente que armazena os bytes de pedaços, os manifestos e as árvores PoR no repertório de dados configurado.
2. **Endpoints gateway.** Expositor de endpoints HTTP Norito para envio de pin, busca de pedaços, conversão de PoR e transferência de armazenamento no processo Torii.
3. **Plomberie de configuração.** Adiciona uma estrutura de configuração `SoraFsStorage` (sinalizador de ativação, capacidade, repertórios, limites de concorrência) baseado em `iroha_config`, `iroha_core` e `iroha_torii`.
4. **Cota/ordenação.** Aplica os limites do disco/paralelismo definidos pelo operador e envia em arquivo as solicitações com contrapressão.
5. **Télémétrie.** Emissão de métricas/logs para sucesso de pin, latência de busca de pedaços, utilização de capacidade e resultados de encantamento PoR.

## Décomposição do trabalho

### A. Estrutura da caixa e módulos

| Tache | Responsáveis(es) | Notas |
|------|----------------|------|
| Crie `crates/sorafs_node` com os módulos: `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Equipamento Armazenamento | Exporte novamente os tipos utilizáveis ​​para a integração Torii. |
| Implemente `StorageConfig` mapeado a partir de `SoraFsStorage` (usuário → real → padrões). | Equipe Storage / Config WG | Certifique-se de que os sofás Norito/`iroha_config` permaneçam determinados. |
| Fornece uma fachada `NodeHandle` que Torii utiliza para adicionar pinos/buscas. | Equipamento Armazenamento | Encapsule os internos de armazenamento e o armazenamento assíncrono. |

### B. Armazenamento de pedaços persistente| Tache | Responsáveis(es) | Notas |
|------|----------------|------|
| Crie um envelope de disco de back-end `sorafs_car::ChunkStore` com um índice de manifesto no disco (`sled`/`sqlite`). | Equipamento Armazenamento | Layout determinado: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| Mantenha os metadados PoR (arbres 64 KiB/4 KiB) via `ChunkStore::sample_leaves`. | Equipamento Armazenamento | Suporta a repetição após o novo casamento; falhar rapidamente em caso de corrupção. |
| Implemente a repetição da integridade durante o descasque (repetir os manifestos, limpar os pinos incompletos). | Equipamento Armazenamento | Bloqueie a inicialização de Torii até o final da repetição. |

### C. Gateway de terminais

| Ponto final | Comportamento | Taches |
|----------|-------------|-------|
| `POST /sorafs/pin` | Aceite `PinProposalV1`, valide os manifestos, com a ingestão no arquivo, responda com o CID do manifesto. | Valide o perfil do chunker, aplique cotas e transmita dados por meio do chunk store. |
| `GET /sorafs/chunks/{cid}` + pedido de alcance | Insira os bytes do bloco com os cabeçalhos `Content-Chunker` ; respeite a especificação de capacidade da faixa. | Use o agendador + orçamentos de stream (sem contar com a capacidade da faixa SF-2d). |
| `POST /sorafs/por/sample` | Lance uma échantillonnage PoR para um manifesto e reenvie um pacote de pré-venda. | Reutilize a manipulação do armazenamento de pedaços, responda com payloads Norito JSON. |
| `GET /sorafs/telemetry` | Currículos: capacidade, sucesso PoR, compteurs d'erreurs de fetch. | Forneça dados para painéis/operadores. |

O tempo de execução do planejamento depende das interações PoR via `sorafs_node::por`: o rastreador registra cada `PorChallengeV1`, `PorProofV1` e `AuditVerdictV1` para que as métricas `CapacityMeter` reflitam os veredictos de governança sem lógica Torii específica.【crates/sorafs_node/src/scheduler.rs#L147】

Notas de implementação:

- Utilize a pilha Axum de Torii com cargas úteis `norito::json`.
- Adicionar esquemas Norito para respostas (`PinResultV1`, `FetchErrorV1`, estruturas de telefonia).

- ✅ `/v1/sorafs/por/ingestion/{manifest_digest_hex}` expõe a manutenção da profundidade do backlog até a época/chegada ao passado e os carimbos de data e hora de sucesso/cheque dos mais recentes pelo provedor, via `sorafs_node::NodeHandle::por_ingestion_status`, e Torii registra os medidores `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` para os painéis.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. Agendador e aplicativo de cotas| Tache | Detalhes |
|------|---------|
| Disco de cota | Mantenha os bytes no disco; rejeite os novos pinos a partir de `max_capacity_bytes`. Fornecer ganchos de despejo para políticas futuras. |
| Concorrência de busca | Sémaphore global (`max_parallel_fetches`) mais orçamentos por fornecedor emitem limites da faixa SF-2d. |
| Arquivo de pinos | Limitar os trabalhos de ingestão com atenção; expõe os endpoints do status Norito para a profundidade do arquivo. |
| Cadência PoR | Trabalhador de fundo pilotado par `por_sample_interval_secs`. |

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

- Telemetria Norito estruturada para controle de ingestão (`StorageTelemetryV1`).
- Alertas quando a utilização > 90% ou que a série de verificações PoR dépasse le seuil.

### F. Estratégia de testes

1. **Testes unitários.** Persistência do armazenamento de blocos, cálculos de cota, invariantes do agendador (veja `crates/sorafs_node/src/scheduler.rs`).
2. **Testes de integração** (`crates/sorafs_node/tests`). Pin → buscar aller-retour, reprise après redemarrage, rejet de quota, verificação de preuve d'échantillonnage PoR.
3. **Testes de integração Torii.** Execute Torii com armazenamento ativo, aplicando endpoints HTTP via `assert_cmd`.
4. **Roadmap caos.** Os exercícios futuros simulam o consumo do disco, a lente IO e a supressão dos provedores.

##Dependências

- Política de admissão SF-2b — garantir que os noeuds verifiquem os envelopes de admissão antes da publicação de anúncios.
- Mercado de capacidade SF-2c — conecte a telemetria às declarações de capacidade.
- Extensões de anúncio SF-2d — consome a capacidade de alcance + orçamentos de stream disponíveis.

## Critérios de surtida do jalon

- `cargo run -p sorafs_node --example pin_fetch` funciona nos locais dos equipamentos.
- Torii compilado com `--features sorafs-storage` e aprovado nos testes de integração.
- Documentação ([guide de stockage du nœud](node-storage.md)) atualizada com padrões de configuração + exemplos CLI ; operador de runbook disponível.
- Telemetria visível nos painéis de teste; alertas configurados para saturação de capacidade e níveis de PoR.

## Documentação e operações disponíveis

- Leia hoje a [referência de armazenamento do nó](node-storage.md) com os padrões de configuração, o uso da CLI e as etapas de solução de problemas.
- Garder le [runbook d'opérations du noeud](node-operations.md) alinhado com a implementação na pele e com a medida da evolução SF-3.
- Publique as referências API para os endpoints `/sorafs/*` no portal do desenvolvedor e confie no manifesto OpenAPI e depois os manipuladores Torii no local.