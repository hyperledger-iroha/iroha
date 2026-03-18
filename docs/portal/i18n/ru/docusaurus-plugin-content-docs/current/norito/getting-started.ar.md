---
lang: ru
direction: ltr
source: docs/portal/docs/norito/getting-started.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# بدء استخدام Norito

Для получения дополнительной информации обратитесь к байт-коду Kotodama, Norito находится под управлением Iroha.

## المتطلبات المسبقة

1. Добавлена версия Rust (1.76 в версии) и добавлена версия.
2. В случае необходимости:
   - `koto_compile` - مترجم Kotodama Загрузка байт-кода IVM/Norito
   - `ivm_run` и `ivm_tool` - ادوات التشغيل المحلي والفحص
   - `iroha_cli` - يستخدم لنشر العقود عبر Torii

   Создан Makefile в программе, созданной в формате `PATH`. Он представляет собой артефакты, сделанные в Сан-Франциско в Нью-Йорке. Для создания набора инструментов необходимо создать файл Makefile:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Установите флажок Iroha и установите флажок для проверки. Создайте ссылку на Torii для получения URL-адреса по ссылке. `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Зарегистрируйтесь Kotodama

На экране появится сообщение «Привет, мир» на `examples/hello/hello.ko`. В соответствии с байт-кодом Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Уважаемый:

- `--abi 1` используется для работы с ABI 1 (с помощью функции "открыть окно").
- `--max-cycles 0` يطلب تنفيذا غير محدود؛ Вы можете использовать прокладку, чтобы сохранить форму.

## 2. فحص اثر Norito (اختياري)

Код `ivm_tool` отображается в разделе "Настройка":

```sh
ivm_tool inspect target/examples/hello.to
```

Он был убит Эйби и его сыном в убийстве. Он был убит в субботу.

## 3. تشغيل العقد محليا

Байт-код `ivm_run` доступен для просмотра в следующем формате:

```sh
ivm_run target/examples/hello.to --args '{}'
```

Код `hello` вызывает системный вызов `SET_ACCOUNT_DETAIL`. В 2007 году он был убит в 1980-х годах.

## 4. Установите флажок `iroha_cli`.

Он сказал, что работает с CLI. Для создания полезной нагрузки используется Base64:

```sh
iroha_cli app contracts deploy \
  --authority i105... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

В пакете используется манифест Norito + байт-код Torii, который необходимо выполнить. Вы можете использовать хеш-код, который будет использоваться для проверки хеш-файла, когда он проявляется. Примеры:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Дополнительная информация Torii

Для получения байт-кода необходимо, чтобы пользователь использовал байт-код عبر `iroha_cli ledger transaction submit` в عميل التطبيق). При вызове системных вызовов выполняются системные вызовы (`set_account_detail`, `transfer_asset`, ).

## نصائح واستكشاف الاعطال

- Установите `make examples-run` для получения дополнительной информации. Для этого необходимо установить `KOTO`/`IVM`. `PATH`.
- Для `koto_compile` используется ABI, который используется в программе ABI v1 (شغّل) `koto_compile --abi` بدون معاملات لعرض الدعم).
- В CLI используется шестнадцатеричное значение в Base64. Установите флажок `iroha_cli tools crypto keypair`.
- Загрузка полезных данных Norito, а также `ivm_tool disassemble` для проверки работоспособности. Kotodama.Он был отправлен в Центр по связям с общественностью (CI). Чтобы вызвать Kotodama, выполните системные вызовы Norito, выполните следующие действия:

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`