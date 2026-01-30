---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/orchestrator-tuning.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: orchestrator-tuning
title: إطلاق وضبط المُنسِّق
sidebar_label: ضبط المُنسِّق
description: قيم افتراضية عملية وإرشادات ضبط ونقاط تدقيق لمواءمة المُنسِّق متعدد المصادر مع GA.
---

:::note المصدر المعتمد
يعكس `docs/source/sorafs/developer/orchestrator_tuning.md`. حافظ على تطابق النسختين إلى أن تُحال مجموعة التوثيق القديمة للتقاعد.
:::

# دليل إطلاق وضبط المُنسِّق

يبني هذا الدليل على [مرجع الإعداد](orchestrator-config.md) و
[دليل إطلاق متعدد المصادر](multi-source-rollout.md). يشرح
كيفية ضبط المُنسِّق لكل مرحلة إطلاق، وكيفية قراءة آثار لوحة النتائج، وما هي
إشارات التليمترية المطلوبة قبل توسيع الحركة. طبّق التوصيات بشكل متسق عبر
CLI وSDKs والأتمتة حتى تتبع كل عقدة سياسة جلب حتمية واحدة.

## 1. مجموعات المعلمات الأساسية

ابدأ من قالب إعداد مشترك واضبط مجموعة صغيرة من المقابض مع تقدم الإطلاق. يلتقط
الجدول أدناه القيم الموصى بها لأكثر المراحل شيوعًا؛ وما لم يُذكر يعود إلى
الافتراضات في `OrchestratorConfig::default()` و`FetchOptions::default()`.

| المرحلة | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | الملاحظات |
|---------|-----------------|-------------------------------|------------------------------------|-----------------------------|------------------------------------|-----------|
| **المختبر / CI** | `3` | `2` | `2` | `2500` | `300` | سقف كمون ضيق ونافذة سماح قصيرة يكشفان التليمترية المزعجة سريعًا. خفّض المحاولات لكشف المانيفستات غير الصالحة مبكرًا. |
| **Staging** | `4` | `3` | `3` | `4000` | `600` | يعكس قيم الإنتاج مع ترك فسحة للنظراء الاستكشافية. |
| **الكناري** | `6` | `3` | `3` | `5000` | `900` | يطابق القيم الافتراضية؛ اضبط `telemetry_region` حتى تتمكن لوحات المتابعة من فصل حركة الكناري. |
| **الإتاحة العامة (GA)** | `None` (استخدم جميع المؤهلين) | `4` | `4` | `5000` | `900` | ارفع حدود إعادة المحاولة والفشل لامتصاص الأعطال العابرة مع استمرار التدقيق في ضمان الحتمية. |

- يبقى `scoreboard.weight_scale` على القيمة الافتراضية `10_000` ما لم يتطلب نظام لاحق دقة عددية مختلفة. زيادة المقياس لا تغيّر ترتيب المزوّدين؛ بل تنتج توزيعًا أدق للرصيد.
- عند الانتقال بين المراحل، احفظ حزمة JSON واستخدم `--scoreboard-out` لتوثيق مجموعة المعلمات الدقيقة في مسار التدقيق.

## 2. نظافة لوحة النتائج

تجمع لوحة النتائج بين متطلبات المانيفست وإعلانات المزوّدين والتليمترية.
قبل المضي قدمًا:

1. **تحقّق من حداثة التليمترية.** تأكد من أن اللقطات المشار إليها عبر
   `--telemetry-json` التقطت ضمن نافذة السماح. الإدخالات الأقدم من
   `telemetry_grace_secs` تفشل مع `TelemetryStale { last_updated }`.
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
   المطابق حقول `provider_count` و`gateway_provider_count` والتسمية `provider_mix` لإثبات
   ما إذا كان التشغيل `direct-only` أو `gateway-only` أو `mixed`. يجب أن تُظهر لقطات
   البوابة `provider_count=0` و`provider_mix="gateway-only"`، بينما تتطلب التشغيلات المختلطة
   أعدادًا غير صفرية للمصدرين. يفرض `cargo xtask sorafs-adoption-check` هذه الحقول
   (ويفشل إذا لم تتطابق العدادات/التسميات)، لذا شغّله دائمًا مع
   `ci/check_sorafs_orchestrator_adoption.sh` أو نص الالتقاط لديك لإنتاج حزمة الأدلة
   `adoption_report.json`. عند إشراك بوابات Torii، احفظ `gateway_manifest_id`/
   `gateway_manifest_cid` ضمن ميتاداتا لوحة النتائج حتى يتمكن حاجز التبني من
   ربط مغلف المانيفست بخليط المزوّدين الملتقط.

