---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/transition-notes.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: notas de transição do nexo
título: Заметки о переходе Nexus
description: Зеркало `docs/source/nexus_transition_notes.md`, охватывающее доказательства перехода Fase B, график аудита и митигации.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Faça o download do Nexus

Este registro foi definido como trabalho **Fase B - Nexus Fundações de Transição** para fazer a lista de verificação da minha lista de verificação. Ao completar o marco em `roadmap.md` e obter o documento, na classe B1-B4, no último mês, você terá governança, SRE e SDK do SDK podem ser criados com um histórico.

## Область e cadência

- Implementar auditorias de rastreamento roteado e guardrails de telemetria (B1/B2), definir delta de configuração, governança de implementação (B3) e acompanhamentos no ensaio de lançamento em várias pistas (B4).
- Заменяет временную nota de cadência, которая была здесь раньше; na auditoria do primeiro trimestre de 2026, você pode obter a solução em `docs/source/nexus_routed_trace_audit_report_2026q1.md`, e esta página contém gráficos e recuperação митигаций.
- Обновляйте таблицы после каждого routed-trace окна, голосования governança ou ensaio de lançamento. Когда артефакты перемещаются, отражайте новый путь здесь, documentos downstream (status, dashboards, portais SDK) podem ser acessados стабильный якорь.

## Снимок доказательств (2026 Q1-Q2)

| Ponto | Documentar | Proprietário(s) | Status | Nomeação |
|------------|----------|----------|--------|-------|
| **B1 – Auditorias de rastreamento roteado** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetria-ops, @governança | Tempo (primeiro trimestre de 2026) | Зафиксированы три окна аудита; TLS lag `TRACE-CONFIG-DELTA` será executado na nova execução do segundo trimestre. |
| **B2 - Remediação de telemetria e guarda-corpos** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetria-ops | Sobreviver | Pacote de alerta, bot de comparação de política e divisão de lote OTLP (log `nexus.scheduler.headroom` + headroom do painel Grafana) postados; открытых renúncia нет. |
| **B3 - Aprovações delta de configuração** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | Sobreviver | Голосование GOV-2026-03-19 зафиксировано; подписанный pacote питает pacote de telemetria ниже. |
| **B4 - Ensaio de lançamento em várias pistas** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Fevereiro (2º trimestre de 2026) | Q2 canary rerun закрыл митигацию TLS лага; manifesto do validador + `.sha256` фиксирует слот-диапазон 912-936, semente de carga de trabalho `NEXUS-REH-2026Q2` e perfil de hash TLS ou reexecutado. |

## Gráfico de auditoria de rastreamento roteado

| ID de rastreamento | Hoje (UTC) | Resultado | Nomeação |
|----------|-------------|---------|-------|
| `TRACE-LANE-ROUTING` | 17/02/2026 09:00-09:45 | Produtivo | Admissão de fila P95 оставался значительно ниже цели <=750 ms. Действий не требуется. |
| `TRACE-TELEMETRY-BRIDGE` | 24/02/2026 10h00-10h45 | Produtivo | Hashes de repetição OTLP usados ​​em `status.md`; parity SDK diff bot pode não ter desvio. |
| `TRACE-CONFIG-DELTA` | 01/03/2026 12h00-12h30 | Redefinição | TLS foi encerrado na reexecução do segundo trimestre; pacote de telemetria para `NEXUS-REH-2026Q2` фиксирует hash perfil профиля TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (см. `artifacts/nexus/tls_profile_rollout_2026q2/`) e não disponível. |
| `TRACE-MULTILANE-CANARY` | 05/05/2026 09:12-10:14 | Produtivo | Semente de carga de trabalho `NEXUS-REH-2026Q2`; pacote de telemetria + manifesto/resumo em `artifacts/nexus/rehearsals/2026q1/` (intervalo de slots 912-936) com agenda em `artifacts/nexus/rehearsals/2026q2/`. |

Будущие кварталы должны добавлять новые строки и переносить завершенные записи в приложение, когда таблица перерастет текущий квартал. Ссылайтесь на этот раздел отчетов отчетов отчетов или governança minutos через якорь roteado-trace через якорь `#quarterly-routed-trace-audit-schedule`.

