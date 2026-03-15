---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/node-client-protocol.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# بروتوكول لا  عميل da SoraFS

هذه هي الطريقة التي تستأنف بها تعريف البروتوكول الكنسي
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
استخدم إعدادًا محددًا للتخطيطات الأولية Norito على مستوى البايت وسجلات التغيير؛
نسخة من البوابة تحمي عمليات التشغيل المتبقية من دفاتر التشغيل
SoraFS.

## إعلانات إثبات صحة

يقوم المجهزون SoraFS بنشر الحمولات النافعة `ProviderAdvertV1` (فيجا
`crates/sorafs_manifest::provider_advert`) القتلة من قبل المشغل الحاكم. نظام التشغيل
تقوم الإعلانات بتثبيت Metadados de Descoberta وحواجز الحماية التي يستخدمها المنسق
تطبيق متعدد المصادر في وقت التشغيل.- ** فيجينسيا ** - `issued_at < expires_at <= issued_at + 86,400 s`. بروفيدوريس
  يجب تجديده كل 12 ساعة.
- **TLVs de capacidade** - قائمة TLV تعلن عن موارد النقل (Torii،
  QUIC+ضوضاء، مرحلات SoraNet، امتدادات من المصدر). Codigos desconhecidos
  يمكن أن يتم تجاهله عند `allow_unknown_capabilities = true`، التالي
  الشحوم الشرقية.
- **تلميحات حول جودة الخدمة** - المستوى `availability` (ساخن/دافئ/بارد)، أقصى زمن استجابة
  الاسترداد والحد من المزامنة وميزانية البث اختيارية. تطوير جودة الخدمة
  قم بالمراقبة والقياس عن بعد والتدقيق في القبول.
- **نقاط النهاية وموضوعات الالتقاء** - عناوين URL للخدمة المحددة مع التعريفات
  TLS/ALPN المزيد من المواضيع التي يمكن اكتشافها حتى يتمكن العملاء من اكتشافها
  بناء مجموعات الحراسة.
- **سياسة التنوع في المسار** - `min_guard_weight`، قبعات التشجيع
  AS/pool e `provider_failure_threshold` tornam posiveis يجلب المحددات
  متعدد الأقران.
- **معرفات الملفات الشخصية** - يجب أن يقوم المتخصصون بالتصدير أو التعامل مع Canonico (على سبيل المثال.
  `sorafs.sf1@1.0.0`)؛ `profile_aliases` خيار مساعدة العملاء على الهجرة.

Regras de validacao rejeitam share صفر، قوائم الإمكانات/نقاط النهاية/الموضوعات،
خدمات ترتيب أو أهداف جودة الخدمة المتاحة. مغلفات القبول مقارنة
مجموعة الإعلانات والعرض (`compare_core_fields`) قبل النشر
com.atualizacoes.

### جلب امتدادات النطاق

يتضمن نطاق المزودين الأوصاف التالية:| كامبو | اقتراح |
|-------|-----------|
| `CapabilityType::ChunkRangeFetch` | أعلن `max_chunk_span`، `min_granularity` وأعلام التحسين/التجربة. |
| `StreamBudgetV1` | مغلف اختياري للتوافق/الإنتاجية (`max_in_flight`، `max_bytes_per_sec`، `burst` اختياري). تتطلب سعة النطاق. |
| `TransportHintV1` | تفضيلات نقل الأوامر (مثال: `torii_http_range`، `quic_stream`، `soranet_relay`). الأولويات هي `0-15` والنسخ المكررة هي التي تم تجديدها. |

دعم الأدوات:

- تعلن خطوط أنابيب المزود عن مدى صلاحية النطاق وميزانية التدفق e
  تلميحات النقل قبل إرسال الحمولات المحددة للمستمعين.
- `cargo xtask sorafs-admission-fixtures` agrupa يعلن عن canonicos متعدد المصادر
  junto com تخفيض التركيبات em `fixtures/sorafs_manifest/provider_admission/`.
- إعلانات com range que omitem `stream_budget` أو `transport_hints` sao rejeitados
  تقوم أدوات التحميل CLI/SDK المسبقة بالتخطيط، والحفاظ على المصادر المتعددة
  alinhado com كما توقعت القبول Torii.

## نقاط نهاية النطاق تفعل البوابة

تتطلب البوابات المتطلبات الأساسية لتحديدات HTTP التي تستخدمها في التعرف على التعريفات
إعلان.

###`GET /v1/sorafs/storage/car/{manifest_id}`| المتطلبات | ديتالز |
|-----------|----------|
| **العناوين** | `Range` (ملف واحد فقط لإزاحة القطعة)، `dag-scope: block`، `X-SoraFS-Chunker`، `X-SoraFS-Nonce` اختياري و`X-SoraFS-Stream-Token` base64 obrigatorio. |
| **الردود** | `206` مع `Content-Type: application/vnd.ipld.car`، `Content-Range` يتم تنزيله إلى خادم الخدمة، والتوصيفات `X-Sora-Chunk-Range` ورؤوس القطع/الرمز المميز البيئي. |
| **فلحس** | `416` لنطاقات المياه المحلاة، `401` للرموز المميزة المتاحة/غير الصالحة، `429` عندما تكون ميزانيات البث/البايت أكثر من اللازم. |

