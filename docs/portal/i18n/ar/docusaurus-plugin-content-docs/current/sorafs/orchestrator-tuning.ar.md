---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-tuning.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: ضبط الأوركسترا
العنوان: تحرير وضبط المُنسِّق
Sidebar_label: ضبط المُنسِّق
الوصف: قيم الطريقة الافتراضية و التوجيهات ضبط و نقاط تدقيق لمواءمة المُنسِّق متعدد الدقة مع GA.
---

:::ملحوظة المصدر مؤهل
تعكس `docs/source/sorafs/developer/orchestrator_tuning.md`. حافظ على تطابق النسختين إلى أن تُحال مجموعة التوثيق القديمة للتقاعد.
:::

#تحرير وضبط المُنسِّق

يبني هذا الدليل على [مرجع الإعداد](orchestrator-config.md) و
[دليل الحصول على مصادر متعددة](multi-source-rollout.md). يشرح
كيفية ضبط المُنسِّق لكل مرحلة محددة، وكيفية قراءة لوحة التأثير، وما إلى ذلك
إشارات التليميترية المطلوبة قبل الموعد المحدد. صلاحيات سيئة بشكل متقاطع
CLI وSDKs والرسم حتى متابعة كل عقدة لنقل حتمية واحدة.

## 1. مجموعات المعلمات الأساسية

ابدأ من إعداد قالب مشترك واضبط مجموعة صغيرة من المقابض مع حدوث التطورات. يلتقط
الجدول أدناه لتحديد الأسباب المحددة؛ وما لم يُذكر يعود إلى
الافتراضات في `OrchestratorConfig::default()` و`FetchOptions::default()`.| المرحلة | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | مذكرة |
|---------|-----------------|----------------|------------------------------------------------------------|-------------------------------------------|-----|------|-----|
| **المختبر / CI** | `3` | `2` | `2` | `2500` | `300` | يكشف السقف الضيق كمون ونافذة سماح قصيرًا عن المزعجة بسرعة. خفّض المحاولات لكشف المانيفستات غير الصالحة. |
| ** التدريج ** | `4` | `3` | `3` | `4000` | `600` | يعكس قيم الإنتاج مع ترك فسحة النصائح الاستكشافية. |
| **الكناري** | `6` | `3` | `3` | `5000` | `900` | يطابق القيم الافتراضية؛ اضبط `telemetry_region` حتى يبدأ التشغيل من فصل حركة الكناري. |
| **الإتاحة العامة (GA)** | `None` (استخدم جميع المؤهلين) | `4` | `4` | `5000` | `900` | ارفع حدود إعادة المحاولة والفشل لعدم الإعفاء من العبء، بالإضافة إلى ضمان في ضمان الحتمية. |

- يبقى `scoreboard.weight_scale` على القيمة الافتراضية `10_000` ما لم يتطلب نظام لاحق غير محدد عددية مختلفة. زيادة المقياس لا تغير ترتيب المتحكمين؛ بل تنتج توزيعًا أدق للرصيد.
- عند الانتقال من المراحل، احفظ حزمة JSON الرقمية `--scoreboard-out` لتوثيق مجموعة المعلمات الدقيقة في مسار التدقيق.

## 2. نتائج لوحة التنظيفمكونات اللوحة الضخمة بين متطلبات المانيفست والإعلانات المتحكمة والتليمترية.
قبل المضي قدما:1. **تحقّق من حداثة التليمترية.** تأكد من أن المقطع المشار إليها عبر
   `--telemetry-json` تم التقاطه ضمن نافذة الدوثر. الإدخالات الأقدم من
   `telemetry_grace_secs` فشل مع `TelemetryStale { last_updated }`.
   اعتبر ذلك توقفًا حرصيًا وراجع تصدير التليمترية قبل المتابعة.
2. ** راجع الأسباب الأهلية.** احفظ الآثار عبر
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. كل المصدر
   الضغط على كتلة `eligibility` بسبب التأخير في الفشل. لا تتجاوز موافقتك أو
   انتهت الرسائل؛ أصلح الحمولة المصدرية.
3. **راجع تغيرات الوزن.** قارن البحث `normalised_weight` بالإصدار السابق. تغيير
   أكبر من 10% يجب أن ترتبط بتغييرات قطعودة في الإعلانات أو التليميترية ويمكن تسجيلها
   في سجل الفشل.
4. **أرشف الآثار.** اضبط `scoreboard.persist_path` لتنزيل كل لقطة للوحة النتائج
   المباراة. أرفق برنامج المصارعة بالإصدار المانيفست والزمة التليميرية.
