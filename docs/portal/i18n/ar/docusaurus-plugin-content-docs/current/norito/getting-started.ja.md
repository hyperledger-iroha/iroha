---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/norito/getting-started.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 20bddba19df7862b60b037871122ede0db8e5306ea301784459b574828d8d4ba
source_last_modified: "2025-11-14T04:43:20.834654+00:00"
translation_last_reviewed: 2026-01-30
---

# بدء استخدام Norito

يعرض هذا الدليل السريع سير العمل الادنى لتجميع عقد Kotodama، وفحص bytecode Norito الناتج، وتشغيله محليا، ثم نشره على عقدة Iroha.

## المتطلبات المسبقة

1. ثبّت سلسلة ادوات Rust (1.76 او احدث) واستنسخ هذا المستودع.
2. ابن او نزّل الثنائيات الداعمة:
   - `koto_compile` - مترجم Kotodama الذي يصدر bytecode IVM/Norito
   - `ivm_run` و `ivm_tool` - ادوات التشغيل المحلي والفحص
   - `iroha_cli` - يستخدم لنشر العقود عبر Torii

   يتوقع Makefile في المستودع هذه الثنائيات ضمن `PATH`. يمكنك تنزيل artefacts جاهزة او بناؤها من المصدر. اذا قمت ببناء toolchain محليا فاشر الى الثنائيات في مساعدات Makefile:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. تاكد من ان عقدة Iroha تعمل عند الوصول الى خطوة النشر. تفترض الامثلة ادناه ان Torii متاح على عنوان URL المهيأ في ملف تعريف `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. تجميع عقد Kotodama

يشمل المستودع عقدا بسيطا "hello world" في `examples/hello/hello.ko`. قم بتجميعه الى bytecode Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

اهم الاعلام:

- `--abi 1` يثبت العقد على نسخة ABI 1 (النسخة الوحيدة المدعومة وقت الكتابة).
- `--max-cycles 0` يطلب تنفيذا غير محدود؛ ضع رقما موجبا لحد padding الدورات لاجل اثباتات المعرفة الصفرية.

## 2. فحص اثر Norito (اختياري)

استخدم `ivm_tool` للتحقق من الرأس والبيانات الوصفية المضمنة:

```sh
ivm_tool inspect target/examples/hello.to
```

ينبغي ان ترى نسخة ABI والاعلام المفعلة ونقاط الدخول المصدرة. هذا فحص سريع قبل النشر.

## 3. تشغيل العقد محليا

نفذ bytecode عبر `ivm_run` لتاكيد السلوك دون لمس العقدة:

```sh
ivm_run target/examples/hello.to --args '{}'
```

مثال `hello` يسجل تحية ويصدر syscall `SET_ACCOUNT_DETAIL`. التشغيل المحلي مفيد اثناء تكرار منطق العقد قبل نشره على السلسلة.

## 4. النشر عبر `iroha_cli`

عندما تكون راضيا عن العقد، انشره على عقدة باستخدام CLI. وفر حساب صلاحية ومفتاح توقيعه واما ملف `.to` او payload بصيغة Base64:

```sh
iroha_cli app contracts deploy \
  --authority ih58... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

يرسل الامر bundle من manifest Norito + bytecode عبر Torii ويطبع حالة المعاملة الناتجة. بعد التزام المعاملة يمكن استخدام hash الكود المعروض في الاستجابة لاسترجاع manifests او سرد instances:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. التشغيل عبر Torii

مع تسجيل bytecode يمكنك استدعاؤه عبر ارسال تعليمات تشير الى الكود المخزن (مثلا عبر `iroha_cli ledger transaction submit` او عميل التطبيق). تاكد من ان صلاحيات الحساب تسمح بالـ syscalls المطلوبة (`set_account_detail`, `transfer_asset`, الخ).

## نصائح واستكشاف الاعطال

- استخدم `make examples-run` لتجميع وتنفيذ الامثلة دفعة واحدة. قم بتجاوز متغيرات البيئة `KOTO`/`IVM` اذا لم تكن الثنائيات على `PATH`.
- اذا رفض `koto_compile` نسخة ABI، تحقق من ان المترجم والعقدة يستهدفان ABI v1 (شغّل `koto_compile --abi` بدون معاملات لعرض الدعم).
- يقبل CLI مفاتيح توقيع بصيغة hex او Base64. للاختبار يمكنك استخدام المفاتيح الصادرة من `iroha_cli tools crypto keypair`.
- عند تصحيح payloads Norito، يساعد امر `ivm_tool disassemble` على ربط التعليمات بمصدر Kotodama.

يعكس هذا التدفق الخطوات المستخدمة في CI واختبارات التكامل. للمزيد حول قواعد Kotodama وربط syscalls وداخل Norito، راجع:

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`
