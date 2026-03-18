---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-tuning.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: ضبط الأوركسترا
العنوان: Déploiement et réglage de l’orchesstrateur
Sidebar_label: تنظيم الأوركسترا
الوصف: قيم من خلال الممارسات الافتراضية ونصائح الضبط ونقاط التدقيق لتحسين تنسيق المصادر المتعددة في GA.
---

:::ملاحظة المصدر الكنسي
ريفليت `docs/source/sorafs/developer/orchestrator_tuning.md`. قم بمحاذاة النسختين حتى تتم إزالة الوثائق الموروثة.
:::

# دليل النشر والتنظيم

يتم الضغط على هذا الدليل على [مرجع التكوين](orchestrator-config.md) ثم
[دليل النشر متعدد المصادر](multi-source-rollout.md). سأشرح
قم بضبط المنسق لكل مرحلة من مراحل النشر، قم بتعليق المترجم
يجب أن تكون عناصر لوحة النتائج وأي إشارات عن بعد في مكانها
قبل توسيع حركة المرور. Appliquez ces recommandations de manière cohérente dans
La CLI وSDK والأتمتة لكل ما يتبع نفس السياسة
جلب التحديد.

## 1. ألعاب الإعدادات الأساسية

مشاركة نموذج تكوين المشاركة وضبط مجموعة صغيرة من الإعدادات
الفراء وقياس النشر. تلتقط اللوحة هذه القيم
يوصى به لمراحل les plus courantes ; القيم غير مدرجة
على القيم الافتراضية `OrchestratorConfig::default()` و`FetchOptions::default()`.| المرحلة | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | ملاحظات |
|-------|-----------------|-------------------------------------------------|-------------|----------------------------------------------------------|------||-------|
| **مختبر/CI** | `3` | `2` | `2` | `2500` | `300` | سيتم إغلاق نافذة الكمون ونافذة الرحمة بسرعة مما يؤدي إلى انفجار عن بعد. قم بتكرار المحاولات بشكل أساسي لإظهار البيانات غير الصالحة بالإضافة إلى ذلك. |
| ** التدريج ** | `4` | `3` | `3` | `4000` | `600` | تعكس قيم الإنتاج كلها على حافة الهاوية للأقران المستكشفين. |
| **الكناري** | `6` | `3` | `3` | `5000` | `900` | تتوافق مع القيم الافتراضية ; حدد `telemetry_region` للسماح بتحريك لوحات المعلومات على حركة المرور الكناري. |
| **Disponibilité générale** | `None` (استخدم جميع المؤهلين) | `4` | `4` | `5000` | `900` | قم بزيادة تكرارات إعادة المحاولة والتحقق لامتصاص الأخطاء الانتقالية بالكامل مع الحفاظ على التحديد من خلال التدقيق. |- `scoreboard.weight_scale` متبقي على القيمة الافتراضية `10_000` إذا كان النظام يحتاج إلى دقة أخرى بالكامل. لا تتغير زيادة المستوى في ترتيب الموردين؛ هذا المنتج مجرد توزيع أرصدة كثيفة.
- عند مرور مرحلة إلى أخرى، استمر في حزمة JSON واستخدم `--scoreboard-out` حتى يتمكن مسار التدقيق من تسجيل إعدادات اللعبة الدقيقة.

## 2. نظافة لوحة النتائج

تجمع لوحة النتائج بين متطلبات البيان وإعلانات الموردين والقياس عن بعد.
الطليعة :1. **التحقق من صحة بيانات الاتصال.** تأكد من أن اللقطات تشير إلى
   تم التقاط `--telemetry-json` في نافذة النعمة التي تم تكوينها. المقبلات
   بالإضافة إلى العناصر القديمة التي `telemetry_grace_secs` تحدث مع `TelemetryStale { last_updated }`.
   Traitez cela comme un stop dur et rafraîchissez l’export de télémétrie avant de continuer.
2. **افحص أسباب الأهلية.** استمر في التحف عبر
   `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`. مدخل تشاك
   نقل كتلة `eligibility` مع السبب الدقيق للفحص. لا كونتورنيز
   لا تتسع لتسع أو إعلانات منتهية الصلاحية ; قم بتصحيح الحمولة بشكل كبير.