###`GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

قم بإحضار قطعة فريدة من نوعها باستخدام الرؤوس نفسها أو لخص تحديد القطعة.
يستخدم لإعادة محاولة التنزيلات أو التنزيلات عندما تكون شرائح السيارة ضرورية.

## سير العمل يقوم به orquestrador متعدد المصادر

عند جلب SF-6 متعدد المصادر المجهز (CLI Rust عبر `sorafs_fetch`،
مجموعات SDK عبر `sorafs_orchestrator`):1. **مدخلات جماعية** - فك تشفير أو إظهار مخطط القطع، أو الضغط عليها
   إعلانات أحدث و، اختياريًا، تمرير لقطة القياس عن بعد
   (`--telemetry-json` أو `TelemetrySnapshot`).
2. **إنشاء لوحة النتائج** - `Orchestrator::build_scoreboard` Avalia a
   الأهلية والتسجيل للحصول على حق التصويت؛ `sorafs_fetch --scoreboard-out`
   تستمر يا JSON.
3. **أجزاء جدول الأعمال** - `fetch_with_scoreboard` (ou `--plan`) إعادة فرض القيود
   النطاق، ميزانيات البث، عدد مرات إعادة المحاولة/النظير (`--retry-budget`، `--max-peers`)
   قم بإصدار رمز دفق مع فرصة البيان لكل ما هو مطلوب.
4. **الإيصالات المؤكدة** - كما تتضمن الأقوال `chunk_receipts` و`provider_reports`؛
   ملخصات CLI المستمرة `provider_reports`، `chunk_receipts` e
   `ineligible_providers` لحزم الأدلة.

الأخطاء المشتركة المقدمة للمشغلين/مجموعات SDK:

| خطأ | وصف |
|------|-----------|
| `no providers were supplied` | قم بإدخالها بشكل أنيق باستخدام الفلتر. |
| `no compatible providers available for chunk {index}` | عدم تطابق النطاق أو الميزانية لقطعة معينة. |
| `retry budget exhausted after {attempts}` | قم بزيادة `--retry-budget` أو قم بإزالة أقرانك من الخطأ. |
| `no healthy providers remaining` | كل ما تم إثباته من أجل الإعاقة هو التكرار. |
| `streaming observer failed` | أيها الكاتب CAR المصب أإجهاض. |
| `orchestrator invariant violated` | التقط البيان ولوحة النتائج ولقطات القياس عن بعد وCLI JSON للفرز. |

## القياس عن بعد والأدلة- Metricas emitidas pelo orquestrador:  
  `sorafs_orchestrator_active_fetches`، `sorafs_orchestrator_fetch_duration_ms`،
  `sorafs_orchestrator_retries_total`، `sorafs_orchestrator_provider_failures_total`
  (tagueadas por Manifest/المنطقة/المزود). تعريف `telemetry_region` في التكوين
  أو عبر أعلام CLI للمشاركة في لوحات المعلومات من البداية.
- لا تتضمن ملخصات الجلب CLI/SDK لوحة النتائج JSON المستمرة وإيصالات القطع
  يُبلغ مقدم الخدمة أننا بحاجة إلى حزم طرح للبوابات SF-6/SF-7.
- تعرض معالجات البوابة `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  لكي تتمكن لوحات المعلومات من ربط قرارات المدير بالسلوك
  تفعل الخادم.

## مساعدو CLI e REST

- `iroha app sorafs pin list|show` و`alias list` و`replication list` يشمل نظام التشغيل
  نقاط النهاية REST تقوم بتسجيل الدبوس والطباعة Norito JSON bruto com blocos de
  شهادة لأدلة السمع.
- `iroha app sorafs storage pin` و `torii /v1/sorafs/pin/register` بيانات الاسيتام
  Norito أو JSON com الأسماء المستعارة للإثباتات والخيارات اللاحقة؛ البراهين المشوهة
  geram `400`، البراهين التي لا معنى لها retornam `503` com `Warning: 110`، البراهين الإلكترونية التي انتهت صلاحيتها
  ريتورنام `412`.
- نقاط النهاية REST (`/v1/sorafs/pin`، `/v1/sorafs/aliases`، `/v1/sorafs/replication`)
  تتضمن استراتيجيات التصديق حتى يتمكن العملاء من التحقق من بياناتهم ضد نظام التشغيل
  آخر رؤوس الكتلة قبل التشغيل.

## المراجع- المواصفات الكنسي:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- تيبوس Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- مساعدو CLI: `crates/iroha_cli/src/commands/sorafs.rs`،
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- صندوق orquestrador: `crates/sorafs_orchestrator`
- حزمة لوحات المعلومات: `dashboards/grafana/sorafs_fetch_observability.json`