---
lang: ja
direction: ltr
source: docs/portal/docs/da/ingest-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::メモ
`docs/source/da/ingest_plan.md`。 بق النسختين متزامنتين حتى يتم سحب
そうです。
:::

# خطة ingest لتوفر البيانات في Sora Nexus

_日: 2026-02-20 - 日: コア プロトコル WG / ストレージ チーム / DA WG_

DA-2 は Torii は BLOB を取り込みます Norito は Blob を取り込みます
SoraFS。 يوثق هذا المستند المخطط المقترح وسطح API وتدفق التحقق حتى
DA-1 をダウンロードしてください。 يجب ان تستخدم جميع تنسيقات
回答 Noritoフォールバックは serde/JSON です。

## ああ

- ブロブ كبيرة (قطاعات Taikai، サイドカー للحارات، وادوات حوكمة) بشكل حتمي
  Torii。
- マニフェスト Norito のブロブ コーデックの消去
  ありがとうございます。
- チャンク الوصفية في تخزين SoraFS الساخن ووضع مهام التكرار في
  ああ。
- PIN + وسوم السياسة في سجل SoraFS ومراقبي الحوكمة.
- 領収書を受け取ることができます。

## API (Torii)

```
POST /v2/da/ingest
Content-Type: application/norito+v1
```

`DaIngestRequest` 、Norito です。ありがとうございます
`application/norito+v1` وتعيد `DaIngestReceipt`。

| और देखेंああ |
| --- | --- |
| 202 承認済み | تم وضع الـ blob في طابور التجزئة/التكرار؛領収書。 |
| 400 不正なリクエスト | انتهاك スキーマ/الحجم (انظر فحوصات التحقق)。 |
| 401 不正 | API を使用してください。 |
| 409 紛争 | `client_blob_id` を確認してください。 |
| 413 ペイロードが大きすぎます |は、ブロブをブロックします。 |
| 429 リクエストが多すぎます |レート制限。 |
| 500 内部エラー | فشل غير متوقع (log + تنبيه)。 |

## مخطط Norito المقترح

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

> ملاحظة تنفيذية: تم نقل التمثيلات Rust الهذه الحمولات الى
> `iroha_data_model::da::types`، مع ラッパー للطلب/領収書 في
> `iroha_data_model::da::ingest` マニフェスト `iroha_data_model::da::manifest`。

`compression` を確認してください。 Torii `identity` و`gzip`
و`deflate` و`zstd` マニフェストのハッシュ化
ああ。

### قائمة تحقق التحقق

1. Norito は `DaIngestRequest` です。
2. افشل اذا كان `total_size` مختلفا عن طول الحمولة القانوني (بعد فك الضغط) و ا
   ありがとうございます。
3. فرض محاذاة `chunk_size` (قوة اثنين، <= 2 MiB)。
4. `data_shards + parity_shards` <= パリティ >= 2。
5. يجب ان يحترم `retention_policy.required_replica_count` خط اساس الحوكمة.
6. تحقق التوقيع مقابل الهاش القياسي (مع استبعاد حقل التوقيع)。
7. رفض `client_blob_id` المكرر ما لم تكن قيمة هاش الحمولة والبيانات الوصفية
   そうです。
8. スキーマ + マニフェストのハッシュ `norito_manifest`
   حسابه بعد التجزئة؛マニフェストを作成します。
9. 評価: يعيد Torii كتابة `RetentionPolicy` عبر
   `torii.da_ingest.replication_policy` (`replication-policy.md`)
   マニフェストは、次のことを示します。

### 評価してください1. BLAKE3 チャンク + マークル。
2. Norito `DaManifestV1` (構造体) チャンク
   (role/group_id)، وتخطيط 消去 (اعداد تكافؤ الصفوف والاعمدة مع
   `ipa_commitment`) を確認してください。
3. バイト、マニフェスト、`config.da_ingest.manifest_store_dir`
   (レーン/エポック/シーケンス/チケット/フィンガープリント)
   SoraFS ストレージ チケットを保存してください。
