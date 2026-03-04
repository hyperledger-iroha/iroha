---
lang: fr
direction: ltr
source: docs/portal/docs/norito/getting-started.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# بدء استخدام Norito

Vous pouvez trouver le code d'accès Kotodama avec le bytecode Norito et le bytecode Norito. محليا، ثم نشره على عقدة Iroha.

## المتطلبات المسبقة

1. Rust (1,76 par jour) est utilisé par Rust.
2. ابن او نزّل الثنائيات الداعمة:
   - `koto_compile` - Utiliser Kotodama pour le bytecode IVM/Norito
   - `ivm_run` et `ivm_tool`
   - `iroha_cli` - يستخدم لنشر العقود عبر Torii

   Makefile est un fichier Makefile `PATH`. يمكنك تنزيل artefacts جاهزة او بناؤها من المصدر. La chaîne d'outils est associée à un Makefile :

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Utilisez le Iroha pour installer le système d'exploitation. Utilisez l'URL Torii pour accéder à l'URL `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Rechercher Kotodama

Il s'agit d'un "hello world" `examples/hello/hello.ko`. Il s'agit du bytecode Norito/IVM (`.to`) :

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

اهم الاعلام:

- `--abi 1` est compatible avec ABI 1 (النسخة الوحيدة المدعومة وقت الكتابة).
- `--max-cycles 0` يطلب تنفيذا غير محدود؛ ضع رقما موجبا لحد padding الدورات لاجل اثباتات المعرفة الصفرية.

## 2. فحص اثر Norito (اختياري)

استخدم `ivm_tool` pour les personnes âgées et les personnes handicapées :```sh
ivm_tool inspect target/examples/hello.to
```

ينبغي ان ترى نسخة ABI والاعلام المفعلة ونقاط الدخول المصدرة. هذا فحص سريع قبل النشر.

## 3. تشغيل العقد محليا

Le bytecode `ivm_run` est utilisé pour la connexion :

```sh
ivm_run target/examples/hello.to --args '{}'
```

Utilisez `hello` pour utiliser l'appel système `SET_ACCOUNT_DETAIL`. التشغيل المحلي مفيد اثناء تكرار منطق العقد قبل نشره على السلسلة.

## 4. النشر عبر `iroha_cli`

Il s'agit d'une option pour la CLI. Vous pouvez utiliser la charge utile `.to` pour Base64 :

```sh
iroha_cli app contracts deploy \
  --authority ih58... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

Il s'agit d'un bundle contenant le manifeste Norito + le bytecode et Torii et le code d'octet. Dans le cas d'un hachage, le hachage se manifeste et les instances sont :

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. التشغيل عبر Torii

مع تسجيل bytecode يمكنك استدعاؤه عبر ارسال تعليمات تشير الى الكود المخزن (مثلا عبر `iroha_cli ledger transaction submit` او عميل التطبيق). Vous pouvez utiliser les appels système pour les appels système (`set_account_detail`, `transfer_asset`, ici).

## نصائح واستكشاف الاعطال- استخدم `make examples-run` لتجميع وتنفيذ الامثلة دفعة واحدة. قم بتجاوز متغيرات البيئة `KOTO`/`IVM` اذا لم تكن الثنائيات على `PATH`.
- اذا رفض `koto_compile` نسخة ABI, تحقق من ان المترجم والعقدة يستهدفان ABI v1 (شغّل `koto_compile --abi` بدون معاملات لعرض الدعم).
- La CLI est une version hexadécimale et Base64. للاختبار يمكنك استخدام المفاتيح الصادرة من `iroha_cli tools crypto keypair`.
- Utilisez les charges utiles Norito, puis `ivm_tool disassemble` pour les charges utiles Kotodama.

يعكس هذا التدفق الخطوات المستخدمة في CI واختبارات التكامل. Pour utiliser Kotodama et les appels système, utilisez Norito:

-`docs/source/kotodama_grammar.md`
-`docs/source/kotodama_examples.md`
-`norito.md`