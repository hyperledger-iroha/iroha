---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# بروتوكول عقدة ↔ عميل SoraFS

يلخص هذا الدليل التعريف المعتمد للبروتوكول في
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Fuentes de alimentación upstream Norito para conexiones y conexiones
Para obtener más información, consulte los runbooks de SoraFS.

## إعلانات المزوّد والتحقق

يقوم مزوّدو SoraFS ببث حمولات `ProviderAdvertV1` (راجع
`crates/sorafs_manifest::provider_advert`) الموقعة من المُشغّل الخاضع للحوكمة.
تثبت الإعلانات بيانات الاكتشاف والضوابط التي يفرضها المُنسِّق متعدد المصادر
أثناء التشغيل.- **مدة الصلاحية** — `issued_at < expires_at ≤ issued_at + 86,400 s`. ينبغي على
  المزوّدين التجديد كل 12 ساعة.
- **TLV القدرات** — تسرد قائمة TLV ميزات النقل (Torii, QUIC+Noise, ترحيلات
  SoraNet, امتدادات مزوّد). يمكن تجاهل الأكواد المجهولة عندما
  `allow_unknown_capabilities = true` مع اتباع إرشادات GREASE.
- **تلميحات QoS** — `availability` (Caliente/Tibio/Frío)
  حد التزامن، وميزانية corriente اختيارية. يجب أن تتوافق QoS مع التليمترية
  الملحوظة ويتم تدقيقها في القبول.
- **Enlaces y puntos finales** — عناوين خدمة محددة مع بيانات TLS/ALPN إضافة
  إلى مواضيع الاكتشاف التي يجب أن يشترك بها العملاء عند بناء مجموعات الحراسة.
- **سياسة تنوع المسارات** — `min_guard_weight` y distribución en abanico de AS/pool y
  `provider_failure_threshold` Esta es una conexión multi-peer.
- **معرّفات الملف الشخصي** — يجب على المزوّدين نشر المقبض المعتمد (مثل
  `sorafs.sf1@1.0.0`)؛ تساعد `profile_aliases` الاختيارية العملاء القدامى على
  الترحيل.

ترفض قواعد التحقق participación الصفري، أو قوائم القدرات/العناوين/المواضيع الفارغة،
أو تواريخ غير مرتبة, أو أهداف QoS مفقودة. تقارن أظرف القبول بين محتوى الإعلان
والاقتراح (`compare_core_fields`) قبل بث التحديثات.

### امتدادات الجلب بالنطاقات

تضمن المزوّدات الداعمة للنطاق البيانات التالية:| الحقل | الغرض |
|-------|-------|
| `CapabilityType::ChunkRangeFetch` | Aquí están `max_chunk_span` y `min_granularity` y están conectados a la red. |
| `StreamBudgetV1` | غلاف اختياري للتزامن/المعدل (`max_in_flight`, `max_bytes_per_sec`, y `burst` اختياري). يتطلب قدرة نطاق. |
| `TransportHintV1` | تفضيلات نقل مرتبة (مثل `torii_http_range`, `quic_stream`, `soranet_relay`). الأولويات ضمن `0–15` ويُرفض التكرار. |

دعم الأدوات:

- يجب على خطوط إعلانات المزوّدين التحقق من قدرة النطاق وميزانية stream وتلميحات
  النقل قبل إصدار حمولات حتمية للتدقيق.
- `cargo xtask sorafs-admission-fixtures` يجمع إعلانات متعددة المصادر مع
  accesorios للخفض تحت `fixtures/sorafs_manifest/provider_admission/`.
- تُرفض الإعلانات الداعمة للنطاق التي تُسقط `stream_budget` أو `transport_hints`
  بواسطة محمّلات CLI/SDK قبل الجدولة، ما يحافظ على توافق مسار مع
  توقعات قبول Torii.

## نقاط نهاية نطاقات البوابة

تقبل البوابات طلبات HTTP حتمية تعكس بيانات الإعلانات.

### `GET /v2/sorafs/storage/car/{manifest_id}`

