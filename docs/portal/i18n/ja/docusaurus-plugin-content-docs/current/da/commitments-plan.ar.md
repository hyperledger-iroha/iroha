---
lang: ja
direction: ltr
source: docs/portal/docs/da/commitments-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::メモ
`docs/source/da/commitments_plan.md`。 بق النسختين متزامنتين حتى يتم سحب
そうです。
:::

# خطة تعهدات توفر البيانات في Sora Nexus (DA-3)

_重要: 2026-03-25 -- 重要: コア プロトコル WG / スマート コントラクト チーム / ストレージ チーム_

DA-3 テスト Nexus テスト レーン テスト ブロブ テスト
マリナDA-2。 هضح هذه المذكرة هياكل البيانات القياسية، وصلات خط انابيب الكتل،
برهان العملاء الخفيفين، واسطح Torii/RPC التكتمل قبل ان يعتمد
DA اثناء فحوصات القبول او الحوكمة 。認証済み
Noritoスケールと JSON のサイズ。

## ああ

- blob (チャンク ルート + マニフェスト ハッシュ + KZG اختياري) のブロック
  Nexus の在庫状況と在庫状況を確認してください。
  ありがとうございます。
- マニフェスト ハッシュのマニフェスト ハッシュ
  そうです。
- Torii (`/v2/da/commitments/*`) リレーと SDK
  空き状況を確認してください。
- حفاظ على ظرف `SignedBlockWire` القياسي عبر تمرير البنى الجديدة من خلال
  Norito ハッシュ値。

## ナオミ

1. **اضافات نموذج البيانات** في `iroha_data_model::da::commitment` مع تغيرات
   `iroha_data_model::block` です。
2. **フック للمنفذ** حتى يقوم `iroha_core` بابتلاع 領収書 الخاصة بـ DA
   Torii (`crates/iroha_core/src/queue.rs` و)
   `crates/iroha_core/src/block.rs`)。
3. **永続化/インデックス** WSV の保存と保存
   (`iroha_core/src/wsv/mod.rs`)。
4. **RPC 番号 Torii** リスト/クエリ/証明 `/v2/da/commitments`。
5. **セキュリティ + フィクスチャ** ワイヤ レイアウトの証明
   `integration_tests/tests/da/commitments.rs`。

## 1. ああ、

### 1.1 `DaCommitmentRecord`

```rust
/// Canonical record stored on-chain and inside SignedBlockWire.
pub struct DaCommitmentRecord {
    pub lane_id: LaneId,
    pub epoch: u64,
    pub sequence: u64,
    pub client_blob_id: BlobDigest,
    pub manifest_hash: ManifestDigest,        // BLAKE3 over DaManifestV1 bytes
    pub proof_scheme: DaProofScheme,          // lane policy (merkle_sha256 or kzg_bls12_381)
    pub chunk_root: Hash,                     // Merkle root of chunk digests
    pub kzg_commitment: Option<KzgCommitment>,
    pub proof_digest: Option<Hash>,           // hash of PDP/PoTR schedule
    pub retention_class: RetentionClass,      // mirrors DA-2 retention policy
    pub storage_ticket: StorageTicketId,
    pub acknowledgement_sig: Signature,       // Torii DA service key
}
```

- `KzgCommitment` يعيد استخدام النقطة ذات 48 بايت الموجودة في
  `iroha_crypto::kzg`。マークル氏。
- `proof_scheme` レーン数レーン マークル ترفض حمولات KZG،
  レーン `kzg_bls12_381` は、KZG のレーンです。 Torii حاليا ينتج
  マークルは KZG レーンを通過します。
- `KzgCommitment` يعيد استخدام النقطة ذات 48 بايت الموجودة في
  `iroha_crypto::kzg`。マークル عود الى براهين マークル فقط。
- `proof_digest` 問題 DA-5 PDP/PoTR 問題
  ブロブが存在します。

### 1.2 の評価

```
pub struct BlockHeader {
    ...
    pub da_commitments_hash: Option<HashOf<DaCommitmentBundle>>,
}

pub struct DaCommitmentBundle {
    pub version: u16,                // start with 1
    pub commitments: Vec<DaCommitmentRecord>,
}
```

ハッシュ値は `SignedBlockWire` です。ああ

重要: `BlockPayload` و`BlockBuilder` الشفاف يعرضان الان
セッター/ゲッター `da_commitments` (`BlockBuilder::set_da_commitments`)
و`SignedBlock::set_da_commitments`) は、ホストをホストします。
ありがとうございます。 جميع المساعدة تترك الحقل `None` حتى يمر Torii
やあ、やあ。

### 1.3 ワイヤー- `SignedBlockWire::canonical_wire()` يضيف ترويسة Norito لـ
  `DaCommitmentBundle` مباشرة بعد قائمة المعاملات الحالية。ありがとうございます
  `0x01`。
- `SignedBlockWire::decode_wire()` يرفض الحزم ذات `version` غير معروفة، بما
  يتوافق مع سياسة Norito الموضحة في `norito.md`。
- ハッシュ موجودة فقط في `block::Hasher`; और देखें
  ワイヤ形式 يحصلون تلقائيا على الحقل الجديد لان ترويسة Norito
  そうです。

## 2. いいえ

1. インジェスト الخاصة بـ Torii DA ايصال `DaIngestReceipt` وتقوم بنشره
   (`iroha_core::gossiper::QueueMessage::DaReceipt`)。
