---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/developer-releases.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
العنوان: عملية الإصدار
ملخص: قم بتنفيذ بوابة الإصدار الخاصة بـ CLI/SDK، وتطبيق سياسة مشاركة الإصدار ونشر ملاحظات الإصدار الأساسية.
---

# عملية الإصدار

الثنائيات من SoraFS (`sorafs_cli`، `sorafs_fetch`، المساعدون) وصناديق SDK
(`sorafs_car`، `sorafs_manifest`، `sorafs_chunker`) sao entregues juntos. يا خط الأنابيب
إصدار غطاء سطر الأوامر وإصدار الكتب الإلكترونية، مع ضمان تغطية الوبر/الاختبار
الاستيلاء على القطع الأثرية للمستهلكين في اتجاه مجرى النهر. قم بتنفيذ قائمة مرجعية abaixo para cada
علامة المرشح.

## 0. تأكيد الموافقة على المراجعة الأمنية

قبل تنفيذ البوابة التقنية للتحرير، قم بالتقاط أحدث المصنوعات
مراجعة الأمان:

- قم بإضافة مذكرة مراجعة الأمان SF-6 الأحدث ([reports/sf6-security-review](./reports/sf6-security-review.md))
  قم بتسجيل تجزئة SHA256 الخاصة بك بدون تذكرة تحرير.
- قم بإرفاق رابط بطاقة الإصلاح (على سبيل المثال، `governance/tickets/SF6-SR-2026.md`) والملاحظة
  نحن خبراء الهندسة الأمنية ومجموعة عمل الأدوات.
