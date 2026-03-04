---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-telemetria-remediação
título: Plano de operação de teste de telefonia Nexus (B2)
descrição: Зеркало `docs/source/nexus_telemetry_remediation_plan.md`, documentação de matriz de teste de telemetria e operação de trabalho.
---

#Обзор

Roteiro de ponto **B2 - владение пробелами телеметрии** требует опубликованного plano, который привязывает каждый оставшийся пробел телеметрии Nexus к сигналу, защитному порогу оповещений, владельцу, дедлайну и O projeto de arte foi feito para a auditoria do primeiro trimestre de 2026. Esta página é `docs/source/nexus_telemetry_remediation_plan.md`, engenharia de liberação, operações de telemetria e SDK instalados Você pode usar o método de repetição de rastreamento roteado e `TRACE-TELEMETRY-BRIDGE`.

#Матрица пробелов

| ID da lacuna | Sinalização e divulgação de segurança | Владелец / эскалация | Crocodilo (UTC) | Documentação e prova |
|----|-------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | Гистограмма `torii_lane_admission_latency_seconds{lane_id,endpoint}` com alerta **`SoranetLaneAdmissionLatencyDegraded`**, instale o `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` na técnica 5 minutos (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (сигнал) + `@telemetry-ops` (alert); эскалация через rastreamento roteado de plantão Nexus. | 23/02/2026 | Teste o alerta em `dashboards/alerts/tests/soranet_lane_rules.test.yml` e verifique as repetições `TRACE-LANE-ROUTING` com alerta e recuperação e raspagem Torii `/metrics` em [Notas de transição Nexus](./nexus-transition-notes). |
| `GAP-TELEM-002` | A chave `nexus_config_diff_total{knob,profile}` com guarda-corpo `increase(nexus_config_diff_total{profile="active"}[5m]) > 0`, conjunto de bloco (`docs/source/telemetry.md`). | `@nexus-core` (exemplo) -> `@telemetry-ops` (alterado); дежурный по governança пейджится при неожиданном росте счетчика. | 26/02/2026 | Выходы simulação de governança сохраняются рядом с `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; A tela de seleção da tela de proteção Prometheus é exibida e os logotipos abertos são exibidos como `StateTelemetry::record_nexus_config_diff` diferença do general. |
| `GAP-TELEM-003` | Событие `TelemetryEvent::AuditOutcome` (metriz `nexus.audit.outcome`) com alerta **`NexusAuditOutcomeFailure`** por segurança ou falha A resolução dura mais de 30 minutos (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) está listado em `@sec-observability`. | 27/02/2026 | CI-гейт `scripts/telemetry/check_nexus_audit_outcome.py` архивирует NDJSON payloads e падает, porque o TRACE não содержит события успеха; A tela de alerta é exibida para que o routed-trace seja aberto. |
| `GAP-TELEM-004` | Medidor `nexus_lane_configured_total` com guardrail `nexus_lane_configured_total != EXPECTED_LANE_COUNT`, который питает on-call чеклист SRE. | `@telemetry-ops` (manômetro/exportação) está escalado em `@nexus-core`, que está disponível em um catálogo desigual. | 28/02/2026 | O plano de telemetria `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` pode ser testado; O operador seleciona o diff Prometheus + abre o log `StateTelemetry::set_nexus_catalogs` nas repetições do pacote TRACE. |

# Операционный рабочий processo

1. **Еженедельный триаж.** Владельцы отчитываются о прогрессе на Nexus prontidão созвоне; Os bloqueadores e artefatos testam a resolução de alertas em `status.md`.
2. **Alertas de simulação.** Каждое правило алерта поставляется вместе с записью `dashboards/alerts/tests/*.test.yml`, чтобы CI запускал `promtool test rules` com guarda-corpo personalizado.
3. **Confira para a auditoria.** Na repetição `TRACE-LANE-ROUTING` e `TRACE-TELEMETRY-BRIDGE` дежурный собирает результаты Use Prometheus, histórico de alertas e scripts relevantes (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` para conexão сигналов) e сохраняет их вместе com artefatos de rastreamento roteado.
4. **Escolamento.** O corrimão de segurança foi colocado em uma posição repetida, o comando-владелец открывает Nexus, ссылаясь на этот план, e прикладывает snapshot métrica e шаги по снижению риска перед возобновлением аудитов.

Com a matriz aberta - e сссылками из `roadmap.md` e `status.md` - roteiro de pontos **B2** теперь соответствует критериям приемки "ответственность, срок, алерт, проверка".