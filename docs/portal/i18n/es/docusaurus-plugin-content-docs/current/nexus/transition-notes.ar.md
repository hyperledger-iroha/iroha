---
lang: es
direction: ltr
source: docs/portal/docs/nexus/transition-notes.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: notas-de-transición-nexus
título: ملاحظات انتقال Nexus
descripción: نسخة مطابقة لـ `docs/source/nexus_transition_notes.md`, تغطي ادلة انتقال Phase B وجدول التدقيق وخطط التخفيف.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# ملاحظات انتقال Nexus

يتتبع هذا السجل العمل المتبقي من **Fase B - Nexus Cimientos de transición** حتى تكتمل قائمة فحص اطلاق الـ multicarril. وهو يكمل بنود المعالم في `roadmap.md` ويحفظ الادلة المشار اليها في B1-B4 في مكان واحد حتى تتمكن فرق الحوكمة El SRE y el SDK son compatibles con el software.

## النطاق والوتيرة

- يغطي تدقيقات routed-trace وحواجز guardrails للتيليمتري (B1/B2), ومجموعة deltas للتكوين المعتمدة من الحوكمة (B3), ومتابعات تدريب Carriles de acceso directo (B4).
- يستبدل ملاحظة الوتيرة المؤقتة التي كانت هنا؛ منذ تدقيق Q1 2026 يوجد التقرير المفصل في `docs/source/nexus_routed_trace_audit_report_2026q1.md`, بينما تحتفظ هذه الصفحة بالجدول التشغيلي وسجل التخفيفات.
- Haga clic en el rastreo enrutado y haga clic en él. عندما تتحرك artefactos, اعكس الموقع الجديد داخل هذه الصفحة كي تتمكن الوثائق اللاحقة (estado, paneles, بوابات SDK) desde الارتباط بمرساة ثابتة.

## لقطة ادلة (2026 Q1-Q2)| مسار العمل | الادلة | الملاك | الحالة | ملاحظات |
|------------|----------|----------|--------|-------|
| **B1 - Auditorías de seguimiento enrutado** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @operaciones de telemetría, @gobernanza | مكتمل (primer trimestre de 2026) | تم تسجيل ثلاث نوافذ تدقيق؛ Para ejecutar TLS y `TRACE-CONFIG-DELTA`, vuelva a ejecutar Q2. |
| **B2 - Remediación de telemetría y barandillas** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | مكتمل | Paquete de alertas, bot de diferencias y OTLP (`nexus.scheduler.headroom` log + headroom de Grafana) لا توجد exenciones مفتوحة. |
| **B3 - Configurar aprobaciones delta** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | مكتمل | تم تسجيل تصويت GOV-2026-03-19؛ الحزمة الموقعة تغذي paquete de telemetría المذكور ادناه. |
| **B4 - Ensayo de lanzamiento de varios carriles** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | مكتمل (segundo trimestre de 2026) | Volver a ejecutar الكاناري في Q2 تخفيف فجوة TLS؛ El manifiesto del validador + `.sha256` incluye ranuras 912-936 y la semilla de carga de trabajo `NEXUS-REH-2026Q2` y el hash del código TLS para volver a ejecutar. |

## جدول تدقيق ruta de seguimiento ربع السنوي| ID de seguimiento | النافذة (UTC) | النتيجة | ملاحظات |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | ناجح | ظل Admisión de cola P95 اقل بكثير من الهدف <=750 ms. لا يلزم اي اجراء. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | ناجح | Utilice hashes de reproducción OTLP como `status.md`; La paridad del bot de diferenciación del SDK es una deriva. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | تم الحل | Para volver a ejecutar TLS, vuelva a ejecutar Q2 Este paquete de telemetría incluye hash `NEXUS-REH-2026Q2` y TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (`artifacts/nexus/tls_profile_rollout_2026q2/`) y aplicaciones. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | ناجح | Semilla de carga de trabajo `NEXUS-REH-2026Q2`; paquete de telemetría + manifiesto/resumen de `artifacts/nexus/rehearsals/2026q1/` (rango de ranura 912-936) de agenda de `artifacts/nexus/rehearsals/2026q2/`. |

يجب على الفصول القادمة اضافة صفوف جديدة ونقل الادخالات المكتملة الى ملحق عندما يتجاوز الجدول الربع الحالي. Utilice el rastreo enrutado y el dispositivo de seguimiento `#quarterly-routed-trace-audit-schedule`.

