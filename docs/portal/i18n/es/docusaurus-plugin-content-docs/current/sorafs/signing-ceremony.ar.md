---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: ceremonia de firma
título: استبدال مراسم التوقيع
descripción: كيف يوافق برلمان سورا ويُوزع accesorios لــ fragmentador SoraFS (SF-1b).
sidebar_label: مراسم التوقيع
---

> خارطة الطريق: **SF-1b — موافقات accesorios برلمان سورا.**
> مسار البرلمان يحل محل "مراسم توقيع المجلس" القديمة خارج الشبكة.

Conecte los accesorios del bloqueador SoraFS. جميع
الموافقات تمر الان عبر **برلمان سورا**، وهي DAO قائمة على القرعة تحكم Nexus.
يقوم اعضاء البرلمان برهن XOR للحصول على المواطنة, ويتناوبون عبر اللجان، ويصوتون
accesorios en cadena. يشرح هذا الدليل
العملية وادوات المطورين.

## نظرة عامة على البرلمان

- **المواطنة** — يقوم المشغلون برهن XOR المطلوب للتسجيل كمواطنين واكتساب اهلية القرعة.
- **اللجان** — تتوزع المسؤوليات عبر لجان دوارة (البنية التحتية، الاشراف، الخزانة، ...).
  لجنة البنية التحتية تمتلك موافقات accesorios الخاصة بـ SoraFS.
- **القرعة والتناوب** — يعاد سحب مقاعد اللجان وفق الوتيرة المحددة في دستور البرلمان
  لضمان عدم احتكار مجموعة واحدة للموافقات.

## accesorios de تدفق موافقة1. **تقديم المقترح**
   - يرفع Tooling WG الحزمة المرشحة `manifest_blake3.json` مع فرق accesorio الى
     السجل en cadena عبر `sorafs.fixtureProposal`.
   - يسجل المقترح digest من BLAKE3 والنسخة الدلالية وملاحظات التغيير.
2. **المراجعة والتصويت**
   - تتلقى لجنة البنية التحتية التكليف عبر طابور مهام البرلمان.
   - يفحص اعضاء اللجنة artefactos الخاصة بـ CI ويجرون اختبارات التماثل ويصوتون
     en cadena باوزان.
3. **الانهاء**
   - عند تحقق النصاب، يصدر الـ runtime حدث موافقة يتضمن digest canonico للـ manifest
     والتزام Merkle لحمولة accesorio.
   - ينعكس الحدث في سجل SoraFS حتى يتمكن العملاء من جلب احدث manifest معتمد من البرلمان.
4. **التوزيع**
   - La CLI (`cargo xtask sorafs-fetch-fixture`) muestra el manifiesto desde Nexus RPC.
     Archivos JSON/TS/Go desde el servidor `export_vectors`
     والتحقق من resumen مقابل السجل en cadena.

## سير عمل المطورين

- اعادة توليد accesorios عبر:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- استخدم اداة جلب البرلمان لتنزيل sobre المعتمد والتحقق من التواقيع وتحديث accesorios
  المحلية. اشِر `--signatures` الى sobre المنشور من البرلمان؛ تقوم الاداة بحل manifiesto
  المرافق واعادة حساب digest BLAKE3 وفرض الملف الشخصي القانوني `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

مرر `--manifest` اذا كان manifiesto في عنوان مختلف. يتم رفض sobres غير الموقعة الا اذا
تم ضبط `--allow-unsigned` لاختبارات humo المحلية.- Utilice el manifiesto para la puesta en escena y utilice Torii para cargas útiles:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- لم يعد الـ CI المحلي يتطلب roster باسم `signer.json`.
  `ci/check_sorafs_fixtures.sh` يقارن حالة المستودع باحدث التزام en cadena y
  عندما تختلف.

## ملاحظات الحوكمة

- دستور البرلمان يحكم النصاب والتناوب والتصعيد؛ لا حاجة لاعدادات على مستوى caja.
- التراجعات الطارئة تتم عبر لجنة الاشراف في البرلمان. تقدم لجنة البنية التحتية
  مقترح revertir يشير الى digest السابق للـ manifest، ويستبدل الاصدار بعد الموافقة.
- Haga clic en el botón SoraFS para que funcione correctamente.

## اسئلة شائعة

- **اين ذهب `signer.json`؟**  
  تمت ازالته. اسناد التوقيعات بالكامل على السلسلة؛ `manifest_signatures.json` aquí
  المستودع مجرد للمطورين يجب ان يطابق حدث الموافقة الاخير.

- **هل ما زلنا نحتاج تواقيع Ed25519 محلية؟**  
  لا. موافقات البرلمان محفوظة كـ artefactos على السلسلة. accesorios المحلية موجودة
  للاتاحة القابلة لاعادة الانتاج لكنها تتحقق مقابل digest البرلمان.

- **كيف تراقب الفرق الموافقات؟**  
  Utilice el software `ParliamentFixtureApproved` y el dispositivo RPC Nexus.
  digerir الحالي للـ manifiesto واسماء اعضاء اللجنة.