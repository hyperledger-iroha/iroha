---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sorafs/storage-capacity-marketplace.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9a6ace39ffa9bb83f82e2f1d927cb24587dce1995cabc43cf5a6b6817cf18c3c
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Arabic (ar) translation. Replace this content with the full translation. -->

---
id: storage-capacity-marketplace
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/storage-capacity-marketplace.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note المصدر المعتمد
هذه الصفحة تعكس `docs/source/sorafs/storage_capacity_marketplace.md`. حافظوا على تزامن النسختين ما دامت الوثائق القديمة نشطة.
:::

# سوق سعة التخزين في SoraFS (مسودة SF-2c)

يقدّم بند خارطة الطريق SF-2c سوقا محكوما حيث يعلن مزودو التخزين عن سعة ملتزم بها، ويتلقون أوامر النسخ المتماثل، ويكسبون رسوما تتناسب مع التوافر المُسلَّم. يحدد هذا المستند نطاق المخرجات المطلوبة للإصدار الأول ويقسمها إلى مسارات قابلة للتنفيذ.

## الأهداف

- التعبير عن التزامات سعة المزودين (إجمالي البايتات، حدود لكل lane، تاريخ الانتهاء) بصيغة قابلة للتحقق يمكن للحوكمة ونقل SoraNet و Torii استهلاكها.
- توزيع pins عبر المزودين وفق السعة المعلنة و stake وقيود السياسة مع الحفاظ على سلوك حتمي.
- قياس تسليم التخزين (نجاح النسخ المتماثل، uptime، أدلة السلامة) وتصدير التليمترية لتوزيع الرسوم.
- توفير عمليات الإلغاء والنزاع حتى يمكن معاقبة المزودين غير الأمناء أو إزالتهم.

## مفاهيم النطاق

| المفهوم | الوصف | المخرج الأولي |
|---------|-------------|---------------------|
| `CapacityDeclarationV1` | حمولة Norito تصف معرف المزود، دعم ملف تعريف chunker، GiB الملتزمة، حدود خاصة بـ lane، تلميحات التسعير، التزام staking، وتاريخ الانتهاء. | المخطط + المدقق في `sorafs_manifest::capacity`. |
| `ReplicationOrder` | تعليمات صادرة عن الحوكمة تعيّن CID للـ manifest إلى مزود واحد أو أكثر، بما في ذلك مستوى التكرار ومقاييس SLA. | مخطط Norito مشترك مع Torii + واجهة smart contract. |
| `CapacityLedger` | سجل on-chain/off-chain يتتبع إعلانات السعة النشطة، أوامر النسخ المتماثل، مقاييس الأداء، وتراكم الرسوم. | وحدة smart contract أو stub لخدمة off-chain مع snapshot حتمي. |
| `MarketplacePolicy` | سياسة حوكمة تحدد الحد الأدنى لـ stake ومتطلبات التدقيق ومنحنيات العقوبة. | config struct في `sorafs_manifest` + وثيقة حوكمة. |

### المخططات المنفذة (الحالة)

## تفصيل العمل

### 1. طبقة المخططات والسجل

| المهمة | Owner(s) | ملاحظات |
|------|----------|-------|
| تعريف `CapacityDeclarationV1` و `ReplicationOrderV1` و `CapacityTelemetryV1`. | Storage Team / Governance | استخدم Norito؛ وأدرج النسخ الدلالية ومراجع القدرات. |
| تنفيذ وحدات parser + validator في `sorafs_manifest`. | Storage Team | فرض IDs أحادية، حدود السعة، ومتطلبات stake. |
| توسيع metadata لسجل chunker بإضافة `min_capacity_gib` لكل ملف تعريف. | Tooling WG | يساعد العملاء على فرض الحد الأدنى من متطلبات العتاد لكل ملف تعريف. |
| صياغة وثيقة `MarketplacePolicy` التي تلتقط ضوابط القبول وجدول العقوبات. | Governance Council | انشرها في docs بجانب policy defaults. |

#### تعريفات المخطط (منفذة)

