---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-profile-authoring.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカープロファイルオーサリング
タイトル: SoraFS チャンカー プロファイル オーサリング ガイド
Sidebar_label: チャンカー オーサリング ガイド
説明: SoraFS チャンカー プロファイル、フィクスチャ、評価、チェックリスト、
---

:::note メモ
:::

# SoraFS チャンカー プロファイル オーサリング ガイド

یہ گائیڈ وضاحت کرتی ہے کہ SoraFS کے لیے نئے チャンカー プロファイル کیسے تجویز اور 公開 کیےああ
アーキテクチャ RFC (SF-1) レジストリ リファレンス (SF-2a)
具体的なオーサリング要件、検証手順、提案書テンプレート、詳細な説明
正規の مثال کے لیے دیکھیں
`docs/source/sorafs/proposals/sorafs_sf1_profile_v1.json`
予行演習ログ
`docs/source/sorafs/reports/sf1_determinism.md` すごい

## 概要

プロフィール レジストリ میں داخل ہوتی ہے اسے یہ کرنا ہوگا:

- 決定論的な CDC パラメータ、マルチハッシュ設定、アーキテクチャ、広告、広告
- 再生可能なフィクスチャ (Rust/Go/TS JSON + ファズ コーパス + PoR 証人) ダウンストリーム SDK のカスタム ツールの検証
- 評議会レビューの決定論的差分スイートの決定

チェックリスト پر عمل کریں تاکہ ایک ایسا プロポーズ تیار ہو جو ان قواعد پر پورا اترے۔

## レジストリ憲章のスナップショット

提案草案 کرنے سے پہلے تصدیق کریں کہ یہ レジストリ憲章 کے مطابق ہے جسے
`sorafs_manifest::chunker_registry::ensure_charter_compliance()` を強制します:

- プロファイル ID 整数 ہوتے ہیں جو بغیر ギャップ کے 単調 طور پر بڑھتے ہیں۔
- エイリアス キャノニカル ハンドル 衝突 衝突 衝突 衝突 衝突 衝突 衝突 衝突 衝突 衝突 衝突
- 空でないエイリアス 空白スペース トリミング トリム ہوں۔

便利な CLI ヘルパー:

```bash
# تمام registered descriptors کی JSON listing (ids, handles, aliases, multihash)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles

# Candidate default profile کے لیے metadata emit کریں (canonical handle + aliases)
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --promote-profile=sorafs.sf1@1.0.0 --json-out=-
```

コマンド提案 レジストリ憲章 マニュアル ガバナンスに関する議論 正規メタデータ マニュアルメタデータ

## 必須のメタデータ

|フィールド |説明 |例 (`sorafs.sf1@1.0.0`) |
|----------|---------------|----------------------------|
| `namespace` |プロファイルの論理グループ化| `sorafs` |
| `name` |人間が読めるラベル| `sf1` |
| `semver` |パラメータセット セマンティックバージョン文字列| `1.0.0` |
| `profile_id` |単調数値識別子 جو プロファイル کے 土地 ہونے پر assign ہوتا ہے۔ ID を予約し、再利用します。 | `1` |
| `profile.min_size` |チャンクの長さ 最小バイト数| `65536` |
| `profile.target_size` |チャンク長 ターゲットバイト数| `262144` |
| `profile.max_size` |チャンクの長さ 最大バイト数| `524288` |
| `profile.break_mask` |ローリング ハッシュ (16 進数) アダプティブ マスク| `0x0000ffff` |
| `profile.polynomial` |歯車多項式定数 (16 進数)۔ | `0x3da3358b4dc173` |
| `gear_seed` | 64 KiB ギア テーブルの導出、シード、 | `sorafs-v1-gear` |
| `chunk_multihash.code` |チャンクごとのダイジェスト、マルチハッシュ コード、 | `0x1f` (ブレイク3-256) |
| `chunk_multihash.digest` |正規のフィクスチャ バンドルのダイジェスト| `13fa...c482` |
| `fixtures_root` |再生成されたフィクスチャの相対ディレクトリ| `fixtures/sorafs_chunker/sorafs.sf1@1.0.0/` |
| `por_seed` |決定論的 PoR サンプリング シード (`splitmix64`) | `0xfeedbeefcafebabe` (例) |

メタデータ 提案ドキュメント 生成されたフィクスチャ セキュリティ レジストリ CLI ツール ガバナンスの自動化 マニュアル相互参照 値の確認チャンクストア マニフェスト CLI `--json-out=-` 計算されたメタデータのレビュー メモ ストリーム

### CLI およびレジストリ タッチポイント

