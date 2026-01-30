---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: signing-ceremony
title: استبدال مراسم التوقيع
description: كيف يوافق برلمان سورا ويُوزع fixtures لــ chunker SoraFS (SF-1b).
sidebar_label: مراسم التوقيع
---

> خارطة الطريق: **SF-1b — موافقات fixtures برلمان سورا.**
> مسار البرلمان يحل محل "مراسم توقيع المجلس" القديمة خارج الشبكة.

تمت اقالة طقس التوقيع اليدوي المستخدم لــ fixtures الخاصة بـ chunker SoraFS. جميع
الموافقات تمر الان عبر **برلمان سورا**، وهي DAO قائمة على القرعة تحكم Nexus.
يقوم اعضاء البرلمان برهن XOR للحصول على المواطنة، ويتناوبون عبر اللجان، ويصوتون
on-chain للموافقة او الرفض او التراجع عن اصدارات fixtures. يشرح هذا الدليل
العملية وادوات المطورين.

## نظرة عامة على البرلمان

- **المواطنة** — يقوم المشغلون برهن XOR المطلوب للتسجيل كمواطنين واكتساب اهلية القرعة.
- **اللجان** — تتوزع المسؤوليات عبر لجان دوارة (البنية التحتية، الاشراف، الخزانة، ...).
  لجنة البنية التحتية تمتلك موافقات fixtures الخاصة بـ SoraFS.
- **القرعة والتناوب** — يعاد سحب مقاعد اللجان وفق الوتيرة المحددة في دستور البرلمان
  لضمان عدم احتكار مجموعة واحدة للموافقات.

## تدفق موافقة fixtures

1. **تقديم المقترح**
   - يرفع Tooling WG الحزمة المرشحة `manifest_blake3.json` مع فرق fixture الى
     السجل on-chain عبر `sorafs.fixtureProposal`.
   - يسجل المقترح digest من BLAKE3 والنسخة الدلالية وملاحظات التغيير.
2. **المراجعة والتصويت**
   - تتلقى لجنة البنية التحتية التكليف عبر طابور مهام البرلمان.
   - يفحص اعضاء اللجنة artefacts الخاصة بـ CI ويجرون اختبارات التماثل ويصوتون
     on-chain باوزان.
3. **الانهاء**
   - عند تحقق النصاب، يصدر الـ runtime حدث موافقة يتضمن digest canonico للـ manifest
     والتزام Merkle لحمولة fixture.
   - ينعكس الحدث في سجل SoraFS حتى يتمكن العملاء من جلب احدث manifest معتمد من البرلمان.
4. **التوزيع**
   - ادوات CLI (`cargo xtask sorafs-fetch-fixture`) تسحب manifest المعتمد عبر Nexus RPC.
     تبقى ثوابت JSON/TS/Go في المستودع متزامنة عبر اعادة تشغيل `export_vectors`
     والتحقق من digest مقابل السجل on-chain.

## سير عمل المطورين

- اعادة توليد fixtures عبر:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- استخدم اداة جلب البرلمان لتنزيل envelope المعتمد والتحقق من التواقيع وتحديث fixtures
  المحلية. اشِر `--signatures` الى envelope المنشور من البرلمان؛ تقوم الاداة بحل manifest
  المرافق واعادة حساب digest BLAKE3 وفرض الملف الشخصي القانوني `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

مرر `--manifest` اذا كان manifest في عنوان مختلف. يتم رفض envelopes غير الموقعة الا اذا
تم ضبط `--allow-unsigned` لاختبارات smoke المحلية.

- عند التحقق من manifest عبر بوابة staging، استهدف Torii بدلا من payloads المحلية:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- لم يعد الـ CI المحلي يتطلب roster باسم `signer.json`.
  `ci/check_sorafs_fixtures.sh` يقارن حالة المستودع باحدث التزام on-chain ويفشل
  عندما تختلف.

## ملاحظات الحوكمة

- دستور البرلمان يحكم النصاب والتناوب والتصعيد؛ لا حاجة لاعدادات على مستوى crate.
- التراجعات الطارئة تتم عبر لجنة الاشراف في البرلمان. تقدم لجنة البنية التحتية
  مقترح revert يشير الى digest السابق للـ manifest، ويستبدل الاصدار بعد الموافقة.
- تبقى الموافقات التاريخية متاحة في سجل SoraFS لاعادة التشغيل الجنائي.

## اسئلة شائعة

- **اين ذهب `signer.json`؟**  
  تمت ازالته. اسناد التوقيعات بالكامل على السلسلة؛ `manifest_signatures.json` في
  المستودع مجرد fixture للمطورين يجب ان يطابق حدث الموافقة الاخير.

- **هل ما زلنا نحتاج تواقيع Ed25519 محلية؟**  
  لا. موافقات البرلمان محفوظة كـ artefacts على السلسلة. fixtures المحلية موجودة
  للاتاحة القابلة لاعادة الانتاج لكنها تتحقق مقابل digest البرلمان.

- **كيف تراقب الفرق الموافقات؟**  
  اشترك في حدث `ParliamentFixtureApproved` او استعلم السجل عبر Nexus RPC لاسترجاع
  digest الحالي للـ manifest واسماء اعضاء اللجنة.
