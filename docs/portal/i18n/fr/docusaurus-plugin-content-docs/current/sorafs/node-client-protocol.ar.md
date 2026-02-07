---
lang: fr
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
استخدم المواصفة Upstream لتخطيطات Norito على مستوى البايت وسجل التغييرات؛
Vous pouvez utiliser les runbooks pour SoraFS.

## إعلانات المزوّد والتحقق

يقوم مزوّدو SoraFS pour `ProviderAdvertV1` (راجع
`crates/sorafs_manifest::provider_advert`) الموقعة من المُشغّل الخاضع للحوكمة.
تثبت الإعلانات بيانات الاكتشاف والضوابط التي يفرضها المُنسِّق متعدد المصادر
أثناء التشغيل.- **مدة الصلاحية** — `issued_at < expires_at ≤ issued_at + 86,400 s`. ينبغي على
  المزوّدين التجديد كل 12 ساعة.
- **TLV القدرات** — Mise à jour du TLV ميزات النقل (Torii, QUIC+Noise, ترحيلات
  SoraNet, امتدادات مزوّد). يمكن تجاهل الأكواد المجهولة عندما
  `allow_unknown_capabilities = true` مع اتباع إرشادات GRAISSE.
- **تلميحات QoS** — طبقة `availability` (Hot/Warm/Froid), أقصى كمون للاسترجاع،
  حد التزامن، وميزانية stream اختيارية. يجب أن تتوافق QoS مع التليمترية
  الملحوظة ويتم تدقيقها في القبول.
- **Points de terminaison et rendez-vous** — عناوين خدمة محددة مع بيانات TLS/ALPN إضافة
  إلى مواضيع الاكتشاف التي يجب أن يشترك بها العملاء عند بناء مجموعات الحراسة.
- **سياسة تنوع المسارات** — `min_guard_weight` et fan-out pour AS/pool et
  `provider_failure_threshold` est un système multi-peer.
- **معرّفات الملف الشخصي** — يجب على المزوّدين نشر المقبض المعتمد (مثل
  `sorafs.sf1@1.0.0`)؛ تساعد `profile_aliases` الاختيارية العملاء القدامى على
  الترحيل.

ترفض قواعد التحقق participation الصفري، أو قوائم القدرات/العناوين/المواضيع الفارغة،
Il s'agit d'une solution de gestion de la qualité de service. تقارن أظرف القبول بين محتوى الإعلان
والاقتراح (`compare_core_fields`) قبل بث التحديثات.

### امتدادات الجلب بالنطاقات

تتضمن المزوّدات الداعمة للنطاق البيانات التالية:| الحقل | الغرض |
|-------|-------|
| `CapabilityType::ChunkRangeFetch` | يعلن `max_chunk_span` و`min_granularity` وأعلام المحاذاة/الأدلة. |
| `StreamBudgetV1` | غلاف اختياري للتزامن/المعدل (`max_in_flight`, `max_bytes_per_sec`, و`burst` اختياري). يتطلب قدرة نطاق. |
| `TransportHintV1` | Utilisez la fonction `torii_http_range`, `quic_stream`, `soranet_relay`. La description est `0–15` et la description. |

دعم الأدوات:

- يجب على خطوط إعلانات المزوّدين التحقق من قدرة النطاق وميزانية stream وتلميحات
  النقل قبل إصدار حمولات حتمية للتدقيق.
- `cargo xtask sorafs-admission-fixtures` يجمع إعلانات متعددة المصادر مع
  luminaires للخفض تحت `fixtures/sorafs_manifest/provider_admission/`.
- تُرفض الإعلانات الداعمة للنطاق التي تُسقط `stream_budget` et `transport_hints`
  Utilisez CLI/SDK pour créer des applications multi-sources
  Il s'agit de Torii.

## نقاط نهاية نطاقات البوابة

Vous pouvez utiliser HTTP pour obtenir des informations sur les connexions HTTP.

### `GET /v1/sorafs/storage/car/{manifest_id}`

