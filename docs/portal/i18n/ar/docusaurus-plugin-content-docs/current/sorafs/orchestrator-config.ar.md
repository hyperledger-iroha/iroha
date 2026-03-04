---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/orchestrator-config.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: تكوين الأوركسترا
العنوان: إعدادات مُنسِّق SoraFS
Sidebar_label: إعدادات المُنسِّق
الوصف: ضبط مُنسِّق جلب المعنى المتعدد، وفسّر الخفاقات، وتتبع مخرجات التليميترية.
---

:::ملحوظة المصدر مؤهل
احترام هذه الصفحة `docs/source/sorafs/developer/orchestrator.md`. احرص على جميع النسختين متزامنتين إلى أن يتم سحب مجموعة التوثيق القديمة.
:::

# دليل مُنسِّق جلب المعنى المتعدد

يقود مُنسِّق جلب الموارد المتعددة في SoraFS تنزيلات حتمية ومتوازنة من المجموعة
المنسقة المنشورة في الإعلانات والمساهمين بالمساهمة. يشرح هذا الدليل كيفية ضبط
المُنسِّق، وما هم مساهمون خلال العمليات غير المكتملة، وأي تدفقات
التليميترية تشير إلى الصحة.

## 1. نظرة عامة على العداد

يدمج المُنسِّق ثلاثة مصادر للاعداد:

| المصدر | اللحوم | مذكرة |
|--------|-------|-----------|
| `OrchestratorConfig.scoreboard` | يطبّع الوزن المقياسين، ويتحقق من الأحداث التليميترية، ويحفظ نتائج لوحة JSON المستخدمة للدقيق. | مدعوم عبر `crates/sorafs_car::scoreboard::ScoreboardConfig`. |
| `OrchestratorConfig.fetch` | يطبّق بحدود وقت التشغيل (ميزانيات إعادة المحاولة، بحدود التوازن، مفاتيح التحقق). | يُطابق `FetchOptions` في `crates/sorafs_car::multi_fetch`. |
| معاملات CLI / SDK | تحد عدد النظراء، ولصق المناطق التليميرية، وسياسات الإنكار/التعزيز. | `sorafs_cli fetch` يتواجد هذه الأعلام ؛ ويتمرّر عبر `OrchestratorConfig` في SDKs. |

تقوم بمساعدة JSON في `crates/sorafs_orchestrator::bindings` بالتسلسل الكامل الاعداد
إلى Norito JSON، ما يجعل قابلاً للنقل بين ربط الـ SDK والطباعة.

