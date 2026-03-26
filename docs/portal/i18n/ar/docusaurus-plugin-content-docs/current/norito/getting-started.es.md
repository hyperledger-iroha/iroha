---
lang: ar
direction: rtl
source: docs/portal/docs/norito/getting-started.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Primeros pasos con Norito

يستخدم هذا الدليل السريع التدفق الأدنى لتجميع عقد Kotodama، وفحص الرمز الثانوي Norito الذي تم إنشاؤه، وتنفيذه محليًا وإزالته في عقدة Iroha.

## المتطلبات السابقة

1. قم بتثبيت سلسلة أدوات Rust (1.76 أو أحدث) واستنساخ هذا المستودع.
2. إنشاء أو تنزيل ثنائيات الدعم:
   - `koto_compile` - المترجم Kotodama الذي يصدر الرمز الثانوي IVM/Norito
   - `ivm_run` و `ivm_tool` - أدوات التشغيل والفحص المحلي
   - `iroha_cli` - يتم استخدامه لاستكمال العقود عبر Torii

   يتوقع أن يكون ملف إنشاء المستودع ثنائيًا في `PATH`. يمكنك تنزيل المصنوعات المجمعة مسبقًا أو المجمعة من الكود الناشئ. إذا قمت بتجميع سلسلة الأدوات المحلية، قم بإضافة مساعدي Makefile إلى الثنائيات:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. تأكد من أن العقدة Iroha سيتم تشغيلها عند الانتهاء منها. تفترض الأمثلة السابقة أنه يمكن الوصول إلى Torii عبر عنوان URL الذي تم تكوينه في ملف `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. قم بتجميع عقد Kotodama

يتضمن المستودع عقدًا صغيرًا "hello World" و`examples/hello/hello.ko`. تجميع الرمز الثانوي Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

الخيارات الرئيسية:- `--abi 1` تمت الموافقة على الإصدار ABI 1 (الوحيد المعتمد في لحظة الكتابة).
- `--max-cycles 0` طلب القذف بدون حدود؛ قم بتثبيت رقم إيجابي للحصول على حشوة الدراجات لتجربة الشعور بالصفراء.

## 2. فحص القطعة الأثرية Norito (اختياري)

استخدام `ivm_tool` للتحقق من البيانات والبيانات التعريفية المضمنة:

```sh
ivm_tool inspect target/examples/hello.to
```

Deberias ver la version ABI، الأعلام المؤهلة ونقاط الدخول المصدرة. إنها تسوية سريعة قبل الانهيار.

## 3. تنفيذ العقد محليًا

قم بتشغيل الرمز الثانوي مع `ivm_run` لتأكيد المعاملة بدون عقدة:

```sh
ivm_run target/examples/hello.to --args '{}'
```

المثال `hello` يسجل تحية ويصدر اتصال نظام `SET_ACCOUNT_DETAIL`. يعد التنفيذ المحلي أمرًا مفيدًا أثناء التكرار حول منطق العقد قبل النشر على السلسلة.

## 4. البث عبر `iroha_cli`

عندما تكون راضيًا عن العقد، أرسله بعقدة واحدة باستخدام CLI. توفير حساب المصدر ومفتاح الشركة والملف `.to` أو الحمولة Base64:

```sh
iroha_cli app contracts deploy \
  --authority <katakana-i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

يرسل الأمر حزمة بيان Norito + الرمز الثانوي لـ Torii ويعرض حالة المعاملة الناتجة. تم التأكيد مرة واحدة، يمكن استخدام تجزئة الكود الذي يظهر في الرد لاستعادة البيانات أو قائمة المثيلات:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```## 5. إخراج كونترا Torii

باستخدام الرمز الثانوي المسجل، يمكنك استدعاء تعليمات تشير إلى الكود المخزن مؤقتًا (ص. على سبيل المثال، في منتصف `iroha_cli ledger transaction submit` أو عميل التطبيق الخاص بك). تأكد من أن أذونات الحساب تسمح بالمكالمات المرغوبة (`set_account_detail`، `transfer_asset`، وما إلى ذلك).

## نصائح وحل المشكلات

- Usa `make examples-run` لتجميع النماذج وتنفيذها بخطوة واحدة فقط. قم بكتابة متغيرات الإعداد `KOTO`/`IVM` إذا لم تكن الثنائيات موجودة في `PATH`.
- إذا أعاد `koto_compile` إصدار ABI، تحقق من أن المترجم والعقدة يدعمان ABI v1 (أخرج `koto_compile --abi` دون وسيطات لسرد الدعم).
- El CLI يقبل مفاتيح الشركة السداسية أو Base64. للاختبار، يمكنك استخدام المفاتيح الصادرة بواسطة `iroha_cli tools crypto keypair`.
- بعد إزالة الحمولات Norito، يساعد الأمر الفرعي `ivm_tool disassemble` على التعليمات المرتبطة بالكود Kotodama.

يعكس هذا التدفق الخطوات المستخدمة في CI واختبارات التكامل. للحصول على تحليل أعمق لقواعد Kotodama وخرائط النظام والداخلية لـ Norito، راجع:

-`docs/source/kotodama_grammar.md`
-`docs/source/kotodama_examples.md`
-`norito.md`