## Митигации e backlog| Artigo | Descrição | Proprietário | Cel | Status / Nomeação |
|------|------------|-------|--------|----------------|
| `NEXUS-421` | Завершить распространение perfil TLS, который отставал em `TRACE-CONFIG-DELTA`, захватить evidência reexecutada e закрыть журнал митигации. | @release-eng, @sre-core | Rastreamento roteado do segundo trimestre de 2026 ok | Закрыто - hash TLS perfil `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` зафиксирован em `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; execute novamente подтвердил отсутствие отстающих. |
| Preparação `TRACE-MULTILANE-CANARY` | Запланировать ensaio Q2, приложить fixtures к telemetry pack e убедиться, что SDK aproveita переиспользуют валидированный helper. | @telemetry-ops, Programa SDK | Plano 2026-04-30 | Завершено - agenda хранится в `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` com slot/carga de trabalho de metadados; reutilizar o chicote de fios do rastreador. |
| Rotação de resumo do pacote de telemetria | Запускать `scripts/telemetry/validate_nexus_telemetry_pack.py` перед каждым ensaio/lançamento e resumos фиксировать рядом с tracker config delta. | @telemetria-ops | На каждый candidato a lançamento | Завершено - `telemetry_manifest.json` + `.sha256` выпущены в `artifacts/nexus/rehearsals/2026q1/` (intervalo de slots `912-936`, semente `NEXUS-REH-2026Q2`); resume os resultados do rastreador e do índice. |

## Integração do pacote delta de configuração

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` остается каноническим resumo diff. Você está comprando o novo `defaults/nexus/*.toml` ou o genesis, conectando o rastreador, abrindo os pontos de conexão sim.
- Подписанные config bundles питают pacote de telemetria de ensaio. Pacote, válido `scripts/telemetry/validate_nexus_telemetry_pack.py`, должен публиковаться вместе с доказательствами config delta, чтобы операторы могли воспроизвести точные артефакты, использованные в B4.
- Bundles Iroha 2 остаются без lanes: configs с `nexus.enabled = false` теперь отклоняют overrides lane/dataspace/routing, если не включен Nexus perfil (`--sora`), você pode usar a seção `nexus.*` de portas de pista única.
- Держите лог голосования governança (GOV-2026-03-19) связанным и с tracker, и с этой заметкой, чтобы будущие голосования Você pode copiar o formato sem uma configuração padrão.

## Acompanhamentos no ensaio de lançamento

