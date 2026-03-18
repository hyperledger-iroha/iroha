---
lang: ar
direction: rtl
source: docs/portal/docs/norito/getting-started.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# الجائزة الرئيسية Norito

يقدم هذا الدليل السريع الحد الأدنى من سير العمل من أجل المترجم بموجب عقد Kotodama، وفحص الكود الثانوي Norito، والتنفيذ المحلي، والموزع على رقم Iroha.

## المتطلبات الأساسية

1. قم بتثبيت Toolchain Rust (1.76 أو أكثر) واسترد هذا المستودع.
2. قم بإنشاء أو تنزيل ملفات الدعم الثنائية :
   - `koto_compile` - المترجم Kotodama الذي يحرر الرمز الثانوي IVM/Norito
   - `ivm_run` و`ivm_tool` - أدوات التنفيذ المحلية والفحص
   - `iroha_cli` - استخدم لنشر العقود عبر Torii

   يحضر ملف Makefile du depot هذه الثنائيات في `PATH`. يمكنك تنزيل العناصر المترجمة مسبقًا أو إنشاءها من المصادر. إذا قمت بتجميع أداة Toolchain المحلية، فقم بتوجيه مساعدي Makefile إلى الثنائيات:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. تأكد من أن Iroha يتم تنفيذه عند الانتهاء من مرحلة النشر. تفترض الأمثلة أنه يمكن الوصول إلى Torii من خلال عنوان URL الذي تم تكوينه في ملف التعريف الخاص بك `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. عقد مترجم Kotodama

يوفر المستودع عقدًا بسيطًا "hello World" في `examples/hello/hello.ko`. ترجمة الرمز الثانوي Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

الخيارات:- `--abi 1` قفل العقد على الإصدار ABI 1 (الدعم الوحيد في لحظة التنقيح).
- `--max-cycles 0` يطلب تنفيذًا بلا حدود؛ قم بتثبيت رقم إيجابي لتكوين حشوة الدورات من أجل معرفة مستوى الصفر.

## 2. Inspecter l'artefact Norito (اختياري)

استخدم `ivm_tool` للتحقق من البيانات الشخصية والبيانات التعريفية المتكاملة:

```sh
ivm_tool inspect target/examples/hello.to
```

يجب عليك الاطلاع على إصدار ABI والأعلام النشطة ونقاط الإدخال المصدرة. إنها أداة تحكم سريعة قبل النشر.

## 3. تنفيذ العقد المحلي

قم بتنفيذ الرمز الثانوي باستخدام `ivm_run` لتأكيد السلوك دون لمسه:

```sh
ivm_run target/examples/hello.to --args '{}'
```

على سبيل المثال `hello` سجل تحية وقم بإجراء مكالمة نظام `SET_ACCOUNT_DETAIL`. يعد التنفيذ المحلي مفيدًا لأنك تتصفح منطق العقد قبل النشر عبر السلسلة.

## 4. الناشر عبر `iroha_cli`

عندما تكون راضيًا عن العقد، قم بنشره عبر CLI. قم بتزويد حساب تلقائي بمفتاح التوقيع وملف `.to` كحمولة Base64 :

```sh
iroha_cli app contracts deploy \
  --authority i105... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

يتم الأمر للحصول على بيان حزمة Norito + الرمز الثانوي عبر Torii وعرض حالة المعاملة الناتجة. بعد إتمام المعاملة الصحيحة، يمكن أن تؤدي تجزئة الكود المعروضة في الرد إلى استعادة البيانات أو سرد المثيلات:```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. المنفذ عبر Torii

مع تسجيل الكود الثانوي، يمكنك استدعاء تعليمات تشير إلى مخزون الكود (على سبيل المثال، عبر `iroha_cli ledger transaction submit` أو تطبيق العميل الخاص بك). تأكد من أن أذونات الحساب تؤذن للمكالمات المطلوبة (`set_account_detail`، `transfer_asset`، وما إلى ذلك).

## Conseils et depannage

- استخدم `make examples-run` لتجميع وتنفيذ الأمثلة في أمر واحد. قم بشحن متغيرات البيئة `KOTO`/`IVM` إذا لم تكن الثنائيات موجودة في `PATH`.
- إذا رفض `koto_compile` إصدار ABI، وتحقق من المترجم والمفتاح بين ABI v1 (نفذ `koto_compile --abi` بدون وسيطات لإدراج الدعم).
- يقبل CLI مفاتيح التوقيع السداسي أو Base64. لإجراء الاختبارات، يمكنك استخدام العناصر المنبعثة وفقًا لـ `iroha_cli tools crypto keypair`.
- عند تصحيح الحمولات Norito، يساعد الأمر الفرعي `ivm_tool disassemble` في ربط التعليمات مع المصدر Kotodama.

يعكس هذا التدفق الخطوات المستخدمة في CI وفي اختبارات التكامل. لإجراء تحليل أكثر عمقًا للقواعد Kotodama، تعيينات النظام والداخلية Norito، انظر:

-`docs/source/kotodama_grammar.md`
-`docs/source/kotodama_examples.md`
-`norito.md`