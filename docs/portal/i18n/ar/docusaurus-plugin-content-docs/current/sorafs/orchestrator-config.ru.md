---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-config.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: تكوين الأوركسترا
العنوان: تكوين الأوركسترا SoraFS
Sidebar_label: منظم التكوين
الوصف: أداة جلب وجلب متعددة الاستخدامات وترجمة فورية وقياس عن بعد.
---

:::note Канонический источник
هذا الجزء يعرض `docs/source/sorafs/developer/orchestrator.md`. قم بإجراء نسخ متزامنة، حتى لا يتم إصدار الوثائق عن طريق الخطأ.
:::

# منظم الجلب متعدد الأغراض

يقوم منظم الجلب المتعدد SoraFS بإدارة عمليات التحديد
مدفوعات متوازية من مجموعة من مقدمي الخدمة، منشورة في الإعلانات تحت
السيطرة على الحكم. في هذه المقالة الرائعة كيفية إنشاء أوركسترا،
كيف يتم مراقبة الإشارات قبل الطرح، وكيف تظهر أجهزة القياس عن بعد
مؤشرات التألق.

## 1. تكوينات البحث

يلتزم المنسق بالتكوينات الثلاثة الأساسية:| أصل | الاسم | مساعدة |
|----------|-----------|------------|
| `OrchestratorConfig.scoreboard` | يتم تطبيع كل شيء، والتحقق من جميع أجهزة القياس عن بعد، ودعم لوحة نتائج JSON لمراجعي الحسابات. | بناء على `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | تمتع بحماية وقت التشغيل (استرداد الميزانية، توازي الحدود، التحقق من الإيقافات). | خريطة `FetchOptions` في `crates/sorafs_car::multi_fetch`. |
| معلمات CLI / SDK | قم بإلغاء حظر كل شيء، وتعزيز أجهزة القياس عن بعد في المناطق، وإظهار الرفض/التعزيز السياسي. | `sorafs_cli fetch` يتم رفع هذه الأعلام على النحو التالي; يتم نقل SDK إليها من خلال `OrchestratorConfig`. |

مساعدات JSON في `crates/sorafs_orchestrator::bindings` تقوم بالتسلسل بالكامل
التكوين في Norito JSON، يتم من خلال الاستبدال بين SDK والأتمتة.

### 1.1 تكوينات JSON الأولية

```json
{
  "scoreboard": {
    "latency_cap_ms": 6000,
    "weight_scale": 12000,
    "telemetry_grace_secs": 900,
    "persist_path": "/var/lib/sorafs/scoreboards/latest.json"
  },
  "fetch": {
    "verify_lengths": true,
    "verify_digests": true,
    "retry_budget": 4,
    "provider_failure_threshold": 3,
    "global_parallel_limit": 8
  },
  "telemetry_region": "iad-prod",
  "max_providers": 6,
  "transport_policy": "soranet_first"
}
```

قم بحفظ الملف من خلال التسجيل القياسي `iroha_config` (`defaults/`، المستخدم،
فعليًا)، لتحديد القيود المفروضة على الجميع
ليلة. للملف الشخصي المباشر فقط، من خلال طرح SNNet-5a، اضغط على
`docs/examples/sorafs_direct_mode_policy.json` وتوصيات الاستخدام
`docs/source/sorafs/direct_mode_pack.md`.

### 1.2 يتجاوز الخصوصية

تعمل SNNet-9 على تعزيز الامتثال القائم على الحوكمة في المعالج. كائن جديد
`compliance` في التكوينات Norito تقوم JSON بإصلاح المنحوتات التي يتم تحويلها
جلب خط الأنابيب в مباشر فقط:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` يتعرف على الكود ISO‑3166 alpha‑2، حيث يمكنك استخدامه
  مثيل الأوركستراتور. تتم تسوية الرموز في السجل الأخير عند التحليل.