### 1.1 مثال إعدادات JSON

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
```احفظ الملف عبر الطبقات `iroha_config` المعتاد (`defaults/`، مستخدم، فعلي) حتى
ترث عمليات النشر الحتمية الحدودية لنفسها في جميع الحفلات. لملف احتياطي مباشر فقط
يتماشى مع الطرح الخاص بـ SNNet-5a، راجع
`docs/examples/sorafs_direct_mode_policy.json` والإرشاد المكمل في
`docs/source/sorafs/direct_mode_pack.md`.

### 1.2 تجاوزات اخرى

تُدخل SNNet-9 إلى المحكوم بالشبكة داخل المُنسِّق. كائن `compliance` جديد
في الإعدادات Norito JSON يلتقط مسار الاستثناءات التي لا يمكن جلبها إلى المباشر فقط:

```json
"compliance": {
  "operator_jurisdictions": ["US", "JP"],
  "jurisdiction_opt_outs": ["US"],
  "blinded_cid_opt_outs": [
    "C6B434E5F23ABD318F01FEDB834B34BD16B46E0CC44CD70536233A632DFA3828"
  ],
  "audit_contacts": ["mailto:compliance@example.org"]
}
```

- `operator_jurisdictions` أعلن عن رموز ISO‑3166 alpha‑2 التي تعمل هنا
  نسخة من المُنسِّق. تُريد كتابة كتب إلى أحرف كبيرة أثناء التحليل.
- `jurisdiction_opt_outs` يعكس سجل التسجيل. عندما يعيش أي ولاية تشغيلية ضمنًا
  قائمة، يفرض المنسِّق `transport_policy=direct-only` ولا سبب يصدر السماء
  `compliance_jurisdiction_opt_out`.
- `blinded_cid_opt_outs` يسرد الملخصات المانيفست (CIDs المعماة بصيغة ست عشرية)
  بأحرف كبيرة). التطابقات المحددة تحديداً مباشرة فقط وظهرت
  `compliance_blinded_cid_opt_out` في التليميترية.
- `audit_contacts` تسجل عناوين URI التي تتوقع الـ تور أن ينشرها وتشغيلها في
  كتب اللعب الخاصة بـ GAR.
- `attestations` يلتقط حزم الموقّعة الباهتة للسياسة. كل النص يعرّف
  `jurisdiction` اختياريًا (ISO-3166 alpha-2)، و`document_uri`، و`digest_hex`
  سيج رقم 64 حرفاً، وطابعة الإصدار `issued_at_ms`، و`expires_at_ms`
  اختياريا. تتدفق هذه الآثار إلى قائمة تدقيق المُنسِّق الرئيسية لأدوات التشارك
  بين التجاوزات والوثائق الموقع.مرّرت الكتلة عبر الطبقات الاعدادية حتى تتمكن من التجاوز
حتمية. يطبق المُنسِّق ضد _بعد_ تلميحات وضع الكتابة: حتى لو طلب SDK
`upload-pq-only`، استثناءات أو مانيفيست النقل المباشر فقط
وتفشل سريعاً عندما لا يوجد مراقبون متوافقون.

هناك كتالوجات إلغاء الاشتراك المعتمدة في
`governance/compliance/soranet_opt_outs.json`؛ وينشر مجلس التحديثات عبر
إصدار موسومة. المثال المثال كامل للاعداد (بما في ذلك الشهادات) في
`docs/examples/sorafs_compliance_policy.json`، كما تقوم بإثبات المسار التشغيلي
[دليل قواعد اللعبة لا يبدو GAR](../../../source/soranet/gar_compliance_playbook.md).

### 1.3 مفاتيح CLI وSDK| العلم / الحقل | الاثر |
|-------------|-------|
| `--max-peers` / `OrchestratorConfig::with_max_providers` | يقتصر عدد المتحكمين الذين يمرون من قائمة نتائج لوحة التصفية. ضبطه على `None` لاستخدام كل المحاسبين المؤهلين. |
| `--retry-budget` / `FetchOptions::per_chunk_retry_limit` | يحد من محاولة لكل شريحة. تجاوز الحد رفع `MultiSourceError::ExhaustedRetries`. |
| `--telemetry-json` | يحقن لقطات الكمون/الفشل في مُنشئ نتائج اللوحة. التليميترية الأقدم من `telemetry_grace_secs` تجعل المتحكمين غير مؤهلين. |
| `--scoreboard-out` | يحفظ لوحة النتائج المحددة (م مؤهلون مؤهلون + غير مؤهلين) للفحص بعد التشغيل. |
| `--scoreboard-now` | تجاوز نتائج لوحة المفاتيح (ثواني Unix) لكي تستمر لقطات التركيبات حتمية. |
| `--deny-provider` / درجة الخطاف | يستبعد المتحكمون بشكل حتمي دون حذف الإعلانات. مفيد للمنع السريع. |
| `--boost-provider=name:delta` | يضبط أرصدة الموزونة المستديرة مع نظام البقاء على الأوزان. |
| `--telemetry-region` / `OrchestratorConfig::with_telemetry_region` | يوسم المقاييس والسجلات المهيكلة لوحات الخطوط الناشئة من التجميع حسب الجغرافيا أو التغيير الجذري. |
| `--transport-policy` / `OrchestratorConfig::with_transport_policy` | الافتراضي `soranet-first` الآن بعد أن أصبح المُنسِّق متعدد المصدر بشكل أساسي. استخدم `direct-only` عند التحضير لخفض أو اتوماتيك، واحجز `soranet-strict` لتجارب PQ فقط؛ لتجاوزات لتتناسب مع التصميم الجديد. |SoraNet-first هو الافتراضي القابل لإعادة الشحن الحالي، ويجب أن تتذكر شخصيات مانع SNNet
مع ذلك. بعد تاخر الاتجاه SNNet-4/5/5a/5b/6a/7/8/12/13 ستشيد التحول إلى الاتجاه
(نحو `soranet-strict`)، وحتى ذلك لا بد أن تضطر إلى تجاوز `direct-only` على
تم تسجيل حالات التعامل مع الحوادث مع تسجيلها في السجل.

كلية الأعلام مقبولة `--` في كل من `sorafs_cli fetch` والثنائي
`sorafs_fetch`. وتعرض خيارات SDKs لنفسها عبر هيكلة البنائين.

### 1.4 إدارة ذاكرة التخزين المؤقت للحراس

لتتمكن الآن CLI الآن من إدارة حراس SoraNet بحيث يمكن للمشغلين تثبيت مرحلات الدخول
بشكل كامل حتمي قبل طرح الإصدار الكامل لـ SNNet-5. ثلاثة أعلام جديدة تتحكم
بالسير:

| علم | اللحوم |
|------|-------|
| `--guard-directory <PATH>` | يشير إلى ملف JSON يصف أحدث الاتصالات للمرحلات (الجزء منه أدناه). تيشير الدليل إلى تحديد ذاكرة التخزين المؤقت قبل الجلب. |
| `--guard-cache <PATH>` | يحفظ `GuardSet` المشفر بنوريتو. عمليات التشغيل التالية لتنظيف ذاكرة التخزين المؤقت حتى بدون دليل جديد. |
| `--guard-target <COUNT>` / `--guard-retention-days <DAYS>` | تجاوزات اختيارية لعدد تسجيل الدخول المسجلين (الافتراضي 3) وفترة الافتراض (الافتراضي 30 يومًا). |
| `--guard-cache-key <HEX>` | مفتاح اختياري بطول 32 بايت لوضع MAC Blake3 على ذاكرة التخزين المؤقت الأمنية حتى يمكن التحقق منها من قبل إعادة الاستخدام. |

إستخدام تكتيكي للتحكم في أداة التحكم الذكية:

أخذ علم `--guard-directory` الآن فاعل `GuardDirectorySnapshotV2` مشفرة بنوريتو.
تحتوي على اللقطة الثنائية على:- `version` — نسخة محتملة (حاليًا `2`).
- `directory_hash`، `published_at_unix`، `valid_after_unix`، `valid_until_unix` —
  يجب أن تتطابق البيانات مع كل علاقة مضمنة.
- `validation_phase` — بوابة الشهادات (`1` = السماح بالتوقيع Ed25519 واحد،
  `2` = تفضيلات التوقيعات المزدوجة، `3` = اشتراط التوقيعات المزدوجة).
- `issuers` — جهات الإصدار الحوكمية مع `fingerprint`, `ed25519_public`,
  `mldsa65_public`. تُحسب البصمات:
  `BLAKE3("soranet.src.v2.issuer" || ed25519 || u32(len(ml-dsa)) || ml-dsa)`.
- `relays` — قائمة بحزم SRCv2 (خرج `RelayCertificateBundleV2::to_cbor()`). تحمل
  كل حزمة واصف تتابع وأعلام الهان وسياسة ML-KEM والتوقيعات Ed25519/ML-DSA-65
  .

تتحقق CLI من كل حزمة مقابل المفاتيح المصدرة قبل دمج الدليل مع ذاكرة التخزين المؤقت
الحراس. لم تعد الرسومات JSON القديمة مقبولة؛ مطلوب لقطات SRCv2.

التمتعِ بـ CLI مع `--guard-directory` لتوافق الاتصال المدمج مع الـذاكرة التخزين المؤقت الحالية.
مراقبة الحراسة الموثقة ضمن نافذة الإصلاح والمؤهلين في الدليل؛
وإستبدال المرحلات الجديدة الإدخالات المنتهية. بعد نجاح الجلب تُكتب ذاكرة التخزين المؤقت
الحدث في المنهج النشط عبر `--guard-cache` في حتمية الجلسات التالية.
يمكن لـ SDK إعادة إنتاج هذا السلوك عبر
`GuardSelector::select(&RelayDirectory, existing_guard_set, now_unix_secs)` وتمرير
`GuardSet` الناتج إلى `SorafsGatewayFetchOptions`.`ml_kem_public_hex` يمكّننا من تفضيل العملاء القادرين على PQ أثناء الطرح
سنيت-5. مفاتيح المراحل (`anon-guard-pq`, `anon-majority-pq`,
`anon-strict-pq`) الآن على المرحلات الشعبية الكلاسيكية التلقائية: عندما يسمى الجندي
يقوم PQ بإجراء عملية إسقاط الأغطية الهجينة بشكل أفضل لتفضيل المصافحات.
تتضمن ملخصات CLI/SDK الموجودة عبر `anonymity_status`/`anonymity_reason`,
`anonymity_effective_policy`، `anonymity_pq_selected`، `anonymity_classical_selected`،
`anonymity_pq_ratio`, `anonymity_classical_ratio` وقول المقاول/العجز/فارق
العرض، ما يجعل brownouts وfallbacks كلاسيكيًا.

يمكن لأدلة الحراس الآن إدراج حزمة SRCv2 كاملة عبر `certificate_base64`. يفك
المُنسِّق كل حزمة ويعيد التصديق من تواقيع Ed25519/ML-DSA ويحتفظ بالشهادة
اختر مخبأة الاستشعار. عند وجود مؤهل ليصبح مؤهلاً لمفاتيح PQ
وتفضيلات المصافحة والترجيح؛ وينتهي الأمر بالنتائج ويعود اكتشافها
الوصف القديم. الحصول على الشهادات عبر إدارة دورة الحياة والعرض عبر
`telemetry::sorafs.guard` و`telemetry::sorafs.circuit` التي رجحت النافذة
وحزم المصافحة وما إذا كانت توقيعات مزدوجة لكل النجوم.

استخدم مساعدات CLI الجوية على تزامن جزء مع الناشرين:

```bash
sorafs_cli guard-directory fetch \
  --url https://directory.soranet.dev/mainnet_snapshot.norito \
  --output ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>

