---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: リリースのプロセス
概要: CLI/SDK のリリースゲートを実行し、バージョン管理のポリシーとリリース正規版の公開ノートを作成します。
---

# リリースのプロセス

SoraFS (`sorafs_cli`、`sorafs_fetch`、ヘルパー) の SDK のバイナリ
(`sorafs_car`、`sorafs_manifest`、`sorafs_chunker`) sao はジュントを巻き込みます。 O パイプライン
CLI を公開文書としてリリース管理し、lint/test を保証します。
下流のコンスミドールのキャプチャー・アルテファト。チェックリストを実行する
タグ候補。

## 0. セグランサの見直しを確認する

リリース前に実行者とゲート技術を実行し、最近の最新データをキャプチャします。
セグランカの見直し:

- 最近の SF-6 の見直しメモ ([reports/sf6-security-review](./reports/sf6-security-review.md))
  ハッシュ SHA256 を登録し、リリースのチケットはありません。
- 救済チケットへのリンク (例、`governance/tickets/SF6-SR-2026.md`) および注記
  セキュリティ エンジニアリングおよびツール ワーキング グループの開発を担当します。
- 救済策のチェックリストをメモなしで確認します。イテンス・ペンダント・ブロック・オ・リリース。
- 実行ログをアップロードして利用する準備をします (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  マニフェストのバンドルを順に実行します。
- `--identity-token-provider` e を含む実行コマンドを確認してください。
  UM `--identity-token-audience=<aud>` は、リリースに関する証拠を明示的に取得します。

政府通知および公開リリースの成果物を含めます。

## 1. 釈放/テストの執行者

O ヘルパー `ci/check_sorafs_cli_release.sh` フォーマット、Clippy と精巣の箱
CLI および SDK com um diretorio ターゲット ローカル Ao ワークスペース (`.target`) パラメータ conflitos
コンテナー CI の実行権限を許可します。

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

検証を続行するためのスクリプト作成:

- `cargo fmt --all -- --check` (ワークスペース)
- `cargo clippy --locked --all-targets` パラ `sorafs_car` (機能 `cli`)、
  `sorafs_manifest` e `sorafs_chunker`
- `cargo test --locked --all-targets` パラエッセ メスモス クレート

Se algum passo falhar, corrija a regressao antes de taguear.ビルドとリリース開発
ser continuos com main;リリースの枝からチェリーピックを選択することもできます。 ○
ゲート タンベム検証 OS フラグ デ アシナチュア セム チャベ (`--identity-token-issuer`,
`--identity-token-audience`) フォルネシドス クアンド アプリカベルを形成します。アルギュロス・ファルタンド
ファゼム・ア・エクスエクカオ・ファルハール。

## 2. バージョン管理の政治的適用

Todos OS は SoraFS の CLI/SDK を使用して SemVer:

- `MAJOR`: リリース 1.0 の紹介。 1.0 メートルのオーメント メナーの準備
  `0.y` **インディカ ムダンカス ケブラドーラス** は、CLI を超えていない Norito。
- `MINOR`: Trabalho de 機能の比較パラトラ (ノボ コマンド/フラグ、ノボ
  Campos Norito protegidos por politica opcional、adicoes de telemetria)。
- `PATCH`: バグを修正し、ドキュメントおよび最新のバージョンをリリースします。
  依存性は、観察に応じて変化します。

マンテンハ センペル `sorafs_car`、`sorafs_manifest` および `sorafs_chunker` のメスマ バージョン
SDK のダウンストリーム possam 依存関係の uma unica 文字列のパラケ オス コンスミドール
ヴァーサオ・アリンハダ。 Ao Atualizar バージョン:

1. OS `version =` を `Cargo.toml` に設定します。
2. `cargo update -p <crate>@<new-version>` 経由で `Cargo.lock` を再生成します (ワークスペース
   exige versoes explicitas)。
3. 保証を解除するためのゲート デ リリースを実行します。

## 3. リリースノートの準備

CLI、SDK の変更履歴のマークダウン クエリのリリース開発を公開
インパクト・デ・ガバナンカ。テンプレートem `docs/examples/sorafs_release_notes.md`を使用してください
(リリース情報を保存するための保存ファイルをコピーし、詳細情報を取得します)
コンクリート）。

コンテウド・ミニモ:

- **ハイライト**: CLI および SDK の管理に関する機能のマンシェット。
- **互換性**: ムダンカス ケブラドーラス、政治のアップグレード、ミニモスの要求
  ゲートウェイ/ノード。
- **アップグレード手順**: コマンド TL;DR パラメタリザル依存性貨物と refazer
  フィクスチャの決定性。
- **Verificacao**: 封筒のハッシュと再確認
  `ci/check_sorafs_cli_release.sh` 実行します。

リリース前のタグとしての付録 (例、GitHub のリリース文書)
決定的な決定性を備えた最新の情報を提供します。

## 4. Executar フックの解放

Rode `scripts/release_sorafs_cli.sh` パラジェラール・デ・アシナチュア・オ・レスモデ
verificacao que acompanham cada release.ラッパーのコンパイルや CLI の必要性、
チャマ `sorafs_cli manifest sign` すぐに再実行 `manifest verify-signature`
para que falhas aparecam antes はタグ付けを行います。例:

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

ディカス:- リリース入力の登録 (ペイロード、計画、概要、トークンのハッシュエスペラード)
  デプロイメントパラメータやスクリプトの再現などのリポジトリは必要ありません。 ○
  フィクスチャのバンドル `fixtures/sorafs_manifest/ci_sample/` ほとんどのレイアウト カノニコ。
- CI em `.github/workflows/sorafs-cli-release.yml` を自動化します。エレ・ロダ・オー
  ゲート デ リリース、チャマ、スクリプト、ACIMA、ARQUIVA バンドル/アシナチュア コモ アートファト
  ワークフローを実行します。 Mantenha a mesma ordem de comandos (ゲート・デ・リリース -> assinatura ->
  Verificacao) EM アウトロス システム CI パラケ オス ログ デ オーディトリア バタム COM ハッシュ
  ゲラドス。
- 満天波 `manifest.bundle.json`、`manifest.sig`、`manifest.sign.summary.json` e
  `manifest.verify.summary.json` ジュントス。参照形式に関する情報
  政府通知。
- Quando リリース atualizar フィクスチャ canonicos、コピー、マニフェスト atualizado、o
  `fixtures/sorafs_manifest/ci_sample/` に関するチャンク プランの OS 概要 (英語)
  `docs/examples/sorafs_ci_sample/manifest.template.json`) タグ付き。オペラドール
  ダウンストリームの依存関係のある Dos フィクスチャは、リリースのバンドルの複製にコミットされます。
