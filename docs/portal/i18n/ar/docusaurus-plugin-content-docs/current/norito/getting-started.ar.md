---
lang: ar
direction: rtl
source: docs/portal/docs/norito/getting-started.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#بدء استخدام Norito

يعرض هذا الدليل السريع سير العمل الادنى لجميع العقد Kotodama، وفحص الرمز الثانوي Norito الأصلي، وتشغيله محليا، ثم نشره على عقدة Iroha.

##المتطلبات المسبقة

1. ثبّت سلسلة ادوات Rust (1.76 او احدث) نسخ هذا المستودع.
2. ابن او نزّل البيدوات الإضافية:
   - `koto_compile` - مترجم Kotodama الذي يصدر bytecode IVM/Norito
   - `ivm_run` و `ivm_tool` - ادوات التشغيل المحلية والفحص
   - `iroha_cli` - يستخدم العقود عبر Torii

   لتتمكن من إنشاء ملف في مستودع هذه الثنائيات ضمن `PATH`. يمكنك تنزيل القطع الأثرية الجاهزة او الباخرة با با من المصدر. اذا قمت باستخدام سلسلة الأدوات المحلية فاشر الى ثنائيات في مساعدات Makefile:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. تأكد من ان عقدة Iroha تعمل عند الوصول إلى خطوة النشر. تفترض الامثلة ادناه ان Torii متاحة على عنوان URL المهيأ في ملف تعريف `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. عقد تجميع Kotodama

تتضمن المستودع العقدي البسيط "hello World" في `examples/hello/hello.ko`. قم بتجميعه الى bytecode Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

اهم الاعلام:

- `--abi 1` يثبت العقد على نسخة ABI 1 (النسخة المدعومة فقط وقت الكتابة).
- `--max-cycles 0` يطلب مجموعة غير محددة؛ ضع رقما موجبا لذلك حشوة المدربين لاجل اثباتات المعرفة الصفرية.

## 2.فحص الاشعاع Norito (اختياري)

استخدم `ivm_tool` من حيث الرأس والبيانات الأصلية المضمنة:```sh
ivm_tool inspect target/examples/hello.to
```

ينبغي أن ترى نسخة والاعلام ABIالمفعلة ونقاط الدخول المصدرة. هذا فحص سريع قبل النشر.

## 3. تشغيل العقدة المحلية

ينفذ bytecode عبر `ivm_run` لتتعلم السلوك دون لمس العقدة:

```sh
ivm_run target/examples/hello.to --args '{}'
```

مثال `hello` تسجيل تسجيل ويصدر syscall `SET_ACCOUNT_DETAIL`. العمل الموضعي مفيد لعقدة العقدة قبل نشره على العقدة.

## 4. النشر عبر `iroha_cli`

عندما تكون راضيًا عن العقد، تنشره على عقدة باستخدام CLI. وفر رقم صلاحية ومفتاح توقيعه وملف `.to` او payload بصيغة Base64:

```sh
iroha_cli app contracts deploy \
  --authority i105... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

أرسل الأمر حزمة من المانيفست Norito + bytecode عبر Torii ويطبع الحالة الطبيعية. بعد الهدف المحدد يمكن استخدام تجزئة الكود المعروضات في لاسترجاع البيانات أو سرد الحالات:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. التشغيل عبر Torii

مع تسجيل الرمز الثانوي، يمكنك التأكد من إرسال التعليمات إلى التعليمات البرمجية المخزنة (مثلا عبر `iroha_cli ledger transaction submit` أو عميل التطبيق). تأكد من ان صلاحيات الحساب تسمح بالـ syscalls الأساسية (`set_account_detail`, `transfer_asset`, الخ).

##نصائح واستكشاف الاعطال- استخدم `make examples-run` لتجميع الامثلة خطوة واحدة. قم بتجاوز متغيرات البيئة `KOTO`/`IVM` اذا لم تكن جديدة على `PATH`.
- إذا رفض `koto_compile` نسخة ABI، تحقق من ان المترجم والعقدة يستهدفان ABI v1 (شغّل `koto_compile --abi` بدون مقترحات لتقديم الدعم).
- يقبل مفاتيح CLI بصيغة hex او Base64. للاختبار يمكنك استخدام المدير من `iroha_cli tools crypto keypair`.
- عند تصحيح الحمولات Norito، يساعد امرر `ivm_tool disassemble` على ربط التعليمات بمصدر Kotodama.

تعكس هذه الخطوات المعقدة في CI واختبارات تكامل. للمزيد حول متطلبات Kotodama وربط syscalls وداخل Norito، راجع:

-`docs/source/kotodama_grammar.md`
-`docs/source/kotodama_examples.md`
-`norito.md`