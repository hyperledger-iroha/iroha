---
lang: ru
direction: ltr
source: docs/portal/docs/norito/quickstart.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
заголовок: Демарраж Norito
описание: Создать, проверить и развернуть контракт Kotodama с использованием одной пары релизов и ресурсов по умолчанию.
слизень: /norito/quickstart
---

Это пошаговое руководство воспроизводит рабочий процесс, который мы сопровождаем разработчиков, которые разработали Norito и Kotodama для премьеры: определение детерминированной монопары, компилятор, локальный тест, затем отправитель через Torii со ссылкой на CLI.

Образец договора содержит пару ключей/ценностей в счете апеллянта, чтобы вы могли немедленно проверить эффект борда с `iroha_cli`.

## Предварительное условие

- [Docker](https://docs.docker.com/engine/install/) с активным Compose V2 (используйте для разделения пары примеров, определенных в `defaults/docker-compose.single.yml`).
- Инструментарий Rust (1.76+) для создания вспомогательных бинарников, если вы не хотите телезаряжать их в публикации.
- Бинары `koto_compile`, `ivm_run` и `iroha_cli`. Вы можете построить кассу рабочего пространства как ci-dessous или телезарядить артефакты корреспондентов выпуска:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Les binaires ci-dessus sont sans sans risque — установщик с остальным рабочим пространством.
> Ils ne lient jamais `serde`/`serde_json` ; Кодеки Norito содержат приложения bout en bout.

## 1. Удаление монопары из резервной копии

В состав склада входит комплект Docker. Создайте родовой код `kagami swarm` (`defaults/docker-compose.single.yml`). Он подключит исходный код по умолчанию, клиент конфигурации и датчики работоспособности, которые Torii можно соединить с `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Laissez le conteneur Tourner (на первом плане или отдельно). Все команды CLI могут быть подключены через `defaults/client.toml`.

## 2. Подтвердить договор

Создайте репертуар труда и зарегистрируйте пример Kotodama минимальный:

```sh
mkdir -p target/quickstart
cat > target/quickstart/hello.ko <<'KO'
// Writes a deterministic account detail for the transaction authority.

seiyaku Hello {
  // Optional initializer invoked during deployment.
  hajimari() {
    info("Hello from Kotodama");
  }

  // Public entrypoint that records a JSON marker on the caller.
  kotoage fn write_detail() {
    set_account_detail(
      authority(),
      name!("example"),
      json!{ hello: "world" }
    );
  }
}
KO
```

> Предпочитаю сохранять исходные файлы Kotodama под контролем версии. Des exmples heberges sur le portail sont aussi disponibles dans la [galerie d'examples Norito](./examples/), если вы хотите отправиться в пункт отправления плюс богатство.

## 3. Компилятор и пробный запуск с IVM

Скомпилируйте контракт в байт-коде IVM/Norito (`.to`) и выполните локаль для подтверждения того, что системные вызовы хоста будут повторно использоваться перед нажатием кнопки:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Бегун заполняет журнал `info("Hello from Kotodama")` и выполняет системный вызов `SET_ACCOUNT_DETAIL` против симуляции хоста. Если опция двоичного кода `ivm_tool` доступна, `ivm_tool inspect target/quickstart/hello.to` добавляет к ABI общий доступ, некоторые функции и экспортируемые точки входа.

## 4. Собрать байт-код через Torii

Во время выполнения команды, отправив байт-код, скомпилируйте Torii с CLI. Идентификатор разработки по умолчанию является производным от публичного общества в `defaults/client.toml`, не используйте идентификатор счета.
```
<i105-account-id>
```

Используйте файл конфигурации для URL-адреса Torii, идентификатора цепочки и файла подписи:```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI кодирует транзакцию с Norito, подпись с кодом разработчика и отправителем в процессе выполнения. Просматривайте журналы Docker для системного вызова `set_account_detail` или просматривайте выборку CLI для хэширования зафиксированной транзакции.

## 5. Проверка изменения состояния

Используйте мем-профиль CLI для восстановления подробностей учетной записи, которые содержат контракт:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <i105-account-id> \
  --key example | jq .
```

Вы разработаете для полезной нагрузки JSON доступ к Norito:

```json
{
  "hello": "world"
}
```

Если значение отсутствует, проверьте, что служба Docker составляет турне toujours и что хеш-сигнал транзакции соответствует `iroha`, и обратите внимание на состояние `Committed`.

## Следующие этапы

- Откройте для себя [галерею примеров] (./examples/) автоматически.
  комментарий к фрагментам Kotodama плюс переход к системным вызовам Norito.
- Lisez le [руководство Norito начало работы] (./getting-started) для объяснения
  плюс приложение для компилятора/исполнителя утилит, развертывания манифестов и метадонов IVM.
- Lorsque vous iterez sur vos propres contrats, utilisez `npm run sync-norito-snippets` dans le
  рабочее пространство для регенерации фрагментов телезарядных устройств, где документы и артефакты остаются в памяти
  синхронизируется с исходными файлами sous `crates/ivm/docs/examples/`.