3. **ارجع إلى عربات الأوزان.** قارن البطل `normalised_weight` مع لا
   الافراج عن سابقة. الاختلافات > 10% تتوافق مع التغييرات
   تطوع للإعلانات أو للقياس عن بعد ويتم إرسالهم إلى مجلة النشر.
4. **أرشفة العناصر.** تكوين `scoreboard.persist_path` لكل تنفيذ
   قم بتحرير اللقطة النهائية للوحة النتائج. قم بإرفاق العنصر في ملف التحرير
   مع البيان وحزمة القياس عن بعد.
5. **أرسل العرض المسبق لموردي المزيج.** البيانات التعريفية لـ `scoreboard.json` _et_ le
   `summary.json` كاشف doivent المطابق `provider_count`،
   `gateway_provider_count` والاتيكيت المشتق `provider_mix` للكشافاتيمكن إثبات ذلك بشكل قوي إذا كان التنفيذ `direct-only` أو `gateway-only` أو `mixed`. ليه
   يلتقط بوابة doivent Rapporter `provider_count=0` بالإضافة إلى `provider_mix="gateway-only"`،
   بينما تتطلب عمليات التنفيذ المختلطة حسابات غير فارغة للمصدرين.
   `cargo xtask sorafs-adoption-check` يفرض هذه الأبطال (وأيضًا ما إذا كانت الحسابات/التسميات
   متباين)، حتى يتم تنفيذه طوال الوقت باستخدام `ci/check_sorafs_orchestrator_adoption.sh`
   أو برنامج الالتقاط الخاص بك لإنتاج حزمة الأدلة `adoption_report.json`.
   عند تضمين البوابات Torii، يجب الحفاظ على `gateway_manifest_id`/`gateway_manifest_cid`
   في البيانات الوصفية للوحة النتائج حتى تتمكن بوابة التبني من تصحيح المغلف
   تم التقاط هذا البيان بواسطة مزودي المزيج.

Pour des définitions de champs détaillées, voir
`crates/sorafs_car/src/scoreboard.rs` وهيكل السيرة الذاتية CLI المعروض على قدم المساواة
`sorafs_cli fetch --json-out`.

## مرجع الأعلام CLI et SDK

`sorafs_cli fetch` (عرض `crates/sorafs_car/src/bin/sorafs_cli.rs`) والمجمع
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) يشارك نفس الشيء
سطح تكوين الأوركسترا. استخدم الأعلام التالية عند ظهورها
التقاط preuves de déploiment أو لتجديد التركيبات القياسية :