- التحقق من عدم ظهور قائمة المراجعة الخاصة بالإصلاحات؛ itens pendentes bloqueiam o Release.
- إعداد وتحميل سجلات dos لتسخير de paridade (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  جنبًا إلى جنب مع حزمة البيان.
- قم بتأكيد أمر الاغتيال الذي ستنفذه بما في ذلك `--identity-token-provider` e
  أم `--identity-token-audience=<aud>` صريح لالتقاط أو استكشاف الاستكمال لإصدار الأدلة.قم بتضمين هذه الأعمال الفنية في إشعار الإدارة ونشر الإصدار.

## 1. تنفيذ بوابة تحرير/الخصيتين

يا مساعد `ci/check_sorafs_cli_release.sh` هو تنسيق و Clippy و خصيتنا في الصناديق
CLI وSDK مع الدليل المستهدف لمساحة العمل المحلية (`.target`) لتجنب التعارضات
يسمح بالتنفيذ داخل حاويات CI.

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

O script faz as seguintes verificacoes:

- `cargo fmt --all -- --check` (مساحة العمل)
- `cargo clippy --locked --all-targets` لـ `sorafs_car` (مع ميزة `cli`)،
  `sorafs_manifest` و`sorafs_chunker`
- `cargo test --locked --all-targets` لصناديق الرسائل

إذا كان الأمر كذلك، فقد تم التراجع قبل التراجع. يبني الإصدار
سر مستمر كوم الرئيسي؛ لا يمكنك اختيار التصحيحات من فروع الإصدار. يا
Gate Tambem Verifica Se os Flags de Assinatura SEM Chave (`--identity-token-issuer`,
`--identity-token-audience`) للتعلم عند التطبيق؛ hackedos faltando
Fazem a execucao falhar.

## 2. تطبيق الإصدار السياسي

جميع الصناديق CLI/SDK من SoraFS تستخدم SemVer:- `MAJOR`: تم تقديمه للإصدار الأول 1.0. ما قبل 1.0 أو أقل
  `0.y` **يشير إلى كائنات quebradoras** على سطح CLI أو مفاهيمنا Norito.
- `MINOR`: ميزات العمل المتوافقة مع (الكوماندوز الجديدة/الأعلام، الجديدة
  Campos Norito المحمي من أجل السياسة الاختيارية، adicoes de telemetria).
- `PATCH`: تصحيح الأخطاء وإصدار بعض المستندات وتحديثها
  التبعيات التي لا تسمح بملاحظة السلوك.

الحفاظ دائمًا على `sorafs_car` و`sorafs_manifest` و`sorafs_chunker` على العكس من ذلك
لكي يعتمد مستهلكو SDK المصب على سلسلة واحدة
العكس صحيح. لتجديد الآيات:

1. قم بتحديث المجال `version =` في كل `Cargo.toml`.
2. قم بإعادة إنشاء `Cargo.lock` عبر `cargo update -p <crate>@<new-version>` (مساحة العمل
   آيات صريحة).
3. توجه مرة أخرى إلى بوابة التحرير لضمان عدم بقاء المصنوعات اليدوية المفقودة.

## 3. إعداد مذكرات الإصدار

يجب أن يقوم كل إصدار بنشر سجل التغيير في عملية تخفيض الأسعار التي تدمر تعديلات CLI وSDK و
تأثير الحوكمة. استخدم القالب `docs/examples/sorafs_release_notes.md`
(نسخة إلى مدير الإصدار الفني الخاص بك وقم بحفظها بالثواني مع التفاصيل
الخرسانة).

الحد الأدنى للمتابعة:- **أبرز النقاط**: مجموعة من الميزات لمستهلكي CLI وSDK.
- **التوافق**: تعديلات صعبة، وترقيات سياسية، والمتطلبات الدنيا
  دي بوابة/عقدة.
- **تصاريح الترقية**: comandos TL;DR لتحديث تبعيات البضائع وإعادة التعيين
  حتمية التركيبات.
- **التحقق**: تجزئات الصيدلة أو المغلفات والمراجعة الإضافية
  تم تنفيذ `ci/check_sorafs_cli_release.sh`.

قم بإرفاق ملاحظات الإصدار المسبق للعلامة (على سبيل المثال، مجموعة إصدار GitHub) و
يحرسون جنبًا إلى جنب مع القطع الأثرية الجيدة من حيث الشكل الحتمي.

## 4. تنفيذ خطافات التحرير

Rode `scripts/release_sorafs_cli.sh` لتشغيل حزمة الاغتيال واستكمالها
التحقق من مرافقة كل إصدار. أو المجمع المجمع أو CLI عندما يكون ذلك ضروريا،
مفتاح `sorafs_cli manifest sign` وإعادة تنفيذ `manifest verify-signature` على الفور
حتى تتمكن من إظهار الكاميرا قبل وضع العلامات. مثال:

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

ديكاس:- تسجيل مدخلات الإصدار (الحمولة، الخطط، الملخصات، التجزئة المتوقعة للرمز المميز)
  لا يوجد مستودع أو تكوين للنشر للحفاظ على إعادة إنتاج البرنامج النصي. يا
  تظهر حزمة التركيبات في `fixtures/sorafs_manifest/ci_sample/` أو تخطيط Canonico.
- قم بتشغيل CI التلقائي على `.github/workflows/sorafs-cli-release.yml`; إلي رودا س
  بوابة الإصدار، والنص، والحزم الرئيسية والأركيفا/القتلة مثل القطع الأثرية
  القيام بسير العمل. الحفاظ على نفس ترتيب الأوامر (بوابة الإصدار -> القتل ->
  التحقق) في أنظمة CI الأخرى حتى يتم دمج سجلات المراجعة مع التجزئات
  جيرادوس.
- مانتينها `manifest.bundle.json`، `manifest.sig`، `manifest.sign.summary.json` ه
  `manifest.verify.summary.json` جونتوس؛ إنها النموذج أو الحزمة المرجعية
  إشعار الحكم.
- عندما تقوم بتحرير التركيبات الكنسي المحدثة، أو نسخ أو تحديث البيان، أو
  خطة القطعة وملخصات نظام التشغيل لـ `fixtures/sorafs_manifest/ci_sample/` (تحديث
  `docs/examples/sorafs_ci_sample/manifest.template.json`) قبل التطوير. المشغلين
  يعتمد المصب على التركيبتين المخصصتين لإعادة إنتاج أو إصدار الحزمة.
- التقاط سجل تنفيذ التحقق من القنوات المحدودة
  `sorafs_cli proof stream` وقم بتحرير الملحق الموجود في الحزمة لتوضيح ذلك
  Salvaguardas de Proof Streaming Continuam Ativas.
- قم بالتسجيل في `--identity-token-audience` الذي تم استخدامه أثناء عملية القتل
  ملاحظات الإصدار؛ رحلة حاكمة أو جمهور مع سياسة فولسيو قبل ذلك
  aprovar a publicacao.استخدم `scripts/sorafs_gateway_self_cert.sh` عندما يتم تحريره بما في ذلك الطرح
دي بوابة. Aponte para o mesmo package de البيان لإثبات التصديق
تتوافق مع المرشحين Artefato:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. ضع علامة على الجمهور

بعد مرور عمليات التحقق والخطافات للتوصل إلى استنتاجات:1. ركب `sorafs_cli --version` و`sorafs_fetch --version` لتأكيد الثنائيات
   Reportam بالعكس.
2. قم بإعداد تكوين الإصدار للإصدار `sorafs_release.toml` (المفضل)
   أو ملف التكوين الآخر الذي تم نشره من خلال مستودع النشر الخاص بك. تخلص من التبعية
   مجموعة متنوعة من البيئة المخصصة؛ قم بتمرير os caminhos لـ CLI com `--config` (ou
   مكافئ) حتى تقوم المدخلات بإصدار الإعلانات الصريحة والمنتجة.
3. قم بالصرخة بقتل العلامة (المفضلة) أو التعليق عليها:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. تحميل Faca dos artefatos (حزم CAR، والبيانات، وملخصات الأدلة، وملاحظات الإصدار،
   مخرجات الشهادة) لتسجيل المشروع التالي لقائمة المراجعة الحكومية
   لا يوجد [دليل النشر](./developer-deployment.md). إذا قمت بإصدار تركيبات جيرو نوفا،
   يمكنك الحسد على مشاركة التركيبات أو تخزين العناصر في السيارة
   ينصح المستمعون بمقارنة الحزمة المنشورة من خلال التحكم في التشفير.
5. إشعار القناة الحاكمة بالروابط الخاصة بالعلامة التي تم حذفها وملاحظات الإصدار والتجزئة
   قم بإنشاء حزمة/بيانات أساسية، واستئنافات أرشيفية من `manifest.sign/verify` e
   مغلفات التصديق. قم بتضمين عنوان URL لمهمة CI (أو ملف السجلات)
   كيو رودو `ci/check_sorafs_cli_release.sh` و `scripts/release_sorafs_cli.sh`. تحقيق
   o تذكرة الحكم حتى يتمكن المدققون من المحاولة لأكل تلك المصنوعات؛
   عند العمل `.github/workflows/sorafs-cli-release.yml`، الإخطارات العامة، الرابطيتم تسجيل التجزئات أثناء تجميع السيرة الذاتية المخصصة.

## 6. إصدار ما بعد البيع

- Garanta que a documentacao apontando para a nova versao (البدء السريع، قوالب CI)
  تم تحديث هذا الأمر أو تأكيد أنه لا يحتاج إلى تعديلات وضرورية.
- سجل المدخلات في خارطة الطريق إذا كانت هناك حاجة للعمل اللاحق (على سبيل المثال، الأعلام
- أرشيف سجلات نظام التشغيل من خلال بوابة الإصدار للاستماع - حراسة نظام التشغيل إلى Lado Dos Artefatos
  assinados.

قم بمتابعة توجيه خط الأنابيب هذا إلى CLI وصناديق SDK والمواد التي تحكمها
في كل ciclo دي الافراج.