5. **دليل الوثيقة المدققة** يجب أن تتعرض ميتاداتا `scoreboard.json` و`summary.json`
   المطابق يستوعب `provider_count` و`gateway_provider_count` والتسمية `provider_mix` لإثبات
   ما إذا كان التشغيل `direct-only` أو `gateway-only` أو `mixed`. يجب أن تقطع لقطات
   البوابة `provider_count=0` و`provider_mix="gateway-only"`، أثناء طلبات التشغيلات المتعددة
   سجلًا غير صفرية للمصدرين. يفرض `cargo xtask sorafs-adoption-check` هذه الكلمة
   (ويفشل إذا لم تتطابق العدادات/التسميات)، لذا شغّله دائمًا مع
   `ci/check_sorafs_orchestrator_adoption.sh` أو نص القاطع يوجد لديك حزمة من الأدلة
   `adoption_report.json`. عند بوابات Torii، احفظ `gateway_manifest_id`/`gateway_manifest_cid` ضمن ميتاداتا لوحة النتائج حتى يتمكن من التبني من
   المغلف م ربط المانيفست بخليط المتحكمين بالملتقط.

للتعريفات الجراحية ذات الصلة
`crates/sorafs_car/src/scoreboard.rs` وبنية ملخص CLI التي يخرجها
`sorafs_cli fetch --json-out`.

## مرجع أعلام CLI وSDK

`sorafs_cli fetch` (راجع `crates/sorafs_car/src/bin/sorafs_cli.rs`) والواجهة
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) اشترك في
سطح إعداد المُنسِّق نفسه. استخدم الأعلام التالي عند التقاط الفساد أو
إعادة تشغيل الـ التركيبات القياسية:

مرجع أعلام متعدد المصادر (أبقِ مساعدة CLI والوثائق المتزامنة بتعديل هذا الملف فقط):- `--max-peers=<count>` يحدد عدد المحاسبين المؤهلين الذين ينجون من لوحة النتائج. اتركه كاملًا لتدفق جميع المحاسبين المؤهلين، وضبطه على `1` فقط عند اختبار الرجوع لمصدر واحد عمدًا. يعكس مفتاح `maxPeers` في SDKs (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` يمرّر حد إعادة المحاولة لكل قطعة الذي يطبقه `FetchOptions`. استخدم جدولًا واضحًا في دليل الضبط للقيم لها؛ يجب أن تتطابق تشغيلات CLI التي تحتوي على أدلة كثيرة لـ SDK للمحافظة على التوافق.
- `--telemetry-region=<label>` يوسم سلاسل Prometheus `sorafs_orchestrator_*` (ومرحّلات OTLP) بسم المنطقة/البيئة حتى البداية لوحات جديدة من فصل الحركة المختبرة والتدريجية والكناري وGA.
- `--telemetry-json=<path>` يحقن لقطة التليمترية المشار إليها في لوحة النتائج. احفظ JSON بجوار لوحة النتائج كي المرقمون من إعادة التشغيل (ولكي يثبت `cargo xtask sorafs-adoption-check --require-telemetry` أي تدفق OTLP غذاء الالتقاط).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) يفعّل الخطافات الخاصة بالجسر. عند ضبطها، يمرّر المُنسِّق الـ Chunks عبر Proxy Norito/Kaigi المحلي حتى يعرف معروف المتصفح وguard التخزين المؤقت لـ Kaigi نفس الإيصالات التي تصدرها Rust.
- `--scoreboard-out=<path>` (اختياريًا مع `--scoreboard-now=<unix_secs>`) يحفظ لقطة الحضانة للمشرفين. ربط دائم الـ JSON المحفوظ بآثار التليميترية والمانيفيست المشار إليها في تذكرة الإصدار.- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` يطبّقان تعديلات حتمية فوق ميتاداتا الإعلانات. استخدم هذه الأعلام للتجارب فقط؛ ويجب أن تخضع لممارسات عامة للإنتاج عبر المصنوعات اليدوية والتوجيه كي تطبّق كل عقدة نفس حزمة السياسة.
- `--provider-metrics-out` / `--chunk-receipts-out` وضوحان بمقاييس صحة المتحكمين وإيصالات الـ القطع المرجعية في قائمة التحقق؛ أرفق كلا الأثرين عند تقديم دليل التبني.

مثال (باستخدام الـ Fixed Post):

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

تستهلك أدوات تطوير البرامج (SDKs) البارعة عبر `SorafsGatewayFetchOptions` في عميل Rust
(`crates/iroha/src/client.rs`) وروابط JS
(`javascript/iroha_js/src/sorafs.js`) وSDK سويفت
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). حافظ على تطابق هذه
المساعدة مع CLI حتى يبدأوا في العمل على الأخذ بالرسوم دون
طبقات مخصصة ترجمة.

## 3. ضبط أبي الجلب

