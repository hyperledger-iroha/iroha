---
lang: ar
direction: rtl
source: docs/portal/docs/norito/getting-started.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Primeiros passos com Norito

هذا هو الدليل السريع للتدفق الأدنى لتجميع عقد Kotodama، والتحقق من الكود الثانوي Norito، وتنفيذه محليًا ونشره في عقدة Iroha.

## المتطلبات المسبقة

1. قم بتثبيت سلسلة أدوات Rust (1.76 أو أحدث) وقم بالخروج من المستودع.
2. قم بتجميع أو تخزين ثنائيات الدعم:
   - `koto_compile` - المترجم Kotodama الذي يصدر الرمز الثانوي IVM/Norito
   - `ivm_run` و `ivm_tool` - أدوات التنفيذ المحلية والفحص
   - `iroha_cli` - يُستخدم لنشر العقود عبر Torii

   يجب أن يكون ملف Makefile الخاص بالمستودع ثنائيًا رقم `PATH`. يمكنك إضافة قطع فنية مجمعة مسبقًا أو تجميعها من خط مشفر. إذا قمت بتجميع سلسلة أدوات محلية، فسيقوم المساعدون بعمل Makefile للثنائيات:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. تأكد من أن العقدة Iroha ستتم عند بدء النشر. تفترض الأمثلة أن Torii يمكن الوصول إليه على عنوان URL الذي تم تكوينه وليس الملف `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. قم بتجميع العقد Kotodama

يحتوي المستودع على الحد الأدنى من عبارة "hello World" في `examples/hello/hello.ko`. ترجمة للرمز الثانوي Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

أعلام شاف:- `--abi 1` تم إصلاح العقد على عكس ABI 1 (دعم فريد في الوقت الحالي).
- `--max-cycles 0` طلب التنفيذ بدون حدود؛ حدد رقمًا إيجابيًا للحد من حشوة الحلقات أو حشوتها لإثبات الإدراك صفرًا.

## 2. فحص القطعة الفنية Norito (اختياري)

استخدم `ivm_tool` للتحقق من صحة البيانات والتوصيفات المضمنة:

```sh
ivm_tool inspect target/examples/hello.to
```

يمكنك رؤية واجهة ABI العكسية، والأعلام المؤهلة، ونقاط الدخول المصدرة. يتم نشر ما قبله بسرعة.

## 3. قم بتنفيذ العقد محليًا

قم بتنفيذ الرمز الثانوي عبر `ivm_run` لتأكيد المعاملة دون عقدة:

```sh
ivm_run target/examples/hello.to --args '{}'
```

على سبيل المثال `hello`، قم بتسجيل اتصال وإصدار اتصال `SET_ACCOUNT_DETAIL`. قم بالتنفيذ محليًا واستخدامه أثناء تكرار الأمر في منطق العقد قبل نشره على السلسلة.

## 4. نشر الواجهة عبر `iroha_cli`

عندما تكون راضيًا عن العقد، قم بتنفيذ العقدة باستخدام سطر الأوامر. للحصول على حساب المصدر، يجب أن يتم اغتياله وملف `.to` أو الحمولة Base64:

```sh
iroha_cli app contracts deploy \
  --authority <katakana-i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

قم بإرسال حزمة البيان Norito + الرمز الثانوي عبر Torii وإظهار حالة التحويل الناتجة. بعد تأكيد المعاملة، يمكن استخدام تجزئة الكود المعروض في الرد لاستعادة البيانات أو سرد المثيلات:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. نفذ كونترا Toriiمن خلال تسجيل الرمز الثانوي، يمكنك استدعاء تعليمات تشير إلى الكود المخزن (على سبيل المثال، عبر `iroha_cli ledger transaction submit` أو عميل التطبيق الخاص بك). تأكد من أن الأذونات تسمح لك بإجراء مكالمات النظام (`set_account_detail`، `transfer_asset`، وما إلى ذلك).

## عرض وحل المشكلات

- استخدم `make examples-run` لتجميع الأمثلة وتنفيذها مرة واحدة. استبدل البيئة المحيطة المتغيرة `KOTO`/`IVM` في حالة عدم وجود ملفات ثنائية في `PATH`.
- إذا تم إعادة `koto_compile` إلى عكس ABI، تحقق من المترجم والعقدة miram ABI v1 (استخدم `koto_compile --abi` بدون وسيطات لإدراج أو دعم).
- O CLI aceita chaves de assinatura em hex ou Base64. بالنسبة للخصيتين، يمكن استخدام الأصوات الصادرة عن `iroha_cli tools crypto keypair`.
- لإزالة الحمولات Norito، أو الأمر الفرعي `ivm_tool disassemble` يساعد على الإرشادات المرتبطة برمز الخط Kotodama.

يخصص هذا التدفق الخطوات المستخدمة في CI وخصى التكامل. لدمج المزيد من القواعد النحوية في Kotodama، لدينا خرائط النظام والداخلية في Norito، كما يلي:

-`docs/source/kotodama_grammar.md`
-`docs/source/kotodama_examples.md`
-`norito.md`