sorafs_cli guard-directory verify \
  --path ./state/guard_directory.norito \
  --expected-directory-hash <directory-hash-hex>
```

ينزّل `fetch` اللقطة SRCv2 ويتحقق منها قبل الكتابة على القرص، بينما يعيد
`verify` خط التحقق لآثار مصدرها فرق آخر، ويتوفر ملخص JSON بوضوح صدر
الحراس المحددون في CLI/SDK.

### 1.5 مدير دورة حياة الدائرةعندما يفترض دليل Relays وcache Senens معاً، يقوم المُنسِّق بتفعيل مدير دورة
حياة إنشاء دائرة وتجديد دوائر SoraNet قبل كل جلب. إغلاق الاعداد ضمن
`OrchestratorConfig` (`crates/sorafs_orchestrator/src/lib.rs:305`) عبر بحثين جديدين:

- `relay_directory`: ضغط لقطة دليل SNNet-3 كي يتم اختيار قفزات الوسط/الخروج بشكل عام
  حتمي.
- `circuit_manager`: إعدادات اختياري (متاح افتراضياً) للتحكم في TTL للدوائر.

يقبل Norito JSON الآن كتلة `circuit_manager`:

```json
"circuit_manager": {
  "enabled": true,
  "circuit_ttl_secs": 900
}
```

اتصالات SDKs بيانات الدليل عبر
`SorafsGatewayFetchOptions::relay_directory`
(`crates/iroha/src/client.rs:320`)، يريد CLI أن يريده حتماً عند الرغبة
`--guard-directory` (`crates/iroha_cli/src/commands/sorafs.rs:365`).

يجدد المدير دواعي تغيير كل البيانات الجارديان (نقطة النهاية أو مفتاح PQ أو الطابع
مدة التثبيت) أو عند انتهاء TTL. يقوم المساعد المستدعى `refresh_circuits`
قبل كل جلب (`crates/sorafs_orchestrator/src/lib.rs:1346`) بإصدار السجلات
`CircuitEvent` يبدأ تشغيل عملية تتبع دورة الحياة. اختبار نقع
`circuit_manager_latency_soak_remains_stable_across_rotations`
(`crates/sorafs_orchestrator/src/soranet.rs:1479`) استقرار الكمون عبر ثلاث
دورة تغيير للحراس؛ تقرير تقرير في
`docs/source/soranet/reports/circuit_stability.md:1`.

### 1.6 وكيل QUIC المحلي

يمكن تشغيل وكيل QUIC اختياريًا محليًا بحيث لا يلزم إضافة إصدار
ومحولات SDK إلى إدارة الشهادات أو مفاتيح التخزين المؤقت للأمناء. المرتبطة بعنوان
الاسترجاع، وينهي اتصالات QUIC، ويعيد البيان من Norito وصف التعريف والمفتاح
ذاكرة التخزين المؤقت الاختيارية للعميل. أحداث الأحداث التي تصدرها المدير ضمن
`sorafs_orchestrator_transport_events_total`.

فعّل الوكيل عبر الكتلة `local_proxy` الجديد في JSON الخاص بالمُنسِّق:

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
```- `bind_addr` يتحكم بمكان استماع خارجي (استخدم المنفذ `0` لطلب منفذ مؤقت).
- `telemetry_label` تمر إلى المعايير لتمييز الوكلاء عن اجتماعات الجلب.
- `guard_cache_key_hex` (اختياري) يسمح للوكيل بإخفاء إعدادات التشفير بالمفتاح الحالي
  التي تعتمد على CLI/SDKs، لتبقى إصدارات متزامنة.
