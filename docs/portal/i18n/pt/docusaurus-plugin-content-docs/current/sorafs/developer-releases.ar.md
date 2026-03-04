---
lang: pt
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: عملية الإصدار
resumo: نفّذ بوابة إصدار CLI / SDK, وطبّق سياسة الإصدارات المشتركة, وانشر ملاحظات الإصدار المعتمدة.
---

# عملية الإصدار

SoraFS (`sorafs_cli`, `sorafs_fetch`, helpers) e SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) Não. يحافظ خط إصدار واحد على
Você pode usar CLI e usar lint/test, além de criar arquivos de teste
اللاحقين. Não há nada que você possa fazer e que não seja adequado.

## 0. تأكيد اعتماد مراجعة الأمان

قبل تنفيذ بوابة الإصدار التقنية, التقط أحدث آرتيفاكتات مراجعة الأمان:

- نزّل أحدث مذكرة مراجعة أمان SF-6 ([reports/sf6-security-review](./reports/sf6-security-review.md))
  وسجّل تجزئة SHA256 الخاصة بها في تذكرة الإصدار.
- أرفق رابط تذكرة المعالجة (مثل `governance/tickets/SF6-SR-2026.md`) e
  Grupo de Trabalho de Ferramentas de Engenharia de Segurança.
- تحقّق من إغلاق قائمة المعالجة في المذكرة؛ Você pode usar o software para obter mais informações.
- استعد لرفع سجلات chicote de fios (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  بجانب حزمة المانيفست.
- تأكد أن أمر التوقيع الذي ستنفذه يتضمن `--identity-token-provider` e
  `--identity-token-audience=<aud>` صريحًا حتى يُلتقط نطاق Fulcio ضمن أدلة الإصدار.

Você pode fazer isso com antecedência.

## 1. تنفيذ بوابة الإصدار/الاختبارات

Você pode usar `ci/check_sorafs_cli_release.sh` como Clippy e Clippy.
O CLI e o SDK podem ser usados como destino (`.target`) para serem atualizados.
Você não pode usar o CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

ينفذ السكربت التحققات التالية:

- `cargo fmt --all -- --check` (área de trabalho)
- `cargo clippy --locked --all-targets` para `sorafs_car` (como `cli`),
  e`sorafs_manifest` e`sorafs_chunker`
- `cargo test --locked --all-targets` para obter mais informações

Isso é o que acontece com o telefone e o computador. يجب أن تكون بناءات
الإصدار متصلة بـ main؛ Não há necessidade de escolher a cereja no lugar certo.
تتحقق البوابة أيضًا من توفير أعلام التوقيع دون مفاتيح (`--identity-token-issuer`,
`--identity-token-audience`) حيث يلزم؛ تؤدي الحجج المفقودة إلى فشل التشغيل.

## 2. تطبيق سياسة الإصدارات

تستخدم جميع حزم CLI/SDK الخاصة SoraFS نظام SemVer:

- `MAJOR`: Versão 1.0. Versão 1.0 da versão `0.y`
  **Você pode usar o arquivo ** na CLI ou no dispositivo Norito.
- `MINOR`: ميزات متوافقة للخلف (أوامر/أعلام جديدة, حقول Norito جديدة خلف سياسة
  اختيارية, إضافات تليمترية).
- `PATCH`: إصلاحات عيوب, وإصدارات وثائق فقط, وتحديثات تبعيات لا تغيّر السلوك
  الملاحظ.

Você pode usar `sorafs_car` e `sorafs_manifest` e `sorafs_chunker` para obter mais informações
Você pode usar o SDK do site para obter mais informações. عند رفع الإصدارات:

1. Insira o `version =` no `Cargo.toml`.
2. Use `Cargo.lock` ou `cargo update -p <crate>@<new-version>` (تفرض
   مساحة العمل نسخًا صريحة).
3. Verifique se o produto está funcionando corretamente.

## 3. إعداد ملاحظات الإصدار

Você pode usar o Markdown para usar o CLI e o SDK e usar o Markdown
الحوكمة. Use o código `docs/examples/sorafs_release_notes.md` (não disponível)
دليل آرتيفاكتات الإصدار eاملأ الأقسام بتفاصيل ملموسة).

الحد الأدنى من المحتوى:- **أبرز النقاط**: Você pode usar CLI e SDK.
- **التوافق**: تغييرات كاسرة, ترقيات السياسات, المتطلبات الدنيا للبوابة/العقدة.
- **خطوات الترقية**: أوامر TL;DR لتحديث تبعيات cargo وإعادة تشغيل fixtures الحتمية.
- **التحقق**: تجزئات مخرجات الأوامر, أو الأظرف والإصدار الدقيق لـ
  `ci/check_sorafs_cli_release.sh` é um problema.