مرجع مشاركة العلامات متعددة المصادر (gardez l'aide CLI et les docs syncronisées en
éditant Uniquement ce fichier):- `--max-peers=<count>` يحد من عدد الموردين المؤهلين الذين يجتازون مرشح لوحة النتائج. اترك الفيديو للبث من جميع الموردين المؤهلين، واضغط `1` بشكل فريد لممارسة المصدر الأحادي الاحتياطي بشكل اختياري. يعكس المفتاح `maxPeers` في SDK (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` ينقل الحد الأقصى لعدد المحاولات حسب القطعة المطبقة على `FetchOptions`. استخدم جدول بدء دليل الضبط للقيم الموصى بها؛ تتوافق عمليات تنفيذ CLI التي تجمع النصائح مع القيم الافتراضية لـ SDK لضمان التكافؤ.
- `--telemetry-region=<label>` يضبط السلسلة Prometheus `sorafs_orchestrator_*` (والمرحلات OTLP) مع تسمية المنطقة/البيئة التي تميز لوحات المعلومات معملًا ومسرحًا وكاناريًا وGA.
- `--telemetry-json=<path>` يقوم بإدخال اللقطة المرجعية على لوحة النتائج. استمر في استخدام JSON على طول لوحة النتائج حتى يتمكن المدققون من تجديد التنفيذ (ولكي يتمكن `cargo xtask sorafs-adoption-check --require-telemetry` من إثبات تدفق OTLP لتغذية الالتقاط).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) نشط خطافات جسر المراقبة. عندما يتم تحديد ذلك، يقوم المُنسق بتوزيع القطع عبر الوكيل Norito/Kaigi المحلي ليتمكن العملاء من التنقل وحراسة ذاكرات التخزين المؤقت وغرف Kaigi من خلال استعادة النماذج المماثلة لـ Rust.- `--scoreboard-out=<path>` (يحدث مع `--scoreboard-now=<unix_secs>`) يحتفظ باللقطة المؤهلة للمراجعين. قم بربط JSON دائمًا بعناصر القياس عن بعد والبيانات المرجعية في تذكرة الإصدار.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` appliquent des Adjustments déterministes au-dessus des métadonnées d'annonces. استخدم هذه العلامات الفريدة للتكرارات؛ إن تخفيضات الإنتاج يجب أن تمر عبر مصوغات الحكم بحيث لا ينطبق كل منها على نفس الحزمة السياسية.
- `--provider-metrics-out` / `--chunk-receipts-out` يحافظ على مقاييس الصحة من قبل المزود ومصادر القطع المرجعية من خلال قائمة التحقق من الطرح؛ قم بإرفاق قطعتين من القطع الأثرية عند إيداع إثبات التبني.

مثال (باستخدام التركيبة المنشورة):

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

تقوم SDK بتنفيذ التكوين نفسه عبر `SorafsGatewayFetchOptions` في
Client Rust (`crates/iroha/src/client.rs`)، الارتباطات JS
(`javascript/iroha_js/src/sorafs.js`) وSDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). Gardez ces helpers alignés
مع القيم الافتراضية لـ CLI حتى يتمكن المشغلون من نسخها
السياسة في الأتمتة بدون أرائك الترجمة المخصصة.

## 3. تعديل سياسة الجلب

`FetchOptions` يتحكم في المحاولات والتزامن والتحقق. شكرا لك
إعادة التنظيم :- **إعادة المحاولة:** زيادة `per_chunk_retry_limit` إلى `4` مع مرور الوقت
  قد يؤدي الاسترداد إلى إخفاء أخطاء الموردين. مفضل
  قم بمراقبة `4` كسطح ومراقب دوران الموردين من أجل
  فضح فناني الأداء.
- **Seuil d’échec :** `provider_failure_threshold` تحدد ما إذا كان المورد
  تم إلغاء تنشيطه لبقية الجلسة. قم بمحاذاة هذه القيمة في السياسة
  عمليات إعادة المحاولات : إن الميزانية الأدنى من عمليات إعادة المحاولات تجبر المُنسق على
  قم بإخراج نظير قبل أن لا يتم حذف جميع المحاولات.
- **التزامن:** اترك `global_parallel_limit` غير محدد (`None`) على الأقل
  qu’un بيئة محددة لا يمكن أن تشبع الشواطئ المعلنة. لورسك
  حدد، وتأكد من أن القيمة هي ≥ جزء من ميزانيات التدفقات
  الموردون قادرون على تجنب المجاعة.
- **تبديل التحقق:** `verify_lengths` و`verify_digests` يقومان بالاستراحة
  النشاط في الإنتاج. إنه يضمن التحديد عند الأسطول
  مزيج من الموردين هم نشطون؛ لا يتم تعطيلها في البيئات
  من العزلة الضبابية.

## 4. النقل والتدريج مجهول الهوية

استخدم الأبطال `rollout_phase` و`anonymity_policy` و`transport_policy` من أجل
يمثل موقف السرية :- تفضيل `rollout_phase="snnet-5"` واترك سياسة إخفاء الهوية افتراضيًا
  التالي SNNet-5. قم بالاستبدال عبر فريدة `anonymity_policy_override`
  عندما يتم توقيع توجيهات الحوكمة.
- Gardez `transport_policy="soranet-first"` comme base tant que SNNet-4/5/5a/5b/6a/7/8/12/13 sont 🈺
  (التقرير `roadmap.md`). استخدم `transport_policy="direct-only"` الفريد من نوعه
  قم بتخفيض مستوى المستندات أو تمارين المطابقة، واحضر المراجعة
  غطاء PQ avant de promouvoir `transport_policy="soranet-strict"` — المستوى ce
  échouera fastement si seuls des relais classics subsistent.