- `emit_browser_manifest` يبدّل ما إذا كان المصافحة غير واضحة يمكن
  للامتدادات حفظه والتحقق منه.
- `proxy_mode` يحدد ما إذا كان المدير التنفيذي لجسر محلياً (`bridge`) أو يكتفي
  بإصدار بيانات وصفية (`metadata-only`) كي تفتح SDKs SoraNet بنفسك.
  افتراضي `bridge`; استخدم `metadata-only` عندما تريد إظهار البيان فقط.
- `prewarm_circuits`، `max_streams_per_circuit`، `circuit_ttl_hint_secs`
  تلميحات إضافية للمتصفح لتقدير التوازي والفهم إعادة استخدام الدوائر.
- `car_bridge` (اختياري) يشير إلى ذاكرة التخزين المؤقت المحلية لأرشيفات CAR. يتحكم في البحث `extension`
  في اللاحقة اللاحقة عند غياب `*.car`؛ واضبط `allow_zst = true` لخدمة
  `*.car.zst` المعروضة مباشرة.
- `kaigi_bridge` (اختياري) يتعرف على استثمارات Kaigi للوكيل. أعلن `room_policy` ما إذا
  كان الجسر يعمل بشكل `public` أو `authenticated` حتى لا يصدر إصدار GAR
  أهمها مسبقاً.
