---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/node-client-protocol.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#بروتوكول عقدة ↔ عميل SoraFS

يلخص هذا الدليل الدليل المعتمد للبروتوكول في
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
استخدم المواصفة المنبع لتخطيط Norito على مستوى البايت لتغيير التغييرات؛
حافظ على نسخة البوابة على النقاط التشغيلية المميزة قرب بقية runbooks الخاصة بـ SoraFS.

##إعلانات المتحكم والتحقق

يقوم بتنظيمو SoraFS ببث حمولات `ProviderAdvertV1` (راجع
`crates/sorafs_manifest::provider_advert`) الموقع من المُشغّل التنظيمي للرقابة التنظيمية.
تثبت الإعلانات بياناتها والضوابط التي تفترضها المُنسِّق ذات المصدر المتعددة
أثناء التشغيل.- **مدة الصلاحية** — `issued_at < expires_at ≤ issued_at + 86,400 s`. ينبغي على
  المدقق الدقيق كل 12 ساعة.
- **TLVhan** — تسرد قائمة TLV صلاحيات الإصدار (Torii، QUIC+Noise، ترحيلات)
  سورانت، امتدادات متحكم). يمكن تجاهل الأكواد المجهولة عندما
  `allow_unknown_capabilities = true` مع اتباع إرشادات GREASE.
- **تلميحات QoS** — `availability` (ساخن/دافئ/بارد)، أقصى كمون للاسترجاع،
  إلى حد التزامن، والتيار الميزاني اختياري. يجب أن تحدد جودة الخدمة مع التليميترية
  الملحوظة أصغر تدقيقها في التقبل.
- **موعد نقاط النهاية ومواضيع** — عناوين خدمة محددة مع إضافة بيانات TLS/ALPN
  إلى أمور لا يجب أن يشترك فيها العملاء عند بناء مجموعات الاستشعار.
- **سياسة تنوع المسارات** — `min_guard_weight` وحدود fan-out لمجموعات AS/pool و
  `provider_failure_threshold` تجعل تعدد الأقران حتمي ممكنًا.
- **معرّفات الملف الشخصي** — يجب على المحكمين نشر المدقق المعتمد (مثل
  `sorafs.sf1@1.0.0`). مساعدة `profile_aliases` اختيارية العملاء القدامى على
  الرحيل.

رفض متطلبات التحقق من الحصة النمساوية، أو قوائم الترشيحات/العناوين/المواضيع الفارغة،
أو تواريخ غير مرتبة، أو أهداف جودة الخدمة مفقودة. تقارن أظرف مقبولة بين محتوى الإعلان
والاقتراح (`compare_core_fields`) قبل تحديث التحديثات.

### امتدادات الجلب بالنطاقات

تتضمن المتحكمات الباتش لنطاق البيانات التالي:| الحقل | اللحوم |
|-------|-------|
| `CapabilityType::ChunkRangeFetch` | أعلن `max_chunk_span` و`min_granularity` وأعلام العازف/الأدلة. |
| `StreamBudgetV1` | غلافي اختياري للتزامن/المعدل (`max_in_flight`, `max_bytes_per_sec`, و`burst` اختياري). تتطلب قدرة النطاق. |
| `TransportHintV1` | تفضيلات نقل مرتبة (مثل `torii_http_range`, `quic_stream`, `soranet_relay`). الأولويات ضمن `0–15` ولا يتم رفض التكرار. |

دعم الأدوات:

- يجب على خطوط الإعلانات المتحكمة التحقق من قدرة النطاق وميزانية الدفق وتلميحات
  الإصدار السابق حمولات حتمية للتدقيق.
- `cargo xtask sorafs-admission-fixtures` يجمع إعلانات متعددة المصدر مع
  تركيبات للخفض تحت `fixtures/sorafs_manifest/provider_admission/`.
- تُرفض الإعلانات الباهتة للنطاق الذي تُسقط `stream_budget` أو `transport_hints`
  بواسطة مجاهدات CLI/SDK قبل الجدولة، ما نلتزم بتوافق متعدد المصادر مع
  توقعات مقبولة Torii.

## نقاط نهاية نطاقات البوابة

تقبل البوابات طلبات HTTP حتمية احترام بيانات الإعلانات.

###`GET /v1/sorafs/storage/car/{manifest_id}`

| المتطلب | التفاصيل |
|---------|---------|
| **العناوين** | `Range` (نافذة واحدة مصطفة مع إزاحات القادة)، `dag-scope: block`، `X-SoraFS-Chunker`، `X-SoraFS-Nonce` اختياري، و`X-SoraFS-Stream-Token` base64. |
| **الردود** | `206` مع `Content-Type: application/vnd.ipld.car`، و`Content-Range` يصف النافذة المقدمة، وبيانات `X-Sora-Chunk-Range`، جاهز لإعادة إرسال الرموز/الرمز المميز. |
| **أوضاع الفشل** | `416` للنطاقات غير المصطفة، `401` للرموز الناطق/غير الصالحة، `429` عند تجاوز ميزانيات الدفق/البايت. |###`GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