أرفق ملاحظات الإصدار المكتملة بالوسم (مثل نص إصدار GitHub) وخزّنها بجانب
Você pode fazer isso.

## 4. تنفيذ خطافات الإصدار

Use `scripts/release_sorafs_cli.sh` para obter informações sobre o produto e o produto
يُشحن مع كل إصدار. يقوم الغلاف ببناء CLI عند الحاجة, ويستدعي
`sorafs_cli manifest sign`, que não é compatível com `manifest verify-signature`
Faça uma pausa e desista. Exemplo:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

Descrição:

- تتبع مدخلات الإصدار (الحمولة, الخطط, الملخصات, تجزئة الرمز المتوقعة) داخل
  Certifique-se de que o dispositivo esteja conectado a um computador portátil. تُظهر حزمة
  luminárias em `fixtures/sorafs_manifest/ci_sample/`.
- أسّس أتمتة CI على `.github/workflows/sorafs-cli-release.yml`; فهي تشغّل بوابة
  O fluxo de trabalho é definido como um fluxo de trabalho.
  حافظ على ترتيب الأوامر نفسه (بوابة الإصدار → التوقيع → التحقق) em أنظمة CI الأخرى
  Verifique se há algum problema com isso.
- بالملفات `manifest.bundle.json` e `manifest.sig` e `manifest.sign.summary.json`
  و`manifest.verify.summary.json` معًا — فهي تشكّل الحزمة المشار إليها في إشعار الحوكمة.
- عندما يحدّث الإصدار fixtures المعتمدة, انسخ المانيفست المحدّث وخطة الـ chunk
  والملخصات إلى `fixtures/sorafs_manifest/ci_sample/` (وحدّث
  `docs/examples/sorafs_ci_sample/manifest.template.json`) é um problema.
  يعتمد المشغلون اللاحقون على fixtures الملتزمة لإعادة إنتاج حزمة الإصدار.
- O código de segurança do produto `sorafs_cli proof stream` é o mais importante e mais barato
  O aplicativo de streaming de prova está disponível para download.
- Verifique o valor `--identity-token-audience` para obter mais informações sobre o produto.
  الإصدار؛ تتحقق الحوكمة من الجمهور مقابل سياسة Fulcio قبل اعتماد النشر.

Use `scripts/sorafs_gateway_self_cert.sh` para obter mais informações
Não. وجّهه إلى حزمة المانيفست نفسها لإثبات أن الشهادة تطابق الآرتيفاكت المرشح:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. وضع الوسم والنشر

بعد نجاح الفحوصات واكتمال الخطافات:1. Verifique `sorafs_cli --version` e `sorafs_fetch --version` para obter mais informações
   الإصدار الجديد.
2. Verifique o valor do arquivo `sorafs_release.toml` no local de trabalho (recomendado) ou
   Isso pode ser feito com bastante frequência. تجنّب الاعتماد على متغيرات بيئة عشوائية؛ مرّر
   A CLI está usando `--config` (ou seja, não importa) para obter mais informações sobre o produto e o software
   لإعادة الإنتاج.
3. أنشئ وسمًا موقّعًا (مفضّل) أو وسمًا مُعلّقًا:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. ارفع الآرتيفاكتات (حزم CAR, المانيفستات, ملخصات الأدلة, ملاحظات الإصدار, مخرجات
   (./developer-deployment.md).
   إذا أنشأ الإصدار fixtures جديدة, ارفعها إلى مستودع fixtures المشترك أو مخزن الكائنات
   Isso pode ser feito por meio de uma mensagem de erro.
5. أخطر قناة الحوكمة بروابط الوسم الموقّع, وملاحظات الإصدار, وتجزئات حزمة المانيفست/
   O código de barras `manifest.sign/verify` está disponível e não está disponível. أضف رابط وظيفة CI
   (أو أرشيف السجلات) التي شغّلت `ci/check_sorafs_cli_release.sh` e
   `scripts/release_sorafs_cli.sh`. حدّث تذكرة الحوكمة ليتمكن المدققون من تتبع
   الموافقات إلى الآرتيفاكتات؛ Executar trabalho `.github/workflows/sorafs-cli-release.yml`
   إشعارات, اربط التجزئات المسجلة بدل لصق ملخصات عشوائية.

## 6. ما بعد الإصدار

- تأكد من تحديث الوثائق التي تشير إلى الإصدار الجديد (البدء السريع, قوالب CI)
  E você não pode fazer isso.
- أضف عناصر إلى خارطة الطريق إذا لزم عمل لاحق (مثل أعلام الترحيل أو إيقاف
  المانيفستات القديمة).
- أرشف سجلات مخرجات بوابة الإصدار للمدققين — واحفظها بجانب الآرتيفاكتات الموقعة.

Você pode usar o CLI e o SDK e usar o CLI no seu computador.