- `sorafs_cli fetch` يوفّر `--local-proxy-mode=bridge|metadata-only` و
  `--local-proxy-norito-spool=PATH` لتبديل تغيير نظام التشغيل تعيين أو استبدال المكبات
  دون تعديل ل JSON.
- `downgrade_remediation` (اختياري) يضبط خطاف الاتجاه البديل. عند التفعيل
  يراقب المُنسِّق التتابعي التتابعي لرصد اندفاعات التخفيضات، وبعد تجاوز`threshold` خلال `window_secs` يفرض على الوكيل الانتقال إلى `target_mode`
  (الافتراضي `metadata-only`). وبعد توقف التخفيض يعود الوكيل إلى `resume_mode`
  بعد `cooldown_secs`. استخدم مصفوفة `modes` لقصر التفعيل على أدوار التتابع آفة
  (افتراضياً مرحلات الدخول).

عندما يعمل بشكل غير رسمي خادمة الجسر تخدمتينا:

- **`norito`** — يتم حل هدف البث الخاص بميل نسبةً إلى
  `norito_bridge.spool_dir`. تمكنك من الوصول (لا اجتياز ولا مسارات مطلقة)،
  وعندما لا يحتوي على الملف الممتد قبل تطبيق الحمولة المحددة.
- **`car`** — تحقق أهداف البث في `car_bridge.cache_dir`، وترث الا العديد
  الافتراضي، ورفض التخصيص النشط ما لم يتم تفعيله `allow_zst`. الجسور
  الناجح بـ `STREAM_ACK_OK` قبل نقل التاريخ كي يبدأ العملاء من التنفيذ
  التحقق بالأنابيب.

