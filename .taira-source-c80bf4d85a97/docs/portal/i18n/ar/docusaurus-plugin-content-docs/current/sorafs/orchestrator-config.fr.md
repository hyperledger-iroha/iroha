---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-config.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: تكوين الأوركسترا
العنوان: تكوين الأوركسترا SoraFS
Sidebar_label: تكوين الأوركسترا
الوصف: تكوين مُنسق الجلب متعدد المصادر، وترجمة العناصر الإلكترونية، وتصحيح القياس عن بعد.
---

:::ملاحظة المصدر الكنسي
هذه الصفحة تعكس `docs/source/sorafs/developer/orchestrator.md`. قم بمزامنة النسختين حتى تتم إزالة الوثائق الموروثة.
:::

# دليل منسق الجلب متعدد المصادر

منسق الجلب متعدد المصادر لـ SoraFS قائد التنزيلات
المتوازيات والمحددات من مجموعة الموردين المنشورة في
الإعلانات تدعم الحكم. دليل CE يشرح مكون التعليق
المنسق، الذي يقوم بتسجيل الدخول، يحضر خلال عمليات النشر والتسجيل
يعرض تدفق البيانات عن بعد المؤشرات الصحية.

## 1. عرض مجموعة التكوين

يدمج المنسق ثلاثة مصادر للتكوين :| المصدر | موضوعي | ملاحظات |
|--------|---------|-------|
| `OrchestratorConfig.scoreboard` | قم بتطبيع أوزان الموردين، وتحقق من ضعف القياس عن بعد، واستمر في لوحة النتائج التي يستخدمها JSON لعمليات التدقيق. | Adossé à `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | تطبيق حدود التنفيذ (ميزانيات إعادة المحاولة، وحدود الموافقة، وأساسيات التحقق). | قم بالتخطيط لـ `FetchOptions` في `crates/sorafs_car::multi_fetch`. |
| معلمات CLI / SDK | قم بتحديد عدد الأزواج، وإرفاق مناطق القياس عن بعد، وكشف سياسات الرفض/التعزيز. | `sorafs_cli fetch` يكشف توجيه أعلام ces ؛ يتم نشر SDK عبر `OrchestratorConfig`. |

مساعدات JSON في `crates/sorafs_orchestrator::bindings` متسلسلة
اكتمل التكوين في Norito JSON، وينتج عن الارتباطات المحمولة SDK وما إلى ذلك
الأتمتة.

### 1.1 مثال لتكوين JSON

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

استمر في حفظ الملف عبر التشغيل المعتاد `iroha_config` (`defaults/`، المستخدم،
فعلي) afin que les déploiements déterministes héritent des mêmes Limites sur
الأحداث. لإنشاء ملف تعريف للرد محاذاة مباشرة فقط عند بدء تشغيل SNNet-5a،
راجع `docs/examples/sorafs_direct_mode_policy.json` والمؤشرات
الشركاء في `docs/source/sorafs/direct_mode_pack.md`.

### 1.2 تجاوزات المطابقةSNNet-9 يتكامل مع المطابقة التجريبية للحوكمة في المنسق.
كائن جديد `compliance` في التكوين Norito JSON يلتقط
اقتطاعات تجبر خط الأنابيب على الجلب في الوضع المباشر فقط :

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```- `operator_jurisdictions` يعلن عن رموز ISO‑3166 alpha‑2 où opère cette
  مثال على الأوركسترا. يتم تطبيع الرموز بشكل كبير أثناء قيامك بذلك
  تحليل.