للتعريفات التفصيلية للحقول راجع
`crates/sorafs_car/src/scoreboard.rs` وبنية ملخص CLI التي يخرجها
`sorafs_cli fetch --json-out`.

## مرجع أعلام CLI وSDK

`sorafs_cli fetch` (راجع `crates/sorafs_car/src/bin/sorafs_cli.rs`) وواجهة
`iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) تشترك في
سطح إعداد المُنسِّق نفسه. استخدم الأعلام التالية عند التقاط أدلة الإطلاق أو
إعادة تشغيل الـ fixtures القياسية:

مرجع أعلام متعدد المصادر (أبقِ مساعدة CLI والوثائق متزامنة بتعديل هذا الملف فقط):

- `--max-peers=<count>` يحدّ عدد المزوّدين المؤهلين الذين ينجون من فلتر لوحة النتائج. اتركه فارغًا لتدفق جميع المزوّدين المؤهلين، واضبطه على `1` فقط عند اختبار الرجوع لمصدر واحد عمدًا. يعكس مفتاح `maxPeers` في SDKs (`SorafsGatewayFetchOptions.maxPeers`, `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` يمرّر حد إعادة المحاولة لكل chunk الذي يطبقه `FetchOptions`. استخدم جدول الإطلاق في دليل الضبط للقيم الموصى بها؛ يجب أن تطابق تشغيلات CLI التي تجمع الأدلة افتراضات SDK للمحافظة على التوافق.
- `--telemetry-region=<label>` يوسم سلاسل Prometheus `sorafs_orchestrator_*` (ومرحّلات OTLP) بوسم المنطقة/البيئة حتى تتمكن لوحات المتابعة من فصل حركة المختبر وstaging والكناري وGA.
- `--telemetry-json=<path>` يحقن لقطة التليمترية المشار إليها في لوحة النتائج. احفظ JSON بجوار لوحة النتائج كي يتمكن المدققون من إعادة التشغيل (ولكي يثبت `cargo xtask sorafs-adoption-check --require-telemetry` أي تدفق OTLP غذّى الالتقاط).
- `--local-proxy-*` (`--local-proxy-mode`, `--local-proxy-norito-spool`, `--local-proxy-kaigi-spool`, `--local-proxy-kaigi-policy`) يفعّل hooks الخاصة بالجسر. عند ضبطها، يمرّر المُنسِّق الـ chunks عبر Proxy Norito/Kaigi المحلي حتى تتلقى عملاء المتصفح وguard caches وغرف Kaigi نفس الإيصالات التي يصدرها Rust.
- `--scoreboard-out=<path>` (اختياريًا مع `--scoreboard-now=<unix_secs>`) يحفظ لقطة الأهلية للمدققين. اربط دائمًا الـ JSON المحفوظ بآثار التليمترية والمانيفست المشار إليها في تذكرة الإصدار.
- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` يطبّقان تعديلات حتمية فوق ميتاداتا الإعلانات. استخدم هذه الأعلام للتجارب فقط؛ يجب أن تمر تخفيضات الإنتاج عبر artefacts الحوكمة كي تطبّق كل عقدة نفس حزمة السياسة.
- `--provider-metrics-out` / `--chunk-receipts-out` يحتفظان بمقاييس صحة المزوّدين وإيصالات الـ chunks المرجعية في قائمة التحقق؛ أرفق كلا الأثرين عند تقديم أدلة التبني.

مثال (باستخدام الـ fixture المنشور):

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

تستهلك SDKs نفس الإعداد عبر `SorafsGatewayFetchOptions` في عميل Rust
(`crates/iroha/src/client.rs`) وروابط JS
(`javascript/iroha_js/src/sorafs.js`) وSDK Swift
(`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`). حافظ على تطابق هذه
المساعدات مع افتراضات CLI حتى يتمكن المشغلون من نسخ السياسات إلى الأتمتة دون
طبقات ترجمة مخصصة.

## 3. ضبط سياسة الجلب

يتحكم `FetchOptions` في إعادة المحاولة والتوازي والتحقق. عند الضبط:

- **إعادة المحاولة:** رفع `per_chunk_retry_limit` فوق `4` يزيد زمن الاستعادة لكنه
  قد يخفي أعطال المزوّدين. يفضّل إبقاء السقف عند `4` والاعتماد على تدوير المزوّدين
  لإظهار الأداء الضعيف.
