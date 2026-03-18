---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/chunker-conformance.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: توافق القطعة
العنوان: Guia de Conformidade do Chunker da SoraFS
Sidebar_label: مطابقة المقطع
الوصف: المتطلبات والتدفقات للحفاظ على تحديد ملف قطع SF1 في التركيبات ومجموعات SDK.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة espelha `docs/source/sorafs/chunker_conformance.md`. Mantenha ambas as copias sincronzadas.
:::

هذا الدليل مقنن للمتطلبات التي يجب تنفيذها جميعًا للاستمرار
متوافق مع ملف تعريف القطع SoraFS (SF1). إنه كذلك
وثيقة أو تدفق التجديد وسياسة الاغتيالات وخطوات التحقق من أجل ذلك
مستهلكو التركيبات في حزم SDK متزامنة.

## بيرفيل كانونيكو

- مقبض الملف الشخصي: `sorafs.sf1@1.0.0`
- بذرة المدخل (ست عشرية): `0000000000dec0ded`
- تامانهو ألفو: 262144 بايت (256 كيلو بايت)
- تامانهو مينيمو: 65536 بايت (64 كيلو بايت)
- تامانهو ماكسيمو: 524288 بايت (512 كيلو بايت)
- بولينوميو دي المتداول: `0x3DA3358B4DC173`
- معدات البذور دا تابيلا: `sorafs-v1-gear`
- قناع الاستراحة: `0x0000FFFF`

مرجع التنفيذ: `sorafs_chunker::chunk_bytes_with_digests_profile`.
جميع أجهزة SIMD سريعة الإنتاج لديها حدود ومكونات متطابقة.

## حزمة التركيبات

`cargo run --locked -p sorafs_chunker --bin export_vectors` التجديد ك
التركيبات وإصدار الملفات التالية في `fixtures/sorafs_chunker/`:- `sf1_profile_v1.{json,rs,ts,go}` - حدود القطع الأساسية للمستهلكين
  الصدأ، TypeScript، والذهاب. يتم الإعلان عن كل ملف أو التعامل مع Canonico كأول مرة
  أدخل إلى `profile_aliases`، تابع للأسماء المستعارة البديلة (على سبيل المثال،
  `sorafs.sf1@1.0.0`، depois `sorafs.sf1@1.0.0`). أمر ه imposta por
  سيتم تغيير `ensure_charter_compliance` و NAO DEVE.
- `manifest_blake3.json` - البيان الذي تم التحقق منه بواسطة BLAKE3 cobrindo كل ملف من التركيبات.
- `manifest_signatures.json` - assinaturas do conselho (Ed25519) حول o ملخص do البيان.
- `sf1_profile_v1_backpressure.json` والهيئات العامة داخل `fuzz/` -
  سيناريوهات التدفق الحتمية المستخدمة من قبل خصيتي الضغط الخلفي للمقطع.

### سياسة القتل

تجديد التركيبات **تطوير** يتضمن نصيحة صالحة. يا جيرادور
لقد تم الترحيب بها دون أن يتم اغتيالها على الأقل `--allow-unsigned` لقد تم تمريرها بشكل صريح (موجهة
apenas para experienceacao محلي). مغلفات assinatura sao إلحاق فقط e
sao deduplicados por Signatario.

لإضافة نصيحة إلى شخص ما:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## التحقق

يتم إعادة تشغيل مساعد CI `ci/check_sorafs_fixtures.sh` أو تشغيله عبر com
`--locked`. إذا كانت التركيبات متباينة أو متفاوتة، أو كانت المهمة خاطئة. استخدم
هذا البرنامج النصي يتضمن سير العمل اليومي وقبل إرسال تعديلات التركيبات.

دليل Passos de verificacao:

1. قم بتنفيذ `cargo test -p sorafs_chunker`.
2. قم بتنفيذ `ci/check_sorafs_fixtures.sh` محليًا.
3. تأكد من أن `git status -- fixtures/sorafs_chunker` قد تم الانتهاء منه.## ترقية قواعد اللعبة التي تمارسها

للحصول على ملف جديد للتقطيع أو تحديث SF1:

قم بالتسجيل: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md) للفقرة
متطلبات التعريف وقوالب الاقتراحات وقوائم التحقق من الصحة.

1. Redija um `ChunkProfileUpgradeProposalV1` (veja RFC SF-1) مع معلمات جديدة.
2. قم بإعادة إنشاء التركيبات عبر `export_vectors` وقم بالتسجيل أو عمل البيان الجديد.
3. يجب تقديم بيان بشأن النصاب القانوني للمشورة المطلوبة. كل ما يجب فعله هو القتل
   anexadas `manifest_signatures.json`.
4. قم بتحديث إعدادات SDK (Rust/Go/TS) وضمان تكافؤها خلال وقت التشغيل.
5. قم بتجديد زغب الجسم مع وضع المعلمات.
6. قم بتنشيط هذه الأداة باستخدام المقبض الجديد للملفات والبذور والهضم.
7. تمتع بالتغيير جنبًا إلى جنب مع الخصيتين المحدثتين وتحديث خارطة الطريق.

كل ما عليك فعله هو إضافة حدود للقطعة أو الهضم دون اتباع هذه العملية
sao validas e nao devem ser mergeadas.