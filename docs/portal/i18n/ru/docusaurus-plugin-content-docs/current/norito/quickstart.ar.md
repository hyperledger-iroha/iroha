---
lang: ru
direction: ltr
source: docs/portal/docs/norito/quickstart.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
Название: بدء سريع ل Norito
описание: ابن وتحقق وانشر عقد Kotodama باستخدام ادوات الاصدار والشبكة الفتراضية ذات العقدة الواحدة.
слизень: /norito/quickstart
---

Он сказал, что Сэнсэй Уилсон был в центре внимания в конце прошлого года. Norito и Kotodama по запросу: Пробный запуск был выполнен с помощью Torii и запущен CLI.

Он был показан Стоуном Миссисипи/Миссисипи в фильме "Старый человек". Во время установки `iroha_cli`.

## المتطلبات

- [Docker](https://docs.docker.com/engine/install/) в Compose V2 (отключен одноранговый узел с `defaults/docker-compose.single.yml`).
- Версия Rust (1.76+) была создана в 1999 году в 1980-х годах.
- Проверьте `koto_compile` и `ivm_run` и `iroha_cli`. Чтобы получить возможность оформить заказ в рабочей области, вам нужно будет просмотреть артефакты:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Откройте рабочее пространство для рабочего пространства.
> لا ترتبط مطلقا بـ `serde`/`serde_json`; Кодеки Norito используются в различных приложениях.

## 1. تشغيل شبكة تطوير بعقدة واحدة

Создайте пакет для Docker. Создайте файл `kagami swarm` (`defaults/docker-compose.single.yml`). يقوم بربط Genesis الافتراضي وتكوين العميل и датчики здоровья بحيث يكون Torii متاحا على `http://127.0.0.1:8080`.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

اترك الحاوية تعمل (في المقدمة او مفصولة). Откройте CLI и подключите одноранговый узел `defaults/client.toml`.

## 2. كتابة العقد

Сообщение от пользователя Kotodama:

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

> يفضل حفظ مصادر Kotodama تحت التحكم بالنسخ. Вы можете получить доступ к файлу [Norito](./examples/) اذا. Он сказал, что это не так.

## 3. Проверка и пробный запуск IVM

Введите байт-код IVM/Norito (`.to`) для получения дополнительной информации. Системные вызовы вызываются при вызове:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Вызовите бегуна `info("Hello from Kotodama")` и выполните системный вызов `SET_ACCOUNT_DETAIL` для запуска. Для этого используется `ivm_tool` и `ivm_tool inspect target/quickstart/hello.to` для заголовка ABI, функциональных битов и точек входа. المصدرة.

## 4. Введите байт-код Torii.

Загрузите байт-код в Torii для CLI. Он был создан в соответствии с требованиями `defaults/client.toml`, لذلك يكون. Автор сообщения:
```
soraカタカナ...
```

Чтобы получить доступ к URL-адресу Torii и идентификатору цепочки, выполните следующие действия:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

Для запуска CLI используйте Norito, а затем установите флажок Он был сверстником. Вызовите Docker для системного вызова `set_account_detail` и откройте CLI для хеш-кода.

## 5. Сделай это в режиме реального времени.

Откройте детали учетной записи CLI и нажмите кнопку:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id soraカタカナ... \
  --key example | jq .
```

Отобразить полезную нагрузку JSON в формате Norito:

```json
{
  "hello": "world"
}
```

Для этого необходимо создать хеш-код Docker и создать хеш-код. Установите флажок `iroha` и установите `Committed`.

## الخطوات التالية- استكشف [معرض الامثلة](./examples/) المولد تلقائيا لرؤية
  При вызове Kotodama происходит вызов системных вызовов Norito.
- اقرأ [دليل البدء Norito](./getting-started) للحصول على شرح اعمق
  Лаборатория/бегун Найт проявляет ошибку IVM.
- عند التكرار على عقودك, استخدم `npm run sync-norito-snippets` في
  рабочее пространство
  Установите флажок `crates/ivm/docs/examples/`.