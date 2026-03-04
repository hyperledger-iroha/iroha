---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: سجل ترحيل SoraFS
descripción: سجل تغييرات قياسي يتتبع كل معلم ترحيل والجهات المالكة والمتابعات المطلوبة.
---

> مقتبس من [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# سجل ترحيل SoraFS

Esta es la configuración de RFC SoraFS. يتم تجميع الإدخالات
حسب المعالم وتعرض نافذة السريان والفرق المتأثرة والإجراءات المطلوبة. يجب أن تقوم
تحديثات خطة البتعديل هذه الصفحة y RFC
(`docs/source/sorafs_architecture_rfc.md`) للحفاظ على اتساق المستهلكين اللاحقين.| المعلم | نافذة السريان | ملخص التغيير | الفرق المتأثرة | عناصر العمل | الحالة |
|--------|---------------|--------------|----------------|-------------|--------|
| M0 | الأسابيع 1–6 | نُشرت accesorios للـ chunker؛ تصدر tuberías حزما من CAR + manifiesto إلى جانب artefactos القديمة؛ تم إنشاء إدخالات السجل. | Documentos, DevRel, SDK | Utilice `sorafs_manifest_stub` para seleccionar banderas que estén conectadas al CDN. | ✅ نشط |
| M1 | الأسابيع 7–12 | يفرض Accesorios CI حتمية؛ أدلة alias متاحة في puesta en escena؛ herramientas يعرض banderas توقع صريحة. | Documentos, almacenamiento, gobernanza | التأكد من بقاء accesorios موقعة، تسجيل alias في سجل staging، تحديث قوائم الإصدار بإنفاذ `--car-digest/--root-cid`. | ⏳ معلّق |
| M2 | الأسابيع 13–20 | يصبح fijar المعتمد على registro هو المسار الأساسي؛ تتحول artefactos القديمة إلى قراءة فقط؛ تفضل البوابات أدلة registro. | Almacenamiento, operaciones y gobernanza | تمرير fijar عبر registro, تجميد hosts القديمة, نشر إشعارات ترحيل للمشغلين. | ⏳ معلّق |
| M3 | الأسبوع 21+ | فرض وصول قائم على alias فقط؛ تنبه المراقبة إلى تكافؤ registro؛ إزالة CDN القديم. | Operaciones, redes, SDK | إزالة DNS القديم، تدوير عناوين URL المخزنة مؤقتا، مراقبة لوحات التكافؤ، تحديث defaults الخاصة بالـ SDK. | ⏳ معلّق || R0–R3 | 2025-03-31 → 2025-07-01 | Anuncio del proveedor de مراحل إنفاذ: R0 مراقبة، R1 تحذير، R2 فرض maneja/capacidades القياسية, R3 تنقية cargas útiles. | Observabilidad, operaciones, SDK, DevRel | استيراد `grafana_sorafs_admission.json`, اتباع قائمة المشغل في `provider_advert_rollout.md`, ، جدولة تجديدات advert قبل بوابة R2 بـ 30+ يوما. | ⏳ معلّق |

محاضر مستوى التحكم في الحوكمة التي تشير إلى هذه المعالم موجودة تحت
`docs/source/sorafs/`. يجب على الفرق إضافة نقاط مؤرخة أسفل كل صف عندما تقع أحداث
ملحوظة (مثل تسجيلات alias جديدة أو مراجعات حوادث registro) لتوفير أثر تدقيقي.

## التحديثات الأخيرة

- 2025-11-01 — تم توزيع `migration_roadmap.md` على مجلس الحوكمة وقوائم المشغلين
  للمراجعة؛ بانتظار المصادقة في جلسة المجلس القادمة (المرجع: متابعة
  `docs/source/sorafs/council_minutes_2025-10-29.md`).
- 2025-11-02 — يفرض ISI تسجيل Registro de PIN الآن التحقق المشترك للـ fragmentador/السياسة عبر
  ayudantes `sorafs_manifest`, مما يبقي المسارات on-chain متسقة مع فحوصات Torii.
- 2026-02-13 — Lanzamiento de أُضيفت مراحل لإعلانات المزوّدين (R0–R3) إلى السجل وتم نشر
  لوحات المراقبة y الإرشادات التشغيلية المرتبطة
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).