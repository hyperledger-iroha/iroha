---
lang: ar
direction: rtl
source: docs/source/nexus_compliance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5635794e962a9fb1b94c5ff550dc198a744a64b4f9f05df588cb70621e9237f9
source_last_modified: "2025-11-21T18:31:26.542844+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/nexus_compliance.md -->

# محرك امتثال مسارات Nexus وسياسة القائمة البيضاء (NX-12)

الحالة: 🈴 منفذ — يصف هذا المستند نموذج السياسة الفعلي وفرضه الحاسم للاجماع المشار اليه في بند خارطة الطريق **NX-12 — محرك امتثال المسارات وسياسة القائمة البيضاء**.
يشرح نموذج البيانات ومسارات الحوكمة والقياس عن بعد واستراتيجية الاطلاق المنفذة داخل `crates/iroha_core/src/compliance` والمطبقة في قبول Torii وفي تحقق معاملات `iroha_core`، حتى يمكن ربط كل lane وكل dataspace بسياسات قضائية حتمية.

## الاهداف

- تمكين الحوكمة من ارفاق قواعد السماح/المنع، اعلام القضائية، حدود تحويل CBDC، ومتطلبات التدقيق لكل manifest للـ lane.
- تقييم كل معاملة مقابل تلك القواعد اثناء قبول Torii وتنفيذ الكتلة، مع ضمان فرض السياسة بشكل حتمي عبر العقد.
- انتاج مسار تدقيق قابل للتحقق تشفيريا، مع حزم ادلة Norito وقياس عن بعد قابل للاستعلام للجهات التنظيمية والمشغلين.
- ابقاء النموذج مرنا: نفس policy engine يغطي مسارات CBDC الخاصة، DS العامة للتسوية، و dataspaces الشركاء الهجينة دون تفريعات مخصصة.

## خارج النطاق

- عدم تعريف اجراءات AML/KYC او مسارات تصعيد قانونية. هذه موجودة في playbooks الامتثال التي تستهلك القياس عن بعد المنتج هنا.
- عدم ادخال مفاتيح تبديل لكل تعليمة في IVM؛ المحرك يتحكم فقط في الحسابات/الاصول/النطاقات التي يمكنها ارسال المعاملات او التفاعل مع lane.
- عدم الغاء Space Directory. تظل manifests هي المصدر الموثوق لبيانات DS؛ سياسة الامتثال تشير فقط الى مدخلات Space Directory وتكملها.

## نموذج السياسة

### الكيانات والمعرفات

يعمل محرك السياسة على:

- `LaneId` / `DataSpaceId` — يحدد النطاق الذي تطبق فيه القواعد.
- `UniversalAccountId (UAID)` — يسمح بتجميع الهويات cross-lane.
- `JurisdictionFlag` — bitmask يسرد التصنيفات التنظيمية (مثل `EU_EEA`, `JP_FIEL`, `US_FED`, `SANCTIONS_SCREENED`).
- `ParticipantSelector` — يصف من يتاثر:
  - `AccountId` او `DomainId` او `UAID`.
  - محددات مبنية على البادئة (`DomainPrefix`, `UaidPrefix`) لمطابقة السجلات.
  - `CapabilityTag` ل manifests في Space Directory (مثل DS التي تم اعتمادها FX فقط).
  - بوابة `privacy_commitments_any_of` لفرض اعلان المسارات عن التزامات الخصوصية في Nexus قبل مطابقة القواعد
    (يعكس سطح manifest لـ NX-10 ويطبق في لقطات `LanePrivacyRegistry`).

### LaneCompliancePolicy

السياسات هي هياكل Norito منشورة عبر الحوكمة:

```text
LaneCompliancePolicy {
    id: LaneCompliancePolicyId,
    version: u32,
    lane_id: LaneId,
    jurisdiction: JurisdictionSet,
    allow: Vec<AllowRule>,
    deny: Vec<DenyRule>,
    transfer_limits: Vec<TransferLimit>,
    audit_controls: AuditControls,
    metadata: MetadataMap,
}
```

- `AllowRule` يجمع `ParticipantSelector` مع override قضائي اختياري و capability tags ورموز الاسباب.
- `DenyRule` يعكس بنية allow لكنه يقيم اولا (deny يفوز).
- `TransferLimit` يلتقط حدودا محددة حسب الاصل/bucket:
  - `max_notional_xor` و `max_daily_notional_xor`.
  - `asset_limits[{asset_id, per_tx, per_day}]`.
  - `relationship_limits` (مثل CBDC retail مقابل wholesale).
- `AuditControls` يضبط:
  - ما اذا كان Torii يجب ان يحفظ كل رفض في سجل التدقيق.
  - ما اذا كانت القرارات الناجحة يجب ان تؤخذ عينات منها في Norito digests.
  - نافذة الاحتفاظ المطلوبة لـ `LaneComplianceDecisionRecord`.

