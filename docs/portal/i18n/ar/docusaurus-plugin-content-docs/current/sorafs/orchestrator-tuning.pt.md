---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-tuning.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: ضبط الأوركسترا
العنوان: الطرح والضبط من قبل orquestrador
Sidebar_label: ضبط أو تنفيذ المهمة
الوصف: تدريبات عملية وتوجيه ضبط ونقاط تفتيش سمعية لرفع أو أوركسترا متعدد الأطراف إلى GA.
---

:::ملاحظة Fonte canônica
إسبيلها `docs/source/sorafs/developer/orchestrator_tuning.md`. احتفظ بنسختين متضمنتين حتى تتم الموافقة على التوثيق البديل.
:::

# دليل التشغيل وضبط الأوركسترا

تم تحديد هذا الدليل على [مرجع التكوين](orchestrator-config.md) ولا
[دليل التشغيل متعدد الأصول](multi-source-rollout.md). هذا موضح
مثل ضبط الأوركسترا لكل مرحلة من بدء التشغيل، وكذلك ترجمة نظام التشغيل
يجب أن تكون صناعات لوحة النتائج والقياس عن بعد بعيدة جدًا
لتوسيع الحركة. قم بتطبيق التوصيات بشكل متسق على CLI، نحن
أدوات تطوير البرمجيات (SDKs) تعمل على الأتمتة بحيث لا يتبع كل شخص سياسة الجلب الحتمية.

## 1. قاعدة تكوينات المعلمات

جزء من قالب التكوين الذي تمت مشاركته وضبط مجموعة صغيرة
المقابض التي يتم طرحها مسبقًا. لوحة تلتقط القيم
توصيات بشأن الخطوات التالية للبلديات؛ القيم ليست مدرجة في القائمة
من `OrchestratorConfig::default()` و`FetchOptions::default()`.| فاس | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | نوتاس |
|------|-----------------|------------------------------------------------|-------------|----------------------------------------------------------|------||------|
| **مختبر/CI** | `3` | `2` | `2` | `2500` | `300` | يعرض الحد من زمن الوصول ومصدر النعمة تباطؤ القياس عن بعد بسرعة. استمر في إعادة المحاولة قليلاً للكشف عن البيانات غير الصالحة بشكل أكبر. |
| ** التدريج ** | `4` | `3` | `3` | `4000` | `600` | أطلق العنان لطرق الإنتاج التي تم تقديمها لأقرانك المستكشفين. |
| **كناري** | `6` | `3` | `3` | `5000` | `900` | Igual aos Padrões; حدد `telemetry_region` حتى تتمكن لوحات المعلومات من فصل حركة المرور. |
| **العجز العام** | `None` (استخدام كل ما هو أنيق) | `4` | `4` | `5000` | `900` | قم بتعزيز حدود إعادة المحاولة لاستيعاب عمليات الانتقال الخاطئة أثناء استمرار المستمعين في إعادة المحاولة أو التحديد. |- `scoreboard.weight_scale` لا يبقى على المسار `10_000` حتى لا يخرج نظام المصب من القرار الداخلي. زيادة الدرجات لا تغير ترتيب الإجراءات؛ لن يؤدي إلا إلى توزيع قروض أكثر كثافة.
- للترحيل بين المراحل، استمر في حزمة JSON واستخدم `--scoreboard-out` لتتمكن من تسجيل مجموعة من المعلمات الإضافية.

## 2. قم بإجراء النظافة على لوحة النتائج

تجمع لوحة النتائج بين متطلبات البيان وإعلانات الشهادات والقياس عن بعد.
ما قبل التقدم:1. **التحقق من دقة القياس عن بعد.** ضمان الرجوع إلى اللقطات
   `--telemetry-json` يتم التقاطه داخل ملف التهنئة الذي تم تكوينه. مدخلات
   mais antigas que `telemetry_grace_secs` falham com `TelemetryStale { last_updated }`.
   قم بذلك كقفل متين وتمكن من تصدير أجهزة القياس عن بعد قبل المتابعة.
2. **الاستقصاء عن أسباب الأهلية.** الاستمرار في العمل عن طريق
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. كل مدخل
   Traz um bloco `eligibility` as a exata da falha. لا سوبرسكريفا
   اختلالات القدرة أو الإعلانات منتهية الصلاحية؛ نقل الحمولة إلى المنبع.
