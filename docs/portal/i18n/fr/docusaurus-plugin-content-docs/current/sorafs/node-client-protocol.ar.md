---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# بروتوكول عقدة ↔ عميل SoraFS

يلخص هذا الدليل التعريف المعتمد للبروتوكول في
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
استخدم المواصفة upstream لتخطيطات Norito على مستوى البايت وسجل التغييرات؛
تحافظ نسخة البوابة على أبرز النقاط التشغيلية قرب بقية runbooks الخاصة بـ SoraFS.

## إعلانات المزوّد والتحقق

يقوم مزوّدو SoraFS ببث حمولات `ProviderAdvertV1` (راجع
`crates/sorafs_manifest::provider_advert`) الموقعة من المُشغّل الخاضع للحوكمة.
تثبت الإعلانات بيانات الاكتشاف والضوابط التي يفرضها المُنسِّق متعدد المصادر
أثناء التشغيل.

- **مدة الصلاحية** — `issued_at < expires_at ≤ issued_at + 86,400 s`. ينبغي على
  المزوّدين التجديد كل 12 ساعة.
- **TLV القدرات** — تسرد قائمة TLV ميزات النقل (Torii، QUIC+Noise، ترحيلات
  SoraNet، امتدادات مزوّد). يمكن تجاهل الأكواد المجهولة عندما
  `allow_unknown_capabilities = true` مع اتباع إرشادات GREASE.
- **تلميحات QoS** — طبقة `availability` (Hot/Warm/Cold)، أقصى كمون للاسترجاع،
  حد التزامن، وميزانية stream اختيارية. يجب أن تتوافق QoS مع التليمترية
  الملحوظة ويتم تدقيقها في القبول.
- **Endpoints ومواضيع rendezvous** — عناوين خدمة محددة مع بيانات TLS/ALPN إضافة
  إلى مواضيع الاكتشاف التي يجب أن يشترك بها العملاء عند بناء مجموعات الحراسة.
- **سياسة تنوع المسارات** — `min_guard_weight` وحدود fan-out لمجموعات AS/pool و
  `provider_failure_threshold` تجعل جلب multi-peer الحتمي ممكنًا.
- **معرّفات الملف الشخصي** — يجب على المزوّدين نشر المقبض المعتمد (مثل
  `sorafs.sf1@1.0.0`)؛ تساعد `profile_aliases` الاختيارية العملاء القدامى على
  الترحيل.

ترفض قواعد التحقق stake الصفري، أو قوائم القدرات/العناوين/المواضيع الفارغة،
أو تواريخ غير مرتبة، أو أهداف QoS مفقودة. تقارن أظرف القبول بين محتوى الإعلان
والاقتراح (`compare_core_fields`) قبل بث التحديثات.

### امتدادات الجلب بالنطاقات

تتضمن المزوّدات الداعمة للنطاق البيانات التالية:

| الحقل | الغرض |
|-------|-------|
| `CapabilityType::ChunkRangeFetch` | يعلن `max_chunk_span` و`min_granularity` وأعلام المحاذاة/الأدلة. |
| `StreamBudgetV1` | غلاف اختياري للتزامن/المعدل (`max_in_flight`, `max_bytes_per_sec`, و`burst` اختياري). يتطلب قدرة نطاق. |
| `TransportHintV1` | تفضيلات نقل مرتبة (مثل `torii_http_range`, `quic_stream`, `soranet_relay`). الأولويات ضمن `0–15` ويُرفض التكرار. |

دعم الأدوات:

- يجب على خطوط إعلانات المزوّدين التحقق من قدرة النطاق وميزانية stream وتلميحات
  النقل قبل إصدار حمولات حتمية للتدقيق.
- `cargo xtask sorafs-admission-fixtures` يجمع إعلانات متعددة المصادر مع
  fixtures للخفض تحت `fixtures/sorafs_manifest/provider_admission/`.
- تُرفض الإعلانات الداعمة للنطاق التي تُسقط `stream_budget` أو `transport_hints`
  بواسطة محمّلات CLI/SDK قبل الجدولة، ما يحافظ على توافق مسار multi-source مع
  توقعات قبول Torii.

## نقاط نهاية نطاقات البوابة

تقبل البوابات طلبات HTTP حتمية تعكس بيانات الإعلانات.

### `GET /v1/sorafs/storage/car/{manifest_id}`

