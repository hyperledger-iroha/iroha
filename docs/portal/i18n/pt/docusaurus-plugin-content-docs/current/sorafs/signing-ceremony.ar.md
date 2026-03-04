---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/signing-ceremony.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: cerimônia de assinatura
título: استبدال مراسم التوقيع
descrição: كيف يوافق برلمان سورا ويُوزع fixtures para chunker SoraFS (SF-1b).
sidebar_label: Nome da barra lateral
---

> خارطة الطريق: **SF-1b — موافقات fixtures برلمان سورا.**
> مسار البرلمان يحل محل "مراسم توقيع المجلس" القديمة خارج الشبكة.

Você pode usar os fixtures no chunker SoraFS. جميع
Você pode usar **برلمان سورا**, e DAO قائمة على القرعة تحكم Nexus.
يقوم اعضاء البرلمان برهن XOR للحصول على المواطنة, ويتناوبون عبر اللجان, ويصوتون
on-chain للموافقة او الرفض او التراجع عن اصدارات luminárias. يشرح هذا الدليل
العملية وادوات المطورين.

## نظرة عامة على البرلمان

- **المواطنة** — يقوم المشغلون برهن XOR المطلوب للتسجيل كمواطنين واكتساب اهلية القرعة.
- **اللجان** — تتوزع المسؤوليات عبر لجان دوارة (البنية التحتية, الاشراف, الخزانة, ...).
  لجنة البنية التحتية تمتلك موافقات fixtures الخاصة بـ SoraFS.
- **القرعة والتناوب** — يعاد سحب مقاعد اللجان وفق الوتيرة المحددة في دستور البرلمان
  Não se preocupe, você pode fazer isso com cuidado e sem problemas.

## تدفق موافقة jogos

1. **تقديم المقترح**
   - يرفع Tooling WG الحزمة المرشحة `manifest_blake3.json` مع فرق fixture الى
     O código on-chain é `sorafs.fixtureProposal`.
   - يسجل المقترح digest من BLAKE3 والنسخة الدلالية وملاحظات التغيير.
2. **المراجعة والتصويت**
   - تتلقى لجنة البنية التحتية التكليف عبر طابور مهام البرلمان.
   - يفحص اعضاء اللجنة artefatos الخاصة بـ CI ويجرون اختبارات التماثل ويصوتون
     on-chain باوزان.
3. **الانهاء**
   - عند تحقق النصاب, يصدر الـ runtime حدث موافقة يتضمن digest canonico للـ manifesto
     والتزام Merkle لحمولة fixture.
   - ينعكس الحدث في سجل SoraFS حتى يتمكن العملاء من جلب احدث manifest معتمد من البرلمان.
4. **التوزيع**
   - A CLI (`cargo xtask sorafs-fetch-fixture`) gera o manifesto do Nexus RPC.
     O JSON/TS/Go é definido como `export_vectors`
     والتحقق من digest مقابل السجل on-chain.

## سير عمل المطورين

- اعادة توليد jogos:

```bash
cargo run -p sorafs_chunker --bin export_vectors
```

- استخدم اداة جلب البرلمان لتنزيل envelope المعتمد والتحقق من التواقيع وتحديث fixtures
  المحلية. Coloque `--signatures` no envelope do envelope تقوم الاداة بحل manifesto
  O resumo BLAKE3 e o resumo do arquivo `sorafs.sf1@1.0.0`.

```bash
cargo xtask sorafs-fetch-fixture \
  --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
  --out fixtures/sorafs_chunker
```

O nome `--manifest` é um manifesto no Reino Unido. Envelopes para envelopes
Use `--allow-unsigned` para evitar fumaça.

- عند التحقق من manifest عبر بوابة staging, استهدف Torii بدلا من cargas úteis:

```bash
sorafs-fetch \
  --plan=fixtures/chunk_fetch_specs.json \
  --gateway-provider=name=staging,provider-id=<hex>,base-url=https://gw-stage.example/,stream-token=<base64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --json-out=reports/staging_gateway.json
```

- A lista do CI é chamada de `signer.json`.
  `ci/check_sorafs_fixtures.sh` é uma opção de rede on-chain e
  Isso é tudo.

## ملاحظات الحوكمة

- دستور البرلمان يحكم النصاب والتناوب والتصعيد؛ Não coloque nenhum engradado na caixa.
- التراجعات الطارئة تتم عبر لجنة الاشراف في البرلمان. تقدم لجنة البنية التحتية
  Você pode reverter o resumo do manifesto, e também reverter o arquivo.
- Verifique o valor do produto no SoraFS para obter mais informações.## اسئلة شائعة

- **`signer.json`؟**  
  Não. اسناد التوقيعات بالكامل على السلسلة؛ `manifest_signatures.json` é
  O dispositivo elétrico principal não é adequado para qualquer instalação.

- **هل ما زلنا نحتاج تواقيع Ed25519 محلية؟**  
  Não. موافقات البرلمان محفوظة كـ artefatos على السلسلة. fixtures المحلية موجودة
  للاتاحة القابلة لاعادة الانتاج لكنها تتحقق مقابل digest البرلمان.

- **كيف تراقب الفرق الموافقات؟**  
  Use o `ParliamentFixtureApproved` e instale o Nexus RPC
  digerir o manifesto e o resumo.