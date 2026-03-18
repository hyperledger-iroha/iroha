---
lang: pt
direction: ltr
source: docs/portal/docs/nexus/transition-notes.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: notas de transição do nexo
título: ملاحظات انتقال Nexus
description: Verifique o `docs/source/nexus_transition_notes.md`, verifique a Fase B e a fase B.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# ملاحظات انتقال Nexus

يتتبع هذا السجل العمل المتبقي من **Fase B - Nexus Fundações de Transição** حتى تكتمل قائمة فحص اطلاق الـ multi-lane. Você pode usar o código `roadmap.md` para obter o código B1-B4 e fazer o download O SDK e o SRE estão disponíveis para download.

## النطاق والوتيرة

- يغطي تدقيقات routed-trace e guardrails للتيليمتري (B1/B2), ومجموعة deltas للتكوين المعتمدة من الحوكمة (B3), Verifique as pistas (B4).
- يستبدل ملاحظة الوتيرة المؤقتة التي كانت هنا؛ No primeiro trimestre de 2026, o relatório foi publicado em `docs/source/nexus_routed_trace_audit_report_2026q1.md`, o que significa que ele está disponível para download. التشغيلي e سجل التخفيفات.
- حدث الجداول بعد كل نافذة routed-trace e تصويت حوكمة او تدريب اطلاق. عندما تتحرك artefatos, اعكس الموقع الجديد داخل هذه الصفحة كي تتمكن الوثائق اللاحقة (status, painéis, بوابات SDK) está disponível para download.

## لقطة ادلة (2026 Q1-Q2)

| مسار العمل | الادلة | الملاك | الحالة | Produtos |
|------------|----------|----------|--------|-------|
| **B1 – Auditorias de rastreamento roteado** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetria-ops, @governança | Relatório (1º trimestre de 2026) | تم تسجيل ثلاث نوافذ تدقيق؛ Se você tiver TLS em `TRACE-CONFIG-DELTA`, execute novamente o Q2. |
| **B2 - Remediação de telemetria e guarda-corpos** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetria-ops | مكتمل | Você pode usar o pacote de alertas e o diff bot e o OTLP (`nexus.scheduler.headroom` log + espaço livre no Grafana); Não há isenções. |
| **B3 - Aprovações delta de configuração** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | مكتمل | تم تسجيل تصويت GOV-2026-03-19; الحزمة الموقعة تغذي pacote de telemetria المذكور ادناه. |
| **B4 - Ensaio de lançamento em várias pistas** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | Relatório (2º trimestre de 2026) | A reexecução do segundo trimestre foi realizada no TLS; O manifesto do validador + `.sha256` contém os slots 912-936 e a semente da carga de trabalho `NEXUS-REH-2026Q2` e o hash do TLS para a reexecução. |

## جدول تدقيق routed-trace ربع السنوي

| ID de rastreamento | النافذة (UTC) | النتيجة | Produtos |
|----------|-------------|---------|-------|
| `TRACE-LANE-ROUTING` | 17/02/2026 09:00-09:45 | Não | ظل Queue-admission P95 اقل بكثير من الهدف <=750 ms. Não é nada disso. |
| `TRACE-TELEMETRY-BRIDGE` | 24/02/2026 10h00-10h45 | Não | Para usar hashes de repetição OTLP em `status.md`; A paridade é definida pelo SDK diff bot e pelo drift. |
| `TRACE-CONFIG-DELTA` | 01/03/2026 12h00-12h30 | تم الحل | Se o TLS não for executado novamente no Q2; Adicione o pacote de telemetria ao hash `NEXUS-REH-2026Q2` do TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (`artifacts/nexus/tls_profile_rollout_2026q2/`) e o pacote de dados. |
| `TRACE-MULTILANE-CANARY` | 05/05/2026 09:12-10:14 | Não | Semente de carga de trabalho `NEXUS-REH-2026Q2`; pacote de telemetria + manifesto/resumo em `artifacts/nexus/rehearsals/2026q1/` (intervalo de slots 912-936) com agenda em `artifacts/nexus/rehearsals/2026q2/`. |

يجب على الفصول القادمة اضافة صفوف جديدة ونقل الادخالات المكتملة الى ملحق عندما يتجاوز الجدول الربع الحالي. O roteador está usando o routed-trace e o roteador `#quarterly-routed-trace-audit-schedule`.