3. **مراجعة تعديلات البيزو.** قارن المجال `normalised_weight` مع الإصدار
   الأمامي. التعديلات التي تزيد عن 10% مرتبطة بالتعديلات المدروسة في الإعلانات
   أو القياس عن بعد ومن الضروري أن يتم تسجيله بدون سجل بدء التشغيل.
4. **احصل على العناصر المصطنعة.** قم بتكوين `scoreboard.persist_path` لكل تنفيذ
   إميتا أو لقطة نهائية من لوحة النتائج. ملحق أو قطعة فنية في سجل الإصدار
   جنبًا إلى جنب مع البيان وحزمة القياس عن بعد.
5. **تسجيل أدلة مزيج الأدلة.** البيانات التعريفية لـ `scoreboard.json` _e_ o
   `summary.json` مراسلي التصدير `provider_count`، `gateway_provider_count`
   e o التسمية المشتقة `provider_mix` لكي تثبت المراجعات أنها قد تم تنفيذها
   `direct-only`، `gateway-only` أو `mixed`. تقرير بوابة Capturas `provider_count=0`e `provider_mix="gateway-only"`، أثناء تنفيذ الأخطاء التي لا تظهر عدوى
   صفر الفقرة ambas كخطوط. `cargo xtask sorafs-adoption-check` يؤثر على هذه المجالات
   (وعندما تكون العدوى/التسميات متباعدة)، قم بتنفيذها دائمًا معًا
   `ci/check_sorafs_orchestrator_adoption.sh` أو برنامج الالتقاط الخاص بك للإنتاج
   حزمة الأدلة `adoption_report.json`. عندما تكون البوابات Torii متجددة
   المضمنة، والحفاظ على `gateway_manifest_id`/`gateway_manifest_cid` مع البيانات الوصفية
   لوحة النتائج لكي ترسل بوابة التخصيص المرتبطة بظرف البيان
   com o mix deprovinores capturado.

لتحديد تفاصيل المجالات، مرة أخرى
`crates/sorafs_car/src/scoreboard.rs` وإستراتيجية عرض CLI للعرض
`sorafs_cli fetch --json-out`.

## مرجع العلامات إلى CLI وSDK

`sorafs_cli fetch` (الإصدار `crates/sorafs_car/src/bin/sorafs_cli.rs`) والمجمّع
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) مشترك
نفس سطح تكوين الأوركسترا. استخدم علامات os seguintes ao
التقاط أدلة الطرح أو إعادة إنتاج التركيبات القانونية:

