---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/developer-releases.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4bb528d5395666ed52d8bd0125a9c0a81e771e205795459b2d0e2096c1aeac42
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: developer-releases
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-releases.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


# リリースプロセス

SoraFS のバイナリ（`sorafs_cli`, `sorafs_fetch`, helpers）と SDK クレート
（`sorafs_car`, `sorafs_manifest`, `sorafs_chunker`）は同時に出荷されます。
リリースパイプラインは CLI とライブラリの整合を保ち、lint/test のカバレッジを
保証し、下流の利用者向けにアーティファクトを記録します。候補タグごとに
以下のチェックリストを実行してください。

## 0. セキュリティレビューのサインオフ確認

技術的なリリースゲートを実行する前に、最新のセキュリティレビュー
アーティファクトを収集します:

- 最新の SF-6 セキュリティレビュー・メモをダウンロードし
  ([reports/sf6-security-review](./reports/sf6-security-review.md))、SHA256 ハッシュを
  リリースチケットに記録します。
- リメディエーションチケットへのリンク（例: `governance/tickets/SF6-SR-2026.md`）を
  添付し、Security Engineering と Tooling Working Group の承認者を記録します。
- メモ内のリメディエーションチェックリストがクローズ済みであることを確認します。
  未解決項目がある場合はリリースをブロックします。
- パリティハーネスのログ（`cargo test -p sorafs_car -- --nocapture sorafs_cli::proof_stream::bounded_channels`）を
  マニフェストバンドルと一緒にアップロードできるよう準備します。
- 実行予定の署名コマンドに `--identity-token-provider` と明示的な
  `--identity-token-audience=<aud>` が含まれていることを確認し、Fulcio のスコープが
  リリース証跡に記録されるようにします。

これらのアーティファクトは、ガバナンスへの通知とリリース公開時に含めてください。

## 1. リリース/テストゲートの実行

`ci/check_sorafs_cli_release.sh` ヘルパーは、CLI と SDK クレート全体で
フォーマット、Clippy、テストを実行します。CI コンテナ内での権限衝突を
避けるため、workspace ローカルの target ディレクトリ（`.target`）を使用します。

```bash
CARGO_TARGET_DIR=.target ci/check_sorafs_cli_release.sh
```

スクリプトは次の検証を行います:

- `cargo fmt --all -- --check`（workspace）
- `cargo clippy --locked --all-targets`（`sorafs_car` は `cli` feature を有効化）
  および `sorafs_manifest`, `sorafs_chunker`
- `cargo test --locked --all-targets`（同じクレート群）

いずれかが失敗した場合は、タグ付け前にリグレッションを修正します。
リリースビルドは main と連続している必要があります。リリースブランチに
修正を cherry-pick してはいけません。ゲートは keyless 署名フラグ
（`--identity-token-issuer`, `--identity-token-audience`）の指定も検査し、
不足している場合は失敗します。

## 2. バージョニング方針の適用

SoraFS の CLI/SDK クレートはすべて SemVer を使用します:

- `MAJOR`: 初回 1.0 リリースで導入。1.0 以前の `0.y` のマイナー上げは
  **CLI サーフェスや Norito スキーマの破壊的変更**を意味します。
- `MINOR`: 後方互換の機能追加（新しいコマンド/フラグ、オプションポリシーで
  ゲートされた新しい Norito フィールド、テレメトリ追加）。
- `PATCH`: バグ修正、ドキュメントのみのリリース、観測可能な挙動を変えない
  依存関係更新。

`sorafs_car`、`sorafs_manifest`、`sorafs_chunker` は常に同じバージョンに揃え、
下流 SDK 利用者が単一のバージョン文字列に依存できるようにします。
バージョンを上げる際は:

1. 各クレートの `Cargo.toml` にある `version =` を更新します。
2. `cargo update -p <crate>@<new-version>` で `Cargo.lock` を再生成します
   （workspace は明示的なバージョンを要求）。
3. リリースゲートを再実行し、古いアーティファクトが残っていないことを確認します。

## 3. リリースノートの準備

各リリースは、CLI/SDK/ガバナンスに影響する変更を強調した markdown の
changelog を公開する必要があります。`docs/examples/sorafs_release_notes.md`
のテンプレートを使用し（リリースアーティファクトのディレクトリへコピーして）、
具体的な内容で各セクションを埋めてください。

最低限の内容:

- **Highlights**: CLI/SDK 利用者向けの機能ハイライト。
- **Upgrade steps**: cargo 依存更新と決定的 fixtures 再生成の TL;DR コマンド。
- **Verification**: コマンド出力のハッシュ/エンベロープと、実行した
  `ci/check_sorafs_cli_release.sh` の正確なリビジョン。

