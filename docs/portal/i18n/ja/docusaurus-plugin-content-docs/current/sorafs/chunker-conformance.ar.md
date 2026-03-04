---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-conformance.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: チャンカー準拠
タイトル: دليل مطابقة チャンカー في SoraFS
サイドバーラベル: مطابقة チャンカー
説明: SF1 フィクスチャと SDK のチャンカー。
---

:::note ノート
テストは `docs/source/sorafs/chunker_conformance.md` です。 حرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف الوثائق القديمة。
:::

يوثق هذا الدليل المتطلبات التي يجب على كل تطبيق اتباعها للبقاء متوافقاً مع ملف chunker الحتمي SoraFS (SF1)。
フィクスチャ SDK のダウンロード重要です。

## ああ、

- メッセージ: `sorafs.sf1@1.0.0` (`sorafs.sf1@1.0.0`)
- 数値 (16 進数): `0000000000dec0ded`
- バイト数: 262144 バイト (256 KiB)
- サイズ: 65536 バイト (64 KiB)
- サイズ: 524288 バイト (512 KiB)
- メッセージ: `0x3DA3358B4DC173`
- ギア: `sorafs-v1-gear`
- 番号: `0x0000FFFF`

番号: `sorafs_chunker::chunk_bytes_with_digests_profile`。
SIMD のダイジェストをご覧ください。

## 備品

`cargo run --locked -p sorafs_chunker --bin export_vectors` 評価
備品 `fixtures/sorafs_chunker/`:

- `sf1_profile_v1.{json,rs,ts,go}` — チャンク、Rust、TypeScript、Go。
  يعلن كل ملف المقبض المعتمد كأول إدخال في `profile_aliases`، يتبعه أي بدائل قديمة (مثل
  `sorafs.sf1@1.0.0` と `sorafs.sf1@1.0.0`)。ログインしてください。
  `ensure_charter_compliance` です。
- `manifest_blake3.json` — マニフェストの BLAKE3 フィクスチャ。
- `manifest_signatures.json` — テスト (Ed25519) マニフェストのダイジェスト。
- `sf1_profile_v1_backpressure.json` コーパス الخام داخل `fuzz/` —
  バックプレッシャーのチャンカー。

### 認証済み

試合は、試合の日程を決定します。 और देखें
الإخراج غير الموقّع ما لم يتم تمرير `--allow-unsigned` صراحة (مخصص)
جارب المحلية فقط)。追加専用です。

重要な情報:

```bash
cargo run --locked -p sorafs_chunker --bin export_vectors \
  --signing-key=<ed25519-private-key-hex> \
  --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
```

## ありがとう

يعيد مساعد CI `ci/check_sorafs_fixtures.sh` تشغيل المولد مع
`--locked`。試合の試合結果を確認します。ああ
ワークフローとフィクスチャ。

回答:

1. `cargo test -p sorafs_chunker`。
2. `ci/check_sorafs_fixtures.sh` محلياً。
3. 評価 `git status -- fixtures/sorafs_chunker` 。

## دليل الترقية

SF1 のチャンカー:

回答: [`docs/source/sorafs/chunker_profile_authoring.md`](./chunker-profile-authoring.md)
ログインしてください。

1. `ChunkProfileUpgradeProposalV1` (RFC SF-1) を参照してください。
2. フィクスチャ `export_vectors` ダイジェスト マニフェスト。
3. マニフェストを作成する。 يجب إلحاق كل التواقيع بـ `manifest_signatures.json`。
4. フィクスチャと SDK (Rust/Go/TS) の組み合わせ。
5. コーパスファズを分析する。
6. ダイジェスト。
7. ロードマップ。

チャンク ダイジェスト ダイジェスト チャンク ダイジェスト
すごいです。