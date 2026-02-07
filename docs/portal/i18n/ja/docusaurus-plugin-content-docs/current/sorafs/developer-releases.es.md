---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: ランサミエントの手続き
概要: CLI/SDK の出口とランザミエントの公開情報、バージョン比較の政治アプリ。
---

# プロセソ・デ・ランサミエント

SoraFS (`sorafs_cli`、`sorafs_fetch`、ヘルパー) の SDK の損失
(`sorafs_car`、`sorafs_manifest`、`sorafs_chunker`) パブリック・ジュント。エルパイプライン
ランザミエント マンティエン エル CLI と las bibliotecas alineados、asegura cobertura de
lint/test y キャプチャ アーティファクト パラ コンスミドール ダウンストリーム。エジェクタ ラ リスタ デ
タグ候補の検証。

## 0. 不正行為の見直しを確認する

出口までのアクセス、ランサミエントの技術情報のキャプチャ
セグリダードの改訂履歴:

- SF-6 の見直しに関するメモを削除 ([reports/sf6-security-review](./reports/sf6-security-review.md))
  ランザミエントのチケットのハッシュ SHA256 を登録します。
- 救済チケットの補助 (例、`governance/tickets/SF6-SR-2026.md`) とその内容
  セキュリティ エンジニアリングとツール ワーキング グループの概要。
- 修復メモのリストを確認し、セラダを確認します。ロス・テムス・シン・リゾルバー・ブロックアン・エル・ランザミエント。
- ハーネスのログを準備してください (`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`)
  マニフェストのバンドルを調整します。
- タント `--identity-token-provider` コモを含む飛行機の脱出コマンドを確認してください
  Un `--identity-token-audience=<aud>` は、リリースの証拠となるキャプチャを明示的に示します。

政府の通知およびランサミエントの公開情報を含めます。

## 1. ランザミエント/プルエバスの出口

El ヘルパー `ci/check_sorafs_cli_release.sh` 出力フォーマット、Clippy y テスト
CLI と SDK のディレクトリ ターゲットのローカル ワークスペースの損失が深刻です (`.target`)
パラ エビタール コンフリクト デ パーミソス アル エジェキュタルス デントロ デ コンテネドール CI。

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

El script realiza las siguientes comprobaciones:

- `cargo fmt --all -- --check` (ワークスペース)
- `cargo clippy --locked --all-targets` パラ `sorafs_car` (機能 `cli`)、
  `sorafs_manifest` y `sorafs_chunker`
- `cargo test --locked --all-targets` パラ エソス ミスモス クレート

失敗した場合は、倫理規定を修正してください。リリース後のビルドの損失
デベン・エスター・コンメイン。リリース時に修正をチェリーピックする必要はありません。
クラベスのフラグを確認してください (`--identity-token-issuer`,
`--identity-token-audience`) 比例して対応します。ロス・アーギュロス
ファルタンテス・ハセン・フォールラル・ラ・エジェクシオン。

## 2. 政治的バージョンの適用

SoraFS の SemVer を使用した CLI/SDK のタスク:

- `MAJOR`: パラエルプライマーリリース 1.0 を導入します。 1.0 エルインクリメントのアンテス
  menor `0.y` **インディカ カンビオス コン ルプチュラ** en la superficie del CLI o en los
  エスケマスNorito。
- `MINOR`: 破裂機能のトラバホ (ヌエボス コマンド/フラグ、ヌエボス)
  カンポス Norito 政治オプショナル、遠隔測定のプロテギドス)。
- `PATCH`: バグの修正、ドキュメントおよび実際の単独のリリース
  カンビアンと同胞は観察できない依存性。

管理 `sorafs_car`、`sorafs_manifest` および `sorafs_chunker` アン ラ ミスマ バージョン
SDK のダウンストリーム プエダンに依存するパラ ケ ロス コンスミドール デ ウナ カデナ
アリネアダ。すべての増分バージョン:

1. 実際は `version =` と `Cargo.toml` です。
2. `cargo update -p <crate>@<new-version>` 経由で `Cargo.lock` を再生成 (ワークスペースから)
   exige versiones explícitas)。
3. ランサミエントの門を出て、芸術品のない宮殿を出てください
   時代遅れ。

## 3. ランサミエントの準備をする

Cada release debe publicar un changelog en markdown que resalte cambios que
CLI、SDK、およびゴーベルナンザに影響を与えます。アメリカ・ラ・プランティラ・エン
`docs/examples/sorafs_release_notes.md` (成果物ディレクトリのコピー)
具体的なセクションを完了してください)。

ミニモの内容:

- **デスタカード**: CLI および SDK のコンスミドールの機能のタイトル。
- **影響**: 混乱、政治のアップグレード、要求事項
  ゲートウェイ/ノードの最小限。
- **現実化の手順**: TL;DR パラメタ現実化依存関係の貨物とコマンド
  リージェキュターの試合結果は決定者。
