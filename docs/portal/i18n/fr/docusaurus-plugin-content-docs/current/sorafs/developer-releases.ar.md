---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
Titre : عملية الإصدار
Résumé : Vous utilisez CLI/SDK et vous avez des liens avec les éléments suivants.
---

#عملية الإصدار

Utiliser les logiciels SoraFS (`sorafs_cli`, `sorafs_fetch`, helpers) et le SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) ici. يحافظ خط إصدار واحد على
Utilisez la CLI pour utiliser les lint/test et les fonctionnalités de lint/test.
اللاحقين. نفّذ قائمة التحقق أدناه لكل وسم مرشح.

## 0. تأكيد اعتماد مراجعة الأمان

قبل تنفيذ بوابة الإصدار التقنية، التقط أحدث آرتيفاكتات مراجعة الأمان:

- نزّل أحدث مذكرة مراجعة أمان SF-6 ([reports/sf6-security-review](./reports/sf6-security-review.md))
  وسجّل تجزئة SHA256 الخاصة بها في تذكرة الإصدار.
- أرفق رابط تذكرة المعالجة (مثل `governance/tickets/SF6-SR-2026.md`) et
  Groupe de travail sur l'ingénierie de sécurité et l'outillage.
- تحقّق من إغلاق قائمة المعالجة في المذكرة؛ العناصر غير المحسومة تحظر الإصدار.
- Harnais pour harnais de sécurité (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  بجانب حزمة المانيفست.
- تأكد أن أمر التوقيع الذي ستنفذه يتضمن `--identity-token-provider` et
  `--identity-token-audience=<aud>` صريحًا حتى يُلتقط نطاق Fulcio ضمن أدلة الإصدار.

ضمّن هذه الآرتيفاكتات عند إخطار الحوكمة ونشر الإصدار.

## 1. تنفيذ بوابة الإصدار/الاختبارات

يشغّل المساعد `ci/check_sorafs_cli_release.sh` التنسيق وClippy والاختبارات عبر
Utilisez CLI et SDK pour la cible avec la clé (`.target`) pour les détails
الأذونات عند التنفيذ داخل حاويات CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

ينفذ السكربت التحققات التالية:- `cargo fmt --all -- --check` (espace de travail)
- `cargo clippy --locked --all-targets` pour `sorafs_car` (pour `cli`)
  و`sorafs_manifest` و`sorafs_chunker`
- `cargo test --locked --all-targets` pour le téléchargement

إذا فشل أيٌّ من الخطوات، أصلح التراجع قبل وضع الوسم. يجب أن تكون بناءات
الإصدار متصلة بـ main؛ لا تقم بعمل cherry-pick لإصلاحات ضمن فروع الإصدار.
تتحقق البوابة أيضًا من توفير أعلام التوقيع دون مفاتيح (`--identity-token-issuer`,
`--identity-token-audience`) حيث يلزم؛ تؤدي الحجج المفقودة إلى فشل التشغيل.

## 2. تطبيق سياسة الإصدارات

Utilisez la CLI/SDK pour SoraFS de SemVer :

- `MAJOR` : version 1.0. Version 1.0 de la version `0.y`
  **إلى تغييرات كاسرة** في واجهة CLI أو مخططات Norito.
- `MINOR` : ميزات متوافقة للخلف (أوامر/أعلام جديدة، حقول Norito جديدة خلف سياسة
  اختيارية، إضافات تليمترية).
- `PATCH` : Les appareils et les appareils électroniques sont également disponibles pour les appareils ménagers.
  الملاحظ.

حافظ دائمًا على نسخ `sorafs_car` و`sorafs_manifest` و`sorafs_chunker` متطابقة كي
Vous devez utiliser le SDK pour créer un lien vers votre compte. عند رفع الإصدارات:

1. Utilisez `version =` pour `Cargo.toml`.
2. Utilisez `Cargo.lock` pour `cargo update -p <crate>@<new-version>` (démarrage
   مساحة العمل نسخًا صريحة).
3. شغّل بوابة الإصدار مرة أخرى لضمان عدم بقاء آرتيفاكتات قديمة.

## 3. إعداد ملاحظات الإصدارVous pouvez également télécharger Markdown et les fonctionnalités CLI et SDK ainsi que les fonctionnalités de Markdown.
الحوكمة. استخدم القالب في `docs/examples/sorafs_release_notes.md` (انسخه إلى
دليل آرتيفاكتات الإصدار واملأ الأقسام بتفاصيل ملموسة).

الحد الأدنى من المحتوى:

- **أبرز النقاط** : عناوين الميزات لمستخدمي CLI et SDK.
- **التوافق** : تغييرات كاسرة، ترقيات السياسات، المتطلبات الدنيا للبوابة/العقدة.
- **خطوات الترقية** : أوامر TL;DR لتحديث تبعيات cargo وإعادة تشغيل luminaires الحتمية.
- **التحقق** : تجزئات مخرجات الأوامر أو الأظرف والإصدار الدقيق لـ
  `ci/check_sorafs_cli_release.sh` Vous êtes ici.

أرفق ملاحظات الإصدار المكتملة بالوسم (مثل نص إصدار GitHub) et خزّنها بجانب
الآرتيفاكتات المولدة حتميًا.

## 4. تنفيذ خطافات الإصدار

شغّل `scripts/release_sorafs_cli.sh` لتوليد حزمة التوقيع وملخص التحقق الذي
يُشحن مع كل إصدار. يقوم الغلاف ببناء CLI عند الحاجة، ويستدعي
`sorafs_cli manifest sign`, vous avez besoin de `manifest verify-signature` pour
لتظهر الإخفاقات قبل وضع الوسم. مثال:

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

Version:- تتبع مدخلات الإصدار (الحمولة، الخطط، الملخصات، تجزئة الرمز المتوقعة) داخل
  المستودع أو إعداد النشر ليبقى السكربت قابلاً لإعادة الإنتاج. تُظهر حزمة
  luminaires selon `fixtures/sorafs_manifest/ci_sample/` التخطيط المعتمد.
- أسّس أتمتة CI على `.github/workflows/sorafs-cli-release.yml`؛ فهي تشغّل بوابة
  Les flux de travail sont également liés au flux de travail.
  حافظ على ترتيب الأوامر نفسه (بوابة الإصدار → التوقيع → التحقق) في أنظمة CI الأخرى
  حتى تتطابق سجلات التدقيق مع التجزئات الناتجة.
- احتفظ بالملفات `manifest.bundle.json` et `manifest.sig` et `manifest.sign.summary.json`
  و`manifest.verify.summary.json` معًا — فهي تشكّل الحزمة المشار إليها في إشعار الحوكمة.
- عندما يحدّث الإصدار luminaires المعتمدة، انسخ المانيفست المحدّث وخطة الـ chunk
  والملخصات إلى `fixtures/sorafs_manifest/ci_sample/` (وحدّث
  `docs/examples/sorafs_ci_sample/manifest.template.json`) قبل وضع الوسم.
  يعتمد المشغلون اللاحقون على calendriers الملتزمة لإعادة إنتاج حزمة الإصدار.
- التقط سجل تشغيل تحقق قنوات `sorafs_cli proof stream` ذات الحدود وأرفقه بحزمة
  Il s'agit d'une preuve en streaming pour vous.
- سجّل قيمة `--identity-token-audience` الدقيقة المستخدمة أثناء التوقيع في ملاحظات
  الإصدار؛ تتحقق الحوكمة من الجمهور مقابل سياسة Fulcio قبل اعتماد النشر.

استخدم `scripts/sorafs_gateway_self_cert.sh` عندما يحمل الإصدار أيضًا إطلاقًا
للبوابة. وجّهه إلى حزمة المانيفست نفسها لإثبات أن الشهادة تطابق الآرتيفاكت المرشح:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. وضع الوسم والنشر

بعد نجاح الفحوصات واكتمال الخطافات:1. شغّل `sorafs_cli --version` و`sorafs_fetch --version` لتأكيد أن الثنائيات تعلن
   الإصدار الجديد.
2. جهّز إعداد الإصدار في ملف `sorafs_release.toml` مُلتزم في المستودع (مفضّل) أو
   ملف إعداد آخر يتتبعه مستودع النشر. تجنّب الاعتماد على متغيرات بيئة عشوائية؛ مرّر
   Liens CLI vers `--config` (pour plus d'informations)
   لإعادة الإنتاج.
3. أنشئ وسمًا موقّعًا (مفضّل) أو وسمًا مُعلّقًا:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. ارفع الآرتيفاكتات (حزم CAR، المانيفستات، ملخصات الأدلة، ملاحظات الإصدار، مخرجات
   (./developer-deployment.md).
   إذا أنشأ الإصدار luminaires جديدة، ارفعها إلى مستودع luminaires المشترك أو مخزن الكائنات
   كي تتمكن أتمتة التدقيق من مقارنة الحزمة المنشورة مع التحكم بالمصدر.
5. أخطر قناة الحوكمة بروابط الوسم الموقّع، وملاحظات الإصدار، وتجزئات حزمة المانيفست/
   التواقيع، وملخصات `manifest.sign/verify` المؤرشفة، وأي أظرف شهادات. أضف رابط وظيفة CI
   (أو أرشيف السجلات) التي شغّلت `ci/check_sorafs_cli_release.sh` et
   `scripts/release_sorafs_cli.sh`. حدّث تذكرة الحوكمة ليتمكن المدققون من تتبع
   الموافقات إلى الآرتيفاكتات؛ وعندما يرسل emploi `.github/workflows/sorafs-cli-release.yml`
   إشعارات، اربط التجزئات المسجلة بدل لصق ملخصات عشوائية.

## 6. ما بعد الإصدار- تأكد من تحديث الوثائق التي تشير إلى الإصدار الجديد (البدء السريع، قوالب CI)
  أو أكد عدم الحاجة إلى تغييرات.
- أضف عناصر إلى خارطة الطريق إذا لزم عمل لاحق (مثل أعلام الترحيل أو إيقاف
  المانيفستات القديمة).
- أرشف سجلات مخرجات بوابة الإصدار للمدققين — واحفظها بجانب الآرتيفاكتات الموقعة.

Il s'agit de la CLI et du SDK ainsi que des éléments de support pour la mise à jour.