## Trabajos pendientes| العنصر | الوصف | المالك | الهدف | الحالة / الملاحظات |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | Para volver a ejecutar TLS, seleccione `TRACE-CONFIG-DELTA`, y vuelva a ejecutarlo. | @release-eng, @sre-core | نافذة routed-trace Q2 2026 | مغلق - hash ملف TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` ملتقط في `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; اكد volver a ejecutar عدم وجود متخلفات. |
| Preparación `TRACE-MULTILANE-CANARY` | جدولة تدريب Q2, ارفاق accesorios بحزمة التيليمتري وضمان اعادة استخدام SDK arneses للمساعد المعتمد. | @telemetry-ops, Programa SDK | اجتماع التخطيط 2026-04-30 | مكتمل - agenda محفوظة في `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` مع ranura de metadatos/carga de trabajo؛ تم توثيق اعادة استخدام arnés في tracker. |
| Rotación de resumen del paquete de telemetría | تشغيل `scripts/telemetry/validate_nexus_telemetry_pack.py` قبل كل تدريب/Release وتسجيل resúmenes بجانب tracker الخاص بـ config delta. | @operaciones de telemetría | لكل candidato de liberación | مكتمل - `telemetry_manifest.json` + `.sha256` صدرت في `artifacts/nexus/rehearsals/2026q1/` (rango de ranura `912-936`, semilla `NEXUS-REH-2026Q2`); تم نسخ resume el rastreador وفهرس الادلة. |

## Configuración delta delta- يظل `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` الملخص القانوني للفروقات. عند وصول `defaults/nexus/*.toml` جديدة او تغييرات genesis, حدث هذا tracker اولا ثم اعكس ابرز النقاط هنا.
- تغذي paquetes de configuración firmados حزمة تيليمتري التدريب. La configuración delta se realiza mediante `scripts/telemetry/validate_nexus_telemetry_pack.py`, y la configuración delta se establece en la configuración delta. artefactos المستخدمة في B4.
- Utilice Iroha 2 carriles: configuraciones de `nexus.enabled = false` y anule el carril/espacio de datos/enrutamiento para evitar el uso de Nexus. (`--sora`) ، لذا ازل اقسام `nexus.*` من قوالب de un solo carril.
- ابق سجل تصويت الحوكمة (GOV-2026-03-19) مرتبطا من tracker ومن هذه الملاحظة لكي تتمكن التصويتات المستقبلية من نسخ التنسيق دون اعادة اكتشاف طقوس الموافقة.