- `jurisdiction_opt_outs` зералиruет еестра الحكم. عندما تحب الولاية القضائية
  يظهر المشغل في القائمة، ويؤدي الأوركيستراتور
  `transport_policy=direct-only` وإلغاء البديل الاحتياطي
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` لإعادة بيان الملخص (CID الأعمى، في النهاية)
  عرافة). الحمولات المصاحبة أيضًا للتخطيط والنشر المباشر فقط
  احتياطي `compliance_blinded_cid_opt_out` في أجهزة القياس عن بعد.
- قام `audit_contacts` بكتابة URI الذي تشاهده الإدارة في GAR
  مشغلي كتب اللعب.
- `attestations` حزم الامتثال الخاصة بالملفات المطلوبة
  السياسة. يمكنك كتابة إضافة اختيارية `jurisdiction` (ISO‑3166 alpha‑2),
  `document_uri`، الكنسي `digest_hex` (64 رمزًا)، التحقق من الطابع الزمني
  `issued_at_ms` واختياري `expires_at_ms`. يتم نشر هذه القطع الأثرية في
  منسق مراقبة التدقيق الذي يقوم بتجاوز أدوات الحوكمة
  الوثائق المصاحبة.قم بإعادة امتثال الكتلة من خلال تكوينات التثبيت القياسية
حصل المشغلون على تجاوزات تحديد. يشرح الأوركيستراتور
الامتثال _ بعد _ تلميحات وضع الكتابة: حتى إذا تم إلغاء SDK `upload-pq-only`،
إلغاء الاشتراك بموجب الولاية القضائية أو البيان لجميع وسائل النقل المباشرة فقط
وينتهي الأمر تمامًا إذا لم يتأخر مقدمو الخدمة المتطورون.

يتم إلغاء الاشتراك في الكتالوجات الكنسي
`governance/compliance/soranet_opt_outs.json`; سوفيت للحكم العام
الاشتراك من خلال الإصدارات الموسومة. تكوينات التمهيدي الكاملة (بما في ذلك
الشهادات) تم التسليم في `docs/examples/sorafs_compliance_policy.json`، أ
عملية التشغيل الموضحة في
[قواعد اللعبة التي تمارسها GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 ميزات CLI وSDK| العلم / القطب | تأثير |
|-------------|--------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | تأكد من توفير لوحة النتائج لمرشح المنتج. قم بتثبيت `None` لاستخدام جميع الموردين المؤهلين. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | قم بإعادة تشيسلو إلى القطعة. تم تحديد الحد السابق `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | قم بنسخ اللقطات المخفية/الخلفية في لوحة النتائج. جهاز القياس عن بعد السابق `telemetry_grace_secs` يجعل مقدم الخدمة غير مؤهل. |
| `--scoreboard-out` | تحتوي على لوحة النتائج النهائية (المؤهلة + المحققين غير المؤهلين) للتحليل اللاحق. |
| `--scoreboard-now` | قم باقتراح لوحة نتائج الطابع الزمني (ثواني Unix)، لتتمكن من التقاط التركيبات لتتمكن من تحديد المحددات. |
| `--deny-provider` / نقاط هوك السياسية | التحديد يستثني مقدمي التخطيط بدون الإعلانات المزعجة. مفيد لتعزيز القائمة السوداء. |
| `--boost-provider=name:delta` | قم بتصحيح مقدمي الائتمانات المستديرة المرجحة، وليس أي حوكمة. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | قم بتسويق المقاييس والشعارات الهيكلية بحيث يمكن تجميع لوحات البيانات حسب الموقع الجغرافي أو الطرح الكامل. || `--transport-policy` / `OrchestratorConfig::with_transport_policy` | من خلال `soranet-first`، كما هو الحال مع أوركسترا متعدد الاستخدامات - أساسي. استخدم `direct-only` عند التخفيض أو الامتثال التوجيهي، ويتوقف `soranet-strict` للطيارين PQ فقط؛ يتجاوز الامتثال остаются естким потолком. |