- `jurisdiction_opt_outs` يعكس سجل الإدارة. لورشيون
  تظهر الولاية القضائية للمشغل في القائمة، التي يفرضها المنسق
  `transport_policy=direct-only` وهذا هو سبب التراجع
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` يسرد ملخصات البيان (CIDs المخفية والمشفرة في
  الست عشري الكبير). الحمولات المقابلة لها قوة كبيرة
  التخطيط المباشر فقط والكشف الاحتياطي
  `compliance_blinded_cid_opt_out` في جهاز القياس عن بعد.
- `audit_contacts` قم بتسجيل URI الذي تديره المشغلين
  في كتب اللعب GAR.
- `attestations` يلتقط حزم المطابقة التي تبررها
  سياسة. كل ما عليك فعله هو تحديد خيار `jurisdiction` (رمز ISO‑3166
  alpha-2)، `document_uri`، `digest_hex` الكنسي بـ 64 حرفًا، le
  الطابع الزمني للإصدار `issued_at_ms`، وخيار `expires_at_ms`. سيس
  المصنوعات اليدوية هي قائمة مراجعة التدقيق الخاصة بالمدير لتتمكن من ذلك
  قد تعتمد أدوات الحوكمة على تجاوزات المستندات الموقعة.قم بتوفير كتلة المطابقة عبر عملية التكوين المعتادة
أن المشغلين يحصلون على تجاوزات محددة. المنسق
تطبيق المطابقة _بعد_ تلميحات وضع الكتابة : حتى إذا طلبت SDK
`upload-pq-only`، عمليات إلغاء الاشتراك في الاختصاص القضائي أو إظهارها دائمًا
مقابل النقل المباشر فقط والاستجابة السريعة لموردي الخدمة
لا يوجد.

الكتالوجات الأساسية هي إلغاء الاشتراك في الموقع
`governance/compliance/soranet_opt_outs.json` ; مجلس الحكم العام
les Misses à jour via des Releases taggées. اكتمل مثال التكوين
(بما في ذلك الشهادات) متاحة هناك
`docs/examples/sorafs_compliance_policy.json`، والعملية قيد التشغيل
القبض على لو
[قواعد اللعبة المتوافقة مع GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 معلمات CLI وSDK| العلم / البطل | تأثير |
|-------------|-------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | حدد عدد الموردين الذين يجتازون مرشح لوحة النتائج. Mettez `None` لاستخدام جميع الموردين المؤهلين. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | Plafonne les المحاولات على قدم المساواة. تجاوز الحد الفاصل `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | أدخل لقطات زمن الاستجابة/التحقق في مُنشئ لوحة النتائج. يؤدي إجراء اتصال عن بعد عبر `telemetry_grace_secs` إلى جعل الموردين غير مؤهلين. |
| `--scoreboard-out` | استمر في حساب لوحة النتائج (الموردون المؤهلون + غير المؤهلين) للفحص بعد التشغيل. |
| `--scoreboard-now` | قم بإضافة الطابع الزمني للوحة النتائج (ثواني Unix) لحفظ لقطات المباريات المحددة. |
| `--deny-provider` / خطاف النتيجة السياسية | باستثناء موردي طرق التحديد دون حذف الإعلانات. مفيد للقائمة السوداء للرد السريع. |
| `--boost-provider=name:delta` | قم بضبط الائتمانات المستديرة من قبل مورد دون لمس أدوات الإدارة. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | قم بضبط المقاييس والسجلات المصممة بحيث يمكن للوحات المعلومات أن تدور حول المنطقة الجغرافية أو غامضة عند الطرح. || `--transport-policy` / `OrchestratorConfig::with_transport_policy` | بشكل افتراضي `soranet-first`، يظل المُنسق متعدد المصادر هو القاعدة. استخدم `direct-only` عند الرجوع إلى إصدار سابق أو توجيه مطابقة، واحتفظ بـ `soranet-strict` لطياري PQ فقط؛ تبقى تجاوزات المطابقة على السقف الثابت. |

تم تعطيل SoraNet-first للكتابة الافتراضية، ويجب الإشارة إلى عمليات التراجع
مراسل SNNet متفاخر. بعد تخرج SNNet-4/5/5a/5b/6a/7/8/12/13,
الحكم يتطلب الوضعية المطلوبة (الإصدار `soranet-strict`) ؛ ديسي لا، ليه
فقط يتجاوز الدوافع حسب الحادث الذي له امتياز `direct-only`، وما إلى ذلك
يتم إرساله إلى سجل التشغيل.

