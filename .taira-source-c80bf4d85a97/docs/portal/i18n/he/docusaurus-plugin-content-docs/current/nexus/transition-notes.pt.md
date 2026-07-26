---
lang: he
direction: rtl
source: docs/portal/docs/nexus/transition-notes.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-transition-notes
כותרת: Notas de transicao do Nexus
תיאור: Espelho de `docs/source/nexus_transition_notes.md`, cobrindo evidencia de transicao da Phase B, o calendario de auditoria e as mitigacoes.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Notas de transicao do Nexus

Este registro acompanha o trabalho pendente de **Phase B - Nexus Transition Foundations** ate que a checklist de lancamento multi-lane termine. אלה משלימים כמו אבני דרך ב-`roadmap.md` והוכחות ל-B1-B4 ב-Unico lugar para que governanca, SRE e lideres de SDK compartilhem a mesma fonte de verdade.

## Escopo e cadencia

- Cobre כמו auditorias מנותב-trace e os מעקות בטיחות de telemetria (B1/B2), o conjunto de deltas de configuracao aprovado por governanca (B3) e os acompanhamentos do ensaio de lancamento multi-lane (B4).
- Substitui a nota temporaria de cadencia que antes vivia aqui; a partir da auditoria de Q1 2026 o Relatorio detalhado reside em `docs/source/nexus_routed_trace_audit_report_2026q1.md`, enquanto esta page mantem o calendario corrente e o registro de mitigacoes.
- להגדיר כ- tableas apos cada janela routed-trace, voto de governanca ou ensaio de lancamento. Quando os artefatos se moverem, reflita a nova localizacao dentro desta pagina para que os docs posteriores (סטטוס, לוחות מחוונים, Portais SDK) possam linkar um ancoradouro estavel.

## תמונת מצב של evidencia (2026 Q1-Q2)

| זרם עבודה | Evidencia | בעל/ים | סטטוס | Notas |
|------------|--------|--------|--------|------|
| **B1 - ביקורת מעקב מנותב** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @ממשל | Completo (Q1 2026) | Tres janelas de auditoria registradas; o atraso TLS de `TRACE-CONFIG-DELTA` foi fechado durante או שידור חוזר של Q2. |
| **B2 - מעקות בטיחות של Remediacao de telemetria e** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | קומפלטו | חבילת התראה, פוליטיקה לעשות הבדל בוט ותפעול OTLP (`nexus.scheduler.headroom` יומן + כאב Grafana מרווח ראש) sem waivers em aberto. |
| **B3 - Aprovacoes de deltas de configuracao** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | קומפלטו | Voto GOV-2026-03-19 registrado; o צרור assinado alimenta o pack de telemetria citado abaixo. |
| **B4 - Ensaio de lancamento מרובה נתיבים** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Completo (Q2 2026) | או שידור חוזר Canary de Q2 fechou a mitigacao do atraso TLS; o מניפסט מאמת + `.sha256` קבלה או מרווחי משבצות 912-936, תחילת עומס עבודה `NEXUS-REH-2026Q2` או hash לבצע רישום TLS ללא הפעלה חוזרת. |

## Calendario trimestral de auditorias מנותב-עקבות| מזהה מעקב | ג'נלה (UTC) | תוצאות | Notas |
|--------|--------------|--------|------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | אפרובדו | כניסה לתור P95 ficou bem abaixo do alvo <=750 ms. Nenhuma acao necessaria. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | אפרובדו | השידור החוזר של OTLP משבש anexados a `status.md`; a paridade do SDK diff bot מאשר אפס סחיפה. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | Resolvido | O atraso TLS foi fechado durante או שידור חוזר של Q2; o pack de telemetria para `NEXUS-REH-2026Q2` registra o hash do perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (ver `artifacts/nexus/tls_profile_rollout_2026q2/`) e zero atrasados. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | אפרובדו | seed עומס עבודה `NEXUS-REH-2026Q2`; pack de telemetria + manifest/digest em `artifacts/nexus/rehearsals/2026q1/` (טווח משבצות 912-936) com agenda em `artifacts/nexus/rehearsals/2026q2/`. |

Os trimestres futuros devem adicionar novas linhas e mover as entradas concluiidas para um apendice quando a tablea crescer alem do trimestre atual. Referencie esta secao a partir de relatorios מנותב-trace ou atas de governanca usando a ancora `#quarterly-routed-trace-audit-schedule`.

## מיטיגקועס e items de backlog