SoraNet-الخطوة الأولى من التراجع، والتراجعات الإضافية التي تم إجراؤها على نحو سلس
مانع SNNet. تم تحديث الحوكمة بعد إصدار SNNet-4/5/5a/5b/6a/7/8/12/13
الوضع المناسب (في الطابق `soranet-strict`); إلى هذا فقط تجاوز
يجب إعطاء الأولوية للعنصر `direct-only`، ويجب إصلاحه في السجل
الطرح.

جميع الأعلام الجديدة تبدأ سينتاكسيس `--` مثل `sorafs_cli fetch`، إلخ.
الثنائي العملي `sorafs_fetch`. يوفر SDK هذه الخيارات من خلال كتابتها
بناة.

### 1.4 إدارة ذاكرة التخزين المؤقت للحارس

يدعم خط CLI حراس المحدد SoraNet، بحيث يمكن للمشغلين
بشكل محدد، قم بإغلاق مرحلات الدخول إلى الطرح الكامل لـ SNNet-5.
تتحكم العملية الصناعية في ثلاثة أعلام جديدة:| علم | الاسم |
|------|-----------|
| `--guard-directory <PATH>` | قم بالإشارة إلى ملف JSON باستخدام إجماع التتابع التالي (متوافق تمامًا). يقوم الدليل السابق بإحداث ذاكرة تخزين مؤقت للحماية قبل الجلب. |
| `--guard-cache <PATH>` | Сограняет Norito المشفر `GuardSet`. تستخدم الإصدارات التالية ذاكرة التخزين المؤقت، حتى لو لم يتم إضافة الدليل الجديد. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | تجاوزات اختيارية لحراس الدخول (للفتح 3) والمراقبة الدقيقة (للغلق لمدة 30 يومًا). |
| `--guard-cache-key <HEX>` | مفتاح اختياري بسعة 32 بايت لذاكرة التخزين المؤقت للحماية مع Blake3 MAC، والذي يمكنك من خلاله التحقق من الملف قبل الاستخدام التالي. |

يستخدم دليل حماية الحمولات نظامًا مضغوطًا:

العلم `--guard-directory` teperь одидает الحمولة النافعة المشفرة Norito
`GuardDirectorySnapshotV2`. لقطة ثنائية مشتركة:- `version` — مخططات الإصدار (مثل `2`).
- `directory_hash`، `published_at_unix`، `valid_after_unix`، `valid_until_unix` —
  الإجماع التحويلي الذي يتم إرفاقه بالشهادة الأساسية.
- `validation_phase` — بوابة الشهادة السياسية (`1` = razreshiть одну Ed25519
  اكتب، `2` = اقتراح منتجين، `3` = مطلوب منتجين).
- `issuers` — إدارة العناصر مع `fingerprint` و`ed25519_public` و
  `mldsa65_public`. يتم تحديد بصمة الإصبع مثل
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — قائمة حزم SRCv2 (выод `RelayCertificateBundleV2::to_cbor()`).
  في بعض الأحيان، يتم ترحيل واصف وحدة التحكم، وعلامات القدرة، والسياسة ML-KEM،
  نموذج مزدوج Ed25519/ML-DSA-65.

تتحقق CLI من كل حزمة ضد جهة إصدار المفتاح قبل الالتزام بها
لقطات.

