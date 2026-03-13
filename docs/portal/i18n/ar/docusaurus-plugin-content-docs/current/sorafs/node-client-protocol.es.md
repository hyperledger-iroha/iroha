---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/node-client-protocol.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# بروتوكول العقدة ↔ العميل SoraFS

هذا دليل استئناف التعريف القانوني للبروتوكول أون
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
استخدام المواصفات الأولية للتخطيطات Norito على مستوى البايتات والوحدات
سجلات التغيير؛ تحافظ نسخة البوابة على نقاط التشغيل التي تبحث عنها
من دفاتر التشغيل SoraFS.

## إعلان الموثق والتحقق

موردو SoraFS الحمولات الصافية `ProviderAdvertV1` (الإصدار
`crates/sorafs_manifest::provider_advert`) شركة تابعة للمشغل الحاكم.
يتم الإعلان عن بيانات تعريف الاكتشاف والحماية التي يقوم بها
يقوم الأوركسترا متعدد الوظائف بتشغيل وقت التشغيل.- ** فيجينسيا ** — `issued_at < expires_at ≤ issued_at + 86,400 s`. المصدرين
  ديبن تجديد كل 12 ساعة.
- **سعة TLVs** — قائمة TLV تعلن عن وظائف النقل (Torii,
  QUIC+Noise، relés SoraNet، ملحقات المورد). الرموز المزعجة
  يمكنك حذف عندما `allow_unknown_capabilities = true`، اتبع الدليل
  الشحوم.
- **مرتكزات جودة الخدمة** — مستوى `availability` (ساخن/دافئ/بارد)، أقصى زمن وصول
  الاسترداد والحد من التوافق وافتراض البث الاختياري. لا جودة الخدمة
  يجب أن تكون خطيًا مع القياس عن بعد الذي يتم ملاحظته ويتم تدقيقه عند القبول.
- **نقاط النهاية وموضوعات الالتقاء** — عناوين URL الخاصة بالخدمة المحددة
  تعد بيانات تعريف TLS/ALPN أكثر الموضوعات اكتشافًا للعملاء
  يجب الاشتراك في إنشاء مجموعات الحماية.
- **سياسة التنوع في المسارات** — `min_guard_weight`، قمم التفرع
  AS/pool و`provider_failure_threshold` قاما بجلب المحددات الممكنة
  متعدد الأقران.
- **معرفات الشخصية** — يجب على الموردين أن يشرحوا المقبض
  canónico (ص. على سبيل المثال، `sorafs.sf1@1.0.0`)؛ `profile_aliases` اختيارية تساعد أ
  العملاء المهاجرين أنتيغوس.

تم تغيير قواعد التحقق من الصحة، وسرد القدرات الفارغة،
نقاط النهاية أو المواضيع أو الأنشطة غير المرغوب فيها أو أهداف جودة الخدمة الرديئة. لوس
تقارن نصائح القبول بين جسم الإعلان والعرض
(`compare_core_fields`) قبل التحديث.

### امتدادات الجلب حسب النطاقاتيتضمن الموردون المتنوعون البيانات الوصفية التالية:

| كامبو | اقتراح |
|-------|----------|
| `CapabilityType::ChunkRangeFetch` | أعلن `max_chunk_span`، `min_granularity` وأعلام التخصيص/الفحص. |
| `StreamBudgetV1` | مغلف اختياري للتزامن/الإنتاجية (`max_in_flight`، `max_bytes_per_sec`، `burst` اختياري). يتطلب نطاقًا واسعًا من السعة. |
| `TransportHintV1` | تفضيلات نقل الأوامر (ص. على سبيل المثال، `torii_http_range`، `quic_stream`، `soranet_relay`). الأولويات من `0–15` وتم حذف النسخ المكررة. |

دعم الأدوات:

- يجب أن تعلن خطوط أنابيب المزود عن سعة النطاق والبث
  تلميحات حول الميزانية والنقل قبل تحديد الحمولات الصافية للمستمعين.
- `cargo xtask sorafs-admission-fixtures` empaqueta anuncios multifuente
  Canonicos جنبًا إلى جنب مع تركيبات الرجوع إلى إصدار أقدم
  `fixtures/sorafs_manifest/provider_admission/`.
- Los anuncios con rango que omiten `stream_budget` o `transport_hints` son
  إعادة التدوير بواسطة أدوات تحميل CLI/SDK قبل البرمجة، والحفاظ على المعرفة
  متعدد الاستخدامات مع توقعات القبول Torii.

## نقاط نهاية البوابة

تقبل البوابات طلبات HTTP التي تحدد البيانات الوصفية
الإعلانات.

###`GET /v2/sorafs/storage/car/{manifest_id}`| المتطلبات | تفاصيل |
|-----------|----------|
| **العناوين** | `Range` (نافذة واحدة مخصصة لإزاحة القطع) و`dag-scope: block` و`X-SoraFS-Chunker` و`X-SoraFS-Nonce` اختيارية و`X-SoraFS-Stream-Token` base64 إلزامية. |
| **الردود** | `206` مع `Content-Type: application/vnd.ipld.car`، `Content-Range` الذي يصف خادم النافذة والبيانات الوصفية `X-Sora-Chunk-Range` ورؤوس القطع/الرمز المميز البيئي. |
| **طرق السقوط** | `416` للنطاقات التي تم تحليتها، `401` للرموز المميزة الخاطئة أو غير الصالحة، `429` عند تجاوز متطلبات الدفق/البايت. |

