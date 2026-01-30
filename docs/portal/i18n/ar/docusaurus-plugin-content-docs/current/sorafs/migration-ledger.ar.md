---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/migration-ledger.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: سجل ترحيل SoraFS
description: سجل تغييرات قياسي يتتبع كل معلم ترحيل والجهات المالكة والمتابعات المطلوبة.
---

> مقتبس من [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# سجل ترحيل SoraFS

يعكس هذا السجل سجل تغييرات الترحيل الموثق في RFC معمارية SoraFS. يتم تجميع الإدخالات
حسب المعالم وتعرض نافذة السريان والفرق المتأثرة والإجراءات المطلوبة. يجب أن تقوم
تحديثات خطة الترحيل بتعديل هذه الصفحة والـ RFC
(`docs/source/sorafs_architecture_rfc.md`) للحفاظ على اتساق المستهلكين اللاحقين.

| المعلم | نافذة السريان | ملخص التغيير | الفرق المتأثرة | عناصر العمل | الحالة |
|--------|---------------|--------------|----------------|-------------|--------|
| M0 | الأسابيع 1–6 | نُشرت fixtures للـ chunker؛ تصدر pipelines حزما من CAR + manifest إلى جانب artefacts القديمة؛ تم إنشاء إدخالات السجل. | Docs, DevRel, SDKs | اعتماد `sorafs_manifest_stub` مع flags التوقع، تسجيل الإدخالات في هذا السجل، الحفاظ على الـ CDN القديم. | ✅ نشط |
| M1 | الأسابيع 7–12 | يفرض CI fixtures حتمية؛ أدلة alias متاحة في staging؛ tooling يعرض flags توقع صريحة. | Docs, Storage, Governance | التأكد من بقاء fixtures موقعة، تسجيل aliases في سجل staging، تحديث قوائم الإصدار بإنفاذ `--car-digest/--root-cid`. | ⏳ معلّق |
| M2 | الأسابيع 13–20 | يصبح pinning المعتمد على registry هو المسار الأساسي؛ تتحول artefacts القديمة إلى قراءة فقط؛ تفضل البوابات أدلة registry. | Storage, Ops, Governance | تمرير pinning عبر registry، تجميد hosts القديمة، نشر إشعارات ترحيل للمشغلين. | ⏳ معلّق |
| M3 | الأسبوع 21+ | فرض وصول قائم على alias فقط؛ تنبه المراقبة إلى تكافؤ registry؛ إزالة CDN القديم. | Ops, Networking, SDKs | إزالة DNS القديم، تدوير عناوين URLs المخزنة مؤقتا، مراقبة لوحات التكافؤ، تحديث defaults الخاصة بالـ SDK. | ⏳ معلّق |
| R0–R3 | 2025-03-31 → 2025-07-01 | مراحل إنفاذ provider advert: R0 مراقبة، R1 تحذير، R2 فرض handles/capabilities القياسية، R3 تنقية payloads القديمة. | Observability, Ops, SDKs, DevRel | استيراد `grafana_sorafs_admission.json`، اتباع قائمة المشغل في `provider_advert_rollout.md`، جدولة تجديدات advert قبل بوابة R2 بـ 30+ يوما. | ⏳ معلّق |

محاضر مستوى التحكم في الحوكمة التي تشير إلى هذه المعالم موجودة تحت
`docs/source/sorafs/`. يجب على الفرق إضافة نقاط مؤرخة أسفل كل صف عندما تقع أحداث
ملحوظة (مثل تسجيلات alias جديدة أو مراجعات حوادث registry) لتوفير أثر تدقيقي.

## التحديثات الأخيرة

- 2025-11-01 — تم توزيع `migration_roadmap.md` على مجلس الحوكمة وقوائم المشغلين
  للمراجعة؛ بانتظار المصادقة في جلسة المجلس القادمة (المرجع: متابعة
  `docs/source/sorafs/council_minutes_2025-10-29.md`).
- 2025-11-02 — يفرض ISI تسجيل Pin Registry الآن التحقق المشترك للـ chunker/السياسة عبر
  helpers `sorafs_manifest`، مما يبقي المسارات on-chain متسقة مع فحوصات Torii.
- 2026-02-13 — أُضيفت مراحل rollout لإعلانات المزوّدين (R0–R3) إلى السجل وتم نشر
  لوحات المراقبة والإرشادات التشغيلية المرتبطة
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).
