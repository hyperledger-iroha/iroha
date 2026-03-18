---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: リリースプロセス
概要: CLI/SDK リリース ゲート バージョン管理ポリシー 正規リリース ノート
---

# リリースプロセス

SoraFS バイナリ (`sorafs_cli`、`sorafs_fetch`、ヘルパー) SDK クレート
(`sorafs_car`、`sorafs_manifest`、`sorafs_chunker`) ایک ساتھ 船 ہوتے ہیں۔リリース
パイプライン CLI ライブラリ 整列されたライブラリ lint/テスト カバレッジ テスト カバレッジ
下流の消費者が成果物をキャプチャするタグ候補タグ
チェックリスト نلائیں۔

## 0. セキュリティ レビューの承認

テクニカル リリース ゲートのセキュリティ レビュー アーティファクトのキャプチャ:

- SF-6 セキュリティ レビュー メモ ([reports/sf6-security-review](./reports/sf6-security-review.md))
  SHA256 ハッシュ リリース チケット درج کریں۔
- 修復チケットのリンク (مثلاً `governance/tickets/SF6-SR-2026.md`)
  セキュリティ エンジニアリング ツール ワーキング グループ サインオフ承認者
- 修復チェックリストのメモ未解決アイテムのリリース ブロック ہیں۔
- パリティ ハーネス ログ (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  マニフェスト バンドル アップロードする アップロードする
- یہ بھی تصدیق کریں کہ 署名コマンド میں `--identity-token-provider` کے ساتھ
  واضح `--identity-token-audience=<aud>` شامل ہو تاکہ Fulcio スコープ解放証拠 میں Capture ہو۔

ガバナンス リリース 公開 アーティファクト リリース

## 1. ゲートのリリース/テスト

`ci/check_sorafs_cli_release.sh` ヘルパー CLI、SDK クレート、フォーマット、Clippy、テスト
ワークスペース ローカル ターゲット ディレクトリ (`.target`) CI の名前
コンテナーのパーミッションの競合、パーミッションの競合、パーミッションの競合、パーミッションの競合、パーミッションの競合

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

スクリプト アサーション スクリプト:

- `cargo fmt --all -- --check` (ワークスペース)
- `cargo clippy --locked --all-targets` `sorafs_car` 機能 (機能 `cli` 機能)
  `sorafs_manifest` `sorafs_chunker`
- `cargo test --locked --all-targets` 木枠箱

失敗する 失敗する タグ付けする 回帰する 回帰するリリースビルドのメイン
کے ساتھ 連続 رہنا چاہیے؛ブランチをリリースし、チェリーピックを修正します。ゲート
یہ بھی چیک کرتا ہے کہ キーレス署名フラグ (`--identity-token-issuer`、`--identity-token-audience`)
جہاں ضروری ہوں فراہم کیے گئے ہوں؛引数がありません 実行します 失敗します دیتے ہیں۔

## 2. バージョン管理ポリシー

SoraFS CLI/SDK クレートの SemVer 番号:

- `MAJOR`: پہلی 1.0 リリース پیں 導入 ہوتا ہے۔ 1.0 インチ `0.y` マイナーバンプ
  **重大な変更** ٩و ظاہر کرتا ہے، چاہے وہ CLI サーフェス میں ہوں یا Norito スキーマ میں۔
- `PATCH`: バグ修正、ドキュメントのみのリリース、依存関係の更新、観察可能な動作の修正

`sorafs_car`، `sorafs_manifest` اور `sorafs_chunker` کو ہمیشہ ایک ہی バージョン پر رکھیں تاکہ
SDK のダウンストリーム コンシューマは、調整されたバージョン文字列に依存します。バージョンバンプ:

1. 木箱 `Cargo.toml` میں `version =` フィールド اپڈیٹ کریں۔
2. `cargo update -p <crate>@<new-version>` ذریعے `Cargo.lock` 再生成 (ワークスペースの明示的なバージョンでは強制されます)۔
3. ゲートを解放する 古いアーティファクトを解放する

## 3. リリースノート

リリース、マークダウン変更ログ、リリース、マークダウン、変更履歴、CLI、SDK、ガバナンスに影響を与える変更
ハイライトハイライト`docs/examples/sorafs_release_notes.md` テンプレート استعمال کریں (リリース
アーティファクト ディレクトリの詳細 セクションの詳細 具体的な詳細の説明

最小コンテンツ:

- **ハイライト**: CLI および SDK コンシューマーの特集の見出し
- **アップグレード手順**: 貨物の依存関係が変化し、決定的なフィクスチャが再実行され、TL;DR コマンドが実行されます。
- **検証**: コマンド出力ハッシュ、エンベロープ、`ci/check_sorafs_cli_release.sh`、正確なリビジョン、実行、

リリース ノート タグ کے リリース ノート (GitHub リリース本体) を決定的に添付する
生成されたアーティファクトの数

## 4. フックをリリースします。

`scripts/release_sorafs_cli.sh` 署名バンドル 検証概要の生成 リリース
ساتھ 船 ہوتے ہیں۔ラッパーの作成と実行 CLI のビルドの実行 `sorafs_cli manifest sign` の実行
`manifest verify-signature` リプレイ ٩رتا ہے تاکہ タグ付け سے پہلے 失敗 سامنے آ جائیں۔意味:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/site.manifest.to \
  --chunk-plan artifacts/site.chunk_plan.json \
  --chunk-summary artifacts/site.car.json \
  --bundle-out artifacts/release/manifest.bundle.json \
  --signature-out artifacts/release/manifest.sig \
  --identity-token-provider=github-actions \
  --identity-token-audience=sorafs-release \
  --expect-token-hash "$(cat .release/token.hash)"
```

ヒント:- リリース入力 (ペイロード、計画、概要、予想されるトークン ハッシュ) リポジトリ デプロイメント構成のトラック 再現可能なスクリプト`fixtures/sorafs_manifest/ci_sample/` フィクスチャ バンドルの正規レイアウト دکھاتا ہے۔
- CI 自動化 `.github/workflows/sorafs-cli-release.yml` ベースの機能ゲートのリリース ゲートのリリース スクリプト バンドル/署名 ワークフロー アーティファクト アーカイブ アーカイブCI システムの管理 コマンドの順序 (ゲートの解放 -> 署名 -> 検証) 監査ログの生成されたハッシュ
- `manifest.bundle.json`、`manifest.sig`、`manifest.sign.summary.json`、`manifest.verify.summary.json` کو اکٹھا رکھیں؛ یہی パケット ガバナンス通知 میں 参照 ہوتا ہے۔
- 正規フィクスチャのリリース、マニフェストの更新、チャンク プラン、要約の更新 `fixtures/sorafs_manifest/ci_sample/` میں کاپی کریں (اور `docs/examples/sorafs_ci_sample/manifest.template.json` اپڈیٹ) کریں) タグ付け سے پہلے۔下流のオペレーターは、フィクスチャに依存して、バンドルをリリースし、再現します。
- `sorafs_cli proof stream` 制限付きチャネルの検証 実行ログのキャプチャ パケットのリリース パケットの添付 証明ストリーミングのセーフガード
- 正確な `--identity-token-audience` リリース ノートに署名する میں درج کریں؛ガバナンス フルシオの政策 聴衆のクロスチェック

ゲートウェイのリリースと展開の完了 `scripts/sorafs_gateway_self_cert.sh` のリリースマニフェスト バンドル、ポイント、証明候補アーティファクト、一致マッチ:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. タグを付けて公開する

フックをチェックします:

1. `sorafs_cli --version` `sorafs_fetch --version` バイナリ バージョン レポート
2. リリース構成 チェックイン `sorafs_release.toml` (推奨) 構成ファイル 構成ファイル 構成ファイル 配備リポジトリ トラック ہو۔アドホック環境変数CLI کو `--config` (相当) パス パス リリース入力 再現可能なパス
3. 署名付きタグ (推奨) 注釈付きタグ :
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. アーティファクト (CAR バンドル、マニフェスト、証明概要、リリース ノート、証明出力) プロジェクト レジストリのアップロード ガバナンス チェックリスト (展開ガイド: [展開ガイド](./developer-deployment.md)) フォローフィクスチャのリリースとフィクスチャの共有 フィクスチャ リポジトリの共有 オブジェクト ストアのプッシュ フィクスチャの監査 自動化公開バンドル ソース管理の管理 差分の確認
5. ガバナンス チャネル、署名タグ、リリース ノート、マニフェスト バンドル/署名ハッシュ、アーカイブされた `manifest.sign/verify` 概要、認証封筒、リンク、管理CI ジョブ URL (ログ アーカイブ) `ci/check_sorafs_cli_release.sh` اور `scripts/release_sorafs_cli.sh` چلایا۔ガバナンス チケット 監査人の承認 成果物 追跡 トレース`.github/workflows/sorafs-cli-release.yml` ジョブ通知の投稿 アドホック サマリー 記録されたハッシュ リンク

## 6. リリース後のフォローアップ

- バージョンの説明 (クイックスタート、CI テンプレート) バージョンの説明 (クイックスタート、CI テンプレート) バージョンの説明और देखें
- リリース ゲート出力ログ - 監査人 - アーカイブ - - 署名済みアーティファクト - 署名済みアーティファクト

パイプライン リリース サイクル クリティカル SDK クレート ガバナンス担保 ロックステップ ロックステップ リリース サイクル