### التخزين والتوزيع

- توجد احدث hashes للسياسة في manifest الخاص بـ Space Directory بجوار مفاتيح المدققين.
  يصبح `LaneCompliancePolicyReference` (policy id + version + hash) حقلا في manifest ليتمكن المدققون و SDKs من جلب policy blob القياسي.
- يكشف `iroha_config` عن `compliance.policy_cache_dir` لتخزين Norito payload وتوقيعه المنفصل.
  تتحقق العقد من التواقيع قبل تطبيق التحديثات للحماية من العبث.
- يتم تضمين السياسات ايضا داخل Norito admission manifests المستخدمة بواسطة Torii كي تتمكن CI/SDKs من اعادة تشغيل تقييم السياسات دون الاتصال بالمدققين.

## الحوكمة ودورة الحياة

1. **اقتراح** — ترسل الحوكمة `ProposeLaneCompliancePolicy` مع Norito payload ومبرر الاختصاص وفترة التفعيل.
2. **مراجعة** — يوقع مراجعو الامتثال `LaneCompliancePolicyReviewEvidence` (قابل للتدقيق ومخزن في `governance::ReviewEvidenceStore`).
3. **تفعيل** — بعد نافذة التاخير، يستوعب المدققون السياسة عبر استدعاء `ActivateLaneCompliancePolicy`.
   يتم تحديث manifest في Space Directory بشكل ذري مع مرجع السياسة الجديد.
4. **تعديل/الغاء** — يحمل `AmendLaneCompliancePolicy` بيانات diff مع الاحتفاظ بالنسخة السابقة لاعادة التشغيل الجنائي؛
   يقوم `RevokeLaneCompliancePolicy` بتثبيت policy id على `denied` بحيث يرفض Torii اي حركة موجهة لتلك lane حتى يتم تفعيل بديل.

يوفر Torii:

- `GET /v2/lane-compliance/policies/{lane_id}` — جلب مرجع السياسة الاحدث.
- `POST /v2/lane-compliance/policies` — endpoint خاص بالحوكمة يعكس ISI proposal helpers.
- `GET /v2/lane-compliance/decisions` — سجل تدقيق分页 مع مرشحات لـ `lane_id`, `decision`, `jurisdiction`, و `reason_code`.

تغلف اوامر CLI/SDK هذه الاسطح HTTP ليتمكن المشغلون من اتمتة المراجعات وجلب artefacts
( policy blob موقع + reviewer attestations).

## مسار الفرض

1. **القبول (Torii)**
   - يقوم `Torii` بتنزيل السياسة النشطة عند تغيير lane manifest او عند انتهاء توقيع الكاش.
   - يتم وسم كل معاملة تدخل طابور `/v2/pipeline` بـ `LaneComplianceContext`
     (ids المشاركين، UAID، بيانات manifest للـ dataspace، policy id، واحدث snapshot لـ `LanePrivacyRegistry` الموصوف في `crates/iroha_core/src/interlane/mod.rs`).
   - يجب على السلطات الحاملة لـ UAID امتلاك manifest نشط في Space Directory للـ dataspace الموجه؛
     يرفض Torii المعاملات عندما لا يكون UAID مرتبطا بذلك dataspace قبل تقييم اي قواعد سياسة.
   - يقوم `compliance::Engine` بتقييم قواعد `deny` ثم قواعد `allow` ثم يطبق حدود التحويل.
     تعيد المعاملات الفاشلة خطأ نمطيا (`ERR_LANE_COMPLIANCE_DENIED`) مع السبب و policy id لمسارات التدقيق.
   - القبول مرشح سريع؛ يعيد تحقق الاجماع فحص القواعد نفسها باستخدام snapshots الحالة للحفاظ على الحتمية.
2. **التنفيذ (iroha_core)**
   - اثناء بناء الكتلة، يعيد `iroha_core::tx::validate_transaction_internal` نفس فحوصات حوكمة/UAID/الخصوصية/الامتثال للـ lane
     باستخدام snapshots `StateTransaction` (`lane_manifests`, `lane_privacy_registry`,
     `lane_compliance`). يحافظ ذلك على فرض حاسم للاجماع حتى لو كانت كاشات Torii قديمة.
   - المعاملات التي تغير manifests الخاصة بالـ lane او سياسات الامتثال تمر بنفس مسار التحقق؛
     لا يوجد bypass خاص بالقبول فقط.
3. **خطافات غير متزامنة**
   - يقوم RBC gossip و DA fetchers بارفاق policy id مع القياس عن بعد حتى يمكن ربط القرارات المتاخرة بالاصدار الصحيح من القواعد.
   - يعرض `iroha_cli` و SDK helpers الدالة `LaneComplianceDecision::explain()` كي تتمكن الاتمتة من انتاج تشخيص قابل للقراءة.