- `docs/source/runbooks/nexus_multilane_rehearsal.md` фиксирует plano canário, lista участников e шаги rollback; обновляйте runbook при изменениях топологии pistas ou exportador телеметрии.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` перечисляет все артефакты, проверенные на репетиции 9 de abril, e теперь включает Q2 notas de preparação/agenda. Faça uma repetição de botão no seu rastreador quando você estiver usando o rastreador, e este será o seu monofone.
- Publicar trechos de coletor OTLP e exportações Grafana (como `docs/source/telemetry.md`) por exportador de orientação em lote; O Q1 aumentou o tamanho do lote para 256 amostras, gerando alertas de headroom.
- Доказательства CI/testes multi-lane теперь живут в `integration_tests/tests/nexus/multilane_pipeline.rs` e запускаются через fluxo de trabalho `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), nome устаревшую ссылку `pytests/nexus/test_multilane_pipeline.py`; держите hash `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) sincronizado com rastreador por pacotes de ensaio обновлении.

## Ciclo de vida da pista de tempo de execução

- Planeje o ciclo de vida da pista de tempo de execução para validar as ligações do espaço de dados e прерываются при сбое reconciliação Kura/armazenamento em camadas, оставляя каталог без изменений. Ajudantes очищают кэшированные relés de pista para pistas aposentadas, чтобы síntese de livro-razão de mesclagem не переиспользовал устаревшие provas.
- Применяйте планы через Nexus config/lifecycle helpers (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) para добавления/вывода pistas não restauração; roteamento, instantâneos TEU e registros de manifesto são executados automaticamente no plano de uso.
- Руководство оператора: при сбое плана проверьте отсутствие или raízes de armazenamento, которые не удается создать (diretórios hierárquicos de raiz fria/Kura lane). Исправьте базовые пути и повторите; успешные планы повторно эмитят telemetry diff lane/dataspace, чтобы dashboards отражали новую топологию.

## NPoS телеметрия и доказательства contrapressão

O ensaio de lançamento da Fase B do запросило детерминированные телеметрийные captura, доказывающие, что marcapasso NPoS e fofocas слои остаются в contrapressão excessiva. A integração do chicote em `integration_tests/tests/sumeragi_npos_performance.rs` reproduz este cenário e exibe resumos JSON (`sumeragi_baseline_summary::<scenario>::...`) por meio de uma nova métrica. Localização local:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Definindo `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` ou `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R`, чтобы исследовать более стрессовые топологии; значения по умолчанию отражают perfil 1 s/`k=3`, использованный в B4.| Cenário / teste | Pesquisa | Telemetria |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Bloqueie 12 rodadas com tempo de bloco de ensaio, чтобы записать envelopes de latência EMA, глубины очередей e medidores de envio redundante para сериализацией pacote de evidências. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Переполняет очередь транзакций, чтобы гарантировать детерминированный запуск adiamentos de admissão e эксport счетчиков capacidade/saturação. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Семплирует jitter do marcapasso e tempo limite de exibição, mas não há limites máximos de +/-125 permille. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | Проталкивает крупные RBC payloads para soft/hard лимитов store, чтобы показать рост, откат и стабилизацию session/byte счетчиков без loja especializada. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Форсирует ретрансмиты, чтобы mede a taxa de envio redundante e conta a produção de coletores no alvo, fornecendo telemetria de ponta a ponta. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Determinado a descartar pedaços, esses backlogs monitoram possíveis falhas em relação às cargas úteis de maior intensidade. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

Use a linha JSON, use o chicote de fios, use o raspador Prometheus, faça o seu trabalho de governança, sua governança просит доказательства, что alarmes de contrapressão соответствуют топологии de ensaio.

## Verifique a conformidade

1. Crie um novo arquivo routed-trace e instale-o em apenas alguns lugares.
2. Abra a tabela de monitoramento do Alertmanager de acompanhamento, que é a opção de download - feche o tíquete.
3. Você pode configurar os deltas, rastrear o rastreador, ele e o resumo digerem o pacote de telemetria em uma solicitação pull recente.
4. Ссылайтесь здесь на новые ensaio/telemetria артефакты, чтобы будущие обновления roteiro ссылались на единый документ, e não há configurações ad-hoc.

## Индекс доказательств

| Ativo | Localização | Nomeação |
|-------|----------|-------|
| Relatório de auditoria de rastreamento roteado (1º trimestre de 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Канонический источник доказательств Fase B1; зеркалируется no porta через `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Configurar rastreador delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Содержит TRACE-CONFIG-DELTA diff resumos, revisores iniciais e log голосования GOV-2026-03-19. |
| Plano de remediação de telemetria | `docs/source/nexus_telemetry_remediation_plan.md` | Pacote de alerta de documentação, tamanho do lote OTLP e grades de proteção do orçamento de exportação, связанные с B2. |
| Rastreador de ensaio com várias pistas | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Список артефактов репетиции 9 de abril, manifesto/resumo do validador, notas/agenda do segundo trimestre e evidências de reversão. |
| Manifesto/resumo do pacote de telemetria (mais recente) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+`.sha256`) | Faixa de slot de gerenciamento 912-936, semente `NEXUS-REH-2026Q2` e hashes são pacotes de governança de artefatos. |
| Manifesto do perfil TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+`.sha256`) | Hash утвержденного perfil TLS, executado na reexecução do segundo trimestre; consulte os apêndices do routed-trace. |
| Agenda TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Плановые заметки для ensaio Q2 (janela, intervalo de slots, semente de carga de trabalho, proprietários de ação). |
| Lançar manual de ensaios | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Операционный чеклист staging -> execução -> rollback; обновлять при изменении топологии pistas ou exportadores de orientação. |
| Validador de pacote de telemetria | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI, упомянутый no B4 ретро; архивируйте digests вместе с tracker при любом изменении pack. |
| Regressão multilane | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Prove `nexus.enabled = true` para configurações multi-lane, obtenha hashes de catálogo Sora e forneça lane-local Kura/merge-log paths (`blocks/lane_{id:03}_{slug}`) por `ConfigLaneRouter` перед публикацией resumos de artefatos. |