完成したリリースノートはタグ（例: GitHub のリリース本文）に添付し、
決定的に生成されたアーティファクトと一緒に保存します。

## 4. リリースフックの実行

`scripts/release_sorafs_cli.sh` を実行し、各リリースに同梱する署名バンドルと
検証サマリーを生成します。ラッパーは必要に応じて CLI をビルドし、
`sorafs_cli manifest sign` を呼び出し、すぐに `manifest verify-signature` を
再実行してタグ付け前に失敗を顕在化させます。例:

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

Tips:

- リリース入力（payload、plans、summaries、期待トークンハッシュ）を repo または
  デプロイ構成に記録して、スクリプトを再現可能にします。
  `fixtures/sorafs_manifest/ci_sample/` の CI fixture bundle が正規レイアウトを示します。
- CI 自動化は `.github/workflows/sorafs-cli-release.yml` を基準にします。リリースゲートを
  実行し、上記スクリプトを呼び出し、bundle/署名を workflow アーティファクトとして保存します。
  他の CI システムでも同じ順序（リリースゲート → 署名 → 検証）を保ち、監査ログが
  生成ハッシュと一致するようにしてください。
- `manifest.bundle.json`, `manifest.sig`, `manifest.sign.summary.json`,
  `manifest.verify.summary.json` はまとめて保管します。ガバナンス通知で参照されるパケットです。
- リリースで正規 fixtures を更新する場合は、更新された manifest、chunk plan、summaries を
  `fixtures/sorafs_manifest/ci_sample/` にコピーし（`docs/examples/sorafs_ci_sample/manifest.template.json`
  も更新）、タグ付け前に反映します。下流の運用者は、コミットされた fixtures に依存して
  リリースバンドルを再現します。
- `sorafs_cli proof stream` の bounded-channel 検証の実行ログを取得し、リリースパケットに
  添付して proof streaming の保護が有効であることを示します。
- 署名に使用した正確な `--identity-token-audience` をリリースノートに記録します。
  ガバナンスは公開承認前に Fulcio ポリシーと照合します。

リリースに gateway のロールアウトが含まれる場合は `scripts/sorafs_gateway_self_cert.sh`
を使用します。同じ manifest bundle を指定して、attestation が候補アーティファクトと
一致することを示します:

```bash
scripts/sorafs_gateway_self_cert.sh --config docs/examples/sorafs_gateway_self_cert.conf \
  --manifest artifacts/site.manifest.to \
  --manifest-bundle artifacts/release/manifest.bundle.json
```

## 5. タグ付けと公開

チェックが通り、フックが完了したら:

1. `sorafs_cli --version` と `sorafs_fetch --version` を実行し、バイナリが新しい
   バージョンを報告することを確認します。
2. チェックインされた `sorafs_release.toml`（推奨）またはデプロイ repo で管理される
   別の設定ファイルにリリース設定を準備します。アドホックな環境変数に頼らず、
   CLI に `--config`（または同等）でパスを渡して、入力が明示的かつ再現可能になるようにします。
3. 署名付きタグ（推奨）または注釈付きタグを作成します:
   ```bash
   git tag -s sorafs-vX.Y.Z -m "SoraFS CLI & SDK vX.Y.Z"
   git push origin sorafs-vX.Y.Z
   ```
4. アーティファクト（CAR バンドル、manifests、proof サマリー、リリースノート、
   attestation 出力）をプロジェクトのレジストリへアップロードし、
   [デプロイガイド](./developer-deployment.md) にあるガバナンスチェックリストに従います。
   新しい fixtures を生成した場合は、共有 fixture repo または object store に反映し、
   監査自動化が公開バンドルをソースコントロールと照合できるようにします。
5. ガバナンスチャンネルに、署名タグ、リリースノート、manifest の bundle/署名ハッシュ、
   `manifest.sign/verify` のアーカイブ済みサマリー、attestation エンベロープへのリンクを通知します。
   `ci/check_sorafs_cli_release.sh` と `scripts/release_sorafs_cli.sh` を実行した CI job の URL
   （またはログアーカイブ）を含めてください。監査担当が承認をアーティファクトに紐付けられるよう、
   ガバナンスチケットを更新します。`.github/workflows/sorafs-cli-release.yml` の job が通知を
   投稿する場合は、アドホックなサマリーではなく記録済みのハッシュをリンクしてください。

## 6. リリース後フォローアップ

- 新バージョンを参照するドキュメント（quickstart、CI テンプレート）を更新するか、
  変更不要であることを確認します。
- リリースゲートの出力ログを監査用に保管し、署名済みアーティファクトと並べて保存します。

このプロセスを守ることで、CLI、SDK クレート、ガバナンス関連資料が各リリースサイクルで
足並みを揃えられます。
