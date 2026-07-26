---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/migration-ledger.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: سجل الهجرة SoraFS
الوصف: Journal des Changes canonique qui Suit chaque jalon de emigration, les responsables et les suivis requis.
---

> التكيف مع [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# سجل الهجرة SoraFS

قم بالتسجيل لعرض مجلة الترحيلات الملتقطة في RFC للهندسة المعمارية
SoraFS. Les Entrees sont groupees par jalon et indiquent la fenetre فعالة،
المعدات المتأثرة والإجراءات التي تتطلبها. Les Miss a jour du Plan de
ترحيل DOIVENT modifier cette page et le RFC
(`docs/source/sorafs_architecture_rfc.md`) لحماية المستهلكين
محاذاة.

| جالون | فينتر فعال | استئناف التغيير | تجهيزات المتأثرين | الإجراءات | النظام الأساسي |
|-------|-------------------|---------------------|------------------|---------|--------|
| م1 | سيمين 7-12 | Le CI يفرض تركيبات محددة; إن Preuves d'alias متاحة للتدريج؛ تعرض الأدوات علامات الحضور الصريحة. | المستندات والتخزين والحوكمة | تأكد من أن التركيبات موجودة، وقم بتسجيل الأسماء المستعارة في سجل التدريج، وقم بإعداد قوائم المراجعة يوميًا مع شرط `--car-digest/--root-cid`. | ⏳ في حالة انتظار |محاضر خطة مراقبة الحكم التي تشير إلى أن هذه المياه حية
`docs/source/sorafs/`. تحتاج المعدات إلى إضافة تواريخ كل منها على حدة
عندما تبقى الأحداث البارزة (على سبيل المثال: nouveaux enregistrements d'alias،
بأثر رجعي لحوادث التسجيل) لتوفير أثر قابل للتدقيق.

## افتقد آخر يوم

- 01-11-2025 — نشر `migration_roadmap.md` في مجلس الحكم والقوائم
  المشغلون يصبون المراجعة؛ في انتظار التحقق من الصحة أثناء جلسة prochaine du
  مجلس (المرجع: suivi `docs/source/sorafs/council_minutes_2025-10-29.md`).
- 02-11-2025 — إلغاء التحقق من صحة L'ISI d'enregistrement du Pin Registry
  Partagee Chunker/politique via les helpers `sorafs_manifest`, gardant les chemins
  تتم محاذاة السلسلة مع الشيكات Torii.
- 13-02-2026 — إضافة مراحل بدء تشغيل إعلان الموفر (R0–R3) من خلال التسجيل والتسجيل
  نشر لوحات المعلومات وجمعيات مشغلي التوجيه
  (`provider_advert_rollout.md`، `grafana_sorafs_admission.json`).