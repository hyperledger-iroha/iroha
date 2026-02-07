---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: リリースのプロセス
概要: CLI/SDK のリリース ゲートを実行し、バージョン管理のためのアプリケーションとリリース基準のノートを公開します。
---

# リリースのプロセス

ビネール SoraFS (`sorafs_cli`、`sorafs_fetch`、ヘルパー) およびクレート SDK
(`sorafs_car`、`sorafs_manifest`、`sorafs_chunker`) ソン リブレ アンサンブル。ルパイプライン
CLI および図書館情報のリリース、クーベルチュールのリント/テストの保証
加工品を捕獲し、コンソマチュールを下流に流し込みます。チェックリストを実行する
ci-dessous pour Chaque タグ候補。

## 0. レビューセキュリティの確認者

リリース前にゲート技術を実行し、アーティファクトをキャプチャします
レビューセキュリティ:

- SF-6 の安全性レビューに関する情報 ([reports/sf6-security-review](./reports/sf6-security-review.md))
  チケットのリリース時にハッシュ SHA256 を登録します。
- Joignez le lien du ticket de remédiation (par ex. `governance/tickets/SF6-SR-2026.md`) およびメモ
  セキュリティ エンジニアリングおよびツール ワーキング グループの承認。
- 修復のチェックリストをメモ的に検証します。解放されたブロックを解放しないでください。
- ハーネスのログをアップロードする前に (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  マニフェストの平均バンドル。
- `--identity-token-provider` などの署名を含む実行者の署名を確認します
  Un `--identity-token-audience=<aud>` では、キャプチャー ファイルのスコープを明示的にリリースする必要があります。

政府および出版物の通知に関する成果物が含まれます。

## 1. リリース/テストのゲートの実行者

ヘルパー `ci/check_sorafs_cli_release.sh` ファイルのフォーマット、Clippy などのテストを実行します
CLI と SDK のレパートリー ターゲットのローカル au ワークスペースの詳細 (`.target`)
コンテニュール CI の実行時に権限を与えてください。

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

スクリプトのアサーションの効果:

- `cargo fmt --all -- --check` (ワークスペース)
- `cargo clippy --locked --all-targets` は `sorafs_car` を注ぎます (平均機能 `cli`)、
  `sorafs_manifest` と `sorafs_chunker`
- `cargo test --locked --all-targets` ケースを注ぎます

Si une étape ecchoue, corrigez la regression avant de tagger.リリースのビルド
doivent être continuus avec main ;枝のチェリーピケを正しく修正してください
解放する。キーレス署名のオーストラリア国旗検証 (`--identity-token-issuer`、
`--identity-token-audience`) ソント・フォーニス・クアンドが必要です。 les argument manquants フォント
エシュエ・レ・エグゼキュション。

## 2. バージョン管理の政治的なアップリケ

CLI/SDK SoraFS utilisent SemVer のタスク:

- `MAJOR` : プレミア リリース 1.0 の紹介。アバント 1.0、ル バンプ マイナー `0.y`
  **カサントの変更点** CLI の表面とスキーマ Norito を確認します。
- `MINOR` : Nouvelles fonctionnalités (ヌーヴォー コマンド/フラグ、ヌーヴォー チャンピオン Norito)
  政治的オプション、テレメトリーに関する情報）。
- `PATCH` : バグを修正し、独自のドキュメントを定期的にリリース
  観察可能な範囲の変更を加えた依存。

Gardez toujours `sorafs_car`、`sorafs_manifest` および `sorafs_chunker` のミーム バージョン
コンソマトゥール SDK のダウンストリーム バージョンを依存するように注ぐ
アライン。バージョンの説明:

1. メテズ・ア・ジュール・レ・シャン `version =` とチャック `Cargo.toml`。
2. `cargo update -p <crate>@<new-version>` 経由で `Cargo.lock` を更新 (ワークスペース
   des バージョンを明示的に適用します)。
3. アーティファクトのペリメを解放するためのゲートを開きます。

## 3. リリースノートの準備

Chaque リリースでは、変更履歴とマークダウンのメタントが公開されます。
CLI、SDK、および統治に影響を与えます。テンプレートを活用する
`docs/examples/sorafs_release_notes.md` (芸術品のレパートリーをコピー
リリースと詳細セクションの詳細)。

連続最小値:

- **ハイライト** : CLI および SDK の機能強化。
- **互換性** : 急激な変更、政治的なアップグレード、必要最低限の機能
  ゲートウェイ/ヌード。
- **Étapes d'upgrade** : TL;DR の貨物輸送の命令
  リランサー・レ・フィクスチャー・デターミニスト。
- **検証** : 封筒のハッシュと改訂の正確性
  `ci/check_sorafs_cli_release.sh` 執行者。

Joignez les Notes de release au タグ (par ex. corps de la release GitHub) など
Stockez-les à côté des artefacts générés de façon deterministe。

## 4. 解放の執行者Exécutez `scripts/release_sorafs_cli.sh` は署名などのバンドルを生成します
履歴書の検証履歴は、最新のリリース情報を参照してください。 CLI si を構成するラッパー
必要性、控訴 `sorafs_cli manifest sign` および即時救済
`manifest verify-signature` は、前衛的なタグを表示します。例:

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

ヒント:

- リリースの入力情報 (ペイロード、計画、概要、トークンのハッシュ)
  再作成可能なスクリプトの展開設定をリポジトリに保存します。
  ル バンドル CI sous `fixtures/sorafs_manifest/ci_sample/` モントレ ル レイアウト カノニーク。
- Basez l'automatization CI sur `.github/workflows/sorafs-cli-release.yml` ;エル・エグゼキュート
  リリースゲート、スクリプト呼び出し、ci-dessus、およびアーカイブバンドル/署名の実行
  ワークフローのアーティファクト。 Reproduisez le même ordre de commandes (門 → 署名 →
  検証) ダン ドートル システム CI は、アライナー レ ログ ドディット アベック レ ハッシュを注ぎます。
- Gardez `manifest.bundle.json`、`manifest.sig`、`manifest.sign.summary.json` など
  `manifest.verify.summary.json` アンサンブル : イルス・フォーメント・ル・パケット・レフェレンス・ダン・ラ
  ガバナンス通知。
- 正規の備品のリリースと、マニフェストのコピー、
  チャンク プランと概要の `fixtures/sorafs_manifest/ci_sample/` (メッツ
  à jour `docs/examples/sorafs_ci_sample/manifest.template.json`) 前タグ。レ
  下流の設備管理委員会はバンドルの再現を担当します。
- 境界チャネルの検証の実行ログをキャプチャします。
  `sorafs_cli proof stream` および joignez-le au paquet de release pour démontler que les
  厳重な証拠のストリーミング待機活動。
- Notez l'`--identity-token-audience` 正確な utilisé lors de la signed dans les Notes
  リリース; la gouvernance recoupe l'audience avec la politique Fulcio avant approbation。

ロールアウトを含む `scripts/sorafs_gateway_self_cert.sh` のリリースを利用
ゲートウェイ。 Pointez-le sur le même バンドル デ マニフェスト プルーバー クエリ アテステーション
アーティファクト候補に対応します:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. タガーと出版社

小切手の通過とフックの終了後:

1. Exécutez `sorafs_cli --version` et `sorafs_fetch --version` pourconfirmer que les binaires
   リポート・ラ・ヌーベルバージョン。
2. `sorafs_release.toml` バージョンのリリース設定を準備します (事前)
   あなたは、開発レポートの投票のための構成管理を行っています。エビテス・デ・デペンドル
   アドホックな環境変数。 passez les chemins au CLI avec `--config` (ou
   同等) 明示的かつ再現可能な入力を定義します。
3. タグ署名 (プレフェレ) またはタグ注釈の作成:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. アーティファクトのアップロード (CAR、マニフェスト、証明の履歴書、リリースのメモ、
   証明書の出力) とプロジェクトセロンのレジストリ、行政チェックリスト
   ダン ル [導入ガイド](./developer-deployment.md)。製品をリリースします
   新しい備品、プセ・レ・ヴェル・レポ・デ・備品のパーツ・オブジェクト・ストア
   ソース管理のバンドル公開を監査する自動化機能を提供します。
5. 統治権に関する通知、タグ署名、リリースに関する通知、
   バンドルのハッシュ/マニフェストの署名、履歴書アーカイブ `manifest.sign/verify`
   証明書の封筒を宣伝します。ジョブ CI (ログのアーカイブ) の URL を含めます
   `ci/check_sorafs_cli_release.sh` と `scripts/release_sorafs_cli.sh` を実行します。メテス・ア
   定期審査チケットを審査して承認を得る
   補助アーティファクト ; lorsque `.github/workflows/sorafs-cli-release.yml` 特使、
   履歴書はその場限りで登録されます。

## 6. スイヴィリリース後

- ドキュメントの重要なバージョンと新しいバージョンを保証する (クイックスタート、テンプレート CI)
  変更が必要でないことを確認してください。
- 必要な作業のロードマップを作成する (移行フラグなど)
- Archivez les logs de sortie du Gate de release pour les Auditeurs : Stockez-les à côté des
  工芸品のサイン。

CLI、SDK およびガバナンス管理のためのパイプライン保守管理
チャックサイクルとリリースを調整します。