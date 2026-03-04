---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-tuning.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: ضبط الأوركسترا
العنوان: آركسٹریٹر رول آؤٹ و ٹیوننگ
Sidebar_label: آرکستریٹر ٹیوننگ
الوصف: العديد من صانعي الأخبار الذين يقدمون GA T لمجموعات متنوعة من العمل، والتقنيات الحديثة، والعديد من النقاط.
---

:::ملاحظة مستند ماخذ
هذه هي الصفحة `docs/source/sorafs/developer/orchestrator_tuning.md`. عندما لا يتم إجراء أي تلاعب بالمعلومات، فلا داعي للقلق بشأن ما إذا كان الأمر كذلك.
:::

# آركستري رولر و ٹيوننغ رول

هذه هي [النسخة الخامسة](orchestrator-config.md) و
[ملٹي رول سور آؤٹ رن بک](multi-source-rollout.md) للبن. إنها واضحة وضوح الشمس
ما هو الدور الرئيسي الذي يخوضه الكاتب في كل جولة، لوحة النتائج؟
الأفكار الرائعة التي تبعث على الدهشة، والأفضل من ذلك كله هو عدد الإشارات
لا بد منه. تعد نوافذ CLI وSDKs وAutomation إحدى أكثر الابتكارات نجاحًا في عالم الأعمال
هناك واحدة جديدة من نوعها تجلب باليس للعمل.

## 1. قاعدة برمائية

هذا المنتج عبارة عن نقرة دوارة ولفافة واحدة بالإضافة إلى المقابض المختارة
هذا هو جوهر اللعبة. لا يوجد جدول عام للمراحل التي ستحدد مقدار دكاهاتا؛
مقدار الدرج غير موجود، و`OrchestratorConfig::default()` و`FetchOptions::default()`
هذا هو المكان الذي يتم فيه إجراء الاتصالات الهاتفية.| رحلةہ | `max_providers` | `fetch.per_chunk_retry_limit` | `fetch.provider_failure_threshold` | `scoreboard.latency_cap_ms` | `scoreboard.telemetry_grace_secs` | أخبار |
|-------|-----------------|-------------------------------------------------|-------------|----------------------------------------------------------|------||-------|
| **مختبر/CI** | `3` | `2` | `2` | `2500` | `300` | الحد الأقصى لزمن الاستجابة ونافذة سماح صغيرة تظهر فجأة على الفور. تظهر إعادة المحاولة خطأً فادحًا. |
| ** التدريج ** | `4` | `3` | `3` | `4000` | `600` | لقد ساعدنا مشروع منع الاختلاسات والأقران الاستكشافيين في القيام بذلك. |
| **كناري** | `6` | `3` | `3` | `5000` | `900` | وفقًا لذلك؛ `telemetry_region` بطاقة الائتمان الخاصة بك هي كل ما تحتاجه. |
| **التوفر العام (GA)** | `None` (تمام الاستخدام المؤهل کریں) | `4` | `4` | `5000` | `900` | تتضمن عارضات الجذب إعادة المحاولة وعتبات الفشل أيضًا من خلال اتخاذ إجراءات وقائية. |- `scoreboard.weight_scale` لا يلزم `10_000` إعادة النظر في نظام المصب ودقة العدد الصحيح. لا يوجد أي بدل من مزود الخدمة؛ لقد تم تخصيص عدد كبير من توزيع الائتمان في بنتا.
- تستخدم مراحل انتقال الجلد إلى حزمة JSON المحفوظة و`--scoreboard-out` استخدام أداة تسجيل الفيديو الصحيحة.

## 2. لوحة النتائج رائعة

بيان لوحة النتائج هو الضروريات، وإعلانات المزود، والبيانات التي توفرها هذه الأداة.
ما هي الخطوات التالية:1. **مجلة إبداعية حديثة.** تم تحويل لقطات `--telemetry-json` إلى لقطات
   تم إرسال نافذة سماح إلى اندر ريكارڈ. `telemetry_grace_secs` إدخالات سے پرانی
   `TelemetryStale { last_updated }` تم تسجيله مرة أخرى. إنه أمر صعب التوقف و
   لم يعد هناك أي شيء على الإطلاق في لعبة "يليمايت سبورتس".
