---
lang: ru
direction: ltr
source: docs/portal/docs/norito/quickstart.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
заголовок: Norito کوئک اسٹارٹ
описание: ریلیز ٹولنگ اور ڈیفالٹ سنگل-پیئر نیٹ ورک کے ساتھ Kotodama کنٹریکٹ بنائیں، ویلیڈیٹ کریں اور ڈپلائے کریں۔
слизень: /norito/quickstart
---

یہ Прохождение игры, в которой вы можете найти игру, которая поможет вам в этом. Если у вас есть Norito или Kotodama, выберите нужный вариант: Проведение пробного прогона в режиме пробного прогона Используйте интерфейс CLI, чтобы установить Torii, чтобы использовать интерфейс командной строки.

Если вы хотите использовать ключ/значение, выберите `iroha_cli`. Возможные побочные эффекты

## پیشگی تقاضے

- [Docker](https://docs.docker.com/engine/install/) Запустите Compose V2 и выберите `defaults/docker-compose.single.yml` для выборки однорангового узла. کے لئے استعمال کیا جاتا ہے).
- Набор инструментов Rust (1.76+) Вспомогательные двоичные файлы и дополнительные двоичные файлы, которые можно использовать.
- `koto_compile`, `ivm_run`, двоичные файлы `iroha_cli`. آپ انہیں проверка рабочей области سے نیچے گئے ڷریقے کے کے مطابق بنا سکتے ہیں گئے ڈاؤن Ниже приведены примеры:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Использование двоичных файлов в рабочей области или в рабочей области
> یہ کبھی `serde`/`serde_json` سے کبھی کرتے؛ Сквозные кодеки Norito

## 1. سنگل-پیئر dev نیٹ ورک شروع کریں

Создайте пакет `kagami swarm` и выберите Docker. Создайте пакет в формате `defaults/docker-compose.single.yml`. Здесь можно найти генезис, конфигурацию клиента, датчики работоспособности и Torii `http://127.0.0.1:8080` для проверки работоспособности.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

کنٹینر کو چلتا رہنے دیں (отдельный план на переднем плане). Воспользуйтесь CLI для `defaults/client.toml`, чтобы получить доступ к одноранговому узлу.

## 2. کنٹریکٹ لکھیں

В качестве минимального значения Kotodama можно использовать:

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

> Kotodama Управление версиями для управления версиями Размещено на хостинге [Norito галерея примеров](./examples/) بھرپور نقطہ آغاز چاہتے ہیں۔

## 3. IVM может использоваться для пробного прогона.

Введите байт-код IVM/Norito (`.to`) Для того, чтобы получить доступ к системным вызовам хоста, необходимо выполнить следующие действия: ہو:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Runner `info("Hello from Kotodama")` запускает системный вызов, который высмеивает хост `SET_ACCOUNT_DETAIL` и вызывает системный вызов Двоичный файл `ivm_tool` с заголовком ABI `ivm_tool inspect target/quickstart/hello.to`, функциональные биты и экспортированные точки входа.

## 4. Torii — изменение байт-кода

Для этого необходимо использовать байт-код или интерфейс командной строки Torii для ввода-вывода. Идентификатор разработки `defaults/client.toml` для открытого ключа, идентификатора учетной записи или идентификатора учетной записи:
```
i105...
```

URL-адрес Torii, идентификатор цепочки и ключ подписи. Для этого необходимо настроить конфигурацию, а также указать:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```CLI Norito позволяет закодировать код ключа разработчика и знак کرتا چلتے ہوئے сверстник, пожалуйста, отправьте сообщение ہے۔ `set_account_detail` Системный вызов Docker logs Создание хэша зафиксированной транзакции и выходных данных CLI

## 5. Изменение состояния کی توثیق کریں

В интерфейсе командной строки можно просмотреть сведения об учетной записи, а также просмотреть данные учетной записи:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id i105... \
  --key example | jq .
```

Полезная нагрузка JSON с поддержкой Norito может быть выполнена следующим образом:

```json
{
  "hello": "world"
}
```

Если вы хотите, чтобы Docker составил список, вы можете Воспользуйтесь `iroha`, чтобы получить информацию о `Committed`. ہے۔

## مراحل

- خودکار طور پر تیار کی گئی [пример галереи](./examples/) دیکھیں تاکہ
  Расширенные фрагменты Kotodama и системные вызовы Norito
- مزید گہرائی کے لئے [Norito руководство по началу работы](./getting-started)
  инструменты компилятора/исполнителя, развертывание манифеста и метаданные IVM.
- Чтобы создать рабочую область `npm run sync-norito-snippets`, необходимо выполнить итерацию.
  загружаемые фрагменты можно найти в документации, артефактах, `crates/ivm/docs/examples/`, источниках и синхронизированных файлах.