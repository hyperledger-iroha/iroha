---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : سجل ترحيل SoraFS
description: سجل تغييرات قياسي يتتبع كل معلم ترحيل والجهات المالكة والمتابعات المطلوبة.
---

> مقتبس من [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# سجل ترحيل SoraFS

Il s'agit d'une demande de modification de la RFC SoraFS. يتم تجميع الإدخالات
حسب المعالم وتعرض نافذة السريان والفرق المتأثرة والإجراءات المطلوبة. يجب أن تقوم
Ajouter des informations sur RFC
(`docs/source/sorafs_architecture_rfc.md`) للحفاظ على اتساق المستهلكين اللاحقين.| المعلم | نافذة السريان | ملخص التغيير | الفرق المتأثرة | عناصر العمل | الحالة |
|--------|---------------|--------------|----------------|-------------|--------|
| M0 | أسابيع 1–6 | نُشرت luminaires للـ chunker؛ تصدر pipelines حزما من CAR + manifeste إلى جانب artefacts القديمة؛ تم إنشاء إدخالات السجل. | Docs, DevRel, SDK | Utilisez `sorafs_manifest_stub` pour les drapeaux associés aux paramètres du CDN. | ✅ نشط |
| M1 | الأسابيع 7–12 | يفرض CI rencontres أدلة alias متاحة في staging؛ outillage يعرض drapeaux توقع صريحة. | Documents, stockage, gouvernance | Vous pouvez également utiliser les alias pour la mise en scène, ainsi que les alias pour la mise en scène, ainsi que les alias pour la mise en scène `--car-digest/--root-cid`. | ⏳ معلّق |
| M2 | الأسابيع 13-20 | Vous pouvez épingler le registre dans le registre تتحول artefacts القديمة إلى قراءة فقط؛ تفضل البوابات أدلة registre. | Stockage, opérations, gouvernance | Vous avez épinglé le registre et vous avez hébergé des hôtes pour créer des liens. | ⏳ معلّق |
| M3 | الأسبوع 21+ | فرض وصول قائم على alias فقط؛ تنبه المراقبة إلى تكافؤ registre؛ إزالة CDN القديم. | Opérations, mise en réseau, SDK | Les URL DNS sont configurées pour les URL par défaut avec le SDK. | ⏳ معلّق || R0-R3 | 2025-03-31 → 2025-07-01 | Annonce du fournisseur ci-dessous : R0 disponible R1 ou R2 avec poignées/capacités pour les charges utiles R3 pour les charges utiles. | Observabilité, Ops, SDK, DevRel | Il s'agit d'une publicité pour `grafana_sorafs_admission.json` pour `provider_advert_rollout.md` pour une publicité pour R2 à partir de 30 ans. | ⏳ معلّق |

محاضر مستوى التحكم في الحوكمة التي تشير إلى هذه المعالم موجودة تحت
`docs/source/sorafs/`. يجب على الفرق إضافة نقاط مؤرخة أسفل كل صف عندما تقع أحداث
ملحوظة (مثل تسجيلات alias جديدة أو مراجعات حوادث) لتوفير أثر تدقيقي.

## التحديثات الأخيرة

- 2025-11-01 — تم توزيع `migration_roadmap.md` على مجلس الحوكمة وقوائم المشغلين
  للمراجعة؛ بانتظار المصادقة في جلسة المجلس القادمة (المرجع: متابعة
  `docs/source/sorafs/council_minutes_2025-10-29.md`).
- 2025-11-02 — ISI تسجيل Pin Registry الآن التحقق المشترك للـ chunker/السياسة عبر
  helpers `sorafs_manifest`, sont des outils en chaîne pour les Torii.
- 2026-02-13 — Déploiement du système de gestion des coûts (R0–R3) et contrôle des coûts
  لوحات المراقبة والإرشادات التشغيلية المرتبطة
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).