| المتطلب | التفاصيل |
|---------|----------|
| **Headers** | `Range` (نافذة واحدة مصطفة مع إزاحات الشرائح)، `dag-scope: block`، `X-SoraFS-Chunker`، `X-SoraFS-Nonce` اختياري، و`X-SoraFS-Stream-Token` base64 إلزامي. |
| **Responses** | `206` مع `Content-Type: application/vnd.ipld.car`، و`Content-Range` يصف النافذة المقدمة، وبيانات `X-Sora-Chunk-Range`، وإعادة إرسال رؤوس chunker/token. |
| **Failure modes** | `416` للنطاقات غير المصطفة، `401` للرموز المفقودة/غير الصالحة، `429` عند تجاوز ميزانيات stream/byte. |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

جلب شريحة واحدة بنفس الرؤوس بالإضافة إلى digest الحتمي للشريحة. مفيد لإعادة
المحاولة أو تنزيلات الطب الشرعي عندما لا تكون شرائح CAR ضرورية.

## سير عمل المُنسِّق متعدد المصادر

عند تفعيل جلب SF-6 متعدد المصادر (CLI Rust عبر `sorafs_fetch`، وSDKs عبر
`sorafs_orchestrator`):

1. **جمع المدخلات** — فك خطة شرائح المانيفست، وجلب أحدث الإعلانات، وتمرير
   لقطة تليمترية اختيارية (`--telemetry-json` أو `TelemetrySnapshot`).
2. **بناء scoreboard** — يقوم `Orchestrator::build_scoreboard` بتقييم الأهلية
   وتسجيل أسباب الرفض؛ يحفظ `sorafs_fetch --scoreboard-out` ملف JSON.
3. **جدولة الشرائح** — يفرض `fetch_with_scoreboard` (أو `--plan`) قيود النطاق،
   ميزانيات stream، وحدود إعادة المحاولة/الأقران (`--retry-budget`, `--max-peers`)
   ويصدر stream token بنطاق المانيفست لكل طلب.
4. **التحقق من الإيصالات** — تشمل المخرجات `chunk_receipts` و`provider_reports`؛
   تحفظ ملخصات CLI كلًا من `provider_reports` و`chunk_receipts` و`ineligible_providers`
   لحزم الأدلة.

أخطاء شائعة تصل إلى المشغلين/SDKs:

| الخطأ | الوصف |
|-------|-------|
| `no providers were supplied` | لا توجد إدخالات مؤهلة بعد التصفية. |
| `no compatible providers available for chunk {index}` | عدم توافق نطاق أو ميزانية لشريحة محددة. |
| `retry budget exhausted after {attempts}` | زد `--retry-budget` أو استبعد الأقران المتعثرين. |
| `no healthy providers remaining` | تم تعطيل جميع المزوّدين بعد إخفاقات متكررة. |
| `streaming observer failed` | انهار كاتب CAR في المسار السفلي. |
| `orchestrator invariant violated` | التقط المانيفست وscoreboard ولقطة التليمترية وJSON الخاص بالـ CLI للتحليل. |

## التليمترية والأدلة

- المقاييس الصادرة عن المُنسِّق:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (موسومة حسب manifest/region/provider). اضبط `telemetry_region` في الإعداد أو عبر
  أعلام CLI ليتم تقسيم لوحات المتابعة بحسب الأسطول.
- تتضمن ملخصات الجلب في CLI/SDK ملف scoreboard JSON المحفوظ وإيصالات الشرائح
  وتقارير المزوّدين التي يجب شحنها ضمن حزم الإطلاق لبوابات SF-6/SF-7.
- تكشف معالجات البوابة `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  كي تتمكن لوحات SRE من ربط قرارات المُنسِّق بسلوك الخادم.

## مساعدات CLI وREST

- `iroha app sorafs pin list|show` و`alias list` و`replication list` تغلف نقاط REST
  الخاصة بسجل pins وتطبع Norito JSON الخام مع كتل attestation لأدلة التدقيق.
- `iroha app sorafs storage pin` و`torii /v1/sorafs/pin/register` يقبلان manifests
  بنمط Norito أو JSON مع proofs اختيارية للـ alias والـ successor؛ تؤدي proofs
  المشوهة إلى `400`، وتُظهر proofs القديمة `503` مع `Warning: 110`، بينما تعيد
  proofs المنتهية تمامًا `412`.
- نقاط REST (`/v1/sorafs/pin`, `/v1/sorafs/aliases`, `/v1/sorafs/replication`)
  تتضمن هياكل attestation حتى يتمكن العملاء من التحقق من البيانات مقابل أحدث
  رؤوس الكتل قبل التنفيذ.

## المراجع

- المواصفة المعتمدة:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- أنواع Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- مساعدات CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- مكتبة المُنسِّق: `crates/sorafs_orchestrator`
- حزمة لوحات المتابعة: `dashboards/grafana/sorafs_fetch_observability.json`