- 境界チャネルの実行および検証のログをキャプチャします。
  `sorafs_cli proof stream` 別のパコートとしてデモをリリースします
  証拠ストリーミングの継続的なサルバガード。
- `--identity-token-audience` を持続的に使用するための記録
  リリースノート。政府は、聴衆を集めて、フルシオの政治を行う
  パブリックカオの承認。

`scripts/sorafs_gateway_self_cert.sh` を使用して、ロールアウトを含むリリースを解除します
デ・ゲートウェイ。証明に必要なマニフェストの管理バンドルを受け取る
候補者候補:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. publicacao にタグを付ける

Depois que os checks passarem e os Hooks forem concluiidos:

1. Rode `sorafs_cli --version` と `sorafs_fetch --version` はバイナリオを確認します
   nova versao をレポートします。
2. リリースエミュレーション `sorafs_release.toml` バージョン (優先) の構成を準備します。
   デプロイメントの構成を確認したり、インストールしたり、リポジトリを作成したりできます。依存者をエビトする
   アドホックなアンビエントのバリエーション。 CLI コム `--config` (ou
   同等) パラケ OS 入力は、SEJAM 明示的再現をリリースします。
3. 叫びなさい、タグ・アッシナド (プレフェリド) またはアノタド:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. 技術的なアップロード機能 (CAR、マニフェスト、証明履歴書、リリースノート、
   認証の出力) レジストリのプロジェクト管理のチェックリストの作成
   [展開ガイド](./developer-deployment.md) はありません。 Gerou novas のフィクスチャーをリリースし、
   envie-as パラオ リポジトリ フィクスチャのコンパートメントとオブジェクト ストア パラケラ オートマカオ
   オーディオコンシガと比較して、パブリックコードをバンドルし、コードを制御します。
5. 政府機関の通信リンクに関する通知、暗殺行為、リリース通知、ハッシュのタグ付け
   バンドル/アシナチュアを実行し、マニフェストを実行し、`manifest.sign/verify` のコマンドを実行します。
   Quaisquer 封筒の認証。 CI のジョブを実行する URL (ログの保存) を含める
   que Rodou `ci/check_sorafs_cli_release.sh` e `scripts/release_sorafs_cli.sh`。実現する
   o ポッサム・ラストレア・アプロヴァコスが芸術品を食べたときのオーディトレス・パラ・ガバナンカ・チケット。
   Quando o job `.github/workflows/sorafs-cli-release.yml` 公開通知、リンク
   OS は、アドホックなレジストラのハッシュを記録します。

## 6. リリース後

- 新しいバージョンのドキュメントを作成する (クイックスタート、CI テンプレート)
  エステジャは、必要性を確認し、必要性を確認します。
- 必要なロードマップを登録しないでください (例、フラグなど)
- OS ログをアーカイブし、聴覚的にゲートを解放します - Guarde-OS のアオラド ドス アートファトス
  アジナド。

パイプライン管理、CLI、SDK の管理、アリニャドス管理の資料の管理
リリースが完了しました。