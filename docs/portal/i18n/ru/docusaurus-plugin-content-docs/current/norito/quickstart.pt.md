---
lang: ru
direction: ltr
source: docs/portal/docs/norito/quickstart.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
заголовок: Быстрое начало Norito
описание: Создание, проверка и развертывание контракта Kotodama с инструментами для выпуска и повторным запуском единого узла.
слизень: /norito/quickstart
---

Este passo a passo espelha or fluxo que esperamos que desenvolvedores sigam ao aprender Norito e Kotodama pela primeira vez: iniciar uma rede deterministica de um unico peer, compilar um contrato, fazer Dry-run localmente e depois enviar via Torii через CLI ссылки.

Контрато-де-пример важен для того, чтобы вести себя/доблестно, если вы хотите, чтобы вы могли проверить или получить залоговое преимущество немедленно с `iroha_cli`.

## Предварительные требования

- [Docker](https://docs.docker.com/engine/install/) с привычным составлением V2 (используется для запуска или однорангового примера, определенного в `defaults/docker-compose.single.yml`).
- Инструментарий Rust (1.76+) для компиляции вспомогательных двоичных файлов, когда вы их публикуете.
- Бинарные файлы `koto_compile`, `ivm_run` и `iroha_cli`. Вы можете скомпилировать часть оформления заказа в рабочем пространстве в качестве рабочего места или добавить артефакты корреспондентов выпуска:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Наши двоичные файлы должны быть безопасными для установки соединения с рабочей областью или восстановления рабочего пространства.
> Eles nunca fazem link com `serde`/`serde_json`; Кодеки ОС Norito являются приложениями для мостов и мостов.

## 1. Начало работы с единым одноранговым узлом

В репозиторий включен пакет Docker. Создайте файл для `kagami swarm` (`defaults/docker-compose.single.yml`). Он подключился к начальному устройству, настроил клиента и датчики состояния здоровья для Torii, доступного для `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Дейкс или контейнер родандо (на первом плане или отдельно). Все последующие запросы CLI доступны для однорангового соединения через `defaults/client.toml`.

## 2. Написать или договориться

Назовите директорию по труду и мазок или минимальный пример Kotodama:

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

> Введите шрифты Kotodama для управления версией. Примеры госпиталей без портала также доступны на [галерее примеров Norito](./examples/), если вы хотите, чтобы вы сказали, что хотите стать частью моего Рико.

## 3. Скомпилируйте и выполните пробный запуск com IVM.

Скомпилируйте строку с байт-кодом IVM/Norito (`.to`) и выполните локально, чтобы подтвердить, что системные вызовы выполняют функции хоста перед повторным выполнением:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Бегун заполняет журнал `info("Hello from Kotodama")` и выполняет системный вызов `SET_ACCOUNT_DETAIL` против хоста-макада. Если установлен дополнительный бинарный файл `ivm_tool`, `ivm_tool inspect target/quickstart/hello.to` отображается или заголовок ABI, биты функций и экспортированные точки входа.

## 4. Зависть от байт-кода через Torii

Как ни странно, завидуйте компиляции байт-кода для Torii с использованием CLI. Идентификация вовлеченного человека и полученная от него публичная информация под номером `defaults/client.toml`, в порту или с идентификатором содержания
```
i105...
```

Используйте архив конфигурации для получения URL-адреса Torii, идентификатора цепочки и ввода-вывода:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```В CLI кодируется транзакция с Norito, подключение к устройству и передача через одноранговый узел при выполнении. Просмотрите журналы операционной системы, чтобы выполнить Docker для системного вызова `set_account_detail` или отслеживать данные CLI для хэширования зафиксированной транзакции.

## 5. Проверьте наличие статуса

Используйте запрос CLI для загрузки данных учетной записи или серьезного контракта:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id i105... \
  --key example | jq .
```

При разработке версии полезной нагрузки JSON, поддерживаемой Norito:

```json
{
  "hello": "world"
}
```

Если вы достигли успеха, подтвердите, что сервис Docker составил родандо и хэш отчета о транзакции для `iroha`, который находится в состоянии `Committed`.

## Проксимос пассос

- Изучите [галерею образцов](./examples/), автоматически добавляемую для проверки.
  как фрагменты Kotodama, которые можно использовать для системных вызовов Norito.
- Лея или [начальный текст Norito](./getting-started) для объяснения
  Более глубокое создание инструментов компилятора/исполнителя, развертывание манифестов и метаданных IVM.
- Чтобы повторить наши контракты, используйте `npm run sync-norito-snippets` без рабочего пространства для параметра
  повторно сгенерировать фрагменты, добавить документы и синхронизировать артефакты с порталом в виде шрифтов `crates/ivm/docs/examples/`.