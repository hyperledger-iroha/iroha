---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/node-client-protocol.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# البروتوكول SoraFS المستخدم ↔ العميل

هذه هي السيرة الذاتية لبروتوكول الامتياز القانوني في
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
استخدم مواصفات المنبع لتخطيط Norito وسجل التغيير؛
نسخة البوابة تعمل على استخدام اللمسات الأصلية مع دليل التشغيل الأصلي SoraFS.

## التسليم والتحقق من الصحة

يقوم المثبتون SoraFS برفع الحمولة `ProviderAdvertV1` (cm.
`crates/sorafs_manifest::provider_advert`)، المشغل الموجه.
إصلاح الخلل في الأغطية والدرابزين, منها
يقوم أوركسترا متعدد الأطوار بتخصيص الأموال.- **الصناعة الدقيقة** — `issued_at < expires_at ≤ issued_at + 86 400 s`. مقدمو الخدمة
  يجب أن تبدأ كل 12 ساعة.
- **قدرة TLV** — قائمة إعلان TLV لإمكانية النقل (Torii,
  QUIC+ضوضاء، مرحلات SoraNet، ملحقات البائعين). من الممكن أن تكون هناك رموز غير معروفة
  النشر عن `allow_unknown_capabilities = true`، التوصية التالية
  الشحوم.
- **تلميحات جودة الخدمة** — المستوى `availability` (ساخن/دافئ/بارد)، الحد الأقصى للخصم
  التحسين والحد من التزامن وميزانية التدفق الاختيارية. جودة الخدمة مستمرة
  التواصل مع أجهزة القياس عن بعد والتحقق منها قبل القبول.
- **نقاط النهاية ومواضيع الالتقاء** — عنوان URL محدد للخدمة مع TLS/ALPN
  المواضيع الوصفية والاستكشافية التي يقدمها العملاء
  تعزيز مجموعات الحراسة.
- **الطريق السياسي المتنوع** — `min_guard_weight`، حدود المنافذ المتاحة
  AS/بولوف و`provider_failure_threshold` يحددان معايير
  جلب متعدد الأقران.
- **الملفات التعريفية للمُعرِّفات** — ينشر الموردون الكنسيون
  المقبض (على سبيل المثال، `sorafs.sf1@1.0.0`)؛ دعم اختياري `profile_aliases`
  ترحيل العملاء القدامى.

التحقق من الصحة بشكل صحيح لتجاوز الحصة الجديدة، وإمكانيات السجلات الثابتة/نقاط النهاية/الموضوعات،
الاتصالات المتنقلة التي تحقق أهداف جودة الخدمة. مظاريف القبول
قم بإلقاء نظرة على التسليم والاقتراح (`compare_core_fields`) السابق
распространением обновлений.

### جلب النطاق rasshireniya

يتضمن مقدمو النطاق المتخصصون الوصفات التالية:| بول | الاسم |
|------|-----------|
| `CapabilityType::ChunkRangeFetch` | يرجى ملاحظة `max_chunk_span` و`min_granularity` وتفعيل الأعلام/التوصيل. |
| `StreamBudgetV1` | توافق المغلف/الإنتاجية الاختيارية (`max_in_flight`، `max_bytes_per_sec`، الاختياري `burst`). قدرة نطاق TREBUEET. |
| `TransportHintV1` | النقل المقترح (على سبيل المثال، `torii_http_range`، `quic_stream`، `soranet_relay`). الأولويات `0–15`، تنغلق بشكل مزدوج. |

أدوات الدعم:

- متطلبات إعلان مقدم الخدمة للتحقق من قدرة النطاق وميزانية التدفق
  وتلميحات النقل لتحديد الحمولة النافعة لمراجعة الحسابات.
- `cargo xtask sorafs-admission-fixtures` الحزمة الكنسيه متعددة المصادر
  اعلانات вместе مع خفض مستوى التركيبات в
  `fixtures/sorafs_manifest/provider_admission/`.
- يتم إلغاء حظر إعلانات النطاق باستثناء `stream_budget` أو `transport_hints`.
  CLI/SDK للتخطيط، متضمنًا تسخير متعدد المصادر مع الدعم
  озиданиями القبول Torii.

## بوابة نقاط نهاية النطاق

تتيح البوابات تحديد بروتوكولات HTTP، بالإضافة إلى الالتزامات التبادلية.

###`GET /v2/sorafs/storage/car/{manifest_id}`| تريبوفانيا | التفاصيل |
|------------|--------|
| **العناوين** | `Range` (فاصل زمني واحد، قابل للتعديل على إزاحة القطعة)، `dag-scope: block`، `X-SoraFS-Chunker`، اختياري `X-SoraFS-Nonce` وقاعدة خاصة64 `X-SoraFS-Stream-Token`. |
| **الردود** | `206` مع `Content-Type: application/vnd.ipld.car`، `Content-Range`، فاصل زمني وصفي، `X-Sora-Chunk-Range` مقسم/رمز مميز. |
| **أوضاع الفشل** | `416` للرموز المميزة/غير الصالحة، `401` للرموز المميزة/غير الصالحة، `429` ميزانية البث/البايت السابقة. |

