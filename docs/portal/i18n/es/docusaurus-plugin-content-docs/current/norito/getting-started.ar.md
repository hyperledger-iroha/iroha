---
lang: es
direction: ltr
source: docs/portal/docs/norito/getting-started.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# بدء استخدام Norito

El código de bytes Norito es Kotodama y el código de bytes Norito. Para obtener más información, consulte Iroha.

## المتطلبات المسبقة

1. ثبّت سلسلة ادوات Rust (1.76 او احدث) واستنسخ هذا المستودع.
2. ابن او نزّل الثنائيات الداعمة:
   - `koto_compile` - Archivo Kotodama Código de bytes IVM/Norito
   - `ivm_run` y `ivm_tool` - Otros productos y servicios
   - `iroha_cli` - يستخدم لنشر العقود عبر Torii

   Instale Makefile en el archivo `PATH`. يمكنك تنزيل artefactos جاهزة او بناؤها من المصدر. Esta es la cadena de herramientas que contiene los archivos Makefile:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Coloque el botón Iroha en el lugar correspondiente. Utilice la URL Torii para acceder a la URL `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. تجميع عقد Kotodama

يشمل المستودع عقدا بسيطا "hola mundo" en `examples/hello/hello.ko`. Para obtener el código de bytes Norito/IVM (`.to`):

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

Utilice `ivm_tool` para obtener información sobre el producto y el producto:```sh
ivm_tool inspect target/examples/hello.to
```

ينبغي ان ترى نسخة ABI y المفعلة ونقاط الدخول المصدرة. هذا فحص سريع قبل النشر.

## 3. تشغيل العقد محليا

Este código de bytes es `ivm_run` según el código de bytes:

```sh
ivm_run target/examples/hello.to --args '{}'
```

Aquí está `hello` y syscall `SET_ACCOUNT_DETAIL`. التشغيل المحلي مفيد اثناء تكرار منطق العقد قبل نشره على السلسلة.

## 4. النشر عبر `iroha_cli`

Utilice el CLI para acceder a la CLI. Aquí está la configuración y la carga útil de Base64:

```sh
iroha_cli app contracts deploy \
  --authority i105... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

Hay un paquete con el manifiesto Norito + código de bytes con Torii y un código de barras. بعد التزام المعاملة يمكن استخدام hash الكود المعروض في الاستجابة لاسترجاع manifiestos y siguientes instancias:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. التشغيل عبر Torii

Este código de bytes es un código de bytes que contiene varios códigos de barras (entre ellos `iroha_cli ledger transaction submit` y otros). تاكد من ان صلاحيات الحساب تسمح بالـ syscalls المطلوبة (`set_account_detail`, `transfer_asset`, الخ).

## نصائح واستكشاف الاعطال- Utilice `make examples-run` para conectar y desconectar. قم بتجاوز متغيرات البيئة `KOTO`/`IVM` اذا لم تكن الثنائيات على `PATH`.
- La versión `koto_compile` de ABI incluye la versión `koto_compile --abi` de ABI v1 (la versión `koto_compile --abi`) لعرض الدعم).
- Haga clic en CLI para usar hexadecimal y Base64. Para obtener más información, consulte la página `iroha_cli tools crypto keypair`.
- Utilice las cargas útiles Norito, utilice `ivm_tool disassemble` y utilice Kotodama.

يعكس هذا التدفق الخطوات المستخدمة في CI واختبارات التكامل. Para obtener información sobre Kotodama y syscalls y Norito, escriba:

-`docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`