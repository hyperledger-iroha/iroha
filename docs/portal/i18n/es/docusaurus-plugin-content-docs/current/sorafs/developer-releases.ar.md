---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: عملية الإصدار
summary: نفّذ بوابة إصدار CLI/SDK، وطبّق سياسة الإصدارات المشتركة، وانشر ملاحظات الإصدار المعتمدة.
---

# عملية الإصدار

تُشحن ثنائيات SoraFS (`sorafs_cli`, `sorafs_fetch`, helpers) وحزم SDK
(`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`) معًا. يحافظ خط إصدار واحد على
تناسق CLI والمكتبات، ويضمن تغطية lint/test، ويلتقط آرتيفاكتات للمستهلكين
اللاحقين. نفّذ قائمة التحقق أدناه لكل وسم مرشح.

## 0. تأكيد اعتماد مراجعة الأمان

قبل تنفيذ بوابة الإصدار التقنية، التقط أحدث آرتيفاكتات مراجعة الأمان:

- نزّل أحدث مذكرة مراجعة أمان SF-6 ([reports/sf6-security-review](./reports/sf6-security-review.md))
  وسجّل تجزئة SHA256 الخاصة بها في تذكرة الإصدار.
- أرفق رابط تذكرة المعالجة (مثل `governance/tickets/SF6-SR-2026.md`) وسجّل
  الموافقين من Security Engineering ومجموعة Tooling Working Group.