- يلتقط `CapacityDeclarationV1` التزامات سعة موقعة لكل مزود، بما في ذلك handles chunker القياسية، مراجع capabilities، حدود اختيارية لكل lane، تلميحات التسعير، نوافذ الصلاحية، وmetadata. تضمن عملية التحقق stake غير صفري، handles قياسية، aliases مزالة التكرار، حدود lane ضمن الإجمالي المعلن، ومحاسبة GiB أحادية.【crates/sorafs_manifest/src/capacity.rs:28】
- يربط `ReplicationOrderV1` manifests بتعيينات صادرة عن الحوكمة مع أهداف التكرار، عتبات SLA، وضمانات لكل assignment؛ تفرض validators handles chunker القياسية، مزودين فريدين، وقيود deadline قبل أن يقوم Torii أو السجل بابتلاع الأمر.【crates/sorafs_manifest/src/capacity.rs:301】
- يعبر `CapacityTelemetryV1` عن snapshots الحقبة (GiB المعلنة مقابل المستخدمة، عدادات النسخ، نسب uptime/PoR) التي تغذي توزيع الرسوم. تحقق الحدود يبقي الاستخدام ضمن الإعلانات والنسب ضمن 0-100%.【crates/sorafs_manifest/src/capacity.rs:476】
- توفر helpers المشتركة (`CapacityMetadataEntry` و`PricingScheduleV1` ومدققات lane/assignment/SLA) تحقق مفاتيح حتمي وتقارير أخطاء يمكن لـ CI والـ downstream tooling إعادة استخدامها.【crates/sorafs_manifest/src/capacity.rs:230】
- يعرض `PinProviderRegistry` الآن snapshot على السلسلة عبر `/v1/sorafs/capacity/state`، جامعاً إعلانات المزودين وإدخالات fee ledger خلف Norito JSON حتمي.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- تغطي اختبارات التحقق فرض handles القياسية، كشف التكرار، حدود lane، حمايات تعيين النسخ المتماثل، وفحوص نطاق التليمترية حتى تظهر الانحدارات فوراً في CI.【crates/sorafs_manifest/src/capacity.rs:792】
- أدوات المشغل: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` تحول specs المقروءة من البشر إلى Norito payloads قياسية، base64 blobs وملخصات JSON حتى يتمكن المشغلون من تجهيز fixtures لـ `/v1/sorafs/capacity/declare` و`/v1/sorafs/capacity/telemetry` وأوامر النسخ المتماثل مع تحقق محلي.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 توجد Reference fixtures في `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) ويتم توليدها عبر `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. تكامل طبقة التحكم

| المهمة | Owner(s) | ملاحظات |
|------|----------|-------|
| إضافة معالجات Torii لـ `/v1/sorafs/capacity/declare` و`/v1/sorafs/capacity/telemetry` و`/v1/sorafs/capacity/orders` بحمولات Norito JSON. | Torii Team | محاكاة منطق التحقق؛ إعادة استخدام Norito JSON helpers. |
| تمرير snapshots الخاصة بـ `CapacityDeclarationV1` إلى metadata لوحة نقاط orchestrator وخطط الجلب في gateway. | Tooling WG / Orchestrator team | تمديد `provider_metadata` بمراجع السعة حتى يحترم scoring متعدد المصادر حدود lane. |
| تغذية أوامر النسخ المتماثل إلى عملاء orchestrator/gateway لقيادة assignments وتلميحات failover. | Networking TL / Gateway team | يستهلك Scoreboard builder أوامر النسخ الموقعة من الحوكمة. |
| أدوات CLI: توسيع `sorafs_cli` بـ `capacity declare` و`capacity telemetry` و`capacity orders import`. | Tooling WG | توفير JSON حتمي + مخرجات scoreboard. |

### 3. سياسة السوق والحوكمة

| المهمة | Owner(s) | ملاحظات |
|------|----------|-------|
| إقرار `MarketplacePolicy` (الحد الأدنى لـ stake، مضاعفات العقوبة، وتواتر التدقيق). | Governance Council | نشرها في docs وتسجيل سجل المراجعات. |
| إضافة خطافات الحوكمة حتى يستطيع Parliament اعتماد وتجديد وإلغاء declarations. | Governance Council / Smart Contract team | استخدام Norito events + manifest ingestion. |
| تنفيذ جدول العقوبات (تخفيض الرسوم، slashing للبوند) المرتبط بانتهاكات SLA المقاسة عن بعد. | Governance Council / Treasury | مواءمة ذلك مع مخرجات التسوية في `DealEngine`. |
| توثيق عملية النزاع ومصفوفة التصعيد. | Docs / Governance | الربط إلى dispute runbook + CLI helpers. |

### 4. الميترينغ وتوزيع الرسوم

| المهمة | Owner(s) | ملاحظات |
|------|----------|-------|
| توسيع ingest الميترينغ في Torii لقبول `CapacityTelemetryV1`. | Torii Team | التحقق من GiB-hour ونجاح PoR و uptime. |
| تحديث خط أنابيب الميترينغ في `sorafs_node` للإبلاغ عن استخدام لكل أمر + إحصاءات SLA. | Storage Team | المواءمة مع أوامر النسخ المتماثل وhandles chunker. |
| خط أنابيب التسوية: تحويل التليمترية + بيانات النسخ إلى payouts مقومة بـ XOR، وإنتاج ملخصات جاهزة للحوكمة، وتسجيل حالة ledger. | Treasury / Storage Team | التوصيل إلى Deal Engine / Treasury exports. |
| تصدير dashboards/alerts لصحة الميترينغ (backlog ingestion، تليمترية قديمة). | Observability | توسيع حزمة Grafana المشار إليها في SF-6/SF-7. |

- يعرض Torii الآن `/v1/sorafs/capacity/telemetry` و`/v1/sorafs/capacity/state` (JSON + Norito) بحيث يمكن للمشغلين إرسال snapshots تليمترية لكل حقبة ويمكن للمراجعين استرجاع ledger الحتمي للتدقيق أو تغليف الأدلة.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- يضمن تكامل `PinProviderRegistry` إمكانية الوصول إلى أوامر النسخ المتماثل عبر نفس endpoint؛ تساعد أدوات CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) في التحقق/النشر لتليمترية تشغيلات الأتمتة مع hashing حتمي وحل alias.
- تنتج snapshots الميترينغ إدخالات `CapacityTelemetrySnapshot` المثبتة على snapshot `metering`، وتغذي Prometheus exports لوحة Grafana الجاهزة للاستيراد في `docs/source/grafana_sorafs_metering.json` حتى تتمكن فرق الفوترة من مراقبة تراكم GiB-hour والرسوم nano-SORA المتوقعة والامتثال لـ SLA في الوقت الحقيقي.【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- عند تفعيل metering smoothing، يتضمن snapshot الحقول `smoothed_gib_hours` و`smoothed_por_success_bps` حتى يتمكن المشغلون من مقارنة قيم EMA الملساء بالعدادات الخام التي تعتمد عليها الحوكمة في payouts.【crates/sorafs_node/src/metering.rs:401】

### 5. معالجة النزاعات والإلغاء

| المهمة | Owner(s) | ملاحظات |
|------|----------|-------|
| تعريف حمولة `CapacityDisputeV1` (صاحب الشكوى، evidence، المزود المستهدف). | Governance Council | مخطط Norito + مدقق. |
| دعم CLI لتقديم disputes والرد عليها (مع مرفقات evidence). | Tooling WG | ضمان hashing حتمي لحزمة evidence. |
| إضافة فحوص تلقائية لتكرار خروقات SLA (تصعيد تلقائي إلى dispute). | Observability | عتبات تنبيه وخطافات حوكمة. |
| توثيق playbook الإلغاء (فترة سماح، إخلاء بيانات pinned). | Docs / Storage Team | الربط إلى وثيقة السياسة وoperator runbook. |

## متطلبات الاختبار و CI

- اختبارات وحدات لكل مدققات المخطط الجديدة (`sorafs_manifest`).
- اختبارات تكامل تحاكي: إعلان → أمر نسخ → metering → payout.
- workflow في CI لإعادة توليد إعلانات/تليمترية السعة وضمان تزامن التواقيع (توسيع `ci/check_sorafs_fixtures.sh`).
- اختبارات تحميل لواجهة registry API (محاكاة 10k مزود و100k أمر).

## التليمترية ولوحات المعلومات

- لوحات dashboard:
  - السعة المعلنة مقابل المستخدمة لكل مزود.
  - backlog أوامر النسخ ومتوسط تأخير التعيين.
  - امتثال SLA (نسبة uptime، معدل نجاح PoR).
  - تراكم الرسوم والغرامات لكل حقبة.
- Alerts:
  - مزود أقل من الحد الأدنى للسعة الملتزم بها.
  - أمر نسخ متعثر لأكثر من SLA.
  - إخفاقات خط أنابيب الميترينغ.

## مخرجات التوثيق

- دليل المشغل لإعلان السعة وتجديد الالتزامات ومراقبة الاستخدام.
- دليل الحوكمة للموافقة على الإعلانات وإصدار الأوامر ومعالجة النزاعات.
- مرجع API لنقاط نهاية السعة وتنسيق أوامر النسخ المتماثل.
- أسئلة شائعة للـ marketplace موجهة للمطورين.

## قائمة تحقق جاهزية GA

بند خارطة الطريق **SF-2c** يبوّب إطلاق الإنتاج على أدلة ملموسة عبر المحاسبة ومعالجة النزاعات والالتحاق. استخدموا الأدوات أدناه لإبقاء معايير القبول متزامنة مع التنفيذ.

### Nightly accounting & XOR reconciliation
- صدّر snapshot حالة السعة وملف XOR ledger للفترة نفسها، ثم شغّل:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  ينهي المساعد التنفيذ بكود غير صفري عند وجود تسويات أو غرامات مفقودة/زائدة، ويصدر ملخصا بصيغة Prometheus textfile.
- تنبيه `SoraFSCapacityReconciliationMismatch` (ضمن `dashboards/alerts/sorafs_capacity_rules.yml`)
  يطلق عندما تشير مقاييس reconciliation إلى فجوات؛ لوحات المعلومات موجودة تحت
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- أرشِف ملخص JSON و hashes داخل `docs/examples/sorafs_capacity_marketplace_validation/`
  بجانب governance packets.

### Dispute & slashing evidence
- قدّم disputes عبر `sorafs_manifest_stub capacity dispute` (اختبارات:
  `cargo test -p sorafs_car --test capacity_cli`) حتى تبقى payloads قياسية.
- شغّل `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` وحزم العقوبات
  (`record_capacity_telemetry_penalises_persistent_under_delivery`) لإثبات أن disputes و slashes تُعاد حتميا.
- اتبع `docs/source/sorafs/dispute_revocation_runbook.md` لالتقاط الأدلة والتصعيد؛ اربط موافقات strike في تقرير التحقق.

### Provider onboarding & exit smoke tests
- أعد توليد artefacts للإعلان/التليمترية باستخدام `sorafs_manifest_stub capacity ...` وأعد تشغيل اختبارات CLI قبل الإرسال (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- أرسل عبر Torii (`/v1/sorafs/capacity/declare`) ثم التقط `/v1/sorafs/capacity/state` مع لقطات Grafana. اتبع مسار الخروج في `docs/source/sorafs/capacity_onboarding_runbook.md`.
- أرشِف artefacts الموقعة ومخرجات reconciliation داخل `docs/examples/sorafs_capacity_marketplace_validation/`.

## الاعتماديات والتسلسل

1. إكمال SF-2b (admission policy) - يعتمد marketplace على مزودين مدققين.
2. تنفيذ طبقة المخطط + السجل (هذا المستند) قبل تكامل Torii.
3. إكمال metering pipeline قبل تفعيل المدفوعات.
4. الخطوة الأخيرة: تفعيل توزيع الرسوم المحكوم من الحوكمة بعد التحقق من بيانات metering في staging.

يجب تتبع التقدم في roadmap مع الإحالات إلى هذا المستند. حدّث roadmap بمجرد وصول كل قسم رئيسي (المخططات، control plane، التكامل، metering، معالجة النزاعات) إلى حالة feature complete.