2. **مفاضلة الأهلية.** `--scoreboard-out=/var/lib/sorafs/scoreboards/preflight.json`
   هذا ما حدث في مقالة رائعة. لقد تم إدخال `eligibility` باللون الأسود
   حق وباتاتا. عدم تطابق القدرات أو الإعلانات منتهية الصلاحية التي لم يتم نشرها مؤخرًا؛
   الحمولة المنبع صحيحة.
3. **الوزن المسموح به.** `normalised_weight` يتم تحريره بشكل دقيق.
   10% ستساعدك الكثير من التحسينات في الإعلان/القياس عن بعد على المزيد من التقدم
   وسجل الطرح لا يزال جديدًا.
4. **مقالات كريك الفنية.** `scoreboard.persist_path` تم تطوير لعبة كريت سيت
   لقطة من لوحة نتائج فينل محفوظ ہو۔ لقد أطلق سراحه سجلًا واضحًا
   حزمة القياس عن بعد مستمرة.
5. **مزيج الموفر لملفات التسجيل.** `scoreboard.json` البيانات التعريفية والمتعلقات
   `summary.json` `provider_count`، `gateway_provider_count` و`provider_mix`
   يجب أن تحصل ليبل على جائزتك النهائية من السجل الثابت `direct-only`, `gateway-only`
   أو `mixed` ھا۔ تلتقط البوابة `provider_count=0` و`provider_mix="gateway-only"`
   هناك شيء واحد، حيث أن التحاليل المختلطة التي لا تحتوي على أي صفر تعتبر ضرورية.`cargo xtask sorafs-adoption-check` ماكينة صنع الفشار (وعدم تطابق ماكينة صنع الفشار)،
   هذا هو `ci/check_sorafs_orchestrator_adoption.sh` أو التقط البرنامج النصي تلقائيًا
   قم بتجديد حزمة الأدلة `adoption_report.json`. تتضمن بوابات J18NT00000007X ما تحتاجه
   يمكن اعتماد `gateway_manifest_id`/`gateway_manifest_cid` والبيانات التعريفية للوحة النتائج
   مغلف بيان البوابة الذي تم التقاطه مزيج الموفر سے جوڑ سکے.

قم بقراءة هذه التفاصيل بالتفصيل
`crates/sorafs_car/src/scoreboard.rs` وهيكل ملخص CLI (جو `sorafs_cli fetch --json-out`
سے نكلت ہے) دیكھيں.

## CLI وSDK فليج ريفيرنس

`sorafs_cli fetch` (ديكا `crates/sorafs_car/src/bin/sorafs_cli.rs`) و
غلاف `iroha_cli app sorafs fetch` (`crates/iroha_cli/src/commands/sorafs.rs`) موجود
سطح تكوين المنسق. الأدلة التمهيدية أو الكنسي
إعادة تشغيل المباريات باستخدام الأعلام أو:

