---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: afinación del orquestador
título: إطلاق وضبط المُنسِّق
sidebar_label: ضبط المُنسِّق
descripción: قيم افتراضية عملية وإرشادات ضبط ونقاط تدقيق لمواءمة المُنسِّق متعدد المصادر مع GA.
---

:::nota المصدر المعتمد
Aquí `docs/source/sorafs/developer/orchestrator_tuning.md`. حافظ على تطابق النسختين إلى أن تُحال مجموعة التوثيق القديمة للتقاعد.
:::

# دليل إطلاق وضبط المُنسِّق

يبني هذا الدليل على [مرجع الإعداد](orchestrator-config.md) y
[دليل إطلاق متعدد المصادر](multi-source-rollout.md). يشرح
كيفية ضبط المُنسِّق لكل مرحلة إطلاق، وكيفية قراءة آثار لوحة النتائج، وما هي
إشارات التليمترية المطلوبة قبل توسيع الحركة. طبّق التوصيات بشكل متسق عبر
CLI y SDK están disponibles para su instalación y configuración.

## 1. مجموعات المعلمات الأساسية

ابدأ من قالب إعداد مشترك واضبط مجموعة صغيرة من المقابض مع تقدم الإطلاق. يلتقط
الجدول أدناه القيم الموصى بها لأكثر المراحل شيوعًا؛ وما لم يُذكر يعود إلى
Aquí están `OrchestratorConfig::default()` y `FetchOptions::default()`.| المرحلة | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | الملاحظات |
|---------|-----------------|-------------------------------|------------------------------------|-----------------------|------------------------------|-----------|
| **المختبر / CI** | `3` | `2` | `2` | `2500` | `300` | سقف كمون ضيق ونافذة سماح قصيرة يكشفان التليمترية المزعجة سريعًا. خفّض المحاولات لكشف المانيفستات غير الصالحة مبكرًا. |
| **Puesta en escena** | `4` | `3` | `3` | `4000` | `600` | يعكس قيم الإنتاج مع ترك فسحة للنظراء الاستكشافية. |
| **الكناري** | `6` | `3` | `3` | `5000` | `900` | يطابق القيم الافتراضية؛ Utilice `telemetry_region` para conectar el cable de alimentación a la fuente de alimentación. |
| **الإتاحة العامة (GA)** | `None` (استخدم جميع المؤهلين) | `4` | `4` | `5000` | `900` | ارفع حدود إعادة المحاولة والفشل لامتصاص الأعطال العابرة مع استمرار التدقيق في ضمان الحتمية. |

- يبقى `scoreboard.weight_scale` على القيمة الافتراضية `10_000` ما لم يتطلب نظام لاحق دقة عددية مختلفة. زيادة المقياس لا تغيّر ترتيب المزوّدين؛ بل تنتج توزيعًا أدق للرصيد.
- Utilice un archivo JSON y un archivo `--scoreboard-out` para obtener archivos adjuntos.

## 2. نظافة لوحة النتائجتجمع لوحة النتائج بين متطلبات المانيفست وإعلانات المزوّدين والتليمترية.
قبل المضي قدمًا:1. **تحقّق من حداثة التليمترية.** تأكد من أن اللقطات المشار إليها عبر
   `--telemetry-json` التقطت ضمن نافذة السماح. الإدخالات الأقدم من
   `telemetry_grace_secs` Utilice `TelemetryStale { last_updated }`.
   اعتبر ذلك توقفًا حتميًا وحدّث تصدير التليمترية قبل المتابعة.
2. **راجع أسباب الأهلية.** احفظ الآثار عبر
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. كل إدخال
   يحمل كتلة `eligibility` بالسبب الدقيق للفشل. لا تتجاوز عدم توافق القدرات أو
   الإعلانات المنتهية؛ أصلح الحمولة المصدرية.
3. **راجع تغيّر الأوزان.** قارن حقل `normalised_weight` بالإصدار السابق. تغيّر
   أكبر من 10% يجب أن يرتبط بتغييرات مقصودة في الإعلانات أو التليمترية ويُسجّل
   في سجل الإطلاق.
4. **أرشف الآثار.** اضبط `scoreboard.persist_path` ليصدر كل تشغيل لقطة لوحة النتائج
   النهائية. أرفق الأثر بسجل الإصدار مع المانيفست وحزمة التليمترية.