جميع العلامات ci-dessus تقبل بناء الجملة `--` في `sorafs_cli fetch` et le
ثنائي الاتجاه المطورين `sorafs_fetch`. يعرض SDK خيارات الصور
عبر أنواع المنشئين.

### 1.4 إدارة ذاكرة التخزين المؤقت للحراس

يقوم كابل CLI بتعطيل محدد حماية SoraNet للسماح بالدخول
مشغلو مرحلات الدخول تحدد الطريقة قبل
اكتمل الطرح بواسطة شركة النقل SNNet-5. أعلام جديدة تتحكم في التدفق:| علم | موضوعي |
|------|----------|
| `--guard-directory <PATH>` | أشر إلى ملف JSON ينبثق من توافق التتابعات الأحدث (sous-ensemble ci-dessous). قم بتمرير الدليل لفتح ذاكرة التخزين المؤقت للحماية قبل تنفيذ الجلب. |
| `--guard-cache <PATH>` | استمر في تشفير `GuardSet` في Norito. يتم إعادة استخدام عمليات التنفيذ اللاحقة في ذاكرة التخزين المؤقت بدون دليل جديد. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | يتجاوز الخيارات لعدد واقيات الدخول إلى pingler (افتراضيًا 3) ونافذة الاحتفاظ (افتراضيًا 30 يومًا). |
| `--guard-cache-key <HEX>` | تم استخدام مفتاح مكون من 32 ثمانيًا لتجميع ذاكرات التخزين المؤقت باستخدام MAC Blake3 للتحقق من الملف قبل إعادة استخدامه. |

تستخدم حمولات دليل الحراسة مخططًا مضغوطًا:

العلم `--guard-directory` يعطل الحمولة `GuardDirectorySnapshotV2`
تم ترميزه في Norito. محتوى اللقطة الثنائية :- `version` — نسخة المخطط (العنصر الحالي `2`).
- `directory_hash`، `published_at_unix`، `valid_after_unix`، `valid_until_unix` —
  يجب أن تتوافق بيانات الإجماع مع كل شهادة متكاملة.
- `validation_phase` — بوابة سياسة الشهادات (`1` = مُؤذن
  التوقيع الوحيد Ed25519، `2` = يفضل التوقيعات المزدوجة، `3` = exiger
  التوقيعات المزدوجة).
- `issuers` — أدوات الحكم مع `fingerprint`، `ed25519_public` وآخرون
  `mldsa65_public`. بصمات الأصابع هي محسوبة مثل
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — قائمة حزم SRCv2 (فرز
  `RelayCertificateBundleV2::to_cbor()`). تتضمن الحزمة Chaque واصف du
  التتابع وأعلام القدرة والسياسة ML-KEM والتوقيعات المزدوجة
  Ed25519/ML-DSA-65.

يتحقق CLI من كل حزمة مقابل المفاتيح التي تم الإعلان عنها مسبقًا
دمج الدليل مع حراس ذاكرة التخزين المؤقت. Les Esquisses JSON héritées ne
ابن زائد المقبولين ; لقطات SRCv2 مطلوبة.اتصل بـ CLI مع `--guard-directory` لدمج التوافق الإضافي
حديثة مع وجود ذاكرة التخزين المؤقت. يحافظ المحدد على ظهور الواقيات مرة أخرى
صالحة في نافذة الاحتجاز ومؤهلة في الدليل؛ ليه
المرحلات الجديدة تحل محل الإدخالات التي انتهت صلاحيتها. بعد إحضار ملف جديد، ذاكرة التخزين المؤقت
يتم تسجيل Mis à jour في المورد الرئيسي عبر `--guard-cache`، مع مراعاة
جلسات suivantes déterministes. تعمل SDK بنفس الطريقة
المستأنف `GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)`
ويتم حقن `GuardSet` الناتج في `SorafsGatewayFetchOptions`.

