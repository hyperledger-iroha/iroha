---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカーレジストリ
タイトル: Реестр профилей チャンカー SoraFS
サイドバーラベル: チャンカー
説明: ID は、チャンカー SoraFS です。
---

:::note Канонический источник
Эта страница отражает `docs/source/sorafs/chunker_registry.md`。スフィンクスの正体は、スフィンクスだ。
:::

## Реестр профилей チャンカー SoraFS (SF-2a)

Стек SoraFS はチャンク化を実行し、ネームスペースを取得します。
CDC のバージョン、ダイジェスト/マルチコーデック、マニフェストのバージョンなどCAR-архивах。

Авторы профилей должны обратиться к
[`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
для получения требуемых метаданных, чеклиста валидации и заблона предложения перед
отправкой новых записей。ガバナンスを強化する
[чеклисту ロールアウト реестра](./chunker-registry-rollout-checklist.md) および
[プレイブック マニフェストとステージング](./staging-manifest-playbook)、 чтобы продвинуть
備品、ステージング、プロダクション。

### Профили

|ネームスペース | Имя |セミバージョン | ID は | Мин (Мин) | Цель (ブガレッド) | Макс (Макс) | Маска разрыва |マルチハッシュ | Алиасы | Примечания |
|----------|-----|----------|----------|---------------|--------------|--------------|--------------|-----------|----------|-----------|
| `sorafs` | `sf1` | `1.0.0` | `1` | 65536 | 262144 | 524288 | `0x0000ffff` | `0x1f` (ブレイク3-256) | `["sorafs.sf1@1.0.0", "sorafs.sf1@1.0.0"]` | SF-1 のフィクスチャーを確認してください。

Реестр живет в коде как `sorafs_manifest::chunker_registry` (регулируется [`chunker_registry_charter.md`](./chunker-registry-charter.md))。 Каждая запись
`ChunkerProfileDescriptor` の結果:

* `namespace` – логическая группировка связанных профилей (например、`sorafs`)。
* `name` – читаемый человеком ярлык профиля (`sf1`, `sf1-fast`, …)。
* `semver` – строка семантической версии для набора параметров.
* `profile` – фактический `ChunkProfile` (最小/ターゲット/最大/マスク)。
* `multihash_code` – マルチハッシュ、ダイジェスト чанка (`0x1f`)
  SoraFS です)。

マニフェストは `ChunkingProfileV1` です。 Структура записывает метаданные реестра
(namespace, name, semver) は、CDC-параметрами и списком алиасов выге Потребители
должны сначала попытаться выполнить lookup в реестре по `profile_id` использовать inline-параметры,
когда встречаются неизвестные ID;最低のHTTP-клиенты могут продолжать
ハンドル (`namespace.name@semver`) のハンドル、`profile_aliases`、ハンドル

ツール、ヘルパー CLI の機能:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles
[
  {
    "namespace": "sorafs",
    "name": "sf1",
    "semver": "1.0.0",
    "handle": "sorafs.sf1@1.0.0",
    "profile_id": 1,
    "min_size": 65536,
    "target_size": 262144,
    "max_size": 524288,
    "break_mask": "0x0000ffff",
    "multihash_code": 31
  }
]
```

CLI、JSON (`--json-out`、`--por-json-out`、`--por-proof-out`、
`--por-sample-out`)、`-` と качестве пути、что стримит ペイロードと stdout と
Создания файла。ツールを使用した作業を行うことができます。
поведения печати основного отчета.

### Матрица совместимости и план ロールアウト


Таблица ниже отражает текущий статус поддержки `sorafs.sf1@1.0.0` в ключевых компонентах.
「ブリッジ」 относится к совместимому каналу CARv1 + SHA-256、который требует явной
クラス (`Accept-Chunker` + `Accept-Digest`)。

| Компонент | Статус | Примечания |
|----------|----------|---------------|
| `sorafs_manifest_chunk_store` | ✅ Поддерживается | Валидирует канонический ハンドル + алиасы、стримит отчеты через `--json-out=-` および применяет チャーター реестра через `ensure_charter_compliance()`。 |
| `sorafs_fetch` (開発者オーケストレーター) | ✅ Поддерживается | `chunk_fetch_specs`、ペイロードは `range` と CARv2 をサポートします。 |
| SDK フィクスチャ (Rust/Go/TS) | ✅ Поддерживается | Перегенерированы через `export_vectors`;市議会の封筒を処理します。 |
| Torii ゲートウェイとのネゴシエーション | ✅ Поддерживается | Реализует полную грамматику `Accept-Chunker`、включает заголовки `Content-Chunker` и открывает CARv1 ブリッジ только по явнымダウングレードします。 |

説明:- **フェッチ чанков** — CLI Iroha `sorafs toolkit pack` ダイジェスト、自動車、ダッシュボードの PoR 取得。
- **プロバイダー広告** — ペイロード広告、機能およびエイリアス。 `/v1/sorafs/providers` (機能 `range`) です。
- **Мониторинг ゲートウェイ** — ダウングレード `Content-Chunker`/`Content-Digest`、ダウングレード。 ожидается、что использование 橋を渡ってください。

Политика депрекации: как только утвержден профиль-преемник, запланируйте окно двойной публикации
CARv1 ブリッジと本番ゲートウェイ。

PoR を使用して、チャンク/セグメント/リーフを確認し、必要な情報を取得します。
証拠の詳細:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof=0:0:0 --por-proof-out=leaf.proof.json
```

ID (`--profile-id=1`) とハンドルを取得します。
(`--profile=sorafs.sf1@1.0.0`);ハンドル удобен для скриптов, которые
名前空間/名前/サーバーの名前とガバナンス メタデータ。

Используйте `--promote-profile=<handle>` для вывода JSON-блока метаданных (включая все)
зарегистрированные алиасы), который можно вставить в `chunker_registry_data.rs` при
必要な情報:

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
    --promote-profile=sorafs.sf1@1.0.0
```

Основной отчет (и необязательный файл 証明) включает корневой ダイジェスト、байты выбранного 葉
(16 進数) および兄弟ダイジェスト сегмента/чанка、чтобы верификаторы могли пересчитать ハッシュ слоев
64 KiB/4 KiB относительно значения `por_root_hex`。

ペイロードの証明、ペイロードの確認
`--por-proof-verify` (CLI добавляет `"por_proof_verified": true`, когда свидетель)
必要な情報):

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-proof-verify=leaf.proof.json
```

`--por-sample=<count>` はシード/ファイルを取得します。
CLI は、детерминированный порядок (`splitmix64` シード) および усечет запрос、
結果は次のとおりです。

```
$ cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- ./docs.tar \
    --por-sample=8 --por-sample-seed=0xfeedface --por-sample-out=por.samples.json
