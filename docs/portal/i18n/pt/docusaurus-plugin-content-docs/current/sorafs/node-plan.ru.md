---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/node-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: plano de nó
título: Plano de realização do SoraFS
sidebar_label: Plano de execução usado
descrição: Instale o cartão de segurança SF-3 em um trabalho de engenharia com testes, testes e testes.
---

:::nota História Canônica
:::

SF-3 é fornecido pela caixa de armazenamento `sorafs-node`, que fornece o processo Iroha/Torii no teste Versão SoraFS. Utilize este plano em [гайдом по хранилищу узла](node-storage.md), [политикой допуска prova](provider-admission-policy.md) e [дорожной картой маркетплейса емкости хранения](storage-capacity-marketplace.md) при выстраивании последовательности работ.

## Целевой объем (веха M1)

1. **Integração da loja de pedaços.** Use `sorafs_car::ChunkStore` no backend de segurança, который хранит байты чанков, манифесты и PoR-деревья в заданной директории данных.
2. **Gateway Gateway.** Abra Norito HTTP эндпоинты para conseguir pin, fetch чанков, amostragem PoR e telêmetro хранилища внутри processo Torii.
3. **Configurações de configuração.** Baixe a configuração da estrutura `SoraFsStorage` (configure a configuração, emoticon, diretórios, limites de concorrência), fornecendo o código `iroha_config`, `iroha_core` e `iroha_torii`.
4. **Avalie/instale.** Verifique os limites de operação/pressão do disco e ajuste a pressão de retorno.
5. **Телеметрия.** Эмитить метрики/логи по успехам pin, задержке fetch чанков, загрузке емкости и результатам PoR amostragem.

## Разбиение работ

### A. Estrutura de criação e módulo

| Bem | Ответственный(е) | Nomeação |
|------|------------------|-----------|
| Compre `crates/sorafs_node` com módulos: `config`, `store`, `gateway`, `scheduler`, `telemetry`. | Equipe de armazenamento | Verifique os tipos de relatórios para integração com Torii. |
| Selecione `StorageConfig`, compatível com `SoraFsStorage` (usuário → real → padrões). | Equipe de armazenamento / GT de configuração | Garantir a determinação de Norito/`iroha_config`. |
| Verifique o painel `NodeHandle`, enquanto o Torii é usado para pinos/buscas. | Equipe de armazenamento | Инкапсулировать внутренности хранения e async-provoдку. |

### B. Loja de pedaços seguros

| Bem | Ответственный(е) | Nomeação |
|------|------------------|-----------|
| Use o back-end do disco `sorafs_car::ChunkStore` com manipuladores de índice no disco (`sled`/`sqlite`). | Equipe de armazenamento | Layout determinado: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| A meta PoR de capacidade (de 64 KiB/4 KiB) é `ChunkStore::sample_leaves`. | Equipe de armazenamento | Поддерживает replay após reinicializar; falhar rápido por corporação. |
| Realize a repetição de integridade no início (rehash манифестов, чистка незавершенных pinos). | Equipe de armazenamento | Bloqueie a inicialização Torii para reproduzir novamente. |

### C. Gateway de entrada| Endpoint | Sugestão | Baixar |
|--------|-----------|--------|
| `POST /sorafs/pin` | Verifique `PinProposalV1`, valide os manifestos, configure a ingestão no local e abra o manifesto CID. | Валидировать профиль chunker, применять квоты, стримить данные через chunk store. |
| `GET /sorafs/chunks/{cid}` + consulta de intervalo | Отдает байты чанка с заголовками `Content-Chunker`; capacidade de alcance específico. | Использовать agendador + бюджеты стрима (com capacidade de intervalo SF-2d). |
| `POST /sorafs/por/sample` | Faça a amostragem PoR para a manifestação e forneça o pacote de provas. | Selecione o armazenamento de blocos de amostragem, exibindo cargas úteis JSON Norito. |
| `GET /sorafs/telemetry` | Сводки: емкость, успех PoR, счетчики ошибок buscar. | Предоставлять данные для дашбордов/операторов. |