مرجع إشارة مشترك متعدد المصادر (تستخدم مساعدة CLI والمستندات في مزامنة المزامنة):- `--max-peers=<count>` مقدمو الخدمة المؤهلون لديهم نطاق محدود من لوحة النتائج. يقوم كل مقدمي الخدمات المؤهلين ببث البث الفارغ، و`1` يستغلون فقط مصدرًا احتياطيًا واحدًا. يتم استخدام مقبض SDK `maxPeers` (`SorafsGatewayFetchOptions.maxPeers`، `SorafsGatewayFetchOptions.max_peers`).
- `--retry-budget=<count>` `FetchOptions` الحد المسموح به لإعادة المحاولة لكل قطعة. ابحث عن مقدار الكمية التي ستستخدمها في جدول طرح الاستخدام؛ مجموعة الأدلة التي تم جمعها من CLI هي افتراضات SDK الافتراضية.
- `--telemetry-region=<label>` Prometheus `sorafs_orchestrator_*` سيريز (ومرحلات OTLP) على مستوى المنطقة/البيئة ومختبر بور، والتدريج، والكناري، وGA .
- لوحة النتائج `--telemetry-json=<path>` لحقن اللقطة المشار إليها کرتا ہے۔ JSON هي لوحة النتائج الرئيسية التي تسمح للاعبين بإعادة تشغيل كرة السلة (و`cargo xtask sorafs-adoption-check --require-telemetry` وهي عبارة عن بطاقة ثابتة تسمح بالتقاط دفق OTLP وتغذية هذه اللعبة).
- `--local-proxy-*` (`--local-proxy-mode`، `--local-proxy-norito-spool`، `--local-proxy-kaigi-spool`، `--local-proxy-kaigi-policy`) خطافات مراقب الجسر التي يتم تفعيلها. يحتوي الموقع على وكيل منسق Norito/Kaigi وهو عبارة عن قطع من دفق الكرتا وعملاء المتصفح ومخبأ الحراسة وغرف Kaigi وإيصالات متعددة تنبعث منها الصدأ.
- `--scoreboard-out=<path>` (اختیاری `--scoreboard-now=<unix_secs>` کے ساتھ) لقطة الأهلية محفوظ کرتا ہے۔ يحتوي محفوظات JSON في تذكرة الإصدار على قياسات مرجعية عن بعد وبيانات مصطنعة منذ ذلك الحين.- `--deny-provider name=ALIAS` / `--boost-provider name=ALIAS:delta` البيانات التعريفية للإعلان هي أول حتمية للقرارات. يتم استخدام الأعلام في التدريبات؛ لقد أدى التخفيضات الترويجية لأدوات الحوكمة إلى إحداث تغيير جديد وحزمة السياسات الناجحة.
- `--provider-metrics-out` / `--chunk-receipts-out` المقاييس الصحية لكل مزود وإيصالات القطع محفوظات کرتے ہیں؛ أدلة التبني التي تم جمعها من قبل القطع الأثرية التي لا بد من تضمينها.

مثال (شائع شدہ تركيبات استخدام کرتے ہوئے):

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

تكوين SDKs المتكامل `SorafsGatewayFetchOptions` عميل Rust
(`crates/iroha/src/client.rs`)، روابط JS (`javascript/iroha_js/src/sorafs.js`) و
يتم استخدام Swift SDK (`IrohaSwift/Sources/IrohaSwift/SorafsOptions.swift`).
تعمل الأدوات المساعدة في إعدادات CLI الافتراضية على تثبيت قفل الخطوة وأتمتة أجهزة الكمبيوتر
باليسكو هي ترجمة مخصصة لقصائد تزلج.

## 3. جلب باليس ٹیوننج

`FetchOptions` إعادة المحاولة والتزامن والتحقق من السيطرة. ٹیوننگ الوقت:- **إعادة المحاولات:** `per_chunk_retry_limit` إلى `4` يتم استردادها مرة أخرى بمجرد حدوث أخطاء أخرى في الموفر. تم تنفيذ `4` من خلال سقف السقف ودوران المزود من خلال فناني الأداء سامنے ئيں.
- **عتبة الفشل:** `provider_failure_threshold` طے کرتا ہے کہ موفر شبكة کے باقی حصے کے لیے تعطيل ہو. قام وايليو بإعادة المحاولة باليس مرة أخرى: إذا كانت عتبة إعادة المحاولة للميزانية ستجعل منسقًا كل المحاولات، قم بإغلاق آخر مرة من نظير إلى آخر.
- **التزامن:** `global_parallel_limit` إلى `None` تم تحديد النطاقات المعلن عنها بشكل خاص ولا تكون مشبعة. إذا كان مقدمو خدمات ويليو يبثون ميزانيات إجمالية أقل من أو أقل من المجاعة.
- **تبديل التحقق:** `verify_lengths` و`verify_digests` يعملان على تنشيط البرنامج. تتمتع أساطيل المزودين المختلطة بالحتمية المضمونة؛ إنها بيئات غامضة معزولة فقط.

## 4. النقل والمواصلات والحركة

تُستخدم الوضعية السابقة للظهور لـ `rollout_phase` و`anonymity_policy` و`transport_policy`:- `rollout_phase="snnet-5"` الذي تم تحديثه وسياسة عدم الكشف عن هويته الافتراضية هي أهم معالم SNNet-5. `anonymity_policy_override` لم يتم التوقيع على التوجيهات الحالية بشأن استخدام الحوكمة.
- `transport_policy="soranet-first"` خط الأساس لـ SNNet-4/5/5a/5b/6a/7/8/12/13 🈺
  (ديكا `roadmap.md`). `transport_policy="direct-only"` يصرف تخفيضات التصنيف أو تدريبات الامتثال لاستخدامها، ومراجعة تغطية PQ بعد `transport_policy="soranet-strict"` في كل مرة - في جميع أنحاء المرحلات الكلاسيكية تفشل فورًا.