| المتطلب | التفاصيل |
|---------|----------|
| **En-têtes** | `Range` (prise en charge par les utilisateurs), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` Il s'agit d'un `X-SoraFS-Stream-Token` base64. |
| **Réponses** | `206` avec `Content-Type: application/vnd.ipld.car` et `Content-Range` pour les appareils `X-Sora-Chunk-Range` et `X-Sora-Chunk-Range` Il s'agit d'un chunker/jeton. |
| **Modes de défaillance** | `416` Dispositifs de sécurité `401` Dispositifs de protection/de sécurité `429` Dispositifs de protection ميزانيات flux/octet. |### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

جلب شريحة واحدة بنفس الرؤوس بالإضافة إلى digest الحتمي للشريحة. مفيد لإعادة
المحاولة أو تنزيلات الطب الشرعي عندما لا تكون شرائح CAR ضرورية.

## سير عمل المُنسِّق متعدد المصادر

Utilisez SF-6 avec la fonction CLI Rust pour `sorafs_fetch` et les SDK.
`sorafs_orchestrator`) :

1. **جمع المدخلات** — فك خطة شرائح المانيفست، وجلب أحدث الإعلانات، وتمرير
   Utilisez le code d'accès (`--telemetry-json` et `TelemetrySnapshot`).
2. **Tableau de bord** — يقوم `Orchestrator::build_scoreboard` بتقييم الأهلية
   وتسجيل أسباب الرفض؛ Il s'agit de `sorafs_fetch --scoreboard-out` ou JSON.
3. **جدولة الشرائح** — يفرض `fetch_with_scoreboard` (أو `--plan`) par `fetch_with_scoreboard` (أو `--plan`)
   ميزانيات stream, وحدود إعادة المحاولة/الأقران (`--retry-budget`, `--max-peers`)
   Ce jeton de flux est disponible pour vous.
4. **التحقق من الإيصالات** — تشمل المخرجات `chunk_receipts` et `provider_reports`؛
   Utilisez la CLI pour `provider_reports` et `chunk_receipts` et `ineligible_providers`.
   لحزم الأدلة.

Voici la liste des SDK :

| الخطأ | الوصف |
|-------|-------|
| `no providers were supplied` | لا توجد إدخالات مؤهلة بعد التصفية. |
| `no compatible providers available for chunk {index}` | عدم توافق نطاق أو ميزانية لشريحة محددة. |
| `retry budget exhausted after {attempts}` | Il s'agit du `--retry-budget`. |
| `no healthy providers remaining` | تم تعطيل جميع المزوّدين بعد إخفاقات متكررة. |
| `streaming observer failed` | انهار كاتب CAR في المسار السفلي. |
| `orchestrator invariant violated` | Les tableaux de bord et les tableaux de bord sont également disponibles en JSON et en CLI. |## التليمترية والأدلة

- المقاييس الصادرة عن المُنسِّق :  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (موسومة حسب manifeste/région/fournisseur). اضبط `telemetry_region` pour le client
  أعلام CLI ليتم تقسيم لوحات المتابعة بحسب الأسطول.
- Prise en charge des éléments CLI/SDK ainsi que du tableau de bord JSON et des éléments de tableau de bord
  وتقارير المزوّدين التي يجب شحنها ضمن حزم الإطلاق لبوابات SF-6/SF-7.
- تكشف معالجات البوابة `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  Il s'agit d'un système SRE qui est doté d'un système de gestion des risques.

## مساعدات CLI وREST

- `iroha app sorafs pin list|show` et `alias list` et `replication list` pour REST
  Les broches sont également Norito JSON pour l'attestation.
- `iroha app sorafs storage pin` و`torii /v1/sorafs/pin/register` manifestes
  Utilisez Norito et JSON pour les preuves pour l'alias et le successeur. تؤدي preuves
  المشوهة إلى `400`, وتُظهر proofs القديمة `503` مع `Warning: 110`, بينما تعيد
  preuves المنتهية تمامًا `412`.
- Fonction REST (`/v1/sorafs/pin`, `/v1/sorafs/aliases`, `/v1/sorafs/replication`)
  تتضمن هياكل attestation حتى يتمكن العملاء من التحقق من البيانات مقابل أحدث
  رؤوس الكتل قبل التنفيذ.

## المراجع

- المواصفة المعتمدة:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Modèle Norito : `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- CLI de référence : `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Nom du produit : `crates/sorafs_orchestrator`
- Nom du produit : `dashboards/grafana/sorafs_fetch_observability.json`