Рантайм-проводка направляет PoR взаимодействия через `sorafs_node::por`: трекер фиксирует каждый `PorChallengeV1`, `PorProofV1` e `AuditVerdictV1`, essas métricas `CapacityMeter` fornecem governança sem lógica externa Torii.【crates/sorafs_node/src/scheduler.rs#L147】

Recomendações para a realização:

- Use cargas úteis Axum Torii e `norito::json`.
- Добавить Norito схемы ответов (`PinResultV1`, `FetchErrorV1`, estruturas de telemetria).

- ✅ `/v2/sorafs/por/ingestion/{manifest_digest_hex}` теперь показывает глубину backlog, самый старый época/prazo e последние carimbos de data/hora de sucesso/falha по каждому провайдеру, за selecione `sorafs_node::NodeHandle::por_ingestion_status`, e Torii, métrica física `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` para дашбордов.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. Agendador e chave preliminar

| Bem | Detalhes |
|------|--------|
| Discagem de moeda | Отслеживать байты no disco; отклонять novos pinos при превышении `max_capacity_bytes`. Você pode usar suas roupas para a política do budismo. |
| Buscar por Concursado | O semáforo global (`max_parallel_fetches`) é mais barato por provedor na faixa SF-2d limitada. |
| Alfinetes de segurança | Лимитировать незавершенные trabalhos de ingestão; Verifique o status do Norito para as lâmpadas instaladas. |
| Cadenia PoR | Funcionalidade adequada, управляемый `por_sample_interval_secs`. |

### E. Telemetria e Logística

Métricas (Prometheus):

-`sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (гистограмма с метками `result`)
-`torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
-`torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
-`torii_sorafs_storage_fetch_bytes_per_sec`
-`torii_sorafs_storage_por_inflight`
-`torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

Logística/Social:

- Estrutura de telemetria Norito para ingestão de governança (`StorageTelemetryV1`).
- Алерты при использовании > 90% или превышении порога серии PoR-ошибок.

### F. Teste de estratégia1. **Юнит-тесты.** Персистентность chunk store, вычисления квот, инварианты agendador (см. `crates/sorafs_node/src/scheduler.rs`).
2. **Testes de integração** (`crates/sorafs_node/tests`). Pin → buscar ida e volta, восстановление после рестарта, отказ по квоте, проверка PoR prova de amostragem.
3. **Torii testes de integração.** Abra Torii com a interface de usuário, progнать HTTP энддпоинты через `assert_cmd`.
4. **Roadmap caos.** Будущие drills моделируют исчерпание диска, медленный IO, удаление провайдеров.

## Зависимости

- Политика допуска SF-2b — убедиться, что узлы проверяют envelopes de admissão para anúncio publicitário.
- Маркетплейс емкости SF-2c — conecta o telefone com a declaração de energia.
- Расширения anúncio SF-2d — capacidade de alcance de uso + бюджеты стримов по мере появления.

## Критерии завершения вехи

- `cargo run -p sorafs_node --example pin_fetch` funciona em luminárias locais.
- Torii é compatível com `--features sorafs-storage` e realiza testes de integração.
- Документация ([гайд по хранилищу узла](node-storage.md)) é configurado com configuração padrão + CLI; runbook para o pacote operacional.
- Visualização da tela no painel de teste; алерты настроены на насыщение емкости и PoR ошибки.

## Documentação e entregas operacionais

- Обновить [справочник по хранилищу узла](node-storage.md) com configuração padrão, configuração CLI e solução de problemas.
- Execute [runbook операций узла](node-operations.md) em uma solução realizável com mais atualizações do SF-3.
- Abra a API `/sorafs/*` no portal de distribuição e use o manifesto OpenAPI, juntamente com os manipuladores Torii появятся.