في الحالتين يتولى المدير التنفيذي HMAC الخاص بـ ذاكرة التخزين المؤقت (عند وجود مفتاح ذاكرة التخزين المؤقت أثناء
المصافحة) ويسجل هدف التليميترية `norito_*` / `car_*` حتى تميز اللوحات
متابعة بين النجاح، وفقدان الملفات، وفشل التنقية بسرعة.

`Orchestrator::local_proxy().await` يقابل المقبض التشغيلي حتى يستدعون
من قراءة شهادة PEM، أو جلب مانيفست للمتصفح، أو إلغاء إيقاف لطيف عند الخروج
التطبيق.

عند التفعيل، قم بعرض الوثائق التنفيذية **البيان الإصدار الثاني**. إضافة إلى عناصر ومفتاح
ذاكرة التخزين المؤقت، محملة v2 ما يلي:- `alpn` (`"sorafs-proxy/1"`) ومصفوفة `capabilities` لتأكيد بروتوكول البث.
- `session_id` لكل مصافحة وكتلة `cache_tagging` لاشتقاق affinity للحراس ووسوم
  HMAC على مستوى البصر.
- تلميحات دوائر الدائرة الخارجية (`circuit`, `guard_selection`, `route_hints`)
  واجهة أغنى قبل فتح البث.
- `telemetry_v2` مع مفاتيح أخذ العينات و خصوصية الأدوات المحلية.
- كل `STREAM_ACK_OK` يشمل `cache_tag_hex`. يعكس العميل القيمة في ترويسة
  `x-sorafs-cache-tag` عند الإصدار يطلب HTTP أو TCP حتى تبقى اختيارات الحراس
  مشفرة عند التخزين.

هذه متوافقة مع الخلف — يمكن أن تتجاهل المفاتيح الجديدة
والاعتماد على المجموعة v1.

## 2.دلالات

يفرض المُنسِّق التحقق صارماً من القدرات والميزانيات قبل نقل أي بايت. تقع
التخفيضات في ثلاث نسخ:1. **إخفاء التجمعات الأهلية (قبل التنفيذ).** المتحكمون الذين يختارون نطاقة، أو
   يتم تسجيلها لمدة طويلة أو مؤقتة في لوحة النتائج ولا يمكن استبعادها
   الجدولة. تملأ ملخصات CLI مصفوفة `ineligible_providers` بالأسباب كي تتمكن
   يقوم بفحص انحراف ال تور دون كشط السجلات.
2. **الاستنزاف أثناء التشغيل.** يتتبع كل جهاز قياس التخفيضات المتتالية. عند قدم
   `provider_failure_threshold` يتم تحديد المتحكم `disabled` لبقية النظر. إذا
   انتقل جميع المتحكمين إلى `disabled` يعيد المُنسِّق
   `MultiSourceError::NoHealthyProviders { last_error, chunk_index }`.
3. ** إجهاضات حتمية.** فترات الحدود الصارمة مثل أخطاء مهيكلة:
   - `MultiSourceError::NoCompatibleProviders` — تتطلب مانيفست مدى شرائح أو
     بما لا يمكن للمنظمين الباقين تلبيتها.
   - `MultiSourceError::ExhaustedRetries` — تم تخفيض تكلفة إعادة المحاولة لكل
     شريحة شريحة.
   - `MultiSourceError::ObserverFailed` — رفض الحساسون المصب (خطاطيف البث)
     شريحة تم التحقق منها.