###`GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

قم بإحضار قطعة واحدة بنفس الرؤوس بشكل أكبر من التحديد
قطعة. يستخدم لإعادة التحميل أو التنزيلات عندما لا تحتاج إلى شرائح
سيارة.

## تدفق العمل للسائق متعدد الوظائف

عندما تتمكن من الجلب متعدد الاستخدامات SF-6 (CLI Rust عبر `sorafs_fetch`،
مجموعات SDK عبر `sorafs_orchestrator`):1. **إعادة نسخ المدخلات** — فك تشفير خطة أجزاء البيان، وتتبعها
   أحدث الإعلانات، واختياريًا، التقاط لقطة للقياس عن بعد
   (`--telemetry-json` أو `TelemetrySnapshot`).
2. **إنشاء لوحة نتائج** — تقييم `Orchestrator::build_scoreboard`
   الأهلية والتسجيل لأسباب الاسترداد؛ `sorafs_fetch --scoreboard-out`
   استمر في JSON.
3. **الأجزاء البرمجية** — `fetch_with_scoreboard` (o `--plan`)
   قيود النطاق، واشتراطات البث، وأجزاء من الاحتفاظ/الأقران
   (`--retry-budget`, `--max-peers`) وقم بإصدار رمز دفق مميز بكمية كبيرة
   واضح لكل طلب.
4. **الشهادات المؤكدة** — تتضمن الخرجات `chunk_receipts` و
   `provider_reports`; تستمر نتائج CLI في `provider_reports`,
   `chunk_receipts` و`ineligible_providers` لحزم الأدلة.

الأخطاء الشائعة التي تتعلق بالمشغلين/حزم تطوير البرامج (SDK):

| خطأ | الوصف |
|-------|------------|
| `no providers were supplied` | لا توجد مدخلات مؤهلة عبر التصفية. |
| `no compatible providers available for chunk {index}` | قم بإلغاء ضبط النطاق أو الافتراض لقطعة معينة. |
| `retry budget exhausted after {attempts}` | زيادة `--retry-budget` أو طرد الأقران الفاشلين. |
| `no healthy providers remaining` | Todos los profeedores quedaron dehabilitados tras Fallos repetidos. |
| `streaming observer failed` | تم إحباط الكاتب CAR المصب. |
| `orchestrator invariant violated` | بيان الالتقاط ولوحة النتائج ولقطة القياس عن بعد وJSON من CLI للفرز. |

## القياس عن بعد والأدلة- القياسات الصادرة عن الأوركيستادور:  
  `sorafs_orchestrator_active_fetches`، `sorafs_orchestrator_fetch_duration_ms`،
  `sorafs_orchestrator_retries_total`، `sorafs_orchestrator_provider_failures_total`
  (البيانات الخاصة بالبيان/المنطقة/المزود). التكوين `telemetry_region` ar
  التكوين أو عبر أعلام CLI لفصل لوحات المعلومات عن الأسطول.
- تتضمن نتائج الجلب عبر CLI/SDK لوحة النتائج JSON المستمرة والاستقبالات
  القطع وإعلام الموردين الذين سيتعين عليهم السفر عبر حزم الطرح
  للبوابات SF-6/SF-7.
- معالجات البوابة تعرض `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  لكي تتمكن لوحات المعلومات من اتخاذ قرارات SRE المرتبطة بالأداة
  خصائص الخادم.

## مساعدة من CLI y REST

- `iroha app sorafs pin list|show`، `alias list` و`replication list` مغلفان
  نقاط النهاية REST del pin-registry والطباعة Norito JSON الخام مع الكتل
  شهادة لإثبات السمع.
- `iroha app sorafs storage pin` و `torii /v2/sorafs/pin/register` بيانات الأسيبتان
  Norito o المزيد من إثباتات JSON للأسماء المستعارة الاختيارية واللاحقة؛ البراهين المشوهة
  elevan `400`، البراهين القديمة الموضحة `503` مع `Warning: 110`، والبراهين
  انتهت الصلاحية `412`.
- نقاط النهاية REST (`/v2/sorafs/pin`، `/v2/sorafs/aliases`،
  `/v2/sorafs/replication`) تتضمن هياكل التصديق الخاصة بهم
  يتحقق العملاء من البيانات مقابل رؤوس الكتلة الأخيرة قبل التشغيل.

## المراجع- المواصفات القانونية:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- تيبوس Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- مساعدة CLI: `crates/iroha_cli/src/commands/sorafs.rs`،
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- صندوق الأوركيستادور: `crates/sorafs_orchestrator`
- حزمة لوحات المعلومات: `dashboards/grafana/sorafs_fetch_observability.json`