---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/chunker-registry-rollout-checklist.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: チャンカーレジストリロールアウトチェックリスト
タイトル: SoraFS チャンカー レジストリのロールアウト
Sidebar_label: チャンカーのロールアウト
説明: チャンカー レジストリの更新、ロールアウト、ロールアウト
---

:::note メモ
:::

# SoraFS レジストリのロールアウト

チャンカーのプロフィール プロバイダー入場バンドル レビュー 制作
ガバナンス憲章を推進し、ガバナンス憲章を獲得する
批准する ہو چکا ہو۔

> **対象範囲:** リリース日
> `sorafs_manifest::chunker_registry`、プロバイダー入場エンベロープ、正規
> フィクスチャ バンドル (`fixtures/sorafs_chunker/*`)

## 1. 飛行前の検証

1. フィクスチャは、決定論を生成し、検証します。
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. `docs/source/sorafs/reports/sf1_determinism.md` (یا متعلقہ プロフィール レポート)
   決定論ハッシュ再生成されたアーティファクトの一致
3. یقینی بنائیں کہ `sorafs_manifest::chunker_registry`،
   `ensure_charter_compliance()` コンパイル:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. 提案書類:
   - `docs/source/sorafs/proposals/<profile>.json`
   - 議会議事録エントリ `docs/source/sorafs/council_minutes_*.md`
   - 決定論レポート

## 2. ガバナンスの承認

1. ツーリングワーキンググループのレポートと提案のダイジェスト
   ソラ議会インフラパネル میں پیش کریں۔
2. 承認の詳細
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md` میں ریکارڈ کریں۔
3. 議会が署名した封筒と備品の発行:
   `fixtures/sorafs_chunker/manifest_signatures.json`。
4. ガバナンスフェッチヘルパー: アクセス可能なエンベロープ:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. ステージングのロールアウト

手順の説明 ウォークスルー [ステージング マニフェスト プレイブック](./staging-manifest-playbook) 説明

1. Torii `torii.sorafs` ディスカバリーが有効になり、アドミッション強制が有効になりました
   (`enforce_admission = true`) 展開する
2. 承認されたプロバイダーの入場エンベロープをステージング レジストリ ディレクトリにプッシュする
   جسے `torii.sorafs.discovery.admission.envelopes_dir` 参照 کرتا ہے۔
3. ディスカバリー API プロバイダーの広告、伝播、検証:
   ```bash
   curl -sS http://<torii-host>/v2/sorafs/providers | jq .
   ```
4. ガバナンス ヘッダーとマニフェスト/プラン エンドポイントの演習:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. テレメトリ ダッシュボード (`torii_sorafs_*`) アラート ルール、プロファイル、
   エラー 確認 確認

## 4. 本番環境への展開

1. ステージング手順と本番 Torii ノードの繰り返し
2. アクティベーション ウィンドウ (日付/時刻、猶予期間、ロールバック プラン) オペレータと SDK
   チャンネル アナウンス アナウンス
3. PR マージをリリースします:
   - 更新された備品とエンベロープ
   - 文書の変更 (憲章参照、決定論レポート)
   - ロードマップ/ステータスの更新
4. リリースタグ 署名済みアーティファクト 来歴 アーカイブ アーカイブ

## 5. ロールアウト後の監査

1. ロールアウト 24 時間の最終メトリクス (検出数、フェッチ成功率、エラー)
   ヒストグラム) キャプチャ
2. `status.md` 概要 決定論レポート リンク 更新情報
3. フォローアップ タスク (プロフィール作成ガイダンス) `roadmap.md` درج کریں۔