---
lang: ja
direction: ltr
source: docs/portal/docs/da/ingest-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
Эта страница отражает `docs/source/da/ingest_plan.md`. жержите обе версии
:::

# 取得データの可用性 Sora Nexus

_Черновик: 2026-02-20 — Владельцы: コア プロトコル WG / ストレージ チーム / DA WG_

DA-2 から Torii API を取り込み、BLOB を取得します。
Norito-метаданные и запускает репликацию SoraFS。 Документ описывает предложенную
схему、API-область и поток валидации, чтобы реализация зла без блокировки на
ожидающих симуляциях (フォローアップ DA-1)。ペイロードを取得する
Norito; serde/JSON のフォールバック。

## Цели

- ブロブ (сегменты Taikai、サイドカー レーン、артефакты управления)
  детерминированно через Torii。
- Создавать канонические Norito マニフェスト、blob、コーデック、
  消去と保持の両方を備えています。
- チャンクとホット ストレージ SoraFS の接続を確認します。
  очередь。
- ピン インテント + ポリシー タグ、SoraFS および наблюдателям
  そうです。
- 入場証、чтобы клиенты получали детерминированное
  ждтверждение публикации。

## API サーフェス (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

ペイロード — это `DaIngestRequest`、закодированный Norito。 Ответы используют
`application/norito+v1` と `DaIngestReceipt`。

| Ответ | Значение |
| --- | --- |
| 202 承認済み | BLOB はチャンク/レプリケーションに対応します。領収書を受け取ります。 |
| 400 不正なリクエスト |スキーマ/размера (см.проверки)。 |
| 401 不正 | Отсутствует/некорректен API-токен. |
| 409 紛争 | Дубликат `client_blob_id` с несовпадающей метаданной. |
| 413 ペイロードが大きすぎます |ブロブのようなもの。 |
| 429 リクエストが多すぎます |レート制限を解除します。 |
| 500 内部エラー | Неожиданная озибка (лог + алерт)。 |

## Предлагаемая Norito схема

```rust
/// Top-level ingest request.
pub struct DaIngestRequest {
    pub client_blob_id: BlobDigest,      // submitter-chosen identifier
    pub lane_id: LaneId,                 // target Nexus lane
    pub epoch: u64,                      // epoch blob belongs to
    pub sequence: u64,                   // monotonic sequence per (lane, epoch)
    pub blob_class: BlobClass,           // TaikaiSegment, GovernanceArtifact, etc.
    pub codec: BlobCodec,                // e.g. "cmaf", "pdf", "norito-batch"
    pub erasure_profile: ErasureProfile, // parity configuration
    pub retention_policy: RetentionPolicy,
    pub chunk_size: u32,                 // bytes (must align with profile)
    pub total_size: u64,
    pub compression: Compression,        // Identity, gzip, deflate, or zstd
    pub norito_manifest: Option<Vec<u8>>, // optional pre-built manifest
    pub payload: Vec<u8>,                 // raw blob data (<= configured limit)
    pub metadata: ExtraMetadata,          // optional key/value metadata map
    pub submitter: PublicKey,             // signing key of caller
    pub signature: Signature,             // canonical signature over request
}

pub enum BlobClass {
    TaikaiSegment,
    NexusLaneSidecar,
    GovernanceArtifact,
    Custom(u16),
}

pub struct ErasureProfile {
    pub data_shards: u16,
    pub parity_shards: u16,
    pub chunk_alignment: u16, // chunks per availability slice
    pub fec_scheme: FecScheme,
}

pub struct RetentionPolicy {
    pub hot_retention_secs: u64,
    pub cold_retention_secs: u64,
    pub required_replicas: u16,
    pub storage_class: StorageClass,
    pub governance_tag: GovernanceTag,
}

pub struct ExtraMetadata {
    pub items: Vec<MetadataEntry>,
}

pub struct MetadataEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub visibility: MetadataVisibility, // public vs governance-only
}

pub enum MetadataVisibility {
    Public,
    GovernanceOnly,
}

pub struct DaIngestReceipt {
    pub client_blob_id: BlobDigest,
    pub lane_id: LaneId,
    pub epoch: u64,
    pub blob_hash: BlobDigest,          // BLAKE3 of raw payload
    pub chunk_root: BlobDigest,         // Merkle root after chunking
    pub manifest_hash: BlobDigest,      // Norito manifest hash
    pub storage_ticket: StorageTicketId,
    pub pdp_commitment: Option<Vec<u8>>,     // Norito-encoded PDP bytes
    #[norito(default)]
    pub stripe_layout: DaStripeLayout,   // total_stripes, shards_per_stripe, row_parity_stripes
    pub queued_at_unix: u64,
    #[norito(default)]
    pub rent_quote: DaRentQuote,        // XOR rent + incentives derived from policy
    pub operator_signature: Signature,
}
```

