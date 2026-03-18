---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/node-client-protocol.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS جديد ↔ بروتوكول الشبكة

هذا هو السعر الأساسي للبروتوكول
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
سألخص الأمر ہے. تخطيطات Norito على مستوى البايت وسجلات التغيير في المنبع
استخدام المواصفات؛ نسخة البوابة SoraFS أدلة التشغيل التي تتضمن أبرز العمليات التشغيلية
قريب رکھتی ہے۔

## إعلانات الموفر والتحقق من صحتها

موفرو SoraFS حمولات `ProviderAdvertV1` (دیکھیں
`crates/sorafs_manifest::provider_advert`) مشغل القيل والقال الذي يحكمه
لا توجد علامة کیے ہوتے ہیں. يتم تثبيت البيانات الوصفية لاكتشاف الإعلانات وحواجز الحماية
يعمل وقت تشغيل المنسق متعدد المصادر على فرض القيود.- **مدى الحياة** — `issued_at < expires_at ≤ issued_at + 86,400 s`. مقدمي الخدمة
  في غضون 12 ثانية، قم بتحديث الصفحة.
- **قدرات TLVs** — تعلن ميزات نقل قائمة TLV عن کرتی ہے (Torii,
  QUIC+ضوضاء، مرحلات SoraNet، ملحقات البائعين). رموز غير معروفة کو
  `allow_unknown_capabilities = true` قم بالتخطي إلى ما هو أبعد من ذلك، يتوافق توجيه GREASE.
- **تلميحات جودة الخدمة** — الطبقة `availability` (ساخن/دافئ/بارد)، الحد الأقصى لزمن وصول الاسترجاع،
  حد التزامن، وميزانية البث الاختيارية. مراقبة جودة الخدمة والقياس عن بعد
  وفقًا لمتطلباتنا ومراجعة القبول في جاتا.
- **نقاط النهاية ومواضيع الالتقاء** — بيانات تعريف TLS/ALPN محددة بشكل ملموس
  عناوين URL للخدمة وموضوعات الاكتشاف جن پر العملاء کو مجموعات الحراسة للفتيات وقت الاشتراك
  كرانا چاہيے۔
- **سياسة تنوع المسار** — `min_guard_weight`، أغطية مراوح AS/pool، و
  `provider_failure_threshold` عمليات الجلب الحتمية متعددة الأقران التي يمكن أن تساعد الفتيات.
- **معرفات الملفات الشخصية** — يعرض الموفرون المقبض الأساسي معلوماتهم الشخصية
  (مثلاً `sorafs.sf1@1.0.0`); اختياري `profile_aliases` عملاء التشغيل
  الهجرة أكثر تنوعا.

قواعد التحقق من الصحة حصة صفر، القدرة الفارغة/نقاط النهاية/قوائم المواضيع، مرتبة بشكل غير صحيح
عمر، أو فقدان أهداف جودة الخدمة يؤدي إلى رفض العناصر. إعلان مظاريف القبول
وهيئات الاقتراح (`compare_core_fields`) کو مقارنة کرتے ہیں پھر تحديثات القيل والقال
كرتے ہيں.

### امتدادات جلب النطاق

يشمل موفرو القدرة على النطاق نطاقًا من البيانات التعريفية:| المجال | الغرض |
|-------|---------|
| `CapabilityType::ChunkRangeFetch` | `max_chunk_span`، `min_granularity` وأعلام المحاذاة/الإثبات تعلن کرتا ہے۔ |
| `StreamBudgetV1` | مظروف التزامن/الإنتاجية الاختياري (`max_in_flight`، `max_bytes_per_sec`، `burst` اختياري). قدرة النطاق المطلوبة ہے۔ |
| `TransportHintV1` | تفضيلات النقل المطلوبة (مثلاً `torii_http_range`، `quic_stream`، `soranet_relay`). الأولويات `0–15` ويتم رفض التكرارات. |

دعم الأدوات:

- خطوط أنابيب الإعلان عن الموفر، والقدرة على النطاق، وميزانية التدفق، وتلميحات النقل
  التحقق من صحة عمليات التدقيق الإضافية للحمولات النافعة الحتمية التي تنبعث منها كريات.
- `cargo xtask sorafs-admission-fixtures` الإعلانات الأساسية متعددة المصادر کو
  تحتوي تركيبات الرجوع إلى إصدار أقدم على `fixtures/sorafs_manifest/provider_admission/` على حزمة كرتا.
- إعلانات ذات قدرة على النطاق مثل `stream_budget` أو `transport_hints` تحذف کریں، CLI/SDK
  اللوادر تقوم بجدولة الرفض والرفض والتسخير متعدد المصادر
  لقد تمت محاذاة توقعات القبول Torii.

## نقاط نهاية نطاق البوابة

تقبل طلبات HTTP الحتمية للبوابات البيانات الوصفية للإعلان والمرآة
شكرا جزيلا.