- `write_mode="pq-only"` يستغل هذا الوقت في كتابة مسار (SDK، المنسق، أدوات الإدارة) PQ المطلوبة للتنفيذ. عمليات الطرح التي تم إجراؤها أثناء تداول `write_mode="allow-downgrade"` هي عملية رد فعل للطرق المباشرة لمنع الحصار من خفض مستوى القياس عن بعد وزيادة عددها.
- اختيار الحرس وتنظيم الدائرة دليل SoraNet للحصر. لقطة موقعة `relay_directory` لقطة شاشة و`guard_set` ذاكرة تخزين مؤقت محفوظات نقرة حماية نافذة الاحتفاظ. `sorafs_cli fetch` تم حفظه وتخزينه مؤقتًا لدليل نشر بصمات الأصابع.

## 5. خطافات الرجوع إلى إصدار أقدم والامتثال

يوجد في هذا النظام جهازان داخليان في بغداد بالاس:- **معالجة الرجوع إلى إصدار أقدم** (`downgrade_remediation`): `handshake_downgrade_total` تجاوز عدد مرات استخدام الوكيل المحلي و`window_secs` اندر `threshold` `target_mode` للملفات التعريفية (بيانات التعريف فقط). لا يوجد أي شيء آخر (`threshold=3`، `window=300`، `cooldown=900`) من خلال قراءات مختلفة لسجلات الكمبيوتر. يتم أيضًا تجاوز سجل الطرح من خلال سجل الطرح و`sorafs_proxy_downgrade_state` الذي تم تحديثه مؤخرًا.
- **سياسة الامتثال** (`compliance`): الاختصاص القضائي وقوائم إلغاء الاشتراك الواضحة التي تديرها الحوكمة. تتميز حزمة المزامنة بتجاوزات مخصصة؛ لقد تم بالفعل `governance/compliance/soranet_opt_outs.json` للحصول على طلب التحديث الموقّع وإنشاء JSON تكرار النشر.

قم بإسقاط سلسلة من محفوظات حزمة الإنشاء الخاصة بها وأدلة الإصدار التي تتضمن أدوات لعب لللاعبين ومشغلات خفض المستوى التي تؤدي إلى تجميد اللعبة.

## 6. الويليمي وكل أنواع البورسلين

هناك أيضًا دور فعال في متابعة حركة درج الذعر المستهدف:- `sorafs_orchestrator_fetch_failures_total{reason="no_healthy_providers"}` —
  تم الانتهاء من ذلك بعد الصفر .
- `sorafs_orchestrator_retries_total` و
  `sorafs_orchestrator_retry_ratio` — تراجعت بنسبة 10% في معدل الإيداع وGA بعد 5% في الانخفاض.
- `sorafs_orchestrator_policy_events_total` — مرحلة الطرح المتوقعة لتحديث البيانات (التسمية `stage`) و`outcome` التي تؤدي إلى تسجيل براونوتس.
-`sorafs_orchestrator_pq_candidate_ratio` /
  `sorafs_orchestrator_pq_deficit_ratio` — التوقعات المستقبلية التي تقابلها PQ تتابع بطاقات التتابع.
- `telemetry::sorafs.fetch.*` لاغراد — المزيد من لاغجر و `status=failed` للحفظ.

`dashboards/grafana/sorafs_fetch_observability.json` سجل لوحة القيادة Grafana الأساسي
(الصفحة الرئيسية **SoraFS → جلب إمكانية الملاحظة** موجودة في ايكسبورت)، حدد المنطقة/محددات البيان،
إعادة محاولة الموفر للخريطة الحرارية، والمخططات البيانية لزمن الاستجابة، وعدادات التوقف، وSRE
عمليات الحرق التي تمت أثناء دیکھتا ہے۔ قواعد مدير التنبيهات `dashboards/alerts/sorafs_fetch_rules.yml`
بناء الجملة و`scripts/telemetry/test_sorafs_fetch_alerts.sh` وPrometheus
ويلي كريت (المساعد `promtool test rules` أو Docker هو كلاتا). تنبيه عمليات التسليم
من الضروري أن يكون هذا التوجيه أسودًا وأن يتم نشره باستخدام سكربت النص
ٹکٹ سے جوڑ سكيں.

