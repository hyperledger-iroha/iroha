---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/node-client-protocol.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# البروتوكول الجديد ↔ العميل SoraFS

يلخص هذا الدليل التعريف القانوني للبروتوكول
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
استخدم المواصفات الأولية للتخطيطات Norito على المستوى الثماني وما إلى ذلك
سجلات التغيير ; تحافظ نسخة الباب على النقاط التشغيلية الموجودة أمامك
بقية دفاتر التشغيل SoraFS.

## الإعلانات fournisseur والتحقق من صحتها

يقوم الموردون SoraFS بنشر الحمولات الصافية `ProviderAdvertV1` (العرض
`crates/sorafs_manifest::provider_advert`) مُوقع من قبل المشغل الحكومي. ليه
الإعلانات تثبت les métadonnées de découverte و les garde-fous que l'orchesstrateur
تطبيق متعدد المصادر للتنفيذ.- **مدة الصلاحية** — `issued_at < expires_at ≤ issued_at + 86 400 s`. ليه
  يجب أن يقوم الموردون بتفكيك كل 12 ساعة.
- **TLV de capacité** — تعلن قائمة TLV عن ميزات النقل
  (Torii، QUIC+Noise، relais SoraNet، بائع الملحقات). الرموز غير معروفة
  يمكن أن يتم تجاهله عندما `allow_unknown_capabilities = true`، التالي
  توصيات الشحوم.
- **مؤشرات جودة الخدمة** — المستوى `availability` (ساخن/دافئ/بارد)، الحد الأقصى لزمن الوصول
  الاسترداد والحد من التوافق وميزانية خيارات البث. جودة الخدمة تفعل ذلك
  يتم محاذاة القياس عن بعد ويتم تدقيقه عند الدخول.
- **نقاط النهاية وموضوعات الالتقاء** — عناوين URL للخدمة الملموسة
  Métadonnées TLS/ALPN، بالإضافة إلى الموضوعات التي يتم اكتشافها لدى العملاء
  يجب أن يكون الأمر كذلك عند إنشاء مجموعات الحماية.
- **سياسة التنوع الكيميائي** — `min_guard_weight`، أغطية التفرع
  يقدم AS/pool و`provider_failure_threshold` عمليات الجلب الممكنة
  محددات متعددة الأقران.
- **معرفات الملف الشخصي** — يجب أن يكشف الموردون عن المقبض
  كانونيك (على سبيل المثال `sorafs.sf1@1.0.0`)؛ مساعد خيارات `profile_aliases`
  العملاء القدماء يهاجرون.تم رفض قواعد التحقق من الصحة، وقوائم مقاطع الفيديو الخاصة بالإمكانات،
نقاط النهاية أو الموضوعات أو فترات زمنية محددة أو أهداف جودة الخدمة. ليه
مغلفات القبول تقارن بين مجموعة الإعلان والاقتراح
(`compare_core_fields`) قبل نشر أخبار اليوم.

### ملحقات الجلب الاسمية

يشمل الموردون ذوو الكفاءات العناصر التالية:

| بطل | موضوعي |
|-------|----------|
| `CapabilityType::ChunkRangeFetch` | قم بتعريف `max_chunk_span` و`min_granularity` وعلامات المحاذاة/الدقة. |
| `StreamBudgetV1` | خيار المغلف للتزامن/الإنتاجية (`max_in_flight`، `max_bytes_per_sec`، `burst` optionnel). تتطلب نطاق السعة. |
| `TransportHintV1` | تفضيلات النقل ordonnées (مثال: `torii_http_range`، `quic_stream`، `soranet_relay`). تم رفض الأولويات الخاصة بـ `0–15` والمضاعفات. |

أدوات الدعم:

- توفر خطوط الأنابيب الإعلانية نطاقًا صالحًا للسعة والتدفق
  تلميحات حول الميزانية والنقل قبل تحديد الحمولات الصافية لهم
  عمليات التدقيق.
- `cargo xtask sorafs-admission-fixtures` إعادة تجميع الإعلانات متعددة المصادر
  Canoniques avec des Installations de downgrade dans
  `fixtures/sorafs_manifest/provider_admission/`.
- تتراوح الإعلانات التي تحتوي على `stream_budget` أو `transport_hints`
  تم رفضه بواسطة محمل CLI/SDK للتخطيط المسبق ومحاذاة الحزام
  مصادر متعددة حول تنبيهات القبول Torii.

## نطاق نقاط النهاية بوابة دوتقبل البوابات طلبات HTTP التي تحددها
métadonnées des adverts.

###`GET /v1/sorafs/storage/car/{manifest_id}`

| الضرورة | التفاصيل |
|----------|--------|
| **العناوين** | `Range` (نافذة فريدة محاذية لإزاحات القطع)، و`dag-scope: block`، و`X-SoraFS-Chunker`، وخيار `X-SoraFS-Nonce`، و`X-SoraFS-Stream-Token` base64 إلزامي. |
| **الردود** | `206` مع `Content-Type: application/vnd.ipld.car`، `Content-Range` يتم إنشاء نافذة الخدمة، والتحويلات `X-Sora-Chunk-Range`، ومقسم الرؤوس/الرمز المميز. |
| **أوضاع الفحص** | `416` للبطاقات غير المحاذاة، `401` للرموز المميزة المفقودة/غير الصالحة، `429` عندما يتم إلغاء تدفق الميزانيات/الثمانية. |

