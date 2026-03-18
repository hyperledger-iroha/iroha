---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-releases.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: عملية الإصدار
ملخص: تنفيذ بوابة إصدار CLI/SDK وتطبيق سياسة مشاركة الإصدار ونشر ملاحظات الإصدار الأساسية.
---

# عملية الإصدار

الثنائيات SoraFS (`sorafs_cli`، `sorafs_fetch`، المساعدون) والصناديق SDK
(`sorafs_car`، `sorafs_manifest`، `sorafs_chunker`) هي مجموعة كتب. لو خط الأنابيب
إطلاق سراح CLI والمكتبات المتوافقة مع ضمان تغطية الخيط/الاختبار
والتقاط القطع الأثرية للمستهلكين في اتجاه مجرى النهر. قم بتنفيذ قائمة التحقق
ci-dessous pour chaque tag المرشح.

## 0. تأكيد التحقق من صحة المراجعة الأمنية

قبل تنفيذ تقنية البوابة، قم بالتقاط آخر القطع الأثرية
مراجعة الأمن :- قم بتنزيل مذكرة المراجعة الأمنية SF-6 الأحدث ([reports/sf6-security-review](./reports/sf6-security-review.md))
  وقم بتسجيل تجزئة SHA256 في تذكرة الإصدار.
- قم بتسجيل رهن تذكرة الإصلاح (على سبيل المثال `governance/tickets/SF6-SR-2026.md`) ولاحظه
  معتمدو هندسة الأمن ومجموعة عمل الأدوات.
