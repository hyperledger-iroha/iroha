---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-tuning.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: ضبط الأوركسترا
العنوان: Despliegue y ajuste del orquestador
Sidebar_label: ضبط الطلب
الوصف: قيم عملية محددة مسبقًا، ودليل ضبط ونقاط سمعية لجلب الأوركسترا متعدد الأصول إلى GA.
---

:::ملاحظة فوينتي كانونيكا
ريفليجا `docs/source/sorafs/developer/orchestrator_tuning.md`. احتفظ بنسخ منفصلة حتى يتم سحب مجموعة المستندات المتوارثة.
:::

# دليل التنقل وضبط الأوركيستادور

هذا هو الأساس في [مرجع التكوين](orchestrator-config.md) وال
[دليل التشغيل متعدد الأصول](multi-source-rollout.md). شرح
كيف تقوم بضبط الأوركيستادور لكل عملية تشغيل، كما تترجمها
مصنوعات لوحة النتائج وما هي قوائم القياس عن بعد التي يجب أن تكون موجودة مسبقًا
لتوسيع حركة المرور. قم بتطبيق التوصيات بطريقة متسقة
CLI وSDK والأتمتة حتى تتمكن كل نقطة من متابعة نفس السياسة
جلب الحتمية.

## 1. قاعدة تكوينات المعلمات

جزء من مجموعة تكوين مقسمة وضبط مجموعة صغيرة
مخاطر التقدم في التقدم. اللوحة تستقر في مكانها
القيم الموصى بها للطرق الأكثر شيوعًا؛ القيم لم يتم عرضها في القائمة
إلى المحددات المسبقة في `OrchestratorConfig::default()` و`FetchOptions::default()`.| فاس | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | نوتاس |
|------|-----------------|------------------------------------------------|-------------|----------------------------------------------------------|------||------|
| **مختبر/CI** | `3` | `2` | `2` | `2500` | `300` | يعمل زمن الوصول المحدود ونافذة السماح المحدودة على تقليل وقت القياس عن بعد بسرعة. احتفظ بالاحتفاظ بالتخفيضات لاكتشاف البيانات غير الصالحة مسبقًا. |
| ** التدريج ** | `4` | `3` | `3` | `4000` | `600` | تعكس قيم الإنتاج على هامش الهامش للأقران المستكشفين. |
| **كناري** | `6` | `3` | `3` | `5000` | `900` | تساوي قيم العيب; قم بتكوين `telemetry_region` حتى تتمكن لوحات المعلومات من تقسيم حركة المرور المستمرة. |
| **العجز العام** | `None` (استخدام جميع العناصر المؤهلة) | `4` | `4` | `5000` | `900` | قم بزيادة المظلات والسقوط لامتصاص السقطات الانتقالية أثناء قيام المستمعين بإعادة تشكيل الحتمية. |- `scoreboard.weight_scale` يتم الحفاظ على القيمة المحددة مسبقًا `10_000` مما يدفع النظام المتدفق إلى اتخاذ قرار آخر. زيادة المستوى لا تغير ترتيب الموردين؛ قم فقط بإصدار توزيع قروض أكثر كثافة.
- للترحيل بين المراحل، استمر في حزمة JSON واستخدم `--scoreboard-out` حتى تقوم قائمة الاستماع بتسجيل مجموعة المعلمات الدقيقة.

## 2. نظافة لوحة النتائج

تجمع لوحة النتائج بين متطلبات البيانات وإعلانات الموردين والقياس عن بعد.
ما قبل التقدم:1. **التحقق من دقة القياس عن بعد.** تأكد من أن اللقطات تشير إلى ذلك
   `--telemetry-json` يتم التقاطه داخل نافذة الرحمة التي تم تكوينها. لاس إنترداس
   المزيد من `telemetry_grace_secs` يقع مع `TelemetryStale { last_updated }`.
   استخدم كحظر متين وقم بتحديث تصدير القياس عن بعد قبل الاستمرار.
2. **التحقق من دوافع الأهلية.** استمر في استخدام المصنوعات اليدوية
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. كل مدخل
   تتضمن كتلة `eligibility` مع سبب السقوط الدقيق. لا يوجد سوبريسكريباس
   إلغاء السعة أو الإعلان عن انتهاء الصلاحية؛ تصحيح الحمولة المنبع.
