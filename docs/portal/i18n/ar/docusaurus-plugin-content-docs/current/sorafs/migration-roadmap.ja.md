---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sorafs/migration-roadmap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ac85b407a987a8ea88af5a2809cb2e41001c217ff7b04bc238fb876fe583faa8
source_last_modified: "2025-11-14T04:43:21.811018+00:00"
translation_last_reviewed: 2026-01-30
---

> مقتبس من [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md).

# خارطة طريق ترحيل SoraFS (SF-1)

هذا المستند يُحوِّل إرشادات الترحيل الموثقة في
`docs/source/sorafs_architecture_rfc.md` إلى خطة تشغيلية. يوسِّع مخرجات SF-1 إلى
معالم جاهزة للتنفيذ ومعايير بوابة وقوائم تحقق للمالكين حتى تتمكن فرق التخزين
والحوكمة وDevRel وSDK من تنسيق الانتقال من استضافة artefacts القديمة إلى نشر
مدعوم بـ SoraFS.

خارطة الطريق حتمية عمدا: كل معلم يسمي artefacts المطلوبة واستدعاءات الأوامر وخطوات
الاستيثاق حتى تنتج خطوط الأنابيب اللاحقة مخرجات متطابقة وتحافظ الحوكمة على أثر
قابل للتدقيق.

## نظرة عامة على المعالم

| المعلم | النافذة | الأهداف الأساسية | ما يجب تسليمه | المالكون |
|--------|---------|------------------|----------------|----------|
| **M1 - Deterministic Enforcement** | الأسابيع 7-12 | فرض fixtures موقعة وتجهيز إثباتات alias بينما تعتمد خطوط الأنابيب expectation flags. | تحقق ليلي من fixtures، manifests موقعة من المجلس، إدخالات staging في سجل alias. | Storage, Governance, SDKs |

يتم تتبع حالة المعالم في `docs/source/sorafs/migration_ledger.md`. كل تغيير في هذه
الخارطة يجب أن يحدِّث السجل ليبقى الحوكمة وهندسة الإصدارات على نفس النسق.

## مسارات العمل

### 2. اعتماد pinning الحتمي

| الخطوة | المعلم | الوصف | المالك(ون) | المخرجات |
|--------|--------|-------|------------|----------|
| تدريبات fixtures | M0 | Dry-runs أسبوعية تقارن digests المحلية للـ chunk مع `fixtures/sorafs_chunker`. نشر التقرير تحت `docs/source/sorafs/reports/`. | Storage Providers | `determinism-<date>.md` مع مصفوفة pass/fail. |
| فرض التواقيع | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` تفشل إذا انحرفت التواقيع أو manifests. overrides التطوير تتطلب waiver من الحوكمة مرفق بالـ PR. | Tooling WG | سجل CI، رابط تذكرة waiver (إن وجدت). |
| Expectation flags | M1 | خطوط الأنابيب تستدعي `sorafs_manifest_stub` بتوقعات صريحة لتثبيت المخرجات: | Docs CI | سكربتات محدثة تشير إلى expectation flags (انظر كتلة الأمر أدناه). |
| Registry-first pinning | M2 | `sorafs pin propose` و`sorafs pin approve` يغلِّفان تقديمات manifest؛ CLI الافتراضي يستخدم `--require-registry`. | Governance Ops | سجل تدقيق CLI للـ registry، تليمترية فشل المقترحات. |
| تكافؤ observability | M3 | لوحات Prometheus/Grafana تنبه عند اختلاف مخزون chunks عن manifests في registry؛ التنبيهات موصولة بمناوبة ops. | Observability | رابط لوحة، IDs قواعد التنبيه، نتائج GameDay. |

#### أمر النشر القياسي

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

استبدل قيم digest والحجم وCID بالمراجع المتوقعة المسجلة في إدخال سجل الترحيل
للـ artefact.

### 3. انتقال alias والاتصالات

| الخطوة | المعلم | الوصف | المالك(ون) | المخرجات |
|--------|--------|-------|------------|----------|
| إثباتات alias في staging | M1 | تسجيل مطالبات alias في Pin Registry الخاص بـ staging وإرفاق إثباتات Merkle مع manifests (`--alias`). | Governance, Docs | bundle إثباتات مخزن بجوار manifest + تعليق في السجل باسم alias. |
| فرض الإثباتات | M2 | Gateways ترفض manifests بدون رؤوس `Sora-Proof` حديثة؛ CI يضيف خطوة `sorafs alias verify` لجلب الإثباتات. | Networking | تصحيح إعدادات gateway + مخرجات CI توثق التحقق الناجح. |

### 4. الاتصالات والتدقيق

- **انضباط السجل:** كل تغيير حالة (drift للـ fixtures، تقديم registry، تفعيل alias) يجب أن يضيف
  ملاحظة مؤرخة في `docs/source/sorafs/migration_ledger.md`.
- **محاضر الحوكمة:** جلسات المجلس التي تعتمد تغييرات Pin Registry أو سياسات alias يجب أن تشير
  إلى هذه الخارطة والسجل معا.
- **الاتصالات الخارجية:** DevRel ينشر تحديثات الحالة عند كل معلم (مدونة + مقتطف changelog)
  مع إبراز الضمانات الحتمية وجداول alias الزمنية.

## التبعيات والمخاطر

| التبعية | الأثر | التخفيف |
|---------|-------|---------|
| توفر عقد Pin Registry | يمنع rollout M2 pin-first. | تجهيز العقد قبل M2 مع اختبارات replay؛ الحفاظ على fallback للـ envelope حتى زوال أي regressions. |
| مفاتيح توقيع المجلس | مطلوبة لـ manifest envelopes وموافقات registry. | مراسم التوقيع موثقة في `docs/source/sorafs/signing_ceremony.md`؛ تدوير المفاتيح بتداخل وتدوين ذلك في السجل. |
| إيقاع إصدارات SDK | يجب على العملاء احترام إثباتات alias قبل M3. | مواءمة نوافذ إصدار SDK مع بوابات المعالم؛ إضافة checklists للترحيل في قوالب الإصدار. |

المخاطر المتبقية ووسائل التخفيف مذكورة أيضا في `docs/source/sorafs_architecture_rfc.md`
ويجب الرجوع إليها عند إجراء تعديلات.

## قائمة تحقق معايير الخروج

| المعلم | المعايير |
|--------|----------|
| M1 | - مهمة fixtures الليلية خضراء لسبعة أيام متتالية. <br /> - تحقق إثباتات alias في staging داخل CI. <br /> - الحوكمة تصادق على سياسة expectation flags. |

## إدارة التغيير

1. اقترح التعديلات عبر PR يقوم بتحديث هذا الملف **و**
   `docs/source/sorafs/migration_ledger.md`.
2. اربط محاضر الحوكمة وأدلة CI في وصف الـ PR.
3. بعد الدمج، أخطر قائمة بريد storage + DevRel بملخص وإجراءات متوقعة للمشغلين.

اتباع هذا الإجراء يضمن أن rollout SoraFS يبقى حتميا وقابلا للتدقيق وشفافا عبر
الفرق المشاركة في إطلاق Nexus.
