---
lang: ru
direction: ltr
source: docs/portal/docs/norito/quickstart.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
заголовок: Быстрое начало Norito
описание: Crea, valida y despliega un contrato Kotodama con las herramientas de Release y la red predeterminada de un Solo Peer.
слизень: /norito/quickstart
---

Это записанное отражение эль-флюо, которое эсперамо que sigan los desarrolladores al aprender Norito y Kotodama por primera vez: arrancar una red determinista de одиночный одноранговый узел, компиляция un contrato, hacer un Dry-run local и luego enviarlo por Torii с справочным интерфейсом командной строки.

В приведенном примере опишите ключ/доблесть в куэнте дель Ламадор, чтобы можно было проверить боковой эффект немедленного действия с `iroha_cli`.

## Предыдущие реквизиты

- [Docker](https://docs.docker.com/engine/install/) с навыком Compose V2 (используйте его для запуска определенного узла в `defaults/docker-compose.single.yml`).
- Toolchain de Rust (1.76+), позволяющий создавать вспомогательные двоичные файлы, не удаляя их из опубликованных источников.
- Бинарные файлы `koto_compile`, `ivm_run` и `iroha_cli`. Можно создать из извлечения рабочей области как нужное место или удалить артефакты из соответствующего выпуска:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Предшествующие двоичные файлы обеспечивают безопасность установки в рабочем пространстве.
> Nunca enlazan с `serde`/`serde_json`; Кодеки Norito являются сквозными.

## 1. Начало красного разработчика одиночного узла

Репозиторий включает в себя пакет Docker, созданный для `kagami swarm` (`defaults/docker-compose.single.yml`). Подключите источник из-за дефекта, конфигурацию клиента и датчики состояния здоровья, чтобы Torii был доступен в море через `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Deja el contenedor corriendo (в начале плана или desacoplado). Все задние пояснения CLI доступны на уровне `defaults/client.toml`.

## 2. Редактирование контракта

Создайте директорию по работе и охрану минимального задания Kotodama:

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

> Предпочитайте поддерживать функции Kotodama при управлении версиями. Лос-ejemplos alojados на портале также доступны в [galeria de ejemplos Norito](./examples/), если вы хотите, чтобы точка была полной.

## 3. Компиляция и пробный запуск с IVM

Скомпилируйте контратаку с байт-кодом IVM/Norito (`.to`) и выведите локально для подтверждения того, что системные вызовы хоста функционируют до того, как он будет открыт:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Бегун вводит журнал `info("Hello from Kotodama")` и запускает системный вызов `SET_ACCOUNT_DETAIL` с помощью симуляции хоста. Если дополнительный бинарный файл `ivm_tool` доступен, `ivm_tool inspect target/quickstart/hello.to` должен включать в себя ABI, биты функций и экспортированные точки входа.

## 4. Передача байт-кода через Torii

С адресом исправления, отправьте байт-код, скомпилированный в Torii, используя CLI. Идентификатор desarrollo por deriva de la clave publica en `defaults/client.toml`, por lo que el ID de cuenta es
```
i105...
```

Используйте архив конфигурации для просмотра URL-адреса Torii, идентификатора цепочки и ключа фирмы:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```CLI кодирует транзакцию с Norito, фирма с ключом десарролла и отправкой через одноранговый узел в изгнании. Просмотрите журналы Docker для системного вызова `set_account_detail` или отслеживайте вызов CLI для хэша скомпрометированной транзакции.

## 5. Проверка состояния страны

Используйте файл CLI для получения сведений об учетной записи, которые необходимо написать договор:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id i105... \
  --key example | jq .
```

Проверьте полезную нагрузку JSON, измененную на Norito:

```json
{
  "hello": "world"
}
```

Если вы не доблестны, подтвердите, что служба Docker создала сообщение об удалении и что хеш-отчет о транзакции для `iroha` находится на территории `Committed`.

## Сигиентес Пасос

- Исследуйте [галерею изображений] (./examples/), автоматически создаваемую для версии.
  Как фрагменты Kotodama могут быть преобразованы в системные вызовы Norito.
- Ли ла [начальное описание Norito](./getting-started) для объяснения
  более глубокие инструменты компилятора/бегуна, описание манифестов и метаданные IVM.
- Cuando iteres en tus propios contratos, USA `npm run sync-norito-snippets` en el
  Рабочая область для регенерации фрагментов данных, извлеченных из режима, который содержит документы портала и артефакты
  были синхронизированы с фуэнтами в `crates/ivm/docs/examples/`.