يتيح `ml_kem_public_hex` تحديد أولويات الواقيات القادرة على PQ
تعليق الطرح SNNet-5. تبديل الشريط (`anon-guard-pq`،
`anon-majority-pq`, `anon-strict-pq`) يتحلل تلقائيًا
المرحلات الكلاسيكية: عندما يكون حارس PQ متاحًا، يقوم المحدد بإيقافه
دبابيس كلاسيكية للغاية تساعد على تفضيل الجلسات التالية
الهجينة المصافحة. تعرض السيرة الذاتية CLI/SDK المزيج الناتج عبر
`anonymity_status`/`anonymity_reason`، `anonymity_effective_policy`،
`anonymity_pq_selected`، `anonymity_classical_selected`، `anonymity_pq_ratio`،
`anonymity_classical_ratio` والأبطال المرتبطون بالمرشحين/العجز/دلتا دي
العرض، يوضح التخفيضات والتراجعات الكلاسيكية.قد تؤدي أدلة الحراسة إلى تعطيل حزمة SRCv2 مكتملة عبر
`certificate_base64`. تقوم L’orchesstrateur بفك تشفير كل حزمة، وإعادة تقييمها
التوقيعات Ed25519/ML-DSA والحفاظ على تحليل الشهادة من خلال أجزاء ذاكرة التخزين المؤقت
دي حراس. عندما تكون الشهادة موجودة، فإنها ترجع إلى المصدر القانوني
مفاتيح PQ وتفضيلات المصافحة والتفكير ; الشهادات
انتهاء الصلاحية يتم حذفه ويعود المحدد إلى الأبطال الموروثة من الواصف.
يتم نشر الشهادات في إدارة دورة حياة الدوائر وما إلى ذلك
يتم الكشف عن ذلك عبر `telemetry::sorafs.guard` و`telemetry::sorafs.circuit`، وهو
إرسال نافذة الصلاحية ومجموعات المصافحة والملاحظة
التوقيعات غير المزدوجة من أجل حارس Chaque.

استخدم مساعدات CLI لحماية اللقطات المتوافقة مع المحررين:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

`fetch` قم بتنزيل لقطة SRCv2 والتحقق منها قبل كتابة القرص،
ثم يتم إصدار `verify` خط الأنابيب للتحقق من صحة العناصر
d'autres équipes، مما يؤدي إلى سيرة ذاتية JSON تعكس عملية الاختيار
حراس CLI/SDK.

### 1.5 إدارة دورة حياة الدوائرعندما يتم توفير دليل المرحلات وذاكرة التخزين المؤقت للمدير
نشط إدارة دورة حياة الدوائر من أجل البناء المسبق وما إلى ذلك
تجديد الدوائر SoraNet كل ما هو جديد. تم العثور على التكوين
sous `OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) عبر deux
الأبطال الجدد :

- `relay_directory`: نقل لقطة الدليل SNNet-3 من أجلها
  يقول الأوسط/الخروج soient sélectionnés de manière déterminist.
- `circuit_manager`: خيار التكوين (التنشيط الافتراضي) للتحكم في
  TTL للدوائر.

Norito يقبل JSON اختلال الكتلة `circuit_manager` :

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

يقوم SDK بنقل بيانات الدليل عبر
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`)، وCLI للكابل تلقائيًا
`--guard-directory` هو المورد (`crates/iroha_cli/src/commands/sorafs.rs:365`).

تعمل الإدارة على تجديد الدوائر عند تغيير عمليات الحماية
(نقطة النهاية، اضغط على PQ أو الطابع الزمني للطباعة) أو عند انتهاء صلاحية TTL. لو المساعد
`refresh_circuits` جلب الجلب الأمامي (`crates/sorafs_orchestrator/src/lib.rs:1346`)
تم إنشاء السجلات `CircuitEvent` حتى يتمكن المشغلون من تتبع القرارات
Liées au دورة الحياة. اختبار لو نقع
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) يعمل على استقرار زمن الاستجابة
على مدار ثلاث دورات حراسة؛ voir le Rapport associé dans
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 وكيل QUIC محلييمكن للمنسق أن يطلق وكيل QUIC محليًا للوصول إليه
لا تساعد ملحقات التنقل ومحولات SDK في إدارة الشهادات
لا توجد مفاتيح لذاكرة التخزين المؤقت. الوكيل يقع في عنوان استرجاع، ينتهي
les connexions QUIC وأعد بيان Norito من خلال الشهادة والشهادة
مفتاح حماية ذاكرة التخزين المؤقت للعميل. أحداث النقل تنطلق بالتساوي
يتم حساب الوكيل عبر `sorafs_orchestrator_transport_events_total`.