5. **وثّق دليل خليط المزوّدين.** يجب أن تعرض ميتاداتا `scoreboard.json` و`summary.json`
   Cables de conexión `provider_count`, `gateway_provider_count` y `provider_mix`
   Aquí está el `direct-only`, el `gateway-only` y el `mixed`. يجب أن تُظهر لقطات
   البوابة `provider_count=0` و`provider_mix="gateway-only"`, بينما تتطلب التشغيلات المختلطة
   أعدادًا غير صفرية للمصدرين. يفرض `cargo xtask sorafs-adoption-check` هذه الحقول
   (ويفشل إذا لم تتطابق العدادات/التسميات)، لذا شغّله دائمًا مع
   `ci/check_sorafs_orchestrator_adoption.sh` أو نص الالتقاط لديك لإنتاج حزمة الأدلة
   `adoption_report.json`. Para obtener más información, consulte Torii y `gateway_manifest_id`/.`gateway_manifest_cid` ضمن ميتاداتا لوحة النتائج حتى يتمكن حاجز التبني من
   ربط مغلف المانيفست بخليط المزوّدين الملتقط.

للتعريفات التفصيلية للحقول راجع
`crates/sorafs_car/src/scoreboard.rs` Interfaz de usuario CLI
`sorafs_cli fetch --json-out`.

## مرجع أعلام CLI y SDK

`sorafs_cli fetch` (راجع `crates/sorafs_car/src/bin/sorafs_cli.rs`) y واجهة
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) تشترك في
سطح إعداد المُنسِّق نفسه. استخدم الأعلام التالية عند التقاط أدلة الإطلاق أو
إعادة تشغيل الـ accesorios القياسية:

مرجع أعلام متعدد المصادر (أبقِ مساعدة CLI والوثائق متزامنة بتعديل هذا الملف فقط):- `--max-peers=<count>` يحدّ عدد المزوّدين المؤهلين الذين ينجون من فلتر لوحة النتائج. اتركه فارغًا لتدفق جميع المزوّدين المؤهلين، واضبطه على `1` فقط عند اختبار الرجوع لمصدر واحد عمدًا. Utilice `maxPeers` para SDK (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` يمرّر حد إعادة المحاولة لكل chunk الذي يطبقه `FetchOptions`. استخدم جدول الإطلاق في دليل الضبط للقيم الموصى بها؛ Para ello, utilice la CLI y acceda al SDK.
- `--telemetry-region=<label>` يوسم سلاسل Prometheus `sorafs_orchestrator_*` (ومرحّلات OTLP) بوسم المنطقة/البيئة حتى تمكن لوحات المتابعة من فصل حركة المختبر و puesta en escena y والكناري وGA.
- `--telemetry-json=<path>` يحقن لقطة التليمترية المشار إليها في لوحة النتائج. La configuración JSON es compatible con el software `cargo xtask sorafs-adoption-check --require-telemetry` y el código OTLP. الالتقاط).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) يفعّل ganchos الخاصة بالجسر. عند ضبطها، يمرّر المُنسِّق الـ trozos عبر Proxy Norito/Kaigi المحلي حتى تتلقى عملاء المتصفح وguard caches y Kaigi نفس الإيصالات التي يصدرها Rust.
- `--scoreboard-out=<path>` (اختياريًا مع `--scoreboard-now=<unix_secs>`) يحفظ لقطة الأهلية للمدققين. Utilice el formato JSON para crear archivos y archivos adjuntos.- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` يطبّقان تعديلات حتمية فوق ميتاداتا الإعلانات. استخدم هذه الأعلام للتجارب فقط؛ يجب أن تمر تخفيضات الإنتاج عبر artefactos الحوكمة كي تطبّق كل عقدة نفس حزمة السياسة.
- `--provider-metrics-out` / `--chunk-receipts-out` أرفق كلا الأثرين عند تقديم أدلة التبني.

مثال (باستخدام الـ accesorio المنشور):

```bash
sorafs_cli fetch \
  --plan fixtures/sorafs_orchestrator/multi_peer_parity_v1/plan.json \
  --gateway-provider gw-alpha=... \
  --telemetry-source-label otlp::staging \
  --scoreboard-out artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --json-out artifacts/sorafs_orchestrator/latest/summary.json \
  --provider-metrics-out artifacts/sorafs_orchestrator/latest/provider_metrics.json \
  --chunk-receipts-out artifacts/sorafs_orchestrator/latest/chunk_receipts.json

cargo xtask sorafs-adoption-check \
  --scoreboard artifacts/sorafs_orchestrator/latest/scoreboard.json \
  --summary artifacts/sorafs_orchestrator/latest/summary.json
```

Los SDK están disponibles en `SorafsGatewayFetchOptions` en Rust
(`crates/iroha/src/client.rs`) y JS
(`javascript/iroha_js/src/sorafs.js`) y SDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). حافظ على تطابق هذه
La CLI de la configuración de la interfaz de usuario de la CLI está conectada a la interfaz de usuario
طبقات ترجمة مخصصة.