###`GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

جلب قطعة واحدة تحتوي على مواضيع ورؤوس بالإضافة إلى قطعة ملخص محددة.
مفيد لاسترجاع أو تنزيلات الطب الشرعي، عندما لا تكون هناك حاجة إلى شرائح CAR.

## سير العمل للمنسق المتعدد التخصصات

بما في ذلك جلب متعدد المصادر SF-6 (Rust CLI من خلال `sorafs_fetch`،
مجموعات SDK من خلال `sorafs_orchestrator`):1. **الحصول على البيانات المالية** — فك تشفير بيان خطة القطعة، احصل على
   الإعلانات اللاحقة، واختياريًا، لقطة القياس عن بعد
   (`--telemetry-json` أو `TelemetrySnapshot`).
2. **نشر لوحة النتائج** — تقييم `Orchestrator::build_scoreboard`
   تسهيل وتسجيل الأسباب; `sorafs_fetch --scoreboard-out`
   сограняет JSON.
3. **تخطيط القطعة** — `fetch_with_scoreboard` (أو `--plan`) مكرر
   نطاق النطاق، ميزانية البث، حدود إعادة المحاولة/النظير (`--retry-budget`،
   `--max-peers`) وأدخل رمز الدفق المميز في بيان النطاق لأي سبب من الأسباب.
4. **التحقق من الإيصالات** — تتضمن النتائج `chunk_receipts` و
   `provider_reports`; ملخص CLI يخزن `provider_reports`, `chunk_receipts`
   و`ineligible_providers` لحزم الأدلة.

الملحقات الشاملة، المشغلين/مجموعات SDK:

| أوشيبكا | الوصف |
|--------|---------|
| `no providers were supplied` | لا تقم بكتابة أي شيء بعد التصفية. |
| `no compatible providers available for chunk {index}` | لا يوجد تنازل أو ميزانية لقطعة محددة. |
| `retry budget exhausted after {attempts}` | احصل على `--retry-budget` أو باستثناء أقرانهم. |
| `no healthy providers remaining` | يتم استبعاد جميع الموردين بعد التخفيضات اللاحقة. |
| `streaming observer failed` | الكاتب المصب CAR انتهى مع أوشيبكو. |
| `orchestrator invariant violated` | قم بإظهار البيان ولوحة النتائج ولقطة القياس عن بعد وCLI JSON للفرز. |

## القياس عن بعد والتوصيل- مقاييس الأوركسترا الفخرية:  
  `sorafs_orchestrator_active_fetches`، `sorafs_orchestrator_fetch_duration_ms`،
  `sorafs_orchestrator_retries_total`، `sorafs_orchestrator_provider_failures_total`
  (من خلال البيان/المنطقة/المزود). قم بتحميل `telemetry_region` في التكوين أو
  من خلال أعلام CLI لتوزيع لوحات الوصول على متن الطائرة.
- تتضمن ملخصات جلب CLI/SDK لوحة النتائج المضمنة JSON وإيصالات القطع وما إلى ذلك.
  تقارير الموفر، التي يجب تضمينها في حزم الطرح للبوابة SF-6/SF-7.
- تنشر معالجات البوابة `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`،
  لربط لوحات المعلومات SRE بقرارات الأوركسترا مع الخادم المحدث.

## مساعدي CLI وREST

- `iroha app sorafs pin list|show` و`alias list` و`replication list` ملتزمون
  نقاط نهاية REST لسجل الدبوس وتسجيل Norito JSON مع شهادة الكتل
  لمراجعي الحسابات.
- `iroha app sorafs storage pin` و `torii /v2/sorafs/pin/register` يمثلان Norito
  أو بيانات JSON بالإضافة إلى البراهين والخلفاء الاختيارية للاسم المستعار؛ البراهين المشوهة
  تم إنشاء `400`, البراهين القديمة حتى `503` مع `Warning: 110`, البراهين منتهية الصلاحية
  возвращают `412`.
- نقاط نهاية REST (`/v2/sorafs/pin`، `/v2/sorafs/aliases`،
  `/v2/sorafs/replication`) تتضمن هياكل التصديق التي يمكن للعملاء
  تحقق من صحة رؤوس الكتل التالية قبل الانتهاء منها.

## مرحبا- المواصفات القانونية:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- أنواع Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- مساعدو CLI: `crates/iroha_cli/src/commands/sorafs.rs`،
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- منظم الصندوق: `crates/sorafs_orchestrator`
- حزمة الوصول: `dashboards/grafana/sorafs_fetch_observability.json`