4. نشر نوايا pin عبر `sorafs_car::PinIntent` مع وسم الحوكمة والسياسة.
5. بث حدث Norito `DaIngestPublished` لاخطار المراقبين (عملاء خفيفين، الحوكمة،
   ）。
6. ارجاع `DaIngestReceipt` للمتصل (موقع بمفتاح خدمة Torii DA) وارسال ترويسة
   `Sora-PDP-Commitment` は SDK をサポートしています。領収書
   `rent_quote` (Norito `DaRentQuote`) و`stripe_layout` ログイン
   PDP/PoTR を使用してください。
   消去と保存チケットの保存。

## 評価/評価

- `sorafs_manifest` `DaManifestV1` を解析します。
- ストリーム メッセージ `da.pin_intent` メッセージ メッセージ ハッシュ
  マニフェスト + チケット ID。
- 評価と分析、取り込みとスループットの評価
  ありがとうございます。

## いいえ

- スキーマを定義する必要があります。
- ゴールデン メッセージ Norito メッセージ `DaIngestRequest` マニフェスト 領収書。
- ハーネス SoraFS + レジストリ チャンク + ピン。
- 削除、削除、削除、削除。
- ファジング Norito メタデータのファジング。

## CLI と SDK (DA-8)- `iroha app da submit` (CLI セキュリティ) ビルダー/パブリッシャーの取り込み
  حتى يتمكن المشغلون من ادخال blob عشوائية خارج مسار Taikai バンドル。翻訳
  `crates/iroha_cli/src/commands/da.rs:1` وتستهلك حمولة وملف
  消去/保持 メタデータ/マニフェスト
  `DaIngestRequest` CLI です。翻訳する
  `da_request.{norito,json}` و`da_receipt.{norito,json}` 評価
  `artifacts/da/submission_<timestamp>/` (`--artifact-dir` をオーバーライド)
  アーティファクト バイト Norito を取り込みます。
- `client_blob_id = blake3(payload)` によるオーバーライド
  `--client-blob-id` メタデータ JSON (`--metadata-json`)
  マニフェスト (`--manifest`) と `--no-submit` オフライン メッセージ
  `--endpoint` は Torii です。受信確認 JSON stdout 認証
  ツール "submit_blob" を使用して、DA-8 を使用してください。
  SDK です。
- `iroha app da get` يضيف エイリアス موجه لـ DA للمشغل متعدد المصادر الذي يشغل بالفعل
  `iroha app sorafs fetch`。アーティファクトマニフェスト + チャンクプラン
  (`--manifest`、`--plan`、`--manifest-id`) **او** ストレージ チケット من Torii
  `--storage-ticket`。 CLI のマニフェストを確認する
  `/v2/da/manifests/<ticket>`، وتخزن الحزمة تحت `artifacts/da/fetch_<timestamp>/`
  (オーバーライド `--manifest-cache-dir`) ハッシュ ブロブ `--manifest-id`
  オーケストレーターは `--gateway-provider` です。ノブ
  المتقدمة من جالب SoraFS كما هي (マニフェスト封筒) ガード
  キャッシュは、スコアボード (`--output`) をオーバーライドします。
  マニフェスト エンドポイント `--manifest-endpoint` Torii マニフェスト エンドポイント
  空室状況を確認するには、`da` を確認してください。
  オーケストレーター。
- `iroha app da get-blob` يسحب マニフェスト القياسية مباشرة من Torii عبر
  `GET /v2/da/manifests/{storage_ticket}`。やあ
  `manifest_{ticket}.norito` و`manifest_{ticket}.json` و`chunk_plan_{ticket}.json`
  تحت `artifacts/da/fetch_<timestamp>/` (او `--output-dir` يحدده المستخدم) مع
  طباعة امر `iroha app da get` الدقيق (بما في ذلك `--manifest-id`) المطلوب لجلب
  オーケストレーター。マニフェスト スプール マニフェスト スプール
  フェッチャー يستخدم دائما アーティファクト الموقعة الصادرة عن Torii。 يعكس عميل Torii
  JavaScript هذا التدفق عبر `ToriiClient.getDaManifest(storageTicketHex)`،
  バイト Norito マニフェスト JSON チャンク プラン呼び出し元 SDK
  CLI のオーケストレーターです。 SDK Swift の開発
  और देखें
  `fetchDaPayloadViaGateway(...)`) オーケストレーター SoraFS
  iOS のマニフェストをフェッチする
  CLI を使用します。
  [IrohaSwift/ソース/IrohaSwift/ToriiClient.swift:240][IrohaSwift/ソース/IrohaSwift/SorafsOrchestratorClient.swift:12]
- `iroha app da rent-quote` يحسب 家賃 حتمي وتفصيل الحوافز لحجم تخزين ونافذة احتفاظ
  すごい。 يستهلك المساعد `DaRentPolicyV1` النشط (JSON バイト Norito) الافتراضي
  JSON (`gib`、`months` 、 بيانات السياسة، )وحقول `DaRentQuote`) حتى يتمكن المدققون من الاستشهاد برسوم XOR الدقيقة في
  حاضر الحوكمة دون نصوص مخصصة。 كما يصدر الامر ملخصا من سطر واحد
  `rent_quote ...` JSON 形式の JSON 形式の形式
  最高です。 قم بقرن `--quote-out artifacts/da/rent_quotes/<stamp>.json`
  `--policy-label "governance ticket #..."` のアーティファクトを確認してください。
  ログインしてください。 CLI を使用して、アプリケーションを実行します。
  حتى تبقى قيم `policy_source` قابلة للتنفيذ في لوحات خزينة.やあ
  `crates/iroha_cli/src/commands/da.rs` ログイン
  `docs/source/da/rent_policy.md` です。
  [crates/iroha_cli/src/commands/da.rs:1][docs/source/da/rent_policy.md:1]
- `iroha app da prove-availability` يربط كل ما سبق: ياخذ storage ticket، ينزل حزمة
  マニフェスト القياسية، يشغل Orchestrator متعدد المصادر (`iroha app sorafs fetch`) مقابل
  قائمة `--gateway-provider` المعطاة، ويحفظ الحمولة التي تم تنزيلها + スコアボード
  評価 `artifacts/da/prove_availability_<timestamp>/` 評価 PoR
  (`iroha app da prove`) バイト数。ノブ
  オーケストレーター (`--max-peers`、`--scoreboard-out`、マニフェストをオーバーライドします)
  サンプラー (`--sample-count`, `--leaf-index`, `--sample-seed`)
  DA-5/DA-9 のアーティファクトの検索結果: DA-5/DA-9 の検索結果
  スコアボードは JSON です。