## 3. ضبط سياسة الجلب

يتحكم `FetchOptions` في إعادة المحاولة والتوازي والتحقق. عند الضبط:- **إعادة المحاولة:** رفع `per_chunk_retry_limit` فوق `4` يزيد زمن الاستعادة لكنه
  قد يخفي أعطال المزوّدين. يفضّل إبقاء السقف عند `4` والاعتماد على تدوير المزوّدين
  لإظهار الأداء الضعيف.
- **عتبة الفشل:** تحدد `provider_failure_threshold` متى يتم تعطيل المزوّد لبقية الجلسة.
  يجب أن تتوافق هذه القيمة مع سياسة إعادة المحاولة: عتبة أقل من ميزانية المحاولة
  تُخرج المزوّد قبل استنفاد جميع المحاولات.
- **التوازي:** اترك `global_parallel_limit` غير مضبوط (`None`) ما لم تكن بيئة محدة
  غير قادرة على تشبع النطاقات المُعلنة. عند ضبطه، تأكد أن القيمة ≤ مجموع ميزانيات
  التدفق للمزوّدين لتجنب التجويع.
- **مفاتيح التحقق:** يجب إبقاء `verify_lengths` و`verify_digests` مفعّلين في الإنتاج.
  فهي تضمن الحتمية عندما تعمل أساطيل مزوّدين مختلطة؛ عطّلها فقط في بيئات fuzzing المعزولة.

## 4. مراحل النقل والخصوصية

Utilice los siguientes valores `rollout_phase`, `anonymity_policy` y `transport_policy`:- Utilice `rollout_phase="snnet-5"` y conecte el dispositivo SNNet-5. استعمل
  `anonymity_policy_override` فقط عندما تصدر الحوكمة توجيهًا موقّعًا.
- أبقِ `transport_policy="soranet-first"` كخيار أساس بينما SNNet-4/5/5a/5b/6a/7/8/12/13 في 🈺
  (راجع `roadmap.md`). استخدم `transport_policy="direct-only"` فقط لخفض موثق أو تدريبات امتثال،
  وانتظر مراجعة تغطية PQ قبل الترقية إلى `transport_policy="soranet-strict"`—هذا المستوى يفشل سريعًا
  إذا بقيت ترحيلات كلاسيكية فقط.
- Para obtener `write_mode="pq-only"`, se requiere una actualización de software (SDK, المُنسِّق، أدوات الحوكمة)
  تلبية متطلبات PQ. أثناء الإطلاق، أبقِ `write_mode="allow-downgrade"` حتى يمكن لخطط الطوارئ
  الاعتماد على المسارات المباشرة بينما تُعلِّم التليمترية حالات التخفيض.
- يعتمد اختيار الحراس وتجهيز الدوائر على دليل SoraNet. زوّد لقطة `relay_directory` الموقعة
  Y la caché `guard_set` está disponible para su uso. بصمة الكاش
  التي يسجلها `sorafs_cli fetch` تُعد جزءًا من أدلة الإطلاق.

## 5. ganchos التخفيض والامتثال

يساعد نظامان فرعيان في المُنسِّق على فرض السياسة دون تدخل يدوي:- **معالجة التخفيض** (`downgrade_remediation`): تراقب أحداث `handshake_downgrade_total`, وبعد تجاوز
  `threshold` incluye `window_secs` y el proxy es `target_mode` (solo metadatos).
  احتفظ بالقيم الافتراضية (`threshold=3`, `window=300`, `cooldown=900`) ما لم تظهر مراجعات الحوادث
  نمطًا مختلفًا. وثّق أي anular في سجل الإطلاق وتأكد أن لوحات المتابعة تراقب
  `sorafs_proxy_downgrade_state`.
- **سياسة الامتثال** (`compliance`): تمر استثناءات الولاية القضائية والمانيفست عبر قوائم opt-out التي
  تديرها الحوكمة. لا تُدرِج anula عشوائية في حزمة الإعداد؛ اطلب تحديثًا موقعًا
  Aquí `governance/compliance/soranet_opt_outs.json` y el código JSON.

بالنسبة لكلا النظامين، احفظ حزمة الإعداد الناتجة وأرفقها بأدلة الإصدار حتى يتمكن
المدققون من تتبع كيفية تفعيل التخفيضات.

## 6. التليمترية ولوحات المتابعة

قبل توسيع الإطلاق، تأكد أن الإشارات التالية نشطة في البيئة المستهدفة:

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  يجب أن تكون صفرًا بعد اكتمال الكناري.
- `sorafs_orchestrator_retries_total` y
  `sorafs_orchestrator_retry_ratio` — يجب أن تستقر تحت 10% أثناء الكناري وتبقى
  5% de GA.
