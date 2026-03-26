---
lang: ru
direction: ltr
source: docs/portal/docs/norito/quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
заголовок: Быстрый старт Norito
описание: Соберите, проверьте и задеплойте контракт Kotodama с релизным инструментарием и дефолтной одноузловой сетью.
слизень: /norito/quickstart
---

В этом пошаговом описании процесса мы ожидаем стать разработчиком при первом знакомстве с Norito и Kotodama: поднять детерминированную одноузловую сеть, составить контракт, выполнить локальный пробный прогон, а затем отправить его через Torii с эталонным CLI.

Пример контракта записывает пару ключей/значений в вызывающем аккаунте, чтобы вы могли сразу проверить побочный эффект с помощью `iroha_cli`.

## Требования

- [Docker](https://docs.docker.com/engine/install/) с включенным Compose V2 (используется для запуска образца однорангового узла, определение в `defaults/docker-compose.single.yml`).
- Набор инструментов Rust (1.76+) для сборки вспомогательных бинарников, если вы не скачаете опубликованные файлы.
- Бинарники `koto_compile`, `ivm_run` и `iroha_cli`. Их можно собрать из рабочей области оформления заказа, как показано ниже, или скачать соответствующие артефакты выпуска:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Эти бинарники безопасно хранить рядом с коронавирусами на рабочем месте.
> Они никогда не линкуются с `serde`/`serde_json`; кодеки Norito применяются сквозные.

## 1. Запустите одноузловую сеть разработчиков.

В репозитории есть Docker Составьте пакет, сгенерированный `kagami swarm` (`defaults/docker-compose.single.yml`). Он подключает дефолтную генерацию, настройку клиента и датчики состояния, чтобы Torii был доступен на `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Оставьте контейнер запущенным (на фоне или на переднем плане). Все вызовы CLI будут ориентированы на этот узел через `defaults/client.toml`.

## 2. Напишите контракт

Создайте руководство и сохраните производство, пример Kotodama:

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

> Предпочитайте хранить исходники Kotodama в системе контроля управления. Примеры, размещенные на портале, также доступны в [галерее примеров Norito](./examples/), если нужен более богатый стартовый набор.

## 3. Компиляция и пробный прогон с IVM

Скомпилируйте контракт в байткод IVM/Norito (`.to`) и запустите его локально, чтобы убедиться, что системные вызовы хоста передаются к обращению к сети:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Бегун выводит лог `info("Hello from Kotodama")` и выполняет системный вызов `SET_ACCOUNT_DETAIL` против смоделированного хоста. Если доступен дополнительный бинарник `ivm_tool`, команда `ivm_tool inspect target/quickstart/hello.to` показывает заголовок ABI, функциональные биты и экспортированные точки входа.

## 4. Отправьте байткод через Torii

Пока узел работает, отправьте скомпилированный байткод в Torii через CLI. Дефолтная дев-идентичность выводится из публичного ключа в `defaults/client.toml`, поэтому ID аккаунта:
```
soraカタカナ...
```

Используйте конфигурационный файл, чтобы задать URL Torii, идентификатор цепочки и ключ загрузки:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```CLI кодирует транзакцию Norito, подписывает ее dev-ключом и отправляет работающему узлу. Следите за логами Docker для системного вызова `set_account_detail` или отслеживайте вывод CLI для хэша зафиксированной передачи.

## 5. Проверьте изменение состояния

Используйте тот же профиль CLI, чтобы получить сведения об аккаунте, который заключил контракт:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id soraカタカナ... \
  --key example | jq .
```

Вы должны увидеть полезную нагрузку JSON на базе Norito:

```json
{
  "hello": "world"
}
```

Если значение отсутствует, убедитесь, что сервис Docker compose все еще работает и что хэш-транзакция, которая вывела `iroha`, достигла состояния `Committed`.

## Следующие шаги

- Изучите авто-сгенерированную [галерею примеров](./examples/), чтобы увидеть,
  как более продвинутые фрагменты Kotodama маппируются на системных вызовах Norito.
- Прочитайте [Norito руководство по началу работы] (./getting-started) для более глубокого
  объяснения инструментов компилятора/раннера, манифестов развертывания и метаданных IVM.
- При работе над моими контрактами викор `npm run sync-norito-snippets` в
  рабочая область, чтобы регенерировать загружаемые фрагменты и хранить документы портала и архивы.
  синхронизированными исходниками в `crates/ivm/docs/examples/`.