| פריט | תיאור | בעלים | אלבו | סטטוס / Notas |
|------|-------------|--------|--------|----------------|
| `NEXUS-421` | סיום הפרסום של TLS עבור `TRACE-CONFIG-DELTA`, הוכחות לשידור חוזר או רישום המיטיגקאו. | @release-eng, @sre-core | Janela routed-trace de Q2 2026 | Fechado - hash do perfil TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` capturado em `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; או הפעלה חוזרת של confirmou que nao ha atrasados. |
| `TRACE-MULTILANE-CANARY` הכנה | תוכנת Q2, אביזרי עזר וחבילת טלמטריה מספקת רתמות מחדש של SDK או אימות עוזר. | @telemetry-ops, תוכנית SDK | Chamada de planejamento 2026-04-30 | Completo - סדר היום armazenada em `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` com metadados de slot/load work; reutilizacao לעשות לרתום את anotada no tracker. |
| סיבוב ערכת טלמטריה | מבצע `scripts/telemetry/validate_nexus_telemetry_pack.py` לפני תחילת עבודה/שחרור e registrar digests ao lado do tracker de config delta. | @telemetry-ops | Por מועמד לשחרור | Completo - `telemetry_manifest.json` + `.sha256` emitidos em `artifacts/nexus/rehearsals/2026q1/` (טווח חריצים `912-936`, זרע `NEXUS-REH-2026Q2`); מעכל copiados no tracker e no indice de evidencia. |

## Integracao do bundle de config delta- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` כמו רזומה קנוניקו דה הבדלים. Quando chegarem novos `defaults/nexus/*.toml` או mudancas de genesis, לממש את esse tracker primeiro e depois reflita os destaques aqui.
- חבילות תצורה חתומות של מערכת הפעלה או חבילת טלמטריה. O pack, validado por `scripts/telemetry/validate_nexus_telemetry_pack.py`, deve ser publicado junto com a evidencia de config delta para que os operadores possam reproduzir os artefatos exatos usados ​​durante B4.
- Os bundles de Iroha 2 permanecem sem lanes: configs com `nexus.enabled = false` agora rejeitam עוקף את הנתיב/מרחב הנתונים/ניתוב של menos que o perfil Nexus esteja habilitado (0100o 1X), as secoes `nexus.*` זה תבניות חד-נתיב.
- Mantenha o log de voto de governanca (GOV-2026-03-19) linkado tanto no tracker quanto nesta not para que futuros votos possam copiar o formato sem redescobrir o ritual de aprovacao.

## Acompanhamentos do ensaio de lancamento

