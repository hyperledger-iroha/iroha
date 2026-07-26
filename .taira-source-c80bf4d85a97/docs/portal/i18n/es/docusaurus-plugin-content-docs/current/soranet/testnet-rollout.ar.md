---
lang: es
direction: ltr
source: docs/portal/docs/soranet/testnet-rollout.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: lanzamiento de testnet
título: Testnet de SoraNet (SNNet-10)
sidebar_label: Testnet (SNNet-10)
descripción: kit de instalación de telemetría y testnet en SoraNet.
---

:::nota المصدر القياسي
Utilice el SNNet-10 para `docs/source/soranet/testnet_rollout_plan.md`. حافظ على النسختين متطابقتين حتى يتم تقاعد الوثائق القديمة.
:::

SNNet-10 está conectado a SoraNet. Entregables, runbooks, telemetría y operador. SoraNet النقل الافتراضي.

## مراحل الاطلاق| المرحلة | الجدول الزمني (المستهدف) | النطاق | القطع المطلوبة |
|-------|-------------------|-------|--------------------|
| **T0 - Prueba de red** | Cuarto trimestre de 2026 | 20-50 relés عبر >=3 ASN يديرها مساهمون اساسيون. | Kit de incorporación de Testnet, conjunto de humo para fijación de guardia, línea de base + métricas PoW, y simulacro de apagón. |
| **T1 - بيتا عامة** | Primer trimestre de 2027 | >=100 relés, rotación de guardia, unión de salida y versiones beta del SDK de SoraNet con `anon-guard-pq`. | Kit de incorporación, lista de verificación, directorio de SOP, paneles de control, telemetría, y ensayo. |
| **T2 - Red principal افتراضي** | Q2 2027 (مشروط باكتمال SNNet-6/7/9) | شبكة الانتاج تعتمد SoraNet افتراضيا؛ تفعيل transports من نوع obfs/MASQUE y PQ ratchet. | محاضر موافقة gobernancia, اجراء rollback لنمط direct-only, انذارات downgrade, وتقرير نجاح موقع. |

لا يوجد **مسار تجاوز** - يجب ان تشحن كل مرحلة telemetría y gobernanza من المرحلة السابقة قبل الترقية.

## Kit de prueba de red

كل operador de relé يتلقى حزمة حتمية تحتوي على الملفات التالية:| القطعة | الوصف |
|----------|-------------|
| `01-readme.md` | نظرة عامة، نقاط تواصل، وجدول زمني. |
| `02-checklist.md` | lista de verificación قبل الاطلاق (hardware، امكانية الوصول للشبكة، تحقق من política de protección). |
| `03-config-example.toml` | Este relé + orquestador es compatible con SoraNet para cumplir con el cumplimiento de SNNet-9 y con `guard_directory` para obtener una instantánea de protección de hash. |
| `04-telemetry.md` | تعليمات لربط paneles de control لمقاييس الخصوصية في SoraNet y التنبيه. |
| `05-incident-playbook.md` | اجراء الاستجابة لحالات brownout/downgrade مع مصفوفة تصعيد. |
| `06-verification-report.md` | قالب يكمله المشغلون ويعيدونه بعد نجاح pruebas de humo. |

Utilice el dispositivo `docs/examples/soranet_testnet_operator_kit/`. كل ترقية تحدث kit؛ ارقام الاصدار تتبع المرحلة (مثلا `testnet-kit-vT0.1`).

بالنسبة لمشغلي beta العامة (T1), breve مختصر في `docs/source/soranet/snnet10_beta_onboarding.md` يلخص المتطلبات y مخرجات telemetría y عمل التسليم مع الاشارة الى kit الحتمي وhelpers التحقق.

`cargo xtask soranet-testnet-feed` Alimentación JSON Relés, taladros y hashes puerta del escenario. وقع سجلات taladros والمرفقات اولا باستخدام `cargo xtask soranet-testnet-drill-bundle` لكي يسجل feed ان `drill_log.signed = true`.

## مقاييس النجاح