اختر CLI باستخدام `--guard-directory` للالتزام بالإجماع الفعلي مع
ذاكرة التخزين المؤقت المتاحة. يحتوي المحدد على واقيات مزخرفة، والتي توجد أيضًا في النهاية
الخدمة والإمدادات في الدليل; يتم تقليل المرحلات الجديدة بشكل متزايد.
بعد نجاح جلب ذاكرة التخزين المؤقت العامة، تم تسجيلها عبر `--guard-cache`،
الالتزام بتحديد الجلسات التالية. تحديث SDK,
вызывая `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
ويتم تحويل `GuardSet` إلى `SorafsGatewayFetchOptions`.`ml_kem_public_hex` يتيح للمحدد إعطاء الأولوية لحراس PQ في الوقت المناسب
طرح SNNet-5. تبديل المرحلة (`anon-guard-pq`، `anon-majority-pq`،
`anon-strict-pq`) يقوم بتشغيل المرحلات الكلاسيكية: عندما
حارس PQ ممتاز، محدد ذو دبابيس كلاسيكية لطيفة، للمتابعة
сссии почитали هجين المصافحات. ملخصات CLI/SDK توضح ذلك
ميكس عبر `anonymity_status`/`anonymity_reason`، `anonymity_effective_policy`،
`anonymity_pq_selected`، `anonymity_classical_selected`، `anonymity_pq_ratio`،
`anonymity_classical_ratio` وكامل المرشحين/العيوب/توريد الديون,
تأخير انقطاع التيار الكهربائي والاحتياطيات الكلاسيكية.

يمكن الآن تأمين أدلة الحراسة لحزمة SRCv2 الكاملة عبر
`certificate_base64`. يقوم برنامج فك تشفير كل حزمة، التحقق مرة أخرى
قم بتضمين Ed25519/ML-DSA وتضمن شهادة متنوعة مع ذاكرة التخزين المؤقت للحماية.
عند الحصول على الشهادة، على مفاتيح PQ التقليدية الأساسية،
المصافحة والصلاة; يتم الحصول على الشهادات التالية والمحدد
دائرة سيكلوم حيوية وقابلة للشحن عبر `telemetry::sorafs.guard` و
`telemetry::sorafs.circuit` وصلاحية النقر الثابتة وأجنحة المصافحة وغيرها
تم الانتهاء من طلبين لكل حارس.

استخدم مساعدات CLI لتتمكن من مزامنة اللقطات مع
الناشرين:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```يقوم `fetch` بتسجيل لقطة SRCv2 والتحقق من صحتها قبل كتابة القرص على `verify`
التحقق من صحة خط الأنابيب للمنتج من أمر آخر، التحقق من JSON
ملخص، الذي يشير إلى محدد حارس الإخراج CLI/SDK.

### 1.5 دائرة إدارة الدائرة الكهربائية

عند الوصول إلى دليل الترحيل، وذاكرة التخزين المؤقت للحماية، ودائرة تنشيط المنظم
مدير دورة الحياة للتعزيز المسبق ودوائر SoraNet
قبل كل شيء، الجلب. يتم التكوين في `OrchestratorConfig`
(`crates/sorafs_orchestrator/src/lib.rs:305`) من خلال خطين جديدين:

- `relay_directory`: عرض لقطة دليل SNNet-3، للقفزات الوسطى/الخروج
  يتم اختيار التحديد.
- `circuit_manager`: التكوين الاختياري (المتضمن في الترقية)،
  التحكم في TTL.

Norito JSON الخطوة الأولى للكتلة `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

يقوم SDK بإعادة توجيه البيانات من خلال الدليل
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`)، يتضمن CLI تلك التلقائية عندما
يتم نقله إلى `--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`).