- `sorafs_orchestrator_policy_events_total` — يتحقق من أن مرحلة الإطلاق المطلوبة
  فعالة (etiqueta `stage`) y حالات apagón عبر `outcome`.
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — تتبع عرض ترحيلات PQ مقابل توقعات السياسة.
- أهداف سجلات `telemetry::sorafs.fetch.*` — يجب بثها إلى مجمّع السجلات المشترك مع
  Utilice el dispositivo `status=failed`.حمّل لوحة Grafana القياسية من
`dashboards/grafana/sorafs_fetch_observability.json` (المصدَّرة في البوابة تحت
**SoraFS → Obtener observabilidad**) لضمان تطابق محددات المنطقة/المانيفست وخريطة
Los equipos y las instalaciones de SRE están quemados.
اربط قواعد Alertmanager في `dashboards/alerts/sorafs_fetch_rules.yml` y تحقق من
صياغة Prometheus عبر `scripts/telemetry/test_sorafs_fetch_alerts.sh` (يشغّل المساعد
`promtool test rules` محليًا أو في Docker). تتطلب عمليات تسليم التنبيه نفس كتلة
التوجيه التي يطبعها السكربت حتى يتمكن المشغلون من إرفاق الأدلة بتذكرة الإطلاق.

### تدفق quemado للتليمترية

يتطلب بند خارطة الطريق **SF-6e** فترة burn-in للتليمترية لمدة 30 يومًا قبل تحويل
المُنسِّق متعدد المصادر إلى افتراضاته GA. استخدم سكربتات المستودع لالتقاط حزمة
آثار قابلة لإعادة الإنتاج لكل يوم في النافذة:

1. Haga clic en `ci/check_sorafs_orchestrator_adoption.sh` para evitar que se queme. Nombre:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```يعيد المساعد تشغيل `fixtures/sorafs_orchestrator/multi_peer_parity_v1`, ويكتب
   `scoreboard.json` y `summary.json` y `provider_metrics.json` y `chunk_receipts.json`
   و`adoption_report.json` تحت `artifacts/sorafs_orchestrator/<timestamp>/`،
   Esto se debe a que el dispositivo está conectado a `cargo xtask sorafs-adoption-check`.
2. عند توفر متغيرات burn-in, يصدر السكربت أيضًا `burn_in_note.json` لتوثيق الوسم،
   وفهرس اليوم، ومعرّف المانيفست، ومصدر التليمترية، وملخصات الآثار. Archivo JSON
   La limpieza del hogar se realizará durante 30 días.
3. استورد لوحة Grafana المحدثة (`dashboards/grafana/sorafs_fetch_observability.json`)
   في مساحة puesta en escena/producción, وضع وسم burn-in, وتأكد من أن كل لوحة تعرض عينات
   للمانيفست/المنطقة قيد الاختبار.
4. شغّل `scripts/telemetry/test_sorafs_fetch_alerts.sh` (أو `promtool test rules …`)
   عند تغيير `dashboards/alerts/sorafs_fetch_rules.yml` لتوثيق أن توجيه التنبيهات
   يطابق المقاييس المصدرة أثناء quemado.
5. أرشف لقطة لوحة المتابعة، ومخرجات اختبار التنبيهات، وذيل السجلات لعمليات البحث
   `telemetry::sorafs.fetch.*` Para obtener más información, consulte la página de inicio.
   الأدلة دون الاعتماد على أنظمة حية.

## 7. قائمة تحقق الإطلاق1. أعد توليد marcadores في CI باستخدام الإعداد المرشح والتقط الآثار تحت التحكم بالإصدارات.
2. شغّل جلب accesorios الحتمي في كل بيئة (المختبر، puesta en escena, الكناري، الإنتاج) وأرفق
   آثار `--scoreboard-out` و`--json-out` بسجل الإطلاق.
3. راجع لوحات التليمترية مع مهندس المناوبة, وتأكد أن كل المقاييس أعلاه لديها عينات حية.
4. سجّل مسار الإعداد النهائي (عادة عبر `iroha_config`) y confirme git لسجل الحوكمة المستخدم
   للإعلانات والامتثال.
5. حدّث متعقّب الإطلاق وأبلغ فرق SDK بالافتراضات الجديدة حتى تبقى تكاملات العملاء متسقة.

اتباع هذا الدليل يحافظ على إطلاقات المُنسِّق حتمية وقابلة للتدقيق، مع توفير حلقات
تغذية راجعة واضحة لضبط ميزانيات إعادة المحاولة, وسعة المزوّدين، ووضع الخصوصية.