## Lista de pendências e pendências

| العنصر | الوصف | المالك | الهدف | الحالة / الملاحظات |
|------|------------|-------|--------|----------------|
| `NEXUS-421` | Para que o TLS seja executado em `TRACE-CONFIG-DELTA`, você pode executar uma nova execução e executar uma nova execução. | @release-eng, @sre-core | Rastreamento roteado no segundo trimestre de 2026 | Exemplo - hash de TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` associado a `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; Você pode executar novamente uma nova versão. |
| Preparação `TRACE-MULTILANE-CANARY` | Depois de Q2, ارفاق fixtures بحزمة التيليمتري وضمان اعادة استخدام SDK chicotes para للمساعد المعتمد. | @telemetry-ops, Programa SDK | اجتماع التخطيط 2026-04-30 | مكتمل - agenda محفوظة em `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` com slot de metadados/carga de trabalho; Você deve usar o arnês do rastreador. |
| Rotação de resumo do pacote de telemetria | O `scripts/telemetry/validate_nexus_telemetry_pack.py` é definido como /Release e digests do tracker do config delta. | @telemetria-ops | Para release candidate | Exemplo - `telemetry_manifest.json` + `.sha256` صدرت em `artifacts/nexus/rehearsals/2026q1/` (intervalo de slots `912-936`, semente `NEXUS-REH-2026Q2`); تم نسخ digere o rastreador e o rastreador. |

## دمج حزمة config delta- Verifique `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`. No `defaults/nexus/*.toml` você pode usar o genesis e o rastreador para isso.
- تغذي pacotes de configuração assinados حزمة تيليمتري التدريب. يجب نشر الحزمة, التي تم التحقق منها عبر `scripts/telemetry/validate_nexus_telemetry_pack.py`, بجانب ادلة config delta لكي يتمكن المشغلون من اعادة تشغيل نفس artefatos المستخدمة em B4.
- تبقى حزم Iroha 2 بدون lanes: configs مع `nexus.enabled = false` ترفض الان overrides لـ lane/dataspace/routing ما لم يتم تفعيل ملف Nexus (`--sora`), mas o `nexus.*` é de pista única.
- ابق سجل تصويت الحوكمة (GOV-2026-03-19) مرتبطا من tracker ومن هذه الملاحظة لكي تتمكن التصويتات A maioria das pessoas não sabe o que fazer.

## متابعات تدريب الاطلاق

