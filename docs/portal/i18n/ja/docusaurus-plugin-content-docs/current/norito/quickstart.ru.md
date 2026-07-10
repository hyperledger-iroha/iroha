---
lang: ja
direction: ltr
source: docs/portal/docs/norito/quickstart.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: Быстрый старт Norito
説明: Соберите、проверьте и задеплойте контракт Kotodama с релизным инструментарием и дефолтной одноузловойさいしょ。
スラグ: /norito/quickstart
---

Этот walkthrough отражает процесс, которому мы ожидаем следовать разработчиков при первом знакомстве с Norito и Kotodama: テスト、テスト、ドライラン、テストотправить его через Torii с эталонным CLI.

Пример контракта записывает пару ключ/значение в аккаунт вызывающего, чтобы вы могли сразу проверить `iroha_cli` を確認してください。

## Требования

- [Docker](https://docs.docker.com/engine/install/) Compose V2 (サンプル ピア、`defaults/docker-compose.single.yml`) を実行します。
- Rust ツールチェーン (1.76 以降) がサポートされています。
- Бинарники `koto build`、`ivm_run`、`iroha_cli`。チェックアウト ワークスペース、リリース アーティファクトの説明:

```sh
cargo install --locked --path crates/ivm --bin koto --bin ivm_run
cargo install --locked --path crates/iroha_cli --bin iroha
```

> ワークスペースを使用してください。
> Они никогда не линкуются с `serde`/`serde_json`; кодеки Norito エンドツーエンド。

## 1. 開発を完了する

Docker バンドルの作成、`kagami swarm` (`defaults/docker-compose.single.yml`)。ジェネシス、健康プローブ、Torii および `http://127.0.0.1:8080` を参照してください。

```sh
docker compose -f defaults/docker-compose.single.yml up --build
```

Оставьте контейнер запущенным (前景)。 CLI を使用して、ピア `defaults/client.toml` を確認します。

## 2. Написите контракт

Создайте рабочую директорию и сохраните минимальный пример Kotodama:

```sh
mkdir -p target/quickstart
cat > target/quickstart/hello.ko <<'KO'
seiyaku Hello {
    hajimari() {
        debug::info("Hello from hajimari");
    }

    kotoage fn write_detail() authorize("Admin") {
        ledger::account::set_detail(
            context::authority(),
            Name::parse("example"),
            Json::parse("{\"hello\":\"world\"}")
        );
    }

    view fn healthy() -> bool {
        return true;
    }
}
KO
```

> Предпочитайте хранить исходники Kotodama в системе контроля версий. Примеры、размещенные на портале、также доступны в [галерее примеров Norito](./examples/)、если нужен Солее богатый стартовый набор.

## 3. ドライラン с IVM

Скомпилируйте контракт в байткод IVM/Norito (`.to`) и запустите его локально, чтобы必要なシステムコールの数:

```sh
koto build target/quickstart/hello.ko \
  --max-cycles 1000000 \
  --out target/quickstart/hello.to

ivm_run target/quickstart/hello.to --args '{}'
```

ランナーは `debug::info("Hello from Kotodama")` とシステムコール `SET_ACCOUNT_DETAIL` を実行します。 ABI ヘッダー、機能ビット、およびエントリ ポイントをサポートします。

## 4. Отправьте байткод через Torii

CLI を使用して、Torii を実行します。 dev-идентичность выводится из публичного ключа в `defaults/client.toml`, поэтому ID аккаунта:
```
<i105-account-id>
```

Используйте конфигурационный файл、чтобы задать URL Torii、チェーン ID および ключ подписи:

```sh
iroha --config defaults/client.toml \
  transaction ivm \
  --path target/quickstart/hello.to
```

CLI は、Norito、dev-ключом および отправляет на работающий ピアを表示します。 Docker とシステムコール `set_account_detail` がコミットされた CLI を実行しました。

## 5. Проверьте изменение состояния

CLI、アカウント詳細、который записал контракт:

```sh
iroha --config defaults/client.toml \
  account meta get \
  --id <i105-account-id> \
  --key example | jq .
```

JSON ペイロードを Norito で取得します:

```json
{
  "hello": "world"
}
```

Если значение отсутствует, убедитесь, что сервис Docker compose все работает и что хэл транзакции, `iroha`、`Committed` を参照してください。

## Следующие заги

- Изучите авто-сгенерированную [галерею примеров](./examples/), чтобы увидеть,
  Kotodama はシステムコール Norito を呼び出します。
- [Norito スタート ガイド](./getting-started) の説明
  объяснения инструментов компилятора/раннера、деплоя は метаданных IVM を示します。
- При работе над своими контрактами используйте `npm run sync-norito-snippets` в
  ワークスペース、ワークスペースを使用して、必要な作業を実行します。
  `crates/ivm/docs/examples/` を参照してください。

```kotodama
seiyaku Hello {
    hajimari() {
        debug::info("Hello from hajimari");
    }

    kotoage fn write_detail() authorize("Admin") {
        ledger::account::set_detail(
            context::authority(),
            Name::parse("example"),
            Json::parse("{\"hello\":\"world\"}")
        );
    }

    view fn healthy() -> bool {
        return true;
    }
}
```

```kotodama
seiyaku Hello {
    hajimari() {
        debug::info("Hello from hajimari");
    }

    kotoage fn write_detail() authorize("Admin") {
        ledger::account::set_detail(
            context::authority(),
            Name::parse("example"),
            Json::parse("{\"hello\":\"world\"}")
        );
    }

    view fn healthy() -> bool {
        return true;
    }
}
```

```kotodama
seiyaku Hello {
    hajimari() {
        debug::info("Hello from hajimari");
    }

    kotoage fn write_detail() authorize("Admin") {
        ledger::account::set_detail(
            context::authority(),
            Name::parse("example"),
            Json::parse("{\"hello\":\"world\"}")
        );
    }

    view fn healthy() -> bool {
        return true;
    }
}
```
