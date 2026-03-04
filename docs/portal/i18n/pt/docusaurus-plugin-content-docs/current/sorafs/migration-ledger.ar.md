---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: سجل ترحيل SoraFS
description: سجل تغييرات قياسي يتتبع كل معلم ترحيل والجهات المالكة والمتابعات المطلوبة.
---

> مقتبس من [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# سجل ترحيل SoraFS

Isso é feito através do RFC SoraFS. يتم تجميع الإدخالات
حسب المعالم وتعرض نافذة السريان والفرق المتأثرة والإجراءات المطلوبة. يجب أن تقوم
O que você precisa saber sobre o RFC e o RFC
(`docs/source/sorafs_architecture_rfc.md`).

| المعلم | نافذة السريان | ملخص التغيير | الفرق المتأثرة | عناصر العمل | الحالة |
|--------|---------------|--------------|----------------|-------------|--------|
| M0 | أسابيع 1–6 | نُشرت fixtures للـ chunker؛ Construir pipelines no CAR + manifesto ou nos artefatos Isso é tudo. | Documentos, DevRel, SDKs | Use `sorafs_manifest_stub` para sinalizar o código de barras no CDN. | ✅ نشط |
| M1 | Capítulo 7–12 | يفرض CI jogos أدلة alias متاحة في staging؛ ferramentas يعرض sinalizadores توقع صريحة. | Documentos, armazenamento, governança | No caso de fixtures, você pode usar aliases no staging, usando o `--car-digest/--root-cid`. | ⏳ معلّق |
| M2 | Dias 13–20 | Fixando o registro no registro e fixando-o تتحول artefatos القديمة إلى قراءة فقط؛ تفضل البوابات أدلة registro. | Armazenamento, operações, governança | Fixando o registro, fixando os hosts do sistema, você pode instalar o sistema. | ⏳ معلّق |
| M3 | أسبوع 21+ | فرض وصول قائم على alias فقط؛ تنبه المراقبة إلى تكافؤ registro; إزالة CDN القديم. | Operações, redes, SDKs | O DNS DNS, os URLs não são os padrões do SDK e os padrões do SDK. | ⏳ معلّق |
| R0–R3 | 31/03/2025 → 01/07/2025 | O anúncio do provedor é o seguinte: R0 مراقبة, R1 تحذير, R2 فرض handles/capabilities القياسية, R3 تنقية payloads القديمة. | Observabilidade, operações, SDKs, DevRel | استيراد `grafana_sorafs_admission.json`, اتباع قائمة المشغل في `provider_advert_rollout.md`, جدولة تجديدات anúncio, بوابة R2 30+ anos. | ⏳ معلّق |

محاضر مستوى التحكم في الحوكمة التي تشير إلى هذه المعالم موجودة تحت
`docs/source/sorafs/`. يجب على الفرق إضافة نقاط مؤرخة أسفل كل صف عندما تقع أحداث
ملحوظة (مثل تسجيلات alias جديدة أو مراجعات حوادث registro) لتوفير أثر تدقيقي.

## التحديثات الأخيرة

- 2025-11-01 — تم توزيع `migration_roadmap.md` على مجلس الحوكمة وقوائم المشغلين
  للمراجعة؛ بانتظار المصادقة في جلسة المجلس القادمة (المرجع: متابعة
  `docs/source/sorafs/council_minutes_2025-10-29.md`).
- 2025-11-02 — ISI تسجيل Pin Registry الآن التحقق المشترك للـ chunker/السياسة عبر
  ajudantes `sorafs_manifest`, مما يبقي المسارات on-chain متسقة مع فحوصات Torii.
- 13/02/2026 — Implementação do lançamento do programa لإعلانات المزوّدين (R0–R3) إلى السجل وتم نشر
  لوحات المراقبة والإرشادات التشغيلية المرتبطة
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).