- `write_mode="pq-only"` لا ينبغي تطبيقه عند كل كتاب كتابة
  (SDK، المنسق، أدوات الحوكمة) يمكن أن تلبي متطلبات PQ. ديورانت
  عمليات الإطلاق، تحافظ على `write_mode="allow-downgrade"` للاستجابة للاستجابات العاجلة
  يمكنك الضغط على الطرق المباشرة حتى يشير جهاز القياس عن بعد إلى
  التدهور.
- يتم تطبيق اختيار الحراس وإعداد الدوائر على السطح
  ذخيرة SoraNet. قم بإدخال علامة اللقطة `relay_directory` وآخرون
  استمر في الاحتفاظ بذاكرة التخزين المؤقت `guard_set` حتى تبقى عملية التدوير في النافذة
  مكان الاحتجاز. تشغيل ذاكرة التخزين المؤقت المسجلة حسب `sorafs_cli fetch`
  جزء من دليل الطرح.

## 5. خطافات خفض المستوى والمطابقة

هناك نظامان من أنظمة المنسق يساعدان على احترام السياسة بلا سياسة
مانويل التدخل :- **إصلاح التخفيضات** (`downgrade_remediation`): مراقبة الأحداث
  `handshake_downgrade_total`، وبعد ذلك تم تكوين `threshold` في
  `window_secs`، فرض الوكيل المحلي على `target_mode` (افتراضيًا لبيانات التعريف فقط).
  الحفاظ على القيم الافتراضية (`threshold=3`، `window=300`، `cooldown=900`) أيضًا
  إذا تم إنشاء مخطط آخر بعد الوفاة. Documentez toute override dans le
  مجلة الطرح وتأكد من أن لوحات المعلومات ستتبع
  `sorafs_proxy_downgrade_state`.
- **سياسة المطابقة** (`compliance`): إقتطاعات القضاء والقانون
  تم إثبات ذلك من خلال قوائم إلغاء الاشتراك التي تديرها الحوكمة. غير متكامل
  تجاوزات مخصصة في حزمة التكوين ؛ اطلب قطعة واحدة
  قم بتوقيع اليوم على `governance/compliance/soranet_opt_outs.json` وإعادة النشر
  تم إنشاء JSON.

من أجل النظامين، استمر في حزمة التكوين الناتجة وقم بإدراجها
تساعد هذه الخطوات في الإصدار على تمكين المستمعين من استعادة الأساسيات.

## 6. القياس عن بعد ولوحات المعلومات

قبل توسيع عملية الطرح، تأكد من أن الإشارات اللاحقة نشطة في
سلك البيئة :- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  يجب أن تكون على وشك الصفر بعد نهاية الكناري.
- `sorafs_orchestrator_retries_total` وآخرون
  `sorafs_orchestrator_retry_ratio` — مثبت doivent se sous 10% قلادة le
  الكناري والاستراحة بنسبة 5% بعد GA.
- `sorafs_orchestrator_policy_events_total` — التحقق من أن شريط التشغيل قيد الانتظار
  نشط (التسمية `stage`) وقم بتسجيل الملفات البنية عبر `outcome`.