###`GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

جلب قطعة واحدة باستخدام الرؤوس المماثلة، بالإضافة إلى تحديد الملخص
قطعة. مفيد لإعادة المحاولات أو التنزيلات الجنائية أثناء قيامك بذلك
شرائح CAR sont inutiles.

## سير عمل منسق متعدد المصادر

عندما يكون الجلب متعدد المصادر SF-6 نشطًا (CLI Rust via `sorafs_fetch`,
مجموعات SDK عبر `sorafs_orchestrator`):1. **Collecter les Entrées** — فك شفرة خطة قطع البيان، واستعادتها
   آخر الإعلانات، وخيارات تمرير لقطة عن بعد
   (`--telemetry-json` أو `TelemetrySnapshot`).
2. **إنشاء لوحة نتائج** — قيمة `Orchestrator::build_scoreboard`
   الأهلية وتسجيل أسباب الرفض؛ `sorafs_fetch --scoreboard-out`
   تستمر لو JSON.
3. **مخطط القطع** — `fetch_with_scoreboard` (ou `--plan`) يفرض القطع
   نطاق القيود، تدفق الميزانيات، الحد الأقصى لإعادة المحاولة/النظير (`--retry-budget`،
   `--max-peers`) وتم إنشاء نطاق رمزي دفق على البيان لكل منها
   طلب.
4. **التحقق من صحة الرسالة** — تتضمن الطلعات `chunk_receipts` وآخرون
   `provider_reports`; السيرة الذاتية CLI المستمرة `provider_reports`,
   `chunk_receipts` و`ineligible_providers` للحزم المسبقة.

الأخطاء التي تم إصلاحها من قبل المشغلين/مجموعات SDK:

| خطأ | الوصف |
|--------|------------|
| `no providers were supplied` | يمكن إدخالها بعد الترشيح. |
| `no compatible providers available for chunk {index}` | حدث خطأ في الميزانية أو الميزانية بسبب جزء محدد. |
| `retry budget exhausted after {attempts}` | قم بتعزيز `--retry-budget` أو إثبات الأقران بشكل صحيح. |
| `no healthy providers remaining` | يتم إلغاء تنشيط جميع الموردين بعد إجراء عمليات فحص متكررة. |
| `streaming observer failed` | لو الكاتب CAR المصب avorté. |
| `orchestrator invariant violated` | قم بالتقاط البيان ولوحة النتائج ولقطات القياس عن بعد وJSON CLI للفرز. |

## Télémétrie et preuves- المقاييس المنبعثة من الأوركسترا :  
  `sorafs_orchestrator_active_fetches`، `sorafs_orchestrator_fetch_duration_ms`،
  `sorafs_orchestrator_retries_total`، `sorafs_orchestrator_provider_failures_total`
  (taggées par Manifest/région/fournisseur). تعريف `telemetry_region` en
  قم بتكوين ou عبر علامات CLI لتقسيم لوحات المعلومات حسب الأسطول.
- تتضمن السيرة الذاتية لجلب CLI/SDK لوحة النتائج JSON المستمرة والنتائج
  de Chunks et les Rapports fournisseurs الذين يقومون بالرحلة في الحزم
  تم البدء في بوابات SF-6/SF-7.
- بوابة معالجات ليه مكشوفة `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  لكي تقوم لوحات المعلومات SRE بربط القرارات المنسقة بها
  خادم السلوك.

## مساعدو CLI et REST

- `iroha app sorafs pin list|show` و`alias list` و`replication list` حاصروا الملفات
  نقاط النهاية REST du pin-registry et impriment du Norito JSON brut avec blocks
  شهادة للتدقيق.
- `iroha app sorafs storage pin` و`torii /v1/sorafs/pin/register` مقبول
  بيانات Norito ou JSON، بالإضافة إلى إثباتات الأسماء المستعارة للخيارات والخلفاء؛
  البراهين النموذجية غير الصحيحة `400`، البراهين القديمة المكشوفة `503` مع
  `Warning: 110`، والإثباتات منتهية الصلاحية مرة أخرى `412`.
- نقاط النهاية REST (`/v1/sorafs/pin`، `/v1/sorafs/aliases`،
  `/v1/sorafs/replication`) تتضمن هياكل المصادقة التي ستساعدك
  يتحقق العملاء من البيانات باستخدام آخر رؤوس الكتلة قبل البدء.

## مراجع- المواصفات الكنسي :
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- الأنواع Norito : `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- مساعدي CLI: `crates/iroha_cli/src/commands/sorafs.rs`،
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- منسق الصندوق: `crates/sorafs_orchestrator`
- حزمة لوحات المعلومات: `dashboards/grafana/sorafs_fetch_observability.json`