---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/signing-ceremony.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: حفل التوقيع
العنوان: استبدال حفل التوقيع
الوصف: تعليق على Parlement Sora يوافق على تركيبات القطعة SoraFS (SF-1b) ويوزعها.
Sidebar_label: حفل التوقيع
---

> خريطة الطريق: **SF-1b — موافقة على تركيبات Parlement Sora.**
> يحل سير عمل البرلمان محل القديم «حفل التوقيع على المجلس» خارج الخط.

تم إيقاف طقوس التركيب اليدوية الخاصة بـ Chunker SoraFS. توت ليه
تم إلغاء الموافقات من قبل **Parlement Sora**، قاعدة DAO على النقل
au النوع الذي يحكم Nexus. يحظر أعضاء البرلمان XOR للحصول على ذلك
مواطن، قم بالمشاركة بين اللوحات والتصويت على السلسلة للموافقة أو الرفض أو
Revenir sur des Releases de Installations. يشرح هذا الدليل العملية والأدوات
من أجل المطورين.

## Vue d'ensemble du Parlement- **Citoyennete** — مشغلو XOR الذين يطلبون منك الاشتراك
  المواطنون والمواطنون مؤهلون للتنقل بالفرز.
- **اللوحات** — المسؤوليات هي جزء من اللوحات الدوارة
  (البنية التحتية، الاعتدال، الخزانة، ...). حاجز البنية التحتية لـ Le Panel
  اعتمادات التركيبات SoraFS.
- **النقل بالفرز والتدوير** — يتم إعادة ترتيب حصار اللوحة بمفردها
  الإيقاع المحدد في دستور البرلمان هو مجموعة qu'aucun ne
  احتكار الموافقات.

## تدفق الموافقة على التركيبات1. ** إذن الاقتراح **
   - يعرض Tooling WG الحزمة المرشحة `manifest_blake3.json` والفرق
     يتم التثبيت في التسجيل على السلسلة عبر `sorafs.fixtureProposal`.
   - يقوم الاقتراح بتسجيل ملخص BLAKE3 والإصدار الدلالي والملاحظات
     التغيير.
2. ** المراجعة والتصويت **
   - تستعرض لوحة البنية التحتية التأثير عبر ملف لوحة المفاتيح في البرلمان.
   - يقوم الأعضاء بفحص القطع الأثرية CI وتنفيذ اختبارات التكافؤ وما إلى ذلك
     تصدر الأصوات التي تفكر فيها على السلسلة.
3. **الإنهاء**
   - بمجرد اكتمال النصاب القانوني، سيتضمن وقت التشغيل ليلة موافقة متضمنة
     ملخص البيانات الأساسية والتفاعل مع حمولة التثبيت.
   - الحدث مكرر في السجل SoraFS حتى يتمكن العملاء من النفاذ
     يستعيد البيان الأخير الموافق عليه من قبل البرلمان.
4. ** التوزيع **
   - يستعيد مساعدو CLI (`cargo xtask sorafs-fetch-fixture`) البيان
     الموافقة عبر Nexus RPC. الثوابت JSON/TS/Go في المستودع متزامنة
     مع `export_vectors` وصحيح الملخص حسب العلاقة مع التسجيل
     على السلسلة.

## تطوير سير العمل

- تجديد التركيبات مع :

```bash
cargo run -p sorafs_chunker --bin export_vectors
```- استخدم مساعد جلب Parlement لتنزيل المغلف المعتمد،
  التحقق من التوقيعات وإزالة التركيبات المحلية. المؤشر `--signatures`
  Vers l'enveloppe publiee par le Parlement؛ المصدر المساعد لو البيان المرتبط،
  أعد حساب ملخص BLAKE3 وفرض الملف التعريفي الكنسي `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Passer `--manifest` إذا وجد البيان عنوان URL آخر. المغلفات غير
الموقعون يرفضون sauf si `--allow-unsigned` est active pour des smoke run locaux.

- من أجل التحقق من بيان عبر بوابة التدريج، cibler Torii plutot que des
  موقع الحمولات:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- Le CI local n'exige plus un list `signer.json`.
  `ci/check_sorafs_fixtures.sh` مقارنة حالة الريبو مع المشاركة الأخيرة
  على السلسلة وصدى عندما تكون متباعدة.

## ملاحظات الحوكمة

- دستور البرلمان يحكم النصاب القانوني والتناوب والتصعيد؛
  لا يلزم تكوين مستوى الصندوق.
- يتم تغيير عمليات التراجع عن الطلب عبر لوحة التحكم في Parlement. لو
  تطرح البنية التحتية للوحة عرضًا للعودة يشير إلى الملخص
  سابقة البيان، والإصدار يستبدل بفترة الموافقة عليها.
- الموافقات التاريخية متاحة في السجل SoraFS من أجل
  الأمم المتحدة إعادة الطب الشرعي.

## التعليمات- ** هل تجاوزت `signer.json` ؟**  
  إنه رائع. كل إسناد التوقيع على السلسلة ؛ `manifest_signatures.json`
  في المستودع، لا يوجد شيء أساسي يمكن تطويره وهو ما يجب أن يتوافق مع الأحدث
  حدث الموافقة.

- **فشل في ظهور التوقيعات Ed25519 locales ؟**  
  غير. استحسانات Parlement هي عبارة عن قطع أثرية متصلة بالسلسلة. التركيبات
  اللغات الموجودة لإعادة الإنتاج هي صالحة ضد هضم البرلمان.

- **Comment les Equipments surveillent-elles les approbations ?**  
  قم بإبلاغ الحدث `ParliamentFixtureApproved` أو التحقق من التسجيل عبر
  Nexus RPC للحصول على ملخص البيان الحالي وقائمة أعضاء اللوحة.