- `sorafs_manifest_chunk_store --profile=<handle>` — 提案されたパラメータ、チャンク メタデータ、マニフェスト ダイジェスト、PoR チェック、およびパラメータ
- `sorafs_manifest_chunk_store --json-out=-` — チャンクストア レポート、標準出力、ストリーム、自動比較。
- `sorafs_manifest_stub --chunker-profile=<handle>` — マニフェストの確認 CAR 計画の正規ハンドル エイリアスの埋め込み ہیں۔
- `sorafs_manifest_stub --plan=-` — `chunk_fetch_specs` フィード変更 オフセット/ダイジェスト検証 ہوں۔

コマンド出力 (ダイジェスト、PoR ルート、マニフェスト ハッシュ) 提案書作成 レビュー担当者による逐語的な再現

## 決定論と検証チェックリスト1. **フィクスチャは再生成されます**
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors \
     --signature-out=fixtures/sorafs_chunker/manifest_signatures.json
   ```
2. **パリティ スイート** — `cargo test -p sorafs_chunker` クロスランゲージ デフ ハーネス (`crates/sorafs_chunker/tests/vectors.rs`) 固定具 緑 ہونا چاہیے۔
3. **ファズ/バックプレッシャーコーパスリプレイ** — `cargo fuzz list` ストリーミング ハーネス (`fuzz/sorafs_chunker`) 再生成されたアセット
4. **取得可能性証明証人が検証します** — `sorafs_manifest_chunk_store --por-sample=<n>` 提案されたプロファイルとルートとフィクスチャ マニフェストの一致を確認します。
5. **CI 予行演習** — `ci/check_sorafs_fixtures.sh` テストスクリプト ٩و نئے フィクスチャ اور موجودہ `manifest_signatures.json` کے ساتھ 成功 ہونا چاہیے۔
6. **クロスランタイム確認** — 再生成された Go/TS バインディングは JSON を消費し、同一のチャンク境界を消費し、ダイジェストは放出します。

コマンド 結果のダイジェスト 提案書 ツール WG 推測作業 ツール WG 推測作業

### マニフェスト/PoR 確認

フィクスチャはマニフェスト パイプラインを再生成し、CAR メタデータと一貫した PoR 証明を再生成します。

```bash
# نئے profile کے ساتھ chunk metadata + PoR validate کریں
cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- \
  --profile=sorafs.sf2@1.0.0 \
  --json-out=- --por-json-out=- fixtures/sorafs_chunker/input.bin

# manifest + CAR generate کریں اور chunk fetch specs capture کریں
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --chunk-fetch-plan-out=chunk_plan.json \
  --manifest-out=sf2.manifest \
  --car-out=sf2.car \
  --json-out=sf2.report.json

# محفوظ fetch plan کے ساتھ دوبارہ چلائیں (stale offsets سے بچاتا ہے)
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- \
  fixtures/sorafs_chunker/input.bin \
  --chunker-profile=sorafs.sf2@1.0.0 \
  --plan=chunk_plan.json --json-out=-
```

入力ファイル フィクスチャ استعمال ہونے والے کسی 代表コーパス سے بدلیں
(1 GiB の決定的ストリーム) 結果のダイジェストと提案と添付ファイル

## 提案テンプレート

提案 `ChunkerProfileProposalV1` Norito 記録 `docs/source/sorafs/proposals/` チェックイン جاتا ہے۔ JSON テンプレートは、次の値を期待されます (値は کریں に置き換えられます):


マッチング マークダウン レポート (`determinism_report`) コマンドの出力、チャンク ダイジェスト、検証、偏差の確認

## ガバナンスのワークフロー

1. **提案書 + 備品 PR 送信 作成済みアセット Norito 提案書 `chunker_registry_data.rs` 更新情報
2. **ツール WG レビュー۔** レビュー担当者の検証チェックリスト دوبارہ چلاتے ہیں اور の確認 کرتے ہیں کہ 提案レジストリ ルール کے مطابق ہے (ID 再利用 نہیں، 決定論が満たされている)۔
3. **議会の封筒** 承認 ہونے کے بعد 議会議員提案ダイジェスト (`blake3("sorafs-chunker-profile-v1" || canonical_bytes)`) پر 署名 کرتے ہیں اور 署名 کو プロフィール封筒 میں 追加 کرتے ہیں جو 備品ساتھ رکھا جاتا ہے۔
4. **レジストリの公開** レジストリのマージ、ドキュメント、フィクスチャの更新 ہوتے ہیں۔デフォルトの CLI プロファイル プロファイル ガバナンス移行準備完了

## オーサリングのヒント

- 2 のべき乗、偶数境界、エッジケースのチャンク動作、
- ギア テーブル シード、人間が判読できる世界的にユニークなデータ、監査証跡の記録
- ベンチマーク アーティファクト (スループットの比較) `docs/source/sorafs/reports/` の結果を参照する。

ロールアウトの管理、運用上の期待、移行台帳の管理、および移行元帳の管理
(`docs/source/sorafs/migration_ledger.md`)۔実行時適合規則
`docs/source/sorafs/chunker_conformance.md` دیکھیں۔