يتحكم في `FetchOptions` في إعادة محاولة التوازي والتحقق. عند الضبط:- **إعادة المحاولة:** رفع `per_chunk_retry_limit` فوق `4` يزيد زمن الاستعادة لكنه
  قد يخفي أعطال المتحكمين. متخصصة في الحد الأدنى من `4` والاعتماد على القياس المتحكم فيه
  للوزن الضعيف.
- **عتبة بناء على:** التسجيل `provider_failure_threshold` متى يتم التحقق من المدقق لبقية الحضور.
  يجب أن تأخذ في الاعتبار هذه الاعتبارات مع إعادة المحاولة: توقف أقل من الرغبة في المحاولة
  تُخرج المتحكم قبل استنفاد جميع المحاولات.
- **التوازن:** اترك `global_parallel_limit` غير مضبوط (`None`) ما لم تكن بيئة محددة
  غير قادر على تشبع النطاقات المُعلنة. عند ضبطه، تأكد من أن القيمة ≥ مجموع القياسات
  المقاييس للمحكمين التجويع.
- **مفاتيح التحقق:** يجب أن تشمل `verify_lengths` و`verify_digests` مفعّلين في الإنتاج.
  فهي تشمل الحتمية عندما تعمل أساطر متفاوتة؛ عطّلها فقط في بيئات ضبابية المعزولة.

## 4. مراحل البناء والخصوصية

استخدم `rollout_phase` و`anonymity_policy` و`transport_policy` لتمثيل وضع الخصوصية:- `rollout_phase="snnet-5"` تبرعت بتخصيص خصوصية لتتبع معالم SNNet-5. مستخدم
  `anonymity_policy_override` فقط عندما يخضع التوجيه إلى موقّعًا.
- أبقِ `transport_policy="soranet-first"` كخيار أساس بينما SNNet-4/5/5a/5b/6a/7/8/12/13 في 🈺
  (راجع `roadmap.md`). استخدم `transport_policy="direct-only"` فقط لخفض موثق أو تدريبات اخضر،
  انتظر تفاصيل تفاصيل PQ قبل الترقية إلى `transport_policy="soranet-strict"`—هذا المستوى يفشل سريعًا
  إذا شاركت في رحيلات كلاسيكية فقط.
- لا يشمل `write_mode="pq-only"` إلا عندما يكون لديك كل مسارات الكتابة (SDK، المُنسِّق، أدوات النتو)
  تلبية متطلبات PQ. أثناء الفشل، أبقِ `write_mode="allow-downgrade"` حتى يمكن للخطط الطارئة
  تعتمد على المسارات المباشرة بينما تُعلِّم توضيحًا لحالات التخفيض.
- يعتمد اختيار الحراس وتجهيز الدائرة على دليل SoraNet. أداة لقطة `relay_directory` الموقع
  واحفظ `guard_set` مخبأ كي يبقى تغيير الحراس ضمن نافذة المتفق عليها. بصمة الكاش
  التي سجلتها `sorafs_cli fetch` جزء لا يتجزأ من الفشل.

## 5. خطافات التخفيض والإختيار

يساعد نظامان فرعيان في المُنسِّق على فرض السياسة ولا يؤثر سلباً على:- **معالجة التخفيض** (`downgrade_remediation`): مراقبة أحداث `handshake_downgrade_total`، وبعد تجاوز
  `threshold` داخل `window_secs` تُجبر الـ الوكيل المحلي على `target_mode` (بيانات التعريف الافتراضية فقط).
  اعتمد بالقيم الافتراضية (`threshold=3`, `window=300`, `cooldown=900`) ما لم يحدث مراجعات جديدة
  نمطًا مختلفًا. وثيق أي تجاوز في سجل الوضوح والتأكد من متابعة اللوحات ومراقبتها
  `sorafs_proxy_downgrade_state`.
- **السياسة الشاملة** (`compliance`): باستثناء استثناءات الكوليسترول والمانيفيست عبر قوائم إلغاء الاشتراك التي
  السيطرة عليها. لا تُدرِج يتجاوز التجهيز الملائم؛ اطلب تحديثًا موقعًا
  لـ `governance/compliance/soranet_opt_outs.json` وأعد نشر JSON الناتج.

بالنسبة لكل النظامين، احفظ الإمكانية المتاحة وأرفقها بأمثلة حتى تتنوع
المتمون من تتبع كيفية تفعيل التخفيضات.

## 6.التليمرية ولوحات المتابعة

قبل الانتهاء تمامًا، تأكد من أن الأقسام التالية في التفاصيل الدقيقة:

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  يجب أن يكون صفرًا بعد القانون الكناري.
- `sorafs_orchestrator_retries_total` و
  `sorafs_orchestrator_retry_ratio` — يجب أن تحدد أقل من 10% أثناء الكناري وتتبقى
  تحت 5% بعد GA.