- `docs/source/runbooks/nexus_multilane_rehearsal.md` Captura o Plano Canary, List de participantes e os passos de rollback; להטמיע את ה-Runbook כדי לקבל טופולוגית מסלולים או יצואנים של טלמטריה מודרם.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` רשימה של קוד ה-Artefato checado durante o ensaio de 9 de abril e agora inclui notas/agenda de preparacao Q2. Adicione ensaios futuros ao mesmo tracker em vez de abrir trackers isolados para manter a evidencia monotona.
- Publique snippets do coletor OTLP e exports do Grafana (ver `docs/source/telemetry.md`) quando a orientacao de batching do exporter mudar; רמה גבוהה של Q1 או גודל אצווה עבור 256 אמוסטרס להצגת התראות על מרווח ראש.
- A evidencia de CI/tests multi-lane agora vive em `integration_tests/tests/nexus/multilane_pipeline.rs` e roda sob o workflow `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), substituindo a referencia aposentada `pytests/nexus/test_multilane_pipeline.py`; mantenha o hash de `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) em sync com o tracker ao atualizar bundles de ensaio.

## זמן ריצה

- Os planos de ciclo de vida de lanes em runtime agora validam bindings de dataspace e abortam quando a reconciliacao Kura/armazenamento em caadas falha, mantendo o catalogo inalterado. Os helpers podam relays de lanes em cache para lanes aposentadas, para que a sintese merge-ledger nao reutilize הוכחות מיושנות.
- Aplique planos pelos helpers de config/lifecycle do Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) para adicionar/retirar tranes sem reinicio; ניתוב, תמונות Snapshot TEU e registries de manifests reregreggam automaticamente apos um plano bem-sucedido.
- Orientacao para operadores: quando um plano falha, verifique dataspaces ausentes ou storage roots que nao podem ser criados (שורש קר מדורג/diretorios Kura por lane). Corrija os caminhos base e tente novamente; planos bem-sucedidos re-emitem o diff de telemetria de lane/dataspace para que os dashboards reflitam a nova topologia.

## Telemetria NPoS evidencia de backpressureO retro do ensaio de lancamento da Phase B pediu capturas de telemetria deterministas que proem que o קוצב NPoS e as caadas de gossip permanecem dentro de seus limites depressure back. הרתמה אינטגרלית עם `integration_tests/tests/sumeragi_npos_performance.rs` תרגיל את התסריטים ואת קורות החיים של JSON (`sumeragi_baseline_summary::<scenario>::...`) quando novas metricas chegam. בצע localmente com:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Defina `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` או `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` לבחינת טופולוגיות עיקריות; os valores padrao refletem o perfil de coletores 1 s/`k=3` usado em B4.

| Cenario / מבחן | קוברטורה | Telemetria chave |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Bloqueia 12 rodadas com o block time do ensaio para registrar envelopes de latencia EMA, profundidades de fila e gauges de overundant-send antes de serializar o bundle de evidencia. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | Inunda a fila de transacoes para gararir que as deferrals de admissao sejam ativadas de forma determinista e que a fila exporte contadores de capacidade/saturacao. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Amostra o jitter do קוצב e os timeouts de view ate provar que a banda +/-125 permille e aplicada. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | מטענים של Empurra RBC grandes אכלו את מערכות ההפעלה רך/קשה לאחסן עבור רוב החנויות וחברות הבתים, חזרו והייצבו לחנות. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Forca retransmissoes para que os gauges de ratio redundant-send ו-os contadores de collectors-on-target avancem, provando que a telemetria pedida pelo retro esta conectada מקצה לקצה. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Descarta chunks em intervalos deterministas para verificar que os monitors de backlog levantam falhas em vez de drenar silenciosamente osloadloads. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

אנקס כמו linhas JSON que o לרתום imprime junto com o scrape do Prometheus capturado durante a execucao semper que a governanca solicitar Evidencia de que os alarmes de backpressure correspondem a topologia do ensaio.

## רשימת רשימות של atualizacao

1. Adicione novas janelas מנותב-trace e לפרוש כמו antigas quando os trimestres girarem.
2. להגדיר טבלה דה מיטיגקאו apos cada acompanhamento do Alertmanager, mesmo que a acao seja fechar o ticket.
3. תצורת מערכת ההפעלה של Quando, תגדיר את המעקב, ותאם את רשימת התקצירים.
4. Linke aqui qualquer novo artefato de ensaio/telemetria para que futuras atualizacoes de roadmap possam referenciar um unico documento em vez de notas ad-hoc dispersas.

## Indice de Evidencia| אטיו | Localizacao | Notas |
|-------|----------------|-------|
| Relatorio de auditoria מנותב-עקבות (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Fonte canonica da evidencia de Phase B1; espelhado para o portal em `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Tracker de config delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | קורות חיים שונים TRACE-CONFIG-DELTA. |
| Plano de remediacao de telemetria | `docs/source/nexus_telemetry_remediation_plan.md` | תעודה של חבילת התראה, או תומכת לוט OTLP ומעקות בטיחות של אורקמנטו דה יצוא וינקולאדוס B2. |
| Tracker de ensaio רב נתיב | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | Lista os artefatos do ensaio de 9 de abril, Manifest/digest do validator, notas/agenda Q2 evidencia de rollback. |
| מניפסט/תקציר של חבילת טלמטריה (האחרונה ביותר) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | רישום של טווח חריצים 912-936, seed `NEXUS-REH-2026Q2` e hashes de artefatos para bundles de governanca. |
| מניפסט פרופיל TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Hash do perfil TLS aprovado capturado durante או שידור חוזר של Q2; cite em apendices routed-trace. |
| סדר היום TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Notas de planejamento para o ensaio Q2 (ג'אנלה, טווח משבצות, סיד של עומס עבודה, בעלים של אקו). |
| Runbook de ensaio de lancamento | `docs/source/runbooks/nexus_multilane_rehearsal.md` | רשימת רשימת פעולות עבור בימוי -> execucao -> חזרה לאחור; atualizar quando a topologia de lanes או orientacao de exporters mudar. |
| אימות ערכת טלמטריה | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI referenciado pelo retro B4; arquive digests ao lado do tracker semper que o pack mudar. |
| רגרסיה רב מסלולית | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Prova `nexus.enabled = true` עבור תצורות ריבוי נתיבות, שמירה על ערכי גיבוב לקטלוג. |