2. يجمع `PendingBlocks` كل 領収書 التي تطابق `lane_id` للكتلة قيد البناء،
   `(lane_id, client_blob_id, manifest_hash)` です。
3. ビルダー ビルダー `(lane_id, epoch, sequence)`
   ハッシュ حتمي، ويقوم بترميز الحزمة باستخدام Norito، ويحدث
   `da_commitments_hash`。
4. يتم تخزين الحزمة كاملة في WSV وتصدر مع الكتلة داخل `SignedBlockWire`.

領収書を受け取る 領収書を受け取る 領収書を受け取る
ああビルダー `sequence` レーンのリプレイ。

## 3. RPC を使用する

Torii エンドポイント:

|名前: और देखेंああ |重要 |
|----------|-----------|----------|-----------|
| `/v2/da/commitments` | `POST` | `DaCommitmentQuery` (レーン/エポック/シーケンス、ページネーション) | يعيد `DaCommitmentPage` は、ハッシュ الكتلة を意味します。 |
| `/v2/da/commitments/prove` | `POST` | `DaCommitmentProofRequest` (レーン + マニフェスト ハッシュ タプル `(epoch, sequence)`)。 | يعيد `DaCommitmentProof` (レコード + メルクル + ハッシュ)。 |
| `/v2/da/commitments/verify` | `POST` | `DaCommitmentProof` |ステートレス يعيد حساب ハッシュ الكتلة ويتحقق من الاشتمال؛ SDK は `iroha_crypto` に対応しています。 |

كل الحمولات تعيش تحت `iroha_data_model::da::commitment`. Torii 認証済み
ハンドラーは、エンドポイントでトークン/mTLS を取り込みます。

## 4. ジャイアンツの世界

- يبني منتج الكتل شجرة Merkle ثنائية فوق قائمة `DaCommitmentRecord`
  そうです。 `da_commitments_hash`。
- يحزم `DaCommitmentProof` السجل المستهدف مع متجه من `(sibling_hash, position)`
  重要な問題は次のとおりです。ハッシュ ハッシュ
  ファイナリティを追求します。
- 評価 CLI (`iroha_cli app da prove-commitment`) 評価/評価
  Norito/hex は、Norito/hex です。

## 5. いいえ

WSV は、列ファミリー مخصصة بمفتاح `manifest_hash` です。翻訳
`(lane_id, epoch)` و`(lane_id, sequence)` كي تتجنب الاستعلامات
そうです。 يتتبع كل سجل ارتفاع الكتلة التي ختمته، مما يسمح للعقد في
追いつく、追いつく、追いつく、追いつく、追いつく、追いつく、追いつく、追いつく、追いつく、追いつく。

## 6. いいえ

- `torii_da_commitments_total` يزيد عند ختم كتلة تحتوي على سجل واحد على الاقل。
- `torii_da_commitment_queue_depth` يتتبع 領収書 المنتظرة للتجميع (لكل レーン)。
- ログイン Grafana `dashboards/grafana/da_commitments.json` ログイン
  DA-3 のスループットは最高です。

## 7. いいえ1. **اختبارات وحدات** لترميز/فك ترميز `DaCommitmentBundle` وتحديثات اشتقاق ハッシュ
   ああ。
2. **フィクスチャ ゴールデン** バイト数 `fixtures/da/commitments/` バイト数
   マークル氏。
3. **اختبارات تكامل** تشغل مدققين اثنين، وتبتلع blob تجريبية، وتتحقق من ان
   كلا العقدتين تتفقان على محتوى الحزمة واستجابات الاستعلام/البرهان.
4. **اختبارات عملاء خفيفين** في `integration_tests/tests/da/commitments.rs`
   (Rust) تستدعي `/prove` وتتحقق من البرهان دون التحدث الى Torii.
5. **Smoke CLI** عبر `scripts/da/check_commitments.sh` لابقاء ادوات المشغلين
   ありがとうございます。

## 8. いいえ

|ログイン | ログインああ |ログイン | ログイン
|----------|----------|--------------|
| P0 - 認証済み | 認証済みدمج `DaCommitmentRecord` وتحديثات ترويسة الكتلة وكودكات Norito。 | `cargo test -p iroha_data_model` はフィクスチャを備えています。 |
| P1 - コア/WSV |ブロック ビルダー + ブロック ビルダー RPC ハンドラー。 | `cargo test -p iroha_core` と `integration_tests/tests/da/commitments.rs` のバンドル プルーフ。 |
| P2 - 世界遺産 |ヘルパーは CLI を使用して Grafana を証明します。 | `iroha_cli app da prove-commitment` 開発ネットありがとうございます。 |
| P3 - ニュース |レーン数は `iroha_config::nexus` です。 | DA-3 のステータスとロードマップ。 |

## और देखें

1. **KZG 対 Merkle のデフォルト** - KZG のブロブとマークルの比較
   ああ、回答: `kzg_commitment` 回答
   `iroha_config::da.enable_kzg`。
2. **配列ギャップ** - 問題を解決する翻訳: 翻訳: 翻訳: 翻訳: 翻訳
   `allow_sequence_skips` は、次のとおりです。
3. **ライト クライアント キャッシュ** - SDK の SQLite バージョンすごい
   DA-8 です。

الاجابة على هذه الاسئلة في PRs التنفيذ تنقل DA-3 من مسودة (هذه الوثيقة) الى
重要な問題は、次のとおりです。