يقوم الوسيط بإعادة تشغيل الدوائر عندما يقوم بتشغيل حارس بديل (نقطة النهاية، مفتاح PQ
أو الطابع الزمني المثبت) أو احصل على TTL. المساعد `refresh_circuits`, مميز
قبل الجلب (`crates/sorafs_orchestrator/src/lib.rs:1346`)، قم بكتابة الشعار
`CircuitEvent`، يسمح للمشغل بمراقبة قرارات الحياة. نقع
اختبار `circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) إظهار الاستقرار
latentness على ثلاثة حراس تناوب. اكتشف ذلك في
`docs/source/soranet/reports/circuit_stability.md:1`.### 1.6 وكيل سريع محلي

يمكن للمشرف اختياريًا تشغيل بروكسي QUIC المحلي للتشغيل المباشر
لا تقوم محولات التوسيع وSDK بإدارة الشهادات أو حماية مفاتيح التخزين المؤقت. بروكسي
عنوان الاسترجاع الضمني، وإنهاء ارتباط QUIC والتعبير عن بيان Norito،
شهادة مكتوبة ومفتاح حماية اختياري لذاكرة التخزين المؤقت. وسائل النقل,
الوكيل المدفوع، يتم تعلمه في `sorafs_orchestrator_transport_events_total`.

تضمين الوكيل عبر الكتلة الجديدة `local_proxy` في منظم JSON:

```json
"local_proxy": {
  "bind_addr": "127.0.0.1:9443",
  "telemetry_label": "dev-proxy",
  "guard_cache_key_hex": "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
  "emit_browser_manifest": true,
  "proxy_mode": "bridge",
  "prewarm_circuits": true,
  "max_streams_per_circuit": 64,
  "circuit_ttl_hint_secs": 300,
  "norito_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito"
  },
  "kaigi_bridge": {
    "spool_dir": "./storage/streaming/soranet_routes",
    "extension": "norito",
    "room_policy": "public"
  }
}
```- `bind_addr` يتم إرسال عنوان البريد الإلكتروني (استخدم المنفذ `0` مؤقتًا)
  ميناء).
- `telemetry_label` متناسب مع المقاييس لتبادل الوكيل
  و جلب سيسيسي.
- `guard_cache_key_hex` (اختياري) يسمح للوكيل بالخروج منه بمفتاح
  حماية ذاكرة التخزين المؤقت لاستخدام CLI/SDK لإلغاء الحظر
  مزامنة.
- `emit_browser_manifest` يتضمن البيان الذي يمكن أن يتم نشره
  قم بالتسجيل والتحقق.
- اختيار `proxy_mode`، أو إرسال حركة المرور المحلية (`bridge`) أو بالوكالة
  فقط قم بالتحديث الفوقي لفتح SDK لدوائر SoraNet نفسها
  (`metadata-only`). من خلال `bridge`; استخدم `metadata-only`، إذا
  يجب على الموقف العملي التحقق من البيان بدون خطوات إعادة الترجمة.
- `prewarm_circuits`، `max_streams_per_circuit` و`circuit_ttl_hint_secs`
  قم بإلقاء نظرة على التلميحات الإضافية التي يمكنك من خلالها توفير ميزانية متوازية
  الطرق والتعامل مع دوائر إعادة الاستخدام العدوانية.
- `car_bridge` (اختياري) يسمح بذاكرة التخزين المؤقت المحلية لأرشيف السيارات. بول
  `extension` يتم إضافة ملحقة عندما لا يتم توصيل الهدف `*.car`; مزيد من المعلومات
  `allow_zst = true` للفحص الأول `*.car.zst`.
- `kaigi_bridge` (اختياري) يعرض مسارات Kaigi من التخزين المؤقت في الوكيل. بول
  `room_policy` يطابق الوضع `public` أو `authenticated` للتشغيل السريع
  يطلب العملاء اختيار تسميات GAR الصحيحة.- `sorafs_cli fetch` يسمح بالتجاوزات `--local-proxy-mode=bridge|metadata-only`
  و`--local-proxy-norito-spool=PATH`، يسمح بضبط وقت التشغيل أو إيقافه
  مكبات بديلة باستثناء سياسة JSON.
- `downgrade_remediation` (اختياريًا) يقوم بتثبيت خطاف الرجوع إلى إصدار سابق تلقائيًا.
  عندما يشمل ذلك, يقوم الأوركسترا بتمرير مرحلات القياس عن بعد لجميع الرجوع إلى إصدار أقدم و,
  بعد `threshold` السابق إلى `window_secs`، يتم تحويل الوكيل بشكل تلقائي
  في `target_mode` (من خلال `metadata-only`). عندما يتم تخفيض التصنيف،
  يعود الوكيل إلى `resume_mode` بعد `cooldown_secs`. استخدم كمية كبيرة
  `modes`، لتنشيط مرحلات الأدوار المحددة (من خلال مرحلات الدخول).

عندما يعمل الوكيل في نظام الجسر، لخدمة تطبيقين:

- **`norito`** — يتم إعادة تعيين العملاء المستهدفين للبث بشكل متكرر
  `norito_bridge.spool_dir`. يتم تعقيم الأهداف (بدون الاجتياز، بدون
  الطريق المطلق)، وإذا فشلت بدون إضافة ملحقة أساسية
  до отправки الحمولة في البروزر.
- **`car`** — تقوم أهداف الدفق بإعادة تحديد المدخلات `car_bridge.cache_dir`، التالي
  يتم التوسيع التلقائي وإلغاء استنساخ الحمولات الصافية، إذا لم يتم تضمين `allow_zst`.
  لاحظ الجسر المتميز `STREAM_ACK_OK` قبل إرسال أرشيف البيت، لذلك
  يمكن للعملاء التحقق من خط الأنابيب.في جميع الحالات، يقترح الوكيل علامة ذاكرة التخزين المؤقت HMAC (إذا كان مفتاح حماية ذاكرة التخزين المؤقت موجودًا في
المصافحة المؤقتة) واكتب `norito_*` / `car_*` أكواد السبب لشبكات الاتصال
العديد من النجاحات والفشل والتعقيم.

`Orchestrator::local_proxy().await` قم بفك المقبض لقطع PEM
الشهادة أو الحصول على بيان المتصفح أو التصحيح الصحيح عند الاستخدام
تطبيق.

عندما يتم تضمين الوكيل، على теperь отдает записи **manifest v2**. بوميمو
توفر الشهادات الخاصة ومفتاح حماية ذاكرة التخزين المؤقت، الإصدار الثاني:

- `alpn` (`"sorafs-proxy/1"`) و`capabilities` الضخم لإعلام العملاء
  بروتوكول البروتوكول.
- `session_id` للمصافحة و`cache_tagging` كتلة الملح للاشتقاق
  تقارب حارس الجلسة وعلامات HMAC.
- تلميحات حول اختيار الدائرة والحارس (`circuit`، `guard_selection`،
  `route_hints`) لمزيد من واجهة المستخدم لفتح النافذة.
- `telemetry_v2` مع مقابض للجمع والخصوصية للأجهزة المحلية.
- تتضمن `STREAM_ACK_OK` `cache_tag_hex`. العملاء يتجاهلون هذا الأمر
  في القفل `x-sorafs-cache-tag` عند استخدام HTTP/TCP للحماية المخبأة
  التحديدات تمنع تخزين القرص.

هذه هي أحدث التقنيات — يمكن للعملاء القدامى أن يتجاهلوا المفاتيح الجديدة
استمر في استخدام المجموعة الفرعية v1.

## 2. ملاحظات الدلالة

يقوم الأوركسترا بجهود كبيرة في التحقق من الإمكانيات والميزانية المتاحة
المرة الأولى. ميزات تنقسم إلى ثلاث فئات:1. **الخيارات المؤهلة (ما قبل الرحلة).** مقدمو الخدمة خارج نطاق القدرة،
   مزيد من الإعلانات أو تثبيت القياس عن بعد في لوحة النتائج
   قطعة أثرية ولا يتم نشرها في التخطيط. ملخصات CLI لها تأثير هائل
   `ineligible_providers` نصائح تمكن المشغلين من رؤية الانجراف
   الحكم بدون شعارات التحليل.
2. **استنفاد وقت التشغيل.** يقوم كل مزود بمراقبة الملحقات اللاحقة.
   عندما يتم توصيل `provider_failure_threshold`، يخبرك الخادم كيف
   `disabled` إلى نهاية الجلسة. إذا كان جميع المزودين قد وصلوا إلى `disabled`، المنسق
   возвращает `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **تحديد الأفضلية.** تتوسع حدود القيود حسب الهيكلية
   ملاحظات:
   - `MultiSourceError::NoCompatibleProviders` — بيان مدى/محاذاة الثلاثة،
     لا يمكن للمزودين توفير أي خدمات فرعية.
   - `MultiSourceError::ExhaustedRetries` — استرداد الميزانية إلى قطعة.
   - `MultiSourceError::ObserverFailed` — مراقبو المصب (خطافات التدفق)
     قطعة مثبتة تم تفكيكها.