-`sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — العرض التالي لـ PQ Face aux
  اهتمام بالسياسة.
- Cibles de log `telemetry::sorafs.fetch.*` — يجب أن تكون مبعوثة إلى المقاول
  مشاركة السجلات مع الأبحاث المحفوظة لـ `status=failed`.

قم بشحن لوحة القيادة Grafana canonique depuis
`dashboards/grafana/sorafs_fetch_observability.json` (يتم تصديره من خلال الباب الخلفي
**SoraFS → Fetch Observability**) لتحديد محددات المنطقة/البيان، الخريطة الحرارية
المحاولات التي يقوم بها المورد، والمخططات البيانية لزمن وصول القطع والحسابات
يتوافق الحظر مع ما تقوم به SRE من فحص عمليات الاحتراق. قم بربط القواعد
مدير التنبيهات في `dashboards/alerts/sorafs_fetch_rules.yml` والتحقق من بناء الجملة
Prometheus مع `scripts/telemetry/test_sorafs_fetch_alerts.sh` (المساعد في التنفيذ
`promtool test rules` محليًا عبر Docker). عمليات التسليم للتنبيه تتطلب ذلك
نفس كتلة التوجيه التي يطبعها البرنامج النصي حتى يتمكن المشغلون من الانضمام
دليل على تذكرة الطرح.

### سير عمل النسخ عن بعديتطلب عنصر خريطة الطريق **SF-6e** نسخًا عن بعد لمدة 30 يومًا قبل
basculer l’orchesstrateur multi-source vers ses valeurs GA. استخدم البرامج النصية
مرجعية لالتقاط مجموعة من القطع الأثرية القابلة لإعادة الإنتاج كل يوم
نافذة :

1. قم بتنفيذ `ci/check_sorafs_orchestrator_adoption.sh` باستخدام متغيرات النسخ
   تعريف. مثال :

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```لو المساعد ريجو `fixtures/sorafs_orchestrator/multi_peer_parity_v1`,
   اكتب `scoreboard.json`، `summary.json`، `provider_metrics.json`،
   `chunk_receipts.json` و`adoption_report.json` سو
   `artifacts/sorafs_orchestrator/<timestamp>/`، وفرض رقم أدنى من de
   الموردون المؤهلون عبر `cargo xtask sorafs-adoption-check`.
2. عندما تكون متغيرات النسخ موجودة، يتم تشغيل البرنامج النصي أيضًا
   `burn_in_note.json`، يلتقط الملصق، فهرس اليوم، معرف البيان،
   مصدر عن بعد وملخصات العناصر. Joignez ce JSON au Journal
   de rollout afin qu’il soit clair quelle Capture a satisfait chaque jour de la
   نافذة لمدة 30 يومًا.
3. قم باستيراد اللوحة Grafana الحالية (`dashboards/grafana/sorafs_fetch_observability.json`)
   في مساحة العمل التدريج/الإنتاج، تم وضع ملصق الاحتراق عليه
   وتحقق من أن كل لوحة تعرض الأيقونات للبيان/المنطقة المحددة.
4. قم بتنفيذ `scripts/telemetry/test_sorafs_fetch_alerts.sh` (أو `promtool test rules …`)
   كل مرة يتغير فيها `dashboards/alerts/sorafs_fetch_rules.yml`، لفتح المستند
   يتوافق توجيه التنبيهات مع المقاييس المصدرة أثناء الاحتراق.
5. أرشفة لقطة لوحة القيادة، وفرز اختبار التنبيهات، وقائمة انتظار السجلات
   الأبحاث `telemetry::sorafs.fetch.*` مع المصنوعات اليدوية للمدير
   يمكن للحوكمة أن تجد الأدلة الحية بدون خارج مقاييس الأنظمة.

## 7. قائمة التحقق من الطرح1. قم بإعادة إنشاء لوحات النتائج في CI باستخدام مرشح التكوين والتقاطها
   القطع الأثرية التحكم في الإصدار.
2. قم بتنفيذ عملية تحديد التركيبات في كل بيئة (المختبر، التدريج،
   الكناري والإنتاج) وإرفاق القطع الأثرية `--scoreboard-out` و`--json-out`
   في سجل الطرح.
3. قم باستعراض لوحات المعلومات عن بعد مع مهندس التكنولوجيا، أون
   التحقق من أن جميع المقاييس التي تستخدمها موجودة على الترنيمات.
4. قم بتسجيل نظام التكوين النهائي (يتوفر عبر `iroha_config`) ثم
   الالتزام ببوابة سجل الحوكمة المستخدمة للإعلانات والمطابقة.
5. قم بتحديث متتبع الطرح وأبلغ معدات SDK الجديدة
   من المفترض أن تكون عمليات التكامل العميلة متسقة.

سيستمر هذا الدليل في الحفاظ على عمليات نشر المحددات والمنسقة
المواد القابلة للتدقيق، جميعها توفر حلقات رجعية واضحة لضبط
ميزانيات إعادة المحاولة، وقدرة الموردين، وموقف السرية.