- **عتبة الفشل:** تحدد `provider_failure_threshold` متى يتم تعطيل المزوّد لبقية الجلسة.
  يجب أن تتوافق هذه القيمة مع سياسة إعادة المحاولة: عتبة أقل من ميزانية المحاولة
  تُخرج المزوّد قبل استنفاد جميع المحاولات.
- **التوازي:** اترك `global_parallel_limit` غير مضبوط (`None`) ما لم تكن بيئة محددة
  غير قادرة على تشبع النطاقات المُعلنة. عند ضبطه، تأكد أن القيمة ≤ مجموع ميزانيات
  التدفق للمزوّدين لتجنب التجويع.
- **مفاتيح التحقق:** يجب إبقاء `verify_lengths` و`verify_digests` مفعّلين في الإنتاج.
  فهي تضمن الحتمية عندما تعمل أساطيل مزوّدين مختلطة؛ عطّلها فقط في بيئات fuzzing المعزولة.

## 4. مراحل النقل والخصوصية

استخدم حقول `rollout_phase` و`anonymity_policy` و`transport_policy` لتمثيل وضع الخصوصية:

- فضّل `rollout_phase="snnet-5"` ودع سياسة الخصوصية الافتراضية تتبع معالم SNNet-5. استعمل
  `anonymity_policy_override` فقط عندما تصدر الحوكمة توجيهًا موقّعًا.
- أبقِ `transport_policy="soranet-first"` كخيار أساس بينما SNNet-4/5/5a/5b/6a/7/8/12/13 في 🈺
  (راجع `roadmap.md`). استخدم `transport_policy="direct-only"` فقط لخفض موثق أو تدريبات امتثال،
  وانتظر مراجعة تغطية PQ قبل الترقية إلى `transport_policy="soranet-strict"`—هذا المستوى يفشل سريعًا
  إذا بقيت ترحيلات كلاسيكية فقط.
- لا تفرض `write_mode="pq-only"` إلا عندما تستطيع كل مسارات الكتابة (SDK، المُنسِّق، أدوات الحوكمة)
  تلبية متطلبات PQ. أثناء الإطلاق، أبقِ `write_mode="allow-downgrade"` حتى يمكن لخطط الطوارئ
  الاعتماد على المسارات المباشرة بينما تُعلِّم التليمترية حالات التخفيض.
- يعتمد اختيار الحراس وتجهيز الدوائر على دليل SoraNet. زوّد لقطة `relay_directory` الموقعة
  واحفظ `guard_set` cache كي يبقى تغيّر الحراس ضمن نافذة الاحتفاظ المتفق عليها. بصمة الكاش
  التي يسجلها `sorafs_cli fetch` تُعد جزءًا من أدلة الإطلاق.

## 5. hooks التخفيض والامتثال

يساعد نظامان فرعيان في المُنسِّق على فرض السياسة دون تدخل يدوي:

- **معالجة التخفيض** (`downgrade_remediation`): تراقب أحداث `handshake_downgrade_total`، وبعد تجاوز
  `threshold` داخل `window_secs` تُجبر الـ proxy المحلي على `target_mode` (افتراضيًا metadata-only).
  احتفظ بالقيم الافتراضية (`threshold=3`, `window=300`, `cooldown=900`) ما لم تظهر مراجعات الحوادث
  نمطًا مختلفًا. وثّق أي override في سجل الإطلاق وتأكد أن لوحات المتابعة تراقب
  `sorafs_proxy_downgrade_state`.
- **سياسة الامتثال** (`compliance`): تمر استثناءات الولاية القضائية والمانيفست عبر قوائم opt‑out التي
  تديرها الحوكمة. لا تُدرِج overrides عشوائية في حزمة الإعداد؛ اطلب تحديثًا موقعًا
  لـ `governance/compliance/soranet_opt_outs.json` وأعد نشر JSON الناتج.

بالنسبة لكلا النظامين، احفظ حزمة الإعداد الناتجة وأرفقها بأدلة الإصدار حتى يتمكن
المدققون من تتبع كيفية تفعيل التخفيضات.

## 6. التليمترية ولوحات المتابعة

قبل توسيع الإطلاق، تأكد أن الإشارات التالية نشطة في البيئة المستهدفة:

- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  يجب أن تكون صفرًا بعد اكتمال الكناري.
- `sorafs_orchestrator_retries_total` و
  `sorafs_orchestrator_retry_ratio` — يجب أن تستقر تحت 10% أثناء الكناري وتبقى
  تحت 5% بعد GA.
- `sorafs_orchestrator_policy_events_total` — يتحقق من أن مرحلة الإطلاق المطلوبة
  فعالة (label `stage`) ويسجل حالات brownout عبر `outcome`.
