---
lang: ar
direction: rtl
source: docs/portal/docs/norito/getting-started.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito كا آغاز

رمز صغير Kotodama للشبكة المحمولة، وهو رمز ثانوي Norito كرمز ثانوي، وهو مقام على الخريطة وIroha لقد تم الإعلان عن جديد من خلال العمل الجاد.

##ضروريات

1. سلسلة أدوات الصدأ (1.76 أو جديدة) إنشاء كامل ونسخ صغير جدًا.
2. معونات البناء أو التنزيل:
   - `koto_compile` - Kotodama المحمول جو IVM/Norito رمز البايت خارج البطاقة
   - `ivm_run` و `ivm_tool` - مقام اجرا ومعاير انترنت
   - `iroha_cli` - Torii تم استخدام ذريعة الشبكة

   تم إعداد تقرير Makefile على بانيرز `PATH` بشكل متوقع. يمكن تنزيل القطع الأثرية من خلال هذه القطعة أو الصورة. إذا كانت سلسلة الأدوات بمثابة مكان لمساعدات Makefile في العمل الجماعي، فهي تساعد في إنشاء نقطة انطلاق:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. فيما يتعلق بالرحلة الملائمة لك، يمكنك استخدام Iroha لكل جديد. لا يوجد أي مثال يفترض أن Torii هو عنوان URL القابل للإرجاع و`iroha_cli` المفضل (`~/.config/iroha/cli.toml`).

## 1.Kotodama لعبة كمبيوتر محمول

الريبو موجود حاليًا في مركز "hello World" `examples/hello/hello.ko`. هذا هو الرمز الثانوي Norito/IVM (`.to`)

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

آمال فليغز:- `--abi 1` الجزء الأول من سلسلة ABI لشبكة ABI (الإصدار الأول من الإصدار الرياضي).
- `--max-cycles 0` لا حدود لاجرا كي درخواست كرتا ہے؛ براهين المعرفة الصفرية عبارة عن حشوة دورة تمتد إلى عدد إيجابي.

## 2.Norito فكرة بديلة (اختاري)

فيما يلي المزيد من المعلومات حول استخدام `ivm_tool`:

```sh
ivm_tool inspect target/examples/hello.to
```

يبدو أن ABI هو الربيع، والأعلام النشطة ونقاط دخول الرياضات المائية تبدو رائعة. إنها مناسبة تمامًا لفحص سلامة العقل بشكل فوري.

## 3. الحوسبة في كل مكان

`ivm_run` هو رمز البايت كود الجديد الذي يقوم بإعادة تشغيل شريط التمرير:

```sh
ivm_run target/examples/hello.to --args '{}'
```

`hello` مثال على بطاقة السلامة و `SET_ACCOUNT_DETAIL` بطاقة syscall . لقد أصبح هذا الوقت ممتعًا الآن على السلسلة شائعًا أحدث الابتكارات التكنولوجية الحديثة.

## 4.`iroha_cli` الإضافة إلى قائمة المحتوى المفضّل

تأكد من استخدام CLI الذي تم إصداره حديثًا قبل النشر. تحتوي هذه السلطة على مفتاح التوقيع وصيغة الحمولة `.to` أو Base64:

```sh
iroha_cli app contracts deploy \
  --authority <katakana-i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

تم إصدار Torii من خلال Norito Manifest + bytecode package التي تم جمعها وإخراجها من شبكة الإنترنت عالية الجودة. ٹرانزیکشن الالتزام مرة واحدة بعد ذلك، الرد على المزيد من بيانات تجزئة الكود التي تمكن من الحصول على نسخة أو مثيلات لاستخدامها في ما يلي:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Torii مفاجأة صادمةبعد تسجيل رمز البايت، قم بإرسال تعليمات التطبيق إلى ملف الشبكة الذي يحتوي على ملف حوالة (مثل `iroha_cli ledger transaction submit` أو تطبيق شبكة الإنترنت) ذریعے). هذه هي الأذونات المطلوبة لمكالمات النظام (`set_account_detail`, `transfer_asset`, إلخ) التي تم السماح بها.

## التجاويز والخرابيوں كحل

- `make examples-run` استخدم مثالًا على استخدام هذه الميزة الرائعة والصغيرة. إذا لم يتم استخدام `PATH` في `KOTO`/`IVM`، فستتجاوز متغيرات البيئة.
- إذا كان `koto_compile` ABI قد بدأ في تحديث برنامج الكمبيوتر المحمول وتحديث ABI v1 (`koto_compile --abi`) فهذا هو السبب وراء ذلك. الرياضة دکھے).
- مفاتيح التوقيع CLI hex أو Base64 قبول کرتا ہے۔ تم استخدام مفتاح الإدخال `iroha_cli tools crypto keypair` مرة أخرى.
- الحمولات Norito التي تم إنشاؤها بواسطة `ivm_tool disassemble` تم إنشاؤها بواسطة حمولة Kotodama.

يتم استخدام فلو CI والإنترنت في المراحل التالية من الشرح. Kotodama قواعد البيانات، وتعيينات syscall وNorito الداخلية تزيد من التفاصيل:

-`docs/source/kotodama_grammar.md`
-`docs/source/kotodama_examples.md`
-`norito.md`