الترقية بين المراحل مشروطة بالـ telemetría التالية, y تجمع لمدة لا تقل عن اسبوعين:- `soranet_privacy_circuit_events_total`: 95% de los circuitos sufren caídas de tensión y degradación 5% de descuento en el pago del PQ.
- `sorafs_orchestrator_policy_events_total{outcome="brownout"}`: اقل من 1% من جلسات buscar يوميا تطلق apagón خارج taladros المخطط لها.
- `soranet_privacy_gar_reports_total`: تذبذب ضمن +/-10% من مزيج فئات GAR المتوقع؛ الارتفاعات يجب تفسيرها بتحديثات política معتمدة.
- معدل نجاح تذاكر PoW: >=99% ضمن نافذة 3 ثواني؛ يتم الابلاغ عبر `soranet_privacy_throttles_total{scope="congestion"}`.
- Tiempo (percentil 95) Tiempo de funcionamiento: <200 ms بعد اكتمال circuitos بالكامل، ملتقط عبر `soranet_privacy_rtt_millis{percentile="p95"}`.

قوالب tableros de instrumentos y موجودة في `dashboard_templates/` y `alert_templates/`; انسخها الى مستودع telemetría y الى فحوصات pelusa في CI. Utilice `cargo xtask soranet-testnet-metrics` para que el dispositivo funcione correctamente.

El stage-gate está disponible en `docs/source/soranet/snnet10_stage_gate_template.md` y en Markdown para `docs/examples/soranet_testnet_stage_gate/stage_gate_report_template.md`.

## Lista de verificación

يجب على المشغلين التوقيع على ما يلي قبل دخول كل مرحلة:

- ✅ Anuncio de retransmisión موقع باستخدام sobre de admisión الحالي.
- ✅ Prueba de humo de rotación de guardia (`tools/soranet-relay --check-rotation`) ناجح.
- ✅ `guard_directory` يشير الى احدث artefacto من `GuardDirectorySnapshotV2` y `expected_directory_hash_hex` يطابق digest اللجنة (اقلاع relé يسجل hash الذي تم التحقق منه).
- ✅ مقاييس PQ ratchet (`sorafs_orchestrator_pq_ratio`) تبقى فوق حدود الهدف للمرحلة المطلوبة.
- ✅ Cumplimiento de la etiqueta الخاصة بـ GAR تطابق اخر (راجع كتالوج SNNet-9).
- ✅ محاكاة انذار degradación (تعطيل coleccionistas وتوقع تنبيه خلال 5 دقائق).
- ✅ تنفيذ Drill لـ PoW/DoS مع خطوات تخفيف موثقة.يوجد قالب معبأ مسبقا ضمن kit الانضمام. يرسل المشغلون التقرير المكتمل الى مكتب مساعدة gobernancia قبل استلام بيانات الانتاج.

## Gobernanza y

- **ضبط التغيير:** الترقيات تتطلب موافقة Consejo de Gobernanza مسجلة في محاضر المجلس ومرفقة بصفحة الحالة.
- **ملخص الحالة:** نشر تحديثات اسبوعية تلخص عدد relevadores ونسبة PQ وحوادث apagón والعناصر المفتوحة (تخزن في `docs/source/status/soranet_testnet_digest.md` بعد بدء الوتيرة).
- **Reversiones:** Haga una reversión de 30 días para obtener DNS/guard cache y una copia de seguridad de DNS/guard cache. تواصل العملاء.

## اصول داعمة

- `cargo xtask soranet-testnet-kit [--out <dir>]` Kit de montaje de `xtask/templates/soranet_testnet/` para el hogar (الافتراضي `docs/examples/soranet_testnet_operator_kit/`).
- `cargo xtask soranet-testnet-metrics --input <metrics.json> [--out <path|->]` يقيم مقاييس نجاح SNNet-10 ويصدر تقرير pasa/falla منظم مناسب لمراجعات gobernanza. Utilice el software `docs/examples/soranet_testnet_metrics_sample.json`.
- قوالب Grafana و Alertmanager موجودة تحت `dashboard_templates/soranet_testnet_overview.json` و `alert_templates/soranet_testnet_rules.yml`; Utilice telemetría y utilice pelusa en CI.
- قالب تواصل downgrade del SDK/portal يوجد في `docs/source/soranet/templates/downgrade_communication_template.md`.
- يجب ان تستخدم ملخصات الحالة الاسبوعية `docs/source/status/soranet_testnet_weekly_digest.md` كنموذج قياسي.

Hay solicitudes de extracción que incluyen solicitudes de extracción y telemetría y planes de telemetría.