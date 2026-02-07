---
lang: he
direction: rtl
source: docs/portal/docs/nexus/telemetry-remediation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: nexus-telemetry-remediation
כותרת: Plano de remediacao de telemetria do Nexus (B2)
תיאור: Espelho de `docs/source/nexus_telemetry_remediation_plan.md`, דוקומנטרי למאטריז דה lacunas de telemetria e o fluxo operational.
---

# Visao Geral

O item do מפת הדרכים **B2 - בעלות דה lacunas de telemetria** exige um plano publicado que vincule cada lacuna de telemetria pendente do Nexus a um sinal, um guard de alerta, um responsavel, um prazo e um artefato de verificacao das1 2026. Esta pagina espelha `docs/source/nexus_telemetry_remediation_plan.md` עבור הנדסת שחרור, טלמטריה ואופציות ובעלי מערכות SDK מאשרים את ה-SDK ניתוב מעקב ו-`TRACE-TELEMETRY-BRIDGE`.

# Matriz de lacunas

| ID da lacuna | מעקה בטיחות סינאל e de alerta | Responsavel / escalonamento | פראזו (UTC) | Evidencia e verificacao |
|--------|------------------------|------------------------|-----------|------------------------|
| `GAP-TELEM-001` | Histograma `torii_lane_admission_latency_seconds{lane_id,endpoint}` com alerta **`SoranetLaneAdmissionLatencyDegraded`** disparando quando `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` ל-5 דקות (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (סינאל) + `@telemetry-ops` (התראה); escalonamento באמצעות כוננות מנותב-trace do Nexus. | 23-02-2026 | Testes de alerta em `dashboards/alerts/tests/soranet_lane_rules.test.yml` mais o registro do ensaio `TRACE-LANE-ROUTING` mostrando alerta disparado/recuperado e o scrape Torii `/metrics` arquivado emIX00 transition108NT06 הערות](./nexus-transition-notes). |
| `GAP-TELEM-002` | Contador `nexus_config_diff_total{knob,profile}` מעקה בטיחות `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` bloqueando פורס (`docs/source/telemetry.md`). | `@nexus-core` (instrumentacao) -> `@telemetry-ops` (תראה); oficial de governanca e pagedo quando o contador incrementa de forma inesperada. | 2026-02-26 | Saidas de dry-run de governanca armazenadas ao lado de `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; רשימת שחרור כוללת את התייעצות Prometheus, אך לא ניתן להבחין ב-`StateTelemetry::record_nexus_config_diff`. |
| `GAP-TELEM-003` | Evento `TelemetryEvent::AuditOutcome` (metrica `nexus.audit.outcome`) com alerta **`NexusAuditOutcomeFailure`** quando falhas ou resultados ausentes persistem por >30 minutes (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (צינור) com escalonamento para `@sec-observability`. | 2026-02-27 | O gate de CI `scripts/telemetry/check_nexus_audit_outcome.py` arquiva loads NDJSON e falha quando uma janela TRACE נאו טם אירוע של הצלחה; מעקב אחר התראה ומעקב. |
| `GAP-TELEM-004` | מד `nexus_lane_configured_total` מעקה בטיחות com `nexus_lane_configured_total != EXPECTED_LANE_COUNT` alimentando רשימת בדיקה ב-call de SRE. | `@telemetry-ops` (מד/ייצוא) com escalonamento para `@nexus-core` quando os nos reportam tamanhos de catalogo inconsistentes. | 2026-02-28 | O teste de telemetria do scheduler `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` comprova a missao; מפעילים anexam diff de Prometheus + trecho de log `StateTelemetry::set_nexus_catalogs` ao pacote do ensaio TRACE. |

# Fluxo מבצעי1. **טריאגם סמנאלי.** בעלים מדווחים על התקדמותם של מוכנות יד Nexus; חוסמי e artefatos de testes de alerta sao registrados em `status.md`.
2. **Ensaios de alertas.** Cada regra de alerta e entregue junto com uma entrada `dashboards/alerts/tests/*.test.yml` para que o CI execute `promtool test rules` quando o מעקה בטיחות mudar.
3. **Evidencia de auditoria.** Durante os ensaios `TRACE-LANE-ROUTING` e `TRACE-TELEMETRY-BRIDGE` o כוננות לכוננות תוצאות של ייעוץ Prometheus, היסטוריית התראות e saidas0018NI scripts (Prometheus `scripts/telemetry/check_redaction_status.py` para sinais correlacionados) e as armazena com os artefatos routed-trace.
4. **Escalonamento.** Se algum guard disparar fora de uma janela ensaiada, a equipe responsavel abre um ticket de incidente Nexus referenciando este plano, incluindo o snapshot da metrica e os passos de mittomcao an audites de mittomcao.

Com esta matriz publicada - e referenciada em `roadmap.md` e `status.md` - o item de roadmap **B2** agora atende aos criterios de aceitacao "responsabilidade, prazo, alerta, verificacao".