---
lang: ru
direction: ltr
source: docs/portal/docs/norito/getting-started.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito کا آغاز

Введите Kotodama в качестве байт-кода Norito. Если вы хотите использовать Iroha, вы можете использовать Если вы хотите, чтобы это произошло,

## ضروریات

1. Набор инструментов Rust (версия 1.76)
2. Если вам нужно что-то сделать, вы можете:
   - `koto_compile` - Kotodama کمپائلر جو IVM/Norito байт-код خارج کرتا ہے
   - `ivm_run` اور `ivm_tool` - مقامی اجرا اور معائنہ یوٹیلٹیز
   - `iroha_cli` - Torii کے ذریعے کنٹریکٹ ڈپلائے کرنے کے لئے استعمال ہوتا ہے

   Создайте Makefile или создайте `PATH`, чтобы создать файл Makefile. آپ پری بلٹ артефакты ڈاؤن لوڈ کر سکتے ہیں یا سورس سے بنا سکتے ہیں۔ Используйте инструментальную цепочку, которая поможет вам использовать помощники Makefile, которые вы хотите использовать. Варианты:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Если вы хотите использовать Iroha для Iroha, выберите رہا ہو۔ Если вы хотите использовать Torii для URL-адреса, нажмите здесь. `iroha_cli` پروفائل (`~/.config/iroha/cli.toml`) میں سیٹ ہے۔

## 1. Kotodama کنٹریکٹ کمپائل کریں

ریپو میں کم سے کم کنٹریکٹ `examples/hello/hello.ko` میں موجود ہے۔ Байт-код Norito/IVM (`.to`)

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Ответ:

- `--abi 1` کنٹریکٹ کو کو ورژن 1 پر لاک کرتا ہے (تحریر کے وقت واحد سپورٹڈ ورژن).
- `--max-cycles 0` لامحدود اجرا کی درخواست کرتا ہے؛ Доказательства с нулевым разглашением или заполнение цикла.

## 2. Norito آرٹیفیکٹ کا معائنہ (اختیاری)

Если вы хотите использовать `ivm_tool`, вам нужно:

```sh
ivm_tool inspect target/examples/hello.to
```

Доступ к ABI и флагам, а также точкам входа и выбору точек входа. یہ ڈپلائمنٹ سے پہلے ایک فوری проверка здравомыслия ہے۔

## 3. کنٹریکٹ کو مقامی طور پر چلائیں

`ivm_run` может содержать байт-код, который может быть использован для:

```sh
ivm_run target/examples/hello.to --args '{}'
```

`hello` вызывает вызов системного вызова `SET_ACCOUNT_DETAIL`. Для того, чтобы получить доступ к онлайн-платежу, выберите подходящий вариант. تکرار کر رہے ہوں۔

## 4. `iroha_cli` کے ذریعے ڈپلائے کریں

Вы можете использовать интерфейс командной строки, чтобы узнать, как это сделать. Полномочия, необходимые для подписания ключа, `.to` и полезная нагрузка Base64 могут быть следующими:

```sh
iroha_cli app contracts deploy \
  --authority <i105-account-id> \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

Используйте Torii или Norito манифест + пакет байт-кода. ٹرانزیکشن کی حیثیت پرنٹ کرتی ہے۔ Создание коммита и создание манифестов хеш-кода для создания экземпляров Вот что вам нужно знать:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

## 5. Torii کے خلاف چلائیں

байткод رجسٹر ہونے کے بعد، آپ ایک, инструкция submit کر کے اسے کال کر کال کر سکتے ہیں جو محفوظ شدہ Это может быть сделано (مثلا `iroha_cli ledger transaction submit` یا آپ کے ایپ کلائنٹ کے ذریعے). Доступ к разрешениям и системным вызовам (`set_account_detail`, `transfer_asset`, например). دیتے ہیں۔

## تجاویز اور خرابیوں کا حل- `make examples-run` کریں تاکہ فراہم کردہ مثالیں ایک ہی قدم کمپائل اور سکیں۔ Переменные среды `KOTO`/`PATH` могут переопределять переменные среды `PATH`.
- `koto_compile` ABI может быть использован в качестве программного обеспечения для вашего удобства. ABI v1 может быть установлен в любое время (`koto_compile --abi` может быть использован для установки).
- Шестнадцатеричные ключи подписи Base64 для CLI. Воспользуйтесь ключами `iroha_cli tools crypto keypair`, чтобы получить доступ к клавишам, которые можно использовать.
- Norito полезные нагрузки, которые можно использовать `ivm_tool disassemble` Kotodama سورس سے جوڑا جا سکے۔

یہ CI اور انٹیگریشن ٹیسٹس میں استعمال ہونے والے مراحل کی عکاسیکرتا ہے۔ Kotodama Сопоставления системных вызовов. Внутренние устройства Norito могут быть использованы для следующих целей:

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`