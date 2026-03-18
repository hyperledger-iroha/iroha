---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : cérémonie de signature
titre : استبدال مراسم التوقيع
description: Il s'agit d'un appareil pour le chunker SoraFS (SF-1b).
sidebar_label : مراسم التوقيع
---

> خارطة الطريق : **SF-1b — موافقات luminaires برلمان سورا.**
> مسار البرلمان يحل محل "مراسم توقيع المجلس" القديمة خارج الشبكة.

Vous avez besoin de luminaires pour le chunker SoraFS. جميع
Il s'agit d'un **برلمان سورا**، et DAO قائمة على القرعة تحكم Nexus.
يقوم اعضاء البرلمان برهن XOR للحصول على المواطنة، ويتناوبون عبر اللجان، ويصوتون
les rencontres en chaîne et les rencontres en chaîne. يشرح هذا الدليل
العملية وادوات المطورين.

## نظرة عامة على البرلمان

- **المواطنة** — يقوم المشغلون برهن XOR المطلوب للتسجيل كمواطنين واكتساب اهلية القرعة.
- **اللجان** — تتوزع المسؤوليات عبر لجان دوارة (البنية التحتية، الاشراف، الخزانة، ...).
  L'installation des luminaires est basée sur SoraFS.
- **القرعة والتناوب** — يعاد سحب مقاعد اللجان وفق الوتيرة المحددة في دستور البرلمان
  لضمان عدم احتكار مجموعة واحدة للموافقات.

## تدفق موافقة calendrier

1. **تقديم المقترح**
   - يرفع Tooling WG الحزمة المرشحة `manifest_blake3.json` pour le luminaire
     Lien vers la chaîne `sorafs.fixtureProposal`.
   - يسجل المقترح digest by BLAKE3 والنسخة الدلالية وملاحظات التغيير.
2. **المراجعة والتصويت**
   - تتلقى لجنة البنية التحتية التكليف عبر طابور مهام البرلمان.
   - يفحص اعضاء اللجنة artefacts الخاصة بـ CI ويجرون اختبارات التماثل ويصوتون
     en chaîne باوزان.
3. **الانهاء**
   - عند تحقق النصاب، يصدر الـ runtime حدث موافقة يتضمن digest canonico للـ manifest
     والتزام Merkle لحمولة luminaire.
   - ينعكس الحدث في سجل SoraFS حتى يتمكن العملاء من جلب احدث manifest معتمد من البرلمان.
4. **التوزيع**
   - La CLI (`cargo xtask sorafs-fetch-fixture`) utilise le manifeste pour le Nexus RPC.
     Utiliser JSON/TS/Go pour créer un lien vers le modèle `export_vectors`
     والتحقق من digest مقابل السجل en chaîne.

## سير عمل المطورين

- اعادة توليد calendriers عبر:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- استخدم اداة جلب البرلمان لتنزيل enveloppe المعتمد والتحقق من التواقيع وتحديث luminaires
  المحلية. اشِر `--signatures` enveloppe d'enveloppe pour l'utilisateur تقوم الاداة بحل manifeste
  Il s'agit du digest BLAKE3 et du dossier `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

Il s'agit du manifeste `--manifest`. يتم رفض enveloppes غير الموقعة الا اذا
تم ضبط `--allow-unsigned` لاختبارات smoke المحلية.

- J'utilise le manifeste pour la mise en scène et j'utilise Torii pour les charges utiles :

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- لم يعد الـ CI المحلي يتطلب roster باسم `signer.json`.
  `ci/check_sorafs_fixtures.sh` يقارن حالة المستودع باحدث التزام on-chain et
  عندما تختلف.

## ملاحظات الحوكمة

- دستور البرلمان يحكم النصاب والتناوب والتصعيد؛ لا حاجة لاعدادات على مستوى caisse.
- Les informations relatives à l'information sont prises en compte par l'utilisateur. تقدم لجنة البنية التحتية
  مقترح revert يشير الى digest السابق للـ manifest, ويستبدل الاصدار بعد الموافقة.
- تبقى الموافقات التاريخية متاحة في سجل SoraFS لاعادة التشغيل الجنائي.## اسئلة شائعة

- **اين ذهب `signer.json`؟**  
  تمت ازالته. اسناد التوقيعات بالكامل على السلسلة؛ `manifest_signatures.json` dans
  Le luminaire مستودع مجرد للمطورين يجب ان يطابق حدث الموافقة الاخير.

- **هل ما زلنا نحتاج تواقيع Ed25519 محلية؟**  
  Oui. موافقات البرلمان محفوظة كـ artefacts على السلسلة. calendrier المحلية موجودة
  للاتاحة القابلة لاعادة الانتاج لكنها تتحقق مقابل digest البرلمان.

- **كيف تراقب الفرق الموافقات؟**  
  Mettre en place le `ParliamentFixtureApproved` et le module RPC Nexus
  digest الحالي للـ manifest واسماء اعضاء اللجنة.