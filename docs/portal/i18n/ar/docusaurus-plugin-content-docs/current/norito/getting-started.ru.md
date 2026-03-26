---
lang: ar
direction: rtl
source: docs/portal/docs/norito/getting-started.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# بداية العمل مع Norito

يُظهر هذا الجزء الصغير الحد الأدنى من عملية تجميع العقد Kotodama، للتحقق من كود البيتوكودا البيئي Norito، الغلق والنشر المحلي على Iroha.

## تريبوفانيا

1. قم بتثبيت سلسلة أدوات Rust (1.76 أو جديدة) وقم باستنساخ هذا المستودع.
2. تعلم أو قم بتنزيل الدروس التعليمية الرائعة:
   - `koto_compile` - المترجم Kotodama، الذي أنشأ كود البيتكود IVM/Norito
   - `ivm_run` و `ivm_tool` - أدوات الفحص والفحص المحلية
   - `iroha_cli` - يستخدم لعقد النشر من خلال Torii

   يُضيف مستودع Makefile هذه الثنائيات إلى `PATH`. يمكنك تنزيل هذه القطع الأثرية أو اكتشافها من المصادر. إذا قمت بتجميع سلسلة الأدوات محليًا، فاطلب من Makefile المساعدة في الدخول إلى البرنامج التعليمي:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. تأكد من أن Iroha قد تم إيقافه بعد لحظة النشر. تقترح الأمثلة أنه يمكن الوصول إلى Torii عبر عنوان URL من ملف التعريف `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. تجميع العقد Kotodama

يوجد في المستودعات عقد صغير "hello World" في `examples/hello/hello.ko`. قم بتجميعها في رمز بايت Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

أعلام رئيسية:- `--abi 1` عقد التصديق على نسخة ABI 1 (نسخة ملحقة واحدة في لحظة كتابة النص).
- `--max-cycles 0` يتم إغلاقه بالتشغيل غير المضمون؛ قم بتثبيت أداة مفيدة لتتمكن من تقليل حشوة الحشو لتوزيع المعرفة الصفرية.

## 2. التحقق من القطعة الأثرية Norito (اختياري)

استخدم `ivm_tool` للتحقق من الضمان والتحويلات القوية:

```sh
ivm_tool inspect target/examples/hello.to
```

ستشاهد إصدار ABI والأعلام المتضمنة ونقاط الدخول المصدرة. هذا هو التحقق من سلامة العقل قبل النشر.

## 3. إبرام العقد محليًا

قم بتثبيت رمز البنك من خلال `ivm_run` لتأكيد التحقق دون أي ضرر:

```sh
ivm_run target/examples/hello.to --args '{}'
```

المثال التوضيحي `hello` أدخل الخصوصية في السجل وقم باختيار syscall `SET_ACCOUNT_DETAIL`. عقد محلي مفيد عند تكرار العقد المنطقي للنشر على السلسلة.

## 4. انشر عبر `iroha_cli`

عندما تنتهي من العقد، قم باستغلاله من خلال CLI. قم بتمكين مفوض الحساب، ورسالته الرئيسية، وملفه `.to`، وحمولة Base64:

```sh
iroha_cli app contracts deploy \
  --authority <i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

قم بإدارة بيان الحزمة Norito + بايتكود من خلال Torii وتتبع حالة المعاملات. بعد إرسال اللجنة للإجابة على هذا الرمز، يمكن استخدامها للحصول على البيان أو حالة السجل:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. اتصل عبر Toriiبعد تسجيل رمز البيتكودا، يمكنك اكتشاف نفسك، وتنفيذ التعليمات التي تستخدم الكود المحلي (على سبيل المثال، من خلال `iroha_cli ledger transaction submit` أو تطبيقات العميل الخاص بك). تأكد من أن الحساب الصحيح يقوم بإعادة مكالمات النظام الجديدة (`set_account_detail`، `transfer_asset` وما إلى ذلك).

## مشكلة النصائح والتجديد

- استخدم `make examples-run` للتعرف على التمهيدي الأول واستخدامه. قم بإعادة اقتراح الدعم المؤقت `KOTO`/`IVM`، إذا لم يتم إدخال البرامج التعليمية في `PATH`.
- إذا قمت بإلغاء استنساخ إصدار ABI `koto_compile`، فتأكد من المترجم واستخدام ABI v1 (أغلق `koto_compile --abi` بدون الحجج لمشاهدة العرض).
- CLI يبدأ المفاتيح في السداسي أو Base64. يمكن استخدام المفاتيح المميزة `iroha_cli tools crypto keypair` للاختبار.
- عند نقل الحمولات النافعة Norito، يتم استخدام الأمر الزائد `ivm_tool disassemble`، والتي يمكن أن توفر لك التعليمات اللازمة Kotodama.

هذا الطريق يجذب الأشياء التي يتم استخدامها في CI واختبارات التكامل. لمزيد من التعلم النحوي Kotodama وتخطيط مكالمات النظام والتشغيل Norito sms.:

-`docs/source/kotodama_grammar.md`
-`docs/source/kotodama_examples.md`
-`norito.md`