الرجوع إلى العلامات المشتركة ذات الأصول المتعددة (الحفاظ على مساعدة CLI والمستندات
sincronizados Editando apenas este arquivo):- `--max-peers=<count>` يحدد الكميات المختارة بشكل مختصر من خلال مرشح لوحة النتائج. تم تكوين Deixe Sem لتسهيل البث لجميع المزودين الأنيقين وتحديد `1` فقط عند ممارسة التمرين بشكل مدروس أو احتياطي من المصدر الوحيد. قم بتغيير المقبض `maxPeers` إلى مجموعات SDK (`SorafsGatewayFetchOptions.maxPeers`، `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` يقوم بالتسجيل لوضع حد لإعادة المحاولات للقطعة المطبقة على `FetchOptions`. استخدم لوحة بدء التشغيل بدون دليل ضبط للقيم الموصى بها؛ يجب أن تتوافق عمليات تنفيذ CLI التي تجمع الأدلة مع منصات SDK للحفاظ على الجدار.
- يتم تدوير `--telemetry-region=<label>` كسلسلة Prometheus `sorafs_orchestrator_*` (وتحتوي على OTLP) كتسمية منطقة/بيئة لفصل لوحات المعلومات عن حركة المختبر والتدريج والكناري وGA.
- `--telemetry-json=<path>` أدخل أو لقطة مرجعية للوحة النتائج. استمر في استخدام JSON على لوحة النتائج حتى يتمكن المدققون من إعادة إنتاج التنفيذ (ولإثبات أن `cargo xtask sorafs-adoption-check --require-telemetry` يمكنه بث OTLP للالتقاط).
- `--local-proxy-*` (`--local-proxy-mode`، `--local-proxy-norito-spool`، `--local-proxy-kaigi-spool`، `--local-proxy-kaigi-policy`) تأهيل خطافات نظام التشغيل لجسر المراقبة. عندما يتم تحديده، يقوم الأوركسترا بإرسال قطع عبر الوكيل Norito/Kaigi المحلي حتى يتمكن عملاء المتصفح، ويحرسون ذاكرات التخزين المؤقت والرسائل، من Kaigi يحصلون على نفس الرسائل الصادرة من Rust.- `--scoreboard-out=<path>` (اختياري مع `--scoreboard-now=<unix_secs>`) يستمر في لقطة الأهلية للمدققين. قم دائمًا بتحديث JSON المستمر مع أدوات القياس عن بعد والبيانات المرجعية بدون تذكرة إصدار.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` يضبط التطبيق المحددات المتعلقة بالبيانات الوصفية للإعلانات. استخدم esses flags apenas para ensaios؛ يجب أن يتم تخفيض الإنتاج من خلال صناعات الحكم بحيث لا ينطبق كل شيء على حزمة سياسية واحدة.
- `--provider-metrics-out` / `--chunk-receipts-out` يتم إعادة قياس المقاييس من خلال إثبات وتلقي القطع المرجعية من خلال قائمة التحقق من الطرح؛ قم بإرفاق مجموعة كبيرة من المصنوعات لتسجيل أدلة التأييد.

مثال (استخدام أو تركيبات منشورة):

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

تستهلك حزم SDK نفس التكوين عبر `SorafsGatewayFetchOptions` ولا يوجد عميل
الصدأ (`crates/iroha/src/client.rs`)، روابط nos JS
(`javascript/iroha_js/src/sorafs.js`) ولا يوجد SDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Mantenha esses مساعدين م
التزامن مع منصات CLI حتى يتمكن المشغلون من نسخ السياسة بين
الأتمتة دون مجموعات من التجارة على نطاق واسع.

## 3. ضبط سياسة الجلب

`FetchOptions` يتحكم في إعادة المحاولة والمطابقة والتحقق. أو أجوستار:- **إعادة المحاولة:** رفع `per_chunk_retry_limit` فوق `4` لزيادة وتيرة العمل
  التعافي يمكن أن يكون ماسكارا فالهاس دي بروفورس. Prefira manter `4` como
  تيتو ووثق في دورة المحامين لتصدير الشجار.
- **حدود الخطأ:** `provider_failure_threshold` حدد متى تم إثبات ذلك
  تعطيل الجلسة المتبقية. هذه هي الشجاعة السياسية لإعادة المحاولة:
  حد أدنى من أن أداة إعادة المحاولة تجبر الأوركسترا على الهروب من نظير
  قبل أن تختار جميع المحاولات.
- **التزامن:** تم تعريف Deixe `global_parallel_limit` (`None`) على الأقل
  بيئة معينة لا تشبع النطاقات المعلنة. عندما يتم تحديده,
  يضمن أن القيمة ستكون ≥ على بعض عمليات التدفق من مقدمي الخدمات من أجل
  تخلص من المجاعة.
- **تبديل التحقق:** `verify_lengths` و`verify_digests` دائم
  مؤهلون للإنتاج. يضمنون الحتمية عندما تكون هناك أخطاء كثيرة
  بروفيدوريس. يتخلص من التشويش في البيئات المعزولة.

## 4. مكان النقل المجهول

استخدم الحرمين `rollout_phase` و`anonymity_policy` و`transport_policy` للفقرة
ممثل موقف الخصوصية:- اختر `rollout_phase="snnet-5"` واسمح بسياسة مجهولة المصدر
  يرافق أنظمة التشغيل SNNet-5. استبدال عبر `anonymity_policy_override` apenas
  عندما تصدر الحكومة أمرًا مباشرًا.
- Mantenha `transport_policy="soranet-first"` كما هو الحال في SNNet-4/5/5a/5b/6a/7/8/12/13 🈺
  (فيجا `roadmap.md`). استخدم `transport_policy="direct-only"` أحيانًا للتخفيضات
  توثيق أو تمارين الامتثال وضمان مراجعة تغطية PQ السابقة
  المعزز `transport_policy="soranet-strict"` — هذا مستوى سريع للغاية وهو بسيط
  relés classics permanecerem.
- يجب أن يكون `write_mode="pq-only"` ضروريًا عند كل مسار كتابة (SDK،
  orquestrador، أدوات الحكم) يمكن أن تلبي متطلبات PQ. دورانتي
  عمليات النشر، والحفاظ على `write_mode="allow-downgrade"` لاستجابة الطوارئ
  يمكنك استخدام التدوير المباشر من خلال قياس البيانات عن بعد أو الرجوع إلى إصدار أقدم.
- اختيار الحراس وتنظيم الدوائر التابعة لمدير SoraNet.
  حذف اللقطة من `relay_directory` والحفاظ على ذاكرة التخزين المؤقت `guard_set`
  للإبقاء على تقلبات الحماية على أساس الاحتفاظ المرن. انطباع
  يتم تسجيل ذاكرة التخزين المؤقت الرقمية بواسطة `sorafs_cli fetch` كجزء من دليل الطرح.

## 5. خطافات الالتزام بالرجوع إلى إصدار أقدم

ما هي الأساليب التي يساعد بها الأوركسترا على استخدام دليل التدخل السياسي:- **إصلاح الرجوع إلى إصدار سابق** (`downgrade_remediation`): مراقبة الأحداث
  `handshake_downgrade_total`، بعد تكوين `threshold` سيتم تجاوزه
  `window_secs`، استخدم الوكيل المحلي لـ `target_mode` (بيانات التعريف فقط للنطاق).
  احتفظ بالوسائط (`threshold=3`، `window=300`، `cooldown=900`) على الأقل
  تشير مراجعات الأحداث إلى المسار الآخر. Documente qualquer تجاوز رقم
  سجل بدء التشغيل ويضمن أن لوحات المعلومات مصاحبة لـ `sorafs_proxy_downgrade_state`.
- **سياسة الامتثال** (`compliance`): مقتطفات من التشريع والبيان
  تدفق قوائم إلغاء الاشتراك في نطاق الإدارة العامة. Nunca insira يتجاوز المخصص
  لا توجد حزمة تكوين؛ في وقت لاحق، اطلب تحديثًا سريعًا
  `governance/compliance/soranet_opt_outs.json` وإعادة الزرع أو JSON gerado.

لضم الأنظمة، استمر في حزمة التكوين الناتجة وقم بإدراجها
لدينا أدلة الإصدار لكيفية قيام المدققين بالتراجع كما هو الحال في التخفيضات
com.acionadas.

## 6. لوحات المعلومات والقياس عن بعد

قبل التوسيع أو الطرح، تأكد من أن الخطوات التالية ليست فعالة
البيئة المحيطة:- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  يجب أن نصل إلى الصفر بعد التوصل إلى نتيجة بشأن الكناري.
- `sorafs_orchestrator_retries_total` ه
  `sorafs_orchestrator_retry_ratio` — يجب أن يتم التثبيت لمدة تصل إلى 10% على الأقل
  الكناري ويستمر بنسبة 5% بعد GA.
- `sorafs_orchestrator_policy_events_total` - التحقق من أن مرحلة البدء قيد التنفيذ
  هذا هو الحال (التسمية `stage`) وقم بتسجيل النواتس عبر `outcome`.
-`sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — مرافقة لإصدارات PQ الإضافية
  العلاقة مع التوقعات السياسية.
- Alvos de log `telemetry::sorafs.fetch.*` — يجب أن يتم إرسالها إلى مجمع السجلات
  قم بالمشاركة مع عمليات البحث عن الخلاصات لـ `status=failed`.

قم بنقل لوحة القيادة Grafana canonico em
`dashboards/grafana/sorafs_fetch_observability.json` (لا يتم تصديره إلى البوابة الإلكترونية
**SoraFS → Fetch Observability**) لمحددات المنطقة/البيان، أو
إعادة المحاولة للخريطة الحرارية، والمخططات البيانية لزمن وصول القطع ونظام التشغيل
تتوافق أجهزة التحكم في التوقف مع مراجعة SRE أثناء عمليات النسخ. كونيكت
كما يتم إعادة تعيين مدير التنبيهات في `dashboards/alerts/sorafs_fetch_rules.yml` والصلاحية أ
sintaxe do Prometheus com `scripts/telemetry/test_sorafs_fetch_alerts.sh` (أو المساعد
قم بتنفيذ `promtool test rules` محليًا أو Docker). كممرات للتنبيهات
أطلب من نفس كتلة الدوران أن يفرض البرنامج النصي على المشغلين
قم بإرفاق دليل على بطاقة الطرح.

### تدفق الاحتراق للقياس عن بعديتطلب عنصر خريطة الطريق **SF-6e** حرق القياس عن بعد لمدة 30 يومًا قبل ذلك
البديل أو الأوركسترا متعدد الأصول لأبناء GA الخاص بهم. استخدم البرامج النصية لنظام التشغيل
مخزن لالتقاط مجموعة من المصنوعات اليدوية القابلة لإعادة الإنتاج لكل يوم
جانيلا:

1. قم بتنفيذ `ci/check_sorafs_orchestrator_adoption.sh` كمتغير للنسخ
   التكوينات. مثال:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```يا مساعد المنتج `fixtures/sorafs_orchestrator/multi_peer_parity_v1`،
   جرافا `scoreboard.json`، `summary.json`، `provider_metrics.json`،
   `chunk_receipts.json` و`adoption_report.json` م
   `artifacts/sorafs_orchestrator/<timestamp>/`، ويعطي أقل عدد ممكن من
   ثبت elegíveis عبر `cargo xtask sorafs-adoption-check`.
2. عندما يتم تقديم تنوع في النسخ، سيتم إصدار النص أيضًا
   `burn_in_note.json`، التقاط أو تسمية، أو مؤشر القطر، أو معرف البيان،
   مصدر للقياس عن بعد وهضم المصنوعات. Anexe esse JSON ao log de
   بدء التشغيل لـ deixar claro qual Captura يرضي كل يوم من أيام 30.
3. استيراد لوحة المعلومات Grafana التي تم تحديثها (`dashboards/grafana/sorafs_fetch_observability.json`)
   بالنسبة لمساحة العمل المرحلية/الإنتاجية، قم بتمييزها بملصق النسخ والتأكيد
   كل لوحة تعرض المزيد من التفاصيل للبيان/المنطقة في الاختبار.
4. نفذ `scripts/telemetry/test_sorafs_fetch_alerts.sh` (أو `promtool test rules …`)
   semper que `dashboards/alerts/sorafs_fetch_rules.yml` mudar to documentar que
   o تدوير التنبيهات المتوافقة مع المقاييس المصدرة أثناء عملية الحرق.
5. احفظ لقطة على لوحة القيادة، واتبع اختبار التنبيهات ونهاية السجلات
   das buscas `telemetry::sorafs.fetch.*` junto aos artefatos do orquestrador para
   ما الذي يمكن للحكومة إنتاجه من الأدلة دون خارج مقاييس الأنظمة
   م الإنتاج.

## 7. قائمة التحقق من الطرح1. قم بتجديد لوحات النتائج في CI باستخدام تكوين بيانات المرشحين والتقاطها
   artefatos sob controle de versão.
2. قم بتنفيذ وجلب التركيبات المحددة في كل محيط (المختبر، التدريج،
   الكناري، الإنتاج) وملحق المصنوعات `--scoreboard-out` و`--json-out` ao
   سجل الطرح.
3. قم بمراجعة لوحات القياس عن بعد مع مهندس المزرعة، مع ضمان ذلك
   todas as métricas acima Tenham amostras ao vivo.
4. قم بتسجيل المسار النهائي للتكوين (بشكل عام عبر `iroha_config`) وما إلى ذلك
   الالتزام ببوابة تسجيل الحوكمة المستخدمة للإعلانات والامتثال.
5. قم بتنشيط متتبع الإطلاق والإخطار كمعدات SDK جديدة
   منصات للمساعدة في تكامل العملاء المشتركين.

اتبع هذا الدليل للحفاظ على عمليات نشر المحددات الرئيسية
سلبيات السمع، أثناء توفير حلقات ردود فعل واضحة للضبط
أدوات إعادة المحاولة، وسعة الحماية، ووضعية الخصوصية.