- تحقّق من إغلاق قائمة المعالجة في المذكرة؛ العناصر غير المحسومة تحظر الإصدار.
- استعد لرفع سجلات harness التكافؤ (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  بجانب حزمة المانيفست.
- تأكد أن أمر التوقيع الذي ستنفذه يتضمن `--identity-token-provider` و
  `--identity-token-audience=<aud>` صريحًا حتى يُلتقط نطاق Fulcio ضمن أدلة الإصدار.

ضمّن هذه الآرتيفاكتات عند إخطار الحوكمة ونشر الإصدار.

## 1. تنفيذ بوابة الإصدار/الاختبارات

يشغّل المساعد `ci/check_sorafs_cli_release.sh` التنسيق وClippy والاختبارات عبر
حزم CLI وSDK مع دليل target محلي داخل مساحة العمل (`.target`) لتجنب تعارضات
الأذونات عند التنفيذ داخل حاويات CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

ينفذ السكربت التحققات التالية:

- `cargo fmt --all -- --check` (workspace)
- `cargo clippy --locked --all-targets` لحزمة `sorafs_car` (مع ميزة `cli`)،
  و`sorafs_manifest` و`sorafs_chunker`
- `cargo test --locked --all-targets` لتلك الحزم نفسها

إذا فشل أيٌّ من الخطوات، أصلح التراجع قبل وضع الوسم. يجب أن تكون بناءات
الإصدار متصلة بـ main؛ لا تقم بعمل cherry-pick لإصلاحات ضمن فروع الإصدار.
تتحقق البوابة أيضًا من توفير أعلام التوقيع دون مفاتيح (`--identity-token-issuer`,
`--identity-token-audience`) حيث يلزم؛ تؤدي الحجج المفقودة إلى فشل التشغيل.

## 2. تطبيق سياسة الإصدارات

تستخدم جميع حزم CLI/SDK الخاصة بـ SoraFS نظام SemVer:

- `MAJOR`: يُستخدم لأول إصدار 1.0. قبل 1.0 يشير رفع النسخة الثانوية `0.y`
  **إلى تغييرات كاسرة** في واجهة CLI أو مخططات Norito.
- `MINOR`: ميزات متوافقة للخلف (أوامر/أعلام جديدة، حقول Norito جديدة خلف سياسة
  اختيارية، إضافات تليمترية).
- `PATCH`: إصلاحات عيوب، وإصدارات وثائق فقط، وتحديثات تبعيات لا تغيّر السلوك
  الملاحظ.

حافظ دائمًا على نسخ `sorafs_car` و`sorafs_manifest` و`sorafs_chunker` متطابقة كي
يعتمد مستهلكو SDK اللاحقون على سلسلة إصدار موحّدة. عند رفع الإصدارات:

1. حدّث حقول `version =` في كل `Cargo.toml`.
2. أعد توليد `Cargo.lock` عبر `cargo update -p <crate>@<new-version>` (تفرض
   مساحة العمل نسخًا صريحة).
3. شغّل بوابة الإصدار مرة أخرى لضمان عدم بقاء آرتيفاكتات قديمة.

## 3. إعداد ملاحظات الإصدار

يجب أن ينشر كل إصدار سجل تغييرات Markdown يبرز تغييرات CLI وSDK وما يؤثر على
الحوكمة. استخدم القالب في `docs/examples/sorafs_release_notes.md` (انسخه إلى
دليل آرتيفاكتات الإصدار واملأ الأقسام بتفاصيل ملموسة).

الحد الأدنى من المحتوى:

- **أبرز النقاط**: عناوين الميزات لمستخدمي CLI وSDK.
- **التوافق**: تغييرات كاسرة، ترقيات السياسات، المتطلبات الدنيا للبوابة/العقدة.
- **خطوات الترقية**: أوامر TL;DR لتحديث تبعيات cargo وإعادة تشغيل fixtures الحتمية.
- **التحقق**: تجزئات مخرجات الأوامر أو الأظرف والإصدار الدقيق لـ
  `ci/check_sorafs_cli_release.sh` الذي تم تشغيله.

أرفق ملاحظات الإصدار المكتملة بالوسم (مثل نص إصدار GitHub) وخزّنها بجانب
الآرتيفاكتات المولدة حتميًا.

## 4. تنفيذ خطافات الإصدار

شغّل `scripts/release_sorafs_cli.sh` لتوليد حزمة التوقيع وملخص التحقق الذي
يُشحن مع كل إصدار. يقوم الغلاف ببناء CLI عند الحاجة، ويستدعي
`sorafs_cli manifest sign`، ثم يعيد تشغيل `manifest verify-signature` فورًا
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

نصائح:

- تتبع مدخلات الإصدار (الحمولة، الخطط، الملخصات، تجزئة الرمز المتوقعة) داخل
  المستودع أو إعداد النشر ليبقى السكربت قابلاً لإعادة الإنتاج. تُظهر حزمة
  fixtures في `fixtures/sorafs_manifest/ci_sample/` التخطيط المعتمد.
- أسّس أتمتة CI على `.github/workflows/sorafs-cli-release.yml`؛ فهي تشغّل بوابة
  الإصدار، وتستدعي السكربت أعلاه، وتؤرشف الحزم/التواقيع كآرتيفاكتات workflow.
  حافظ على ترتيب الأوامر نفسه (بوابة الإصدار → التوقيع → التحقق) في أنظمة CI الأخرى
  حتى تتطابق سجلات التدقيق مع التجزئات الناتجة.
- احتفظ بالملفات `manifest.bundle.json` و`manifest.sig` و`manifest.sign.summary.json`
  و`manifest.verify.summary.json` معًا — فهي تشكّل الحزمة المشار إليها في إشعار الحوكمة.
- عندما يحدّث الإصدار fixtures المعتمدة، انسخ المانيفست المحدّث وخطة الـ chunk
  والملخصات إلى `fixtures/sorafs_manifest/ci_sample/` (وحدّث
  `docs/examples/sorafs_ci_sample/manifest.template.json`) قبل وضع الوسم.
  يعتمد المشغلون اللاحقون على fixtures الملتزمة لإعادة إنتاج حزمة الإصدار.
- التقط سجل تشغيل تحقق قنوات `sorafs_cli proof stream` ذات الحدود وأرفقه بحزمة
  الإصدار لإثبات أن ضمانات proof streaming لا تزال نشطة.
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

بعد نجاح الفحوصات واكتمال الخطافات:

1. شغّل `sorafs_cli --version` و`sorafs_fetch --version` لتأكيد أن الثنائيات تعلن
   الإصدار الجديد.
2. جهّز إعداد الإصدار في ملف `sorafs_release.toml` مُلتزم في المستودع (مفضّل) أو
   ملف إعداد آخر يتتبعه مستودع النشر. تجنّب الاعتماد على متغيرات بيئة عشوائية؛ مرّر
   المسارات إلى CLI عبر `--config` (أو ما يعادله) حتى تكون مدخلات الإصدار صريحة وقابلة
   لإعادة الإنتاج.
3. أنشئ وسمًا موقّعًا (مفضّل) أو وسمًا مُعلّقًا:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. ارفع الآرتيفاكتات (حزم CAR، المانيفستات، ملخصات الأدلة، ملاحظات الإصدار، مخرجات
   الشهادات) إلى سجل المشروع وفق قائمة تحقق الحوكمة في [دليل النشر](./developer-deployment.md).
   إذا أنشأ الإصدار fixtures جديدة، ارفعها إلى مستودع fixtures المشترك أو مخزن الكائنات
   كي تتمكن أتمتة التدقيق من مقارنة الحزمة المنشورة مع التحكم بالمصدر.
5. أخطر قناة الحوكمة بروابط الوسم الموقّع، وملاحظات الإصدار، وتجزئات حزمة المانيفست/
   التواقيع، وملخصات `manifest.sign/verify` المؤرشفة، وأي أظرف شهادات. أضف رابط وظيفة CI
   (أو أرشيف السجلات) التي شغّلت `ci/check_sorafs_cli_release.sh` و
   `scripts/release_sorafs_cli.sh`. حدّث تذكرة الحوكمة ليتمكن المدققون من تتبع
   الموافقات إلى الآرتيفاكتات؛ وعندما يرسل job `.github/workflows/sorafs-cli-release.yml`
   إشعارات، اربط التجزئات المسجلة بدل لصق ملخصات عشوائية.

## 6. ما بعد الإصدار

- تأكد من تحديث الوثائق التي تشير إلى الإصدار الجديد (البدء السريع، قوالب CI)
  أو أكد عدم الحاجة إلى تغييرات.
- أضف عناصر إلى خارطة الطريق إذا لزم عمل لاحق (مثل أعلام الترحيل أو إيقاف
  المانيفستات القديمة).
- أرشف سجلات مخرجات بوابة الإصدار للمدققين — واحفظها بجانب الآرتيفاكتات الموقعة.

اتباع هذا المسار يُبقي CLI وحزم SDK وملحقات الحوكمة متزامنة في كل دورة إصدار.