المحرك حتمي ونقي؛ لا يتواصل مع انظمة خارجية بعد تنزيل manifest/policy.
هذا يبسط CI fixtures واعادة الانتاج عبر عدة عقد.

## التدقيق والقياس عن بعد

- **المقاييس**
  - `nexus_lane_policy_decisions_total{lane_id,decision,reason}`.
  - `nexus_lane_policy_rate_limited_total{lane_id,limit_kind}`.
  - `nexus_lane_policy_cache_age_seconds{lane_id}` (يجب ان يبقى < مهلة التفعيل).
- **السجلات**
  - تلتقط السجلات المنظمة `policy_id`, `version`, `participant`, `UAID`، اعلام القضائية و Norito hash للمعاملة المخالفة.
  - يتم ترميز `LaneComplianceDecisionRecord` في Norito وحفظه تحت `world.compliance_logs::<lane_id>::<ts>::<nonce>` عندما تطلب `AuditControls` تخزينا دائما.
- **حزم الادلة**
  - يضيف `cargo xtask nexus-lane-audit` وضع `--lane-compliance <path>` يدمج السياسة وتواقيع المراجعين ولقطة المقاييس
    واحدث سجل تدقيق في مخرجات JSON + Parquet. يتوقع الخيار payload JSON بالشكل التالي:

    ```json
    {
      "lanes": [
        {
          "lane_id": 12,
          "policy": { "...": "LaneCompliancePolicy JSON blob" },
          "reviewer_signatures": [
            {
              "reviewer": "auditor@example.com",
              "signature_hex": "deadbeef",
              "signed_at": "2026-02-12T09:00:00Z",
              "notes": "Q1 regulator packet"
            }
          ],
          "metrics_snapshot": {
            "nexus_lane_policy_decisions_total": {
              "allow": 42,
              "deny": 1
            }
          },
          "audit_log": [
            {
              "decision": "allow",
              "policy_id": "lane-12-policy",
              "recorded_at": "2026-02-12T09:00:00Z"
            }
          ]
        }
      ]
    }
    ```

    تتحقق CLI من تطابق كل `policy` blob مع `lane_id` المدرج قبل الادراج، مما يمنع ادلة قديمة او غير متطابقة
    من دخول حزم الجهات التنظيمية ولوحات roadmap.
  - يقوم `--markdown-out` (الافتراضي `artifacts/nexus_lane_audit.md`) بعرض ملخص قابل للقراءة
    يوضح المسارات المتاخرة، backlog غير صفري، manifests معلقة، وادلة امتثال مفقودة حتى تتضمن
    حزم annex artefacts machine-readable وسطح مراجعة سريع.

## خطة الاطلاق

1. **P0 — الملاحظة فقط**
   - شحن انواع السياسة والتخزين وواجهات Torii والمقاييس.
   - يقيم Torii السياسات في وضع `audit` (بدون enforcement) لجمع البيانات.
2. **P1 — فرض deny/allow**
   - تفعيل اخفاقات صارمة في Torii والتنفيذ عندما يتم تفعيل قواعد deny.
   - اشتراط السياسات لكل مسارات CBDC؛ يمكن للـ DS العامة البقاء في وضع audit.
3. **P2 — الحدود وتجاوزات الاختصاص**
   - تفعيل فرض حدود التحويل واعلام الاختصاص.
   - تغذية القياس عن بعد الى `dashboards/grafana/nexus_lanes.json`.
4. **P3 — اتاحة الامتثال الكامل**
   - دمج صادرات التدقيق مع مستهلكي `SpaceDirectoryEvent`.
   - ربط تحديثات السياسة بدفاتر تشغيل الحوكمة واتوماسيون الاصدار.

## القبول والاختبار

- تغطي اختبارات التكامل تحت `integration_tests/tests/nexus/compliance.rs`:
  - تركيبات allow/deny وتجاوزات الاختصاص وحدود التحويل؛
  - سباقات تفعيل manifest/policy؛ و
  - تماثل قرار Torii مقابل `iroha_core` عبر تشغيل متعدد العقد.
- تتحقق اختبارات الوحدة في `crates/iroha_core/src/compliance` من محرك التقييم النقي، مؤقتات ابطال الكاش، وتحليل البيانات الوصفية.
- يجب ان تظهر تحديثات Docs/SDK (Torii + CLI) كيفية جلب السياسات، تقديم مقترحات الحوكمة، تفسير رموز الاخطاء، وجمع ادلة التدقيق.

اغلاق NX-12 يتطلب القطع السابقة اضافة الى تحديثات الحالة في `status.md`/`roadmap.md` عند تفعيل enforcement عبر عناقيد staging.

</div>
