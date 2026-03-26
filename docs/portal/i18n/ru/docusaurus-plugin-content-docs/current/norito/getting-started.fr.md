---
lang: ru
direction: ltr
source: docs/portal/docs/norito/getting-started.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Главный приз Norito

В этом руководстве быстро представлен минимальный рабочий процесс для компилятора контракта Kotodama, инспектора байт-кода Norito, локального исполнителя и развертывания на нуде Iroha.

## Предварительное условие

1. Установите набор инструментов Rust (1,76 или плюс) и восстановите его в хранилище.
2. Создайте или зарядите бинарники поддержки:
   - `koto_compile` - компилятор Kotodama, который принимает байт-код IVM/Norito
   - `ivm_run` и `ivm_tool` — утилиты для локального выполнения и проверки
   - `iroha_cli` — использовать для развертывания контрактов через Torii.

   Файл Makefile du depot посещает ces binaires dans `PATH`. Вы можете использовать артефакты для предварительной компиляции или создания исходных текстов. Если вы скомпилировали локаль инструментальной цепочки, укажите помощники Makefile и бинарники:

   ```sh
   KOTO=./target/debug/koto_compile IVM=./target/debug/ivm_run make examples-run
   ```

3. Убедитесь, что Iroha находится в процессе выполнения, прежде чем выполнить этап развертывания. Примеры, предполагающие, что Torii доступны для настройки URL-адреса в вашем профиле `iroha_cli` (`~/.config/iroha/cli.toml`).

## 1. Компилятор по контракту Kotodama

Склад представляет собой минимальный контракт «Hello World» в `examples/hello/hello.ko`. Скомпилируйте файл с байт-кодом Norito/IVM (`.to`):

```sh
mkdir -p target/examples
koto_compile examples/hello/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/examples/hello.to
```

Опции:

- `--abi 1` verrouille le contrat sur la version ABI 1 (только поддерживается на момент редактирования).
- `--max-cycles 0` требует неограниченного исполнения; fixez un nombre positif для заполнения циклов для предотвращения нулевых знаний.

## 2. Инспектор артефактов Norito (опция)

Используйте `ivm_tool` для проверки целостности и целостности метадонов:

```sh
ivm_tool inspect target/examples/hello.to
```

Вы разработаете версию ABI, активные флаги и экспортированные точки входа. Это быстрый контроль перед развертыванием.

## 3. Исполнение локального контракта

Выполните байт-код с `ivm_run` для подтверждения соответствия без прикосновения:

```sh
ivm_run target/examples/hello.to --args '{}'
```

Пример `hello` публикует приветствие и вызывает системный вызов `SET_ACCOUNT_DETAIL`. Язык выполнения — это полезный кулон, который вы выполняете по логике контракта перед публикацией в сети.

## 4. Деплоер через `iroha_cli`

Если вы удовлетворены контрактом, разверните его через интерфейс командной строки. Fournissez un compte d'autorite, sacle de Signature et soit un fichier `.to` soit un payload Base64 :

```sh
iroha_cli app contracts deploy \
  --authority soraカタカナ... \
  --private-key <hex-encoded-private-key> \
  --code-file target/examples/hello.to
```

Команда соединяет манифест Norito + байт-код через Torii и добавляет статус результата транзакции. Если транзакция действительна, хеш-код, прикрепленный к ответу, может быть использован для восстановления манифестов или списка экземпляров:

```sh
iroha_cli app contracts manifest get --code-hash 0x<hash>
iroha_cli app contracts instances --namespace apps --table
```

##5. Исполнитель через ToriiПосле регистрации байт-кода вы можете вызвать команду, которая будет ссылаться на стандартный код (например, через `iroha_cli ledger transaction submit` или ваше клиентское приложение). Убедитесь, что у вас есть разрешения на авторизацию системных вызовов (`set_account_detail`, `transfer_asset` и т. д.).

## Советы и депаннаж

- Используйте `make examples-run` для компиляции и выполнения примеров в отдельной команде. Дополните переменные среды `KOTO`/`IVM`, если двоичные файлы не включены в `PATH`.
- Если `koto_compile` откажется от версии ABI, проверьте, что компилятор и работает ли он в ABI v1 (выполните `koto_compile --abi` без аргументов для поддержки списка).
- CLI принимает ключи подписи в шестнадцатеричном формате или Base64. Для проведения тестов вы можете использовать файлы, выданные по номеру `iroha_cli tools crypto keypair`.
- Для отладки полезных нагрузок Norito, вспомогательная команда `ivm_tool disassemble` помогает коррелировать инструкции с исходным кодом Kotodama.

Этот поток отражает этапы использования в CI и в тестах интеграции. Для анализа и применения грамматики Kotodama, сопоставлений системных вызовов и внутренних компонентов Norito, вы увидите:

- `docs/source/kotodama_grammar.md`
- `docs/source/kotodama_examples.md`
- `norito.md`