3. **مراجعة تحويلات البيزو.** مقارنة المجال `normalised_weight` مع
   الافراج عن السابق. يجب أن يرتبط توزيع البيزو >10% بالتغييرات
   تمت مداولاته في إعلان عن القياس عن بعد والتسجيل في سجل النشر.
4. **أرشفة المصنوعات اليدوية.** تكوين `scoreboard.persist_path` لكل هذا
   يتم تنفيذ اللقطة الأخيرة من لوحة النتائج. أضف القطعة الأثرية إلى السجل
   يتم إصداره جنبًا إلى جنب مع البيان وحزمة القياس عن بعد.
5. **تسجيل الأدلة في مجموعة الموردين.** البيانات التعريفية لـ `scoreboard.json`
   y el `summary.json` المطابق لـ deben exponer `provider_count`,
   `gateway_provider_count` والعلامة المشتقة `provider_mix` للمراجعةتأكد من أن التنفيذ سيكون `direct-only` أو `gateway-only` أو `mixed`. لاس كابتورا دي
   تقرير البوابة `provider_count=0` و`provider_mix="gateway-only"`، بينما يحدث ذلك
   تتطلب عمليات تنفيذ المزيج محتوى غير مباشر لجميع الأصول. `cargo xtask sorafs-adoption-check`
   تطبيق هذه المجالات (وإذا لم تكن المحتويات/الملصقات متزامنة)، فقم بإخراجها
   قم دائمًا بالاشتراك مع `ci/check_sorafs_orchestrator_adoption.sh` أو برنامج الالتقاط الخاص بك
   أنتج حزمة الأدلة `adoption_report.json`. بوابات كواندو هيا Torii,
   حفظ `gateway_manifest_id`/`gateway_manifest_cid` في البيانات الوصفية للوحة النتائج
   حتى تتمكن بوابة التبني من ربط اسم البيان بالبيان
   mezcla de proveedores capturada.

للحصول على تعريفات تفصيلية للمجالات، راجع
`crates/sorafs_car/src/scoreboard.rs` وبنية استئناف CLI المعروضة
`sorafs_cli fetch --json-out`.

## مرجع علامات CLI وSDK

`sorafs_cli fetch` (الإصدار `crates/sorafs_car/src/bin/sorafs_cli.rs`) والمجمّع
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) شارك
نفس سطح تكوين orquestador. أعلام الولايات المتحدة الأمريكية لوس siguientes آل
التقاط الأدلة من التحلل أو إعادة إنتاج التركيبات التقليدية:

