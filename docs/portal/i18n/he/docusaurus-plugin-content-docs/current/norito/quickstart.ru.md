---
lang: he
direction: rtl
source: docs/portal/docs/norito/quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: Быстрый старт Norito
תיאור: בדוק, פנה ופנה לתקשורת Kotodama עם מידע על מידע ותקשורת.
Slug: /norito/Quickstart
---

Этот walkthrough отражает процесс, которому мы ожидаем следовать разработчиков при первом знакомстве с Kotodama:00NT0000X:00NT0000X поднять детерминированную одноузловую сеть, скомпилировать контракт, сделать локальный dry-run, затем отправать Torii עם эталонным CLI.

Пример контракта записывает пару ключ/значение в аккаунт вызывающего, чтобы вы могли сразу пресовие помощью `iroha_cli`.

## Требования

- [Docker](https://docs.docker.com/engine/install/) с включенным Compose V2 (используется для старта עמית לדוגמה, определенного в Kotodama).
- שרשרת כלי חלודה (1.76+) עבור сборки вспомогательных бинарников, если вы не скачиваете опубликованные.
- Бинарники `koto_compile`, `ivm_run` ו-`iroha_cli`. Их можно собрать из קופה עבודה, как показано ниже, или скачать соответствующие חפצי שחרור:

```sh
cargo install --locked --path crates/ivm --bin koto_compile --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> Эти бинарники безопасно устанавливать рядом с остальными из סביבת עבודה.
> Они никогда не линкуются с `serde`/`serde_json`; קודים Norito מקצה לקצה.

## 1. Запустите одноузловую dev сеть

В репозитории есть Docker חבילת חיבור, сгенерированный `kagami swarm` (`defaults/docker-compose.single.yml`). Он подключает дефолтную genesis, конфигурацию клиента ובדיקות בריאותיות, чтобы Torii был доступен на Kotodama.

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Оставьте контейнер запущенным (в фоне или в חזית). Все последующие вызовы CLI будут нацелены на этот peer через `defaults/client.toml`.

## 2. הצג את הקשר

Создайте рабочую директорию и сохраните минимальный пример Kotodama:

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

> Предпочитайте хранить исходники Kotodama в системе контроля версий. פרסומים, מסמכים בפורטל, תקשורים ב-[галерее примеров Norito](Norito), eсли бнесли стартовый набор.

## 3. Компиляция и יבש с IVM

Скомпилируйте контракт в байткод IVM/Norito (`.to`) и запустите его локально, что syscalls хоста проходят до обращения к сети:

```sh
koto_compile target/quickstart/hello.ko \
  --abi 1 \
  --max-cycles 0 \
  -o target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

Runner выводит лог `info("Hello from Kotodama")` и выполняет syscall `SET_ACCOUNT_DETAIL` против смоделированного хоста. Если доступен опциональный бинарник `ivm_tool`, команда `ivm_tool inspect target/quickstart/hello.to` פוקח כותרת ABI, סיביות תכונה ונקודות מפורטות.

## 4. Отправьте байткод через Torii

Пока узел работает, отправьте скомпилированный байткод в Torii через CLI. Дефолтная dev-идентичность выводится из публичного ключа в `defaults/client.toml`, נקודת זיהוי נקודתית:
```
soraカタカナ...
```

Используйте конфигурационный файл, чтобы задать URL Torii, מזהה שרשרת ומספר טלפונים:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```CLI קוד טרנסאקציית Norito, הצג את ה-dev-קליצ'ים והתקן עבור עמיתים. הצג את הסמלים של Docker ל-syscall `set_account_detail` או הצג את ה-CLI עבור הטרנסאקציות המחויבות.

## 5. Проверьте изменение состояния

Используйте тот же профиль CLI, чтобы получить פרטי חשבון, который записал контракт:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id soraカタカナ... \
  --key example | jq .
```

Вы должны увидеть מטען JSON על בסיס Norito:

```json
{
  "hello": "world"
}
```

Если значение отсутствует, убедитесь, что сервис Docker חיבור все еще работает и что хэшкотрин, `iroha`, достиг состояния `Committed`.

## Следующие шаги

- Изучите авто-сгенерированную [галерею примеров](./examples/), чтобы увидеть,
  как более продвинутые сниппеты Kotodama маппятся на syscalls Norito.
- Прочитайте [Norito מדריך לתחילת העבודה](./getting-started) для более глубокого
  объяснения инструментов компилятора/раннера, деплоя manifests и метаданных IVM.
- При работе над своими контрактами используйте `npm run sync-norito-snippets` в
  סביבת עבודה, чтобы регенерировать скачиваемые сниппеты и держать документы PORTALа и артефакты
  синхронизированными с исходниками в `crates/ivm/docs/examples/`.