كل شيء يتعلق بمؤشر توصيل القطعة الإشكالية، وعندما يتم التسليم، نهائيًا
شكرًا مقدمًا. اقرأ هذه الأشياء التي تحرر حاصرات — إعادة
رسائل بالموضوع أو إدخال رسالتك أو إعلانك أو قياس المسافة أو النهاية
المقدم لا يخفف.

### 2.1 لوحة النتائج المصاحبة

عند إنشاء `persist_path`، قام المنسق بكتابة لوحة النتائج النهائية بعد ذلك
كل ما عليك فعله هو التقدم. تنسيق مستند JSON:- `eligibility` (`eligible` أو `ineligible::<reason>`).
- `weight` (تطبيع الوقت، أساسًا لهذا التقدم).
- ميتادانني `provider` (المعرف، نقاط النهاية، موازاة الميزانية).

أرشفة لقطات لوحة النتائج مباشرة مع إصدار العناصر لاتخاذ القرار
تم إلغاء التحقق من القائمة السوداء والطرح.

## 3. القياس عن بعد والنقل

### 3.1 متر Prometheus

يقوم الموزع بحذف المقاييس التالية من خلال `iroha_telemetry`:| متريكا | التسميات | الوصف |
|---------|-------|----------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`، `region` | عملية الجلب النشطة. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`، `region` | جلب التسجيل الكامل. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`، `region`، `reason` | ملاحظات نهائية من سكيتشيتش (المراجعة النهائية، لا مقدم، مراقب أوشيبكا). |
| `sorafs_orchestrator_retries_total` | `manifest_id`، `provider`، `reason` | يستعيد المشتري المشتري من خلال مقدم الطلب. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`، `provider`، `reason` | تبرعات مقدم الطلب في دورة الحياة، خاصة بالخصم. |
| `sorafs_orchestrator_policy_events_total` | `region`، `stage`، `outcome`، `reason` | خيار إخفاء الهوية السياسية (اختياري مقابل انقطاع التيار الكهربائي) عند بدء تشغيل الملعب والبدء في التراجع. |
| `sorafs_orchestrator_pq_ratio` | `region`، `stage` | يعد التسجيل بمثابة مرحلات PQ بين مجموعة SoraNet المختارة. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`، `stage` | يتم استخدام سجل PQ في لوحة النتائج اللقطة. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`، `stage` | العجز التاريخي في السياسة (يختلف بين الجميع والتكاليف الواقعية PQ). |
| `sorafs_orchestrator_classical_ratio` | `region`، `stage` | سجل المرحلات الكلاسيكية في كل دورة. |
| `sorafs_orchestrator_classical_selected` | `region`، `stage` | سجل تشيسلا اختيار المرحلات الكلاسيكية في كل مرة. |قم بدمج المقاييس في لوحات المعلومات المرحلية بما في ذلك مقابض الإنتاج.
يوصى بإعادة النظر في خطة مراقبة SF-6:

1. **الجلب النشط** — تنبيه إذا تم إجراء القياس بدون عمليات إكمال رائعة.
2. **نسبة إعادة المحاولة** — استبق خطوط الأساس التاريخية السابقة عند `retry`.
3. **فشل الموفر** — تشغيل تنبيهات جهاز النداء، عند عودة أي موفر
   `session_failure > 0` قبل 15 دقيقة.

### 3.2 أهداف السجل المنظم

يقوم المدير بنشر الجمعية الهيكلية العامة في تحديد الأهداف:

- `telemetry::sorafs.fetch.lifecycle` — العلامات `start` و `complete` مع شيلوم
  قطع، استرجاع ووضوح كبير.
- `telemetry::sorafs.fetch.retry` — события retраев (`provider`, `reason`,
  `attempts`) للفرز الجيد.
- `telemetry::sorafs.fetch.provider_failure` — المزودون، خارج نطاق الخدمة
  очботорающичся озибок.
- `telemetry::sorafs.fetch.error` - الإضافات النهائية مع `reason` والخيارات الاختيارية
  مقدم الخدمة التحويلية.

قم بضبط هذه الخطوات في مسار سجل Norito الخاص بالجهاز، مما يؤدي إلى وقوع الحادث
كانت الاستجابة واحدة من السطور الأصلية. توضح دورة الحياة PQ/classical
مزيج من خلال `anonymity_effective_policy`، `anonymity_pq_ratio`،
`anonymity_classical_ratio` والأدوات الذكية التي تقوم بشراء الجهاز
لوحة القيادة بدون تحليل متري. في أثناء عمليات طرح GA، قم بإضافة الشعار الجديد
`info` لدورة الحياة/إعادة المحاولة واستخدم `warn` للأخطاء الطرفية.