> Примечание по реализации: канонические Rust-представления этих payload теперь
> находятся в `iroha_data_model::da::types`、リクエスト/レシート ラッパー в
> `iroha_data_model::da::ingest` и структурой マニフェスト в
> `iroha_data_model::da::manifest`。

`compression` は、呼び出し元のペイロードを記録します。 Torii です
`identity`、`gzip`、`deflate` および `zstd`、ハッシュ化、
チャンク化と проверкой опциональных が現れます。

### Чеклист валидации1. Проверить、что заголовок Norito соответствует `DaIngestRequest`.
2. Олибка、если `total_size` отличается от канонической длины ペイロード
   (после распаковки) или превылает настроенный максимум.
3. Принудить выравнивание `chunk_size` (最低 двойки, <= 2 MiB)。
4. Убедиться, что `data_shards + parity_shards` <= глобального максимума и
   パリティ >= 2。
5. `retention_policy.required_replica_count` ガバナンス ベースライン。
6. Проверка подписи по каноническому ハッシュ (без поля подписи)。
7. Отклонять дубликаты `client_blob_id`、ハッシュ ペイロードとメッセージ
   と。
8. При наличии `norito_manifest` проверить schema + hash на совпадение с
   マニフェスト、チャンク化。マニフェスト иначе узел генерирует
   Похраняет его。
9. Применять настроенную политику репликации: Torii переписывает отправленный
   `RetentionPolicy` через `torii.da_ingest.replication_policy` (最低。
   `replication-policy.md`) и отклоняет заранее созданные マニフェスト、если их
   保持方法は強制的に保持されます。

### チャンク化と репликации

1. ペイロード - `chunk_size`、BLAKE3 для каждого チャンク + Merkle
   根。
2. Сформировать Norito `DaManifestV1` (новая struct)、фиксируя コミットメントチャンク
   (role/group_id)、消去レイアウト (числа паритета строк и столбцов плюс)
   `ipa_commitment`)、保持期間と方法。
3. バイトマニフェストを確認する
   `config.da_ingest.manifest_store_dir` (Torii は `manifest.encoded` です)
   レーン/エポック/シーケンス/チケット/指紋)、чтобы оркестрация SoraFS могла
   ストレージチケットを保存してください。
4. ピンのインテント через `sorafs_car::PinIntent` с тегом управления и
   。
5. Эмитировать событие Norito `DaIngestPublished` для оповещения наблюдателей
   (ライトクライアント、ガバナンス、分析)。
6. `DaIngestReceipt` 呼び出し元 (подписан ключом Torii DA) および отправлять
   `Sora-PDP-Commitment`、SDK へのコミットメント。領収書
   `rent_quote` (Norito `DaRentQuote`) および `stripe_layout`、と表示されます。
   отправители могли показывать базовую аренду, долю резерва, ожидания бонусов
   PDP/PoTR および 2D 消去レイアウトは、ストレージ チケットを必要とします。

## ストレージ/レジストリ

- Распечив детерминированный `sorafs_manifest` новым `DaManifestV1`, обеспечив детерминированный
  ぱん。
- レジストリ ストリーム `da.pin_intent` のペイロード、
  マニフェスト ハッシュ + チケット ID です。
- 可観測性 - 取り込み、スループットの確認
  チャンク化、バックログ、счетчиков обок。

## Стратегия тестирования- ユニット-тесты для валидации スキーマ、проверки подписей、детекта дубликатов。
- Golden-тесты для проверки Norito エンコード `DaIngestRequest`、マニフェストとレシート。
- Интеграционный ハーネス、поднимающий モック SoraFS + レジストリ и проверяющий
  チャンク + ピン。
- プロパティの削除と保持。
- Norito ペイロードをファジングします。

## CLI および SDK ツール (DA-8)- `iroha app da submit` (CLI エントリポイント) оборачивает общий 取り込みビルダー/
  パブリッシャー、ブロブを取り込み、ブロブを取得します。
  タイカイバンドル。 Команда находится в `crates/iroha_cli/src/commands/da.rs:1` и
  ペイロード、消去/保持、およびそれらの機能
  メタデータ/マニフェスト перед подписью канонического `DaIngestRequest` ключом
  CLI です。 Успезные запуски сохраняют `da_request.{norito,json}` и
  `da_receipt.{norito,json}` および `artifacts/da/submission_<timestamp>/`
  (オーバーライド через `--artifact-dir`)、アーティファクト чтобы релизные точные
  Norito バイト、取り込みます。