## متابعات تدريب الاطلاق- يسجل `docs/source/runbooks/nexus_multilane_rehearsal.md` خطة الكاناري وقائمة المشاركين y rollback؛ حدث runbook عند تغير طوبولوجيا carriles او exportadores التيليمتري.
- يسرد `docs/source/project_tracker/nexus_rehearsal_2026q1.md` كل artefacto تم التحقق منه في تدريب 9 ابريل ويشمل الان ملاحظات/agenda تحضير Q2. اضف التدريبات المستقبلية الى نفس tracker بدلا من فتح trackers مستقلة للحفاظ على تسلسل الادلة.
- Fragmentos de OTLP y exportaciones de Grafana (`docs/source/telemetry.md`) y exportadores de procesamiento por lotes تحديث Q1 رفع tamaño de lote الى 256 عينة لتجنب تنبيهات espacio libre.
- CI/pruebas de CI/pruebas de varios carriles con `integration_tests/tests/nexus/multilane_pipeline.rs` y flujo de trabajo `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`), para más información `pytests/nexus/test_multilane_pipeline.py`; El hash de `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) es un rastreador de paquetes de paquetes.

## دورة حياة carriles في وقت التشغيل- Hay carriles, enlaces, espacio de datos y reconciliación de Kura/التخزين, مع ترك الكتالوج دون تغيير. تقوم ayudantes بقص relés المخزنة لـ carriles المتقاعدة كي لا يعاد استخدام pruebas قديمة في sintetizar el libro mayor de fusión.
- Incluye ayudantes de configuración/ciclo de vida en Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) en carriles/líneas. تشغيل؛ Hay enrutamiento, instantáneas de TEU y registros de manifiestos.
- Uso de espacios de datos y raíces de almacenamiento (raíz fría escalonada/carril Kura لكل). اصلح المسارات الاساسية وحاول مجددا؛ Aquí hay una diferencia entre el carril/espacio de datos y los paneles de control.

## تيليمتري NPoS y contrapresión

طلبت مراجعة تدريب الاطلاق لمرحلة Phase B التقطات تيليمتري حتمية تثبت ان marcapasos الخاص بـ NPoS وطبقات chismes تبقى ضمن حدود contrapresión. Arnés de cables con `integration_tests/tests/sumeragi_npos_performance.rs`, archivos de configuración y archivos JSON (`sumeragi_baseline_summary::<scenario>::...`) para otros usuarios. جديدة. شغله محليا عبر:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

اضبط `SUMERAGI_NPOS_STRESS_PEERS` و `SUMERAGI_NPOS_STRESS_COLLECTORS_K` او `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` لاستكشاف طوبولوجيات اشد ضغطا؛ Conecte el cable de alimentación 1 s/`k=3` hasta B4.| السيناريو / prueba | التغطية | تيليمتري اساسية |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | Hay 12 veces el tiempo de bloqueo, los sobres, la latencia EMA, los indicadores y los calibres de envío redundante para el paquete. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | يغمر طابور المعاملات لضمان تفعيل aplazamientos للـ admisión بشكل حتمي وتصدير العدادين للقدرة/التشبع. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | La fluctuación del marcapasos y los tiempos de espera de la vista son de +/-125 por mil. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | يدفع cargas útiles RBC كبيرة حتى حدود للـ soft/hard للـ store ليظهر ارتفاع جلسات وعدادات bytes ثم تراجعها واستقرارها دون تجاوز store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | Se utilizan medidores de envío redundante y colectores en el objetivo, y se conectan de extremo a extremo. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | Hay trozos que forman parte de la acumulación de pedidos y fallos de las cargas útiles. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

ارفق اسطر JSON التي يطبعها Harness مع scrape الخاص بـ Prometheus الملتقط اثناء التشغيل كلما طلبت الحوكمة ادلة على ان انذارات contrapresión تطابق طوبولوجيا التدريب.## قائمة التحديث

1. Haga clic en el rastreo enrutado y en el enlace de seguimiento.
2. حدث جدول التخفيف بعد كل متابعة Alertmanager حتى لو كان الاجراء هو اغلاق التذكرة.
3. Utilice deltas de configuración, rastreador y resúmenes del paquete de telemetría y solicitud de extracción.
4. اربط هنا اي artefacto جديد للتدريب/التيليمتري كي تشير تحديثات roadmap المستقبلية الى مستند واحد بدلا من ملاحظات ad-hoc متناثرة.

## فهرس الادلة| الاصل | الموقع | ملاحظات |
|-------|----------|-------|
| تقرير تدقيق seguimiento enrutado (primer trimestre de 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | المصدر القانوني لادلة Fase B1؛ Esta es la versión `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Rastreador الخاص بـ config delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | Haga clic en TRACE-CONFIG-DELTA y en GOV-2026-03-19. |
| خطة remediación للتيليمتري | `docs/source/nexus_telemetry_remediation_plan.md` | Paquete de alertas, OTLP y barandillas de seguridad de B2. |
| Rastreador تدريب multicarril | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | يسرد artefactos تدريب 9 ابريل وmanifest/digest الخاص بالـ validador وملاحظات/agenda Q2 y rollback. |
| Manifiesto/resumen del paquete de telemetría (más reciente) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | Hay un rango de ranura 912-936, una semilla `NEXUS-REH-2026Q2` y artefactos hashes. |
| Manifiesto de perfil TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | hash ملف TLS المعتمد الملتقط خلال volver a ejecutar Q2؛ اشر اليه في ملاحق rastreo enrutado. |
| Agenda TRACE-MULTILANO-CANARIA | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | ملاحظات التخطيط لتدريب Q2 (النافذة, rango de ranuras, semilla de carga de trabajo, مالكو الاجراءات). |
| Runbook تدريب الاطلاق | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Lista de verificación تشغيلية لـ puesta en escena -> ejecución -> reversión؛ حدثها عند تغير طوبولوجيا carriles او ارشادات exportadores. |
| Validador de paquetes de telemetría | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI مشار اليه في retro B4؛ ارشف resume el rastreador de بجانب عند تغير الحزمة. || Regresión multicarril | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | Incluye `nexus.enabled = true` para configuraciones de varios carriles, y para hashes de catálogo de Sora y para Kura/merge-log para carril (`blocks/lane_{id:03}_{slug}`) y para `ConfigLaneRouter` نشر digiere الخاصة بالartefactos. |