تتضمن كل أخطاء فهرس المخالفة ووجوده سبب فشل المقياس النهائي. بعض
مع هذه الأسباب كعوّقات الإصدار — إعادة المحاولة بذات المدخلات استعادة حتى ذلك الحين
تضرر الإعلان أو التليميترية أو صحة المرصد الأساسي.

### 2.1 حفظ نتائج اللوحة

عند ضبط `persist_path` كاتب المُنسِّق لوحة النتائج النهائية تشغيل بعد كل. يحتوي على
مستند JSON على:

- `eligibility` (`eligible` أو `ineligible::<reason>`).
- `weight` (الوزن المطبع المعين لهذا التشغيل).
- بيانات `provider` الوصفية (المعرّفة، نقاط النهاية، تكلفة التوازن).أرشِف لقطات لوحة النتائج مع الأحداث اللاحقة حتى استمرار عملية الطرح
قابلة للتدقيق.

## 3. التليميترية وتصحيح الاخطاء

### 3.1 معايير Prometheus

يصدر المُنسِّق المعايير التالية عبر `iroha_telemetry`:| المقياس | التسميات | الوصف |
|---------|-------|-------|
| `sorafs_orchestrator_active_fetches` | `manifest_id`، `region` | مقياس العدة للزراعة. |
| `sorafs_orchestrator_fetch_duration_ms` | `manifest_id`، `region` | هيستوغرام يسجل زمن الجلب من طرف إلى طرف. |
| `sorafs_orchestrator_fetch_failures_total` | `manifest_id`، `region`، `reason` | متعددة للإشتراك في اللقاءات (إستدعاء إعادة المحاولة، عدم وجود منظمين، فشل المراقب). |
| `sorafs_orchestrator_retries_total` | `manifest_id`، `provider`، `reason` | تعددت محاولات إعادة المحاولة لكل متحكم. |
| `sorafs_orchestrator_provider_failures_total` | `manifest_id`، `provider`، `reason` | طرق متعددة لتخفيض مستوى الاتصال على مستوى الضبط للتعطيل. |
| `sorafs_orchestrator_policy_events_total` | `region`، `stage`، `outcome`، `reason` | خلقت فكرة إخفاء (إرجاع/إرجاع) وفقا لمرحلة الطرح وسبب الخيال. |
| `sorafs_orchestrator_pq_ratio` | `region`، `stage` | هيستوغرام لحصة Relays PQ ضمن مجموعة SoraNet. |
| `sorafs_orchestrator_pq_candidate_ratio` | `region`، `stage` | هيستوغرام لنسب وجود مرحلات PQ في لقطة اللوحة. |
| `sorafs_orchestrator_pq_deficit_ratio` | `region`، `stage` | هيستوغرام لعجز السياسة (الفجوة بين الهدف والحصة الخاصة). |
| `sorafs_orchestrator_classical_ratio` | `region`، `stage` | هيستوغرام لحصة التتابعات الكلاسيكية في كل جلسة. |
| `sorafs_orchestrator_classical_selected` | `region`، `stage` | هيستوغرام لعدد من التتابعات الكلاسيكية في كل جلسة. |دمج المعايير في لوحات التدريج قبل مفاتيح الإنتاج. التخطيط به
تعكس خطة إمكانية الملاحظة لـ SF-6:

1. **الجلب فيما بعد** — تنبيه إذا قمت بالقياس دون أن تتأكد من اتصالك بالمريض.
2. **نسبة إعادة المحاولة** — تحذير عند تجاوز عدادات `retry` للخطوط الأساسية.
3. **إخفاقات المتحكم** — جهاز النداء عند تجاوز أي متحكم `session_failure > 0`
   خلال 15 دقيقة.

### 3.2 اهداف السجل الهيكل

ينشر المُنسِّق هيكلاً مهيكلاً إلى اهداف حتمية:

- `telemetry::sorafs.fetch.lifecycle` — علامات `start` و`complete` مع عدد المقاعد
  والمحاولات والمدة الاجمالية.
- `telemetry::sorafs.fetch.retry` — احداث إعادة المحاولة (`provider`, `reason`,
  `attempts`) لدعم الفرز.
- `telemetry::sorafs.fetch.provider_failure` — محاسبون تم تسديدهم بسبب خلفي
  الاخطاء.
- `telemetry::sorafs.fetch.error` — اخفاقات نهائية مع `reason` وبيانات متحكم
  اختيارية.

يوجه هذه الملتحقين إلى مسار البروتوكول Norito الحالي للملكية الضرورية للوادث
مصدر حقيقة واحدة. أخذت أحداث دورة الحياة مزيج PQ/الكلاسيكي عبر
`anonymity_effective_policy`، `anonymity_pq_ratio`، `anonymity_classical_ratio`
معاداتها الشاملة، ما نريده هو دمج لوحات بسيطة دون كشط المعايير. أثناء
الطرح الخاص بـ GA، ثبّت مستوى تسجيل على `info` لأحداث دورة الحياة/إعادة
المحاولة واعتمد `warn` للأخطاء النهائية.

### 3.3 ملخصات JSON

إعادة كل من `sorafs_cli fetch` و SDK Rust هيكلاً يحتوي على:- `provider_reports` مع إعداد النجاح/الفشل وما إذا تم تسجيل المسجّل.
- `chunk_receipts` لصالح المتحكم الذي يدفع كل شريحة.
- مصفوفتا `retry_stats` و `ineligible_providers`.

أرشف ملف الملخص عند تتبع متحكمين سيئين — تتطابق الإيصالات مباشرة مع البيانات
سجل فوق.

## 4. تشغيل قائمة التشغيل

1. **حضّر العداد في CI.** شغّل `sorafs_fetch` بالاعداد المستهدف، ومرر
   `--scoreboard-out` العرض التجريبي الأصلي، والمقارنة مع الإصدار السابق. أي منظم
   غير مؤهل بشكل غير متوقع يوقف الترقية.
2. **التحقق من التليميترية.** التأكد من أن التقارير الصادرة مقاييس `sorafs.fetch.*` ستكتشف
   هيكلة سابقة لجلب الموارد المتعددة للمستخدمين. غياب المتطلب المحدد
   إلى أن واجهة المُنسِّق لم تُستدعَ.
3. **وثق التجاوزات.** عند تطبيق إعدادات طارئة `--deny-provider` أو
   `--boost-provider`، ثبّت JSON (أو الاتصال بـ CLI) في سجل التغييرات. يجب أن تهتم
   عملية نسخ لقطة إزالة التجاوز والقاطع للوحة النتائج الجديدة.
4. **أعد تشغيل السيولة الدخان.** بعد تعديل القياسات إعادة المحاولة أو النطاق
   المتحكم، إعادة جلب جلبة التركيب التقليدي (`fixtures/sorafs_manifest/ci_sample/`) وتأكد
   أن إيصالات ما تا حتمية.

اتباع الإجراءات المتبعة المحافظ على إعادة الإنتاج عبر عمليات الطرح العام، كما هو موضح في الشكل التالي
التليميترية اللازمة للاستجابة للحوادث.

### 4.1 تجاوزات السياسةيمكن للمشغلين تثبيت مرحلة النقل/الاخفاء العناصر دون تعديل الاعداد الأساسي عبر
ضبط `policy_override.transport_policy` و`policy_override.anonymity_policy` في
JSON الخاص بـ `orchestrator` (او تسارع `--transport-policy-override=` /
من `--anonymity-policy-override=` إلى `sorafs_cli fetch`). عند وجود أي تجاوز،
تجاوز المُنسِّق الاحتياطي البنيوت العملاق: إذا عذرت تحقيق PQ المطلوبة،
يفشل الجلب مع `no providers` أضف من السماء الصامت. الرجوع للسلوك الافتراضي
يتم ببساطة عبر مسح التجاوز.