| المتطلب | التفاصيل |
|---------|----------|
| **Encabezados** | `Range` (Nombre y nombre de la marca) `dag-scope: block` `X-SoraFS-Chunker` `X-SoraFS-Nonce` Y `X-SoraFS-Stream-Token` base64. |
| **Respuestas** | `206` مع `Content-Type: application/vnd.ipld.car`, و`Content-Range` يصف النافذة المقدمة, وبيانات `X-Sora-Chunk-Range`, وإعادة إرسال رؤوس trozo/ficha. |
| **Modos de fallo** | `416` Accesorios para el hogar, `401` Accesorios para el hogar/para el hogar, `429` Otros productos ميزانيات flujo/byte. |### `GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

جلب شريحة واحدة بنفس الرؤوس بالإضافة إلى digest الحتمي للشريحة. مفيد لإعادة
المحاولة أو تنزيلات الطب الشرعي عندما لا تكون شرائح CAR ضرورية.

## سير عمل المُنسِّق متعدد المصادر

Utilice el software SF-6 (CLI Rust para `sorafs_fetch` y SDK)
`sorafs_orchestrator`):

1. **جمع المدخلات** — فك خطة شرائح المانيفست، وجلب أحدث الإعلانات، وتمرير
   Utilice el software correspondiente (`--telemetry-json` o `TelemetrySnapshot`).
2. **marcador de بناء** — يقوم `Orchestrator::build_scoreboard` بتقييم الأهلية
   وتسجيل أسباب الرفض؛ Aquí `sorafs_fetch --scoreboard-out` es JSON.
3. **جدولة الشرائح** — يفرض `fetch_with_scoreboard` (أو `--plan`) قيود النطاق،
   ميزانيات stream, وحدود إعادة المحاولة/الأقران (`--retry-budget`, `--max-peers`)
   ويصدر token de flujo بنطاق المانيفست لكل طلب.
4. **التحقق من الإيصالات** — تشمل المخرجات `chunk_receipts` e `provider_reports`؛
   Configuraciones CLI para `provider_reports`, `chunk_receipts` y `ineligible_providers`
   لحزم الأدلة.

أخطاء شائعة تصل إلى المشغلين/SDK:

| الخطأ | الوصف |
|-------|-------|
| `no providers were supplied` | لا توجد إدخالات مؤهلة بعد التصفية. |
| `no compatible providers available for chunk {index}` | عدم توافق نطاق أو ميزانية لشريحة محددة. |
| `retry budget exhausted after {attempts}` | زد `--retry-budget` أو استبعد الأقران المتعثرين. |
| `no healthy providers remaining` | تم تعطيل جميع المزوّدين بعد إخفاقات متكررة. |
| `streaming observer failed` | انهار كاتب CAR في المسار السفلي. |
| `orchestrator invariant violated` | التقط المانيفست و لقطة التليمترية و JSON الخاص بالـ CLI للتحليل. |## التليمترية والأدلة

- المقاييس الصادرة عن المُنسِّق:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (موسومة حسب manifiesto/región/proveedor). اضبط `telemetry_region` في الإعداد أو عبر
  Haga clic en CLI para configurar los ajustes.
- Aplicación de CLI/SDK para el marcador JSON y configuración de archivos
  وتقارير المزوّدين التي يجب شحنها ضمن حزم الإطلاق لبوابات SF-6/SF-7.
- تكشف معالجات البوابة `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  كي تتمكن لوحات SRE من ربط قرارات المُنسِّق بسلوك الخادم.

## مساعدات CLI وREST

- `iroha app sorafs pin list|show`, `alias list` y `replication list` para REST
  Haga clic en los pines y Norito JSON para obtener la certificación de atestación.
- `iroha app sorafs storage pin` y `torii /v2/sorafs/pin/register` manifiestos
  بنمط Norito أو JSON مع pruebas اختيارية للـ alias yالـ sucesor؛ تؤدي pruebas
  Pruebas de `400` y pruebas de `503` y de `Warning: 110`
  pruebas المنتهية تمامًا `412`.
- RESTO (`/v2/sorafs/pin`, `/v2/sorafs/aliases`, `/v2/sorafs/replication`)
  تضمن هياكل atestación حتى يتمكن العملاء من التحقق من البيانات مقابل أحدث
  رؤوس الكتل قبل التنفيذ.

## المراجع

- المواصفة المعتمدة:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Nombre Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Referencia CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- مكتبة المُنسِّق: `crates/sorafs_orchestrator`
- حزمة لوحات المتابعة: `dashboards/grafana/sorafs_fetch_observability.json`