- التحقق من أن قائمة التحقق من معالجة الذاكرة مغلقة؛ العناصر لا تمنع الإصدار.
- التحضير لتحميل سجلات حزام التكافؤ (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  مع حزمة البيان.
- قم بتأكيد أمر التوقيع الذي ستنفذه بما في ذلك `--identity-token-provider` et
  `--identity-token-audience=<aud>` واضح لالتقاط النطاق الكامل في إجراءات الإصدار.

قم بتضمين هذه العناصر عند إخطار الإدارة والنشر.

## 1. قم بتنفيذ بوابة التحرير/الاختبارات

يقوم المساعد `ci/check_sorafs_cli_release.sh` بتنفيذ التنسيق وClippy والاختبارات
على الصناديق CLI وSDK مع مرجع محلي مستهدف في مساحة العمل (`.target`)
لتفادي تعارض الأذونات أثناء التنفيذ في حاويات CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

يقوم البرنامج النصي بتفعيل التأكيدات التالية:

- `cargo fmt --all -- --check` (مساحة العمل)
- `cargo clippy --locked --all-targets` لـ `sorafs_car` (مع الميزة `cli`)،
  `sorafs_manifest` و`sorafs_chunker`
- `cargo test --locked --all-targets` لهذه الصناديقإذا تم صدى الشريط، قم بتصحيح الانحدار قبل العلامة. ليه يبني دي الإصدار
doivent être continus avec main ; لا تقم باختيار التصحيحات في الفروع
الافراج. تتحقق البوابة أيضًا من أعلام التوقيع بدون مفتاح (`--identity-token-issuer`،
`--identity-token-audience`) هو مصدر عند الطلب؛ الخط les الوسيطات manquants
échouer l'execution.

## 2. تطبيق سياسة الإصدار

جميع الصناديق CLI/SDK SoraFS تستخدم SemVer :

- `MAJOR` : مقدمة للإصدار الأول 1.0. Avant 1.0، المضخة الصغيرة `0.y`
  **للإشارة إلى التغييرات التي تحدث** على سطح CLI أو المخططات Norito.
- `MINOR`: الوظائف الجديدة (الأوامر/الأعلام الجديدة، الأبطال الجدد Norito
  derrière une politique optionnelle، ajouts de télémétrie).
- `PATCH`: تصحيحات الأخطاء وإصدار وثائق فريدة وتحديثات يومية
  الاعتماديات التي لا تغير السلوك الذي يمكن ملاحظته.

جارديز toujours `sorafs_car` و`sorafs_manifest` و`sorafs_chunker` في نفس الإصدار
حتى يتمكن مستهلكو SDK المتلقين للمعلومات من الاعتماد على سلسلة إصدار واحدة فقط
محاذاة. Lors des bumps de version :1. ابدأ اليوم في الأبطال `version =` في كل `Cargo.toml`.
2. قم بإعادة إنشاء `Cargo.lock` عبر `cargo update -p <crate>@<new-version>` (مساحة العمل
   فرض الإصدارات الصريحة).
3. أعد فتح بوابة التحرير لتجنب الآثار المحيطة.

## 3. قم بإعداد ملاحظات الإصدار

كل إصدار يجب أن ينشر سجل التغيير وتخفيض السعر قبل التغييرات
يؤثر على CLI وSDK والحوكمة. استخدم القالب في
`docs/examples/sorafs_release_notes.md` (نسخ في ذخيرتك الفنية
قم بتحرير واستعادة الأقسام بالتفاصيل المحددة).

محتوى الحد الأدنى:

- **الإضاءات**: عدد من الميزات المخصصة لمستخدمي CLI وSDK.
- **التوافق**: التغييرات، التحديثات السياسية، الحد الأدنى من المتطلبات
  بوابة/nœud.
- **Étapes d'upgrade** : أوامر TL;DR من أجل مواصلة العمل على الاعتمادات على البضائع وآخرون
  relancer les Installations déterministes.
- **التحقق** : تجزئات فرز أو مغلفات ومراجعة دقيقة
  تم تنفيذ `ci/check_sorafs_cli_release.sh`.

قم بتضمين ملاحظات الإصدار الراجعة على العلامة (على سبيل المثال مجموعة إصدار GitHub) وما إلى ذلك
قم بتخزين القطع الأثرية التي تم إنشاؤها بطريقة محددة.

## 4. تنفيذ خطافات التحريرقم بتنفيذ `scripts/release_sorafs_cli.sh` لإنشاء مجموعة التوقيعات والتوقيعات
السيرة الذاتية للتدقيق الكتابي مع كل إصدار. المجمع يبني CLI si
ضروري، اتصل بـ `sorafs_cli manifest sign` واستمتع على الفور
`manifest verify-signature` لإعادة تثبيت التأثيرات أمام العلامة. مثال :

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

نصائح :- متابعة مدخلات التحرير (الحمولة، الخطط، الملخصات، تجزئة الرمز المميز)
  في الريبو الخاص بك أو تكوين النشر لحماية النص القابل لإعادة الإنتاج.
  يتم عرض الحزمة CI sous `fixtures/sorafs_manifest/ci_sample/` للتخطيط الكنسي.
- إنشاء أتمتة CI على `.github/workflows/sorafs-cli-release.yml` ; تم تنفيذه
  بوابة الإصدار، واستدعاء البرنامج النصي ci-dessus وأرشيف الحزم/التوقيعات
  التحف من سير العمل. قم بإعادة إنتاج نفس أمر الأوامر (البوابة → التوقيع →
  التحقق) في أنظمة أخرى CI لمحاذاة سجلات التدقيق مع التجزئات.
- جارديز `manifest.bundle.json`، `manifest.sig`، `manifest.sign.summary.json` وآخرون
  `manifest.verify.summary.json` ensemble : يتم تشكيل الحزمة المرجعية فيها
  إشعار الحكم.
- عند الإصدار مع التحديثات القانونية، انسخ البيان rafraîchi،
  خطة قطعة وملخصات في `fixtures/sorafs_manifest/ci_sample/` (ومع ذلك
  في اليوم `docs/examples/sorafs_ci_sample/manifest.template.json`) قبل العلامة. ليه
  يعتمد المشغلون النهائيون على التركيبات المخصصة لإعادة إنتاج الحزمة.
- التقط سجل تنفيذ التحقق من القنوات المحدودة
  `sorafs_cli proof stream` وقم بتحريك حزمة الإصدار لتوضيحها
  garde-fous de إثبات تدفق بقية الأنشطة.
- لاحظ أن l'`--identity-token-audience` تم استخدامه تمامًا عند التوقيع في الملاحظات
  دي الافراج؛ تستعيد الحكومة الجمهور بالسياسة بالاستحسان المسبق.استخدم `scripts/sorafs_gateway_self_cert.sh` أثناء الإصدار المتضمن أيضًا الطرح
بوابة. أشر إلى نفس حزمة البيان لإثبات المصادقة
تتوافق مع المرشح الاصطناعي :

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. تاغر وآخرون

بعد مرور الشيكات ونهاية الخطافات :1. قم بتنفيذ `sorafs_cli --version` و`sorafs_fetch --version` لتأكيد الملفات الثنائية
   تقرير النسخة الجديدة.
2. قم بإعداد تكوين الإصدار في إصدار `sorafs_release.toml` (المفضل)
   أو ملف تكوين آخر تابع لمستودع النشر الخاص بك. تجنب الاعتماد
   متغيرات البيئة المخصصة ؛ قم بتمرير الصفوف إلى CLI مع `--config` (ou
   أي ما يعادل) لكي تكون المدخلات صريحة وقابلة للتكرار.
3. قم بإنشاء علامة مسجلة (مفضلة) أو علامة توضيحية :
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. قم بتحميل العناصر (حزم CAR، البيانات، السيرة الذاتية للإثباتات، مذكرات الإصدار،
   مخرجات الشهادة) مقابل سجل المشروع من القائمة المرجعية للحوكمة
   في [دليل النشر](./developer-deployment.md). إذا قمت بإصدار منتج
   تركيبات جديدة، ادفعها نحو مخزن التركيبات من خلال مشاركة أو تخزين العناصر
   حتى تتمكن أتمتة التدقيق من مقارنة الحزمة المنشورة بالتحكم بالمصدر.
5. إخطار قناة الحوكمة من خلال الامتيازات الموقعة ومذكرات الإصدار،
   تجزئات الحزمة/توقيعات البيان، السيرة الذاتية المؤرشفة `manifest.sign/verify`
   وآخرون مغلف الشهادة. قم بتضمين عنوان URL الخاص بوظيفة CI (أو أرشيف السجلات) الذي
   تم تنفيذ `ci/check_sorafs_cli_release.sh` و`scripts/release_sorafs_cli.sh`. ميتز أ
   يوم بطاقة الحوكمة حتى يتمكن المدققون من الحصول على الموافقاتالمصنوعات اليدوية المساعدة؛ عند إرسال `.github/workflows/sorafs-cli-release.yml` للإشعارات،
   قم بتسجيل التجزئات بدلاً من تجميع السيرة الذاتية المخصصة.

## 6. Suivi بعد الإصدار

- تأكد من أن الوثائق تشير إلى الإصدار الجديد (البدء السريع، قوالب CI)
  إنه يوم أو يؤكد أن التغيير ليس مطلوبًا.
- إنشاء مدخلات خريطة الطريق إذا كان العمل التالي ضروريًا (مثل أعلام الهجرة،
- أرشفة سجلات الخروج من بوابة الإصدار للمراجعين: قم بتخزينها في كل مكان
  توقيعات المصنوعات اليدوية.

يحافظ خط الأنابيب هذا على CLI وصناديق SDK وعناصر الإدارة
تتم محاذاة كل دورة من الإصدار.