### عمل حرق العمل

تم الانتهاء من عنصر خريطة الطريق **SF-6e** الذي تم حرقه في أقل من 30 يومًا للقياس عن بعد
العديد من الأخبار التي كتبها آركستراي GA هي على ما يرام. ريبو سكربتس يذرع إلى الأبد
نسخة تجريبية من مشروع إبداعي:

1. `ci/check_sorafs_orchestrator_adoption.sh` هي مقابض بيئة الاحتراق التي تعمل بالبطارية. مثال:

   ```bash
   SORAFS_BURN_IN_LABEL=canary-week-1 \
   SORAFS_BURN_IN_REGION=us-east-1 \
   SORAFS_BURN_IN_MANIFEST=manifest-v4 \
   SORAFS_BURN_IN_DAY=7 \
   SORAFS_BURN_IN_WINDOW_DAYS=30 \
   ci/check_sorafs_orchestrator_adoption.sh
   ```تم تحديث ملف Helper `fixtures/sorafs_orchestrator/multi_peer_parity_v1`،
   `scoreboard.json`، `summary.json`، `provider_metrics.json`، `chunk_receipts.json`،
   و `adoption_report.json` و `artifacts/sorafs_orchestrator/<timestamp>/` متشابهان،
   و`cargo xtask sorafs-adoption-check` يحذر من قيام مقدمي الخدمة المؤهلين بفرض القانون.
2. عند النسخ على ايبلز الموجود، سيتم تشغيل السكربت `burn_in_note.json` هنا، وهو ما يعني التسمية،
   فهرس اليوم ومعرف البيان ومصدر القياس عن بعد وملخصات المصنوعات اليدوية محفوظات ووتے ہیں۔ إنه طرح
   يتضمن السجل تقنية الالتقاط منذ 30 يومًا وهو ما يوضح كيفية الالتقاط بشكل واضح.
3. تم الانتهاء من Grafana باللون الأسود (`dashboards/grafana/sorafs_fetch_observability.json`) للتدريج/الإنتاج
   تم إنشاء مساحة العمل، وتسمية النسخ والنسخ، ومحرك البحث
   البيان/المنطقة کےمینے دکھاتا ہے.
4. إلى جانب `dashboards/alerts/sorafs_fetch_rules.yml`، `scripts/telemetry/test_sorafs_fetch_alerts.sh`
   (أو `promtool test rules …`) استخدم مقاييس احتراق توجيه التنبيه التي تتوافق مع إرشاداتك.
5. كل لقطة الشاشة، ومخرجات اختبار التنبيه و`telemetry::sorafs.fetch.*` لا تزال قائمة
   ابتكارات مهمة في الحوكمة في الأنظمة الحية والمقاييس والأدلة
   ری بلے کر سکے.

## 7. رول آوت چيك لٹ1. يتميز CI بتكوين المرشح وتكرار لوحات النتائج والعناصر التي تتحكم في الإصدار.
2. ماحول (مختبر، التدريج، الكناري، الإنتاج) يحتوي على قطع أثرية حتمية لجلب القطع الأثرية و`--scoreboard-out` و`--json-out` التي يتم نشرها في نفس الوقت.
3. مشغلو المكالمات الهاتفية عبر الإنترنت في بورصة ريو دي جانيرو، وأكبر عدد من المقاييس التي توفرها العينات الحية.
4. حماية إعدادات الشبكة (بشكل عام `iroha_config`) وسجل الإدارة الذي يلتزم فيه git بإعلانات التسجيل والامتثال للاستخدام.
5. يوفر أداة تعقب الطرح للبطاقات وSDK المزيد من الميزات لشبكات الإنترنت.

هذه هي البيروقراطية التي تناسب الـPerminist و كل شيء على ما يرام
أعد محاولة الميزانيات، وقدرة المزود، وموقف الخصوصية بشكل واضح
ملخص تفصيلي لفيلم لوبس.