### 3.3 ملخصات JSON`sorafs_cli fetch` وRust SDK يعرضان ملخصًا هيكليًا ومتوافقًا:

- `provider_reports` مع الترحيب/الموافقة ومقدم الخدمة التفضيلية.
- `chunk_receipts`، يوضح كيف تم تخصيص قطعة كبيرة للمزود.
- ضخمة `retry_stats` و `ineligible_providers`.

أرشفة الملخص عند تقديم مشكلات للمزودين — الإيصالات على سبيل المثال
تمت الموافقة عليه من خلال السجل الفوقي.

## 4. قائمة العمليات

1. ** قم بتنفيذ التكوين في CI.** قم بتثبيت `sorafs_fetch` بالسيلفي
   التكوين، أدخل `--scoreboard-out` لعرض التأهل و
   сравните مع الإصدار المسبق. أي مقدم خدمة غير مؤهل
   حظر الترويج.
2. **التحقق من القياس عن بعد.** التأكد من نشر مقاييس التصدير
   `sorafs.fetch.*` والسجلات الهيكلية التي سبقت تضمين الجلب متعدد المصادر
   للمستفيد. يشير المقياس النهائي بوضوح إلى أن المنسق الموسيقي
   لم يتم إرسالها.
3. **توثيق التجاوزات.** في حالات الطوارئ `--deny-provider` أو
   `--boost-provider` قم بتأكيد JSON (أو CLI вызов) في سجل التغيير. التراجعات
   ما عليك سوى إزالة التجاوز والتقاط لقطة جديدة للوحة النتائج.
4. **التحقق من اختبارات الدخان.** استرجاع الميزانيات أو الحدود القصوى بعد الموازنة
   مقدم الطلب يستخدم جلب لاعبا اساسيا قياسيا
   (`fixtures/sorafs_manifest/ci_sample/`) ويلاحظ أن الإيصالات على شكل قطع
   يتم تحديدها.متابعة أهم ما تم تنظيمه من قبل الأوركسترا المصاحبة
عمليات النشر وتوفر أجهزة القياس عن بعد للاستجابة للحوادث.

### 4.1 يتجاوز السياسة

يمكن للمشغلين اختراق عملية النقل/إخفاء الهوية النشطة دون أساس التحسين
التكوينات، بعد `policy_override.transport_policy` و
`policy_override.anonymity_policy` في JSON `orchestrator` (أو مقدم)
`--transport-policy-override=` / `--anonymity-policy-override=` в
`sorafs_cli fetch`). إذا تم تجاوز الأمر، فإن الأوركسترا يرسل طلبًا عاديًا
احتياطي Brownout: إذا كان مستوى PQ غير ضروري، فسيتم إنهاء الجلب باستخدام
`no providers` الرجوع إلى إصدار أقدم. تأمل في التأمل —
مجرد مراقبة تجاوز القطب.