---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/signing-ceremony.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: حفل التوقيع
العنوان: Substituicao da cerimonia de assinatura
الوصف: Como o Parlamento Sora aprova e Distribui Installations dochunker SoraFS (SF-1b).
Sidebar_label: حفل Assinatura
---

> خريطة الطريق: **SF-1b - تركيبات تركيبية في Parlamento Sora.**
> O Fluxo do Parlamento substitui a antiga "cerimonia de assinatura do conselho" غير متصل بالإنترنت.

لقد تمت الموافقة على دليل الطقوس الخاص بالتركيب المستخدم لتركيبات القطعة SoraFS.
كل ما تم تقديمه الآن هو تمرير بيلو **Parlamento Sora**، بناءً على DAO الذي تم فرزه
الحاكم o Nexus. تقوم أعضاء البرلمان بحظر XOR للحصول على التطهير والتدوير
بين الألم والتصويت على السلسلة للموافقة على إصدارات التركيبات أو تجديدها أو إرجاعها.
يشرح هذا الدليل العملية والأدوات للمطورين.

## فيساو جيرال دو بارلامنتو

- **Cidadania** - يقوم المشغلون بحظر XOR الضروري للكشف عن المواقع الإلكترونية
  إذا كان الاعصار أنيقًا أو فرزًا.
- **Paineis** - كمسؤوليات مقسمة بين الألم الدوار (Infraestrutura،
  موديراكاو، تيسوراريا، ...). طلاء البنية التحتية وتبرع بالموافقات
  التركيبات تفعل SoraFS.
- **الفرز والتدوير** - كما يتم إعادة تصميم صفائح الطلاء في إيقاع محدد
  دستور البرلمان لكي لا تحتكرها المجموعات على أنها تبريرات.

## تدفق تركيبات التركيب1. **إرسال الاقتراح**
   - مجموعة الأدوات التي توفر حزمة `manifest_blake3.json` أكثر من تركيبات مختلفة
     للتسجيل على السلسلة عبر `sorafs.fixtureProposal`.
   - سجل مقترح أو ملخص BLAKE3، والعكس الدلالي والملاحظات المحدثة.
2. **المراجعة والتصويت**
   - احصل على لوحة البنية التحتية من خلال ملف تعريف البرلمان.
   - أعضاء فحص الطلاء المصنوعة من CI، وقضيب الخصيتين، و
     قم بتسجيل الأصوات على السلسلة.
3. **فايناليزاكاو**
   - عند اكتمال النصاب القانوني، يصدر وقت التشغيل حدث موافقة يتضمن ذلك
     ملخص الكنسي للبيان والتنازل عن Merkle للحمولة والتركيبات.
   - الحدث الموسع في السجل SoraFS حتى يتمكن العملاء من البحث عن
     بيان آخر تم الموافقة عليه من قبل البرلمان.
4. ** التوزيع **
   - مساعدات CLI (`cargo xtask sorafs-fetch-fixture`) يتم إرسالها أو بيانها عبر
     Nexus RPC. نظرًا لأن الثوابت JSON/TS/Go تقوم بعمل مستودع متزامن أيضًا
     أعد تنفيذ `export_vectors` والتحقق من صحة الملخص مقابل التسجيل على السلسلة.

## تدفق العمل للمطور

- تركيبات ريجينير كوم:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```- استخدم مساعد جلب المناقشة لتنزيل المظروف والتحقق منه
  Assinaturas e تحديث التركيبات المحلية. Aponte `--signatures` للمغلف
  publicado pelo Parlamento؛ o مساعد في الحل o واضح مرتبط، recomputa o
  ملخص BLAKE3 وقوة الملف الكنسي `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

أدخل `--manifest` حتى يتم إنشاء البيان على عنوان URL آخر. المغلفات sem assinatura
إنه موصى به، ولكن `--allow-unsigned` محدد للدخان في المكان.

- للتحقق من البيان عبر بوابة التدريج، اتصل بـ Torii في وقت واحد
  موقع الحمولات:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- O CI local nao exige mais um list `signer.json`.
  `ci/check_sorafs_fixtures.sh` مقارنة حالة الريبو مع التسوية الأخيرة
  على السلسلة وعندما تتباعد.

## ملاحظات الحكم

- نصاب دستوري للبرلمان، دوران وتصعيد - ناو ه
  من الضروري تكوين أي مستوى في الصندوق.
- التراجع عن حالات الطوارئ من خلال لوحة تعديل البرلمان. يا
  لوحة البنية التحتية عبارة عن اقتراح إرجاع يشير إلى الملخص
  قبل أن يتم البيان، استبدال الإصدار عندما يتم الموافقة عليه.
- البيانات التاريخية المتوفرة دائمًا غير مسجلة SoraFS لإعادة التشغيل
  الطب الشرعي.

## التعليمات- **للحصول على `signer.json`؟**  
  تمت إزالة هذا. Toda a atribuicao de Assinaturas تعيش على السلسلة؛ `manifest_signatures.json`
  لا يوجد مستودع ولا يوجد سوى أداة مطور يجب أن تكون متوافقة في النهاية
  حدث aprovacao.

- **Ainda exigimos assinaturas Ed25519 locais؟**  
  ناو. كما تفعل Parlamento sao armazenadas como artefatos على السلسلة. تركيبات
  يوجد مكان لإعادة الإنتاج، ولكنه مصادق عليه مقابل ملخص البرلمان.

- **كومو كتجهيزات مراقبة aprovacoes؟**  
  قم بالتسجيل في الحدث `ParliamentFixtureApproved` أو استشارة التسجيل عبر Nexus RPC
  للتعافي أو الخلاصة الفعلية للبيان والاتصال بالطلاء.