- `sorafs_orchestrator_policy_events_total` — لماذا من أن لم يتم إلغاء الاشتراك
  فعال (label `stage`) ويسجل حالات brownout عبر `outcome`.
-`sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — تتبع عرض ترحيلات PQ مقابل التوقعات السياسية.
- أهداف السجلات `telemetry::sorafs.fetch.*` — يجب ضبطها إلى مجمّع سجلات مع
  عمليات بحث محفوظة لـ `status=failed`.نتنياهو لوحة Grafana عالية الجودة
`dashboards/grafana/sorafs_fetch_observability.json` (المصدّرة في البوابة تحت
**SoraFS → Fetch Observability**) للتأكد من تطابق محددات المنطقة/المانيفست وخريطة
هدف المتحكمين والهيستوغرامات والعدادات مع ما يراجعه فريق SRE أثناء الاحتراق.
ربط متطلبات Alertmanager في `dashboards/alerts/sorafs_fetch_rules.yml` والتحقق من
صياغة Prometheus عبر `scripts/telemetry/test_sorafs_fetch_alerts.sh` (يشغّل المساعدة
`promtool test rules` محليًا أو في Docker). عمليات شراء إشعارات نفس الكتلة
التوجيه الذي يطبعها السكربت حتى يبدأون من الأدلة التي تذكروا الفشل.

### تدفق حرق للتليمترية

يلزم بند خارطة الطريق **SF-6e** فترة حرق للتليمتر لمدة 30 يومًا قبل تحويل
المُنسِّق مصادره المتعددة إلى مسؤوليته GA. استخدم سكربتات المستودع وتوزيع الحزمة
قابلة للتحرير للإنتاج لكل يوم في النافذة:

1. شغّل `ci/check_sorafs_orchestrator_adoption.sh` مع ضبطات الاحتراق. مثال:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```إعادة تشغيل المساعد `fixtures/sorafs_orchestrator/multi_peer_parity_v1`، ويكتب
   `scoreboard.json` و`summary.json` و`provider_metrics.json` و`chunk_receipts.json`
   و`adoption_report.json` تحت `artifacts/sorafs_orchestrator/<timestamp>/`،
   ونفترض حدًا أدنى من المراقبين المؤهلين عبر `cargo xtask sorafs-adoption-check`.
2. عند توفر متغيرات الاحتراق، يصدر السكربت أيضًا `burn_in_note.json` لتوثيق الوسم،
   وفهرس اليوم، ومعرّف المانيفست، ومصدر التليميترية، وملخصات الآثار. أرفق هذا JSON
   سجل الفشل لتوضيح أي لقطة استوفت كل يوم ضمن نافذة 30 يوم.
3. استراد لوحة Grafana المحدثة (`dashboards/grafana/sorafs_fetch_observability.json`)
   في مساحة التدريج/الإنتاج، وضع توصيل الحرق، والتأكد من أن كل لوحة تم بثها
   للمانيفست/المنطقة التجريبية.
4. شغّل `scripts/telemetry/test_sorafs_fetch_alerts.sh` (أو `promtool test rules …`)
   عند تغيير `dashboards/alerts/sorafs_fetch_rules.yml` لتوثيق أن توجيه الإشعارات
   تتوافق مع المعايير المصدرة أثناء عملية الاحتراق.
5. أرشف لقطة لوحة المتابعة، ومخرجات اختبار التنبيهات، وذيل تسجيل الدخول
   `telemetry::sorafs.fetch.*` مع المهاجم المنسِّق حتى يتم إعادة إنتاجه
   الأدلة دون الاعتماد على الأنظمة البيولوجية.

## 7. قائمة التحقق من كل شيء1. إعادة إنشاء لوحات النتائج في CI باستخدام أدوات تغيير الآثار تحت التحكم بالإصدارات.
2. شغّل تركيبات حطمي في كل بيئة (المختبر، التدريج، الكناري، الإنتاج) وأرفق
   سجل التعقب `--scoreboard-out` و`--json-out` في السجل الشامل.
3. تجدد اللوحات التليميترية مع مهندس المناوبة، والتأكد من كل المقاييس الإضافية لها بيولوجيا.
4. سجل التحرك النهائي (عادة عبر `iroha_config`) والتزم git لسجل التلاعب بالمستخدم
   للإعلانات والإختيار.
5. تحديث مستجدات أحدث وأحدث فرق SDK بالافتراضات الجديدة حتى استمرار تكامل العملاء المتسقين.

اتباع هذا الدليل يحافظ على تحرير المُنسِّق حتمية وقابل للدقيق، مع توفير روابط
إعادة النظر في إعادة النظر لضبط المراقبات الرقمية إعادة المحاولة، والسماح بتغذية السلطة التنظيمية، الخصوصية.