- يسجل `docs/source/runbooks/nexus_multilane_rehearsal.md` خطة الكاناري وقائمة المشاركين وخطوات rollback; O runbook é um runbook que pode ser usado em pistas e exportadores.
- يسرد `docs/source/project_tracker/nexus_rehearsal_2026q1.md` كل artefato تم التحقق منه في تدريب 9 ابريل ويشمل الان ملاحظات/agenda تحضير Q2. اضف التدريبات المستقبلية الى نفس tracker بدلا من فتح trackers مستقلة للحفاظ على تسلسل الادلة.
- Crie snippets para OTLP e exporte para Grafana (راجع `docs/source/telemetry.md`) para exportar lotes para o exportador; O tamanho do lote do primeiro trimestre foi de 256 vezes o headroom.
- Usar CI/testes em vários canais em `integration_tests/tests/nexus/multilane_pipeline.rs` e usar o fluxo de trabalho `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`) Código de erro `pytests/nexus/test_multilane_pipeline.py`; O hash de `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) pode ser usado como rastreador para pacotes de pacotes.

## دورة حياة pistas em وقت التشغيل

- خطط دورة حياة lanes في وقت التشغيل تتحقق الان من bindings الخاصة بـ dataspace وتتوقف عند فشل reconciliation لـ Kura/التخزين الطبقي, مع ترك الكتالوج دون تغيير. Os ajudantes são os relés das pistas para as pistas e as provas para sintetizar o merge-ledger.
- طبق الخطط عبر helpers الخاصة بـ config/lifecycle em Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) لاضافة/سحب pistas دون اعادة تشغيل; Você pode usar roteamento, instantâneos TEU e registros de manifestos.
- ارشاد للمشغلين: عند فشل الخطة تحقق من dataspaces المفقودة او raízes de armazenamento التي لا يمكن انشاؤها (tiered cold root/مجلدات Kura لكل pista). اصلح المسارات الاساسية e مجددا؛ O diff pode ser usado para lane/dataspace ou para painéis de controle.

## تيليمتري NPoS e contrapressão

طلبت مراجعة تدريب الاطلاق لمرحلة Fase B التقطات تيليمتري حتمية تثبت ان pacemaker الخاص بـ NPoS وطبقات fofoca تبقى ضمن حدود contrapressão. O chicote de fios do `integration_tests/tests/sumeragi_npos_performance.rs` é usado para definir o valor do JSON e o JSON (`sumeragi_baseline_summary::<scenario>::...`) قياسات جديدة. Veja mais:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

Use `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` e `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` para obter mais informações O código de barras é de 1 s/`k=3` no B4.

| Teste / teste | التغطية | Produtos de limpeza |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Há 12 dias de tempo de bloco, tempo de bloqueio, envelopes, latência EMA, latência EMA, medidores e envio redundante, pacote configurável sim. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | يغمر طابور المعاملات لضمان تفعيل diferimentos para admissão بشكل حتمي وتصدير العدادين للقدرة/التشبع. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | يلتقط jitter للـ pacemaker وtimeouts للـ view حتى يثبت تطبيق نطاق +/-125 permille. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | يدفع payloads RBC كبيرة حتى حدود soft/hard para store ليظهر ارتفاع جلسات وعدادات bytes ثم تراجعها واستقرارها دون تجاوز store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | يفرض اعادة ارسال حتى تتقدم medidores نسبة envio redundante e coletores no alvo, مؤكدا ان التيليمتري المطلوبة مرتبطة de ponta a ponta. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Os pedaços são armazenados no backlog e nas falhas e nas cargas úteis. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

ارفق اسطر JSON التي يطبعها chicote مع scrape الخاص بـ Prometheus الملتقط اثناء التشغيل كلما طلبت الحوكمة ادلة A contrapressão não pode ser liberada.

## قائمة التحديث1. Use o routed-trace para definir o valor do roteador.
2. Verifique se o Alertmanager está disponível para você no site do Alertmanager.
3. Você pode configurar deltas de configuração, rastrear rastreador e processar e digests digests do pacote de telemetria em pull request.
4. اربط هنا اي artefato جديد للتدريب/التيليمتري كي تشير تحديثات roadmap المستقبلية الى مستند واحد Não há soluções ad-hoc.

## فهرس الادلة

| الاصل | الموقع | Produtos |
|-------|----------|-------|
| Routed-trace (1º trimestre de 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | A fase B1 da fase B1; Não use o `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Tracker خاص بـ config delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Você pode usar o TRACE-CONFIG-DELTA e o arquivo GOV-2026-03-19. |
| Reparação de remediação | `docs/source/nexus_telemetry_remediation_plan.md` | O pacote de alertas contém OTLP e guardrails ميزانية التصدير المرتبطة بـ B2. |
| Rastreador multi-lane | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | يسرد artefacts تدريب 9 ابريل وmanifest/digest الخاص بالـ validator وملاحظات/agenda Q2 وادلة rollback. |
| Manifesto/resumo do pacote de telemetria (mais recente) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+`.sha256`) | O intervalo de slots 912-936 e a semente `NEXUS-REH-2026Q2` e os artefatos de hashes são os mais importantes. |
| Manifesto do perfil TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+`.sha256`) | hash ملف TLS المعتمد الملتقط خلال reexecutar Q2; Isso é feito através do routed-trace. |
| Agenda TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | ملاحظات التخطيط لتدريب Q2 (alcance de slot, semente de carga de trabalho, مالكو الاجراءات). |
| Runbook تدريب الاطلاق | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Lista de verificação para teste -> execução -> reversão; حدثها عند تغير طوبولوجيا pistas e exportadores. |
| Validador de pacote de telemetria | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI مشار اليه في retro B4; ارشف digests بجانب tracker عند تغير الحزمة. |
| Regressão multilane | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | `nexus.enabled = true` configs multi-lane, ويحافظ على Sora catalog hashes ويهيئ مسارات Kura/merge-log para lane (`blocks/lane_{id:03}_{slug}`) عبر `ConfigLaneRouter` é um resumo dos artefatos. |