```
```

Manifest stub отражает те же данные, что удобно при скриптинге выбора `--chunker-profile-id`
в пайплайнах. Оба chunk store CLI также принимают канонический формат handle
(`--profile=sorafs.sf1@1.0.0`), поэтому build-скрипты могут избежать жесткого
хардкода числовых ID:

```
$ Cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- --list-chunker-profiles
[
  {
    「プロファイルID」: 1、
    "名前空間": "sorafs",
    "名前": "sf1",
    "semver": "1.0.0",
    "ハンドル": "sorafs.sf1@1.0.0",
    "min_size": 65536、
    「ターゲットサイズ」: 262144、
    "最大サイズ": 524288、
    "ブレークマスク": "0x0000ffff",
    「マルチハッシュコード」: 31
  }
】
```

Поле `handle` (`namespace.name@semver`) совпадает с тем, что CLIs принимают через
`--profile=…`, поэтому его можно безопасно копировать в автоматизацию.

### Согласование chunker

Gateways и клиенты объявляют поддерживаемые профили через provider adverts:

```
ProviderAdvertBodyV1 {
    ...
    chunk_profile: profile_id (неявно через реестр)
    能力: [...]
}
```

Планирование multi-source чанков объявляется через capability `range`. CLI принимает ее с
`--capability=range[:streams]`, где опциональный числовой суффикс кодирует предпочтительную
параллельность range-fetch у провайдера (например, `--capability=range:64` объявляет бюджет 64 streams).
Когда суффикс отсутствует, потребители возвращаются к общему hint `max_streams`, опубликованному в другом
месте advert.

При запросе CAR-данных клиенты должны отправлять заголовок `Accept-Chunker`, перечисляя кортежи
`(namespace, name, semver)` в порядке предпочтения:

```

ゲートウェイ ソリューション (バージョン 18NI00000076X) および отражают
резение в ответном заголовке `Content-Chunker`.マニフェストを表示する
ダウンストリーム メッセージを受信し、HTTP メッセージを受信します。

### Совместимость CAR

CARv1+SHA-2 の結果:

* **Основной путь** – CARv2、BLAKE3 ペイロード ダイジェスト (`0x1f` マルチハッシュ)、
  `MultihashIndexSorted`、チャンクが見つかりません。
  МОГУТ выдавать этот вариант, когда клиент опускает `Accept-Chunker` или запрасивает
  `Accept-Digest: sha2-256`。

要約すると、ダイジェスト版が表示されます。

### Соответствие

* Профиль `sorafs.sf1@1.0.0` のフィクスチャ в
  `fixtures/sorafs_chunker` и corpora、зарегистрированным в
  `fuzz/sorafs_chunker`。 Rust、Go、Node のエンドツーエンドの機能
  Їерез предоставленные тесты。
* `chunker_registry::lookup_by_profile` подтверждает、что параметры дескриптора
  совпадают с `ChunkProfile::DEFAULT`、чтобы защититься от случайной дивергенции.
* マニフェスト、`iroha app sorafs toolkit pack` および `sorafs_manifest_stub`、включают метаданные реестра。