###`GET /v1/sorafs/storage/car/{manifest_id}`| المتطلبات | التفاصيل |
|------------|---------|
| **العناوين** | `Range` (نافذة واحدة محاذاة لإزاحات المجموعة)، `dag-scope: block`، `X-SoraFS-Chunker`، `X-SoraFS-Nonce` اختياري، وضروري base64 `X-SoraFS-Stream-Token`. |
| **الردود** | `206` مع `Content-Type: application/vnd.ipld.car`، `Content-Range` النافذة المقدمة من قبل البيانات التعريفية `X-Sora-Chunk-Range`، ورؤوس المقطع/الرمز المميز. |
| **أوضاع الفشل** | النطاقات غير المحاذية پر `416`، الرموز المميزة المفقودة/غير الصالحة پر `401`، وتتجاوز ميزانيات الدفق/البايت ہونے پر `429`. |

###`GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

جلب قطعة واحدة عبارة عن رؤوس ثابتة بالإضافة إلى ملخص القطعة الحتمية. إعادة المحاولة
تعتبر تنزيلات الطب الشرعي مفيدة لشرائح CAR غير الضرورية.

## سير عمل منسق متعدد المصادر

تم تمكين جلب SF-6 متعدد المصادر ہو (Rust CLI عبر `sorafs_fetch`، SDKs عبر
`sorafs_orchestrator`):1. **جمع المدخلات** — فك تشفير خطة القطعة الواضحة، سحب أحدث الإعلانات،
   ولقطة قياس عن بعد اختيارية (`--telemetry-json` أو `TelemetrySnapshot`)
2. **إنشاء لوحة نتائج** — تقييم الأهلية `Orchestrator::build_scoreboard`
   كرتا ہے وأسباب الرفض سجل کرتا ہے؛ `sorafs_fetch --scoreboard-out`
   JSON تستمر كرتا ہے۔
3. **جدولة القطع** — `fetch_with_scoreboard` (أو `--plan`) قيود النطاق،
   ميزانيات الدفق، إعادة المحاولة/الأحرف الاستهلالية للأقران (`--retry-budget`، `--max-peers`) فرض الحد الأقصى
   وطلب إصدار رمز مميز للدفق واضح النطاق.
4. **التحقق من الإيصالات** — تتضمن المخرجات `chunk_receipts` و`provider_reports`
   ہوتے ہيں؛ ملخصات CLI `provider_reports`، `chunk_receipts`، و
   `ineligible_providers` لا تزال حزم الأدلة مستمرة.

مشغلي/SDKs أخطاء عامة:

| خطأ | الوصف |
|-------|------------|
| `no providers were supplied` | لا توجد تصفية بعد الإدخال المؤهل. |
| `no compatible providers available for chunk {index}` | مخصوص جزء من النطاق أو عدم تطابق الميزانية. |
| `retry budget exhausted after {attempts}` | `--retry-budget` يطرد الأقران الفاشلين أو يطردونهم من المدرسة. |
| `no healthy providers remaining` | حالات الفشل المتكررة بعد تمام تعطيل مقدمي الخدمة. |
| `streaming observer failed` | الكاتب CAR المصب إحباط ہو گیا۔ |
| `orchestrator invariant violated` | بيان الفرز، ولوحة النتائج، ولقطة القياس عن بعد، والتقاط CLI JSON. |

## القياس عن بعد والأدلة- المقاييس کی:  
  `sorafs_orchestrator_active_fetches`، `sorafs_orchestrator_fetch_duration_ms`،
  `sorafs_orchestrator_retries_total`، `sorafs_orchestrator_provider_failures_total`
  (البيان/المنطقة/علامات المزود). لوحات العدادات کو أسطول کے لحاظ سے
  تظهر علامات القسم على التكوين أو CLI `telemetry_region`.
- ملخصات جلب CLI/SDK تحتوي على لوحة النتائج المستمرة JSON وإيصالات القطع و
  تتضمن تقارير الموفرين بوابات SF-6/SF-7 التي تتضمن حزم الإطلاق المزيد منها.
- معالجات البوابة `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  الكشف عن قرارات المنسق ولوحات معلومات SRE وسلوك الخادم
  ربط کر سکیں۔

## مساعدي CLI وREST

- `iroha app sorafs pin list|show`، `alias list`، و`replication list` دبوس التسجيل
  تقوم نقاط نهاية REST بتغليف البطاقة وأدلة التدقيق لكتل المصادقة تلقائيًا
  طباعة خام Norito JSON.
- `iroha app sorafs storage pin` أو `torii /v1/sorafs/pin/register` Norito أو JSON
  يُظهر البراهين الاسم المستعار الاختياري والخلفاء يقبلون البطاقة؛ مشوه
  بروفات پر `400`، بروفات قديمة پر `503` مع `Warning: 110`، وبراهين منتهية الصلاحية
  پر `412`۔
- نقاط نهاية REST (`/v1/sorafs/pin`، `/v1/sorafs/aliases`، `/v1/sorafs/replication`)
  تشتمل هياكل التصديق على أحدث رؤوس الكتل للعملاء
  التحقق من البيانات المختلفة کر سكای۔

## مراجع- المواصفات الكنسية:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- أنواع Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- مساعدي CLI: `crates/iroha_cli/src/commands/sorafs.rs`،
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- صندوق المنسق: `crates/sorafs_orchestrator`
- حزمة لوحة القيادة: `dashboards/grafana/sorafs_fetch_observability.json`