مرجع مقسم للأعلام متعددة الأصول (يحافظ على مساعدة CLI y los docs en
تحرير متزامن لهذا الأرشيف فقط):- `--max-peers=<count>` هو الحد الأقصى لعدد الموردين المؤهلين من خلال تصفية لوحة النتائج. لا يتم تكوينه للإرسال من جميع الموردين المؤهلين ويتم تشغيله على `1` فقط عندما يتم سحبه عمدًا من مصدر واحد فقط. قم بإعادة تشغيل `maxPeers` في SDK (`SorafsGatewayFetchOptions.maxPeers`، `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` يعيد الاتصال بالحد الأقصى للجزء الذي يتم تطبيقه `FetchOptions`. استخدم لوحة التشغيل في دليل ضبط القيم الموصى بها؛ يجب أن تتزامن عمليات تنفيذ CLI التي تسترجع الأدلة مع القيم التي تعيب SDK للحفاظ على الجدار.
- `--telemetry-region=<label>` علامة السلسلة Prometheus `sorafs_orchestrator_*` (ومقاطع OTLP) مع علامة المنطقة/المنشأ لفصل لوحات المعلومات عن حركة المرور المعملية والتدريج والكناري وGA.
- `--telemetry-json=<path>` يقوم بإدخال اللقطة المرجعية من خلال لوحة النتائج. استمر في استخدام JSON بجوار لوحة النتائج حتى يتمكن المدققون من إعادة إنتاج الإخراج (ولكي يتمكن `cargo xtask sorafs-adoption-check --require-telemetry` من التحقق من بث OTLP الذي يتم التقاطه).
- `--local-proxy-*` (`--local-proxy-mode`، `--local-proxy-norito-spool`، `--local-proxy-kaigi-spool`، `--local-proxy-kaigi-policy`) تأهيل خطافات جسر المراقبة. عند التهيئة، يقوم المُنسق بإرسال قطع عبر الوكيل المحلي Norito/Kaigi حتى يتمكن عملاء المتصفح، ويحرسون ذاكرات التخزين المؤقت والسجلات، من تلقي Kaigi نفس المبالغ الصادرة من Rust.- `--scoreboard-out=<path>` (اختياريًا مع `--scoreboard-now=<unix_secs>`) يحافظ على لقطة الأهلية للمدققين. ستستمر تجربة JSON دائمًا مع عناصر القياس عن بعد والبيانات المرجعية في تذكرة الإصدار.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` يضبط التطبيق المحددات المتعلقة بالبيانات الوصفية للإعلانات. أعلام الولايات المتحدة الأمريكية هي منفردة لـ ensayos; يجب أن تمر الانحطاطات في الإنتاج بمصنوعات الحكم حتى يتم تطبيق نفس الحزمة السياسية في كل مرة.
- `--provider-metrics-out` / `--chunk-receipts-out` يحافظ على مقاييس الصحة من قبل المورّد وتلقيات القطع المرجعية من خلال قائمة التحقق من الطرح؛ إضافة إلى المزيد من المصنوعات اليدوية لتقديم أدلة التبني.

مثال (استخدام المباراة المنشورة):

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

تستهلك مجموعة SDK التكوين نفسه في منتصف `SorafsGatewayFetchOptions` en
العميل Rust (`crates/iroha/src/client.rs`)، الارتباطات JS
(`javascript/iroha_js/src/sorafs.js`) وSDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Manté esas ayudas en
التزامن مع القيم بسبب خلل في CLI حتى يتمكن المشغلون
نسخ السياسة بين الأتمتة دون القدرة على التجارة عبر الوسط.

## 3. ضبط سياسة الجلب

`FetchOptions` يتحكم في سلوكيات الاحتفاظ والتزامن والتحقق.
الاجستار:- **التحديثات:** رفع `per_chunk_retry_limit` بواسطة `4` لزيادة الوقت
  يمكن للتعافي أن يسقط خفيًا من الموردين. تفضل بالصيانة `4`
  كتقنية والثقة في دورة الموردين لاكتشاف الأداء المنخفض.
- **ظلال السقوط:** `provider_failure_threshold` تحدد عند المستورد
  سيتم تعطيله لبقية الجلسة. إنها شجاعة في التعامل مع السياسة
  إعادة التدوير: شيء غامض يُلزم الأوركيستادور بشرط إعادة التدوير
  طرد أحد الأقران قبل أن يتفاوض مع الجميع.
- **التزامن:** Deja `global_parallel_limit` بدون التكوين (`None`) أقل
  لا يمكن لمجموعة محددة أن تشبع النطاقات المعلنة. عندما يكون الأمر كذلك
  قم بتكوين وتأكد من أن القيمة البحرية تساوي مجموع افتراضات
  تيارات الموردين لتجنب الإفلاس.
- **تبديل التحقق:** `verify_lengths` و `verify_digests` دائم الاستخدام
  تأهيل في الإنتاج. ضمان الحتمية عند وجود مزيج من الأسطول
  دي بروفيدورس؛ يتم إلغاء تنشيطه فقط أثناء تداخل التشويش.

## 4. عملية النقل المجهولة

الولايات المتحدة الأمريكية الحرم الجامعي `rollout_phase` و`anonymity_policy` و`transport_policy` الفقرة
ممثل وضع الخصوصية:- اختر `rollout_phase="snnet-5"` واسمح بسياسة مجهولة المصدر
  خلل يتتبع ضربات SNNet-5. Sobrescribe مع `anonymity_policy_override`
  فقط عندما تصدر الإدارة توجيهًا ثابتًا.
- حامل `transport_policy="soranet-first"` كقاعدة أساسية SNNet-4/5/5a/5b/6a/7/8/12/13 🈺
  (الإصدار `roadmap.md`). الولايات المتحدة الأمريكية `transport_policy="direct-only"` منفردة للتحلل
  الوثائق أو محاكاة الولاء، وانتظر مراجعة التغطية PQ
  قبل الترقية `transport_policy="soranet-strict"` — هذا المستوى يتراجع بسرعة
  سولو كويدان يبث الكلاسيكو.
- `write_mode="pq-only"` يجب فقط التحفيز عند كل جولة من الكتابة (SDK،
  orquestador, أدوات الإدارة) يمكن أن يرضي متطلبات PQ. دورانتي
  تستمر عمليات الإطلاق في `write_mode="allow-downgrade"` للاستجابة
  يمكن لحالات الطوارئ الوقوف على الطريق المباشر أثناء علامة القياس عن بعد
  تدهور.
- يعتمد اختيار الحراس وإعداد الدوائر على الدليل
  دي سورانت. قم بنقل اللقطة الثابتة إلى `relay_directory` واستمر في ذلك
  ذاكرة التخزين المؤقت `guard_set` للحفاظ على عملية التدوير داخل الجهاز
  نافذة الاحتفاظ بالكهرباء. تم تسجيل ذاكرة التخزين المؤقت من قبل
  `sorafs_cli fetch` جزء من دليل الطرح.

## 5. عبارات الانحطاط والوفاء

تساعد أنظمة السائق على تحفيز السياسة دون دليل التدخل:- **معالجة التدهور** (`downgrade_remediation`): مراقبة الأحداث
  `handshake_downgrade_total` وبعد أن يتم تكوين `threshold`
  يتم تجاوزه داخل `window_secs`، ويتم تشغيل الوكيل المحلي على `target_mode` (بواسطة
  البيانات الوصفية المعيبة فقط). الحفاظ على القيم المحددة مسبقًا (`threshold=3`،
  `window=300`، `cooldown=900`) يوجهون رسالة بعد الوفاة إلى نقش
  مميز. قم بتوثيق أي تجاوز في سجل البدء والتأكد من ذلك
  لوحات المعلومات سيجان `sorafs_proxy_downgrade_state`.
- **سياسة الوفاء** (`compliance`): المنحوتات من قبل الاختصاص القضائي
  البيان يتدفق من خلال قوائم الاستبعاد الإدارية من أجل الإدارة.
  لم يتم إدراج تجاوزات خاصة في حزمة التكوين؛ أون سو لوغار,
  التماس تحديث ثابت
  `governance/compliance/soranet_opt_outs.json` وقم بإيقاف إنشاء JSON.

لجميع الأنظمة، استمر في حزمة التكوين الناتجة والمتضمنة
في الأدلة الصادرة حتى يتمكن المدققون من التنقيب عن أنفسهم
تنشيط التخفيضات.

## 6. القياس عن بعد ولوحات المعلومات

قبل توسيع التوسيع، تأكد من أن الإشارات التالية نشطة
في الهدف الداخلي:- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  سيكون الأمر كذلك بعد إكمال الكناري.
- `sorafs_orchestrator_retries_total` ذ
  `sorafs_orchestrator_retry_ratio` — يتم التثبيت بنسبة 10% تقريبًا
  خلال الكناري واستمر في دفع 5٪ من GA.
- `sorafs_orchestrator_policy_events_total` — التحقق من بدء التشغيل
  من المتوقع أن يكون نشطًا (التسمية `stage`) وقم بتسجيل التخفيضات عبر `outcome`.
-`sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — مسح ملخص البث PQ
  أمام التوقعات السياسية.
- كائنات السجل `telemetry::sorafs.fetch.*` — عمل مجمع السجلات
  تمت مشاركتها مع البحث المحمي لـ `status=failed`.

قم بتحميل لوحة القيادة الكنسي من Grafana من
`dashboards/grafana/sorafs_fetch_observability.json` (يتم تصديره إلى البوابة الإلكترونية
bajo **SoraFS → Fetch Observability**) لمحددات المنطقة/البيان،
خريطة الحرارة من قبل المورّد، والمخططات البيانية لتأخر القطع والقطع
تتزامن مفاتيح التحكم مع تلك التي تقوم بمراجعة SRE أثناء عمليات النسخ.
قم بتوصيل قواعد التنبيه على `dashboards/alerts/sorafs_fetch_rules.yml`
والتحقق من صحة محور Prometheus مع `scripts/telemetry/test_sorafs_fetch_alerts.sh`
(يقوم المساعد تلقائيًا بتشغيل `promtool test rules` محليًا أو Docker).
تتطلب عمليات نقل التنبيهات نفس حظر التوجيه الذي يفرضه
النص حتى يتمكن المشغلون من إضافة الأدلة إلى بطاقة الطرح.

### تدفق الاحتراق عن بعديتطلب عنصر خريطة الطريق **SF-6e** حرق القياس عن بعد لمدة 30 يومًا مسبقًا
قم بتغيير الأوركيستادور متعدد الأصول إلى قيمك GA. الولايات المتحدة الأمريكية لوس مخطوطات ديل
مستودع لالتقاط مجموعة من القطع الأثرية القابلة لإعادة الإنتاج كل يوم على حدة
نافذة:

1. قم بتشغيل `ci/check_sorafs_orchestrator_adoption.sh` مع المتغيرات
   أثناء تكوينات الاحتراق. مثال:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```المساعد يقوم بإعادة إنتاج `fixtures/sorafs_orchestrator/multi_peer_parity_v1`،
   أكتب `scoreboard.json`، `summary.json`، `provider_metrics.json`،
   `chunk_receipts.json` و `adoption_report.json` آخر
   `artifacts/sorafs_orchestrator/<timestamp>/`، ويطلب رقمًا واحدًا على الأقل
   الموردون المؤهلون الوسيط `cargo xtask sorafs-adoption-check`.
2. عند تقديم متغيرات النسخ، يتم إصدار النص أيضًا
   `burn_in_note.json`، التقاط العلامات، مؤشر اليوم، معرف
   البيان ومصدر القياس عن بعد وملخصات المصنوعات اليدوية. مساعد
   هذا JSON هو سجل الطرح لكي يوضح بوضوح ما الذي تم التقاطه في كل يوم
   نافذة لمدة 30 يومًا.
3. قم باستيراد اللوحة Grafana المحدثة (`dashboards/grafana/sorafs_fetch_observability.json`)
   في مساحة العمل/العرض المسرحي، وعلامات النسخ
   وقم بتأكيد أن كل لوحة تعرض لوحات للبيان/المنطقة قيد الاختبار.
4. إخراج `scripts/telemetry/test_sorafs_fetch_alerts.sh` (س `promtool test rules …`)
   cuando cambie `dashboards/alerts/sorafs_fetch_rules.yml` to documentar que
   يتزامن توجيه التنبيهات مع المقاييس المصدرة أثناء عملية النسخ.
5. أرشفة لقطة لوحة القيادة، واخرج من اختبار التنبيهات، ثم اخترها
   ذيل سجلات البحث `telemetry::sorafs.fetch.*` إلى جانبنا
   مصنوعات الأوركيستادور حتى يتمكن الحاكم من إعادة إنتاجها
   الأدلة بلا مقاييس إضافية للأنظمة في الجسم الحي.

## 7. قائمة التحقق من الطرح1. قم بإعادة إنشاء لوحات النتائج في CI باستخدام تكوين المرشحين والتقاطهم
   التحكم في القطع الأثرية أقل من الإصدارات.
2. قم بتنفيذ عملية تحديد التركيبات في كل مكان (مختبر، مرحلة،
   الكناري، الإنتاج) وملحقاتها `--scoreboard-out` و`--json-out`
   في سجل الطرح.
3. قم بمراجعة لوحات القياس عن بعد باستخدام مهندس تحت الطلب، مع التأكد من ذلك
   جميع المقاييس السابقة لها تأثيرات حية.
4. قم بتسجيل مسار التكوين النهائي (عادة عبر `iroha_config`) ثم
   الالتزام ببوابة السجل الحكومي المستخدم للإعلان والإخلاص.
5. تحديث أداة تعقب الطرح وإخطار معدات SDK حولها
   قيم جديدة من العيوب للحفاظ على تكامل العملاء
   alineadas.

اتبع هذا الدليل للحفاظ على تعقبات محددات الباحث و
قابلة للتدقيق، بينما تقوم برفع حلقات إعادة التغذية الواضحة للضبط
متطلبات الاحتفاظ وقدرة الموردين ووضع الخصوصية.