- `sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — تتبع عرض ترحيلات PQ مقابل توقعات السياسة.
- أهداف سجلات `telemetry::sorafs.fetch.*` — يجب بثها إلى مجمّع السجلات المشترك مع
  عمليات بحث محفوظة لـ `status=failed`.

حمّل لوحة Grafana القياسية من
`dashboards/grafana/sorafs_fetch_observability.json` (المصدَّرة في البوابة تحت
**SoraFS → Fetch Observability**) لضمان تطابق محددات المنطقة/المانيفست وخريطة
محاولات المزوّدين والهيستوغرامات والعدادات مع ما يراجعه فريق SRE أثناء burn-in.
اربط قواعد Alertmanager في `dashboards/alerts/sorafs_fetch_rules.yml` وتحقق من
صياغة Prometheus عبر `scripts/telemetry/test_sorafs_fetch_alerts.sh` (يشغّل المساعد
`promtool test rules` محليًا أو في Docker). تتطلب عمليات تسليم التنبيه نفس كتلة
التوجيه التي يطبعها السكربت حتى يتمكن المشغلون من إرفاق الأدلة بتذكرة الإطلاق.

### تدفق burn-in للتليمترية

يتطلب بند خارطة الطريق **SF-6e** فترة burn-in للتليمترية لمدة 30 يومًا قبل تحويل
المُنسِّق متعدد المصادر إلى افتراضاته GA. استخدم سكربتات المستودع لالتقاط حزمة
آثار قابلة لإعادة الإنتاج لكل يوم في النافذة:

1. شغّل `ci/check_sorafs_orchestrator_adoption.sh` مع ضبط متغيرات burn-in. مثال:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```

   يعيد المساعد تشغيل `fixtures/sorafs_orchestrator/multi_peer_parity_v1`، ويكتب
   `scoreboard.json` و`summary.json` و`provider_metrics.json` و`chunk_receipts.json`
   و`adoption_report.json` تحت `artifacts/sorafs_orchestrator/<timestamp>/`،
   ويفرض حدًا أدنى من المزوّدين المؤهلين عبر `cargo xtask sorafs-adoption-check`.
2. عند توفر متغيرات burn-in، يصدر السكربت أيضًا `burn_in_note.json` لتوثيق الوسم،
   وفهرس اليوم، ومعرّف المانيفست، ومصدر التليمترية، وملخصات الآثار. أرفق هذا JSON
   بسجل الإطلاق لتوضيح أي لقطة استوفت كل يوم ضمن نافذة 30 يومًا.
3. استورد لوحة Grafana المحدثة (`dashboards/grafana/sorafs_fetch_observability.json`)
   في مساحة staging/production، وضع وسم burn-in، وتأكد من أن كل لوحة تعرض عينات
   للمانيفست/المنطقة قيد الاختبار.
4. شغّل `scripts/telemetry/test_sorafs_fetch_alerts.sh` (أو `promtool test rules …`)
   عند تغيير `dashboards/alerts/sorafs_fetch_rules.yml` لتوثيق أن توجيه التنبيهات
   يطابق المقاييس المصدرة أثناء burn-in.
5. أرشف لقطة لوحة المتابعة، ومخرجات اختبار التنبيهات، وذيل السجلات لعمليات البحث
   `telemetry::sorafs.fetch.*` مع آثار المُنسِّق حتى تتمكن الحوكمة من إعادة إنتاج
   الأدلة دون الاعتماد على أنظمة حية.

## 7. قائمة تحقق الإطلاق

1. أعد توليد scoreboards في CI باستخدام الإعداد المرشح والتقط الآثار تحت التحكم بالإصدارات.
2. شغّل جلب fixtures الحتمي في كل بيئة (المختبر، staging، الكناري، الإنتاج) وأرفق
   آثار `--scoreboard-out` و`--json-out` بسجل الإطلاق.
3. راجع لوحات التليمترية مع مهندس المناوبة، وتأكد أن كل المقاييس أعلاه لديها عينات حية.
4. سجّل مسار الإعداد النهائي (عادة عبر `iroha_config`) وcommit git لسجل الحوكمة المستخدم
   للإعلانات والامتثال.
5. حدّث متعقّب الإطلاق وأبلغ فرق SDK بالافتراضات الجديدة حتى تبقى تكاملات العملاء متسقة.

اتباع هذا الدليل يحافظ على إطلاقات المُنسِّق حتمية وقابلة للتدقيق، مع توفير حلقات
تغذية راجعة واضحة لضبط ميزانيات إعادة المحاولة، وسعة المزوّدين، ووضع الخصوصية.