- **検証**: 正確な改訂を実行するためのコマンドのハッシュ
  `ci/check_sorafs_cli_release.sh` を出力します。

ランサミエントの完全なタグの付属品 (一時停止、リリースの解除)
en GitHub) と、決定的な形式の生成を決定するための準備です。

## 4. エジェクターロスフックデリリースEjecuta `scripts/release_sorafs_cli.sh` 一般的な企業バンドル
リリースに関する検証を再開します。ラッパーコンパイルと CLI
cuando es necesario、ラマ `sorafs_cli manifest sign` y メディアを再現します
`manifest verify-signature` パラケロスファロスアパレスカンアンテスデエチケット。
例:

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

コンセホス:

- レジストラのリリース入力 (ペイロード、計画、要約、トークンエスペラードのハッシュ)
  スクリプト全体のリポジトリ設定は再現可能です。エルバンドル
  `fixtures/sorafs_manifest/ci_sample/` のレイアウトの固定具。
- CI の自動化システム `.github/workflows/sorafs-cli-release.yml`。エジェクタ
  リリースゲート、アーカイブバンドル/ファーム共通のスクリプトの呼び出し
  ワークフローのアーティファクト。 Refleja el missmo orden de comandos (ゲート デ リリース → ファームア)
  → verificar) en otros sistemas CI para que los logs de Auditoría concidan con los
  ハッシュジェネラド。
- マンテン ジュントス `manifest.bundle.json`、`manifest.sig`、`manifest.sign.summary.json`
  y `manifest.verify.summary.json`;通知と書式の参照
  デ・ゴベルナンザ。
- Cuando el releaseactualice fixtures canónicos、コピア el マニフェストactualizado、
  チャンク プランとロスの概要 en `fixtures/sorafs_manifest/ci_sample/` (実際の
  `docs/examples/sorafs_ci_sample/manifest.template.json`) 安全策。
  ロス オペラドールのダウンストリームは、ロス フィクスチャのバージョンに応じて再現されます。
  エルバンドルリリース。
- カナレスの検証結果を記録するキャプチャ
  `sorafs_cli proof stream` はデモ用のリリースに関する追加パッケージです
  証拠ストリーミングシグエンアクティバスのサルバガード。
- Registra el `--identity-token-audience` 正確な米国の会社の期間
  ランサミエントのノート。フルシオの政治に対する聴衆の検証
  公開前に。

米国 `scripts/sorafs_gateway_self_cert.sh` は、国連を含むリリースを取得します
ゲートウェイのロールアウト。プロバール・ケ・ラのマニフェスト・バンドルを確認する
芸術品の候補と一致する可能性:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. エチケットと広報

フックのチェックが完了していることを確認します:

1. Ejecuta `sorafs_cli --version` y `sorafs_fetch --version` paraconfirmar que los binarios
   ルタン・ラ・ヌエバ版。
2. `sorafs_release.toml` バージョンのリリース構成を準備します (優先)
   rastreado 設定のアーカイブ、レポートの保存。エビータ依存者
   エンターノアドホック変数。 PASA RUTAS AL CLI con `--config` (同等) パラメータ
   ショーンの明示的な再現可能性を解放するための入力が失われます。
3. タグ ファームド (preferido) を作成し、タグ アノタードを作成します。
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. 完全な成果物 (CAR、マニフェスト、証明の概要、リリースの注意事項、
   アスタシオンの出力) プロジェクト シギエンドおよびゴベルナンザのチェックリストのすべてのレジストリ
   en la [guía de despliegue](./developer-deployment.md)。新しい世代のリリース
   フィクスチャ、オブジェクト ストア パラケラのフィクスチャのリポジトリを保存
   オーディオ プエダの自動制御バンドルの比較および制御の自動化
   バージョン。
5. タグファームド、リリースノート、ハッシュに関する通知を行う
   バンドル/マニフェストのファイル、`manifest.sign/verify` のアーカイブの履歴
   cualquier envoltorio de astación。ジョブCI (またはログのアーカイブ) クエリのURLを含める
   `ci/check_sorafs_cli_release.sh` と `scripts/release_sorafs_cli.sh` を取り出します。アクチュアリーザ
   エル チケット デ ゴベルナンサ パラ ケ ロス アウディトレス プエダン トラザール ラス アプロバシオネス ア ロス
   工芸品。クアンド エル ジョブ `.github/workflows/sorafs-cli-release.yml` パブリック
   通知、一時的な再開のハッシュ登録。

## 6. 後部リリースのセギミエント

- 新しいバージョンのドキュメントを作成する (クイックスタート、CI のプランティージャ)
  事実を確認するためには、カンビオが必要です。
- 事後管理が必要なロードマップの登録 (記録、フラグ)
- アーカイブ ログ デ サリダ デル ゲート デ リリース パラ オーディオ: guárdalos junto a los
  アーティファクト・フィルマドス。

Seguir エステ パイプライン マンティエン エル CLI、ロス デル SDK および ゴベルナンザ マテリアル
リリースに関する最新情報。