- Команда по умолчанию использует `client_blob_id = blake3(payload)`, но
  `--client-blob-id`、JSON クラスのメタデータをオーバーライドします。
  (`--metadata-json`) および事前生成されたマニフェスト (`--manifest`)、также
  `--no-submit` для офлайн подготовки と `--endpoint` для кастомных Torii хостов.
  受信JSON выводится в stdout и пизется на диск、закрывая требование DA-8
  「submit_blob」と SDK パリティの組み合わせ。
- `iroha app da get` добавляет DA-ориентированный 別名 для マルチソース オーケストレーター、
  `iroha app sorafs fetch` を参照してください。アーティファクトを作成する
  マニフェスト + チャンク プラン (`--manifest`、`--plan`、`--manifest-id`) **или** シート
  Torii ストレージ チケット `--storage-ticket`。チケット CLI を使用する
  マニフェストと `/v2/da/manifests/<ticket>`、バンドルの組み合わせ
  `artifacts/da/fetch_<timestamp>/` (`--manifest-cache-dir` をオーバーライド)、
  BLOB ハッシュ `--manifest-id` とオーケストレーターの組み合わせ
  `--gateway-provider` 最低。 SoraFS フェッチャーのノブを使用します。
  сохраняются (マニフェスト エンベロープ、クライアント ラベル、ガード キャッシュ、匿名
  トランスポート オーバーライド、スコアボード、`--output` など)、マニフェスト エンドポイント
  можно переопределить через `--manifest-endpoint` для кастомных Torii хостов,
  エンドツーエンドの可用性の確認と名前空間 `da` の確認
  дублирования オーケストレーター логики。
- `iroha app da get-blob` забирает канонические マニフェスト напрямую из Torii через
  `GET /v2/da/manifests/{storage_ticket}`。 Команда Команда пизет
  `manifest_{ticket}.norito`、`manifest_{ticket}.json`、`chunk_plan_{ticket}.json`
  в `artifacts/da/fetch_<timestamp>/` (`--output-dir`)、
  этом выводит точную команду `iroha app da get` (включая `--manifest-id`), нужную
  オーケストレーターをフェッチします。 Это избавляет операторов от работы с
  マニフェスト スプール メッセージ、フェッチャー メッセージ、フェッチャー メッセージ
  アーティファクト Torii。 JavaScript クラス Torii は、 этот поток через
  `ToriiClient.getDaManifest(storageTicketHex)`、Norito を表示します
  バイト、マニフェスト JSON およびチャンク プラン、SDK 呼び出し元のメッセージ
  オーケストレーターは CLI を使用します。 Swift SDK のダウンロード
  (`ToriiClient.getDaManifestBundle(...)` и `fetchDaPayloadViaGateway(...)`)、
  バンドルとネイティブ ラッパー SoraFS オーケストレーター、iOS クラスの機能
  マニフェスト、マルチソースフェッチ、およびマルチソースフェッチの実行
  CLI を使用します。[IrohaSwift/ソース/IrohaSwift/ToriiClient.swift:240][IrohaSwift/ソース/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` 家賃とインセンティブを受け取る
  保存と保持。 Хелпер потребляет активную
  `DaRentPolicyV1` (JSON または Norito バイト) デフォルトの値、валидирует
  JSON-сводку (`gib`、`months`、ポリシー メタデータとファイル)
  `DaRentQuote`)、XOR 料金を計算します。
  アドホックな機能を備えています。 Команда также печатает однострочный
  `rent_quote ...` は、JSON ペイロードをドリルします。
  Свяжите `--quote-out artifacts/da/rent_quotes/<stamp>.json` с
  `--policy-label "governance ticket #..."`、アーティファクトを取得します
  設定バンドルをインストールします。 CLI を使用してください。
  метку и отвергает пустые строки, чтобы `policy_source` оставался пригодным для
  дабордов。 См. `crates/iroha_cli/src/commands/da.rs` と、
  `docs/source/da/rent_policy.md` が表示されます。
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` ストレージ チケット、
  マニフェスト バンドル、マルチソース オーケストレーター
  (`iroha app sorafs fetch`) `--gateway-provider` を確認してください。
  ペイロード + スコアボード `artifacts/da/prove_availability_<timestamp>/`、
  および PoR ヘルパー (`iroha app da prove`) を使用してください。
  バイト。オーケストレーター ノブを使用する (`--max-peers`、
  `--scoreboard-out`、マニフェスト エンドポイント オーバーライド) およびプルーフ サンプラー
  (`--sample-count`, `--leaf-index`, `--sample-seed`)、 при этом одна команда
  アーティファクト、DA-5/DA-9 のペイロード、ビデオ
  スコアボードと JSON の証明。