جلبة شريحة واحدة بنفس الرؤوس بالإضافة إلى هضم حتمي للشريحة. مفيد مرة أخرى
المحاولة أو تنزيل الطب الشرعي عندما لا تكون شرائح CAR ضرورية.

## سير عمل المُنسِّق ذو المصادر المتعددة

عند تفعيل جلب SF-6 متعدد المصادر (CLI Rust عبر `sorafs_fetch`، وSDKs عبر
`sorafs_orchestrator`):

1. **جمع المدخلات** — خطة تخطيط شرائح المانيفيست، وجلب أحدث الإعلانات، وتمرير
   لقطة تليمترية اختيارية (`--telemetry-json` أو `TelemetrySnapshot`).
2. **بناء لوحة النتائج** — يقوم `Orchestrator::build_scoreboard` بتقييم الأهلية
   وتسجيل سبب الرفض؛ يحفظ `sorafs_fetch --scoreboard-out` ملف JSON.
3. **جدول المقاعد** — يفرض `fetch_with_scoreboard` (أو `--plan`) النطاق الإلكتروني،
   تيار ميزانيات، وحدود إعادة المحاولة/الأقران (`--retry-budget`, `--max-peers`)
   ويصدر تيار رمزي بنطاق المانيفيست لكل طلب.
4. **التحقق من الإيصالات** — تشمل المنفذات `chunk_receipts` و`provider_reports`؛
   تحفظ ملخصات CLI كلًا من `provider_reports` و`chunk_receipts` و`ineligible_providers`
   لحزم الأدلة.

الأخطاء الشائعة تصل إلى المشغلين/SDKs:

| الخطأ | الوصف |
|-------|-------|
| `no providers were supplied` | لا توجد كمية كافية بعد التصفية. |
| `no compatible providers available for chunk {index}` | لا تتوافق مع النطاق أو لشريحة محددة. |
| `retry budget exhausted after {attempts}` | زد `--retry-budget` أو استبعد الأقران المتعثرين. |
| `no healthy providers remaining` | تم جميع المتحكمين بعد إخفاقات متكررة. |
| `streaming observer failed` | كاتب نهاري CAR في المسار السفلي. |
| `orchestrator invariant violated` | التقط المانيفيست ولوحة النتائج واللقطة التليميترية وJSON الخاص بالـ CLI للتحليل. |##التليمترية والأدلة

- المحاسبون عن المُنسِّق:  
  `sorafs_orchestrator_active_fetches`، `sorafs_orchestrator_fetch_duration_ms`،
  `sorafs_orchestrator_retries_total`، `sorafs_orchestrator_provider_failures_total`
  (موسومة حسب المانيفست/المنطقة/المزود). ضبط `telemetry_region` في الطول أو الطول
  أعلام CLI للنسخ الاحتياطي حسب النسخة الطولية.
- تتضمن ملخصات الجلب في CLI/SDK ملف لوحة النتائج JSON المحفوظ وإيصالات المقاعد
  والتقارير المحاسبية التي يجب شحنها ضمن حزم نهائية لبوابات SF-6/SF-7.
- كاميرا معالجات البوابة `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  كي تبدأ لوحات SRE بتكوين المُنسِّق بسلوك الموظفين.

## مساعدات CLI وREST

- `iroha app sorafs pin list|show` و`alias list` و`replication list` تطبع نقاط REST
  الخاصة بسجل الدبابيس وتطبع Norito JSON Raw مع شهادة الكتل لأدلة التدقيق.
- `iroha app sorafs storage pin` و`torii /v1/sorafs/pin/register` يقبلان البيانات
  بنمط Norito أو JSON مع إثباتات اختيارية للـ alias والـ Successor؛ لها البراهين
  المشوهة إلى `400`، وتظهر البراهين القديمة `503` مع `Warning: 110`، بينما
  البراهين كاملة تمامًا `412`.
- نقاط REST (`/v1/sorafs/pin`، `/v1/sorafs/aliases`، `/v1/sorafs/replication`)
  تتضمن الهياكل التصديق حتى يبدأ العملاء في التحقق من البيانات مقابل الأحدث
  رؤوس الكتل قبل التنفيذ.

## المراجع

- المواصفة المعتمدة :
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
-أنواع Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- مساعدات CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- مكتبة المُنسِّق: `crates/sorafs_orchestrator`
- حزمة النسخ: `dashboards/grafana/sorafs_fetch_observability.json`