قم بتنشيط الوكيل عبر الكتلة الجديدة `local_proxy` في JSON de l’orchesstrateur:

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
```- `bind_addr` للتحكم في عنوان الوكيل (استخدم المنفذ `0` من أجل
  طالب بمنفذ éphémère).
- `telemetry_label` يتم نشر المقاييس لتمييز لوحات المعلومات
  بروكسيات جلسات الجلب.
- `guard_cache_key_hex` (اختياري) يسمح بالوكيل لعرض نفس ذاكرة التخزين المؤقت
  يحرس مفتاح CLI/SDK، ويضمن تصفح الامتدادات المحاذاة.
- `emit_browser_manifest` يقوم بإرجاع بيان الإضافات
  يمكن أن يكون مخزنًا وصحيحًا.
- `proxy_mode` اختر ما إذا كان الوكيل يعيد حركة المرور المحلية (`bridge`) أو
  لا يوجد أي بيانات لكي تفتح SDK محاكاة للدوائر
  سورانت (`metadata-only`). تم تعيين الوكيل افتراضيًا على `bridge`؛ com.choisissez
  `metadata-only` عندما يقوم البريد بكشف البيان بدون ترحيل التدفق.
- `prewarm_circuits`، `max_streams_per_circuit` و`circuit_ttl_hint_secs`
  يعرض تلميحات إضافية أثناء التنقل لموازنة التدفق
  المتوازيات وفهم مستوى إعادة استخدام الدوائر.
- `car_bridge` (اختياري) يشير إلى ذاكرة تخزين مؤقت محلية لأرشيفات CAR. لو البطل
  `extension` يتحكم في اللاحقة التي يتم إضافتها عند تشغيل الكابل `*.car` ; تعريف
  `allow_zst = true` لخدمة الحمولات الصافية `*.car.zst` المضغوطة مسبقًا.
- `kaigi_bridge` (اختياري) يعرض المسارات Kaigi spoolées au proxy. لو البطل`room_policy` أعلن أن الجسر يعمل في الوضع `public` ou
  `authenticated` لتمكين العملاء من تصفح التصنيفات المحددة مسبقًا
  GAR مناسب.
- `sorafs_cli fetch` فضح التجاوزات `--local-proxy-mode=bridge|metadata-only`
  و`--local-proxy-norito-spool=PATH`، يسمح بإلغاء وضع التنفيذ
  أو مؤشر على البكرات البديلة بدون تعديل السياسة JSON.
- `downgrade_remediation` (اختياري) قم بتكوين ربط الرجوع إلى إصدار أقدم تلقائيًا.
  عندما يكون نشطًا، يقوم المنسق بمراقبة المرحلات عن بعد من أجل
  اكتشاف طائرات الرافال من الإصدار السابق، بعد تجاوز `threshold` في
  نافذة `window_secs`، فرض الوكيل المحلي على `target_mode` (افتراضيًا
  `metadata-only`). بعد توقف التخفيضات، يعود الوكيل مرة أخرى
  `resume_mode` بعد `cooldown_secs`. استخدم اللوحة `modes` لتحديد
  Le déclencheur à des rôles de Relay spécifiques (par défaut les Relays d'entrée).

عند تشغيل الوكيل في وضع الجسر، يتم تقديم تطبيقات خدمات مزدوجة:- **`norito`** – يتم حل سلك تدفق العميل حسب العلاقة معه
  `norito_bridge.spool_dir`. يتم تعقيم الأقطاب (pas de traversal، pas de
  chemins absolus)، وعندما لا يكون هناك امتداد للملف، يتم تكوين اللاحقة
  تم تطبيقه قبل بث الحمولة على المتصفح.
- **`car`** – يتم حل أسلاك التدفق في `car_bridge.cache_dir`، بشكل متكرر
  de l’extension par défaut configurée و rejettent lesحمولات المضغوطة sauf
  إذا تم تنشيط `allow_zst`. تستجيب الجسور لإعادة الاستخدام مع `STREAM_ACK_OK`
  قبل نقل ثمانيات الأرشيف حتى يكون العملاء قادرين على ذلك
  خط أنابيب التحقق.

في الحالتين، يزود الوكيل HMAC بعلامة ذاكرة التخزين المؤقت (عند مفتاح ذاكرة التخزين المؤقت)
était présente lors du handshake) وقم بتسجيل رموز سبب القياس عن بعد
`norito_*` / `car_*` لتمييز لوحات المعلومات عن انقلابها
النجاح والملفات القديمة وتأثيرات التعقيم.

`Orchestrator::local_proxy().await` يعرض المقبض في الاتجاه المعاكس
يستطيع المستأنفون قراءة PEM للشهادة، واسترداد بيان التنقل
أو تطلب إيقافًا رائعًا لإغلاق التطبيق.

عند تنشيط الوكيل، سيتم تعطيل التسجيلات **البيان v2**.
بالإضافة إلى الشهادة الموجودة ومفتاح حماية ذاكرة التخزين المؤقت، يضيف الإصدار الثاني:- `alpn` (`"sorafs-proxy/1"`) ولوحة `capabilities` للعملاء
  تأكيد استخدام بروتوكول التدفق.
- Un `session_id` من خلال المصافحة وكتلة من المبيعات `cache_tagging` للإخراج
  حماية تقاربات الجلسة وعلامات HMAC.
- تلميحات الدائرة واختيار الحماية (`circuit`، `guard_selection`،
  `route_hints`) لكي تعرض عمليات التكامل واجهة مستخدم أكثر ثراءً
  قبل فتح التدفق.
- `telemetry_v2` مع مقابض التشذيب والسرية من أجل
  لغة الأجهزة.
- تشاك `STREAM_ACK_OK` بما في ذلك `cache_tag_hex`. يعكس العملاء القيمة
  في `x-sorafs-cache-tag` على الرغم من طلبات HTTP أو TCP لتتمكن من ذلك
  يتم حفظ التحديدات المحمية في ذاكرة التخزين المؤقت في مكانها الصحيح.

Ces champs s’ajoutent au manière v2 ; العملاء يفعلون ذلك المستهلكون
قم بتوضيح المفاتيح التي تدعمها وتتجاهل الباقي.

## 2. دلالة المؤثرات

يطبق المنسق عمليات تحقق صارمة على القدرات والميزانيات
avant de transférer le moindre octet. الأدوات الموجودة في ثلاث فئات:1. ** شيك الأهلية (مجلد سابق).** الموردون بلا قدرة على الشاطئ،
   يتم إرسال الإعلانات المنتهية الصلاحية أو البث المباشر إلى القطعة الأثرية
   لوحة النتائج وحذف التخطيط. تمثل السيرة الذاتية CLI
   اللوحة `ineligible_providers` مع الأسباب التي تجعل المشغلين
   قم بفحص مشتقات الحكم دون كشط السجلات.
2. **التنفيذ للتنفيذ.** كل ما يناسب الإجراءات التنفيذية.
   مرة واحدة `provider_failure_threshold` تم تحديد المورد
   `disabled` لبقية الجلسة. إذا كان جميع الموردين موجودين
   `disabled`، منسق الموسيقى
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. **التوقيفات المحددة.** الحدود القاسية هي شكل من أشكال الأخطاء
   الهياكل :
   - `MultiSourceError::NoCompatibleProviders` - يتطلب البيان نطاقًا واسعًا
     قطع أو محاذاة لا يمكن للموردين المتبقين تكريمها.
   - `MultiSourceError::ExhaustedRetries` — ميزانية إعادة المحاولة بمقدار قطعة واحدة
     com.consomé.
   - `MultiSourceError::ObserverFailed` — المراقبون في اتجاه مجرى النهر (خطافات دي
     دفق) تم رفض قطعة تم التحقق منها.

قد يكون هناك خطأ في إغلاق فهرس القطعة الخاطئة، عندما يكون متاحًا، السبب
Finale d’échec du fournisseur. قم بمعالجة هذه الأخطاء مثل حواجز التحرير
— المحاولات بنفس الطريقة يتم إعادة إنتاجها مثل الإعلان،
القياس عن بعد أو صحة مقدم الخدمة الجديد لا يتغير.### 2.1 ثبات لوحة النتائج

عندما يتم تكوين `persist_path`، يقوم المنسق بكتابة لوحة النتائج النهائية
تشغيل كل مرة بعد ذلك. محتوى المستند JSON :

- `eligibility` (`eligible` أو `ineligible::<reason>`).
- `weight` (poidsnormalisé signé pour ce run).
- métadonnées du `provider` (المعرف، نقاط النهاية، ميزانية التزامن).

قم بأرشفة لقطات لوحة النتائج باستخدام العناصر التي تم إصدارها حتى تتمكن من ذلك
اختيار القائمة السوداء والطرح يظل قابلاً للتدقيق.

## 3. القياس عن بعد والتصحيح

### 3.1 المقاييس Prometheus

يقوم المُنسق بإخراج المقاييس التالية عبر `iroha_telemetry` :| متريك | التسميات | الوصف |
|---------|-------|-------------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`، `region` | مقياس جلب الأوركسترا بالمجلد. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`، `region` | الرسم البياني يسجل زمن الوصول لجلب النوبة في كل نوبة. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`، `region`، `reason` | Compteur des échecs terminaux (إعادة المحاولة، أو المورد، أو المراقب). |
| `sorafs_orchestrator_retries_total` | `manifest_id`، `provider`، `reason` | حاسب محاولات إعادة المحاولة من قبل المورد. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`، `provider`، `reason` | قم بقياس نتائج المورد على مستوى الجلسة قبل إلغاء التنشيط. |
| `sorafs_orchestrator_policy_events_total` | `region`، `stage`، `outcome`، `reason` | حساب القرارات السياسية المجهولة (المدة مقابل التوقف) مجمعة على أساس مرحلة الطرح وسبب التراجع. |
| `sorafs_orchestrator_pq_ratio` | `region`، `stage` | رسم بياني لجزء من مرحلات PQ في مجموعة SoraNet المحددة. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`، `stage` | الرسم البياني لنسب عرض التبديلات PQ في لقطة لوحة النتائج. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`، `stage` | رسم بياني للعجز السياسي (مرسوم بين الهدف والجزء PQ الحقيقي). || `sorafs_orchestrator_classical_ratio` | `region`، `stage` | رسم بياني لجزء من المرحلات الكلاسيكية المستخدمة في كل جلسة. |
| `sorafs_orchestrator_classical_selected` | `region`، `stage` | رسم بياني لحسابات المرحلات الكلاسيكية المختارة لكل جلسة. |

قم بدمج هذه المقاييس في لوحات المعلومات المرحلية قبل تنشيط المقابض
أون الإنتاج. الترتيب الموصى به يعكس خطة المراقبة SF-6 :

1. **جلب العناصر** — تنبيه إذا كان المقياس بدون اكتمال المراسلات.
2. **نسبة المحاولات** — تجنب تجاوز أجهزة الكمبيوتر `retry`
   خطوط الأساس التاريخية.
3. **موردي الشيكات** — إلغاء تنبيهات النداء عند وجود مورد
   مرر `session_failure > 0` في نافذة لمدة 15 دقيقة.

### 3.2 سلاسل من هياكل السجلات

ينشر المنسق الأحداث المنظمة مقابل cibles المحددة :

- `telemetry::sorafs.fetch.lifecycle` — العلامات `start` و`complete` مع
  عدد القطع، وإعادة المحاولة، والمدة الإجمالية.
- `telemetry::sorafs.fetch.retry` — أحداث إعادة المحاولة (`provider`, `reason`,
  `attempts`) للتحليل اليدوي.
- `telemetry::sorafs.fetch.provider_failure` — مزودي التعطيل
  الأخطاء المتكررة.
- `telemetry::sorafs.fetch.error` — إنهاء السيرة الذاتية مع `reason` وآخرون
  métadonnées optionsnelles du fournisseur.قم بإجراء هذا التدفق عبر خط أنابيب السجلات Norito الموجود حتى الاستجابة
تعتبر الحوادث مصدرًا فريدًا للحقيقة. أحداث دورة الحياة
تعرض لو ميكس PQ/classique عبر `anonymity_effective_policy`،
`anonymity_pq_ratio`، `anonymity_classical_ratio` والحسابات المرتبطة بها،
مما يسهل كابلات لوحات العدادات بدون كشط المقاييس. قلادة
عمليات الإطلاق GA، تغلق مستوى السجلات على `info` لأحداث الدورة
قم بالتشغيل/إعادة المحاولة واستخدم `warn` للأخطاء الطرفية.

### 3.3 السيرة الذاتية JSON

يقدم `sorafs_cli fetch` وSDK Rust سيرة ذاتية تحتوي على هيكل:

- `provider_reports` مع أجهزة الكمبيوتر الناجحة/التحقق وحالة إلغاء التنشيط.
- `chunk_receipts` من التفاصيل التي توفر قطعة مرضية.
- اللوحات `retry_stats` و`ineligible_providers`.

أرشفة ملف السيرة الذاتية لتصحيح مقدمي الخدمة الفاشلين:
الإيصالات Mappent Directement aux métadonnées de logs ci-dessus.

## 4. قائمة المراجعة التشغيلية1. **قم بإعداد التكوين في CI.** قم بتمرير `sorafs_fetch` مع ذلك
   سلك التكوين، قم بتمرير `--scoreboard-out` لالتقاط الرؤية
   الأهلية، ويحدث فرقًا مع الإصدار السابق. جميع المجهزين
   غير مؤهل لمنع الترقية.
2. **التحقق من البيانات عن بعد.** تأكد من تصدير النشرات
   المقاييس `sorafs.fetch.*` والسجلات التي تم إنشاؤها قبل تنشيط عمليات الجلب
   مصادر متعددة للمستخدمين. يشير غياب المقاييس إلى الشعور بالجوع
   que la facade de l’orchesstrateur n’a pas été appelée.
3. **توثيق التجاوزات.** Lors d’un `--deny-provider` ou `--boost-provider`
   عاجل، قم بإرسال JSON (أو استدعاء CLI) في سجل التغيير. ليه
   يجب أن تؤدي عمليات التراجع إلى إلغاء التجاوز والتقاط لقطة جديدة
   لوحة النتائج.
4. **إعادة اختبارات الدخان.** بعد تعديل ميزانيات إعادة المحاولة أو المحاولة
   قبعات الموردين، قم بإعادة تثبيت الأداة Canonique
   (`fixtures/sorafs_manifest/ci_sample/`) وتحقق من إيصالات القطع
   المحددات الباقية.

متابعة الخطوات التالية للحفاظ على سلوك المنسق
قابلة للتكرار في عمليات النشر على مراحل وتوفر القياس عن بعد الضروري
الرد على الحوادث.

### 4.1 تجاوزات السياسةيمكن للمشغلين أن يتدخلوا في مرحلة النقل/عدم الكشف عن هويتهم بدون نشاط
قم بتعديل التكوين الأساسي المحدد
`policy_override.transport_policy` و`policy_override.anonymity_policy` في
leur JSON `orchestrator` (أو متوفر
`--transport-policy-override=` / `--anonymity-policy-override=` à
`sorafs_cli fetch`). عندما يكون التجاوز موجودًا، يُقلى المُنسق
العادة الاحتياطية للانقطاع : إذا كانت مستوى PQ المطلوب قد لا تكون مرضية،
يتم جلب الصوت باستخدام `no providers` بدلاً من خفض مستوى الصمت. لو
إن العودة إلى السلوك الافتراضي تتكون ببساطة من مشاهدة الأبطال
تجاوز.